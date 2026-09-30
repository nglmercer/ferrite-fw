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

pub(crate) fn record(page: &Page, images: FailureImages<'_>, error: E2eError) -> E2eError {
    let mut failures = Vec::new();
    // A generic retry block can catch this mismatch and later pass. Its probes
    // must not publish failed assertion attachments into the final outer step.
    if let Some(sink) = page
        .snapshot_attachments
        .as_ref()
        .filter(|_| !crate::report::in_retry_probe())
    {
        {
            let mut attach = |kind: &str, bytes: &[u8]| {
                if let Err(error) = sink.attach(&format!("{}-{kind}", images.name), bytes) {
                    failures.push(format!("{kind}: {error}"));
                }
            };
            if let Some(expected) = images.expected {
                attach("expected", expected);
            }
            attach("actual", images.actual);
            if !images.stable {
                if let Some(previous) = images.previous {
                    attach("previous", previous);
                }
            }
        }
        let comparisons = [
            ("diff", images.expected),
            (
                "stability-diff",
                (!images.stable).then_some(images.previous).flatten(),
            ),
        ];
        for (kind, expected) in comparisons {
            if let Some(expected) = expected {
                let result = crate::snapshot::diff_png(images.actual, expected, images.threshold)
                    .and_then(|bytes| sink.attach(&format!("{}-{kind}", images.name), &bytes));
                if let Err(error) = result {
                    failures.push(format!("{kind}: {error}"));
                }
            }
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
    if failures.is_empty() {
        error
    } else {
        error.with_context(&format!(
            "writing screenshot diagnostics also failed: {}",
            failures.join("; ")
        ))
    }
}
