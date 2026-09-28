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
use ferrite_transform::rewrite_specifiers;
use ferrite_transform::TransformRequest;
use std::collections::HashMap;
use std::path::PathBuf;

impl DevServer {
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
                        shake: Some(ferrite_transform::ShakeInfo::default()),
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
                    shake: None, // filled during rewriting
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
            let dynamic =
                import.kind == ferrite_transform::ParsedImportKind::Dynamic;
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
                    } else if should_shim_asset(&url) {
                        url = format!("{url}?asset-shim");
                    }
                    let kind = match import.kind {
                        ferrite_transform::ParsedImportKind::Static => ImportKind::Static,
                        ferrite_transform::ParsedImportKind::Dynamic => ImportKind::Dynamic,
                    };
                    // Bare specifiers stay for the browser; record the URL
                    // for the inline map instead of rewriting.
                    if use_import_map && !resolved.external && is_bare_specifier(&import.specifier)
                    {
                        if let Ok(mut map) = self.inner.import_map.lock() {
                            map.insert(import.specifier.clone(), url.clone());
                        }
                        imports.push((import.specifier.clone(), ModuleId::new(url), kind));
                        import_bindings.push(import.bindings.clone());
                        continue;
                    }
                    mapping.insert(import.specifier.clone(), url.clone());
                    imports.push((import.specifier.clone(), ModuleId::new(url), kind));
                    import_bindings.push(import.bindings.clone());
                }
                Err(error) => {
                    // Bare specifiers can never survive to the browser (no
                    // import map entry exists for a failed resolve), so a
                    // failure here is always a broken module: fail loudly
                    // with the resolver's actionable hint (`ferrite add …`)
                    // instead of serving it. Non-bare failures (absolute
                    // URLs the browser fetches itself, missing files that
                    // 404) keep warn-and-passthrough.
                    if is_bare_specifier(&import.specifier) {
                        return Err(ferrite_core::FerriteError::Resolve(format!(
                            "cannot resolve `{}` from `{}`: {error}",
                            import.specifier, module.id.0
                        )));
                    }
                    tracing::warn!(
                        "cannot resolve `{}` from `{}`: {error}",
                        import.specifier,
                        module.id.0
                    );
                }
            }
        }
        if !mapping.is_empty() {
            let (code, _) = rewrite_specifiers(&module.code, &ModuleType::Js, &mapping)?;
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
        module.imports = imports;
        module.shake = Some(ferrite_transform::ShakeInfo {
            import_bindings,
            exports,
        });
        Ok(module)
    }

    /// Convert CJS to an ESM wrapper (§18).
    pub(crate) async fn convert_cjs(
        &self,
        ctx: &PluginContext<'_>,
        module: PipelineModule,
        environment: &ferrite_core::Environment,
    ) -> Result<PipelineModule> {
        let requires = collect_requires(&module.code);
        let mut mapping: HashMap<String, String> = HashMap::new();
        let mut imports = module.imports.clone();
        let mut shake = module.shake.clone();
        for specifier in &requires {
            if let Ok(resolved) = self
                .resolve_id(ctx, specifier, Some(&module.id), environment)
                .await
            {
                mapping.insert(specifier.clone(), resolved.id.0.clone());
                imports.push((specifier.clone(), resolved.id, ImportKind::Static));
                // The interop reads `default ?? whole`: namespace use.
                if let Some(shake) = shake.as_mut() {
                    shake
                        .import_bindings
                        .push(vec![ferrite_transform::ImportBinding::Namespace]);
                }
            }
        }
        let mut prelude = String::from(
            "const __ferrite_interop__ = (m) => (m && m.__esModule ? m.default : (m?.default ?? m));\n",
        );
        let mut code = module.code.clone();
        for (index, specifier) in requires.iter().enumerate() {
            if let Some(url) = mapping.get(specifier) {
                prelude.push_str(&format!(
                    "import * as __ferrite_cjs_dep{index}__ from {url:?};\n"
                ));
                let replacement = format!("__ferrite_interop__(__ferrite_cjs_dep{index}__)");
                code = replace_require(&code, specifier, &replacement);
            }
        }
        let wrapped = format!(
            "{prelude}\
             const module = {{ exports: {{}} }};\n\
             const exports = module.exports;\n\
             const __filename = {:?};\n\
             const __dirname = {:?};\n\
             {code}\n\
             export default module.exports;\n",
            module.id.0,
            module.id.0.rsplit_once('/').map_or("/", |(dir, _)| dir),
        );
        // The wrapper exports only `default`; re-export facts from the
        // original source no longer describe this code.
        if let Some(shake) = shake.as_mut() {
            shake.exports = vec![ferrite_transform::ParsedExport {
                exported: "default".to_string(),
                local: None,
                from: None,
                imported: None,
                target: None,
            }];
        }
        Ok(PipelineModule {
            code: wrapped,
            imports,
            has_module_syntax: true,
            shake,
            ..module
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
