//! napi-vm helpers.

use std::path::PathBuf;

pub(crate) fn vmkind(error: napi_vm::VmErr) -> String {
    error.to_string()
}

/// Render a Rust string as a JS single-quoted literal.
pub(crate) fn js_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('\'');
    for ch in text.chars() {
        match ch {
            '\'' => out.push_str("\\'"),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            other => out.push(other),
        }
    }
    out.push('\'');
    out
}

/// Resolve an allowlist path: absolute stays, relative joins the first root.
pub(crate) fn resolve_allow_path(roots: &[PathBuf], path: &std::path::Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    match roots.first() {
        Some(root) => root.join(path),
        None => path.to_path_buf(),
    }
}

/// Parse 64 hex chars into 32 bytes.
pub(crate) fn parse_sha256(hex: &str) -> std::result::Result<[u8; 32], String> {
    let hex = hex.trim();
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("expected 64 hex chars".to_string());
    }
    let mut bytes = [0u8; 32];
    for (index, chunk) in hex.as_bytes().chunks(2).enumerate() {
        let text = std::str::from_utf8(chunk).map_err(|error| error.to_string())?;
        bytes[index] = u8::from_str_radix(text, 16).map_err(|error| error.to_string())?;
    }
    Ok(bytes)
}
