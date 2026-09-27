//! Test helpers (spec §67): temp projects, fixtures, assertions.
//!
//! Used by the `tests/vite-compat/` suite and by downstream framework
//! adapters. Every helper works without Node.js.

use std::path::{Path, PathBuf};

use ferrite_config::{resolve_config, CliOverrides, ResolvedConfig, UserConfig};

/// A temporary Ferrite project.
pub struct TempProject {
    /// Temp dir (deleted on drop).
    pub _dir: tempfile::TempDir,
    /// Project root.
    pub root: PathBuf,
}

impl TempProject {
    /// Create a project with `files` (`path` → `contents`).
    pub fn new(files: &[(&str, &str)]) -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().to_path_buf();
        for (path, contents) in files {
            let target = root.join(path);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).expect("mkdir");
            }
            std::fs::write(&target, contents).expect("write");
        }
        Self { _dir: dir, root }
    }

    /// Resolve a default config rooted here.
    pub fn resolve_config(&self) -> ResolvedConfig {
        resolve_config(
            UserConfig::default(),
            Some(self.root.clone()),
            CliOverrides::default(),
        )
        .expect("resolve")
    }

    /// Resolve config with `mode` (`development`/`production`).
    pub fn resolve_config_mode(&self, mode: &str) -> ResolvedConfig {
        resolve_config(
            UserConfig::default(),
            Some(self.root.clone()),
            CliOverrides {
                mode: Some(mode.to_string()),
                ..Default::default()
            },
        )
        .expect("resolve")
    }

    /// Read a project file to string.
    pub fn read(&self, path: &str) -> String {
        std::fs::read_to_string(self.root.join(path)).expect("read")
    }

    /// True when a project file exists.
    pub fn exists(&self, path: &str) -> bool {
        self.root.join(path).exists()
    }
}

/// Minimal Vite-style fixture app.
#[must_use]
pub fn vanilla_files() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "index.html",
            "<!doctype html><html><head><title>t</title></head><body>\
             <script type=\"module\" src=\"/src/main.ts\"></script></body></html>",
        ),
        (
            "src/main.ts",
            "import { greet } from \"./greet\";\n\
             import \"./style.css\";\n\
             document.body.textContent = greet(\"ferrite\");\n",
        ),
        (
            "src/greet.ts",
            "export function greet(name: string): string {\n  return `hello ${name}`;\n}\n",
        ),
        ("src/style.css", ".app { color: red; }\n"),
        ("public/favicon.ico", "ico"),
    ]
}

/// Assert `value` contains every `needle` (pretty failure).
#[track_caller]
pub fn assert_contains_all(value: &str, needles: &[&str]) {
    for needle in needles {
        assert!(
            value.contains(needle),
            "expected output to contain `{needle}`, got:\n{value}"
        );
    }
}

/// Copy a directory tree.
pub fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
