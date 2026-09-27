//! Ferrite repo maintenance tasks.
//!
//! Run with `cargo run -p xtask -- <task>`:
//!
//! - `materialize-tailwind`: inline `*.workspace` inheritance in the
//!   vendored tailwind-rs manifests (same transform `cargo vendor`
//!   performs). Pass `--check` to verify without writing.

use std::path::{Path, PathBuf};

/// `[workspace.package]` values pinned from the vendored root.
const PINNED_PACKAGE: &[(&str, &str)] = &[
    ("edition", "\"2021\""),
    ("rust-version", "\"1.88\""),
    ("version", "\"0.1.0\""),
    ("license", "\"MIT OR Apache-2.0\""),
    ("repository", "\"https://github.com/nglmercer/tailwind-rs\""),
    ("homepage", "\"https://github.com/nglmercer/tailwind-rs\""),
];

/// Inlined `[lints]` tables (from the vendored `[workspace.lints]`).
const LINTS_TABLE: &str = "[lints.rust]\n\
     unsafe_code = \"deny\"\n\
     missing_docs = \"warn\"\n\
     rust_2018_idioms = { level = \"deny\", priority = -1 }\n\
     \n\
     [lints.clippy]\n\
     all = \"warn\"";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives at the workspace root")
        .to_path_buf()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("materialize-tailwind") => {
            let check = args.iter().any(|arg| arg == "--check");
            materialize_tailwind(&repo_root().join("vendor/tailwind-rs"), check)
        }
        _ => {
            eprintln!("usage: cargo run -p xtask -- materialize-tailwind [--check]");
            std::process::exit(2);
        }
    };
    if let Err(error) = result {
        eprintln!("xtask: {error}");
        std::process::exit(1);
    }
}

/// Inline workspace inheritance in every vendored crate manifest.
///
/// Refuses to run when the inner root's pinned values drift (re-pin
/// `PINNED_PACKAGE` / `LINTS_TABLE` after re-vendoring). With `check`,
/// reports drift without writing.
fn materialize_tailwind(vendor_root: &Path, check: bool) -> Result<(), String> {
    let root_manifest = std::fs::read_to_string(vendor_root.join("Cargo.toml"))
        .map_err(|error| format!("cannot read vendored workspace root: {error}"))?;
    for (key, value) in PINNED_PACKAGE {
        let needle = format!("{key} = {value}");
        if !root_manifest.contains(&needle) {
            return Err(format!(
                "pin drift: inner root lacks `{needle}`; update PINNED_PACKAGE"
            ));
        }
    }
    if !root_manifest.contains("[workspace.lints.rust]")
        || !root_manifest.contains("[workspace.lints.clippy]")
    {
        return Err("pin drift: inner root lints moved; update LINTS_TABLE".to_string());
    }
    let mut dirty = 0;
    let mut manifests: Vec<PathBuf> = std::fs::read_dir(vendor_root.join("crates"))
        .map_err(|error| format!("cannot list vendored crates: {error}"))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path().join("Cargo.toml")))
        .filter(|path| path.is_file())
        .collect();
    manifests.sort();
    for manifest in &manifests {
        let original = std::fs::read_to_string(manifest)
            .map_err(|error| format!("cannot read {}: {error}", manifest.display()))?;
        let mut text = original.clone();
        for (key, value) in PINNED_PACKAGE {
            text = text.replace(
                &format!("{key}.workspace = true"),
                &format!("{key} = {value}"),
            );
        }
        // Drop readme inheritance (no per-crate README to point at).
        text = text
            .lines()
            .filter(|line| line.trim() != "readme.workspace = true")
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        text = text.replace("[lints]\nworkspace = true", LINTS_TABLE);
        if text != original {
            dirty += 1;
            if !check {
                std::fs::write(manifest, text)
                    .map_err(|error| format!("cannot write {}: {error}", manifest.display()))?;
            }
        }
    }
    if check && dirty > 0 {
        return Err(format!("{dirty} manifest(s) need materializing"));
    }
    println!(
        "materialized {dirty} manifest(s){}",
        if check { " (check)" } else { "" }
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal fake vendored workspace: root + one inheriting crate.
    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("tail");
        std::fs::create_dir_all(root.join("crates/demo")).unwrap();
        let mut manifest = String::from("[workspace]\n[workspace.package]\n");
        for (key, value) in PINNED_PACKAGE {
            manifest.push_str(&format!("{key} = {value}\n"));
        }
        manifest.push_str("[workspace.lints.rust]\n[workspace.lints.clippy]\n");
        std::fs::write(root.join("Cargo.toml"), manifest).unwrap();
        std::fs::write(
            root.join("crates/demo/Cargo.toml"),
            "[package]\nname = \"demo\"\nedition.workspace = true\nversion.workspace = true\n\
             readme.workspace = true\n\n[lints]\nworkspace = true\n",
        )
        .unwrap();
        dir
    }

    #[test]
    fn materializes_inherited_keys() {
        let dir = fixture();
        let root = dir.path().join("tail");
        materialize_tailwind(&root, false).unwrap();
        let text = std::fs::read_to_string(root.join("crates/demo/Cargo.toml")).unwrap();
        assert!(text.contains("edition = \"2021\""), "{text}");
        assert!(text.contains("version = \"0.1.0\""), "{text}");
        assert!(!text.contains("workspace = true"), "{text}");
        assert!(!text.contains("readme"), "{text}");
        assert!(text.contains("[lints.rust]"), "{text}");
        // Idempotent: second run changes nothing.
        materialize_tailwind(&root, true).unwrap();
    }

    #[test]
    fn check_reports_pending_work() {
        let dir = fixture();
        let error = materialize_tailwind(&dir.path().join("tail"), true).unwrap_err();
        assert!(error.contains("need materializing"), "{error}");
    }

    #[test]
    fn pin_drift_refuses_to_run() {
        let dir = fixture();
        let root = dir.path().join("tail");
        std::fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();
        let error = materialize_tailwind(&root, false).unwrap_err();
        assert!(error.contains("pin drift"), "{error}");
    }
}
