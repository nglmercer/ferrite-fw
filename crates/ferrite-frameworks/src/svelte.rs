//! Svelte adapter: official compilation requires an explicitly enabled host.
//! Legacy block utilities are retained for API compatibility, not compilation.

use ferrite_core::Result;
use ferrite_plugin::{LoadRequest, LoadResult, Plugin, PluginContext, ResolveHookRequest};

use crate::vue::SfcBlock;

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

/// Extract the markup: source minus its script/style blocks.
#[must_use]
pub fn extract_markup(source: &str) -> String {
    split_svelte(source)
        .map(|(_, markup)| markup)
        .unwrap_or_default()
}

/// Svelte plugin (`ferrite:svelte`).
#[derive(Debug)]
pub struct SveltePlugin;

impl SveltePlugin {
    /// Explicitly enable the official compiler pipeline with a validated Node host.
    pub fn with_host(
        host: std::sync::Arc<crate::compiler_host::NodeCompilerHost>,
        development: bool,
    ) -> crate::HostedFrameworkPlugin {
        crate::HostedFrameworkPlugin::new(
            crate::compiler_host::Framework::Svelte,
            host,
            development,
        )
    }

    /// Create the plugin. Compilation remains unavailable until a validated host exists.
    #[must_use]
    pub fn new(_root: std::path::PathBuf) -> Self {
        Self
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
        if crate::registry::owns_component("svelte", request.specifier) {
            let id = if request.specifier.starts_with('.') {
                let importer = request.importer.map(|id| id.0.as_str()).unwrap_or("/");
                crate::join_relative(importer, request.specifier)
            } else {
                request.specifier.to_string()
            };
            let mut resolved = ferrite_resolver::ResolvedId::new(id);
            resolved.module_type = Some(ferrite_core::ModuleType::Custom("svelte".into()));
            return Ok(Some(resolved));
        }
        Ok(None)
    }

    async fn load(&self, _ctx: &PluginContext, request: LoadRequest) -> Result<Option<LoadResult>> {
        if !crate::registry::owns_component("svelte", &request.id) {
            return Ok(None);
        }
        Err(crate::registry::compiler_unavailable("svelte", &request.id))
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
