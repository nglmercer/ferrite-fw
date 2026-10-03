//! Build module loader and preview.

use ferrite_bundler::CssExtract;
use ferrite_bundler::LoadedModule;
use ferrite_bundler::ModuleLoader;
use ferrite_config::ResolvedConfig;
use ferrite_core::Environment;
use ferrite_core::FerriteError;
use ferrite_core::ModuleId;
use ferrite_core::ModuleType;
use ferrite_core::Result;
use ferrite_server::DevServer;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

/// [`ModuleLoader`] over the dev-server pipeline (build mode).
pub struct BuildLoader {
    /// Pipeline server (production config, no watcher).
    pub server: DevServer,
    /// Minify per module.
    pub minify: bool,
    /// Emit source maps.
    pub sourcemap: bool,
    /// Public base with trailing slash.
    pub base: String,
    /// Loader-emitted assets (name → bytes).
    pub(crate) assets: Mutex<HashMap<String, Vec<u8>>>,
}

impl BuildLoader {
    /// Drain emitted assets.
    #[must_use]
    pub fn take_assets(&self) -> HashMap<String, Vec<u8>> {
        self.assets
            .lock()
            .map(|mut map| std::mem::take(&mut *map))
            .unwrap_or_default()
    }

    /// Emit raw bytes as a hashed asset; returns the public URL.
    ///
    /// Runs `resolveFileUrl` hooks over the default URL; the first `Some`
    /// wins, otherwise the hashed `assets/` URL is used.
    async fn emit_url(&self, file_name: &str, bytes: Vec<u8>, env: &str) -> Result<String> {
        let hashed = ferrite_assets::hashed_name(file_name, &bytes);
        let name = format!("assets/{hashed}");
        let default_url = format!("{}{name}", self.base);
        if let Ok(mut assets) = self.assets.lock() {
            assets.insert(name.clone(), bytes);
        }
        let inner = self.server.inner();
        let environment = if env == "ssr" {
            inner.config.ssr_env()
        } else {
            inner.config.client_env()
        };
        let resolver = if environment.kind.is_ssr() {
            &inner.ssr_resolver
        } else {
            &inner.client_resolver
        };
        let ctx = ferrite_plugin::PluginContext {
            graph: &inner.graph,
            resolver,
            environment: &environment,
            emitted: &inner.emitted,
            watch_files: &inner.watch_files,
            warnings: &inner.warnings,
        };
        if let Some(url) = inner
            .plugins
            .hook_resolve_file_url(
                &ctx,
                ferrite_plugin::ResolveFileUrlRequest { file_name: name },
            )
            .await?
        {
            return Ok(url);
        }
        Ok(default_url)
    }

    /// Load a stylesheet for extraction (§29).
    ///
    /// Transforms (module scoping + minify), emits relative `url()` assets,
    /// and reports relative `@import`s as deps; the bundler turns those
    /// into hashed `.css` files. Absolute `/` refs point at `public/` and
    /// pass through; remote refs stay untouched.
    async fn load_css(&self, id: &ModuleId, env: &str) -> Result<LoadedModule> {
        let (source, _) = self.server.load_raw_source(id, env).await?;
        let is_modules = id.0.contains(".module.css");
        let result = ferrite_css::transform_css(
            &id.0,
            &source,
            &ferrite_css::CssOptions {
                modules: is_modules,
                minify: self.minify,
                dev: false,
            },
        );
        let resolve_env = self.server.inner().config.client_env();
        let mut url_mapping: HashMap<String, String> = HashMap::new();
        for found in &result.urls {
            let spec = &found.url;
            if !spec.starts_with("./") && !spec.starts_with("../") {
                continue;
            }
            match self.resolve_css_ref(id, spec, &resolve_env) {
                Ok(dep) => match self.server.id_to_file(&dep).and_then(|file| {
                    let bytes = std::fs::read(&file)?;
                    let name = file
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "asset".to_string());
                    Ok((name, bytes))
                }) {
                    Ok((name, bytes)) => {
                        let url = self.emit_url(&name, bytes, env).await?;
                        url_mapping.insert(spec.clone(), url);
                    }
                    Err(error) => {
                        tracing::warn!("cannot emit CSS asset `{spec}` from `{id}`: {error}");
                    }
                },
                Err(error) => {
                    tracing::warn!("cannot resolve CSS asset `{spec}` from `{id}`: {error}");
                }
            }
        }
        let text =
            ferrite_css::rewrite_css_urls(&result.code, |spec| url_mapping.get(spec).cloned());
        let mut imports = Vec::new();
        for found in &result.imports {
            let spec = &found.specifier;
            if !spec.starts_with("./") && !spec.starts_with("../") {
                continue;
            }
            match self.resolve_css_ref(id, spec, &resolve_env) {
                Ok(dep) => imports.push((spec.clone(), dep, ferrite_graph::ImportKind::Static)),
                Err(error) => {
                    tracing::warn!("cannot resolve CSS @import `{spec}` from `{id}`: {error}");
                }
            }
        }
        // Deterministic stub (sorted export map — content-hashed downstream).
        let code = if is_modules {
            let exports: std::collections::BTreeMap<&String, &String> =
                result.exports.iter().collect();
            format!(
                "export default {};\n",
                serde_json::to_string(&exports).unwrap_or_else(|_| "{}".to_string())
            )
        } else {
            "export default undefined;\n".to_string()
        };
        // CSS `@import` edges pull styles, not JS bindings; the stub exports
        // only `default`.
        let shake = ferrite_transform::ShakeInfo {
            import_bindings: imports
                .iter()
                .map(|_| vec![ferrite_transform::ImportBinding::SideEffect])
                .collect(),
            exports: vec![ferrite_transform::ParsedExport {
                exported: "default".to_string(),
                local: None,
                from: None,
                imported: None,
                target: None,
            }],
        };
        Ok(LoadedModule {
            id: id.clone(),
            code,
            imports,
            side_effects: None,
            module_type: ModuleType::Js,
            map: None,
            css: Some(CssExtract { text, is_modules }),
            shake: Some(shake),
        })
    }

    /// Resolve a CSS-relative ref through the client resolver.
    fn resolve_css_ref(&self, id: &ModuleId, spec: &str, env: &Environment) -> Result<ModuleId> {
        let resolved =
            self.server
                .inner()
                .client_resolver
                .resolve(&ferrite_resolver::ResolveRequest {
                    specifier: spec,
                    importer: Some(id),
                    environment: env.kind.clone(),
                    kind: ferrite_resolver::ResolveKind::Css,
                })?;
        if resolved.external || !resolved.id.0.starts_with('/') {
            return Err(FerriteError::Resolve(format!(
                "CSS ref `{spec}` from `{id}` is not a local file"
            )));
        }
        Ok(resolved.id)
    }
}

#[async_trait::async_trait]
impl ModuleLoader for BuildLoader {
    async fn load(&self, id: &ModuleId, env: &str) -> Result<LoadedModule> {
        // CSS extraction (§29): plain styles ride `LoadedModule.css` into
        // hashed `.css` assets (query variants like `?inline` keep the
        // normal pipeline).
        let (path, query) = id.split_query();
        if query.is_none() && ModuleType::from_path(path) == ModuleType::Css {
            return self.load_css(id, env).await;
        }
        // Component compilers own CSS query resources whose physical owner has
        // a component extension. Honor their resolved type for extraction too.
        if let Some(query) = query {
            let inline = query
                .split('&')
                .any(|part| matches!(part, "inline" | "raw" | "url" | "direct"));
            if !inline
                && self
                    .server
                    .resolve_module(&id.0, None, env)
                    .await?
                    .module_type
                    == Some(ModuleType::Css)
            {
                return self.load_css(id, env).await;
            }
        }
        let module = self.server.pipeline_module(id, None, env).await?;
        // Raw assets become hashed files + URL shims.
        if module.is_raw_bytes {
            let file = self.server.id_to_file(id)?;
            let bytes = std::fs::read(&file)?;
            let name = file
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "asset".to_string());
            let url = self.emit_url(&name, bytes, env).await?;
            return Ok(LoadedModule {
                id: id.clone(),
                code: ferrite_assets::asset_to_js(&url),
                imports: Vec::new(),
                side_effects: module.side_effects,
                module_type: ModuleType::Js,
                map: None,
                css: None,
                shake: Some(ferrite_transform::ShakeInfo::default()),
            });
        }
        // The pipeline already rewrote specifiers to resolved urls, so the
        // bundler must key replacements by the *resolved* text present in
        // `code` (not the original specifier).
        let mut code = module.code.clone();
        let mut imports = Vec::new();
        let mut import_bindings = Vec::new();
        for (index, (_specifier, dep, kind)) in module.imports.iter().enumerate() {
            // Dropped `?asset-shim` edges drop their bindings in step.
            let bindings = module
                .shake
                .as_ref()
                .and_then(|shake| shake.import_bindings.get(index).cloned());
            if let Some(plain) = dep.0.strip_suffix("?asset-shim") {
                // Eagerly resolve `?asset-shim` imports to final URLs.
                let asset_id = ModuleId::new(plain);
                match self.server.id_to_file(&asset_id).and_then(|file| {
                    let bytes = std::fs::read(&file)?;
                    let name = file
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "asset".to_string());
                    Ok((name, bytes))
                }) {
                    Ok((name, bytes)) => {
                        let url = self.emit_url(&name, bytes, env).await?;
                        for quote in ['"', '\'', '`'] {
                            code = code.replace(
                                &format!("{quote}{}{quote}", dep.0),
                                &format!("{quote}{url}{quote}"),
                            );
                        }
                    }
                    Err(error) => {
                        tracing::warn!("cannot emit asset `{plain}`: {error}");
                        imports.push((dep.0.clone(), dep.clone(), kind.clone()));
                        if let Some(bindings) = bindings {
                            import_bindings.push(bindings);
                        }
                    }
                }
            } else {
                imports.push((dep.0.clone(), dep.clone(), kind.clone()));
                if let Some(bindings) = bindings {
                    import_bindings.push(bindings);
                }
            }
        }
        // Alignment is all-or-nothing: a partial binding list would
        // misattribute names, so a short list voids the shake facts.
        let mut shake = module.shake.as_ref().and_then(|shake| {
            (import_bindings.len() == imports.len()).then(|| ferrite_transform::ShakeInfo {
                import_bindings,
                exports: shake.exports.clone(),
            })
        });
        // Minify per module (production).
        let mut map = if self.sourcemap { module.map } else { None };
        if self.minify && module.module_type.is_js_like() {
            let minified =
                self.server
                    .inner()
                    .compiler
                    .minify(ferrite_transform::MinifyRequest {
                        id: id.0.clone(),
                        code: code.clone(),
                        sourcemap: self.sourcemap,
                        input_map: map.clone().map(ferrite_core::SourceMap::external),
                    })?;
            code = minified.code;
            map = minified.map.map(|chained| chained.mappings);
            // Minification can turn re-exports into imports plus local exports.
            // Analyze the emitted syntax: old positional bindings no longer
            // describe the names which the browser must link.
            let parsed = self
                .server
                .inner()
                .compiler
                .parse(ferrite_transform::ParseRequest {
                    id: id.0.clone(),
                    code: code.clone(),
                    module_type: ModuleType::Js,
                })?;
            let (final_imports, final_shake) = final_build_analysis(parsed, &imports)?;
            imports = final_imports;
            shake = Some(final_shake);
        }
        Ok(LoadedModule {
            id: id.clone(),
            code,
            imports,
            side_effects: module.side_effects,
            module_type: module.module_type.clone(),
            map,
            css: None,
            shake,
        })
    }
}

type BuildImport = (String, ModuleId, ferrite_graph::ImportKind);

fn final_build_analysis(
    parsed: ferrite_transform::ParsedModule,
    edges: &[BuildImport],
) -> Result<(Vec<BuildImport>, ferrite_transform::ShakeInfo)> {
    let mut imports = Vec::new();
    let mut import_bindings = Vec::new();
    for import in parsed.imports {
        if import.is_type {
            continue;
        }
        let edge = edges
            .iter()
            .find(|(specifier, _, _)| specifier == &import.specifier)
            .ok_or_else(|| {
                FerriteError::Other(format!(
                    "minifier introduced unresolved import `{}` in `{}`",
                    import.specifier, parsed.id
                ))
            })?;
        imports.push(edge.clone());
        import_bindings.push(import.bindings);
    }
    let mut exports = parsed.export_details;
    for export in &mut exports {
        if let Some(from) = &export.from {
            export.target = imports
                .iter()
                .find(|(specifier, _, _)| specifier == from)
                .map(|(_, id, _)| id.clone());
        }
    }
    Ok((
        imports,
        ferrite_transform::ShakeInfo {
            import_bindings,
            exports,
        },
    ))
}

// --- preview -------------------------------------------------------------------

/// Preview a resolved config with plugin hooks (`configResolved` →
/// `configurePreviewServer` / `configurePreview`), serving the output dir
/// with proxy rules, plugin mounts/headers, and SPA fallback.
pub async fn preview_with_plugins(
    config: &ResolvedConfig,
    plugins: &[Arc<dyn ferrite_plugin::Plugin>],
) -> Result<()> {
    let container =
        ferrite_plugin::PluginContainer::new(plugins.to_vec(), ferrite_plugin::Apply::All);
    container.hook_config_resolved(config).await?;
    let mut control = ferrite_plugin::PreviewControl::new(config.clone());
    container.hook_configure_preview(&mut control).await?;
    preview_with_control(&config.out_dir(), config.server.port, &control).await
}

/// Serve `dir` statically with SPA fallback (`preview`, §83 light).
///
/// Plugin-less shorthand; use [`preview_with_plugins`] for hooks, proxy
/// rules, mounts, and extra headers.
pub async fn preview_dir(dir: &std::path::Path, port: u16) -> Result<()> {
    preview_with_parts(dir, port, &[], &[], &[]).await
}

/// Serve `dir` with an explicit preview control surface.
async fn preview_with_control(
    dir: &std::path::Path,
    port: u16,
    control: &ferrite_plugin::PreviewControl,
) -> Result<()> {
    preview_with_parts(dir, port, &control.headers, &control.mounts, &control.proxies).await
}

/// Shared preview server: proxies → mounts → static + SPA fallback.
async fn preview_with_parts(
    dir: &std::path::Path,
    port: u16,
    headers: &[(String, String)],
    mounts: &[ferrite_plugin::PreviewMount],
    proxies: &[ferrite_plugin::ProxyRule],
) -> Result<()> {
    use axum::extract::State;
    use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri};
    use axum::response::IntoResponse as _;
    #[derive(Clone)]
    struct PreviewState {
        dir: PathBuf,
        headers: Vec<(HeaderName, HeaderValue)>,
        mounts: Vec<ferrite_plugin::PreviewMount>,
        proxies: Vec<ferrite_plugin::ProxyRule>,
        http_client: reqwest::Client,
    }

    fn parsed_headers(headers: &[(String, String)]) -> Vec<(HeaderName, HeaderValue)> {
        headers
            .iter()
            .filter_map(|(name, value)| match (name.parse(), value.parse()) {
                (Ok(name), Ok(value)) => Some((name, value)),
                _ => {
                    tracing::warn!("ignoring invalid preview header `{name}`");
                    None
                }
            })
            .collect()
    }

    fn file_response(
        file: &std::path::Path,
        headers: &[(HeaderName, HeaderValue)],
    ) -> axum::response::Response {
        match std::fs::read(file) {
            Ok(bytes) => {
                let content_type = ferrite_assets::content_type(file.to_string_lossy().as_ref());
                let immutable = file
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.contains('-'));
                let cache = if immutable {
                    "public, max-age=31536000, immutable"
                } else {
                    "no-cache"
                };
                let mut map = HeaderMap::new();
                map.insert(axum::http::header::CONTENT_TYPE, content_type.parse().unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")));
                map.insert(axum::http::header::CACHE_CONTROL, HeaderValue::from_static(cache));
                for (name, value) in headers {
                    map.insert(name, value.clone());
                }
                (StatusCode::OK, map, bytes).into_response()
            }
            Err(_) => (StatusCode::NOT_FOUND, "not found").into_response(),
        }
    }

    async fn handler(
        State(state): State<PreviewState>,
        method: Method,
        uri: Uri,
        headers: HeaderMap,
        body: axum::body::Bytes,
    ) -> axum::response::Response {
        let path = uri.path().to_string();
        let url = match uri.query() {
            Some(query) => format!("{path}?{query}"),
            None => path.clone(),
        };
        // 1. Proxy rules (longest prefix first).
        if let Some(rule) = state.proxies.iter().find(|rule| rule.matches(&path)) {
            let target = rule.forward_url(&url);
            return match ferrite_server::forward_proxy(
                &state.http_client,
                &method,
                &target,
                &headers,
                body,
            )
            .await
            {
                Ok(response) => response,
                Err(error) => (
                    StatusCode::BAD_GATEWAY,
                    format!("proxy to `{target}` failed: {error}"),
                )
                    .into_response(),
            };
        }
        // 2. Plugin static mounts.
        for mount in &state.mounts {
            let prefix = mount.prefix.trim_end_matches('/');
            let relative = if path == mount.prefix || path == prefix {
                Some("")
            } else {
                path.strip_prefix(&format!("{prefix}/"))
            };
            if let Some(relative) = relative {
                let file = mount.dir.join(relative);
                if file.is_file() {
                    return file_response(&file, &state.headers);
                }
            }
        }
        // 3. Output dir with SPA fallback.
        let file = state.dir.join(path.trim_start_matches('/'));
        let file = if file.is_file() {
            file
        } else {
            state.dir.join("index.html")
        };
        file_response(&file, &state.headers)
    }

    if !dir.exists() {
        return Err(FerriteError::Build(format!(
            "`{}` does not exist; run `ferrite build` first",
            dir.display()
        )));
    }
    let router = axum::Router::new().fallback(handler).with_state(PreviewState {
        dir: dir.to_path_buf(),
        headers: parsed_headers(headers),
        mounts: mounts.to_vec(),
        proxies: proxies.to_vec(),
        http_client: reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new()),
    });
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}"))
        .await
        .map_err(|error| FerriteError::Other(format!("cannot bind preview:{port}: {error}")))?;
    let addr = listener
        .local_addr()
        .map_err(|error| FerriteError::Other(error.to_string()))?;
    tracing::info!("preview at http://{addr}");
    println!("  preview at http://{addr}");
    axum::serve(listener, router)
        .await
        .map_err(|error| FerriteError::Other(error.to_string()))?;
    Ok(())
}

// --- helpers -------------------------------------------------------------------

/// Ensure a trailing slash on the base.
pub(crate) fn with_trailing_slash(base: &str) -> String {
    if base.ends_with('/') {
        base.to_string()
    } else {
        format!("{base}/")
    }
}

/// Copy `public/` → `out_dir/` (verbatim).
pub(crate) fn copy_public(config: &ResolvedConfig) -> Result<()> {
    let public = config.root.join("public");
    if !public.exists() {
        return Ok(());
    }
    copy_dir(&public, &config.out_dir())
}

/// Copy a directory tree.
pub(crate) fn copy_dir(from: &std::path::Path, to: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod build_analysis_tests {
    use super::*;
    use ferrite_transform::{JsCompiler, OxcCompiler, OxcOptions, ParseRequest};

    #[test]
    fn minified_reexport_imports_retain_linked_names() {
        let compiler = OxcCompiler::new(OxcOptions::default());
        let result = compiler.minify(ferrite_transform::MinifyRequest {
            id: "/barrel.js".into(),
            code: "import {ref} from './reactivity.js'; export {TrackOpTypes} from './reactivity.js'; export const value = ref(0);".into(),
            sourcemap: false,
            input_map: None,
        }).unwrap();
        let parsed = compiler
            .parse(ParseRequest {
                id: "/barrel.js".into(),
                code: result.code,
                module_type: ModuleType::Js,
            })
            .unwrap();
        let edges = vec![(
            "./reactivity.js".into(),
            ModuleId::new("/reactivity.js"),
            ferrite_graph::ImportKind::Static,
        )];
        let expected: Vec<_> = parsed
            .imports
            .iter()
            .map(|import| import.bindings.clone())
            .collect();
        let (imports, shake) = final_build_analysis(parsed, &edges).unwrap();
        assert_eq!(imports.len(), shake.import_bindings.len());
        assert_eq!(shake.import_bindings, expected);
        assert!(shake
            .exports
            .iter()
            .any(|export| export.exported == "TrackOpTypes"
                && export.target == Some(ModuleId::new("/reactivity.js"))));
    }
}
