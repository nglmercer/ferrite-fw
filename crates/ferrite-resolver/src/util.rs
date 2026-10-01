//! Resolver helpers.

use std::path::Path;

/// True when `path` is under `dir` (lexically).
pub(crate) fn is_under(path: &Path, dir: &Path) -> bool {
    path.strip_prefix(dir).is_ok()
}

/// Bare (unprefixed) Node builtins that must not be treated as packages.
pub(crate) fn is_bare_node_builtin(specifier: &str) -> bool {
    matches!(
        specifier,
        "assert"
            | "buffer"
            | "child_process"
            | "cluster"
            | "crypto"
            | "dgram"
            | "dns"
            | "events"
            | "fs"
            | "http"
            | "https"
            | "net"
            | "os"
            | "path"
            | "process"
            | "querystring"
            | "stream"
            | "string_decoder"
            | "timers"
            | "tls"
            | "tty"
            | "url"
            | "util"
            | "v8"
            | "vm"
            | "worker_threads"
            | "zlib"
    )
}
