//! Specifier splitting.

/// Split `specifier?query`.
pub(crate) fn split_query(specifier: &str) -> (&str, Option<&str>) {
    match specifier.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (specifier, None),
    }
}

/// Re-attach a query string.
pub(crate) fn attach_query(id: &str, query: Option<&str>) -> String {
    match query {
        Some(query) => format!("{id}?{query}"),
        None => id.to_string(),
    }
}

/// Split a bare specifier into `(package_name, /subpath)`.
pub(crate) fn split_package(specifier: &str) -> (String, String) {
    if let Some(rest) = specifier.strip_prefix('@') {
        // Scoped: `@scope/name/...`.
        let mut parts = rest.splitn(3, '/');
        let scope = parts.next().unwrap_or("");
        let name = parts.next().unwrap_or("");
        let tail = parts.next().unwrap_or("");
        let package = format!("@{scope}/{name}");
        if tail.is_empty() {
            (package, String::new())
        } else {
            (package, format!("/{tail}"))
        }
    } else {
        match specifier.split_once('/') {
            Some((name, tail)) => (name.to_string(), format!("/{tail}")),
            None => (specifier.to_string(), String::new()),
        }
    }
}
