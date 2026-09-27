//! Caching layers (spec §59–§61).
//!
//! Covers resolver, parse, transform, dependency, bundle, and SSR module
//! caches behind one key scheme, plus the `.ferrite/` persistent layout.

use std::path::{Path, PathBuf};

use dashmap::DashMap;
use ferrite_core::{FerriteError, Hash, Result};

/// Cache namespace (one directory / key prefix per layer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheLayer {
    /// Resolver results.
    Resolver,
    /// Registry metadata.
    Registry,
    /// Parsed modules.
    Parse,
    /// Transformed modules.
    Transform,
    /// Pre-bundled dependencies.
    Deps,
    /// Bundles.
    Bundle,
    /// SSR modules.
    Ssr,
    /// Fetched remote modules (§72).
    Remote,
}

impl CacheLayer {
    /// Directory name under `.ferrite/cache`.
    #[must_use]
    pub fn dir(&self) -> &'static str {
        match self {
            Self::Resolver => "resolver",
            Self::Registry => "registry",
            Self::Parse => "parse",
            Self::Transform => "transform",
            Self::Deps => "deps",
            Self::Bundle => "bundle",
            Self::Ssr => "ssr",
            Self::Remote => "remote",
        }
    }
}

/// Transform cache key inputs (§59).
#[derive(Debug, Clone)]
pub struct TransformKeyInput<'a> {
    /// Module source.
    pub source: &'a str,
    /// Module id.
    pub module_id: &'a str,
    /// Compiler version string.
    pub compiler_version: &'a str,
    /// Plugin pipeline hash.
    pub pipeline_hash: &'a str,
    /// Environment name.
    pub environment: &'a str,
    /// Target string.
    pub target: &'a str,
    /// Mode string.
    pub mode: &'a str,
    /// Canonicalized compile-time defines (`k=v` pairs, sorted, `\0`-joined).
    pub defines: &'a str,
}

/// Compute the transform cache key:
///
/// `blake3(source + module id + compiler version + pipeline hash +
/// environment + target + mode + defines)`.
#[must_use]
pub fn transform_key(input: &TransformKeyInput<'_>) -> Hash {
    let mut hasher = blake3::Hasher::new();
    for part in [
        input.source,
        "\0",
        input.module_id,
        "\0",
        input.compiler_version,
        "\0",
        input.pipeline_hash,
        "\0",
        input.environment,
        "\0",
        input.target,
        "\0",
        input.mode,
        "\0",
        input.defines,
    ] {
        hasher.update(part.as_bytes());
    }
    Hash(hasher.finalize().to_hex().to_string())
}

/// Dependency pre-bundle cache key (§27).
#[must_use]
pub fn deps_key(parts: &[&str]) -> Hash {
    let mut hasher = blake3::Hasher::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update(b"\0");
    }
    Hash(hasher.finalize().to_hex().to_string())
}

/// Thread-safe in-memory cache.
#[derive(Debug, Default)]
pub struct MemoryCache {
    inner: DashMap<String, Vec<u8>>,
}

impl MemoryCache {
    /// Create an empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Look up a key.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<Vec<u8>> {
        self.inner.get(key).map(|entry| entry.value().clone())
    }

    /// Insert a value.
    pub fn insert(&self, key: &str, value: Vec<u8>) {
        self.inner.insert(key.to_string(), value);
    }

    /// Invalidate one key. Returns true when something was removed.
    pub fn invalidate(&self, key: &str) -> bool {
        self.inner.remove(key).is_some()
    }

    /// Clear the whole cache.
    pub fn clear(&self) {
        self.inner.clear();
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// True when empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

/// Persistent cache rooted at `.ferrite/` (§61).
#[derive(Debug, Clone)]
pub struct DiskCache {
    /// `.ferrite/` directory.
    pub root: PathBuf,
}

impl DiskCache {
    /// Create (and initialize) a persistent cache root.
    pub fn new(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        for dir in ["cache", "deps", "graph", "npm"] {
            std::fs::create_dir_all(root.join(dir))?;
        }
        Ok(Self { root })
    }

    /// Path for a layer entry.
    #[must_use]
    pub fn path(&self, layer: CacheLayer, key: &Hash) -> PathBuf {
        self.root.join("cache").join(layer.dir()).join(&key.0)
    }

    /// Read a cache entry.
    pub fn get(&self, layer: CacheLayer, key: &Hash) -> Option<Vec<u8>> {
        std::fs::read(self.path(layer, key)).ok()
    }

    /// Write a cache entry.
    pub fn insert(&self, layer: CacheLayer, key: &Hash, value: &[u8]) -> Result<()> {
        let path = self.path(layer, key);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, value)?;
        Ok(())
    }

    /// Remove the whole `.ferrite/` directory (`ferrite clean`).
    pub fn clean(root: &Path) -> Result<()> {
        if root.exists() {
            std::fs::remove_dir_all(root)?;
        }
        Ok(())
    }

    /// Read metadata.json, if present.
    #[must_use]
    pub fn metadata(&self) -> Option<serde_json::Value> {
        std::fs::read_to_string(self.root.join("metadata.json"))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
    }

    /// Write metadata.json.
    pub fn write_metadata(&self, value: &serde_json::Value) -> Result<()> {
        std::fs::write(
            self.root.join("metadata.json"),
            serde_json::to_string_pretty(value).map_err(FerriteError::Json)?,
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_keys_differ_by_env() {
        let base = TransformKeyInput {
            source: "const a = 1;",
            module_id: "/src/a.ts",
            compiler_version: "oxc-0.151",
            pipeline_hash: "abc",
            environment: "client",
            target: "es2022",
            mode: "development",
            defines: "",
        };
        let key_client = transform_key(&base);
        let key_ssr = transform_key(&TransformKeyInput {
            environment: "ssr",
            ..base
        });
        assert_ne!(key_client, key_ssr);
    }

    #[test]
    fn transform_keys_differ_by_compiler_and_defines() {
        let base = TransformKeyInput {
            source: "const a = 1;",
            module_id: "/src/a.ts",
            compiler_version: "oxc-0.151",
            pipeline_hash: "abc",
            environment: "client",
            target: "es2022",
            mode: "development",
            defines: "A=1",
        };
        let key_oxc = transform_key(&base);
        let key_swc = transform_key(&TransformKeyInput {
            compiler_version: "swc-81",
            ..base
        });
        assert_ne!(key_oxc, key_swc);
        let key_defines = transform_key(&TransformKeyInput {
            defines: "A=2",
            ..base
        });
        assert_ne!(key_oxc, key_defines);
    }

    #[test]
    fn memory_cache_roundtrip() {
        let cache = MemoryCache::new();
        cache.insert("k", vec![1, 2, 3]);
        assert_eq!(cache.get("k"), Some(vec![1, 2, 3]));
        assert!(cache.invalidate("k"));
        assert!(cache.get("k").is_none());
    }
}
