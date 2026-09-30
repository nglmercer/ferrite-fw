//! Snapshot paths are resolved before capture or writing. Runner inputs are
//! frozen; standalone callers may supply the same owned context explicitly.
use crate::{BrowserKind, E2eError, E2eResult, SnapshotOptions};
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotKind {
    #[default]
    Screenshot,
    Text,
}
impl SnapshotKind {
    pub(crate) fn extension(self) -> &'static str {
        match self {
            Self::Screenshot => "png",
            Self::Text => "snap",
        }
    }
}

/// Values used by an explicit path template. None denotes unavailable metadata.
/// root_dir is the template base and testDir; runner pages receive the run's
/// captured working directory. Relative test_file paths resolve against it.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SnapshotPathContext {
    pub root_dir: Option<PathBuf>,
    pub browser: Option<BrowserKind>,
    pub project: Option<String>,
    pub test_file: Option<PathBuf>,
    pub test_name: Option<String>,
}

const TOKENS: &[&str] = &[
    "arg",
    "ext",
    "platform",
    "projectName",
    "browserName",
    "snapshotDir",
    "testDir",
    "testFileDir",
    "testFileBaseName",
    "testFileName",
    "testFilePath",
    "testName",
];

fn substitute(
    template: &str,
    mut value: impl FnMut(&str) -> E2eResult<String>,
) -> E2eResult<String> {
    if template.trim().is_empty() || template.len() > 8192 || template.contains('\0') {
        return Err(E2eError::Config(
            "snapshot path template must be nonempty, NUL-free and at most 8192 bytes".into(),
        ));
    }
    let mut output = String::new();
    let mut rest = template;
    while let Some(at) = rest.find(['{', '}']) {
        output.push_str(&rest[..at]);
        if rest.as_bytes()[at] == b'}' {
            return Err(E2eError::Config(
                "unmatched closing brace in snapshot path template".into(),
            ));
        }
        rest = &rest[at + 1..];
        let end = rest
            .find('}')
            .ok_or_else(|| E2eError::Config("unclosed snapshot path template token".into()))?;
        let token = &rest[..end];
        let (prefix, key) = if TOKENS.contains(&token) {
            ("", token)
        } else {
            let first = token
                .chars()
                .next()
                .ok_or_else(|| E2eError::Config("empty snapshot path template token".into()))?;
            let (prefix, key) = token.split_at(first.len_utf8());
            if !TOKENS.contains(&key) {
                return Err(E2eError::Config(format!(
                    "unknown snapshot path template token {token:?}"
                )));
            }
            (prefix, key)
        };
        let replacement = value(key)?;
        if !replacement.is_empty() {
            output.push_str(prefix);
            output.push_str(&replacement);
        }
        rest = &rest[end + 1..];
    }
    output.push_str(rest);
    Ok(output)
}

pub(crate) fn validate_template(template: &str) -> E2eResult<()> {
    substitute(template, |_| Ok(String::new())).map(|_| ())
}

/// Normalize trusted literal template segments without requiring directories
/// to exist. Dynamic snapshot arguments are validated separately.
fn normalize(path: &Path) -> PathBuf {
    let mut output = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                output.pop();
            }
            part => output.push(part.as_os_str()),
        }
    }
    output
}

fn root(context: &SnapshotPathContext) -> E2eResult<PathBuf> {
    if let Some(root) = context.root_dir.as_ref().filter(|root| root.is_absolute()) {
        return Ok(normalize(root));
    }
    let working = std::env::current_dir()?;
    let root = context.root_dir.as_deref().unwrap_or(&working);
    if root.as_os_str().is_empty() {
        return Err(E2eError::Config("snapshot root_dir cannot be empty".into()));
    }
    Ok(normalize(&if root.is_absolute() {
        root.to_path_buf()
    } else {
        working.join(root)
    }))
}

fn argument(name: &str, extension: &str) -> E2eResult<String> {
    let name = name.replace('\\', "/");
    if name.is_empty() || name.len() > 8192 || name.contains('\0') || name.starts_with('/') {
        return Err(E2eError::Config(
            "snapshot name must be a nonempty relative path".into(),
        ));
    }
    let without_extension = name.strip_suffix(&format!(".{extension}")).unwrap_or(&name);
    let mut parts = Vec::new();
    for part in without_extension.split('/') {
        if matches!(part, "" | "." | "..") || part.contains(':') {
            return Err(E2eError::Config(
                "snapshot name must contain normal relative path components".into(),
            ));
        }
        parts.push(crate::runner::slug(part));
    }
    Ok(parts.join("/"))
}

impl SnapshotOptions {
    /// Resolve a baseline path without creating files. An explicit template
    /// uses root_dir (or current directory) as its relative base. Without a
    /// template, retain the legacy <snapshot-dir>/<slug>.<extension> contract.
    pub fn path(&self, name: &str, kind: SnapshotKind) -> E2eResult<PathBuf> {
        let extension = kind.extension();
        let directory = crate::snapshot::resolve_dir(self.dir.as_deref());
        let Some(template) = &self.path_template else {
            return Ok(directory.join(format!("{}.{extension}", crate::runner::slug(name))));
        };
        let context = self.path_context.clone().unwrap_or_default();
        let root = root(&context)?;
        let snapshot_dir = normalize(&if directory.is_absolute() {
            directory
        } else {
            root.join(directory)
        });
        let arg = argument(name, extension)?;
        let file = context.test_file.as_ref().map(|file| {
            let absolute = normalize(&if file.is_absolute() {
                file.clone()
            } else {
                root.join(file)
            });
            absolute
                .strip_prefix(&root)
                .map(Path::to_path_buf)
                .map_err(|_| {
                    E2eError::Config(
                        "snapshot test_file must be inside root_dir when using test-file tokens"
                            .into(),
                    )
                })
        });
        let output = substitute(template, |token| {
            let file_value = |part: &str| -> E2eResult<String> {
                let Some(file) = &file else {
                    return Ok(String::new());
                };
                let file = file
                    .as_ref()
                    .map_err(|error| E2eError::Config(error.to_string()))?;
                Ok(match part {
                    "testFileDir" => file
                        .parent()
                        .unwrap_or(Path::new(""))
                        .to_string_lossy()
                        .replace('\\', "/"),
                    "testFileName" => file
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    "testFileBaseName" => file
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    _ => file.to_string_lossy().replace('\\', "/"),
                })
            };
            Ok(match token {
                "arg" => arg.clone(),
                "ext" => format!(".{extension}"),
                "platform" => match std::env::consts::OS {
                    "macos" => "darwin",
                    "windows" => "win32",
                    other => other,
                }
                .into(),
                "browserName" => context
                    .browser
                    .map(|kind| kind.name().into())
                    .unwrap_or_default(),
                "projectName" => context
                    .project
                    .as_deref()
                    .filter(|name| !name.is_empty())
                    .map(crate::runner::slug)
                    .unwrap_or_default(),
                "testName" => context
                    .test_name
                    .as_deref()
                    .filter(|name| !name.is_empty())
                    .map(crate::runner::slug)
                    .unwrap_or_default(),
                "snapshotDir" => snapshot_dir.to_string_lossy().replace('\\', "/"),
                "testDir" => root.to_string_lossy().replace('\\', "/"),
                token => file_value(token)?,
            })
        })?;
        if output.is_empty() {
            return Err(E2eError::Config(
                "snapshot template resolves to an empty path".into(),
            ));
        }
        let output = PathBuf::from(output);
        Ok(normalize(&if output.is_absolute() {
            output
        } else {
            root.join(output)
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn options() -> SnapshotOptions {
        SnapshotOptions {
            dir:Some("baselines".into()),
            path_template:Some("{snapshotDir}{/projectName}/{browserName}/{platform}/{testFilePath}/{testName}/{arg}{ext}".into()),
            path_context:Some(SnapshotPathContext {
                root_dir:Some("/work".into()),browser:Some(BrowserKind::Firefox),project:Some("Desktop & tablet".into()),test_file:Some("tests/home.spec.rs".into()),test_name:Some("suite > home".into()),
            }),..Default::default()
        }
    }
    #[test]
    fn templates_resolve_context_nested_names_extensions_and_optional_prefixes() {
        let options = options();
        let path = options
            .path("nested/Card.png", SnapshotKind::Screenshot)
            .unwrap();
        assert_eq!(
            path,
            Path::new("/work/baselines/desktop-tablet/firefox")
                .join(match std::env::consts::OS {
                    "macos" => "darwin",
                    "windows" => "win32",
                    other => other,
                })
                .join("tests/home.spec.rs/suite-home/nested/card.png")
        );
        let mut options = options;
        options.path_context.as_mut().unwrap().project = None;
        assert!(!options
            .path("text", SnapshotKind::Text)
            .unwrap()
            .to_string_lossy()
            .contains("desktop-tablet"));
        options.path_template =
            Some("snapshots/{testFileBaseName}{-projectName}/{arg}{ext}".into());
        assert_eq!(
            options.path("body", SnapshotKind::Text).unwrap(),
            Path::new("/work/snapshots/home.spec/body.snap")
        );
    }
    #[test]
    fn explicit_directory_context_and_literal_absolute_templates_are_deterministic() {
        let mut options = options();
        options.dir = Some("custom".into());
        options.path_template =
            Some("{snapshotDir}/{testFileDir}/{testFileName}/{arg}{ext}".into());
        assert_eq!(
            options.path("home", SnapshotKind::Screenshot).unwrap(),
            Path::new("/work/custom/tests/home.spec.rs/home.png")
        );
        options.path_template = Some("/shared/{arg}{ext}".into());
        options.path_context.as_mut().unwrap().test_file = Some("/outside/module.rs".into());
        assert_eq!(
            options.path("home", SnapshotKind::Screenshot).unwrap(),
            Path::new("/shared/home.png")
        );
        options.path_template = None;
        assert_eq!(
            options
                .path("home page!", SnapshotKind::Screenshot)
                .unwrap(),
            Path::new("custom/home-page.png")
        );
    }
    #[test]
    fn bad_tokens_names_and_unavailable_file_roots_fail_before_writing() {
        let mut options = options();
        for template in ["", "{unknown}", "{arg", "arg}", "{{arg}}", "{}"] {
            options.path_template = Some(template.into());
            assert!(
                options.path("home", SnapshotKind::Screenshot).is_err(),
                "{template}"
            );
        }
        options.path_template = Some("{snapshotDir}/{arg}{ext}".into());
        for name in [
            "",
            "../home",
            "/home",
            "nested/../home",
            "C:\\home",
            "nested//home",
            ".png",
        ] {
            assert!(
                options.path(name, SnapshotKind::Screenshot).is_err(),
                "{name}"
            );
        }
        options.path_template = Some("{testFilePath}/{arg}{ext}".into());
        options.path_context.as_mut().unwrap().test_file = Some("/outside/file.rs".into());
        assert!(options.path("home", SnapshotKind::Screenshot).is_err());
    }

    #[test]
    fn text_and_png_helpers_use_the_exact_resolved_path_for_baseline_and_actual() {
        let dir = tempfile::tempdir().unwrap();
        let options = SnapshotOptions {
            path_template: Some("{snapshotDir}/nested/{arg}{ext}".into()),
            dir: Some(dir.path().into()),
            update: Some(crate::SnapshotUpdate::Missing),
            ..Default::default()
        };
        crate::assert_snapshot_text("Body", "first", &options).unwrap();
        let path = options.path("Body", SnapshotKind::Text).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "first");
        assert!(crate::assert_snapshot_text("Body", "second", &options).is_err());
        assert_eq!(
            std::fs::read_to_string(path.with_extension("actual.snap")).unwrap(),
            "second"
        );
        let actual_path = path.with_extension("actual.snap");
        std::fs::remove_file(&actual_path).unwrap();
        std::fs::create_dir(&actual_path).unwrap();
        let failure = crate::assert_snapshot_text("Body", "third", &options).unwrap_err();
        assert_eq!(failure.code(), "FERRITE_E2E_EXPECT");
        assert!(failure
            .to_string()
            .contains("writing failure artifact also failed"));
        let png = |red| {
            let image = image::RgbaImage::from_pixel(2, 2, image::Rgba([red, 0, 0, 255]));
            let mut bytes = std::io::Cursor::new(Vec::new());
            image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
            bytes.into_inner()
        };
        crate::assert_snapshot_png("Card.png", &png(10), &options).unwrap();
        let path = options.path("Card.png", SnapshotKind::Screenshot).unwrap();
        assert!(path.is_file());
        assert!(crate::assert_snapshot_png("Card.png", &png(30), &options).is_err());
        assert_eq!(
            std::fs::read(path.with_extension("actual.png")).unwrap(),
            png(30)
        );
    }
}
