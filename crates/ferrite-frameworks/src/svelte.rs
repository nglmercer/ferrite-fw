//! Svelte experiment (`ferrite:svelte`, §70 roadmap).
//!
//! Mirrors the Vue plugin: splits `.svelte` files into `?svelte` virtual
//! modules (instance script through the core transform, markup render
//! stub, style through CSS). The render stub returns the markup string;
//! real compilation is roadmap.

use ferrite_core::{ModuleType, Result};
use ferrite_plugin::{LoadRequest, LoadResult, Plugin, PluginContext, ResolveHookRequest};

use crate::vue::{block_lang, SfcBlock};

/// Split a `.svelte` file: script/style blocks plus the remaining markup.
/// Unlike [`split_sfc`](crate::vue::split_sfc), markup tags are free-form.
pub fn split_svelte(source: &str) -> Result<(Vec<SfcBlock>, String)> {
    let mut blocks = Vec::new();
    let mut markup = source.to_string();
    // Extract script/style spans (later spans first for stable removal).
    let mut spans: Vec<(usize, usize, SfcBlock)> = Vec::new();
    for kind in ["script", "style"] {
        let mut rest = source;
        let mut base = 0;
        while let Some(open) = rest.find(&format!("<{kind}")) {
            // Skip `<scripting>`-style prefixes: require a boundary.
            let after = &rest[open + kind.len() + 1..];
            let boundary = after
                .chars()
                .next()
                .is_some_and(|c| c == '>' || c.is_whitespace() || c == '/');
            if !boundary {
                base += open + 1;
                rest = &source[base..];
                continue;
            }
            let Some(tag_end) = rest[open..].find('>') else {
                return Err(ferrite_core::FerriteError::Build(format!(
                    "malformed Svelte file: unclosed <{kind}>"
                )));
            };
            let tag = &rest[open + 1..open + tag_end];
            let attrs = tag
                .find(char::is_whitespace)
                .map(|space| tag[space..].trim().to_string())
                .unwrap_or_default();
            let body_start = open + tag_end + 1;
            let close = format!("</{kind}>");
            let Some(close_at) = rest[body_start..].find(close.as_str()) else {
                return Err(ferrite_core::FerriteError::Build(format!(
                    "malformed Svelte file: unclosed <{kind}> block"
                )));
            };
            spans.push((
                base + open,
                base + body_start + close_at + close.len(),
                SfcBlock {
                    kind: kind.to_string(),
                    attrs,
                    content: rest[body_start..body_start + close_at].to_string(),
                },
            ));
            base += body_start + close_at + close.len();
            rest = &source[base..];
        }
    }
    spans.sort_by_key(|(start, _, _)| *start);
    for (start, end, block) in spans.iter().rev() {
        markup.replace_range(*start..*end, "");
        blocks.push(block.clone());
    }
    blocks.reverse();
    Ok((blocks, markup.trim().to_string()))
}

/// Main-module code for `id` given its block layout.
#[must_use]
pub fn main_module_code(
    id: &str,
    has_markup: bool,
    style_count: usize,
    script_lang: &str,
) -> String {
    let script_spec = format!("{id}?svelte&type=script&lang={script_lang}");
    let markup_spec = format!("{id}?svelte&type=markup");
    let mut code = format!(
        "import __script from {script_spec:?};\n\
         import {{ render as __render }} from {markup_spec:?};\n"
    );
    for index in 0..style_count {
        let style_spec = format!("{id}?svelte&type=style&index={index}");
        code.push_str(&format!("import {style_spec:?};\n"));
    }
    if has_markup {
        code.push_str("__script.render = __render;\n");
    }
    code.push_str(&format!(
        "__script.__hmr_id = {id:?};\n\
         if (import.meta.hot) {{\n\
         import.meta.hot.accept();\n\
         }}\n\
         export default __script;\n"
    ));
    code
}

/// Experimental markup stub: render returns the markup string.
#[must_use]
pub fn markup_stub(markup: &str) -> String {
    format!(
        "// Experimental Svelte markup stub: real compilation is roadmap.\n\
         export function render() {{\n\
         return {};\n\
         }}\n",
        serde_json::to_string(markup).unwrap_or_else(|_| "\"\"".to_string())
    )
}

/// Extract the markup: source minus its script/style blocks.
#[must_use]
pub fn extract_markup(source: &str) -> String {
    split_svelte(source)
        .map(|(_, markup)| markup)
        .unwrap_or_default()
}

/// Svelte plugin (`ferrite:svelte`).
#[derive(Debug)]
pub struct SveltePlugin {
    /// Project root for file reads.
    root: std::path::PathBuf,
}

impl SveltePlugin {
    /// Create the plugin for `root`.
    #[must_use]
    pub fn new(root: std::path::PathBuf) -> Self {
        Self { root }
    }

    /// Read the file behind `id` (strips any `?svelte` query).
    fn read_file(&self, id: &str) -> Result<String> {
        let path = id.split('?').next().unwrap_or(id);
        let fs_path = self.root.join(path.trim_start_matches('/'));
        std::fs::read_to_string(&fs_path).map_err(|error| {
            ferrite_core::FerriteError::Build(format!("cannot read {id}: {error}"))
        })
    }
}

#[async_trait::async_trait]
impl Plugin for SveltePlugin {
    fn name(&self) -> &'static str {
        "ferrite:svelte"
    }

    async fn resolve_id(
        &self,
        _ctx: &PluginContext,
        request: ResolveHookRequest<'_>,
    ) -> Result<Option<ferrite_resolver::ResolvedId>> {
        if request.specifier.contains(".svelte") {
            let id = if request.specifier.starts_with('.') {
                let importer = request.importer.map(|id| id.0.as_str()).unwrap_or("/");
                crate::join_relative(importer, request.specifier)
            } else {
                request.specifier.to_string()
            };
            return Ok(Some(ferrite_resolver::ResolvedId::new(id)));
        }
        Ok(None)
    }

    async fn load(&self, _ctx: &PluginContext, request: LoadRequest) -> Result<Option<LoadResult>> {
        let (path, query) = match request.id.split_once('?') {
            Some((path, query)) => (path, Some(query)),
            None => (request.id.as_str(), None),
        };
        if !path.ends_with(".svelte") {
            return Ok(None);
        }
        let source = self.read_file(&request.id)?;
        let query = query.unwrap_or("");
        if query.contains("type=script") {
            // Instance script: first <script> block without `context`.
            let (blocks, _) = split_svelte(&source)?;
            let script = blocks
                .iter()
                .find(|block| block.kind == "script" && !block.attrs.contains("context"))
                .map(|block| (block.content.clone(), block_lang(&block.attrs, "js")))
                .unwrap_or_else(|| ("export default {};\n".to_string(), "js".to_string()));
            let module_type = match script.1.as_str() {
                "ts" => ModuleType::Ts,
                _ => ModuleType::Js,
            };
            return Ok(Some(LoadResult {
                code: script.0,
                module_type,
                dependencies: vec![path.to_string()],
            }));
        }
        if query.contains("type=markup") {
            return Ok(Some(LoadResult {
                code: markup_stub(&extract_markup(&source)),
                module_type: ModuleType::Js,
                dependencies: vec![path.to_string()],
            }));
        }
        if query.contains("type=style") {
            let (blocks, _) = split_svelte(&source)?;
            let styles: Vec<&crate::vue::SfcBlock> = blocks
                .iter()
                .filter(|block| block.kind == "style")
                .collect();
            let index: usize = query
                .split('&')
                .find_map(|part| part.strip_prefix("index=").and_then(|n| n.parse().ok()))
                .unwrap_or(0);
            let style = styles
                .get(index)
                .map(|block| block.content.clone())
                .unwrap_or_default();
            return Ok(Some(LoadResult {
                code: style,
                module_type: ModuleType::Css,
                dependencies: vec![path.to_string()],
            }));
        }
        let (blocks, _) = split_svelte(&source)?;
        let style_count = blocks.iter().filter(|block| block.kind == "style").count();
        let script_lang = blocks
            .iter()
            .find(|block| block.kind == "script")
            .map(|block| block_lang(&block.attrs, "js"))
            .unwrap_or_else(|| "js".to_string());
        let has_markup = !extract_markup(&source).is_empty();
        Ok(Some(LoadResult {
            code: main_module_code(path, has_markup, style_count, &script_lang),
            module_type: ModuleType::Js,
            dependencies: vec![path.to_string()],
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SFC: &str = "<script lang=\"ts\">\nlet count = 0;\n</script>\n\
        <button>{count}</button>\n\
        <style>\nbutton { color: blue; }\n</style>\n";

    #[test]
    fn markup_excludes_script_and_style() {
        let markup = extract_markup(SFC);
        assert!(markup.contains("<button>{count}</button>"), "{markup}");
        assert!(!markup.contains("let count"), "{markup}");
        assert!(!markup.contains("color: blue"), "{markup}");
    }

    #[test]
    fn main_module_wires_blocks() {
        let code = main_module_code("/src/App.svelte", true, 1, "ts");
        assert!(code.contains("?svelte&type=script&lang=ts"), "{code}");
        assert!(code.contains("?svelte&type=markup"), "{code}");
        assert!(code.contains("?svelte&type=style&index=0"), "{code}");
        assert!(code.contains("import.meta.hot.accept()"), "{code}");
    }
}
