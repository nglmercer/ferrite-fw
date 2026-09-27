//! Utility-CSS plugin backed by vendored `tailwind-rs`.
//!
//! [`TailwindPlugin`] serves the virtual module `ferrite:tailwind.css`: on
//! load it scans the project root for content files, extracts utility
//! candidates with the vendored `utilitycss` compiler, and returns the
//! generated CSS. Sites opt in with one import:
//!
//! ```js
//! import "ferrite:tailwind.css";
//! ```
//!
//! The vendored tree lives at `vendor/tailwind-rs` (see `vendor/README.md`
//! for the pinned upstream commit). This crate only depends on the
//! compiler-front crates (`utilitycss-compiler`, `utilitycss-span`).

use std::path::{Path, PathBuf};

use ferrite_core::{ModuleType, Result};
use ferrite_plugin::{LoadRequest, LoadResult, Plugin, PluginContext, ResolveHookRequest};
use utilitycss_compiler::{Compiler, CompilerConfig, SourceInput};
use utilitycss_span::SourceId;
use utilitycss_theme::Theme;

/// Virtual specifier sites import to get generated utilities.
pub const TAILWIND_SPEC: &str = "ferrite:tailwind.css";
/// Virtual module id (never hits the filesystem).
pub const TAILWIND_VIRTUAL: &str = "\0tailwind.css";

/// Content extensions scanned for utility candidates.
const CONTENT_EXTENSIONS: &[&str] = &[
    "html", "js", "mjs", "cjs", "jsx", "ts", "mts", "cts", "tsx", "md", "vue", "svelte", "astro",
];

/// Directories never scanned (build output, deps, vendored sources).
const SKIPPED_DIRS: &[&str] = &[
    "node_modules",
    "dist",
    "target",
    ".git",
    ".ferrite",
    "vendor",
    ".embed",
];

/// One compilation input: stable id + full text.
#[derive(Debug, Clone)]
pub struct ContentSource {
    /// Stable id (usually the root-relative path).
    pub id: String,
    /// Full source text.
    pub content: String,
}

/// Ferrite's theme: vendored baseline plus the color/width tokens the
/// docs site needs (upstream is pre-1.0 with a minimal palette).
#[must_use]
pub fn ferrite_theme() -> Theme {
    let mut theme = Theme::builder();
    for (key, value) in [
        ("stone-50", "#fafaf9"),
        ("stone-100", "#f5f5f4"),
        ("stone-200", "#e7e5e4"),
        ("stone-300", "#d6d3d1"),
        ("stone-400", "#a8a29e"),
        ("stone-500", "#78716c"),
        ("stone-600", "#57534e"),
        ("stone-700", "#44403c"),
        ("stone-800", "#292524"),
        ("stone-900", "#1c1917"),
        ("stone-950", "#0c0a09"),
        ("orange-50", "#fff7ed"),
        ("orange-100", "#ffedd5"),
        ("orange-200", "#fed7aa"),
        ("orange-300", "#fdba74"),
        ("orange-400", "#fb923c"),
        ("orange-600", "#ea580c"),
        ("orange-700", "#c2410c"),
        ("orange-800", "#9a3412"),
        ("orange-900", "#7c2d12"),
        ("orange-950", "#431407"),
    ] {
        theme = theme.color(key, value);
    }
    theme.width("6xl", "72rem").build()
}

/// Compile utility CSS for `sources` with the [`ferrite_theme`].
///
/// Returns the serialized CSS. Unknown candidates produce diagnostics
/// inside the compiler but never fail the build; only I/O-level or
/// oversized-source errors surface here.
pub fn compile_sources(sources: &[ContentSource]) -> Result<String> {
    let mut compiler = Compiler::new(CompilerConfig::new().with_theme(ferrite_theme()));
    for source in sources {
        let input = SourceInput::new(SourceId::new(source.id.clone()), source.content.clone());
        compiler.update_source(input).map_err(|error| {
            ferrite_core::FerriteError::Build(format!(
                "tailwind: cannot scan {}: {error}",
                source.id
            ))
        })?;
    }
    Ok(compiler.build().css().to_string())
}

/// Collect content files under `root` (sorted, deterministic).
#[must_use]
pub fn collect_content_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let walker = walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            entry
                .file_name()
                .to_str()
                .is_none_or(|name| !SKIPPED_DIRS.contains(&name))
        });
    for entry in walker.flatten() {
        let path = entry.path();
        if !entry.file_type().is_file() {
            continue;
        }
        let content = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| CONTENT_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()));
        if content {
            files.push(path.to_path_buf());
        }
    }
    files.sort();
    files
}

/// Read `files` into [`ContentSource`]s, skipping unreadable entries loudly.
fn read_sources(root: &Path, files: &[PathBuf]) -> Result<Vec<ContentSource>> {
    let mut sources = Vec::with_capacity(files.len());
    for path in files {
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let id = path
                    .strip_prefix(root)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .replace('\\', "/");
                sources.push(ContentSource { id, content });
            }
            Err(error) => {
                return Err(ferrite_core::FerriteError::Build(format!(
                    "tailwind: cannot read {}: {error}",
                    path.display()
                )));
            }
        }
    }
    Ok(sources)
}

/// Utility-CSS plugin (`ferrite:tailwind`).
#[derive(Debug)]
pub struct TailwindPlugin {
    root: PathBuf,
}

impl TailwindPlugin {
    /// Create for project `root` (content scan base).
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

#[async_trait::async_trait]
impl Plugin for TailwindPlugin {
    fn name(&self) -> &'static str {
        "ferrite:tailwind"
    }

    async fn resolve_id(
        &self,
        _ctx: &PluginContext,
        request: ResolveHookRequest<'_>,
    ) -> Result<Option<ferrite_resolver::ResolvedId>> {
        if request.specifier == TAILWIND_SPEC || request.specifier == TAILWIND_VIRTUAL {
            return Ok(Some(ferrite_resolver::ResolvedId::new(TAILWIND_VIRTUAL)));
        }
        Ok(None)
    }

    async fn load(&self, _ctx: &PluginContext, request: LoadRequest) -> Result<Option<LoadResult>> {
        if request.id != TAILWIND_VIRTUAL {
            return Ok(None);
        }
        let files = collect_content_files(&self.root);
        let sources = read_sources(&self.root, &files)?;
        let css = compile_sources(&sources)?;
        Ok(Some(LoadResult {
            code: css,
            module_type: ModuleType::Css,
            dependencies: Vec::new(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(id: &str, content: &str) -> ContentSource {
        ContentSource {
            id: id.to_string(),
            content: content.to_string(),
        }
    }

    #[test]
    fn compiles_known_utilities() {
        let css = compile_sources(&[source(
            "index.html",
            r#"<main class="flex gap-4 p-4 hover:bg-red-500 md:grid"></main>"#,
        )])
        .unwrap();
        assert!(css.contains(".flex"), "{css}");
        assert!(
            css.contains("display: flex") || css.contains("display:flex"),
            "{css}"
        );
        assert!(css.contains(".gap-4"), "{css}");
        assert!(css.contains(".p-4"), "{css}");
    }

    #[test]
    fn variants_extract_from_js_template_strings() {
        let css = compile_sources(&[source(
            "main.js",
            "const c = `block md:block hover:flex lg:hidden`;",
        )])
        .unwrap();
        assert!(css.contains(".md\\:block"), "{css}");
        assert!(css.contains(".hover\\:flex"), "{css}");
        assert!(css.contains(".lg\\:hidden"), "{css}");
    }

    #[test]
    fn ferrite_theme_covers_docs_palette() {
        let css = compile_sources(&[source(
            "index.html",
            r#"<div class="text-stone-900 bg-stone-950 border-stone-200 max-w-6xl hover:bg-orange-50 text-orange-800"></div>"#,
        )])
        .unwrap();
        assert!(css.contains(".text-stone-900"), "{css}");
        assert!(css.contains(".bg-stone-950"), "{css}");
        assert!(css.contains(".border-stone-200"), "{css}");
        assert!(css.contains(".max-w-6xl"), "{css}");
        assert!(css.contains(".hover\\:bg-orange-50"), "{css}");
        assert!(css.contains(".text-orange-800"), "{css}");
    }

    #[test]
    fn unknown_candidates_do_not_fail() {
        let css = compile_sources(&[source(
            "a.html",
            r#"<div class="not-a-real-utility-xyz"></div>"#,
        )])
        .unwrap();
        assert!(!css.contains("not-a-real-utility-xyz"), "{css}");
    }

    #[test]
    fn empty_sources_yield_empty_css() {
        let css = compile_sources(&[]).unwrap();
        assert!(css.trim().is_empty(), "{css:?}");
    }

    #[test]
    fn skips_build_and_vendor_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for rel in [
            "index.html",
            "dist/index.html",
            "node_modules/x/index.html",
            "vendor/tailwind-rs/x.html",
            "target/y.html",
            ".git/z.html",
        ] {
            let path = root.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, "<div></div>").unwrap();
        }
        std::fs::write(root.join("notes.txt"), "flex").unwrap();
        let files = collect_content_files(root);
        assert_eq!(files, vec![root.join("index.html")]);
    }

    #[test]
    fn unreadable_source_is_loud() {
        let dir = tempfile::tempdir().unwrap();
        let error = read_sources(dir.path(), &[dir.path().join("missing.html")]).unwrap_err();
        assert!(error.to_string().contains("cannot read"), "{error}");
    }

    fn test_ctx<'a>(
        graph: &'a ferrite_graph::ModuleGraph,
        resolver: &'a ferrite_resolver::Resolver,
        environment: &'a ferrite_core::Environment,
        emitted: &'a std::sync::Mutex<
            std::collections::HashMap<String, ferrite_plugin::EmittedFile>,
        >,
        watch_files: &'a std::sync::Mutex<Vec<String>>,
        warnings: &'a std::sync::Mutex<Vec<String>>,
    ) -> PluginContext<'a> {
        PluginContext {
            graph,
            resolver,
            environment,
            emitted,
            watch_files,
            warnings,
        }
    }

    #[tokio::test]
    async fn plugin_serves_virtual_css() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("index.html"),
            r#"<main class="flex"></main>"#,
        )
        .unwrap();
        let plugin = TailwindPlugin::new(dir.path().to_path_buf());
        let graph = ferrite_graph::ModuleGraph::new();
        let resolver = ferrite_resolver::Resolver::new(
            dir.path().to_path_buf(),
            &ferrite_config::ResolveConfig::default(),
        );
        let environment =
            ferrite_core::Environment::new("client", ferrite_core::EnvironmentKind::Client);
        let emitted = std::sync::Mutex::new(std::collections::HashMap::new());
        let watch_files = std::sync::Mutex::new(Vec::new());
        let warnings = std::sync::Mutex::new(Vec::new());
        let ctx = test_ctx(
            &graph,
            &resolver,
            &environment,
            &emitted,
            &watch_files,
            &warnings,
        );
        let resolved = plugin
            .resolve_id(
                &ctx,
                ResolveHookRequest {
                    specifier: TAILWIND_SPEC,
                    importer: None,
                    environment: ferrite_core::EnvironmentKind::Client,
                    ssr: false,
                },
            )
            .await
            .unwrap()
            .expect("claimed");
        assert_eq!(resolved.id.0, TAILWIND_VIRTUAL);
        let loaded = plugin
            .load(
                &ctx,
                LoadRequest {
                    id: TAILWIND_VIRTUAL.to_string(),
                    environment: ferrite_core::EnvironmentKind::Client,
                },
            )
            .await
            .unwrap()
            .expect("loaded");
        assert!(matches!(loaded.module_type, ModuleType::Css));
        assert!(loaded.code.contains(".flex"), "{}", loaded.code);
        // Unrelated ids pass through.
        let skipped = plugin
            .load(
                &ctx,
                LoadRequest {
                    id: "/src/app.css".to_string(),
                    environment: ferrite_core::EnvironmentKind::Client,
                },
            )
            .await
            .unwrap();
        assert!(skipped.is_none());
    }
}
