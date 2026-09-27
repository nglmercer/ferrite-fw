//! SSR externals.

use crate::adapter::*;
use ferrite_config::SsrConfig;

/// True when `specifier` names a native `.node` binary (query stripped).
#[must_use]
pub fn is_native_specifier(specifier: &str) -> bool {
    specifier
        .split('?')
        .next()
        .unwrap_or(specifier)
        .ends_with(".node")
}

/// True when `id` (bare specifier or path) is SSR-external (§23).
///
/// Native `.node` binaries are always external: they cannot be bundled.
/// Load them at runtime through an embedded backend (see
/// [`native_shim_module`]).
#[must_use]
pub fn is_external(specifier: &str, config: &SsrConfig) -> bool {
    if is_native_specifier(specifier) {
        return true;
    }
    if config.bundle_all {
        return false;
    }
    let matches = |list: &[String]| {
        list.iter().any(|pattern| {
            pattern == specifier
                || specifier.starts_with(&format!("{pattern}/"))
                || (pattern.contains('*') && glob_match(pattern, specifier))
        })
    };
    if matches(&config.no_external) {
        return false;
    }
    if matches(&config.external) {
        return true;
    }
    // Default: bare imports of known server-only packages stay external.
    !specifier.starts_with('.')
        && !specifier.starts_with('/')
        && !specifier.starts_with("virtual:")
        && is_server_only(specifier)
}

/// SSR placeholder for a native `.node` binary (§22–§23).
///
/// Bundlers and `ssrLoadModule` implementations must substitute this for
/// any [`is_native_specifier`] import instead of reading the binary: the
/// stub throws on evaluation with an actionable message telling the user
/// to allowlist the file under `[runtime]` and select the `napi-vm`
/// backend (feature `napi-vm`, disabled by default).
#[must_use]
pub fn native_shim_module(specifier: &str) -> SsrModule {
    let literal = js_single_quoted(specifier);
    SsrModule {
        id: specifier.to_string(),
        code: format!(
            "throw new Error(\"[ferrite] cannot bundle native module {literal}: \
             .node binaries stay external in SSR. Allowlist the file under [runtime] \
             native_allow (+ native_integrity) and run with `--runtime napi-vm` \
             (build with `--features napi-vm`).\");\n"
        ),
        dependencies: Vec::new(),
    }
}

/// Render a Rust string as a JS single-quoted literal.
pub(crate) fn js_single_quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('\'');
    for ch in text.chars() {
        match ch {
            '\'' => out.push_str("\\'"),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            other => out.push(other),
        }
    }
    out.push('\'');
    out
}

/// Known server-only packages (kept external unless `noExternal`).
pub(crate) fn is_server_only(specifier: &str) -> bool {
    let name = specifier.split('/').next().unwrap_or(specifier);
    matches!(
        name,
        "pg" | "sharp" | "fsevents" | "node-gyp" | "esbuild" | "workerd"
    )
}

pub(crate) fn glob_match(pattern: &str, text: &str) -> bool {
    // Single-`*` glob.
    match pattern.split_once('*') {
        Some((prefix, suffix)) => text.starts_with(prefix) && text.ends_with(suffix),
        None => pattern == text,
    }
}
