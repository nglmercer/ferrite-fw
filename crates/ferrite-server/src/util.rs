//! Dev server helpers.

use ferrite_core::ModuleId;
use ferrite_core::ModuleType;
use ferrite_transform::JsCompiler;
use ferrite_transform::OxcCompiler;
use ferrite_transform::OxcOptions;
use std::collections::HashMap;
use std::sync::Arc;

/// True when the module must be served as raw bytes.
pub(crate) fn is_raw_asset(path: &str, module_type: &ModuleType, query: Option<&str>) -> bool {
    if query.is_some() {
        return false; // queries are handled as shims/transforms
    }
    matches!(
        module_type,
        ModuleType::Asset | ModuleType::Wasm | ModuleType::Data
    ) || is_binary_extension(path)
}

/// True for binary extensions served verbatim.
pub(crate) fn is_binary_extension(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    [
        ".png", ".jpg", ".jpeg", ".gif", ".webp", ".avif", ".ico", ".bmp", ".svg", ".woff",
        ".woff2", ".ttf", ".otf", ".eot", ".mp3", ".wav", ".ogg", ".mp4", ".webm", ".pdf", ".zip",
        ".wasm",
    ]
    .iter()
    .any(|ext| lower.ends_with(ext))
}

/// Heuristic binary detection (NUL byte in the first 8 KiB).
pub(crate) fn is_binary_asset(path: &str, bytes: &[u8]) -> bool {
    if is_binary_extension(path) {
        return true;
    }
    bytes.iter().take(8192).any(|byte| *byte == 0)
}

/// Map an internal `\0` virtual id to its servable `/@id/` URL (Vite-style).
#[must_use]
pub fn virtual_url(id: &str) -> String {
    format!("/@id/{}", id.trim_start_matches('\0'))
}

/// Map a `/@id/` URL back to its internal virtual id.
#[must_use]
pub fn url_to_virtual(url: &str) -> Option<ModuleId> {
    url.strip_prefix("/@id/")
        .map(|rest| ModuleId::new(format!("\0{rest}")))
}

/// Resolve a possibly-virtual module id (§14): `/@id/` URLs map back to
/// their internal `\0` ids, everything else borrows as-is.
pub(crate) fn unvirtualize(id: &ModuleId) -> std::borrow::Cow<'_, ModuleId> {
    url_to_virtual(&id.0).map_or(std::borrow::Cow::Borrowed(id), std::borrow::Cow::Owned)
}

/// True when an imported URL should go through the `?asset-shim` path.
pub(crate) fn should_shim_asset(url: &str) -> bool {
    let path = url.split('?').next().unwrap_or(url);
    matches!(ModuleType::from_path(path), ModuleType::Asset) || is_binary_extension(path)
}

/// True when CJS conversion applies (§18).
pub(crate) fn needs_cjs_conversion(id: &ModuleId, code: &str, has_module_syntax: bool) -> bool {
    if has_module_syntax {
        return false;
    }
    let (path, _) = id.split_query();
    if path.ends_with(".cjs") || path.ends_with(".cts") {
        return true;
    }
    ferrite_transform::analyze_commonjs(&id.0, code).is_ok_and(|analysis| analysis.is_commonjs)
}

/// Self-contained production CSS injection (no dev client import).
pub(crate) fn production_css_js(
    module_id: &str,
    css: &str,
    exports: &HashMap<String, String>,
) -> String {
    let exports_json = serde_json::to_string(exports).unwrap_or_else(|_| "{}".to_string());
    format!(
        "const __css__ = {css:?};\n\
         if (typeof document !== \"undefined\") {{\n\
         const __el__ = document.createElement(\"style\");\n\
         __el__.setAttribute(\"data-ferrite-id\", {module_id:?});\n\
         __el__.textContent = __css__;\n\
         document.head.appendChild(__el__);\n\
         }}\n\
         export default {exports_json};\n"
    )
}

/// Minimal `node:` shim (§19 browser-shims mode).
pub(crate) fn node_shim(name: &str) -> String {
    match name {
        "process" => "export const env = {};\nexport const argv = [];\nexport default { env, argv };\n".to_string(),
        "buffer" => "export const Buffer = globalThis.Buffer;\nexport default globalThis.Buffer;\n".to_string(),
        "events" => "export class EventEmitter { on(){} off(){} emit(){} }\nexport default EventEmitter;\n".to_string(),
        "util" => "export const format = (...a) => a.join(\" \");\nexport const inspect = (v) => String(v);\nexport default { format, inspect };\n".to_string(),
        "path" => "export const sep = \"/\";\nexport const join = (...p) => p.join(\"/\");\nexport const dirname = (p) => p.split(\"/\").slice(0, -1).join(\"/\");\nexport const basename = (p) => p.split(\"/\").pop();\nexport default { sep, join, dirname, basename };\n".to_string(),
        "url" => "export const URL = globalThis.URL;\nexport const URLSearchParams = globalThis.URLSearchParams;\nexport default { URL, URLSearchParams };\n".to_string(),
        _ => format!("// ferrite node:{name} shim (browser-shims)\nexport default {{}};\n"),
    }
}

/// Maximum fetched remote module size (§72).
pub(crate) const MAX_REMOTE_BYTES: usize = 8 * 1024 * 1024;

/// True when `host` matches the `[remote] allow` list (exact or `*.` suffix).
pub(crate) fn remote_host_allowed(host: &str, allow: &[String]) -> bool {
    let host = host.strip_suffix('.').unwrap_or(host).to_ascii_lowercase();
    allow.iter().any(|entry| {
        let entry = entry
            .strip_suffix('.')
            .unwrap_or(entry)
            .to_ascii_lowercase();
        entry == host
            || entry
                .strip_prefix("*.")
                .is_some_and(|suffix| !suffix.is_empty() && host.ends_with(&format!(".{suffix}")))
    })
}

/// True for bare npm-style specifiers (resolver step 11): anything that is
/// not virtual, builtin, remote, absolute, relative, or a `#` import.
pub(crate) fn is_bare_specifier(specifier: &str) -> bool {
    !(specifier.starts_with('\0')
        || specifier.starts_with("node:")
        || specifier.starts_with("rust:")
        || specifier.starts_with("https://")
        || specifier.starts_with("http://")
        || specifier.starts_with("data:")
        || specifier.starts_with("blob:")
        || specifier.starts_with('/')
        || specifier.starts_with("./")
        || specifier.starts_with("../")
        || specifier == "."
        || specifier == ".."
        || specifier.starts_with('#'))
}

/// Rewrite `<link rel="stylesheet" href>` to `?direct` URLs.
pub(crate) fn rewrite_link_direct(html: &str) -> String {
    let pattern = regex::Regex::new(r#"<link([^>]*?)href="([^"]+)"([^>]*?)>"#).unwrap();
    pattern
        .replace_all(html, |captures: &regex::Captures| {
            let (pre, href, post) = (&captures[1], &captures[2], &captures[3]);
            let is_stylesheet = pre.contains("stylesheet") || post.contains("stylesheet");
            if is_stylesheet && !href.contains('?') && !href.starts_with("http") {
                format!("<link{pre}href=\"{href}?direct\"{post}>")
            } else {
                captures[0].to_string()
            }
        })
        .into_owned()
}

pub(crate) fn now_millis() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

/// Default compiler: Oxc (spec §5).
#[must_use]
/// Best-effort LAN IP for Network URLs: the source address the kernel
/// would use toward the public internet (no packets are sent).
pub(crate) fn lan_ip() -> Option<std::net::IpAddr> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    // `connect` on UDP only selects a route; nothing is transmitted.
    socket.connect("8.8.8.8:80").ok()?;
    let ip = socket.local_addr().ok()?.ip();
    if ip.is_loopback() || ip.is_unspecified() {
        return None;
    }
    Some(ip)
}

pub fn default_compiler() -> Arc<dyn JsCompiler> {
    Arc::new(OxcCompiler::new(OxcOptions::default()))
}

/// Numeric `t` is reserved for the dev client's import cache busting.
/// Resource query flags and their order remain part of module identity.
pub(crate) fn strip_hmr_timestamp(id: &ferrite_core::ModuleId) -> ferrite_core::ModuleId {
    let (path, Some(query)) = id.split_query() else {
        return id.clone();
    };
    let retained: Vec<_> = query
        .split('&')
        .filter(|part| {
            !part.strip_prefix("t=").is_some_and(|value| {
                !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
            })
        })
        .collect();
    if retained.len() == query.split('&').count() {
        return id.clone();
    }
    ferrite_core::ModuleId::new(if retained.is_empty() {
        path.to_string()
    } else {
        format!("{path}?{}", retained.join("&"))
    })
}

#[cfg(test)]
mod timestamp_tests {
    #[test]
    fn transport_timestamp_preserves_resource_query_identity() {
        for (input, expected) in [
            ("/dep.js?t=123", "/dep.js"),
            ("/dep.js?raw&t=123", "/dep.js?raw"),
            (
                "/@id/component?ferrite-style=0&t=456&scoped",
                "/@id/component?ferrite-style=0&scoped",
            ),
            ("/dep.js?t=123&t=456", "/dep.js"),
            ("/dep.js?t=other", "/dep.js?t=other"),
            ("/dep.js?t=", "/dep.js?t="),
            ("/dep.js?target=123", "/dep.js?target=123"),
        ] {
            assert_eq!(
                super::strip_hmr_timestamp(&ferrite_core::ModuleId::new(input)).0,
                expected
            );
        }
    }
}
