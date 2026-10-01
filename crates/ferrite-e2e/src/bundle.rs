//! Portable report export. Only artifact paths are rewritten; execution metadata
//! and the original in-memory report retain their source paths.
use crate::{E2eError, E2eResult, StepInfo, TestReport};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

/// A report folder with HTML, JSON, JUnit and deduplicated artifact files.
#[derive(Debug, Clone)]
pub struct ReportBundle {
    pub html: PathBuf,
    pub json: PathBuf,
    pub junit: PathBuf,
    pub artifact_count: usize,
}

impl TestReport {
    /// Copy all screenshots, videos, traces and attachments into a portable folder.
    /// HTML/JSON/JUnit reference paths relative to that folder. Source paths are
    /// resolved against the working directory; missing/unreadable files fail export.
    /// The original report and source artifacts are never modified.
    pub fn write_bundle(&self, directory: impl AsRef<Path>) -> E2eResult<ReportBundle> {
        self.write_bundle_with_report(directory.as_ref())
            .map(|(bundle, _)| bundle)
    }
    pub(crate) fn write_bundle_with_report(
        &self,
        directory: &Path,
    ) -> E2eResult<(ReportBundle, TestReport)> {
        fs::create_dir_all(directory.join("artifacts"))?;
        let mut copier = Copier {
            directory: directory.to_path_buf(),
            copied: HashMap::new(),
            next: 1,
        };
        let mut exported = self.clone();
        rewrite_steps(&mut exported.run_steps, &mut copier)?;
        for result in &mut exported.results {
            for path in &mut result.screenshots {
                copier.rewrite(path)?;
            }
            copier.optional(&mut result.trace)?;
            copier.optional(&mut result.video)?;
            for attachment in &mut result.attachments {
                copier.rewrite(&mut attachment.path)?;
            }
            for attempt in &mut result.attempt_results {
                for path in &mut attempt.screenshots {
                    copier.rewrite(path)?;
                }
                copier.optional(&mut attempt.trace)?;
                copier.optional(&mut attempt.video)?;
                for attachment in &mut attempt.attachments {
                    copier.rewrite(&mut attachment.path)?;
                }
                rewrite_steps(&mut attempt.steps, &mut copier)?;
            }
        }
        let bundle = ReportBundle {
            html: directory.join("report.html"),
            json: directory.join("results.json"),
            junit: directory.join("junit.xml"),
            artifact_count: copier.copied.len(),
        };
        fs::write(&bundle.html, exported.to_html())?;
        fs::write(&bundle.json, exported.to_json())?;
        fs::write(&bundle.junit, exported.to_junit())?;
        Ok((bundle, exported))
    }
}

struct Copier {
    directory: PathBuf,
    copied: HashMap<PathBuf, String>,
    next: usize,
}
impl Copier {
    fn optional(&mut self, path: &mut Option<String>) -> E2eResult<()> {
        if let Some(path) = path {
            self.rewrite(path)?;
        }
        Ok(())
    }
    fn rewrite(&mut self, path: &mut String) -> E2eResult<()> {
        let source = fs::canonicalize(&*path)
            .map_err(|e| E2eError::Config(format!("report artifact {path:?}: {e}")))?;
        if let Some(relative) = self.copied.get(&source) {
            *path = relative.clone();
            return Ok(());
        }
        if !source.is_file() {
            return Err(E2eError::Config(format!(
                "report artifact {path:?} is not a file"
            )));
        }
        let filename = source.file_name().unwrap().to_string_lossy();
        let safe: String = filename
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                    c
                } else {
                    '-'
                }
            })
            .take(100)
            .collect();
        let (relative, mut destination) = loop {
            let relative = format!("artifacts/{:04}-{safe}", self.next);
            self.next += 1;
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(self.directory.join(&relative))
            {
                Ok(file) => break (relative, file),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        };
        std::io::copy(&mut fs::File::open(&source)?, &mut destination)?;
        self.copied.insert(source, relative.clone());
        *path = relative;
        Ok(())
    }
}
fn rewrite_steps(steps: &mut [StepInfo], copier: &mut Copier) -> E2eResult<()> {
    for step in steps {
        for attachment in &mut step.attachments {
            copier.rewrite(&mut attachment.path)?;
        }
        rewrite_steps(&mut step.steps, copier)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Attachment, TestResult, TestStatus};
    fn report(path: String) -> TestReport {
        TestReport {
            configuration: None,
            run_steps: vec![],
            results: vec![TestResult {
                name: "export".into(),
                status: TestStatus::Passed,
                attempts: 1,
                duration_ms: 1,
                error: None,
                screenshots: vec![path.clone()],
                trace: Some(path.clone()),
                video: None,
                project: None,
                repeat_each_index: 0,
                annotations: vec![],
                flaky: false,
                attempt_results: vec![],
                attachments: vec![Attachment {
                    name: "alias".into(),
                    path,
                    content_type: "text/plain".into(),
                }],
            }],
        }
    }
    #[test]
    fn canonical_aliases_copy_once_without_overwriting_prior_exports() {
        let source = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();
        let path = source.path().join("state #é.txt");
        fs::write(&path, "first").unwrap();
        let mut report = report(path.display().to_string());
        report.results[0].attachments[0].path =
            source.path().join("./state #é.txt").display().to_string();
        let first = report.write_bundle(dest.path()).unwrap();
        assert_eq!(first.artifact_count, 1);
        let exported: TestReport = serde_json::from_slice(&fs::read(first.json).unwrap()).unwrap();
        let old = exported.results[0].screenshots[0].clone();
        assert_eq!(exported.results[0].trace.as_ref().unwrap(), &old);
        assert_eq!(exported.results[0].attachments[0].path, old);
        assert!(old.ends_with(".txt"));
        assert!(!old.contains(['#', 'é', ' ']));
        fs::write(&path, "second").unwrap();
        report.write_bundle(dest.path()).unwrap();
        let exported: TestReport =
            serde_json::from_slice(&fs::read(dest.path().join("results.json")).unwrap()).unwrap();
        assert_ne!(old, exported.results[0].screenshots[0]);
        assert_eq!(fs::read_to_string(dest.path().join(old)).unwrap(), "first");
        assert_eq!(
            fs::read_to_string(dest.path().join(&exported.results[0].screenshots[0])).unwrap(),
            "second"
        );
    }
    #[test]
    fn missing_and_directory_artifacts_fail_explicitly() {
        let source = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();
        for path in [source.path().join("missing"), source.path().to_path_buf()] {
            let error = report(path.display().to_string())
                .write_bundle(dest.path())
                .unwrap_err();
            assert!(error.to_string().contains("report artifact"));
            assert!(!dest.path().join("report.html").exists());
        }
    }
    #[test]
    fn relative_sources_resolve_from_working_directory() {
        let source = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let path = source.path().join("relative.txt");
        fs::write(&path, b"relative").unwrap();
        let relative = path.strip_prefix(std::env::current_dir().unwrap()).unwrap();
        let dest = tempfile::tempdir().unwrap();
        let bundle = report(relative.display().to_string())
            .write_bundle(dest.path())
            .unwrap();
        assert_eq!(bundle.artifact_count, 1);
        let exported: TestReport = serde_json::from_slice(&fs::read(bundle.json).unwrap()).unwrap();
        assert_eq!(
            fs::read(dest.path().join(&exported.results[0].screenshots[0])).unwrap(),
            b"relative"
        );
    }
}
