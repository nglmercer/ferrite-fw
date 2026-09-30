//! Successive screenshot comparison under one clock. Keep the last completed
//! capture on expiry; never start a new capture to produce a failure artifact.
use std::{future::Future, sync::Arc, time::Duration};

use crate::{operation::Deadline, snapshot::SnapshotOptions, E2eError, E2eResult};

pub(crate) struct Comparison {
    pub result: E2eResult<()>,
    pub actual: Option<Arc<Vec<u8>>>,
    pub previous: Option<Arc<Vec<u8>>>,
    /// Last completed successive-pair assessment. The newest native capture
    /// may still be awaiting validation/comparison when the clock expires.
    pub stable: bool,
}

/// Capture twice before accepting a baseline or a match. An existing baseline
/// may become correct after an initially stable mismatch, so keep retrying it.
/// Only pixel/dimension mismatches are retryable; operational errors propagate.
pub(crate) async fn compare_with_deadline<F, Fut>(
    deadline: Deadline,
    description: &str,
    expected: Option<Arc<Vec<u8>>>,
    options: &SnapshotOptions,
    negated: bool,
    mut capture: F,
) -> Comparison
where
    F: FnMut() -> Fut,
    Fut: Future<Output = E2eResult<Vec<u8>>>,
{
    let mut actual: Option<Arc<Vec<u8>>> = None;
    let mut previous: Option<Arc<Vec<u8>>> = None;
    let mut last = "no completed capture".to_string();
    let mut stable = false;
    let result = deadline
        .run(description, async {
            // Nest the probe result so a returned Timeout cannot be confused
            // with expiry of the one assertion clock.
            let work = async {
                options.validate()?;
                if deadline.expired() {
                    std::future::pending::<()>().await;
                }
                if let Some(expected) = &expected {
                    compare_bytes(expected.clone(), expected.clone(), options.threshold).await?;
                } else if negated {
                    return Err(E2eError::Expect(
                        "negated screenshot requires an existing snapshot".into(),
                    ));
                }
                loop {
                    if deadline.expired() {
                        std::future::pending::<()>().await;
                    }
                    let next = Arc::new(capture().await?);
                    previous = actual.replace(next.clone());
                    if previous.is_none() {
                        last = "only one completed capture".into();
                    }
                    // A completed native capture remains available even if its
                    // CPU validation/comparison is interrupted by the shared clock.
                    if deadline.expired() {
                        std::future::pending::<()>().await;
                    }
                    compare_bytes(next.clone(), next, options.threshold).await?;
                    if deadline.expired() {
                        std::future::pending::<()>().await;
                    }
                    if let Some(previous) = &previous {
                        let diff = compare_bytes(
                            actual.as_ref().unwrap().clone(),
                            previous.clone(),
                            options.threshold,
                        )
                        .await;
                        if deadline.expired() {
                            std::future::pending::<()>().await;
                        }
                        match diff {
                            Ok(diff) => {
                                stable = diff.passed(options);
                                if !stable {
                                    last =
                                        format!("successive captures differ: {}", diff.summary());
                                }
                            }
                            Err(error) if error.code() == "FERRITE_E2E_EXPECT" => {
                                stable = false;
                                last = error.to_string()
                            }
                            Err(error) => return Err(error),
                        }
                        if stable {
                            match &expected {
                                None => return Ok(()),
                                Some(expected) => {
                                    let diff = compare_bytes(
                                        actual.as_ref().unwrap().clone(),
                                        expected.clone(),
                                        options.threshold,
                                    )
                                    .await;
                                    if deadline.expired() {
                                        std::future::pending::<()>().await;
                                    }
                                    let matches = match diff {
                                        Ok(diff) => {
                                            last = diff.summary();
                                            diff.passed(options)
                                        }
                                        Err(error) if error.code() == "FERRITE_E2E_EXPECT" => {
                                            last = error.to_string();
                                            false
                                        }
                                        Err(error) => return Err(error),
                                    };
                                    if matches != negated {
                                        return Ok(());
                                    }
                                }
                            }
                        }
                    } else {
                        last = "only one completed capture".into();
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            };
            Ok(crate::report::retry_probe(work).await)
        })
        .await;
    let result = match result {
        Ok(result) => result.map_err(|error| error.with_context(description)),
        Err(E2eError::Timeout(..)) => Err(E2eError::Expect(format!(
            "{description} differs: {} (last: {last})",
            if stable {
                "stable capture did not satisfy the baseline"
            } else {
                "failed to take two consecutive stable screenshots"
            }
        ))),
        Err(error) => Err(error.with_context(description)),
    };
    Comparison {
        result,
        actual,
        previous,
        stable,
    }
}

fn compare_bytes(
    actual: Arc<Vec<u8>>,
    expected: Arc<Vec<u8>>,
    threshold: u8,
) -> impl Future<Output = E2eResult<crate::SnapshotDiff>> {
    crate::snapshot_work::run(move |stop| {
        // Validation uses the same immutable buffer on both sides. Decode it
        // once without allocating a second raster or comparing it to itself.
        if Arc::ptr_eq(&actual, &expected) {
            crate::snapshot::validate_png_checked(&actual, || stop.check())
        } else {
            crate::snapshot::compare_png_checked(&actual, &expected, threshold, || stop.check())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, io::Cursor};
    async fn compare<F, Fut>(
        timeout: Duration,
        description: &str,
        expected: Option<&[u8]>,
        options: &SnapshotOptions,
        negated: bool,
        capture: F,
    ) -> Comparison
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = E2eResult<Vec<u8>>>,
    {
        compare_with_deadline(
            Deadline::new(timeout),
            description,
            expected.map(|bytes| Arc::new(bytes.to_vec())),
            options,
            negated,
            capture,
        )
        .await
    }
    fn png(width: u32, red: u8) -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(width, 2, image::Rgba([red, 0, 0, 255]));
        let mut output = Cursor::new(Vec::new());
        image
            .write_to(&mut output, image::ImageFormat::Png)
            .unwrap();
        output.into_inner()
    }

    #[tokio::test]
    async fn pending_validation_keeps_last_completed_stability_assessment_and_native_bytes() {
        use std::{cell::RefCell, pin::Pin, rc::Rc};
        let _exclusive = crate::snapshot_work::TEST_ADMISSION_LOCK.lock().await;
        type Held = Pin<Box<dyn Future<Output = E2eResult<()>>>>;
        let held: Rc<RefCell<Vec<Held>>> = Rc::new(RefCell::new(Vec::new()));
        let calls = Cell::new(0);
        let expected = png(2, 0);
        let capture = || {
            let call = calls.get();
            calls.set(call + 1);
            let held = held.clone();
            async move {
                if call == 2 {
                    // Hold both production admission slots only after the first
                    // pair was proved stable and different from the baseline.
                    // The third capture completes; its validation must wait.
                    for _ in 0..2 {
                        let (entered, entry) = tokio::sync::oneshot::channel();
                        let mut job: Held = Box::pin(crate::snapshot_work::run(move |stop| {
                            entered.send(()).unwrap();
                            loop {
                                stop.check()?;
                                std::thread::sleep(Duration::from_millis(1));
                            }
                        }));
                        tokio::select! {
                            result=&mut job=>panic!("holding job returned: {result:?}"),
                            entered=entry=>entered.unwrap(),
                        }
                        held.borrow_mut().push(job);
                    }
                }
                Ok(png(2, if call < 2 { 30 } else { 45 }))
            }
        };
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            compare(
                Duration::from_millis(500),
                "pending validation",
                Some(&expected),
                &SnapshotOptions::default(),
                false,
                capture,
            ),
        )
        .await
        .unwrap();
        held.borrow_mut().clear();
        assert_eq!(calls.get(), 3);
        assert_eq!(result.result.unwrap_err().code(), "FERRITE_E2E_EXPECT");
        assert!(result.stable);
        assert_eq!(result.actual.as_deref().unwrap().as_slice(), png(2, 45));
        assert_eq!(result.previous.as_deref().unwrap().as_slice(), png(2, 30));
    }

    #[tokio::test(start_paused = true)]
    async fn new_and_existing_baselines_need_successive_stable_captures() {
        for existing in [false, true] {
            let calls = Cell::new(0);
            let target = png(2, 30);
            let result = compare(
                Duration::from_secs(1),
                "stable",
                existing.then_some(target.as_slice()),
                &SnapshotOptions::default(),
                false,
                || {
                    let call = calls.get();
                    calls.set(call + 1);
                    let bytes = png(2, if call == 0 { 10 } else { 30 });
                    async move { Ok(bytes) }
                },
            )
            .await;
            result.result.unwrap();
            assert_eq!(calls.get(), 3);
            assert_eq!(result.actual.as_deref().unwrap(), &target);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn stable_mismatch_can_become_correct_within_the_same_window() {
        let calls = Cell::new(0);
        let target = png(2, 30);
        let result = compare(
            Duration::from_secs(1),
            "delayed",
            Some(&target),
            &SnapshotOptions::default(),
            false,
            || {
                let call = calls.get();
                calls.set(call + 1);
                let bytes = png(2, if call < 2 { 10 } else { 30 });
                async move { Ok(bytes) }
            },
        )
        .await;
        result.result.unwrap();
        assert_eq!(calls.get(), 4);
    }

    #[tokio::test(start_paused = true)]
    async fn never_stable_and_dimension_mismatch_keep_last_bytes_without_extra_capture() {
        for change_size in [false, true] {
            let calls = Cell::new(0);
            let target = png(2, 0);
            let start = tokio::time::Instant::now();
            let result = compare(
                Duration::from_millis(125),
                "deadline",
                Some(&target),
                &SnapshotOptions::default(),
                false,
                || {
                    let call = calls.get();
                    calls.set(call + 1);
                    let bytes = png(
                        if change_size { 3 } else { 2 },
                        if change_size { 0 } else { call as u8 },
                    );
                    async move { Ok(bytes) }
                },
            )
            .await;
            assert_eq!(calls.get(), 3);
            assert_eq!(
                tokio::time::Instant::now() - start,
                Duration::from_millis(125)
            );
            assert_eq!(result.stable, change_size);
            assert_eq!(
                result.previous.as_deref().unwrap().as_slice(),
                png(
                    if change_size { 3 } else { 2 },
                    if change_size { 0 } else { 1 }
                )
            );
            let error = result.result.unwrap_err();
            assert_eq!(error.code(), "FERRITE_E2E_EXPECT");
            assert!(
                error.to_string().contains(if change_size {
                    "size differs"
                } else {
                    "consecutive stable"
                }),
                "{error}"
            );
            assert_eq!(
                result.actual.as_deref().unwrap().as_slice(),
                png(
                    if change_size { 3 } else { 2 },
                    if change_size { 0 } else { 2 }
                )
            );
        }
    }

    #[tokio::test(start_paused = true)]
    async fn hung_probe_is_bounded_and_operational_timeout_is_not_assertion_expiry() {
        let hung = compare(
            Duration::from_millis(100),
            "hung",
            None,
            &SnapshotOptions::default(),
            false,
            std::future::pending,
        )
        .await;
        assert_eq!(hung.result.unwrap_err().code(), "FERRITE_E2E_EXPECT");
        assert!(hung.actual.is_none());
        for error in [
            E2eError::Timeout(7, "native".into()),
            E2eError::Cancelled("caller".into()),
            E2eError::Disconnected("wire".into()),
            E2eError::Config("invalid".into()),
        ] {
            let code = error.code();
            let mut error = Some(error);
            let calls = Cell::new(0);
            let result = compare(
                Duration::from_secs(1),
                "control",
                None,
                &SnapshotOptions::default(),
                false,
                || {
                    calls.set(calls.get() + 1);
                    std::future::ready(Err(error.take().unwrap()))
                },
            )
            .await;
            assert_eq!(result.result.unwrap_err().code(), code);
            assert_eq!(calls.get(), 1);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn zero_has_no_local_cutoff_and_negation_requires_valid_baseline() {
        let calls = Cell::new(0);
        let target = png(2, 10);
        let result = compare(
            Duration::ZERO,
            "zero",
            Some(&target),
            &SnapshotOptions::default(),
            true,
            || {
                calls.set(calls.get() + 1);
                async {
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    Ok(png(3, 10))
                }
            },
        )
        .await;
        result.result.unwrap();
        assert_eq!(calls.get(), 2);
        for expected in [None, Some(b"invalid".as_slice())] {
            let result = compare(
                Duration::ZERO,
                "negation",
                expected,
                &SnapshotOptions::default(),
                true,
                || async { panic!("must not capture") },
            )
            .await;
            assert_eq!(
                result.result.unwrap_err().code(),
                if expected.is_some() {
                    "FERRITE_E2E_CONFIG"
                } else {
                    "FERRITE_E2E_EXPECT"
                }
            );
        }
    }

    #[tokio::test(start_paused = true)]
    async fn elapsed_preparation_and_zero_local_windows_cannot_renew_outer_clock() {
        let deadline = Deadline::new(Duration::from_millis(100));
        tokio::time::sleep(Duration::from_millis(75)).await;
        let start = tokio::time::Instant::now();
        let result = compare_with_deadline(
            deadline,
            "preparation",
            None,
            &SnapshotOptions::default(),
            false,
            std::future::pending,
        )
        .await;
        assert_eq!(
            tokio::time::Instant::now() - start,
            Duration::from_millis(25)
        );
        assert_eq!(result.result.unwrap_err().code(), "FERRITE_E2E_EXPECT");
        let result = Deadline::new(Duration::from_millis(100))
            .run("enclosing", async {
                Ok(compare(
                    Duration::ZERO,
                    "untimed local",
                    None,
                    &SnapshotOptions::default(),
                    false,
                    std::future::pending,
                )
                .await)
            })
            .await;
        assert!(matches!(result, Err(E2eError::Timeout(100, _))));
    }
}
