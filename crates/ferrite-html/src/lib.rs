//! HTML transformation pipeline (spec §31).
//!
//! Order: pre hooks → core rewrites → normal hooks → post hooks → serialize.
//! Hook ordering is handled by [`ferrite_plugin::PluginContainer`]; this
//! crate owns entry discovery, core rewrites, and tag injection.

use std::collections::{BTreeMap, HashMap};

use ferrite_plugin::{HtmlInjectTo, HtmlTag};

/// Script entries discovered in HTML.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlEntry {
    /// Script `src` (as written).
    pub src: String,
    /// True for `<script type="module">`.
    pub is_module: bool,
}

/// Discover `<script src>` entries (module scripts first).
#[must_use]
pub fn discover_entries(html: &str) -> Vec<HtmlEntry> {
    let mut entries = Vec::new();
    for tag in find_tags(html, "script") {
        if let Some(src) = tag.attrs.get("src") {
            let is_module = tag.attrs.get("type").is_some_and(|kind| kind == "module");
            entries.push(HtmlEntry {
                src: src.clone(),
                is_module,
            });
        }
    }
    entries.sort_by_key(|entry| !entry.is_module);
    entries
}

/// Core HTML rewrites: base prefix, dev client injection, import-map support.
#[must_use]
pub fn apply_core_rewrites(html: &str, base: &str, inject_client: bool) -> String {
    let mut output = html.to_string();
    if base != "/" {
        output = rewrite_asset_prefix(&output, base);
    }
    if inject_client {
        let client = "<script type=\"module\" src=\"/@ferrite/client\"></script>";
        if !output.contains("/@ferrite/client") {
            output = inject_before(&output, "</head>", client);
        }
    }
    output
}

/// Inject plugin tags at their injection points.
#[must_use]
pub fn inject_tags(html: &str, tags: &[HtmlTag]) -> String {
    let mut output = html.to_string();
    for to in [
        HtmlInjectTo::HeadPrepend,
        HtmlInjectTo::Head,
        HtmlInjectTo::BodyPrepend,
        HtmlInjectTo::Body,
    ] {
        let group: Vec<String> = tags
            .iter()
            .filter(|tag| tag.inject_to == to)
            .map(render_tag)
            .collect();
        if group.is_empty() {
            continue;
        }
        let chunk = group.join("\n");
        output = match to {
            HtmlInjectTo::Head => inject_before(&output, "</head>", &chunk),
            HtmlInjectTo::HeadPrepend => inject_after(&output, "<head>", &chunk),
            HtmlInjectTo::Body => inject_before(&output, "</body>", &chunk),
            HtmlInjectTo::BodyPrepend => inject_after(&output, "<body>", &chunk),
        };
    }
    output
}

/// Render a tag to HTML.
#[must_use]
pub fn render_tag(tag: &HtmlTag) -> String {
    let mut attrs = String::new();
    let mut keys: Vec<&String> = tag.attrs.keys().collect();
    keys.sort();
    for key in keys {
        attrs.push_str(&format!(" {}=\"{}\"", key, escape_attr(&tag.attrs[key])));
    }
    match &tag.children {
        Some(children) => format!("<{}{}>{}</{}>", tag.tag, attrs, children, tag.tag),
        None if is_void(&tag.tag) => format!("<{tag}{attrs}>", tag = tag.tag),
        None => format!("<{tag}{attrs}></{tag}>", tag = tag.tag),
    }
}

/// Rewrite `<script type="module">` sources via `rewrite`.
/// Build an inline import-map script tag (dev `import-map` strategy).
///
/// Keys are emitted in sorted order for deterministic output. Values are
/// dev URLs (absolute paths or `/@npm/...`).
#[must_use]
pub fn import_map_script(imports: &BTreeMap<String, String>) -> String {
    let mut map = String::from("{\"imports\":{");
    for (index, (specifier, url)) in imports.iter().enumerate() {
        if index > 0 {
            map.push(',');
        }
        map.push_str(&serde_json::to_string(specifier).unwrap_or_default());
        map.push(':');
        map.push_str(&serde_json::to_string(url).unwrap_or_default());
    }
    map.push_str("}}");
    format!("<script type=\"importmap\">\n{map}\n</script>")
}

/// Inject an import-map script before the first module script.
///
/// The spec requires the map to precede any module load; when the page
/// has no module script the map goes right after `<head>`, else at the
/// start of the document.
#[must_use]
pub fn inject_import_map(html: &str, script: &str) -> String {
    let lower = html.to_lowercase();
    let mut cursor = 0;
    while let Some(start) = lower[cursor..].find("<script") {
        let tag_start = cursor + start;
        let Some(end) = lower[tag_start..].find('>') else {
            break;
        };
        let tag = &html[tag_start..tag_start + end];
        if tag.contains("type=\"module\"") || tag.contains("type='module'") {
            return format!("{}{script}\n{}", &html[..tag_start], &html[tag_start..]);
        }
        cursor = tag_start + end + 1;
    }
    if let Some(head_end) = find_head_end(html) {
        return format!("{}{script}\n{}", &html[..head_end], &html[head_end..]);
    }
    format!("{script}\n{html}")
}

/// Byte offset just past the opening `<head...>` tag, if any.
fn find_head_end(html: &str) -> Option<usize> {
    let lower = html.to_lowercase();
    let start = lower.find("<head")?;
    let end = lower[start..].find('>')?;
    Some(start + end + 1)
}

pub fn rewrite_module_scripts(html: &str, rewrite: impl Fn(&str) -> String) -> String {
    let mut output = html.to_string();
    // Collect spans first (match positions), then edit from the end.
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let lower = output.to_lowercase();
    let mut cursor = 0;
    while let Some(start) = lower[cursor..].find("<script") {
        let tag_start = cursor + start;
        let Some(tag_end) = lower[tag_start..].find('>') else {
            break;
        };
        let tag_end = tag_start + tag_end;
        let tag = &output[tag_start..=tag_end];
        let is_module = tag.contains("type=\"module\"") || tag.contains("type='module'");
        if is_module {
            if let Some((value_start, value_end, value)) = attr_span(tag, "src") {
                let replacement = rewrite(&value);
                edits.push((tag_start + value_start, tag_start + value_end, replacement));
            }
        }
        cursor = tag_end + 1;
    }
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.0));
    for (start, end, replacement) in edits {
        output.replace_range(start..end, &replacement);
    }
    output
}

// --- internals ---------------------------------------------------------------

struct FoundTag {
    attrs: HashMap<String, String>,
}

fn find_tags(html: &str, name: &str) -> Vec<FoundTag> {
    let lower = html.to_lowercase();
    let open = format!("<{name}");
    let mut tags = Vec::new();
    let mut cursor = 0;
    while let Some(start) = lower[cursor..].find(&open) {
        let tag_start = cursor + start;
        let Some(end) = lower[tag_start..].find('>') else {
            break;
        };
        let tag_end = tag_start + end;
        let tag = &html[tag_start..=tag_end];
        tags.push(FoundTag {
            attrs: parse_attrs(tag),
        });
        cursor = tag_end + 1;
    }
    tags
}

fn parse_attrs(tag: &str) -> HashMap<String, String> {
    let mut attrs = HashMap::new();
    let bytes = tag.as_bytes();
    let mut i = 0;
    // Skip `<name`.
    while i < bytes.len() && bytes[i] != b' ' && bytes[i] != b'>' {
        i += 1;
    }
    while i < bytes.len() {
        while i < bytes.len()
            && (bytes[i] == b' '
                || bytes[i] == b'\t'
                || bytes[i] == b'\n'
                || bytes[i] == b'\r'
                || bytes[i] == b'/')
        {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] == b'>' {
            break;
        }
        let key_start = i;
        while i < bytes.len() && bytes[i] != b'=' && bytes[i] != b' ' && bytes[i] != b'>' {
            i += 1;
        }
        let key = tag[key_start..i].to_ascii_lowercase();
        while i < bytes.len() && bytes[i] == b' ' {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b'=' {
            i += 1;
            while i < bytes.len() && bytes[i] == b' ' {
                i += 1;
            }
            if i < bytes.len() && (bytes[i] == b'"' || bytes[i] == b'\'') {
                let quote = bytes[i];
                i += 1;
                let value_start = i;
                while i < bytes.len() && bytes[i] != quote {
                    i += 1;
                }
                attrs.insert(key, tag[value_start..i].to_string());
                i += 1;
            } else {
                let value_start = i;
                while i < bytes.len() && bytes[i] != b' ' && bytes[i] != b'>' {
                    i += 1;
                }
                attrs.insert(key, tag[value_start..i].to_string());
            }
        } else if !key.is_empty() {
            attrs.insert(key, String::new());
        }
    }
    attrs
}

/// Byte span (relative to `tag`) of an attribute *value* (without quotes).
fn attr_span(tag: &str, wanted: &str) -> Option<(usize, usize, String)> {
    let lower = tag.to_ascii_lowercase();
    let mut cursor = 0;
    while let Some(found) = lower[cursor..].find(wanted) {
        let key_start = cursor + found;
        let before = key_start.checked_sub(1).map(|i| tag.as_bytes()[i]);
        if !matches!(before, Some(b' ') | Some(b'\t') | Some(b'\n') | None) {
            cursor = key_start + wanted.len();
            continue;
        }
        let mut i = key_start + wanted.len();
        while tag.as_bytes().get(i) == Some(&b' ') {
            i += 1;
        }
        if tag.as_bytes().get(i) != Some(&b'=') {
            cursor = i;
            continue;
        }
        i += 1;
        while tag.as_bytes().get(i) == Some(&b' ') {
            i += 1;
        }
        let quote = tag.as_bytes().get(i).copied();
        if matches!(quote, Some(b'"') | Some(b'\'')) {
            let value_start = i + 1;
            let rest = &tag[value_start..];
            let end = rest.find(quote.unwrap() as char)?;
            return Some((value_start, value_start + end, rest[..end].to_string()));
        }
        let value_start = i;
        let mut end = i;
        while let Some(byte) = tag.as_bytes().get(end) {
            if *byte == b' ' || *byte == b'>' {
                break;
            }
            end += 1;
        }
        return Some((value_start, end, tag[value_start..end].to_string()));
    }
    None
}

fn inject_before(html: &str, marker: &str, chunk: &str) -> String {
    match html.find(marker) {
        Some(pos) => format!("{}{}\n{}", &html[..pos], chunk, &html[pos..]),
        None => format!("{html}\n{chunk}"),
    }
}

fn inject_after(html: &str, marker: &str, chunk: &str) -> String {
    match html.find(marker) {
        Some(pos) => {
            let end = pos + marker.len();
            format!("{}{}\n{}", &html[..end], chunk, &html[end..])
        }
        None => format!("{chunk}\n{html}"),
    }
}

fn rewrite_asset_prefix(html: &str, base: &str) -> String {
    let base = base.trim_end_matches('/');
    let mut output = html.to_string();
    for attr in ["src=\"/", "href=\"/"] {
        output = output.replace(attr, &format!("{}{}/", &attr[..attr.len() - 1], base));
    }
    output
}

fn escape_attr(value: &str) -> String {
    value.replace('&', "&amp;").replace('"', "&quot;")
}

fn is_void(tag: &str) -> bool {
    matches!(
        tag,
        "link" | "meta" | "base" | "br" | "hr" | "img" | "input" | "source"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const HTML: &str = "<!doctype html><html><head><title>t</title></head><body><script type=\"module\" src=\"/src/main.ts\"></script></body></html>";

    #[test]
    fn discovers_module_entries() {
        let entries = discover_entries(HTML);
        assert_eq!(entries.len(), 1);
        assert!(entries[0].is_module);
        assert_eq!(entries[0].src, "/src/main.ts");
    }

    #[test]
    fn base_rewrite_preserves_attribute_syntax() {
        let html =
            "<link href=\"/favicon.svg\"><script type=\"module\" src=\"/src/main.js\"></script>";
        let output = apply_core_rewrites(html, "/app/", false);
        assert_eq!(output, "<link href=\"/app/favicon.svg\"><script type=\"module\" src=\"/app/src/main.js\"></script>");
    }

    #[test]
    fn injects_client() {
        let output = apply_core_rewrites(HTML, "/", true);
        assert!(output.contains("/@ferrite/client"));
    }

    #[test]
    fn rewrites_module_src() {
        let output = rewrite_module_scripts(HTML, |src| format!("/assets{src}"));
        assert!(output.contains("/assets/src/main.ts"));
    }

    #[test]
    fn injects_tags() {
        let tags = vec![HtmlTag {
            tag: "meta".to_string(),
            attrs: HashMap::from([("name".to_string(), "x".to_string())]),
            children: None,
            inject_to: HtmlInjectTo::Head,
        }];
        let output = inject_tags(HTML, &tags);
        assert!(output.contains("<meta name=\"x\">"));
    }

    #[test]
    fn import_map_script_sorts_and_escapes() {
        let imports = BTreeMap::from([
            ("z-lib".to_string(), "/@npm/z-lib/index.js".to_string()),
            ("a\"b".to_string(), "/x.js".to_string()),
        ]);
        let script = import_map_script(&imports);
        assert!(
            script.starts_with("<script type=\"importmap\">"),
            "{script}"
        );
        assert!(script.ends_with("</script>"), "{script}");
        let a_pos = script.find("a\\\"b").expect("escaped key");
        let z_pos = script.find("z-lib").expect("z key");
        assert!(a_pos < z_pos, "{script}");
        let json_text = script
            .strip_prefix("<script type=\"importmap\">\n")
            .and_then(|rest| rest.strip_suffix("\n</script>"))
            .expect("script wrapper");
        let json: serde_json::Value = serde_json::from_str(json_text).expect("valid json");
        assert_eq!(json["imports"]["z-lib"], "/@npm/z-lib/index.js");
    }

    #[test]
    fn inject_import_map_precedes_first_module_script() {
        let html = "<html><head><script src=\"/classic.js\"></script></head>\
            <body><script type=\"module\" src=\"/a.js\"></script></body></html>";
        let output = inject_import_map(html, "<script type=\"importmap\"></script>");
        assert!(
            output.starts_with("<html><head><script src=\"/classic.js\"></script></head>"),
            "{output}"
        );
        let map_pos = output.find("importmap").expect("map");
        let mod_pos = output.find("type=\"module\"").expect("module");
        assert!(map_pos < mod_pos, "{output}");
    }

    #[test]
    fn inject_import_map_falls_back_to_head_then_start() {
        let html = "<html><head><title>t</title></head><body>hi</body></html>";
        let output = inject_import_map(html, "<!--map-->");
        assert!(output.contains("<head><!--map-->\n<title>"), "{output}");
        let bare = "<p>no head</p>";
        let output = inject_import_map(bare, "<!--map-->");
        assert!(output.starts_with("<!--map-->\n<p>"), "{output}");
    }
}
