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
        let module = self.server.pipeline_stylesheet_module(id, env).await?;
        let stylesheet = module.stylesheet.as_ref().ok_or_else(|| FerriteError::Build(format!("CSS extraction for `{id}` requires stylesheet output; the configured transform changed its type")))?;
        let is_modules = stylesheet.is_modules;
        let result = ferrite_css::transform_css(
            &id.0,
            &stylesheet.code,
            &ferrite_css::CssOptions {
                modules: false,
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
            let dep = self
                .resolve_css_ref(id, spec, &resolve_env)
                .map_err(|error| {
                    FerriteError::Build(format!(
                        "cannot resolve CSS asset `{spec}` from `{id}`: {error}"
                    ))
                })?;
            let file = self.server.id_to_file(&dep)?;
            let bytes = std::fs::read(&file).map_err(|error| {
                FerriteError::Build(format!(
                    "cannot read CSS asset `{spec}` from `{id}` at {}: {error}",
                    file.display()
                ))
            })?;
            let name = file
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "asset".to_string());
            let url = self.emit_url(&name, bytes, env).await?;
            url_mapping.insert(spec.clone(), url);
        }
        let text =
            ferrite_css::rewrite_css_urls(&result.code, |spec| url_mapping.get(spec).cloned());
        let mut imports = Vec::new();
        for found in &result.imports {
            let spec = &found.specifier;
            if !spec.starts_with("./") && !spec.starts_with("../") {
                continue;
            }
            let dep = self
                .resolve_css_ref(id, spec, &resolve_env)
                .map_err(|error| {
                    FerriteError::Build(format!(
                        "cannot resolve CSS @import `{spec}` from `{id}`: {error}"
                    ))
                })?;
            imports.push((spec.clone(), dep, ferrite_graph::ImportKind::Static));
        }
        let css_import_count = imports.len();
        let mut js_imports: Vec<_> = module
            .imports
            .iter()
            .map(|(_, dep, kind)| (dep.0.clone(), dep.clone(), kind.clone()))
            .collect();
        js_imports.append(&mut imports);
        let shake = module.shake.map(|mut shake| {
            shake.import_bindings.extend(
                (0..css_import_count).map(|_| vec![ferrite_transform::ImportBinding::SideEffect]),
            );
            shake
        });
        let keep_js =
            module.code.trim() != "export default undefined;" || !module.imports.is_empty();
        Ok(LoadedModule {
            id: id.clone(),
            code: module.code,
            imports: js_imports,
            side_effects: module.side_effects,
            module_type: module.module_type,
            map: module.map,
            css: Some(CssExtract {
                text,
                is_modules,
                keep_js,
            }),
            shake,
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
    preview_with_parts(
        dir,
        port,
        PreviewRendering {
            base: "/",
            renderer: None,
        },
        &[],
        &[],
        &[],
    )
    .await
}

struct PreviewRendering<'a> {
    base: &'a str,
    renderer: Option<Arc<dyn ferrite_ssr::SsrAdapter>>,
}

/// Serve `dir` with an explicit preview control surface.
async fn preview_with_control(
    dir: &std::path::Path,
    port: u16,
    control: &ferrite_plugin::PreviewControl,
) -> Result<()> {
    let server_dir = dir.join("server");
    let has_server = server_dir.join("manifest.json").is_file();
    let explicit_entry = control
        .config
        .ssr
        .entry
        .as_deref()
        .is_some_and(|entry| entry != "src/server.rs");
    let renderer = if has_server || explicit_entry {
        if control.config.runtime.backend != "napi-vm" || !cfg!(feature = "napi-vm") {
            return Err(FerriteError::Ssr(format!("built SSR preview requires the explicitly selected napi-vm runtime and its compiled feature; selected `{}`. Configure [runtime].backend = 'napi-vm' and build Ferrite with --features napi-vm", control.config.runtime.backend)));
        }
        let graph = crate::load_built_ssr_graph(&server_dir)?;
        let shell = std::fs::read_to_string(dir.join("index.html"))?;
        let adapter = ferrite_ssr::JsSsrAdapter::from_resolved_graph(&control.config, graph)?
            .with_shell(shell);
        let adapter = Arc::new(tokio::sync::Mutex::new(adapter));
        Some(
            Arc::new(ferrite_ssr::FnAdapter::new(move |request, context| {
                let adapter = adapter.clone();
                async move {
                    let adapter = adapter.lock().await;
                    ferrite_ssr::SsrAdapter::render(&*adapter, request, context).await
                }
            })) as Arc<dyn ferrite_ssr::SsrAdapter>,
        )
    } else {
        None
    };
    preview_with_parts(
        dir,
        port,
        PreviewRendering {
            base: &control.config.base,
            renderer,
        },
        &control.headers,
        &control.mounts,
        &control.proxies,
    )
    .await
}

/// Shared preview server: proxies → mounts → static + SPA fallback.
async fn preview_with_parts(
    dir: &std::path::Path,
    port: u16,
    application: PreviewRendering<'_>,
    headers: &[(String, String)],
    mounts: &[ferrite_plugin::PreviewMount],
    proxies: &[ferrite_plugin::ProxyRule],
) -> Result<()> {
    let PreviewRendering { base, renderer } = application;
    use axum::extract::State;
    use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri};
    use axum::response::IntoResponse as _;
    #[derive(Clone)]
    struct PreviewState {
        dir: PathBuf,
        base: String,
        headers: Vec<(HeaderName, HeaderValue)>,
        mounts: Vec<ferrite_plugin::PreviewMount>,
        proxies: Vec<ferrite_plugin::ProxyRule>,
        http_client: reqwest::Client,
        renderer: Option<Arc<dyn ferrite_ssr::SsrAdapter>>,
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
                let mut map = HeaderMap::new();
                map.insert(
                    axum::http::header::CONTENT_TYPE,
                    content_type
                        .parse()
                        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
                );
                map.insert(
                    axum::http::header::CACHE_CONTROL,
                    // Preview output and plugin mounts may change in place
                    // between builds. Filenames alone cannot prove immutability.
                    HeaderValue::from_static("no-cache"),
                );
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
        if state.renderer.is_none() && method != Method::GET && method != Method::HEAD {
            return (
                StatusCode::METHOD_NOT_ALLOWED,
                [(axum::http::header::ALLOW, "GET, HEAD")],
                "static preview supports GET and HEAD",
            )
                .into_response();
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
                if method != Method::GET && method != Method::HEAD {
                    return (
                        StatusCode::METHOD_NOT_ALLOWED,
                        [(axum::http::header::ALLOW, "GET, HEAD")],
                        "mounted assets support GET and HEAD",
                    )
                        .into_response();
                }
                let file = mount.dir.join(relative);
                if file.is_file() {
                    return file_response(&file, &state.headers);
                }
                return (StatusCode::NOT_FOUND, "mounted file not found").into_response();
            }
        }
        // 3. Built URLs are rooted at the configured base, while plugin
        // mounts and proxies above keep their explicitly configured prefixes.
        if state.base != "/" && (path == "/" || path == state.base.trim_end_matches('/')) {
            let location = match uri.query() {
                Some(query) => format!("{}?{query}", state.base),
                None => state.base.clone(),
            };
            return (
                StatusCode::TEMPORARY_REDIRECT,
                [(axum::http::header::LOCATION, location)],
            )
                .into_response();
        }
        let Some(relative) = path.strip_prefix(&state.base) else {
            return (
                StatusCode::NOT_FOUND,
                "request is outside the configured base",
            )
                .into_response();
        };
        if state.renderer.is_some() && (relative == "server" || relative.starts_with("server/")) {
            return (StatusCode::NOT_FOUND, "server output is private").into_response();
        }
        if std::path::Path::new(relative).extension().is_none() {
            if let Some(renderer) = &state.renderer {
                let request_headers: Vec<_> = headers
                    .iter()
                    .filter_map(|(name, value)| {
                        value
                            .to_str()
                            .ok()
                            .map(|value| (name.to_string(), value.to_string()))
                    })
                    .collect();
                let request = ferrite_ssr::SsrHttpRequest {
                    method: method.to_string(),
                    uri: url.clone(),
                    headers: request_headers.clone(),
                    body: body.to_vec(),
                };
                let context = ferrite_ssr::SsrContext {
                    url,
                    headers: request_headers.into_iter().collect(),
                    ..Default::default()
                };
                return match renderer.render(request, context).await {
                    Ok(response) => {
                        ferrite_server::ssr_http_response(response, method == Method::HEAD)
                    }
                    Err(error) => {
                        (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
                    }
                };
            }
        }
        if method != Method::GET && method != Method::HEAD {
            return (
                StatusCode::METHOD_NOT_ALLOWED,
                [(axum::http::header::ALLOW, "GET, HEAD")],
                "assets support GET and HEAD",
            )
                .into_response();
        }
        let file = state.dir.join(relative);
        let file = if file.is_file() {
            file
        } else {
            let accepts_html = headers
                .get(axum::http::header::ACCEPT)
                .and_then(|value| value.to_str().ok())
                .is_none_or(|value| value.contains("text/html") || value.contains("*/*"));
            if std::path::Path::new(&path).extension().is_some() || !accepts_html {
                return (StatusCode::NOT_FOUND, "file not found").into_response();
            }
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
    let base = if base.is_empty() || matches!(base, "." | "./") {
        "/".to_string()
    } else if base.starts_with("http://") || base.starts_with("https://") || base.starts_with("//")
    {
        let url = if base.starts_with("//") {
            format!("https:{base}")
        } else {
            base.to_string()
        };
        reqwest::Url::parse(&url)
            .map_err(|error| {
                FerriteError::Config(format!("invalid preview base `{base}`: {error}"))
            })?
            .path()
            .to_string()
    } else {
        base.to_string()
    };
    if !base.starts_with('/')
        || base.split('/').any(|part| matches!(part, "." | ".."))
        || base.contains(['?', '#'])
    {
        return Err(FerriteError::Config(format!("preview base `{base}` must be an absolute URL path without traversal, query, or fragment")));
    }
    let base = with_trailing_slash(&base);
    let router = axum::Router::new()
        .fallback(handler)
        .with_state(PreviewState {
            dir: dir.to_path_buf(),
            base,
            headers: parsed_headers(headers),
            mounts: mounts.to_vec(),
            proxies: proxies.to_vec(),
            renderer,
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

#[cfg(test)]
mod ssr_preview_validation_tests {
    #[tokio::test]
    async fn explicit_ssr_preview_rejects_unavailable_runtime_before_listening() {
        let root = tempfile::tempdir().unwrap();
        let mut config = ferrite_config::resolve_config(
            Default::default(),
            Some(root.path().into()),
            Default::default(),
        )
        .unwrap();
        config.ssr.entry = Some("src/entry-server.ts".into());
        config.runtime.backend = "quickjs".into();
        let error = super::preview_with_plugins(&config, &[]).await.unwrap_err();
        assert!(
            error.to_string().contains("quickjs") && error.to_string().contains("napi-vm"),
            "{error}"
        );
    }
}
