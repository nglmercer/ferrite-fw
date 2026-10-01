//! Dev server pipeline context.

//! Dev server module transform pipeline.

use crate::util::*;
use crate::DevServer;
use crate::PipelineModule;
use ferrite_core::FerriteError;
use ferrite_core::Hash;
use ferrite_core::ModuleId;
use ferrite_core::ModuleType;
use ferrite_core::Result;
use ferrite_graph::ImportEdge;
use ferrite_graph::ModuleNode;
use ferrite_plugin::PluginContext;
use std::collections::HashMap;
use std::path::PathBuf;

impl DevServer {
    /// `import.meta.env.*` defines (§42).
    pub(crate) fn env_defines(&self, ssr: bool) -> HashMap<String, String> {
        let mut defines = HashMap::new();
        for (key, value) in &self.inner.env_vars {
            defines.insert(
                format!("import.meta.env.{key}"),
                serde_json::to_string(value).unwrap_or_default(),
            );
        }
        defines.insert(
            "import.meta.env.MODE".to_string(),
            serde_json::to_string(&self.inner.config.mode).unwrap_or_default(),
        );
        defines.insert(
            "import.meta.env.BASE_URL".to_string(),
            serde_json::to_string(&self.inner.config.base).unwrap_or_default(),
        );
        defines.insert(
            "import.meta.env.DEV".to_string(),
            (!self.inner.config.is_production).to_string(),
        );
        defines.insert(
            "import.meta.env.PROD".to_string(),
            self.inner.config.is_production.to_string(),
        );
        defines.insert("import.meta.env.SSR".to_string(), ssr.to_string());
        defines
    }

    /// Raw source for one module id: plugin `load` first, then fs.
    ///
    /// Build CSS extraction uses this so virtual stylesheets (e.g.
    /// `ferrite:tailwind.css`) resolve through plugins instead of failing
    /// on a missing file.
    pub async fn load_raw_source(&self, id: &ModuleId, env: &str) -> Result<(String, ModuleType)> {
        let environment = self.environment_for(env);
        let ctx = self.plugin_context(&environment);
        self.load_source(&ctx, &unvirtualize(id), &environment)
            .await
    }

    /// Map a module id to a file path.
    pub fn id_to_file(&self, id: &ModuleId) -> Result<PathBuf> {
        let (path, _) = id.split_query();
        if let Some(rest) = path.strip_prefix("/@npm/") {
            return Ok(self
                .inner
                .config
                .root
                .join(".ferrite/npm/packages")
                .join(rest));
        }
        if let Some(rest) = path.strip_prefix("/@fs") {
            return Ok(PathBuf::from(rest));
        }
        if let Some(rest) = path.strip_prefix("/@ferrite/") {
            return Err(FerriteError::Resolve(format!(
                "no file for virtual `{rest}`"
            )));
        }
        Ok(self.inner.config.root.join(path.trim_start_matches('/')))
    }

    /// True when `url` maps to a project file.
    pub(crate) fn is_project_file(&self, url: &str) -> bool {
        self.id_to_file(&ModuleId::new(url))
            .is_ok_and(|file| file.is_file())
    }

    /// Cache key for a transform.
    pub(crate) fn cache_key(
        &self,
        id: &ModuleId,
        source: &str,
        env: &str,
        defines: &HashMap<String, String>,
    ) -> Hash {
        let lock_state = std::fs::read(self.inner.config.lockfile()).ok().map(|bytes| Hash::of_bytes(&bytes).0);
        let pipeline = format!(
            "pipeline-v4:{}:{:?}:{}:{}:{:?}:{lock_state:?}",
            self.inner.plugins.cache_key(),
            self.inner.config.react,
            self.inner.config.is_production,
            self.inner.config.build.sourcemap.enabled(),
            self.inner.config.npm
        );
        let mut pairs: Vec<(&str, &str)> = defines
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();
        pairs.sort();
        let defines = pairs
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join("\0");
        ferrite_cache::transform_key(&ferrite_cache::TransformKeyInput {
            source,
            module_id: &id.0,
            compiler_version: self.inner.compiler.version(),
            pipeline_hash: &Hash::of_str(&pipeline).0,
            environment: env,
            target: &self.inner.config.build.target,
            mode: &self.inner.config.mode,
            defines: &defines,
        })
    }

    /// Compile-time defines for an environment (§42–§43).
    pub(crate) fn transform_defines(
        &self,
        environment: &ferrite_core::Environment,
    ) -> HashMap<String, String> {
        let mut define = environment.define.clone();
        define.extend(self.env_defines(environment.kind.is_ssr()));
        define
    }

    /// Record a module in the graph.
    pub(crate) fn update_graph(&self, module: &PipelineModule, env: &str) {
        let mut node = self.inner.graph.get(&module.id).unwrap_or_else(|| {
            ModuleNode::new(
                module.id.clone(),
                module.id.0.clone(),
                module.module_type.clone(),
            )
        });
        node.module_type = module.module_type.clone();
        node.url = module.id.0.clone();
        if let Ok(file) = self.id_to_file(&module.id) {
            node.file = Some(file);
        }
        node.hmr.self_accepting = module.uses_import_meta_hot;
        self.inner.graph.upsert(node);
        let mut edges: Vec<ImportEdge> = module
            .imports
            .iter()
            .map(|(specifier, resolved, kind)| ImportEdge {
                specifier: specifier.clone(),
                resolved: resolved.clone(),
                kind: kind.clone(),
            })
            .collect();
        for dependency in &module.dependencies {
            let file = PathBuf::from(dependency);
            let id = ModuleId::new(ferrite_core::file_to_url(&self.inner.config.root, &file));
            if id == module.id {
                continue;
            }
            if !self.inner.graph.contains(&id) {
                let mut node = ModuleNode::new(
                    id.clone(),
                    id.0.clone(),
                    ferrite_core::ModuleType::from_path(dependency),
                );
                node.file = Some(file);
                self.inner.graph.upsert(node);
            }
            if !edges.iter().any(|edge| edge.resolved == id) {
                edges.push(ImportEdge {
                    specifier: dependency.clone(),
                    resolved: id,
                    kind: ferrite_graph::ImportKind::Static,
                });
            }
        }
        self.inner.graph.set_imports(&module.id, edges);
        self.inner.graph.set_transformed(
            &module.id,
            env,
            module.code.clone(),
            &Hash::of_str(&module.code),
        );
    }

    /// Environment for an env name (`ssr`, or anything else → client).
    pub(crate) fn environment_for(&self, env: &str) -> ferrite_core::Environment {
        if env == "ssr" {
            self.inner.config.ssr_env()
        } else {
            self.inner.config.client_env()
        }
    }

    /// Build a plugin context for `environment`.
    pub(crate) fn plugin_context<'a>(
        &'a self,
        environment: &'a ferrite_core::Environment,
    ) -> PluginContext<'a> {
        PluginContext {
            graph: &self.inner.graph,
            resolver: &self.inner.client_resolver,
            environment,
            emitted: &self.inner.emitted,
            watch_files: &self.inner.watch_files,
            warnings: &self.inner.warnings,
        }
    }
}
