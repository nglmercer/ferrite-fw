//! Vue adapter: official compilation currently unavailable.
//! Legacy block utilities are retained for API compatibility, not compilation.

use ferrite_core::Result;
use ferrite_plugin::{LoadRequest, LoadResult, Plugin, PluginContext, ResolveHookRequest};

/// One SFC block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SfcBlock {
    /// `template`, `script`, or `style`.
    pub kind: String,
    /// Raw attributes (`setup lang="ts"`).
    pub attrs: String,
    /// Inner content.
    pub content: String,
}

/// Split SFC source into top-level blocks. Loud on malformed input.
pub fn split_sfc(source: &str) -> Result<Vec<SfcBlock>> {
    let mut blocks = Vec::new();
    let mut rest = source;
    // Track byte offset for error messages.
    let mut offset = 0;
    while let Some(open) = rest.find('<') {
        offset += open;
        rest = &rest[open..];
        // Skip comments and closing tags at top level.
        if rest.starts_with("<!--") {
            let Some(end) = rest.find("-->") else {
                return Err(ferrite_core::FerriteError::Build(
                    "malformed SFC: unclosed comment".to_string(),
                ));
            };
            offset += end + 3;
            rest = &rest[end + 3..];
            continue;
        }
        if rest.starts_with("</") {
            return Err(ferrite_core::FerriteError::Build(format!(
                "malformed SFC: stray closing tag at byte {offset}"
            )));
        }
        let Some(tag_end) = rest.find('>') else {
            return Err(ferrite_core::FerriteError::Build(
                "malformed SFC: unclosed tag".to_string(),
            ));
        };
        let tag = &rest[1..tag_end];
        // Self-closing top-level tags carry no block.
        if tag.ends_with('/') {
            offset += tag_end + 1;
            rest = &rest[tag_end + 1..];
            continue;
        }
        let (name, attrs) = match tag.find(char::is_whitespace) {
            Some(space) => (&tag[..space], tag[space..].trim().to_string()),
            None => (tag, String::new()),
        };
        // Only SFC blocks open at top level; anything else must be inside
        // a block (a previous block swallowed it) — treat unknown
        // top-level tags as malformed to stay loud.
        if name != "template" && name != "script" && name != "style" {
            return Err(ferrite_core::FerriteError::Build(format!(
                "malformed SFC: unexpected top-level <{name}> at byte {offset}"
            )));
        }
        let close = format!("</{name}>");
        let body_start = tag_end + 1;
        let Some(close_at) = rest[body_start..].find(close.as_str()) else {
            return Err(ferrite_core::FerriteError::Build(format!(
                "malformed SFC: unclosed <{name}> block"
            )));
        };
        // Reject nesting of the same block tag (ambiguous split).
        if rest[body_start..body_start + close_at].contains(&format!("<{name}")) {
            return Err(ferrite_core::FerriteError::Build(format!(
                "malformed SFC: nested <{name}> blocks"
            )));
        }
        blocks.push(SfcBlock {
            kind: name.to_string(),
            attrs,
            content: rest[body_start..body_start + close_at].to_string(),
        });
        let consumed = body_start + close_at + close.len();
        offset += consumed;
        rest = &rest[consumed..];
    }
    Ok(blocks)
}

/// Extract `lang="x"` from block attrs (default `default`).
#[must_use]
pub fn block_lang(attrs: &str, default: &str) -> String {
    let mut words = attrs.split_whitespace();
    while let Some(word) = words.next() {
        if let Some(lang) = word.strip_prefix("lang=") {
            return lang.trim_matches('"').trim_matches('\'').to_string();
        }
        // `lang = "ts"` (spaced).
        if word == "lang" && words.next() == Some("=") {
            if let Some(lang) = words.next() {
                return lang.trim_matches('"').trim_matches('\'').to_string();
            }
        }
    }
    default.to_string()
}

/// True for `<script setup>`.
#[must_use]
pub fn is_setup(attrs: &str) -> bool {
    attrs.split_whitespace().any(|word| word == "setup")
}

/// Main-module code for `id` given its block layout.
#[must_use]
pub fn main_module_code(
    id: &str,
    has_template: bool,
    style_count: usize,
    script_lang: &str,
) -> String {
    let script_spec = format!("{id}?vue&type=script&lang={script_lang}");
    let template_spec = format!("{id}?vue&type=template");
    let mut code = format!(
        "import __script from {script_spec:?};\n\
         import {{ render as __render }} from {template_spec:?};\n"
    );
    for index in 0..style_count {
        let style_spec = format!("{id}?vue&type=style&index={index}");
        code.push_str(&format!("import {style_spec:?};\n"));
    }
    if has_template {
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

/// Vue SFC plugin (`ferrite:vue`).
#[derive(Debug)]
pub struct VuePlugin;

impl VuePlugin {
    /// Create the plugin. Compilation remains unavailable until a validated host exists.
    #[must_use]
    pub fn new(_root: std::path::PathBuf) -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl Plugin for VuePlugin {
    fn name(&self) -> &'static str {
        "ferrite:vue"
    }

    async fn resolve_id(
        &self,
        _ctx: &PluginContext,
        request: ResolveHookRequest<'_>,
    ) -> Result<Option<ferrite_resolver::ResolvedId>> {
        // Claim `.vue` ids (plain + `?vue` virtuals), resolving
        // importer-relative specs lexically (plugin hooks run first).
        if crate::registry::owns_component("vue", request.specifier) {
            let id = if request.specifier.starts_with('.') {
                let importer = request.importer.map(|id| id.0.as_str()).unwrap_or("/");
                crate::join_relative(importer, request.specifier)
            } else {
                request.specifier.to_string()
            };
            let mut resolved = ferrite_resolver::ResolvedId::new(id);
            resolved.module_type = Some(ferrite_core::ModuleType::Custom("vue".into()));
            return Ok(Some(resolved));
        }
        Ok(None)
    }

    async fn load(&self, _ctx: &PluginContext, request: LoadRequest) -> Result<Option<LoadResult>> {
        if !crate::registry::owns_component("vue", &request.id) {
            return Ok(None);
        }
        Err(crate::registry::compiler_unavailable("vue", &request.id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SFC: &str = "<template>\n  <button>{{ msg }}</button>\n</template>\n\
        <script setup lang=\"ts\">\nimport { ref } from \"vue\";\nconst msg = ref(\"hi\");\n</script>\n\
        <style scoped>\nbutton { color: red; }\n</style>\n";

    #[test]
    fn splits_blocks_with_attrs() {
        let blocks = split_sfc(SFC).unwrap();
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].kind, "template");
        assert_eq!(blocks[1].kind, "script");
        assert!(is_setup(&blocks[1].attrs));
        assert_eq!(block_lang(&blocks[1].attrs, "js"), "ts");
        assert!(blocks[1].content.contains("ref(\"hi\")"));
        assert_eq!(blocks[2].kind, "style");
        assert!(blocks[2].content.contains("color: red"));
    }

    #[test]
    fn rejects_malformed() {
        assert!(split_sfc("<template><div>").is_err());
        assert!(split_sfc("<template><template></template></template>").is_err());
        assert!(split_sfc("</div>").is_err());
        assert!(split_sfc("<div>x</div>").is_err());
    }

    #[test]
    fn main_module_wires_blocks() {
        let code = main_module_code("/src/App.vue", true, 1, "ts");
        assert!(code.contains("?vue&type=script&lang=ts"), "{code}");
        assert!(code.contains("?vue&type=template"), "{code}");
        assert!(code.contains("?vue&type=style&index=0"), "{code}");
        assert!(code.contains("__script.render = __render"), "{code}");
        assert!(code.contains("import.meta.hot.accept()"), "{code}");
    }

    #[test]
    fn block_lang_defaults_and_quotes() {
        assert_eq!(block_lang("", "js"), "js");
        assert_eq!(block_lang("lang=\"scss\"", "css"), "scss");
        assert_eq!(block_lang("lang='ts'", "js"), "ts");
        assert_eq!(block_lang("scoped lang = \"css\"", "x"), "css");
    }
}
