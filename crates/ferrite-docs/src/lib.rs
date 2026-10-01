//! Markdown docs plugin (`ferrite:markdown`).
//!
//! `.md` files become JavaScript modules exporting rendered HTML plus
//! frontmatter metadata:
//!
//! ```js
//! import page, { title } from "./guide.md";
//! document.querySelector("#app").innerHTML = page.html;
//! ```
//!
//! Module shape: default export `{ html, title, headings }` plus named
//! `title` / `headings` re-exports. `headings` is `[{ level, text, id }]`
//! for tables of contents. Only `---` frontmatter (`title:` / `order:`)
//! is parsed; anything else is loud-free and ignored.

use std::path::PathBuf;

use ferrite_core::{ModuleType, Result};
use ferrite_plugin::{LoadRequest, LoadResult, Plugin, PluginContext, ResolveHookRequest};
use pulldown_cmark::{html, Options, Parser};

/// One extracted heading (`{ level, text, id }`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    /// Heading level (1–6).
    pub level: u8,
    /// Plain heading text.
    pub text: String,
    /// Slugified anchor id.
    pub id: String,
}

/// Parsed frontmatter (`---` block; `title:` / `order:` only).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Frontmatter {
    /// Page title override.
    pub title: Option<String>,
    /// Sidebar sort order (lower first).
    pub order: Option<i64>,
}

/// Split `---` frontmatter from the body. Returns `(meta, body)`.
#[must_use]
pub fn split_frontmatter(source: &str) -> (Frontmatter, &str) {
    let mut meta = Frontmatter::default();
    let Some(rest) = source.strip_prefix("---") else {
        return (meta, source);
    };
    // First line after `---` must end (no `--- title` one-liners).
    let Some(after_open) = rest.strip_prefix(['\n', '\r']) else {
        return (meta, source);
    };
    let body_start = after_open
        .lines()
        .position(|line| line.trim() == "---")
        .map(|index| {
            after_open
                .lines()
                .take(index + 1)
                .map(|line| line.len() + 1)
                .sum::<usize>()
        });
    let Some(offset) = body_start else {
        return (meta, source);
    };
    let (front, body) = after_open.split_at(offset);
    for line in front.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        match key.trim() {
            "title" => meta.title = Some(value.trim().trim_matches('"').to_string()),
            "order" => meta.order = value.trim().parse().ok(),
            _ => {}
        }
    }
    (meta, body)
}

/// Slugify heading text into an anchor id.
#[must_use]
pub fn slugify(text: &str) -> String {
    let mut slug = String::with_capacity(text.len());
    let mut dash = false;
    for ch in text.chars().flat_map(|ch| ch.to_lowercase()) {
        if ch.is_alphanumeric() {
            slug.push(ch);
            dash = false;
        } else if !dash && !slug.is_empty() {
            slug.push('-');
            dash = true;
        }
    }
    slug.trim_end_matches('-').to_string()
}

/// Render Markdown to HTML with GitHub-flavored extensions.
#[must_use]
pub fn render_markdown(body: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    let parser = Parser::new_ext(body, options);
    let mut html = String::new();
    html::push_html(&mut html, parser);
    html
}

/// Collect headings (level, text, id) from a Markdown body.
#[must_use]
pub fn extract_headings(body: &str) -> Vec<Heading> {
    use pulldown_cmark::{Event, Tag, TagEnd};
    let mut headings = Vec::new();
    let mut current: Option<(u8, String)> = None;
    for event in Parser::new(body) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                current = Some((level as u8, String::new()));
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, text)) = current.take() {
                    let text = text.trim().to_string();
                    let id = slugify(&text);
                    headings.push(Heading { level, text, id });
                }
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some((_, acc)) = current.as_mut() {
                    acc.push_str(&text);
                }
            }
            _ => {}
        }
    }
    headings
}

/// Inject `id` anchors into rendered `<hN>` tags (in heading order).
#[must_use]
pub fn anchor_headings(html: &str, headings: &[Heading]) -> String {
    let mut out = html.to_string();
    for heading in headings {
        let open = format!("<h{}>", heading.level);
        let anchored = format!("<h{} id=\"{}\">", heading.level, heading.id);
        if let Some(pos) = out.find(&open) {
            out.replace_range(pos..pos + open.len(), &anchored);
        }
    }
    out
}

/// Render one `.md` file into its JS module source.
#[must_use]
pub fn render_module(path: &str, source: &str) -> String {
    let (meta, body) = split_frontmatter(source);
    let headings = extract_headings(body);
    let html = anchor_headings(&render_markdown(body), &headings);
    let title = meta.title.or_else(|| {
        headings
            .iter()
            .find(|heading| heading.level == 1)
            .map(|heading| heading.text.clone())
    });
    let headings_json: Vec<serde_json::Value> = headings
        .iter()
        .map(|heading| {
            serde_json::json!({"level": heading.level, "text": heading.text, "id": heading.id})
        })
        .collect();
    let module = serde_json::json!({
        "path": path,
        "title": title,
        "order": meta.order,
        "headings": headings_json,
        "html": html,
    });
    format!(
        "const page = {};\nexport const title = page.title;\n\
         export const headings = page.headings;\nexport default page;\n",
        module
    )
}

/// Join an importer-relative specifier lexically (plugin hooks run first).
fn join_relative(importer: &str, specifier: &str) -> String {
    let base = importer.split('?').next().unwrap_or(importer);
    let dir = base.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("");
    let mut parts: Vec<&str> = dir.split('/').filter(|part| !part.is_empty()).collect();
    for part in specifier.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    format!("/{}", parts.join("/"))
}

/// Markdown docs plugin (`ferrite:markdown`).
#[derive(Debug)]
pub struct MarkdownPlugin {
    root: PathBuf,
}

impl MarkdownPlugin {
    /// Create for project `root` (`.md` files resolve under it).
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn read_markdown(&self, id: &str) -> Result<String> {
        // Reject escapes: `..` past the root is refused, the rest joins.
        let mut parts: Vec<&str> = Vec::new();
        for part in id.split('/') {
            match part {
                "" | "." => {}
                ".." => {
                    if parts.pop().is_none() {
                        return Err(ferrite_core::FerriteError::Build(format!(
                            "markdown: refusing escape `{id}`"
                        )));
                    }
                }
                _ => parts.push(part),
            }
        }
        let mut path = self.root.clone();
        path.extend(parts);
        if path != self.root && !path.starts_with(&self.root) {
            return Err(ferrite_core::FerriteError::Build(format!(
                "markdown: refusing escape `{id}`"
            )));
        }
        std::fs::read_to_string(&path).map_err(|error| {
            ferrite_core::FerriteError::Build(format!("markdown: cannot read `{id}`: {error}"))
        })
    }
}

#[async_trait::async_trait]
impl Plugin for MarkdownPlugin {
    fn name(&self) -> &'static str {
        "ferrite:markdown"
    }

    async fn resolve_id(
        &self,
        _ctx: &PluginContext,
        request: ResolveHookRequest<'_>,
    ) -> Result<Option<ferrite_resolver::ResolvedId>> {
        if !request.specifier.ends_with(".md") {
            return Ok(None);
        }
        let id = if request.specifier.starts_with('.') {
            let importer = request.importer.map(|id| id.0.as_str()).unwrap_or("/");
            join_relative(importer, request.specifier)
        } else {
            request.specifier.to_string()
        };
        Ok(Some(ferrite_resolver::ResolvedId::new(id)))
    }

    async fn load(&self, _ctx: &PluginContext, request: LoadRequest) -> Result<Option<LoadResult>> {
        if !request.id.ends_with(".md") {
            return Ok(None);
        }
        let source = self.read_markdown(&request.id)?;
        Ok(Some(LoadResult {
            code: render_module(&request.id, &source),
            module_type: ModuleType::Js,
            dependencies: Vec::new(),

            ..Default::default()
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn frontmatter_splits_and_parses() {
        let (meta, body) = split_frontmatter("---\ntitle: Hello\norder: 3\n---\n# Hi\n");
        assert_eq!(
            meta,
            Frontmatter {
                title: Some("Hello".to_string()),
                order: Some(3),
            }
        );
        assert_eq!(body, "# Hi\n");
    }

    #[test]
    fn missing_or_broken_frontmatter_passes_through() {
        let (meta, body) = split_frontmatter("# No frontmatter\n");
        assert_eq!(meta, Frontmatter::default());
        assert_eq!(body, "# No frontmatter\n");
        let (meta, body) = split_frontmatter("---\ntitle: unterminated\n");
        assert_eq!(meta, Frontmatter::default());
        assert!(body.starts_with("---"));
    }

    #[test]
    fn renders_gfm_and_anchors() {
        let html = anchor_headings(
            &render_markdown("# Guide & Intro\n\n| a | b |\n|---|---|\n| 1 | 2 |\n"),
            &extract_headings("# Guide & Intro\n"),
        );
        assert!(html.contains("<h1 id=\"guide-intro\">"), "{html}");
        assert!(html.contains("<table>"), "{html}");
    }

    #[test]
    fn module_exports_page_shape() {
        let code = render_module("/docs/guide.md", "---\ntitle: Guide\n---\n# Guide\n\nHi.\n");
        assert!(code.contains("export default page;"), "{code}");
        assert!(code.contains("\"title\":\"Guide\""), "{code}");
        assert!(code.contains("\"path\":\"/docs/guide.md\""), "{code}");
        // Falls back to the first h1 when no frontmatter title.
        let fallback = render_module("/x.md", "# Fallback\n");
        assert!(fallback.contains("\"title\":\"Fallback\""), "{fallback}");
    }

    #[test]
    fn escape_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let plugin = MarkdownPlugin::new(dir.path().to_path_buf());
        // `..` survives lexical join but must not escape root on read.
        let error = plugin.read_markdown("/../../etc/hostname").unwrap_err();
        assert!(error.to_string().contains("escape"), "{error}");
    }

    #[tokio::test]
    async fn plugin_claims_md_only() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("guide.md"), "# Guide\n").unwrap();
        let plugin = MarkdownPlugin::new(dir.path().to_path_buf());
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
        let importer = ferrite_core::ModuleId::new("/src/main.js");
        let resolved = plugin
            .resolve_id(
                &ctx,
                ResolveHookRequest {
                    kind: ferrite_resolver::ResolveKind::Import,
                    specifier: "../guide.md",
                    importer: Some(&importer),
                    environment: ferrite_core::EnvironmentKind::Client,
                    ssr: false,
                },
            )
            .await
            .unwrap()
            .expect("claimed");
        assert_eq!(resolved.id.0, "/guide.md");
        let skipped = plugin
            .resolve_id(
                &ctx,
                ResolveHookRequest {
                    kind: ferrite_resolver::ResolveKind::Import,
                    specifier: "./app.js",
                    importer: Some(&importer),
                    environment: ferrite_core::EnvironmentKind::Client,
                    ssr: false,
                },
            )
            .await
            .unwrap();
        assert!(skipped.is_none());
        let loaded = plugin
            .load(
                &ctx,
                LoadRequest {
                    id: "/guide.md".to_string(),
                    environment: ferrite_core::EnvironmentKind::Client,
                },
            )
            .await
            .unwrap()
            .expect("loaded");
        assert!(matches!(loaded.module_type, ModuleType::Js));
        assert!(
            loaded.code.contains("export default page;"),
            "{}",
            loaded.code
        );
    }
}
