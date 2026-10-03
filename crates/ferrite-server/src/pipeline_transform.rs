//! Dev server core transforms.

//! Dev server module transform pipeline.

use crate::util::*;
use crate::DevServer;
use crate::PipelineModule;
use ferrite_assets::asset_to_js;
use ferrite_assets::raw_to_js;
use ferrite_assets::to_data_url;
use ferrite_core::ModuleId;
use ferrite_core::ModuleType;
use ferrite_core::Result;
use ferrite_graph::ImportKind;
use ferrite_plugin::PluginContext;
use ferrite_resolver::ResolveKind;
use ferrite_resolver::ResolveRequest;
use ferrite_transform::rewrite_specifiers_mapped;
use ferrite_transform::TransformRequest;
use std::collections::HashMap;
use std::path::PathBuf;

impl DevServer {
    /// Package boundaries are filesystem identities, never browser/virtual URLs.
    pub(crate) fn jsx_manifest_candidates(&self, id: &ModuleId) -> Vec<String> {
        let Ok(file) = self.id_to_file(id) else {
            return Vec::new();
        };
        let mut candidates = Vec::new();
        let Some(parent) = file.parent() else {
            return candidates;
        };
        for directory in parent.ancestors() {
            let manifest = directory.join("package.json");
            candidates.push(manifest.to_string_lossy().into_owned());
            if manifest.exists() || directory == self.inner.config.root {
                break;
            }
        }
        candidates
    }

    fn validate_inferred_jsx_owner(&self, id: &ModuleId) -> Result<()> {
        // Explicit framework or JSX runtime selection takes precedence over detection.
        if self.inner.config.framework.is_some()
            || self.inner.config.react.import_source.is_some()
            || self.inner.config.react.factory.is_some()
        {
            return Ok(());
        }
        for candidate in self.jsx_manifest_candidates(id) {
            let contents = match std::fs::read_to_string(&candidate) {
                Ok(contents) => contents,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    return Err(ferrite_core::FerriteError::Config(format!(
                        "cannot inspect JSX owner in {candidate}: {error}"
                    )))
                }
            };
            let manifest: serde_json::Value = serde_json::from_str(&contents).map_err(|error| {
                ferrite_core::FerriteError::Config(format!(
                    "cannot inspect JSX owner in {candidate}: {error}"
                ))
            })?;
            let owners = ferrite_frameworks::registry::jsx_owners(&manifest);
            if owners.len() > 1 || owners.first().is_some_and(|owner| *owner != "react") {
                return Err(ferrite_core::FerriteError::Transform {
                    id: id.0.clone(),
                    message: format!("JSX ownership in {candidate} is {} ({}); select an explicit framework/compiler profile or provide a JSX-lowering plugin. Native Preact, Solid and Qwik framework adapters are unavailable", if owners.len() > 1 { "ambiguous" } else { "unavailable" }, owners.join(", ")),
                });
            }
            break;
        }
        Ok(())
    }

    /// Core transform dispatch by module type.
    pub(crate) async fn core_transform(
        &self,
        _ctx: &PluginContext<'_>,
        id: &ModuleId,
        source: &str,
        module_type: &ModuleType,
        environment: &ferrite_core::Environment,
    ) -> Result<PipelineModule> {
        match module_type {
            &ModuleType::Css => {
                let (path_part, query) = id.split_query();
                let direct = query.is_some_and(|q| q.contains("direct"));
                let css_id = path_part.to_string();
                let result = ferrite_css::transform_css(
                    &css_id,
                    source,
                    &ferrite_css::CssOptions {
                        modules: css_id.contains(".module.css"),
                        minify: false,
                        dev: true,
                    },
                );
                // Rewrite @import/url() relative to the CSS file.
                let mut mapping: HashMap<String, String> = HashMap::new();
                for import in &result.imports {
                    if let Ok(resolved) =
                        self.resolve_relative(&css_id, &import.specifier, environment)
                    {
                        mapping.insert(import.specifier.clone(), resolved);
                    }
                }
                for url in &result.urls {
                    if url.url.starts_with("./")
                        || url.url.starts_with("../")
                        || url.url.starts_with('/')
                    {
                        if let Ok(resolved) = self.resolve_relative(&css_id, &url.url, environment)
                        {
                            mapping.insert(url.url.clone(), resolved);
                        }
                    }
                }
                let code =
                    ferrite_css::rewrite_css_refs(&result.code, |spec| mapping.get(spec).cloned());
                if direct {
                    return Ok(PipelineModule {
                        id: id.clone(),
                        code,
                        imports: Vec::new(),
                        side_effects: None,
                        module_type: ModuleType::Css,
                        map: None,
                        has_module_syntax: false,
                        uses_import_meta_hot: false,
                        is_raw_bytes: false,
                        dependencies: Vec::new(),
                        shake: Some(ferrite_transform::ShakeInfo::default()),
                        commonjs: None,
                    });
                }
                // Production builds emit self-contained JS (no /@ferrite/client
                // import); file extraction is the bundler roadmap (§29).
                let production = self.inner.config.is_production;
                let js = if production {
                    production_css_js(&id.0, &code, &result.exports)
                } else {
                    ferrite_css::css_to_js(&id.0, &code, &result.exports)
                };
                let mut module = PipelineModule::code_only(id.clone(), js, ModuleType::Js);
                if !production {
                    module.imports = vec![(
                        "/@ferrite/client".to_string(),
                        ModuleId::new("/@ferrite/client"),
                        ImportKind::Static,
                    )];
                }
                Ok(module)
            }
            _ if module_type.is_js_like() || *module_type == ModuleType::Json => {
                if matches!(module_type, ModuleType::Jsx | ModuleType::Tsx) {
                    self.validate_inferred_jsx_owner(id)?;
                }
                if matches!(module_type, ModuleType::Jsx | ModuleType::Tsx)
                    && self
                        .inner
                        .config
                        .framework
                        .as_ref()
                        .is_some_and(|framework| {
                            !framework.enabled.iter().any(|owner| owner == "react")
                        })
                {
                    return Err(ferrite_core::FerriteError::Transform {
                        id: id.0.clone(),
                        message: "JSX has no enabled framework owner: explicit framework.enabled excludes React; select enabled = [\"react\"] with compiler_host = \"native\", or have a framework plugin lower this module to JavaScript before core lowering. Other JSX framework adapters are unavailable".into(),
                    });
                }
                let define = self.transform_defines(environment);
                let result = self.inner.compiler.transform(TransformRequest {
                    id: id.0.clone(),
                    code: source.to_string(),
                    module_type: module_type.clone(),
                    environment: environment.kind.clone(),
                    ssr: environment.kind.is_ssr(),
                    target: environment.target.clone(),
                    minify: false,
                    sourcemap: self.inner.config.build.sourcemap.enabled(),
                    define,
                    jsx_runtime: self.inner.config.react.runtime.clone(),
                    jsx_import_source: self.inner.config.react.import_source.clone(),
                    jsx_factory: self.inner.config.react.factory.clone(),
                    jsx_fragment: self.inner.config.react.fragment.clone(),
                    development: !self.inner.config.is_production,
                })?;
                let parsed = self.inner.compiler.parse(ferrite_transform::ParseRequest {
                    id: id.0.clone(),
                    code: result.code.clone(),
                    module_type: ModuleType::Js,
                })?;
                Ok(PipelineModule {
                    id: id.clone(),
                    code: result.code,
                    imports: Vec::new(), // filled during rewriting
                    side_effects: None,
                    module_type: ModuleType::Js,
                    map: result.map.map(|map| map.mappings),
                    has_module_syntax: parsed.has_module_syntax,
                    uses_import_meta_hot: parsed.uses_import_meta_hot,
                    is_raw_bytes: false,
                    dependencies: Vec::new(),
                    shake: None, // filled during rewriting
                    commonjs: None,
                })
            }
            _ => Ok(PipelineModule::code_only(
                id.clone(),
                source.to_string(),
                module_type.clone(),
            )),
        }
    }

    /// Rewrite a module's imports to dev URLs.
    pub(crate) async fn rewrite_module_imports(
        &self,
        ctx: &PluginContext<'_>,
        mut module: PipelineModule,
        environment: &ferrite_core::Environment,
    ) -> Result<PipelineModule> {
        let parsed = self.inner.compiler.parse(ferrite_transform::ParseRequest {
            id: module.id.0.clone(),
            code: module.code.clone(),
            module_type: ModuleType::Js,
        })?;
        // `import-map` is client-only: SSR has no browser to resolve maps.
        let use_import_map =
            self.inner.config.npm.dev_strategy == "import-map" && !environment.kind.is_ssr();
        let mut mapping: HashMap<String, String> = HashMap::new();
        let mut imports = Vec::new();
        let mut import_bindings = Vec::new();
        for import in &parsed.imports {
            if import.is_type {
                continue;
            }
            let dynamic = import.kind == ferrite_transform::ParsedImportKind::Dynamic;
            // Dynamic imports consult `resolveDynamicImport` first.
            let hooked = if dynamic {
                self.inner
                    .plugins
                    .hook_resolve_dynamic_import(
                        ctx,
                        ferrite_plugin::DynamicImportRequest {
                            specifier: import.specifier.clone(),
                            importer: module.id.clone(),
                            environment: environment.kind.clone(),
                        },
                    )
                    .await?
            } else {
                None
            };
            let resolved = match hooked {
                Some(resolved) => Ok(resolved),
                None => {
                    self.resolve_id(ctx, &import.specifier, Some(&module.id), environment)
                        .await
                }
            };
            match resolved {
                Ok(resolved) => {
                    let mut url = resolved.id.0.clone();
                    if url.starts_with('\0') {
                        url = virtual_url(&url);
                    } else if should_shim_asset(&url)
                        && resolved.module_type.as_ref().is_none_or(|kind| {
                            matches!(
                                kind,
                                ModuleType::Asset | ModuleType::Wasm | ModuleType::Data
                            )
                        })
                    {
                        url = format!("{url}?asset-shim");
                    }
                    let kind = match import.kind {
                        ferrite_transform::ParsedImportKind::Static => ImportKind::Static,
                        ferrite_transform::ParsedImportKind::Dynamic => ImportKind::Dynamic,
                    };
                    let timestamp = if !environment.kind.is_ssr()
                        && !self.inner.config.is_production
                        && !resolved.external
                    {
                        self.inner
                            .graph
                            .get(&ModuleId::new(&url))
                            .and_then(|node| node.last_invalidated)
                    } else {
                        None
                    };
                    let browser_url = timestamp.map_or_else(
                        || url.clone(),
                        |timestamp| {
                            format!(
                                "{url}{}t={timestamp}",
                                if url.contains('?') { "&" } else { "?" }
                            )
                        },
                    );
                    // Bare specifiers stay for the browser; record the URL
                    // for the inline map instead of rewriting.
                    if use_import_map
                        && timestamp.is_none()
                        && !resolved.external
                        && is_bare_specifier(&import.specifier)
                    {
                        if let Ok(mut map) = self.inner.import_map.lock() {
                            map.insert(import.specifier.clone(), url.clone());
                        }
                        imports.push((import.specifier.clone(), ModuleId::new(url), kind));
                        import_bindings.push(import.bindings.clone());
                        continue;
                    }
                    mapping.insert(import.specifier.clone(), browser_url);
                    imports.push((import.specifier.clone(), ModuleId::new(url), kind));
                    import_bindings.push(import.bindings.clone());
                }
                Err(error) => {
                    if !environment.kind.is_ssr() && !self.inner.config.is_production {
                        let mut candidates = self
                            .inner
                            .client_resolver
                            .unresolved_file_candidates(&import.specifier, &module.id);
                        if candidates.is_empty() && is_bare_specifier(&import.specifier) {
                            candidates.push(self.inner.config.lockfile());
                        }
                        self.track_missing_import(&module.id, &import.specifier, candidates)?;
                    }
                    return Err(ferrite_core::FerriteError::Resolve(format!(
                        "cannot resolve `{}` from `{}`: {error}",
                        import.specifier, module.id.0
                    )));
                }
            }
        }
        if !mapping.is_empty() {
            let (code, _, map) = rewrite_specifiers_mapped(
                &module.id.0,
                &module.code,
                &ModuleType::Js,
                &mapping,
                module.map.is_some(),
            )?;
            module.map = crate::loader::merge_maps(map, module.map, code == module.code)?;
            module.code = code;
        }
        // Acceptance literals are executable HMR protocol URLs, even when
        // ordinary bare imports use an import map. Resolve them after plugins.
        let mut edits = Vec::new();
        for (specifier, (start, end)) in
            ferrite_transform::hmr_dependencies(&module.id.0, &module.code)?
        {
            let resolved = self
                .resolve_id(ctx, &specifier, Some(&module.id), environment)
                .await
                .map_err(|error| {
                    ferrite_core::FerriteError::Resolve(format!(
                        "cannot resolve HMR dependency `{specifier}` from `{}`: {error}",
                        module.id.0
                    ))
                })?;
            if resolved.external {
                return Err(ferrite_core::FerriteError::Config(format!(
                    "cannot hot-accept external dependency `{specifier}` from `{}`",
                    module.id.0
                )));
            }
            let url = if resolved.id.0.starts_with('\0') {
                virtual_url(&resolved.id.0)
            } else {
                resolved.id.0.clone()
            };
            edits.push((start, end, serde_json::to_string(&url)?));
            if !imports.iter().any(|(_, id, _)| id.0 == url) {
                imports.push((specifier, ModuleId::new(url), ImportKind::Dynamic));
                import_bindings.push(Vec::new());
            }
        }
        if !edits.is_empty() {
            let (code, map) = ferrite_transform::apply_text_edits(
                &module.id.0,
                &module.code,
                &edits,
                module.map.is_some(),
            )?;
            module.map = crate::loader::merge_maps(map, module.map, code == module.code)?;
            module.code = code;
        }
        // Resolve re-export sources to module ids (specifier matching would
        // not survive the resolved-URL rewriting below; `mapping` misses
        // the import-map path, so key off the recorded imports instead).
        let mut exports = parsed.export_details.clone();
        for export in &mut exports {
            if let Some(from) = export.from.as_deref() {
                if let Some((_, id, _)) = imports.iter().find(|(spec, _, _)| spec == from) {
                    export.target = Some(id.clone());
                }
            }
        }
        module.has_module_syntax = parsed.has_module_syntax;
        module.uses_import_meta_hot = parsed.uses_import_meta_hot;
        module.imports = imports;
        module.shake = Some(ferrite_transform::ShakeInfo {
            import_bindings,
            exports,
        });
        Ok(module)
    }

    /// Apply shared CommonJS interop to already-lowered plugin output.
    /// Library callers reuse this service without running compiler hooks twice.
    pub async fn interop_commonjs_module(
        &self,
        mut module: PipelineModule,
        environment_kind: ferrite_core::EnvironmentKind,
    ) -> Result<PipelineModule> {
        let environment = if environment_kind.is_ssr() {
            self.inner.config.ssr_env()
        } else {
            self.inner.config.client_env()
        };
        let ctx = self.plugin_context(&environment);
        if module.id.split_query().0.ends_with(".json") {
            let (code, map) = ferrite_transform::commonjs_json_factory(
                &module.id.0,
                &module.code,
                self.inner.config.build.sourcemap.enabled(),
            )?;
            module.map = crate::loader::merge_maps(map, module.map, false)?;
            module.code = code;
            module.commonjs = Some(Default::default());
        } else {
            module = self.convert_cjs(&ctx, module, &environment).await?;
        }
        self.rewrite_module_imports(&ctx, module, &environment)
            .await
    }

    /// Convert CJS to an ESM wrapper (§18).
    pub(crate) async fn convert_cjs(
        &self,
        ctx: &PluginContext<'_>,
        module: PipelineModule,
        environment: &ferrite_core::Environment,
    ) -> Result<PipelineModule> {
        self.convert_cjs_output(ctx, module, environment, false)
            .await
    }

    pub(crate) async fn convert_cjs_output(
        &self,
        ctx: &PluginContext<'_>,
        module: PipelineModule,
        environment: &ferrite_core::Environment,
        inline: bool,
    ) -> Result<PipelineModule> {
        use ferrite_transform::{
            analyze_commonjs, commonjs_facade, commonjs_factory, commonjs_factory_id,
            validate_commonjs, CJS_FACTORY_QUERY,
        };
        use std::collections::{BTreeMap, BTreeSet};
        let factory = module
            .id
            .split_query()
            .1
            .is_some_and(|query| query.split('&').any(|part| part == CJS_FACTORY_QUERY));
        let analysis = analyze_commonjs(&module.id.0, &module.code)?;
        validate_commonjs(&module.id.0, &module.code, &analysis)?;
        if self
            .inner
            .compiler
            .parse(ferrite_transform::ParseRequest {
                id: module.id.0.clone(),
                code: module.code.clone(),
                module_type: ModuleType::Js,
            })?
            .has_module_syntax
        {
            return Err(ferrite_core::FerriteError::Transform {
                id: module.id.0.clone(),
                message:
                    "mixed ESM/CommonJS or synchronous require(ESM) is unavailable; use ESM imports"
                        .into(),
            });
        }
        let mut targets = BTreeMap::new();
        let mut metadata = crate::types::CommonJsMetadata {
            names: analysis.named_exports,
            reexports: Vec::new(),
        };
        for specifier in &analysis.requires {
            let resolved = self
                .resolve_id_with_kind(
                    ctx,
                    specifier,
                    Some(&module.id),
                    environment,
                    ResolveKind::Require,
                )
                .await
                .map_err(|error| {
                    ferrite_core::FerriteError::Resolve(format!(
                        "cannot resolve require({specifier:?}) from {}: {error}",
                        module.id.0
                    ))
                })?;
            if resolved.external {
                return Err(ferrite_core::FerriteError::Resolve(format!("require({specifier:?}) in {} resolves to an external module; this pipeline needs a synchronous factory, not an unprovided Node builtin", module.id.0)));
            }
            let target = if resolved.id.0.starts_with('\0') {
                virtual_url(&resolved.id.0)
            } else {
                resolved.id.0
            };
            if target
                .split('?')
                .next()
                .is_some_and(|path| path.ends_with(".node"))
            {
                return Err(ferrite_core::FerriteError::Resolve(format!("native addon {target} is unavailable in the browser/embedded pipeline; use an explicitly configured Node runtime")));
            }
            targets.insert(specifier.clone(), commonjs_factory_id(&target));
            if analysis.reexports.contains(specifier) {
                metadata.reexports.push(target);
            }
        }
        let sourcemap = self.inner.config.build.sourcemap.enabled();
        let (code, map) = if factory {
            commonjs_factory(&module.id.0, &module.code, &targets, sourcemap)?
        } else {
            let mut names = metadata.names.clone();
            let mut dependencies = module.dependencies.clone();
            let mut visited = BTreeSet::new();
            for target in &metadata.reexports {
                self.collect_commonjs_exports(
                    target,
                    environment,
                    &mut visited,
                    &mut names,
                    &mut dependencies,
                )
                .await?;
            }
            let (code, map) = if inline {
                ferrite_transform::commonjs_inline(
                    &module.id.0,
                    &module.code,
                    &names,
                    &targets,
                    sourcemap,
                )?
            } else {
                commonjs_facade(&module.id.0, &module.code, &names, sourcemap)?
            };
            let map = crate::loader::merge_maps(map, module.map, false)?;
            return Ok(PipelineModule {
                code,
                map,
                has_module_syntax: true,
                commonjs: Some(metadata),
                dependencies,
                ..module
            });
        };
        let map = crate::loader::merge_maps(map, module.map, false)?;
        Ok(PipelineModule {
            code,
            map,
            has_module_syntax: true,
            commonjs: Some(metadata),
            side_effects: Some(false),
            ..module
        })
    }

    fn collect_commonjs_exports<'a>(
        &'a self,
        target: &'a str,
        environment: &'a ferrite_core::Environment,
        visited: &'a mut std::collections::BTreeSet<String>,
        names: &'a mut std::collections::BTreeSet<String>,
        dependencies: &'a mut Vec<String>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            if !visited.insert(target.into()) {
                return Ok(());
            }
            let factory = ModuleId::new(ferrite_transform::commonjs_factory_id(target));
            let module = self
                .pipeline_module(
                    &factory,
                    None,
                    if environment.kind.is_ssr() {
                        "ssr"
                    } else {
                        "client"
                    },
                )
                .await?;
            if let Ok(file) = self.id_to_file(&factory) {
                self.watch_extra(&file);
                dependencies.push(file.to_string_lossy().into_owned());
            }
            dependencies.extend(module.dependencies);
            let metadata =
                module
                    .commonjs
                    .ok_or_else(|| {
                        ferrite_core::FerriteError::Transform {
                    id: target.into(),
                    message:
                        "CommonJS re-export target is not a synchronous factory; use ESM re-exports"
                            .into(),
                }
                    })?;
            names.extend(metadata.names);
            for target in &metadata.reexports {
                self.collect_commonjs_exports(target, environment, visited, names, dependencies)
                    .await?;
            }
            Ok(())
        })
    }

    /// Asset query shims (`?raw`, `?url`, `?inline`, `?worker`, `?wasm`).
    pub(crate) async fn asset_query_shim(
        &self,
        ctx: &PluginContext<'_>,
        id: &ModuleId,
        path_part: &str,
        query: &str,
        source: &str,
    ) -> Result<Option<PipelineModule>> {
        let kind = query.split('&').next().unwrap_or("");
        match kind {
            "raw" => Ok(Some(PipelineModule::code_only(
                id.clone(),
                raw_to_js(source),
                ModuleType::Js,
            ))),
            "url" => {
                // `resolveFileUrl` may rewrite the public URL.
                let url = self
                    .inner
                    .plugins
                    .hook_resolve_file_url(
                        ctx,
                        ferrite_plugin::ResolveFileUrlRequest {
                            file_name: path_part.to_string(),
                        },
                    )
                    .await?
                    .unwrap_or_else(|| path_part.to_string());
                Ok(Some(PipelineModule::code_only(
                    id.clone(),
                    asset_to_js(&url),
                    ModuleType::Js,
                )))
            }
            "inline" => {
                let file = self
                    .id_to_file(id)
                    .unwrap_or_else(|_| PathBuf::from(path_part));
                let bytes = std::fs::read(&file).unwrap_or_default();
                Ok(Some(PipelineModule::code_only(
                    id.clone(),
                    asset_to_js(&to_data_url(&bytes, path_part)),
                    ModuleType::Js,
                )))
            }
            "worker" => {
                let js = format!(
                    "export default class FerriteWorker extends Worker {{\n  constructor(options) {{\n    super({path_part:?}, {{ type: \"module\", ...options }});\n  }}\n}}\n"
                );
                Ok(Some(PipelineModule::code_only(
                    id.clone(),
                    js,
                    ModuleType::Js,
                )))
            }
            "wasm" => {
                let js = format!(
                    "export default async function init(input) {{\n  const source = input ?? {path_part:?};\n  const bytes = await fetch(source).then((r) => r.arrayBuffer());\n  const {{ instance }} = await WebAssembly.instantiate(bytes);\n  return instance.exports;\n}}\n"
                );
                Ok(Some(PipelineModule::code_only(
                    id.clone(),
                    js,
                    ModuleType::Js,
                )))
            }
            _ => Ok(None),
        }
    }

    /// Resolve a path relative to a module (CSS refs).
    pub(crate) fn resolve_relative(
        &self,
        from_id: &str,
        specifier: &str,
        environment: &ferrite_core::Environment,
    ) -> Result<String> {
        let importer = ModuleId::new(from_id);
        let resolver = if environment.kind.is_ssr() {
            &self.inner.ssr_resolver
        } else {
            &self.inner.client_resolver
        };
        Ok(resolver
            .resolve(&ResolveRequest {
                specifier,
                importer: Some(&importer),
                environment: environment.kind.clone(),
                kind: ResolveKind::Css,
            })?
            .id
            .0)
    }

    /// Rewrite `<script type="module">` + `<link>` sources in HTML.
    pub(crate) async fn rewrite_html_scripts(&self, html: &str) -> String {
        let environment = self.inner.config.client_env();
        let ctx = self.plugin_context(&environment);
        let mut mapping: HashMap<String, String> = HashMap::new();
        for entry in ferrite_html::discover_entries(html) {
            let spec = entry.src.clone();
            if spec.starts_with("/@") || spec.starts_with("http") {
                continue;
            }
            if let Ok(resolved) = self.resolve_id(&ctx, &spec, None, &environment).await {
                mapping.insert(spec, resolved.id.0);
            }
        }
        let rewritten = ferrite_html::rewrite_module_scripts(html, |src| {
            mapping.get(src).cloned().unwrap_or_else(|| src.to_string())
        });
        // Stylesheet links get `?direct` so the pipeline serves CSS, not the JS wrapper.
        rewrite_link_direct(&rewritten)
    }
}

impl DevServer {
    fn track_missing_import(
        &self,
        importer: &ModuleId,
        specifier: &str,
        candidates: Vec<PathBuf>,
    ) -> Result<()> {
        let mut missing = self.inner.missing_imports.lock().map_err(|_| {
            ferrite_core::FerriteError::Other("missing-import watch lock poisoned".into())
        })?;
        for file in candidates {
            let id = ModuleId::new(ferrite_core::file_to_url(&self.inner.config.root, &file));
            self.inner.graph.ensure(&id, ModuleType::from_path(&file));
            self.inner.graph.add_edge(
                importer,
                ferrite_graph::ImportEdge {
                    specifier: specifier.into(),
                    resolved: id.clone(),
                    kind: ImportKind::Static,
                },
            );
            let owned = missing.entry(importer.clone()).or_default();
            if !owned.contains(&id) {
                owned.push(id);
            }
        }
        Ok(())
    }
}
