//! Environment file loading.

use std::collections::HashMap;
use std::path::Path;

/// Load `.env*` files (§42), keeping only `prefix`-allowed keys.
#[must_use]
pub fn load_env_files(root: &Path, mode: &str, prefixes: &[String]) -> HashMap<String, String> {
    let mut values = HashMap::new();
    for file in [
        ".env",
        ".env.local",
        &format!(".env.{mode}"),
        &format!(".env.{mode}.local"),
    ] {
        let path = root.join(file);
        if let Ok(text) = std::fs::read_to_string(&path) {
            for (key, value) in parse_dotenv(&text) {
                values.insert(key, value);
            }
        }
    }
    values.retain(|key, _| prefixes.iter().any(|prefix| key.starts_with(prefix)));
    values
}

/// Minimal dotenv parser (no variable expansion in v0.1).
#[must_use]
pub fn parse_dotenv(text: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_string();
        let mut value = value.trim().to_string();
        if (value.starts_with('"') && value.ends_with('"') && value.len() >= 2)
            || (value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2)
        {
            value = value[1..value.len() - 1].to_string();
        }
        if !key.is_empty() {
            pairs.push((key, value));
        }
    }
    pairs
}
