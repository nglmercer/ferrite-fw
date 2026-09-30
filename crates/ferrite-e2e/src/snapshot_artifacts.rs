//! Failure images use attempt-owned attachment copies, never baseline links.
//! No extra capture or baseline read is performed while constructing diagnostics.
use crate::{E2eError, Page};
use std::path::Path;

pub(crate) struct FailureImages<'a> {
    pub name: &'a str,
    pub path: &'a Path,
    pub expected: Option<&'a [u8]>,
    pub actual: &'a [u8],
    pub previous: Option<&'a [u8]>,
    pub stable: bool,
    pub threshold: u8,
}

/// One completed probe's last screenshot mismatch. No page/context owner is held.
/// Replacing this value drops earlier images rather than accumulating retry history.
pub(crate) struct DeferredImages {
    sink: crate::runner::SnapshotAttachmentSink,
    name: String,
    expected: Option<Vec<u8>>,
    actual: Vec<u8>,
    previous: Option<Vec<u8>>,
    stable: bool,
    threshold: u8,
}

tokio::task_local! {
    static PROBE_IMAGES: std::sync::Arc<std::sync::Mutex<Option<Box<DeferredImages>>>>;
}

/// Isolate each invocation, including nested polls. Dropping an unfinished
/// invocation drops its images; only completed assertion mismatches are retained.
pub(crate) fn retry_probe<F: std::future::Future>(
    future: F,
) -> impl std::future::Future<Output = (F::Output, Option<Box<DeferredImages>>)> {
    // Box before building the async wrapper: generic probe futures can be large
    // before their first poll, including borrowed/non-Send assertion blocks.
    let future = Box::pin(future);
    async move {
        let images = std::sync::Arc::new(std::sync::Mutex::new(None));
        let result = PROBE_IMAGES
            .scope(images.clone(), crate::report::retry_probe(future))
            .await;
        let deferred = images.lock().unwrap_or_else(|e| e.into_inner()).take();
        (result, deferred)
    }
}

fn defer(images: Box<DeferredImages>) -> Result<(), Box<DeferredImages>> {
    match PROBE_IMAGES.try_with(std::sync::Arc::clone) {
        Ok(slot) => {
            *slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(images);
            Ok(())
        }
        Err(_) => Err(images),
    }
}

/// Publish inside the final outer assertion step, or transfer to its parent's
/// probe. Typed operational errors and successful results discard stored images.
pub(crate) fn finish<T>(
    result: crate::E2eResult<T>,
    images: Option<Box<DeferredImages>>,
) -> crate::E2eResult<T> {
    match (result, images) {
        (Err(error), Some(images)) if error.code() == "FERRITE_E2E_EXPECT" => match defer(images) {
            Ok(()) => Err(error),
            Err(images) if !crate::report::in_retry_probe() => Err(with_failures(
                error,
                attach(
                    &images.sink,
                    &images.name,
                    images.expected.as_deref(),
                    &images.actual,
                    images.previous.as_deref(),
                    images.stable,
                    images.threshold,
                ),
            )),
            Err(_) => Err(error),
        },
        (result, _) => result,
    }
}

fn attach(
    sink: &crate::runner::SnapshotAttachmentSink,
    name: &str,
    expected: Option<&[u8]>,
    actual: &[u8],
    previous: Option<&[u8]>,
    stable: bool,
    threshold: u8,
) -> Vec<String> {
    let mut failures = Vec::new();
    {
        let mut copy = |kind: &str, bytes: &[u8]| {
            if let Err(error) = sink.attach(&format!("{name}-{kind}"), bytes) {
                failures.push(format!("{kind}: {error}"));
            }
        };
        if let Some(expected) = expected {
            copy("expected", expected);
        }
        copy("actual", actual);
        if !stable {
            if let Some(previous) = previous {
                copy("previous", previous);
            }
        }
    }
    for (kind, expected) in [
        ("diff", expected),
        ("stability-diff", (!stable).then_some(previous).flatten()),
    ] {
        if let Some(expected) = expected {
            let result = crate::snapshot::diff_png(actual, expected, threshold)
                .and_then(|bytes| sink.attach(&format!("{name}-{kind}"), &bytes));
            if let Err(error) = result {
                failures.push(format!("{kind}: {error}"));
            }
        }
    }
    failures
}

fn with_failures(error: E2eError, failures: Vec<String>) -> E2eError {
    if failures.is_empty() {
        error
    } else {
        error.with_context(&format!(
            "writing screenshot diagnostics also failed: {}",
            failures.join("; ")
        ))
    }
}

pub(crate) fn record(page: &Page, images: FailureImages<'_>, error: E2eError) -> E2eError {
    let mut failures = Vec::new();
    if let Some(sink) = &page.snapshot_attachments {
        if !crate::report::in_retry_probe() {
            failures = attach(
                sink,
                images.name,
                images.expected,
                images.actual,
                images.previous,
                images.stable,
                images.threshold,
            );
        } else if PROBE_IMAGES.try_with(|_| ()).is_ok() {
            let _ = defer(Box::new(DeferredImages {
                sink: sink.clone(),
                name: images.name.to_owned(),
                expected: images.expected.map(<[u8]>::to_vec),
                actual: images.actual.to_vec(),
                previous: (!images.stable)
                    .then_some(images.previous)
                    .flatten()
                    .map(<[u8]>::to_vec),
                stable: images.stable,
                threshold: images.threshold,
            }));
        }
    }
    // Keep the historical standalone diagnostic path as well. Report copies
    // remain independent when another attempt updates this baseline/actual file.
    let actual_path = images.path.with_extension("actual.png");
    let write = (|| -> std::io::Result<()> {
        if let Some(parent) = actual_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&actual_path, images.actual)
    })();
    if let Err(error) = write {
        failures.push(format!("actual path {}: {error}", actual_path.display()));
    }
    let error = error.with_context(&format!("actual: {}", actual_path.display()));
    with_failures(error, failures)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        future::Future,
        pin::Pin,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        task::{Context, Poll},
    };
    struct LargeProbe {
        bytes: [u8; 65_536],
        ready: bool,
        dropped: Arc<AtomicBool>,
    }
    impl Future for LargeProbe {
        type Output = bool;
        fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<bool> {
            assert!(crate::report::in_retry_probe());
            if self.ready {
                Poll::Ready(self.bytes[0] == 1)
            } else {
                Poll::Pending
            }
        }
    }
    impl Drop for LargeProbe {
        fn drop(&mut self) {
            self.dropped.store(true, Ordering::SeqCst);
        }
    }
    #[tokio::test]
    async fn probe_wrapper_boxes_large_inputs_before_first_poll_and_releases_dropped_work() {
        use futures::FutureExt;
        for ready in [true, false] {
            let dropped = Arc::new(AtomicBool::new(false));
            let future = retry_probe(LargeProbe {
                bytes: [1; 65_536],
                ready,
                dropped: dropped.clone(),
            });
            // Guard the regression that overflowed the large native soft-poll fixture.
            assert!(std::mem::size_of_val(&future) < 1024);
            assert!(!dropped.load(Ordering::SeqCst));
            let mut future = Box::pin(future);
            if ready {
                let (result, images) = future.as_mut().await;
                assert!(result);
                assert!(images.is_none());
            } else {
                assert!(future.as_mut().now_or_never().is_none());
            }
            assert!(!crate::report::in_retry_probe());
            drop(future);
            assert!(dropped.load(Ordering::SeqCst));
        }
    }
}
