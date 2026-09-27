//! Static asset pipeline (spec §30).
//!
//! Handles `?raw`, `?url`, `?inline`, `?worker`, `?wasm` queries, content
//! hashing (`logo.4ad83f.svg`), inlining limits, and JS shims for imports.

use ferrite_core::Hash;

/// Asset import query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AssetQuery {
    /// Plain import (hashed URL in build, served path in dev).
    #[default]
    Auto,
    /// `?raw`: inline file text as a string.
    Raw,
    /// `?url`: always emit a URL.
    Url,
    /// `?inline`: always inline as a data URL.
    Inline,
    /// `?worker`: build as a worker entry.
    Worker,
    /// `?wasm`: build as a WASM entry.
    Wasm,
}

impl AssetQuery {
    /// Parse the query suffix of a module id.
    #[must_use]
    pub fn from_id(id: &str) -> (String, Self) {
        match id.split_once('?') {
            Some((path, query)) => {
                let kind = match query.split('&').next().unwrap_or("") {
                    "raw" => Self::Raw,
                    "url" => Self::Url,
                    "inline" => Self::Inline,
                    "worker" => Self::Worker,
                    "wasm" => Self::Wasm,
                    _ => Self::Auto,
                };
                (path.to_string(), kind)
            }
            None => (id.to_string(), Self::Auto),
        }
    }
}

/// Default inline limit (4 KiB, Vite-compatible).
pub const DEFAULT_INLINE_LIMIT: usize = 4096;

/// Content-hash an asset: `logo.svg` → `logo.4ad83f.svg`.
#[must_use]
pub fn hashed_name(file_name: &str, bytes: &[u8]) -> String {
    let hash = Hash::of_bytes(bytes).short(6);
    match file_name.rsplit_once('.') {
        Some((stem, ext)) => format!("{stem}.{hash}.{ext}"),
        None => format!("{file_name}.{hash}"),
    }
}

/// True when the asset should be inlined as a data URL.
#[must_use]
pub fn should_inline(bytes_len: usize, query: AssetQuery, limit: usize) -> bool {
    match query {
        AssetQuery::Inline | AssetQuery::Raw => true,
        AssetQuery::Url | AssetQuery::Worker | AssetQuery::Wasm => false,
        AssetQuery::Auto => bytes_len <= limit,
    }
}

/// Encode bytes as a data URL.
#[must_use]
pub fn to_data_url(bytes: &[u8], file_name: &str) -> String {
    use base64::Engine as _;
    let mime = mime_guess::from_path(file_name).first_or_octet_stream();
    format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

/// Guess the content type for a file name.
#[must_use]
pub fn content_type(file_name: &str) -> String {
    mime_guess::from_path(file_name)
        .first_or_octet_stream()
        .to_string()
}

/// Render the dev/build JS shim for an asset import.
#[must_use]
pub fn asset_to_js(url_or_data: &str) -> String {
    format!("export default {url_or_data:?};\n")
}

/// Render the JS shim for a `?raw` import.
#[must_use]
pub fn raw_to_js(text: &str) -> String {
    format!("export default {text:?};\n")
}

/// An emitted asset file.
#[derive(Debug, Clone)]
pub struct EmittedAsset {
    /// Output file name (hashed).
    pub file_name: String,
    /// File bytes.
    pub bytes: Vec<u8>,
    /// Content type.
    pub content_type: String,
}

/// Emit an asset: returns either inline JS or the emitted file + URL.
#[must_use]
pub fn emit_asset(
    file_name: &str,
    bytes: Vec<u8>,
    query: AssetQuery,
    inline_limit: usize,
    public_url: impl Fn(&str) -> String,
) -> AssetEmit {
    if query == AssetQuery::Raw {
        let text = String::from_utf8_lossy(&bytes).into_owned();
        return AssetEmit::InlineJs(raw_to_js(&text));
    }
    if should_inline(bytes.len(), query, inline_limit) {
        return AssetEmit::InlineJs(asset_to_js(&to_data_url(&bytes, file_name)));
    }
    let hashed = hashed_name(file_name, &bytes);
    let url = public_url(&hashed);
    AssetEmit::File {
        asset: EmittedAsset {
            file_name: hashed,
            content_type: content_type(file_name),
            bytes,
        },
        js: asset_to_js(&url),
    }
}

/// Result of emitting an asset.
#[derive(Debug)]
pub enum AssetEmit {
    /// The import becomes inline JS (no file emitted).
    InlineJs(String),
    /// A file is emitted and the import becomes a URL shim.
    File {
        /// Emitted file.
        asset: EmittedAsset,
        /// JS shim.
        js: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_queries() {
        assert_eq!(
            AssetQuery::from_id("./a.svg?raw"),
            ("./a.svg".to_string(), AssetQuery::Raw)
        );
        assert_eq!(
            AssetQuery::from_id("./a.bin"),
            ("./a.bin".to_string(), AssetQuery::Auto)
        );
    }

    #[test]
    fn hashes_names() {
        let name = hashed_name("logo.svg", b"bytes");
        assert!(name.starts_with("logo."));
        assert!(name.ends_with(".svg"));
    }

    #[test]
    fn inlines_small_assets() {
        let emit = emit_asset(
            "a.svg",
            b"<svg/>".to_vec(),
            AssetQuery::Auto,
            4096,
            |name| format!("/assets/{name}"),
        );
        assert!(matches!(emit, AssetEmit::InlineJs(_)));
    }
}
