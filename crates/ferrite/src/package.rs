//! Single-binary packaging + cross-target builds (spec §51–§54).
//!
//! `ferrite build --standalone` writes a self-contained Rust server scaffold
//! next to `dist/`. With `embed_assets` (default) every output file is baked
//! into the binary via `include_bytes!` (optionally gzipped), so the result
//! is one file that serves the app with no sibling `dist/`. With `--target
//! <triple>` the scaffold is compiled in place (`cargo build --release
//! --target`), with loud errors when the toolchain or target is missing.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use ferrite_core::{FerriteError, Result};

/// Scaffold directory name inside the output dir.
pub const SCAFFOLD_DIR: &str = "standalone";
/// Scaffold binary name.
pub const APP_NAME: &str = "ferrite-app";

/// Standalone packaging options (from [`ferrite_config::PackageConfig`]).
#[derive(Debug, Clone)]
pub struct StandaloneOptions {
    /// Bake output files into the binary (default true).
    pub embed_assets: bool,
    /// Gzip embedded files (default true; adds a `flate2` scaffold dep).
    pub compress_assets: bool,
    /// Cross-compilation triple (`cargo build --target`), when set.
    pub target: Option<String>,
    /// Explicit `cargo` binary (caller-owned; `None` = resolve from `PATH`).
    pub cargo: Option<PathBuf>,
}

/// What [`write_standalone`] produced.
#[derive(Debug, Clone)]
pub struct StandaloneReport {
    /// Scaffold directory.
    pub dir: PathBuf,
    /// Embedded/served files.
    pub files: usize,
    /// Raw output bytes covered.
    pub bytes: u64,
    /// Bytes that land in the binary (post-gzip when compressed).
    pub embedded_bytes: u64,
    /// Built binary (copied next to the output dir), when `--target` built.
    pub binary: Option<PathBuf>,
}

pub(crate) fn unsupported_ssr_standalone() -> FerriteError {
    FerriteError::Build("SSR standalone packaging is unavailable: the current scaffold serves static files and cannot execute a renderer. Build without --standalone and use ferrite preview with the explicitly selected napi-vm runtime; standalone SSR renderer packaging must be implemented before this profile can be advertised".into())
}

/// MIME type for a web path (generated server + tests share this).
#[must_use]
pub fn content_type_for(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "wasm" => "application/wasm",
        "txt" | "md" => "text/plain; charset=utf-8",
        "xml" => "application/xml; charset=utf-8",
        "pdf" => "application/pdf",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        _ => "application/octet-stream",
    }
}

/// Walk `out_dir` into web-path → disk-path pairs, skipping the scaffold
/// itself. Web paths are `/`-rooted with `/` separators, sorted.
pub fn collect_assets(out_dir: &Path) -> Result<BTreeMap<String, PathBuf>> {
    let mut files = BTreeMap::new();
    let private_server = out_dir.join("server");
    let has_server = private_server.join("manifest.json").is_file()
        || private_server
            .join(crate::SsrRendererArtifact::FILE_NAME)
            .is_file();
    let mut stack = vec![out_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).map_err(|error| {
            FerriteError::Build(format!("cannot read {}: {error}", dir.display()))
        })?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                FerriteError::Build(format!("cannot read {}: {error}", dir.display()))
            })?;
            let path = entry.path();
            if path.is_dir() {
                if has_server && path == private_server {
                    continue;
                }
                if path.file_name().and_then(|name| name.to_str()) == Some(SCAFFOLD_DIR)
                    && path.parent() == Some(out_dir)
                {
                    continue;
                }
                stack.push(path);
            } else if path.is_file() {
                let rel = path.strip_prefix(out_dir).map_err(|error| {
                    FerriteError::Build(format!("cannot relativize {}: {error}", path.display()))
                })?;
                let mut web = String::from("/");
                web.push_str(&rel.to_string_lossy().replace('\\', "/"));
                files.insert(web, path);
            }
        }
    }
    Ok(files)
}

/// Gzip `bytes` (deterministic: mtime 0).
fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(bytes).expect("gzip write");
    encoder.finish().expect("gzip finish")
}

/// Render `src/assets.rs`: one `include_bytes!` per file plus a lookup.
///
/// `entries` is `(web path, include path relative to src/, gzipped)`.
#[must_use]
pub fn render_assets_rs(entries: &[(String, String, bool)], compress: bool) -> String {
    let mut out = String::from(
        "//! Generated by `ferrite build --standalone`: embedded output files.\n\
         //! Do not edit; regenerate with a fresh build.\n\
         \n\
         /// One embedded file.\n\
         pub struct File {\n\
         /// Web path (`/assets/main-abc.js`).\n\
         pub path: &'static str,\n\
         /// MIME type.\n\
         pub mime: &'static str,\n\
         /// File bytes (gzipped when `gzipped`).\n\
         pub bytes: &'static [u8],\n\
         /// True when `bytes` need gunzipping.\n\
         pub gzipped: bool,\n\
         }\n\
         \n\
         /// Embedded files.\n\
         pub const FILES: &[File] = &[\n",
    );
    for (web, include, gzipped) in entries {
        out.push_str(&format!(
            "    File {{ path: {web:?}, mime: {:?}, bytes: include_bytes!({include:?}), gzipped: {gzipped} }},\n",
            content_type_for(web),
        ));
    }
    out.push_str("];\n\n/// Look up a web path.\n#[must_use]\n");
    if compress {
        out.push_str(
            "pub fn get(path: &str) -> Option<(&'static str, Vec<u8>)> {\n\
             let file = FILES.iter().find(|file| file.path == path)?;\n\
             if file.gzipped {\n\
             let mut decoder = flate2::read::GzDecoder::new(file.bytes);\n\
             let mut out = Vec::new();\n\
             std::io::Read::read_to_end(&mut decoder, &mut out).ok()?;\n\
             Some((file.mime, out))\n\
             } else {\n\
             Some((file.mime, file.bytes.to_vec()))\n\
             }\n\
             }\n",
        );
    } else {
        out.push_str(
            "pub fn get(path: &str) -> Option<(&'static str, Vec<u8>)> {\n\
             let file = FILES.iter().find(|file| file.path == path)?;\n\
             Some((file.mime, file.bytes.to_vec()))\n\
             }\n",
        );
    }
    out
}

/// Render the scaffold server: embedded lookup + SPA fallback, or a
/// sibling-`dist/` static server when not embedding.
fn render_main_rs(embed: bool) -> String {
    if embed {
        String::from(
            "//! Generated by `ferrite build --standalone`.\n\
             //! Serves files embedded in this binary. `PORT`/`HOST` configure binding.\n\
             \n\
             mod assets;\n\
             \n\
             use axum::http::{StatusCode, Uri};\n\
             use axum::response::{IntoResponse, Response};\n\
             \n\
             #[tokio::main]\n\
             async fn main() {\n\
             let host = std::env::var(\"HOST\").unwrap_or_else(|_| \"0.0.0.0\".to_string());\n\
             let port: u16 = std::env::var(\"PORT\")\n\
             .ok()\n\
             .and_then(|port| port.parse().ok())\n\
             .unwrap_or(8080);\n\
             let app = axum::Router::new().fallback(handler);\n\
             let listener = tokio::net::TcpListener::bind(format!(\"{host}:{port}\"))\n\
             .await\n\
             .unwrap();\n\
             println!(\"listening on http://{host}:{port}\");\n\
             axum::serve(listener, app).await.unwrap();\n\
             }\n\
             \n\
             async fn handler(uri: Uri) -> Response {\n\
             let mut path = uri.path().to_string();\n\
             if path.ends_with('/') {\n\
             path.push_str(\"index.html\");\n\
             }\n\
             if let Some(found) = assets::get(&path) {\n\
             return respond(found);\n\
             }\n\
             // SPA fallback: extensionless paths serve the shell.\n\
             let leaf = path.rsplit('/').next().unwrap_or(\"\");\n\
             if !leaf.contains('.') {\n\
             if let Some(found) = assets::get(\"/index.html\") {\n\
             return respond(found);\n\
             }\n\
             }\n\
             StatusCode::NOT_FOUND.into_response()\n\
             }\n\
             \n\
             fn respond((mime, bytes): (&str, Vec<u8>)) -> Response {\n\
             let mut headers = axum::http::HeaderMap::new();\n\
             headers.insert(\n\
             axum::http::header::CONTENT_TYPE,\n\
             mime.parse().expect(\"valid mime\"),\n\
             );\n\
             (headers, bytes).into_response()\n\
             }\n",
        )
    } else {
        String::from(
            "//! Generated by `ferrite build --standalone`.\n\
             //! Serves the sibling `dist/` output. `PORT`/`HOST` configure binding.\n\
             \n\
             #[tokio::main]\n\
             async fn main() {\n\
             let host = std::env::var(\"HOST\").unwrap_or_else(|_| \"0.0.0.0\".to_string());\n\
             let port: u16 = std::env::var(\"PORT\")\n\
             .ok()\n\
             .and_then(|port| port.parse().ok())\n\
             .unwrap_or(8080);\n\
             let app = axum::Router::new().fallback(tower_http::services::ServeDir::new(\"..\")\n\
             .append_index_html_on_directories(true));\n\
             let listener = tokio::net::TcpListener::bind(format!(\"{host}:{port}\"))\n\
             .await\n\
             .unwrap();\n\
             println!(\"listening on http://{host}:{port}\");\n\
             axum::serve(listener, app).await.unwrap();\n\
             }\n",
        )
    }
}

fn render_cargo_toml(embed: bool, compress: bool) -> String {
    let mut out = String::from(
        "[package]\nname = \"ferrite-app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n\
         [dependencies]\n\
         axum = { version = \"0.8\", features = [\"ws\"] }\n\
         tokio = { version = \"1\", features = [\"full\"] }\n",
    );
    if compress {
        out.push_str("flate2 = \"1\"\n");
    }
    if !embed {
        out.push_str("tower-http = { version = \"0.6\", features = [\"fs\"] }\n");
    }
    out
}

/// Reject malformed target triples before invoking cargo.
fn validate_triple(target: &str) -> Result<()> {
    let ok = !target.is_empty()
        && target.len() <= 128
        && target.contains('-')
        && !target.contains('/')
        && !target.contains('\\')
        && !target.contains('.')
        && target
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok {
        Ok(())
    } else {
        Err(FerriteError::Build(format!(
            "invalid target triple `{target}` (expected e.g. `x86_64-unknown-linux-musl`)"
        )))
    }
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join(name))
            .find(|path| {
                path.is_file()
                    && path
                        .metadata()
                        .map(|meta| {
                            std::os::unix::fs::PermissionsExt::mode(&meta.permissions()) & 0o111
                                != 0
                        })
                        .unwrap_or(false)
            })
    })
}

/// True when the target's std is installed. Prefers `rustup`; without it,
/// accepts any triple `rustc` knows and lets cargo fail loudly otherwise.
fn target_installed(target: &str) -> Result<bool> {
    if let Some(rustup) = find_on_path("rustup") {
        let output = Command::new(rustup)
            .args(["target", "list", "--installed"])
            .output()
            .map_err(|error| FerriteError::Build(format!("cannot run rustup: {error}")))?;
        let list = String::from_utf8_lossy(&output.stdout);
        return Ok(list.lines().any(|line| line.trim() == target));
    }
    let Some(rustc) = find_on_path("rustc") else {
        return Err(FerriteError::Build(
            "cannot check target: neither `rustup` nor `rustc` is on PATH".to_string(),
        ));
    };
    let output = Command::new(rustc)
        .args(["--print", "target-list"])
        .output()
        .map_err(|error| FerriteError::Build(format!("cannot run rustc: {error}")))?;
    let list = String::from_utf8_lossy(&output.stdout);
    Ok(list.lines().any(|line| line.trim() == target))
}

/// Compile the scaffold for `target`; return the built binary path.
fn cargo_build(dir: &Path, target: &str, cargo: Option<&Path>) -> Result<PathBuf> {
    validate_triple(target)?;
    if !target_installed(target)? {
        return Err(FerriteError::Build(format!(
            "target `{target}` is not installed (run `rustup target add {target}`)"
        )));
    }
    let cargo = match cargo {
        Some(path) => path.to_path_buf(),
        None => find_on_path("cargo").ok_or_else(|| {
            FerriteError::Build("cannot cross-compile: `cargo` is not on PATH".to_string())
        })?,
    };
    let output = Command::new(&cargo)
        .arg("build")
        .arg("--release")
        .arg("--target")
        .arg(target)
        .current_dir(dir)
        .output()
        .map_err(|error| FerriteError::Build(format!("cannot run {}: {error}", cargo.display())))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let tail: Vec<&str> = stderr.lines().rev().take(15).collect();
        return Err(FerriteError::Build(format!(
            "scaffold build for `{target}` failed:\n{}",
            tail.into_iter().rev().collect::<Vec<_>>().join("\n")
        )));
    }
    let exe = if target.contains("windows") {
        format!("{APP_NAME}.exe")
    } else {
        APP_NAME.to_string()
    };
    let binary = dir.join("target").join(target).join("release").join(exe);
    if !binary.is_file() {
        return Err(FerriteError::Build(format!(
            "cargo reported success but {} is missing",
            binary.display()
        )));
    }
    Ok(binary)
}

/// Write the standalone scaffold into `out_dir`, embedding output files and
/// optionally cross-compiling for `opts.target` (§51–§54).
pub fn write_standalone(out_dir: &Path, opts: &StandaloneOptions) -> Result<StandaloneReport> {
    if out_dir.join("server/manifest.json").is_file()
        || out_dir
            .join("server")
            .join(crate::SsrRendererArtifact::FILE_NAME)
            .is_file()
    {
        return Err(unsupported_ssr_standalone());
    }
    let dir = out_dir.join(SCAFFOLD_DIR);
    // Never embed a previous scaffold into the next one.
    let assets = collect_assets(out_dir)?;
    let bytes: u64 = assets
        .values()
        .map(|path| path.metadata().map(|meta| meta.len()).unwrap_or(0))
        .sum();
    std::fs::create_dir_all(dir.join("src"))?;
    let mut embedded_bytes = 0u64;
    if opts.embed_assets {
        let embed_dir = dir.join(".embed");
        if opts.compress_assets {
            // Fresh gzips: stale files would shadow changed outputs.
            if embed_dir.exists() {
                std::fs::remove_dir_all(&embed_dir)?;
            }
            std::fs::create_dir_all(&embed_dir)?;
        }
        let mut entries = Vec::new();
        for (index, (web, path)) in assets.iter().enumerate() {
            let raw = std::fs::read(path)?;
            if opts.compress_assets {
                let gz = gzip(&raw);
                embedded_bytes += gz.len() as u64;
                let name = format!("{index}.gz");
                std::fs::write(embed_dir.join(&name), &gz)?;
                entries.push((web.clone(), format!("../.embed/{name}"), true));
            } else {
                embedded_bytes += raw.len() as u64;
                let rel = path
                    .strip_prefix(out_dir)
                    .map_err(|error| {
                        FerriteError::Build(format!(
                            "cannot relativize {}: {error}",
                            path.display()
                        ))
                    })?
                    .to_string_lossy()
                    .replace('\\', "/");
                entries.push((web.clone(), format!("../{rel}"), false));
            }
        }
        std::fs::write(
            dir.join("src/assets.rs"),
            render_assets_rs(&entries, opts.compress_assets),
        )?;
    }
    std::fs::write(
        dir.join("Cargo.toml"),
        render_cargo_toml(opts.embed_assets, opts.embed_assets && opts.compress_assets),
    )?;
    std::fs::write(dir.join("src/main.rs"), render_main_rs(opts.embed_assets))?;
    let binary = match opts.target.as_deref() {
        Some(target) => {
            let built = cargo_build(&dir, target, opts.cargo.as_deref())?;
            let file = built
                .file_name()
                .map(|name| format!("{APP_NAME}-{target}-{}", name.to_string_lossy()))
                .unwrap_or_else(|| format!("{APP_NAME}-{target}"));
            let dest = out_dir.join(file);
            std::fs::copy(&built, &dest)?;
            Some(dest)
        }
        None => None,
    };
    std::fs::write(dir.join("STANDALONE.md"), render_readme(opts, &binary))?;
    std::fs::write(dir.join("Dockerfile"), render_dockerfile(opts))?;
    Ok(StandaloneReport {
        dir,
        files: assets.len(),
        bytes,
        embedded_bytes,
        binary,
    })
}

fn render_readme(opts: &StandaloneOptions, binary: &Option<PathBuf>) -> String {
    let mut out = String::from("# Standalone deployment\n\n");
    if opts.embed_assets {
        out.push_str(&format!(
            "The server binary embeds every output file{}: deploy the binary \
             alone, no `dist/` needed.\n\n",
            if opts.compress_assets {
                " (gzipped)"
            } else {
                ""
            }
        ));
    } else {
        out.push_str(
            "The server binary serves the sibling `dist/` output: deploy the \
             binary next to it.\n\n",
        );
    }
    out.push_str("```bash\ncd standalone\ncargo build --release\nPORT=8080 ./target/release/ferrite-app\n```\n");
    if let Some(target) = opts.target.as_deref() {
        out.push_str(&format!(
            "\nBuilt for `{target}` via `cargo build --release --target {target}`.\n"
        ));
    }
    if let Some(binary) = binary {
        out.push_str(&format!(
            "\nPrebuilt artifact: `{}`.\n",
            binary
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default()
        ));
    }
    out.push_str("\nOr with Docker (`docker build -f Dockerfile ..` from this directory).\n");
    out
}

fn render_dockerfile(opts: &StandaloneOptions) -> String {
    if opts.embed_assets {
        String::from(
            "FROM debian:stable-slim AS runtime\n\
             WORKDIR /srv\n\
             COPY target/release/ferrite-app /srv/ferrite-app\n\
             ENV HOST=0.0.0.0 PORT=8080\n\
             EXPOSE 8080\n\
             ENTRYPOINT [\"/srv/ferrite-app\"]\n",
        )
    } else {
        String::from(
            "FROM debian:stable-slim AS runtime\n\
             WORKDIR /srv\n\
             COPY ../assets ./assets\n\
             COPY ../index.html ../manifest.json ./\n\
             COPY target/release/ferrite-app /srv/ferrite-app\n\
             ENV HOST=0.0.0.0 PORT=8080\n\
             EXPOSE 8080\n\
             ENTRYPOINT [\"/srv/ferrite-app\"]\n",
        )
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read as _;

    use super::*;

    fn options() -> StandaloneOptions {
        StandaloneOptions {
            embed_assets: true,
            compress_assets: true,
            target: None,
            cargo: None,
        }
    }

    static FIXTURE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    fn unique_dir(prefix: &str) -> PathBuf {
        let seq = FIXTURE_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        std::env::temp_dir().join(format!("{prefix}-{}-{seq}", std::process::id()))
    }

    fn dist_fixture() -> PathBuf {
        let dir = unique_dir("ferrite-pkg");
        std::fs::create_dir_all(dir.join("assets")).unwrap();
        std::fs::write(dir.join("index.html"), "<!doctype html><html></html>\n").unwrap();
        std::fs::write(dir.join("assets").join("a-1.js"), "console.log(1);\n").unwrap();
        dir
    }

    #[test]
    fn mime_types() {
        assert_eq!(content_type_for("/a.js"), "text/javascript; charset=utf-8");
        assert_eq!(content_type_for("/a.css"), "text/css; charset=utf-8");
        assert_eq!(content_type_for("/index.html"), "text/html; charset=utf-8");
        assert_eq!(content_type_for("/x.wasm"), "application/wasm");
        assert_eq!(content_type_for("/nope.bin"), "application/octet-stream");
    }

    #[test]
    fn collect_skips_scaffold() {
        let dir = dist_fixture();
        std::fs::create_dir_all(dir.join("standalone").join("src")).unwrap();
        std::fs::write(dir.join("standalone").join("x"), "y").unwrap();
        let assets = collect_assets(&dir).unwrap();
        assert_eq!(assets.len(), 2);
        assert!(assets.contains_key("/index.html"));
        assert!(assets.contains_key("/assets/a-1.js"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn gzip_roundtrips() {
        let raw = b"hello world, hello world, hello world\n".repeat(20);
        let gz = gzip(&raw);
        assert!(gz.len() < raw.len());
        let mut decoder = flate2::read::GzDecoder::new(gz.as_slice());
        let mut out = Vec::new();
        decoder.read_to_end(&mut out).unwrap();
        assert_eq!(out, raw);
    }

    #[test]
    fn assets_rs_embeds_entries() {
        let code = render_assets_rs(
            &[
                (
                    "/index.html".to_string(),
                    "../index.html".to_string(),
                    false,
                ),
                ("/a.css".to_string(), "../.embed/1.gz".to_string(), true),
            ],
            true,
        );
        assert!(code.contains("path: \"/index.html\""), "{code}");
        assert!(code.contains("include_bytes!(\"../index.html\")"), "{code}");
        assert!(
            code.contains("include_bytes!(\"../.embed/1.gz\")"),
            "{code}"
        );
        assert!(code.contains("GzDecoder"), "{code}");
        let plain = render_assets_rs(&[], false);
        assert!(!plain.contains("GzDecoder"), "{plain}");
    }

    #[test]
    fn triples_validate() {
        assert!(validate_triple("x86_64-unknown-linux-musl").is_ok());
        assert!(validate_triple("aarch64-apple-darwin").is_ok());
        for bad in ["", "x", "../evil", "a/b-c", "a b-c", "a.c-d", "x--y;rm"] {
            assert!(validate_triple(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn missing_target_fails_loudly() {
        let error = validate_triple("not a triple").unwrap_err();
        assert!(
            error.to_string().contains("invalid target triple"),
            "{error}"
        );
    }

    #[test]
    fn scaffold_embeds_and_reports() {
        let dir = dist_fixture();
        let report = write_standalone(&dir, &options()).unwrap();
        assert_eq!(report.files, 2);
        assert!(report.binary.is_none());
        assert!(report.dir.join("Cargo.toml").is_file());
        assert!(report.dir.join("src/main.rs").is_file());
        let assets = std::fs::read_to_string(report.dir.join("src/assets.rs")).unwrap();
        assert!(assets.contains("/index.html"), "{assets}");
        assert!(assets.contains("/assets/a-1.js"), "{assets}");
        assert!(report.dir.join(".embed/0.gz").is_file());
        assert!(report.dir.join("Dockerfile").is_file());
        // Second run refreshes gzips instead of stacking stale ones.
        let report = write_standalone(&dir, &options()).unwrap();
        assert_eq!(report.files, 2);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn uninstalled_target_fails_loudly() {
        let dir = dist_fixture();
        let mut opts = options();
        opts.target = Some("mips-unknown-linux-nonexistent".to_string());
        let error = write_standalone(&dir, &opts).unwrap_err();
        assert!(
            error.to_string().contains("not installed")
                || error.to_string().contains("target-list"),
            "{error}"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn fake_toolchain_builds_and_copies() {
        let dir = dist_fixture();
        let tools = unique_dir("ferrite-pkg-tools");
        std::fs::create_dir_all(&tools).unwrap();
        // Fake rustup: claims the triple installed.
        std::fs::write(
            tools.join("rustup"),
            "#!/bin/sh\necho fake-unknown-linux-fake\n",
        )
        .unwrap();
        // Fake cargo: records args, fabricates the binary.
        std::fs::write(
            tools.join("cargo"),
            "#!/bin/sh\necho \"$@\" > \"$FERRITE_ARGS\"\nmkdir -p target/fake-unknown-linux-fake/release\nprintf '#!/bin/sh\\n' > target/fake-unknown-linux-fake/release/ferrite-app\n",
        )
        .unwrap();
        for tool in ["rustup", "cargo"] {
            use std::os::unix::fs::PermissionsExt as _;
            let path = tools.join(tool);
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let args_file = tools.join("args.txt");
        // Serialized: no other test in this process resolves toolchains.
        static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = ENV_LOCK.lock().unwrap();
        let saved_path = std::env::var_os("PATH");
        unsafe {
            std::env::set_var("FERRITE_ARGS", &args_file);
            std::env::set_var(
                "PATH",
                format!("{}:{}", tools.display(), std::env::var("PATH").unwrap()),
            );
        }
        let mut opts = options();
        opts.target = Some("fake-unknown-linux-fake".to_string());
        let report = write_standalone(&dir, &opts).unwrap();
        unsafe {
            std::env::remove_var("FERRITE_ARGS");
            if let Some(path) = saved_path {
                std::env::set_var("PATH", path);
            }
        }
        drop(_guard);
        let args = std::fs::read_to_string(&args_file).unwrap();
        assert!(args.contains("--target fake-unknown-linux-fake"), "{args}");
        assert!(args.contains("--release"), "{args}");
        let binary = report.binary.expect("binary");
        assert!(binary.is_file(), "{}", binary.display());
        assert!(binary
            .file_name()
            .unwrap()
            .to_string_lossy()
            .contains("fake-unknown-linux-fake"));
        std::fs::remove_dir_all(&dir).unwrap();
        std::fs::remove_dir_all(&tools).unwrap();
    }
}
