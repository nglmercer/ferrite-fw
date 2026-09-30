//! Bounded active image/read work with cancellation on dropped waits. No
//! browser or context is captured by image/read jobs. Baseline staging jobs
//! write only temporary files; their caller owns foreground installation.
//! The admission limit bounds active callbacks, not Tokio's blocking threads
//! or queued input bytes. Codec, resize and OS calls are opaque phases; a
//! dropped wait stops subsequent phases and discards their eventual results.
use crate::{E2eError, E2eResult};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Condvar, Mutex,
};
use std::time::Duration;

struct Admission {
    active: Mutex<usize>,
    changed: Condvar,
}
static ADMISSION: Admission = Admission {
    active: Mutex::new(0),
    changed: Condvar::new(),
};
const ACTIVE_LIMIT: usize = 2;
// Tests that deliberately hold every admission slot must not each retain one
// slot while waiting for the other. Ordinary production jobs never do this.
#[cfg(test)]
pub(crate) static TEST_ADMISSION_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Clone)]
pub(crate) struct StopCheck(Arc<AtomicBool>);
impl StopCheck {
    pub(crate) fn check(&self) -> E2eResult<()> {
        if self.0.load(Ordering::Acquire) {
            Err(E2eError::Cancelled(
                "snapshot CPU work was abandoned".into(),
            ))
        } else {
            Ok(())
        }
    }
}
struct StopGuard(StopCheck);
impl Drop for StopGuard {
    fn drop(&mut self) {
        self.0 .0.store(true, Ordering::Release);
        ADMISSION.changed.notify_all();
    }
}
struct Permit;
impl Permit {
    fn acquire(stop: &StopCheck) -> E2eResult<Self> {
        let mut active = ADMISSION.active.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            stop.check()?;
            if *active < ACTIVE_LIMIT {
                *active += 1;
                return Ok(Self);
            }
            // A timed wait also covers cancellation between checking the flag
            // and entering the condvar wait; no notification can strand a job.
            active = ADMISSION
                .changed
                .wait_timeout(active, Duration::from_millis(10))
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        let mut active = ADMISSION.active.lock().unwrap_or_else(|e| e.into_inner());
        *active -= 1;
        ADMISSION.changed.notify_all();
    }
}
struct Job<T>(tokio::task::JoinHandle<E2eResult<T>>);
impl<T> Drop for Job<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}

pub(crate) fn run<T, F>(work: F) -> impl std::future::Future<Output = E2eResult<T>>
where
    T: Send + 'static,
    F: FnOnce(StopCheck) -> E2eResult<T> + Send + 'static,
{
    // Keep a potentially large callback out of the async future before its
    // first poll, just as the generic assertion probe wrapper does.
    let work = Box::new(work);
    async move {
        let stop = StopCheck(Arc::new(AtomicBool::new(false)));
        let _stop = StopGuard(stop.clone());
        let mut job = Job(tokio::task::spawn_blocking(move || {
            let _permit = Permit::acquire(&stop)?;
            stop.check()?;
            work(stop)
        }));
        (&mut job.0)
            .await
            .map_err(|error| E2eError::Config(format!("snapshot CPU task failed: {error}")))?
    }
}

/// Read only regular baselines and cap actual bytes, including files that grow
/// after metadata inspection. The worker owns no browser or report state.
pub(crate) fn read_baseline(
    path: std::path::PathBuf,
) -> impl std::future::Future<Output = E2eResult<Option<Arc<Vec<u8>>>>> {
    run(move |stop| {
        use std::io::Read;
        const LIMIT: usize = 512 * 1024 * 1024;
        stop.check()?;
        let metadata = match std::fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let validate = |metadata: &std::fs::Metadata| -> E2eResult<()> {
            if !metadata.is_file() {
                return Err(E2eError::Config(
                    "screenshot baseline must be a regular file".into(),
                ));
            }
            if metadata.len() > LIMIT as u64 {
                return Err(E2eError::Config(
                    "screenshot baseline exceeds the 512 MiB encoded input limit".into(),
                ));
            }
            Ok(())
        };
        validate(&metadata)?;
        stop.check()?;
        let mut file = match std::fs::File::open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        stop.check()?;
        validate(&file.metadata()?)?;
        let mut bytes = Vec::with_capacity((metadata.len().min(65_536)) as usize);
        let mut buffer = [0; 65_536];
        loop {
            stop.check()?;
            let max = buffer.len().min(LIMIT - bytes.len() + 1);
            let read = file.read(&mut buffer[..max])?;
            if read == 0 {
                break;
            }
            if bytes.len() + read > LIMIT {
                return Err(E2eError::Config(
                    "screenshot baseline exceeds the 512 MiB encoded input limit".into(),
                ));
            }
            if bytes.capacity() < bytes.len() + read {
                // Control capacity explicitly: the overflow sentinel must not
                // cause Vec's geometric growth to allocate a full extra GiB.
                let capacity = (bytes.len() + read).next_power_of_two().min(LIMIT);
                bytes.reserve_exact(capacity - bytes.len());
            }
            bytes.extend_from_slice(&buffer[..read]);
        }
        stop.check()?;
        Ok(Some(Arc::new(bytes)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[tokio::test]
    async fn baseline_reads_preserve_bytes_and_reject_oversized_and_non_regular_inputs() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("baseline.png");
        assert!(read_baseline(path.clone()).await.unwrap().is_none());
        std::fs::write(&path, []).unwrap();
        assert!(read_baseline(path.clone())
            .await
            .unwrap()
            .unwrap()
            .is_empty());
        let bytes: Vec<u8> = (0..200_000).map(|index| (index % 251) as u8).collect();
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(
            read_baseline(path.clone()).await.unwrap().unwrap().as_ref(),
            &bytes
        );

        // A sparse file tests the metadata rejection without allocating or
        // reading hundreds of MiB. Rejection also leaves its contents intact.
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(512 * 1024 * 1024 + 1).unwrap();
        let error = read_baseline(path).await.unwrap_err();
        assert_eq!(error.code(), "FERRITE_E2E_CONFIG");
        assert!(error.to_string().contains("512 MiB"));
        assert_eq!(file.metadata().unwrap().len(), 512 * 1024 * 1024 + 1);

        let error = read_baseline(root.path().to_path_buf()).await.unwrap_err();
        assert_eq!(error.code(), "FERRITE_E2E_CONFIG");
        assert!(error.to_string().contains("regular file"));
        #[cfg(unix)]
        {
            let socket = root.path().join("socket");
            let _listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
            let error = read_baseline(socket).await.unwrap_err();
            assert_eq!(error.code(), "FERRITE_E2E_CONFIG");
            assert!(error.to_string().contains("regular file"));
        }
    }

    #[tokio::test]
    async fn png_raster_wait_is_preempted_without_blocking_runtime_and_stops_on_resume() {
        let mut bytes = Cursor::new(Vec::new());
        image::RgbaImage::from_pixel(32, 32, image::Rgba([255, 0, 0, 255]))
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let bytes = bytes.into_inner();
        let (entered, entry) = tokio::sync::oneshot::channel();
        let (resume, resumed) = std::sync::mpsc::channel();
        let (stopped, done) = tokio::sync::oneshot::channel();
        let mut job = Box::pin(run(move |stop| {
            let mut entered = Some(entered);
            let mut calls = 0;
            let result = crate::snapshot::compare_png_checked(&bytes, &bytes, 0, || {
                calls += 1;
                if calls == 4 {
                    entered.take().unwrap().send(()).unwrap();
                    resumed.recv().unwrap();
                }
                stop.check()
            });
            stopped.send(result.as_ref().unwrap_err().code()).unwrap();
            result
        }));
        tokio::select! {
            result=&mut job=>panic!("PNG work returned before its row barrier: {result:?}"),
            entered=entry=>entered.unwrap(),
        }
        let start = tokio::time::Instant::now();
        let error = crate::operation::Deadline::new(Duration::from_millis(25))
            .run("PNG CPU budget", job)
            .await
            .unwrap_err();
        assert_eq!(error.code(), "FERRITE_E2E_TIMEOUT");
        assert!(start.elapsed() < Duration::from_millis(500));
        // The codec/row phase is still blocked; returning above demonstrates
        // that neither its thread nor admission can block the async timer.
        resume.send(()).unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), done)
                .await
                .unwrap()
                .unwrap(),
            "FERRITE_E2E_CANCELLED"
        );
    }

    #[tokio::test]
    async fn queued_jobs_drop_inputs_without_execution_and_release_all_admission_slots() {
        use futures::FutureExt;
        let _exclusive = TEST_ADMISSION_LOCK.lock().await;
        let mut held = Vec::new();
        let mut released = Vec::new();
        for _ in 0..ACTIVE_LIMIT {
            let (entered, entry) = tokio::sync::oneshot::channel();
            let (stopped, done) = tokio::sync::oneshot::channel();
            let mut job = Box::pin(run(move |stop| {
                entered.send(()).unwrap();
                loop {
                    if let Err(error) = stop.check() {
                        let _ = stopped.send(());
                        return Err::<(), _>(error);
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
            }));
            tokio::select! {
                result=&mut job=>panic!("holding job returned: {result:?}"),
                entered=entry=>entered.unwrap(),
            }
            held.push(job);
            released.push(done);
        }
        let owner = Arc::new(());
        let weak = Arc::downgrade(&owner);
        let ran = Arc::new(AtomicBool::new(false));
        let executing = ran.clone();
        let mut waiting = Box::pin(run(move |_| {
            let _owner = owner;
            executing.store(true, Ordering::SeqCst);
            Ok(())
        }));
        assert!(waiting.as_mut().now_or_never().is_none());
        drop(waiting);
        tokio::time::timeout(Duration::from_secs(1), async {
            while weak.upgrade().is_some() {
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        })
        .await
        .unwrap();
        assert!(!ran.load(Ordering::SeqCst));
        drop(held);
        for done in released {
            tokio::time::timeout(Duration::from_secs(1), done)
                .await
                .unwrap()
                .unwrap();
        }
        assert_eq!(run(|_| Ok(42)).await.unwrap(), 42);
    }
}
