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

/// Resolve an npm alias request while retaining the importing edge's name.
/// Non-registry protocols remain explicit errors rather than malformed semver.
pub fn registry_request(name: &str, range: &str) -> ferrite_core::Result<(String, String)> {
    crate::validate_package_name(name)?;
    let (target, range) = match range.strip_prefix("npm:") {
        Some(alias) => parse_spec(alias),
        None => (name.to_string(), range.to_string()),
    };
    crate::validate_package_name(&target)?;
    if range.contains(':') || range.starts_with('.') || range.starts_with('/') {
        return Err(ferrite_core::FerriteError::Npm(format!("unsupported registry request `{range}` for {name}; use an npm package version, range or tag")));
    }
    Ok((target, range))
}
