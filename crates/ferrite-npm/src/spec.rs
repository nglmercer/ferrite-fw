//! Package spec parsing.

/// Parse `spec` (`name`, `name@range`, `@scope/name@range`) into parts.
#[must_use]
pub fn parse_spec(spec: &str) -> (String, String) {
    if let Some(rest) = spec.strip_prefix('@') {
        // Scoped: split at the *second* `@`.
        match rest.find('@') {
            Some(pos) => {
                let name = format!("@{}", &rest[..pos]);
                let range = rest[pos + 1..].to_string();
                (
                    name,
                    if range.is_empty() {
                        "latest".to_string()
                    } else {
                        range
                    },
                )
            }
            None => (spec.to_string(), "latest".to_string()),
        }
    } else {
        match spec.split_once('@') {
            Some((name, range)) => (
                name.to_string(),
                if range.is_empty() {
                    "latest".to_string()
                } else {
                    range.to_string()
                },
            ),
            None => (spec.to_string(), "latest".to_string()),
        }
    }
}
