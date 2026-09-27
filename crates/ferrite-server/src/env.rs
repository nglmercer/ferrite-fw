//! Environment file loading.

use std::collections::HashMap;
use std::path::Path;

/// Load `.env*` files (§42), keeping only `prefix`-allowed keys.
///
/// Layering (later wins): `.env`, `.env.local`, `.env.{mode}`,
/// `.env.{mode}.local`. Kept for backwards compatibility; see [`load_env`].
#[must_use]
pub fn load_env_files(root: &Path, mode: &str, prefixes: &[String]) -> HashMap<String, String> {
    load_env(mode, root, prefixes)
}

/// Load environment variables the way Vite's `loadEnv` does: read the
/// `.env*` file stack for `mode` under `root`, expand `$VAR` / `${VAR}` /
/// `${VAR:-default}` references (against the process environment first,
/// then already-loaded file values), and keep only `prefix`-allowed keys.
///
/// Only file variables are returned; the process environment is used for
/// expansion lookups, never merged into the result.
#[must_use]
pub fn load_env(mode: &str, root: &Path, prefixes: &[String]) -> HashMap<String, String> {
    let mut values: HashMap<String, String> = HashMap::new();
    for file in [
        ".env".to_string(),
        ".env.local".to_string(),
        format!(".env.{mode}"),
        format!(".env.{mode}.local"),
    ] {
        let path = root.join(file);
        if let Ok(text) = std::fs::read_to_string(&path) {
            for (key, raw) in parse_dotenv(&text) {
                values.insert(key.clone(), expand_vars(&raw, &values));
            }
        }
    }
    values.retain(|key, _| prefixes.iter().any(|prefix| key.starts_with(prefix)));
    values
}

/// Expand `$VAR`, `${VAR}`, and `${VAR:-default}` in `value`.
///
/// Lookup order for each reference: process environment, then `loaded`
/// (values parsed so far). Unknown variables without a default expand to
/// the empty string. `$$` is an escaped literal dollar. Expansion runs one
/// pass (references introduced by expansion are not re-expanded).
#[must_use]
pub fn expand_vars(value: &str, loaded: &HashMap<String, String>) -> String {
    if !value.contains('$') {
        return value.to_string();
    }
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(char) = chars.next() {
        if char != '$' {
            out.push(char);
            continue;
        }
        match chars.peek() {
            Some('$') => {
                chars.next();
                out.push('$');
            }
            Some('{') => {
                chars.next();
                let mut name = String::new();
                for char in chars.by_ref() {
                    if char == '}' {
                        break;
                    }
                    name.push(char);
                }
                let (var, default) = match name.split_once(":-") {
                    Some((var, default)) => (var, Some(default)),
                    None => (name.as_str(), None),
                };
                out.push_str(&lookup_var(var, default, loaded));
            }
            Some(char) if char.is_ascii_alphabetic() || *char == '_' => {
                let mut name = String::new();
                while let Some(char) = chars.peek() {
                    if char.is_ascii_alphanumeric() || *char == '_' {
                        name.push(*char);
                        chars.next();
                    } else {
                        break;
                    }
                }
                out.push_str(&lookup_var(&name, None, loaded));
            }
            _ => out.push('$'),
        }
    }
    out
}

/// Resolve one variable reference for [`expand_vars`].
fn lookup_var(name: &str, default: Option<&str>, loaded: &HashMap<String, String>) -> String {
    if let Ok(value) = std::env::var(name) {
        if !value.is_empty() {
            return value;
        }
    }
    if let Some(value) = loaded.get(name) {
        if !value.is_empty() {
            return value.clone();
        }
    }
    default.unwrap_or_default().to_string()
}

/// Parse dotenv text: `KEY=value`, `export KEY=value`, `#` comments,
/// single/double quotes (double quotes interpret `\"` `\\` `\n` `\r` `\t`),
/// and trailing ` # comment` after unquoted values.
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
        if key.is_empty() {
            continue;
        }
        pairs.push((key, parse_dotenv_value(value.trim())));
    }
    pairs
}

/// Parse one dotenv value (quotes, escapes, trailing comments).
fn parse_dotenv_value(value: &str) -> String {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        if bytes[0] == b'"' {
            return unescape_double(find_closing(value, b'"'));
        }
        if bytes[0] == b'\'' {
            // Single quotes: literal; a trailing ` # comment` still applies.
            let inner = find_closing(value, b'\'');
            return inner.to_string();
        }
    }
    // Unquoted: strip a trailing ` # comment`.
    let mut cut = value.len();
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'#' && (index == 0 || bytes[index - 1].is_ascii_whitespace()) {
            cut = index;
            break;
        }
        index += 1;
    }
    value[..cut].trim_end().to_string()
}

/// Text between the opening quote and its closer (or end of line when
/// unterminated).
fn find_closing(value: &str, quote: u8) -> &str {
    let bytes = value.as_bytes();
    let mut index = 1;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index += 2;
            continue;
        }
        if bytes[index] == quote {
            return &value[1..index];
        }
        index += 1;
    }
    &value[1..]
}

/// Interpret `\"` `\\` `\n` `\r` `\t` in a double-quoted value.
fn unescape_double(inner: &str) -> String {
    if !inner.contains('\\') {
        return inner.to_string();
    }
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(char) = chars.next() {
        if char != '\\' {
            out.push(char);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}
