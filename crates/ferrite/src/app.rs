//! Core application handle.

use ferrite_config::ResolvedConfig;
use ferrite_core::EnvironmentKind;
use ferrite_core::FerriteError;
use ferrite_core::ModuleId;
use ferrite_core::ModuleType;
use ferrite_core::Result;
use ferrite_plugin::Apply;
use ferrite_plugin::Plugin;
use ferrite_plugin::PluginContainer;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

/// First core type: config + plugins + graph + resolver + compiler.
pub struct Ferrite {
    /// Resolved config.
    pub config: ResolvedConfig,
    /// Plugin container.
    pub plugins: PluginContainer,
    /// Module graph.
    pub graph: Arc<ferrite_graph::ModuleGraph>,
    /// Resolver.
    pub resolver: Arc<ferrite_resolver::Resolver>,
    /// JS compiler.
    pub compiler: Arc<dyn ferrite_transform::JsCompiler>,
    /// Emitted files (plugin context backing).
    emitted: Mutex<HashMap<String, ferrite_plugin::EmittedFile>>,
    /// Watch files (plugin context backing).
    watch_files: Mutex<Vec<String>>,
    /// Warnings (plugin context backing).
    warnings: Mutex<Vec<String>>,
}

/// A module transform request.
#[derive(Debug, Clone)]
pub struct ModuleRequest {
    /// Specifier to resolve.
    pub specifier: String,
    /// Importer, if any.
    pub importer: Option<ModuleId>,
    /// Target environment.
    pub environment: EnvironmentKind,
}

impl Ferrite {
    /// Create from resolved config + plugins.
    #[must_use]
    pub fn new(config: ResolvedConfig, plugins: Vec<Arc<dyn Plugin>>) -> Self {
        let mode = if config.is_production {
            Apply::Build
        } else {
            Apply::Serve
        };
        let resolver = ferrite_resolver::Resolver::for_environment(
            config.root.clone(),
            &config.resolve,
            &EnvironmentKind::Client,
        );
        let compiler: Arc<dyn ferrite_transform::JsCompiler> =
            ferrite_transform::compiler_for_engine(&config.compiler.engine).unwrap_or_else(|_| {
                Arc::new(ferrite_transform::OxcCompiler::new(Default::default()))
            });
        Self {
            config,
            plugins: PluginContainer::new(plugins, mode),
            graph: Arc::new(ferrite_graph::ModuleGraph::default()),
            resolver: Arc::new(resolver),
            compiler,
            emitted: Mutex::new(HashMap::new()),
            watch_files: Mutex::new(Vec::new()),
            warnings: Mutex::new(Vec::new()),
        }
    }

    /// Resolve → load → transform one module (spec §99).
    pub async fn transform_request(
        &self,
        request: ModuleRequest,
    ) -> Result<ferrite_transform::TransformResult> {
        let environment = ferrite_core::Environment::new("client", request.environment.clone());
        let module_watches = Mutex::new(Vec::new());
        let ctx = ferrite_plugin::PluginContext {
            graph: &self.graph,
            resolver: &self.resolver,
            environment: &environment,
            emitted: &self.emitted,
            watch_files: &module_watches,
            warnings: &self.warnings,
        };
        // Resolve (plugin first).
        let resolved = match self
            .plugins
            .hook_resolve_id(
                &ctx,
                ferrite_plugin::ResolveHookRequest {
                    specifier: &request.specifier,
                    importer: request.importer.as_ref(),
                    environment: request.environment.clone(),
                    ssr: request.environment.is_ssr(),
                },
            )
            .await?
        {
            Some(resolved) => resolved,
            None => self.resolver.resolve(&ferrite_resolver::ResolveRequest {
                specifier: &request.specifier,
                importer: request.importer.as_ref(),
                environment: request.environment.clone(),
                kind: ferrite_resolver::ResolveKind::Import,
            })?,
        };
        // Load (plugin first, then fs).
        let loaded = match self
            .plugins
            .hook_load(
                &ctx,
                ferrite_plugin::LoadRequest {
                    id: resolved.id.0.clone(),
                    environment: request.environment.clone(),
                },
            )
            .await?
        {
            Some(loaded) => loaded,
            None => {
                let file = self.config.root.join(resolved.id.0.trim_start_matches('/'));
                let code = std::fs::read_to_string(&file).map_err(|_| {
                    FerriteError::Resolve(format!("cannot load `{}`", resolved.id.0))
                })?;
                ferrite_plugin::LoadResult {
                    code,
                    module_type: ModuleType::from_path(&file),
                    dependencies: Vec::new(),

                    ..Default::default()
                }
            }
        };
        let ssr = request.environment.is_ssr();
        let pre = self
            .plugins
            .hook_transform_phase(
                &ctx,
                ferrite_plugin::TransformRequest {
                    id: resolved.id.0.clone(),
                    code: loaded.code.clone(),
                    module_type: loaded.module_type.clone(),
                    environment: request.environment.clone(),
                    ssr,
                },
                ferrite_plugin::TransformPhase::BeforeLowering,
            )
            .await?;
        let pre_map = merge_maps(pre.map.clone(), loaded.map, pre.code == loaded.code)?;
        let compiled = self
            .compiler
            .transform(ferrite_transform::TransformRequest {
                id: resolved.id.0.clone(),
                code: pre.code.clone(),
                module_type: pre.module_type.unwrap_or(loaded.module_type),
                environment: request.environment.clone(),
                ssr,
                target: self.config.target(),
                minify: false,
                sourcemap: self.config.build.sourcemap.enabled(),
                define: HashMap::new(),
                jsx_runtime: self.config.react.runtime.clone(),
                development: !self.config.is_production,
            })?;
        let compiled_map = merge_maps(compiled.map, pre_map, compiled.code == pre.code)?;
        let hooked = self
            .plugins
            .hook_transform_phase(
                &ctx,
                ferrite_plugin::TransformRequest {
                    id: resolved.id.0.clone(),
                    code: compiled.code.clone(),
                    module_type: ModuleType::Js,
                    environment: request.environment,
                    ssr,
                },
                ferrite_plugin::TransformPhase::AfterLowering,
            )
            .await?;
        let map = merge_maps(hooked.map, compiled_map, hooked.code == compiled.code)?;
        let parsed = self.compiler.parse(ferrite_transform::ParseRequest {
            id: resolved.id.0,
            code: hooked.code.clone(),
            module_type: hooked.module_type.unwrap_or(ModuleType::Js),
        })?;
        let mut dependencies = loaded.dependencies;
        dependencies.extend(pre.dependencies);
        dependencies.extend(compiled.dependencies);
        dependencies.extend(hooked.dependencies);
        dependencies.extend(
            module_watches
                .lock()
                .map_err(|_| FerriteError::Other("module watch lock poisoned".into()))?
                .clone(),
        );
        if let Ok(mut watches) = self.watch_files.lock() {
            watches.extend(dependencies.clone());
            watches.sort();
            watches.dedup();
        }
        dependencies.sort();
        dependencies.dedup();
        Ok(ferrite_transform::TransformResult {
            code: hooked.code,
            map,
            dependencies,
            imports: parsed.imports,
            exports: parsed.exports,
        })
    }
}

fn merge_maps(
    outer: Option<ferrite_core::SourceMap>,
    inner: Option<ferrite_core::SourceMap>,
    unchanged: bool,
) -> Result<Option<ferrite_core::SourceMap>> {
    match (outer, inner) {
        (Some(outer), Some(inner)) => Ok(Some(ferrite_core::SourceMap::external(
            ferrite_transform::chain_source_maps(&outer.mappings, &inner.mappings)?,
        ))),
        (Some(outer), None) => Ok(Some(outer)),
        (None, inner) if unchanged => Ok(inner),
        (None, _) => Ok(None),
    }
}

#[cfg(test)]
mod pipeline_tests {
    use super::*;
    use ferrite_plugin::{
        Enforce, LoadRequest, LoadResult, PluginContext, ResolveHookRequest, TransformRequest,
        TransformResult,
    };

    struct Fixture;
    #[async_trait::async_trait]
    impl Plugin for Fixture {
        fn name(&self) -> &'static str {
            "pipeline-parity-fixture"
        }
        fn enforce(&self) -> Enforce {
            Enforce::Pre
        }
        async fn resolve_id(
            &self,
            _: &PluginContext,
            request: ResolveHookRequest<'_>,
        ) -> Result<Option<ferrite_resolver::ResolvedId>> {
            Ok(Some(ferrite_resolver::ResolvedId::new(request.specifier)))
        }
        async fn load(
            &self,
            _: &PluginContext,
            request: LoadRequest,
        ) -> Result<Option<LoadResult>> {
            if request.id != "/fixture.ts" {
                return Ok(None);
            }
            Ok(Some(LoadResult {
                code: "export const answer: number = MAGIC;".into(),
                module_type: ModuleType::Ts,
                ..Default::default()
            }))
        }
        async fn transform(
            &self,
            _: &PluginContext,
            request: TransformRequest,
        ) -> Result<Option<TransformResult>> {
            assert!(request.code.contains(": number"));
            let pos = request.code.find("MAGIC").unwrap();
            let (code, map) = ferrite_transform::apply_text_edits(
                &request.id,
                &request.code,
                &[(pos, pos + 5, "42".into())],
                true,
            )?;
            Ok(Some(TransformResult {
                code,
                map: map.map(ferrite_core::SourceMap::external),
                dependencies: vec![],
                module_type: None,
            }))
        }
    }
    struct FinalFixture;
    #[async_trait::async_trait]
    impl Plugin for FinalFixture {
        fn name(&self) -> &'static str {
            "final-parity-fixture"
        }
        async fn transform(
            &self,
            _: &PluginContext,
            request: TransformRequest,
        ) -> Result<Option<TransformResult>> {
            assert!(!request.code.contains(": number"));
            let (code, map) = ferrite_transform::apply_text_edits(
                &request.id,
                &request.code,
                &[(
                    request.code.len(),
                    request.code.len(),
                    "\nimport '/added.js';\nexport const added = true;\n".into(),
                )],
                true,
            )?;
            Ok(Some(TransformResult {
                code,
                map: map.map(ferrite_core::SourceMap::external),
                dependencies: vec![],
                module_type: None,
            }))
        }
    }

    #[tokio::test]
    async fn library_and_server_share_compiler_order_and_final_analysis() {
        let config =
            ferrite_config::resolve_config(Default::default(), None, Default::default()).unwrap();
        let plugins: Vec<Arc<dyn Plugin>> = vec![Arc::new(Fixture), Arc::new(FinalFixture)];
        let library = Ferrite::new(config.clone(), plugins.clone());
        let server = ferrite_server::DevServer::new_without_watcher(config, plugins)
            .await
            .unwrap();
        let transformed = library
            .transform_request(ModuleRequest {
                specifier: "/fixture.ts".into(),
                importer: None,
                environment: EnvironmentKind::Client,
            })
            .await
            .unwrap();
        let served = server
            .pipeline_module(&ModuleId::new("/fixture.ts"), None, "client")
            .await
            .unwrap();
        assert!(transformed.code.contains("42"));
        assert!(served.code.contains("42"));
        assert!(transformed.exports.contains(&"added".into()));
        assert!(served
            .shake
            .unwrap()
            .exports
            .iter()
            .any(|export| export.exported == "added"));
        assert_eq!(transformed.imports[0].specifier, served.imports[0].0);
        let original_map: serde_json::Value =
            serde_json::from_str(&transformed.map.unwrap().mappings).unwrap();
        let server_map: serde_json::Value = serde_json::from_str(&served.map.unwrap()).unwrap();
        assert_eq!(original_map["sources"], server_map["sources"]);
        assert_eq!(original_map["sourcesContent"], server_map["sourcesContent"]);
        server.close();
    }
}
