//! SSR programmatic tools: `ssrTransform`, `ssrFixStacktrace`, `moduleRunner`.

use crate::util::*;
use crate::CachedTransform;
use crate::DevServer;
use ferrite_core::ModuleId;
use ferrite_core::ModuleType;
use ferrite_core::Result;
use ferrite_graph::ImportKind;
use ferrite_plugin::TransformRequest as HookTransformRequest;
use ferrite_ssr::SsrModule;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Mutex;

/// `ssrTransform` result.
#[derive(Debug, Clone)]
pub struct SsrTransformResult {
    /// Transformed SSR code.
    pub code: String,
    /// Source map JSON, when enabled.
    pub map: Option<String>,
    /// Resolved static dependency ids.
    pub deps: Vec<String>,
    /// Resolved dynamic dependency ids.
    pub dynamic_deps: Vec<String>,
}

/// SSR module runner (`server.moduleRunner`).
///
/// A shared-pipeline-cached `ssrLoadModule` with explicit invalidation. Executing the
/// loaded graph goes through the configured [`ferrite_ssr::SsrAdapter`];
/// without one the runner loads and tracks modules only.
pub struct ModuleRunner {
    server: DevServer,
    cache: Mutex<HashMap<String, String>>,
}

impl ModuleRunner {
    /// Runner over `server`.
    #[must_use]
    pub fn new(server: DevServer) -> Self {
        Self {
            server,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Load an SSR module graph through the shared per-module cache, which
    /// validates source and compiler-input dependency state on every import.
    pub async fn import(&self, url: &str) -> Result<SsrModule> {
        // A second graph cache cannot bypass pipeline validation: source,
        // preprocessing inputs and transitive modules may have changed even
        // without a watcher. Retain only successfully validated graph records.
        if let Ok(mut cache) = self.cache.lock() {
            cache.remove(url);
        }
        let module = self.server.ssr_load_module(url).await?;
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(url.to_string(), module.id.clone());
        }
        Ok(module)
    }

    /// Resolve a recorded alias to its canonical module, invalidate its importers,
    /// and remove every affected recorded URL.
    pub fn invalidate(&self, url: &str) {
        let mut cache = self.cache.lock().ok();
        let canonical = cache
            .as_ref()
            .and_then(|cache| cache.get(url))
            .map_or(url, String::as_str);
        let invalidated: HashSet<_> = self
            .server
            .inner
            .graph
            .invalidate_tree(&ModuleId::new(canonical))
            .into_iter()
            .map(|id| id.0)
            .collect();
        if let Some(cache) = &mut cache {
            cache.retain(|_, canonical| !invalidated.contains(canonical));
        }
    }

    /// Drop the whole runner cache (the graph is untouched).
    pub fn clear(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.clear();
        }
    }

    /// Cached URLs.
    #[must_use]
    pub fn cached_urls(&self) -> Vec<String> {
        self.cache
            .lock()
            .map(|cache| cache.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Close the runner (clears the cache).
    pub async fn close(&self) {
        self.clear();
    }
}

impl DevServer {
    /// Runner handle for this server (`server.moduleRunner`).
    #[must_use]
    pub fn module_runner(&self) -> ModuleRunner {
        ModuleRunner::new(self.clone())
    }

    /// Transform supplied `code` for SSR without inserting it into the graph.
    /// Re-export discovery can consult the shared dependency pipeline/cache.
    /// (`ssrTransform`): pre transforms → JS/TS lowering → post transforms →
    /// CJS interop → import rewriting.
    pub async fn ssr_transform(&self, code: &str, url: &str) -> Result<SsrTransformResult> {
        let environment = self.inner.config.ssr_env();
        let mut ctx = self.plugin_context(&environment);
        let module_watches = std::sync::Mutex::new(Vec::new());
        ctx.watch_files = &module_watches;
        let id = ModuleId::new(url);
        let (path_part, _) = id.split_query();
        let module_type = ModuleType::from_path(path_part);
        let pre = self
            .inner
            .plugins
            .hook_transform_phase(
                &ctx,
                HookTransformRequest {
                    id: id.0.clone(),
                    code: code.into(),
                    module_type: module_type.clone(),
                    environment: environment.kind.clone(),
                    ssr: true,
                },
                ferrite_plugin::TransformPhase::BeforeLowering,
            )
            .await?;
        let mut module = self
            .core_transform(
                &ctx,
                &id,
                &pre.code,
                &pre.module_type.unwrap_or(module_type),
                &environment,
                false,
            )
            .await?;
        module.map = crate::loader::merge_maps(
            module.map,
            pre.map.map(|map| map.mappings),
            module.code == pre.code,
        )?;
        module.dependencies.extend(pre.dependencies);
        if module.module_type.is_js_like() {
            let hooked = self
                .inner
                .plugins
                .hook_transform_phase(
                    &ctx,
                    HookTransformRequest {
                        id: id.0.clone(),
                        code: module.code.clone(),
                        module_type: module.module_type.clone(),
                        environment: environment.kind.clone(),
                        ssr: true,
                    },
                    ferrite_plugin::TransformPhase::AfterLowering,
                )
                .await?;
            module.map = crate::loader::merge_maps(
                hooked.map.map(|map| map.mappings),
                module.map,
                hooked.code == module.code,
            )?;
            module.code = hooked.code;
            module.dependencies.extend(hooked.dependencies);
            if let Some(kind) = hooked.module_type {
                module.module_type = kind;
            }
            let parsed = self.inner.compiler.parse(ferrite_transform::ParseRequest {
                id: id.0.clone(),
                code: module.code.clone(),
                module_type: module.module_type.clone(),
            })?;
            module.has_module_syntax = parsed.has_module_syntax;
            module.uses_import_meta_hot = parsed.uses_import_meta_hot;
            if ferrite_transform::analyze_commonjs(&id.0, &module.code)?.is_commonjs
                || needs_cjs_conversion(&id, &module.code, module.has_module_syntax)
            {
                module = self
                    .convert_cjs_output(&ctx, module, &environment, true)
                    .await?;
            }
            module = self
                .rewrite_module_imports(&ctx, module, &environment)
                .await?;
        }
        let mut deps = Vec::new();
        let mut dynamic_deps = Vec::new();
        for (_, dep, kind) in &module.imports {
            if *kind == ImportKind::Dynamic {
                dynamic_deps.push(dep.0.clone());
            } else {
                deps.push(dep.0.clone());
            }
        }
        Ok(SsrTransformResult {
            code: module.code,
            map: module.map,
            deps,
            dynamic_deps,
        })
    }

    /// Rewrite stack-trace frames through cached SSR transform maps
    /// (`ssrFixStacktrace`). Frames without a cached map are normalized
    /// (origin stripped, `/@id/` URLs decoded) but keep their positions.
    pub async fn ssr_fix_stacktrace(&self, stack: &str) -> String {
        let mut out = Vec::new();
        for line in stack.lines() {
            out.push(self.fix_stack_line(line).await);
        }
        out.join("\n")
    }

    /// Fix one stack line.
    async fn fix_stack_line(&self, line: &str) -> String {
        let Some((start, end, url, line_no, col_no)) = split_frame(line) else {
            return line.to_string();
        };
        let normalized = normalize_frame_url(&self.inner.config.root, url);
        let fixed = self
            .map_frame_position(&normalized, line_no, col_no)
            .await
            .unwrap_or_else(|| format!("{normalized}:{line_no}:{col_no}"));
        format!("{}{fixed}{}", &line[..start], &line[end..])
    }

    /// Map a generated position through the cached SSR transform map.
    async fn map_frame_position(
        &self,
        normalized: &str,
        line_no: usize,
        col_no: usize,
    ) -> Option<String> {
        let id = ModuleId::new(normalized);
        let file = self.id_to_file(&id).ok()?;
        let source = std::fs::read_to_string(&file).ok()?;
        let environment = self.inner.config.ssr_env();
        let defines = self.transform_defines(&environment);
        let key = self.cache_key(&id, &source, "ssr", &defines);
        let bytes = self.inner.cache.get(&key.0)?;
        let cached: CachedTransform = serde_json::from_slice(&bytes).ok()?;
        let map = cached.map?;
        let (source_idx, orig_line, orig_col) = map_generated_position(&map, line_no, col_no)?;
        let source_name =
            map_source_name(&map, source_idx).unwrap_or_else(|| normalized.to_string());
        Some(format!("{source_name}:{orig_line}:{orig_col}"))
    }
}

/// Split a trailing `url:line:col` frame reference from `line`.
///
/// Returns the byte range plus the URL and 1-based position.
fn split_frame(line: &str) -> Option<(usize, usize, &str, usize, usize)> {
    let end = line.rfind(')').map_or(line.len(), |index| index);
    // `segment` is a prefix of `line`, so segment-relative offsets are
    // line-relative.
    let segment = line[..end].trim_end();
    // Walk back over `:col` then `:line`.
    let col_sep = segment.rfind(':')?;
    let col_no: usize = segment[col_sep + 1..].parse().ok()?;
    let head = &segment[..col_sep];
    let line_sep = head.rfind(':')?;
    let line_no: usize = head[line_sep + 1..].parse().ok()?;
    let trimmed = head[..line_sep].trim_end();
    let token = trimmed.split_whitespace().next_back().unwrap_or(trimmed);
    let url = token.strip_prefix('(').unwrap_or(token);
    // The URL must look like a path or URL, not prose with colons.
    if url.is_empty()
        || url.contains(' ')
        || !(url.starts_with('/')
            || url.starts_with("http://")
            || url.starts_with("https://")
            || url.starts_with("file://"))
    {
        return None;
    }
    // `url` is a suffix of `trimmed`, which ends the segment prefix.
    let start = trimmed.len() - url.len();
    let end = start + url.len() + head[line_sep..].len() + segment[col_sep..].len();
    Some((start, end, url, line_no, col_no))
}

/// Normalize a frame URL to a cacheable module id form.
fn normalize_frame_url(root: &Path, url: &str) -> String {
    let path = strip_origin(url);
    let path = path.split(['?', '#']).next().unwrap_or(path);
    if let Some(virtual_id) = crate::url_to_virtual(path) {
        return virtual_id.0;
    }
    if path.starts_with('/') {
        return path.to_string();
    }
    // `file://` URLs that survived origin stripping are absolute paths.
    let absolute = Path::new(path);
    if absolute.is_absolute() {
        return ferrite_core::file_to_url(root, absolute);
    }
    format!("/{path}")
}

/// Strip `scheme://host[:port]` from a URL, keeping the path.
fn strip_origin(url: &str) -> &str {
    if let Some(rest) = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
    {
        return rest.split_once('/').map_or("/", |(_, path)| {
            // Reattach the leading slash without allocating twice.
            &url[url.len() - path.len() - 1..]
        });
    }
    if let Some(rest) = url.strip_prefix("file://") {
        return rest;
    }
    url
}

/// Map a 1-based generated position through a source-map JSON document.
///
/// Returns `(source_index, original_line, original_column)`, 1-based.
fn map_generated_position(
    map_json: &str,
    line_no: usize,
    col_no: usize,
) -> Option<(usize, usize, usize)> {
    let map: serde_json::Value = serde_json::from_str(map_json).ok()?;
    let mappings = map.get("mappings")?.as_str()?;
    let lines: Vec<&str> = mappings.split(';').collect();
    let segments = lines.get(line_no.checked_sub(1)?)?;
    let target_col = col_no.saturating_sub(1);
    let mut gen_col: i64 = 0;
    let mut src_idx: i64 = 0;
    let mut orig_line: i64 = 0;
    let mut orig_col: i64 = 0;
    let mut best: Option<(usize, usize, usize)> = None;
    for segment in segments.split(',') {
        if segment.is_empty() {
            continue;
        }
        let fields = decode_vlq_segment(segment)?;
        if fields.len() == 1 {
            gen_col += fields[0];
            continue;
        }
        if fields.len() < 4 {
            continue;
        }
        gen_col += fields[0];
        src_idx += fields[1];
        orig_line += fields[2];
        orig_col += fields[3];
        if gen_col <= target_col as i64 && src_idx >= 0 && orig_line >= 0 && orig_col >= 0 {
            best = Some((
                src_idx as usize,
                orig_line as usize + 1,
                orig_col as usize + 1,
            ));
        }
    }
    best
}

/// `sources[source_idx]` from a source-map JSON document.
fn map_source_name(map_json: &str, source_idx: usize) -> Option<String> {
    let map: serde_json::Value = serde_json::from_str(map_json).ok()?;
    map.get("sources")?
        .as_array()?
        .get(source_idx)?
        .as_str()
        .map(str::to_string)
}

/// Decode one VLQ mapping segment into its integer fields.
fn decode_vlq_segment(segment: &str) -> Option<Vec<i64>> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut table = [0i64; 256];
    for (index, byte) in ALPHABET.iter().enumerate() {
        table[*byte as usize] = index as i64;
    }
    let mut fields = Vec::new();
    let mut value: i64 = 0;
    let mut shift = 0;
    for byte in segment.bytes() {
        let digit = table[byte as usize];
        value |= (digit & 31) << shift;
        shift += 5;
        if digit & 32 == 0 {
            let negative = value & 1 == 1;
            value >>= 1;
            fields.push(if negative { -value } else { value });
            value = 0;
            shift = 0;
        }
    }
    if shift != 0 {
        return None;
    }
    Some(fields)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_splitting() {
        let line = "    at render (http://127.0.0.1:5173/src/app.ts:10:5)";
        let (start, end, url, line_no, col_no) = split_frame(line).unwrap();
        assert_eq!(url, "http://127.0.0.1:5173/src/app.ts");
        assert_eq!((line_no, col_no), (10, 5));
        assert_eq!(&line[start..end], "http://127.0.0.1:5173/src/app.ts:10:5");

        let bare = "at /src/a.ts:1:2";
        let (start, end, url, _, _) = split_frame(bare).unwrap();
        assert_eq!(url, "/src/a.ts");
        assert_eq!(&bare[start..end], "/src/a.ts:1:2");

        assert!(split_frame("Error: boom").is_none());
        assert!(split_frame("at foo (native)").is_none());
        assert!(split_frame("just some prose: with colons").is_none());
    }

    #[test]
    fn origin_stripping() {
        assert_eq!(strip_origin("http://h:1/a/b?x=1"), "/a/b?x=1");
        assert_eq!(strip_origin("https://h/a"), "/a");
        assert_eq!(strip_origin("http://h"), "/");
        assert_eq!(strip_origin("/a/b"), "/a/b");
        assert_eq!(strip_origin("file:///tmp/x.ts"), "/tmp/x.ts");
    }

    #[test]
    fn vlq_mapping_roundtrip() {
        // `AAAA` = gen_col 0, src 0, orig line 0, orig col 0.
        assert_eq!(decode_vlq_segment("AAAA"), Some(vec![0, 0, 0, 0]));
        // `C` = 1, `D` = -1.
        assert_eq!(decode_vlq_segment("C"), Some(vec![1]));
        assert_eq!(decode_vlq_segment("D"), Some(vec![-1]));
        let map = serde_json::json!({
            "version": 3,
            "sources": ["src/a.ts"],
            "mappings": "AAAA,CACC",
        });
        let text = map.to_string();
        // Line 1 col 1 → src 0, line 1, col 1.
        assert_eq!(map_generated_position(&text, 1, 1), Some((0, 1, 1)));
        // Line 1 col 2 → second segment: gen col 1, orig line 2, col 2.
        assert_eq!(map_generated_position(&text, 1, 2), Some((0, 2, 2)));
        assert_eq!(map_source_name(&text, 0).as_deref(), Some("src/a.ts"));
        assert!(map_generated_position(&text, 9, 1).is_none());
    }
}
