//! CSS pipeline (spec §29).
//!
//! Covers `.css` imports, CSS modules, `@import`/`url()` rewriting, dev-time
//! style injection, extraction, and minification.

use std::collections::HashMap;

use ferrite_core::Hash;

/// A `@import` statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CssImport {
    /// Raw specifier.
    pub specifier: String,
    /// Byte range of the full `@import ...;` statement.
    pub range: (usize, usize),
}

/// A `url()` reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CssUrl {
    /// Raw URL.
    pub url: String,
    /// Byte range of the URL text (without `url()` wrapper/quotes).
    pub range: (usize, usize),
}

/// CSS transform options.
#[derive(Debug, Clone)]
pub struct CssOptions {
    /// Enable CSS modules scoping.
    pub modules: bool,
    /// Minify output.
    pub minify: bool,
    /// Dev mode (keeps readable output for the overlay).
    pub dev: bool,
}

impl Default for CssOptions {
    fn default() -> Self {
        Self {
            modules: false,
            minify: false,
            dev: true,
        }
    }
}

/// CSS transform result.
#[derive(Debug, Clone)]
pub struct CssTransformResult {
    /// Transformed CSS.
    pub code: String,
    /// `@import` dependencies.
    pub imports: Vec<CssImport>,
    /// `url()` dependencies.
    pub urls: Vec<CssUrl>,
    /// CSS modules export map (`class` → `scoped`).
    pub exports: HashMap<String, String>,
}

/// Transform CSS: extract deps, optionally scope modules + minify.
#[must_use]
pub fn transform_css(id: &str, source: &str, options: &CssOptions) -> CssTransformResult {
    let imports = extract_imports(source);
    let urls = extract_urls(source);
    let is_module = options.modules || id.contains(".module.css");
    let (mut code, exports) = if is_module {
        scope_modules(source, &Hash::of_str(id).short(8))
    } else {
        (source.to_string(), HashMap::new())
    };
    if options.minify {
        code = minify_css(&code);
    }
    CssTransformResult {
        code,
        imports,
        urls,
        exports,
    }
}

/// Extract `@import` statements.
#[must_use]
pub fn extract_imports(source: &str) -> Vec<CssImport> {
    let pattern =
        regex::Regex::new(r#"@import\s+(?:url\(\s*)?["']([^"']+)["']\s*\)?[^;]*;"#).unwrap();
    pattern
        .captures_iter(source)
        .filter_map(|captures| {
            let full = captures.get(0)?;
            let specifier = captures.get(1)?.as_str().to_string();
            Some(CssImport {
                specifier,
                range: (full.start(), full.end()),
            })
        })
        .collect()
}

/// Extract `url()` references (excluding `data:` and `#fragment`).
#[must_use]
pub fn extract_urls(source: &str) -> Vec<CssUrl> {
    let pattern = regex::Regex::new(r#"url\(\s*["']?([^"')]+)["']?\s*\)"#).unwrap();
    pattern
        .captures_iter(source)
        .filter_map(|captures| {
            let full = captures.get(1)?;
            let url = full.as_str().to_string();
            if url.starts_with("data:") || url.starts_with('#') {
                return None;
            }
            Some(CssUrl {
                url,
                range: (full.start(), full.end()),
            })
        })
        .collect()
}

/// Rewrite `@import` specifiers via `rewrite` (leaves `url()` alone).
#[must_use]
pub fn rewrite_css_imports(source: &str, rewrite: impl Fn(&str) -> Option<String>) -> String {
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for import in extract_imports(source) {
        if let Some(replacement) = rewrite(&import.specifier) {
            let (start, end) = import.range;
            let statement = &source[start..end];
            if let Some(pos) = statement.find(&import.specifier) {
                edits.push((
                    start + pos,
                    start + pos + import.specifier.len(),
                    replacement,
                ));
            }
        }
    }
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.0));
    let mut output = source.to_string();
    for (start, end, replacement) in edits {
        output.replace_range(start..end, &replacement);
    }
    output
}

/// Rewrite `url()` references via `rewrite` (leaves `@import` alone).
#[must_use]
pub fn rewrite_css_urls(source: &str, rewrite: impl Fn(&str) -> Option<String>) -> String {
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for url in extract_urls(source) {
        if let Some(replacement) = rewrite(&url.url) {
            edits.push((url.range.0, url.range.1, replacement));
        }
    }
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.0));
    let mut output = source.to_string();
    for (start, end, replacement) in edits {
        output.replace_range(start..end, &replacement);
    }
    output
}

/// Rewrite `@import` specifiers and `url()` references via `rewrite`.
#[must_use]
pub fn rewrite_css_refs(source: &str, rewrite: impl Fn(&str) -> Option<String>) -> String {
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for import in extract_imports(source) {
        if let Some(replacement) = rewrite(&import.specifier) {
            let (start, end) = import.range;
            let statement = &source[start..end];
            if let Some(pos) = statement.find(&import.specifier) {
                edits.push((
                    start + pos,
                    start + pos + import.specifier.len(),
                    replacement,
                ));
            }
        }
    }
    for url in extract_urls(source) {
        if let Some(replacement) = rewrite(&url.url) {
            edits.push((url.range.0, url.range.1, replacement));
        }
    }
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.0));
    let mut output = source.to_string();
    for (start, end, replacement) in edits {
        output.replace_range(start..end, &replacement);
    }
    output
}

/// Scope class/ID selectors for CSS modules: `.btn` → `.btn_<hash>`.
#[must_use]
pub fn scope_modules(source: &str, hash: &str) -> (String, HashMap<String, String>) {
    let class_pattern = regex::Regex::new(r"\.(-?[A-Za-z_][A-Za-z0-9_-]*)").unwrap();
    let mut exports = HashMap::new();
    // Collect candidate class names (skip decimal-looking `.5` etc. via the
    // leading-char class above).
    let mut names: Vec<String> = class_pattern
        .captures_iter(source)
        .map(|captures| captures[1].to_string())
        .collect();
    names.sort();
    names.dedup();
    for name in &names {
        exports.insert(name.clone(), format!("{name}_{hash}"));
    }
    let output = class_pattern
        .replace_all(source, |captures: &regex::Captures| {
            format!(".{}", exports[&captures[1]])
        })
        .into_owned();
    (output, exports)
}

/// Minify CSS: strip comments, collapse whitespace.
#[must_use]
pub fn minify_css(source: &str) -> String {
    let comments = regex::Regex::new(r"/\*[^*]*\*+(?:[^/*][^*]*\*+)*/").unwrap();
    let stripped = comments.replace_all(source, "");
    let collapsed = regex::Regex::new(r"\s+")
        .unwrap()
        .replace_all(&stripped, " ");
    let mut output = collapsed.into_owned();
    for (pattern, replacement) in [
        (" {", "{"),
        ("{ ", "{"),
        (" }", "}"),
        ("} ", "}"),
        ("; ", ";"),
        (" :", ":"),
        (": ", ":"),
        (", ", ","),
    ] {
        output = output.replace(pattern, replacement);
    }
    output.trim().to_string()
}

/// Render dev-time JS that injects `css` and exports the modules map (§29).
#[must_use]
pub fn css_to_js(module_id: &str, css: &str, exports: &HashMap<String, String>) -> String {
    let exports_json = serde_json::to_string(exports).unwrap_or_else(|_| "{}".to_string());
    format!(
        "import {{ updateStyle }} from \"/@ferrite/client\";\n\
         const __css__ = {css:?};\n\
         updateStyle({module_id:?}, __css__);\n\
         if (globalThis.__ferrite_create_hot__({module_id:?})) {{}}\n\
         export default {exports_json};\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_imports_and_urls() {
        let source = "@import \"./base.css\";\n.a { background: url(./img.png); }\n";
        assert_eq!(extract_imports(source).len(), 1);
        assert_eq!(extract_urls(source).len(), 1);
    }

    #[test]
    fn scopes_module_classes() {
        let (code, exports) = scope_modules(".btn { color: red; }", "abc123");
        assert!(code.contains(".btn_abc123"));
        assert_eq!(exports.get("btn").unwrap(), "btn_abc123");
    }

    #[test]
    fn minifies() {
        let min = minify_css("/* c */\n.a  {\n  color:  red;\n}\n");
        assert_eq!(min, ".a{color:red;}");
    }

    #[test]
    fn rewrites_refs() {
        let source = "@import \"./base.css\";";
        let output = rewrite_css_refs(source, |spec| {
            (spec == "./base.css").then(|| "/src/base.css".to_string())
        });
        assert!(output.contains("/src/base.css"));
    }
}
