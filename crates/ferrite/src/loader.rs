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
    fn emit_bytes(&self, file_name: &str, bytes: Vec<u8>) -> String {
        let hashed = ferrite_assets::hashed_name(file_name, &bytes);
        let name = format!("assets/{hashed}");
        let url = format!("{}{name}", self.base);
        if let Ok(mut assets) = self.assets.lock() {
            assets.insert(name, bytes);
        }
        url
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
        let env = self.server.inner().config.client_env();
        let mut url_mapping: HashMap<String, String> = HashMap::new();
        for found in &result.urls {
            let spec = &found.url;
            if !spec.starts_with("./") && !spec.starts_with("../") {
                continue;
            }
            match self.resolve_css_ref(id, spec, &env) {
                Ok(dep) => match self.server.id_to_file(&dep).and_then(|file| {
                    let bytes = std::fs::read(&file)?;
                    let name = file
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "asset".to_string());
                    Ok((name, bytes))
                }) {
                    Ok((name, bytes)) => {
                        url_mapping.insert(spec.clone(), self.emit_bytes(&name, bytes));
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
            match self.resolve_css_ref(id, spec, &env) {
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
        let module = self.server.pipeline_module(id, None, env).await?;
        // Raw assets become hashed files + URL shims.
        if module.is_raw_bytes {
            let file = self.server.id_to_file(id)?;
            let bytes = std::fs::read(&file)?;
            let name = file
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "asset".to_string());
            let url = self.emit_bytes(&name, bytes);
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
                        let url = self.emit_bytes(&name, bytes);
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
        let shake = module.shake.as_ref().and_then(|shake| {
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

// --- preview -------------------------------------------------------------------

/// Serve `dir` statically with SPA fallback (`preview`, §83 light).
pub async fn preview_dir(dir: &std::path::Path, port: u16) -> Result<()> {
    use axum::extract::State;
    use axum::http::{StatusCode, Uri};
    use axum::response::IntoResponse as _;
    #[derive(Clone)]
    struct PreviewState {
        dir: PathBuf,
    }
    async fn handler(
        State(state): State<PreviewState>,
        uri: Uri,
    ) -> impl axum::response::IntoResponse {
        let path = uri.path().trim_start_matches('/');
        let file = state.dir.join(path);
        let file = if file.is_file() {
            file
        } else {
            state.dir.join("index.html")
        };
        match std::fs::read(&file) {
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
                (
                    StatusCode::OK,
                    [
                        (axum::http::header::CONTENT_TYPE, content_type),
                        (axum::http::header::CACHE_CONTROL, cache.to_string()),
                    ],
                    bytes,
                )
                    .into_response()
            }
            Err(_) => (StatusCode::NOT_FOUND, "not found").into_response(),
        }
    }
    if !dir.exists() {
        return Err(FerriteError::Build(format!(
            "`{}` does not exist; run `ferrite build` first",
            dir.display()
        )));
    }
    let router = axum::Router::new()
        .fallback(handler)
        .with_state(PreviewState {
            dir: dir.to_path_buf(),
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
