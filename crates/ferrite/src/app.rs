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
        let ctx = ferrite_plugin::PluginContext {
            graph: &self.graph,
            resolver: &self.resolver,
            environment: &environment,
            emitted: &self.emitted,
            watch_files: &self.watch_files,
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
                }
            }
        };
        // Transform (compiler, then plugin chain).
        let ssr = request.environment.is_ssr();
        let compiled = self
            .compiler
            .transform(ferrite_transform::TransformRequest {
                id: resolved.id.0.clone(),
                code: loaded.code,
                module_type: loaded.module_type.clone(),
                environment: request.environment.clone(),
                ssr,
                target: self.config.target(),
                minify: false,
                sourcemap: false,
                define: HashMap::new(),
                jsx_runtime: self.config.react.runtime.clone(),
                development: !self.config.is_production,
            })?;
        let hooked = self
            .plugins
            .hook_transform(
                &ctx,
                ferrite_plugin::TransformRequest {
                    id: resolved.id.0,
                    code: compiled.code,
                    module_type: loaded.module_type,
                    environment: request.environment,
                    ssr,
                },
            )
            .await?;
        Ok(ferrite_transform::TransformResult {
            code: hooked.code,
            map: hooked.map.or(compiled.map),
            dependencies: compiled.dependencies,
            imports: compiled.imports,
            exports: compiled.exports,
        })
    }
}
