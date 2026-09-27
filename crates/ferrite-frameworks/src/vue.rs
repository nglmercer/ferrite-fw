//! Vue SFC experiment (`ferrite:vue`, §70 roadmap).
//!
//! Splits `.vue` files into virtual modules: `?vue&type=script` (through
//! the core transform), `?vue&type=template` (experimental render stub —
//! returns the markup string; real template compilation is roadmap), and
//! `?vue&type=style&index=N` (through the CSS pipeline). The main module
//! wires them together with HMR acceptance.

use ferrite_core::{ModuleType, Result};
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

/// Experimental template stub: render returns the markup string.
#[must_use]
pub fn template_stub(template: &str) -> String {
    format!(
        "// Experimental Vue template stub: real compilation is roadmap.\n\
         export function render() {{\n\
         return {};\n\
         }}\n",
        serde_json::to_string(template).unwrap_or_else(|_| "\"\"".to_string())
    )
}

/// Vue SFC plugin (`ferrite:vue`).
#[derive(Debug)]
pub struct VuePlugin {
    /// Project root for SFC reads.
    root: std::path::PathBuf,
}

impl VuePlugin {
    /// Create the plugin for `root`.
    #[must_use]
    pub fn new(root: std::path::PathBuf) -> Self {
        Self { root }
    }

    /// Read the SFC file behind `id` (strips any `?vue` query).
    fn read_sfc(&self, id: &str) -> Result<String> {
        let path = id.split('?').next().unwrap_or(id);
        let fs_path = self.root.join(path.trim_start_matches('/'));
        std::fs::read_to_string(&fs_path).map_err(|error| {
            ferrite_core::FerriteError::Build(format!("cannot read {id}: {error}"))
        })
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
        if request.specifier.contains(".vue") {
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
        if !path.ends_with(".vue") {
            return Ok(None);
        }
        let source = self.read_sfc(&request.id)?;
        let blocks = split_sfc(&source)?;
        let query = query.unwrap_or("");
        if query.contains("type=script") {
            let script = blocks
                .iter()
                .find(|block| block.kind == "script")
                .map(|block| (block.content.clone(), block_lang(&block.attrs, "js")))
                .unwrap_or_else(|| ("export default {};\n".to_string(), "js".to_string()));
            let module_type = match script.1.as_str() {
                "ts" => ModuleType::Ts,
                "tsx" => ModuleType::Tsx,
                "jsx" => ModuleType::Jsx,
                _ => ModuleType::Js,
            };
            return Ok(Some(LoadResult {
                code: script.0,
                module_type,
                dependencies: vec![path.to_string()],
            }));
        }
        if query.contains("type=template") {
            let template = blocks
                .iter()
                .find(|block| block.kind == "template")
                .map(|block| block.content.clone())
                .unwrap_or_default();
            return Ok(Some(LoadResult {
                code: template_stub(&template),
                module_type: ModuleType::Js,
                dependencies: vec![path.to_string()],
            }));
        }
        if query.contains("type=style") {
            let styles: Vec<&SfcBlock> = blocks
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
        // Main module.
        let has_template = blocks.iter().any(|block| block.kind == "template");
        let style_count = blocks.iter().filter(|block| block.kind == "style").count();
        let script_lang = blocks
            .iter()
            .find(|block| block.kind == "script")
            .map(|block| block_lang(&block.attrs, "js"))
            .unwrap_or_else(|| "js".to_string());
        Ok(Some(LoadResult {
            code: main_module_code(path, has_template, style_count, &script_lang),
            module_type: ModuleType::Js,
            dependencies: vec![path.to_string()],
        }))
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
    fn template_stub_returns_markup() {
        let stub = template_stub("<button>x</button>");
        assert!(stub.contains("export function render"), "{stub}");
        assert!(stub.contains("<button>x</button>"), "{stub}");
    }

    #[test]
    fn block_lang_defaults_and_quotes() {
        assert_eq!(block_lang("", "js"), "js");
        assert_eq!(block_lang("lang=\"scss\"", "css"), "scss");
        assert_eq!(block_lang("lang='ts'", "js"), "ts");
        assert_eq!(block_lang("scoped lang = \"css\"", "x"), "css");
    }
}
