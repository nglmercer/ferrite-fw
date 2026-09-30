//! Failure images use attempt-owned attachment copies, never baseline links.
//! No extra capture or baseline read is performed while constructing diagnostics.
use crate::{operation::Deadline, E2eError, E2eResult, Page};
use std::{future::Future, path::PathBuf, sync::Arc, time::Duration};

pub(crate) struct FailureImages {
    pub name: String,
    pub path: PathBuf,
    pub expected: Option<Arc<Vec<u8>>>,
    pub actual: Arc<Vec<u8>>,
    pub previous: Option<Arc<Vec<u8>>>,
    pub stable: bool,
    pub threshold: u8,
}

/// One completed probe's last screenshot mismatch. No page/context owner is held.
/// Shared immutable buffers avoid bulk copies on the async runtime.
pub(crate) struct DeferredImages {
    sink: crate::runner::SnapshotAttachmentSink,
    name: String,
    expected: Option<Arc<Vec<u8>>>,
    actual: Arc<Vec<u8>>,
    previous: Option<Arc<Vec<u8>>>,
    stable: bool,
    threshold: u8,
}

tokio::task_local! {
    static PROBE_IMAGES: Arc<std::sync::Mutex<Option<Box<DeferredImages>>>>;
}

pub(crate) fn retry_probe<F: Future>(
    future: F,
) -> impl Future<Output = (F::Output, Option<Box<DeferredImages>>)> {
    let future = Box::pin(future);
    async move {
        let images = Arc::new(std::sync::Mutex::new(None));
        let result = PROBE_IMAGES
            .scope(images.clone(), crate::report::retry_probe(future))
            .await;
        let deferred = images.lock().unwrap_or_else(|e| e.into_inner()).take();
        (result, deferred)
    }
}

fn defer(images: Box<DeferredImages>) -> Result<(), Box<DeferredImages>> {
    match PROBE_IMAGES.try_with(Arc::clone) {
        Ok(slot) => {
            *slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(images);
            Ok(())
        }
        Err(_) => Err(images),
    }
}

/// Capture/poll expiration establishes the mismatch. Diagnostic finalization has
/// one bounded five-second cleanup clock, capped by the enclosing operation.
/// It cannot reuse an already-expired matching clock to produce its failure data.
fn finalization_clock() -> Deadline {
    Deadline::new(Duration::from_secs(5))
}

async fn budget<T>(deadline: Deadline, future: impl Future<Output = E2eResult<T>>) -> E2eResult<T> {
    if deadline.expired() {
        return Err(E2eError::Timeout(5000, "screenshot diagnostics".into()));
    }
    let result = deadline
        .run("screenshot diagnostics", async { Ok(future.await) })
        .await?;
    if deadline.expired() {
        return Err(E2eError::Timeout(5000, "screenshot diagnostics".into()));
    }
    result
}

/// Final outer step publication, or transfer to a parent probe. Box the generic
/// result before constructing the future to preserve local/large probe support.
pub(crate) fn finish<T>(
    result: E2eResult<T>,
    images: Option<Box<DeferredImages>>,
) -> impl Future<Output = E2eResult<T>> {
    let mut input = Box::new((result, images));
    Box::pin(async move {
        if input
            .0
            .as_ref()
            .err()
            .is_some_and(|error| error.code() == "FERRITE_E2E_EXPECT")
        {
            if let Some(images) = input.1.take() {
                if let Err(images) = defer(images) {
                    if !crate::report::in_retry_probe() {
                        let failures = attach(&images, finalization_clock()).await;
                        if let Err(error) = &mut input.0 {
                            let original =
                                std::mem::replace(error, E2eError::Expect(String::new()));
                            *error = with_failures(original, failures);
                        }
                    }
                }
            }
        }
        // Keep T in the box through every await, including before first poll.
        input.0
    })
}

async fn attach(images: &DeferredImages, deadline: Deadline) -> Vec<String> {
    let mut failures = Vec::new();
    for (kind, bytes) in [
        ("expected", images.expected.clone()),
        ("actual", Some(images.actual.clone())),
        (
            "previous",
            (!images.stable)
                .then_some(images.previous.clone())
                .flatten(),
        ),
    ] {
        if let Some(bytes) = bytes {
            if let Err(error) = copy(
                &images.sink,
                &format!("{}-{kind}", images.name),
                bytes,
                deadline,
            )
            .await
            {
                failures.push(format!("{kind}: {error}"));
            }
        }
    }
    for (kind, expected) in [
        ("diff", images.expected.clone()),
        (
            "stability-diff",
            (!images.stable)
                .then_some(images.previous.clone())
                .flatten(),
        ),
    ] {
        if let Some(expected) = expected {
            let actual = images.actual.clone();
            let threshold = images.threshold;
            let rendered = budget(
                deadline,
                crate::snapshot_work::run(move |stop| {
                    crate::snapshot::diff_png_checked(&actual, &expected, threshold, || {
                        stop.check()
                    })
                    .map(Arc::new)
                }),
            )
            .await;
            let result = match rendered {
                Ok(bytes) => {
                    copy(
                        &images.sink,
                        &format!("{}-{kind}", images.name),
                        bytes,
                        deadline,
                    )
                    .await
                }
                Err(error) => Err(error),
            };
            if let Err(error) = result {
                failures.push(format!("{kind}: {error}"));
            }
        }
    }
    failures
}

async fn copy(
    sink: &crate::runner::SnapshotAttachmentSink,
    name: &str,
    bytes: Arc<Vec<u8>>,
    deadline: Deadline,
) -> E2eResult<()> {
    let file = budget(
        deadline,
        crate::snapshot_work::stage_attachment(sink.staging_dir(), bytes),
    )
    .await?;
    sink.attach_staged(name, file, deadline).map(|_| ())
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

pub(crate) fn record(
    page: &Page,
    images: FailureImages,
    error: E2eError,
) -> impl Future<Output = E2eError> {
    // Do not retain the Page in the diagnostic future or any worker.
    let input = Box::new((page.snapshot_attachments.clone(), images, error));
    async move {
        let (sink, images, error) = *input;
        let deadline = finalization_clock();
        let mut failures = Vec::new();
        if let Some(sink) = sink {
            let deferred = Box::new(DeferredImages {
                sink,
                name: images.name,
                expected: images.expected,
                actual: images.actual.clone(),
                previous: (!images.stable).then_some(images.previous).flatten(),
                stable: images.stable,
                threshold: images.threshold,
            });
            if !crate::report::in_retry_probe() {
                failures = attach(&deferred, deadline).await;
            } else if PROBE_IMAGES.try_with(|_| ()).is_ok() {
                let _ = defer(deferred);
            }
        }
        // Historical standalone path remains; stage/commit reuse the same
        // data-only writer, without any reread or extra native capture.
        let actual_path = images.path.with_extension("actual.png");
        let result = crate::snapshot_commit::finish(
            deadline,
            actual_path.clone(),
            images.actual,
            None,
            crate::SnapshotUpdate::All,
            &crate::SnapshotOptions::default(),
        )
        .await;
        if let Err(error) = result {
            failures.push(format!("actual path {}: {error}", actual_path.display()));
        }
        with_failures(
            error.with_context(&format!("actual: {}", actual_path.display())),
            failures,
        )
    }
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
    async fn finalization_preserves_large_local_values_and_shares_one_clock() {
        struct LargeValue([u8; 65_536], std::rc::Rc<()>);
        let owner = std::rc::Rc::new(());
        let weak = std::rc::Rc::downgrade(&owner);
        let future = finish(Ok(LargeValue([7; 65_536], owner)), None);
        assert!(std::mem::size_of_val(&future) < 1024);
        drop(future);
        assert!(weak.upgrade().is_none());
        let result = finish(Ok(LargeValue([7; 65_536], std::rc::Rc::new(()))), None)
            .await
            .unwrap();
        assert_eq!(result.0[0], 7);
        assert_eq!(std::rc::Rc::strong_count(&result.1), 1);
        let deadline = Deadline::new(Duration::from_millis(25));
        let error = budget(deadline, std::future::pending::<E2eResult<()>>())
            .await
            .unwrap_err();
        assert_eq!(error.code(), "FERRITE_E2E_TIMEOUT");
        let ran = AtomicBool::new(false);
        let error = budget(deadline, async {
            ran.store(true, Ordering::SeqCst);
            Ok(())
        })
        .await
        .unwrap_err();
        assert!(!ran.load(Ordering::SeqCst));
        let primary = with_failures(
            E2eError::Expect("primary mismatch".into()),
            vec![error.to_string()],
        );
        assert_eq!(primary.code(), "FERRITE_E2E_EXPECT");
        assert!(primary.to_string().contains("primary mismatch"));
        let token = crate::CancellationToken::new();
        let cancel = token.clone();
        let finalizing = token.run(budget(
            finalization_clock(),
            std::future::pending::<E2eResult<()>>(),
        ));
        let (result, ()) = tokio::join!(finalizing, async {
            tokio::task::yield_now().await;
            cancel.cancel();
        });
        assert_eq!(result.unwrap_err().code(), "FERRITE_E2E_CANCELLED");
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
