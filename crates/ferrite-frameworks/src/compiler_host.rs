//! Typed official compiler calls. Node construction is explicit; no host fallback.
use ferrite_core::{FerriteError, ModuleType, Result, SourceMap};
use ferrite_npm::Lockfile;
use ferrite_plugin::node_adapter::NodeAdapterHost;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Framework {
    Vue,
    Svelte,
}
impl Framework {
    fn name(self) -> &'static str {
        match self {
            Self::Vue => "vue",
            Self::Svelte => "svelte",
        }
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompileTarget {
    Client,
    Server,
}
#[derive(Debug, Clone)]
pub struct CompileRequest {
    pub framework: Framework,
    pub filename: PathBuf,
    pub source: String,
    pub target: CompileTarget,
    pub development: bool,
    pub module: bool,
}
#[derive(Debug, Clone, Deserialize)]
pub struct CompileCss {
    pub id: String,
    pub code: String,
    pub map: Option<serde_json::Value>,
    pub modules: Option<serde_json::Value>,
}
#[derive(Debug, Clone, Deserialize)]
pub struct CompileDiagnostic {
    pub severity: String,
    pub code: String,
    pub message: String,
    pub filename: String,
    pub start: Option<serde_json::Value>,
}
#[derive(Debug)]
pub struct CompileResult {
    pub code: String,
    pub map: Option<SourceMap>,
    pub module_type: ModuleType,
    pub css: Vec<CompileCss>,
    pub dependencies: Vec<String>,
    pub diagnostics: Vec<CompileDiagnostic>,
    pub compiler_version: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkerResult {
    pieces: Vec<Piece>,
    language: String,
    css: Vec<CompileCss>,
    dependencies: Vec<String>,
    diagnostics: Vec<CompileDiagnostic>,
    compiler_version: String,
}
#[derive(Deserialize)]
struct Piece {
    code: String,
    map: Option<serde_json::Value>,
}

/// Persistent explicitly enabled Node compiler host. Separate from SSR runtime.
/// Node is a normal child process, not a sandbox. No embedded/native fallback.
pub struct NodeCompilerHost {
    host: Arc<NodeAdapterHost>,
    root: PathBuf,
    lock: Lockfile,
    lock_path: PathBuf,
    _wrapper: tempfile::TempPath,
}
impl NodeCompilerHost {
    pub async fn new(
        root: PathBuf,
        lock_path: PathBuf,
        node_path: Option<PathBuf>,
        timeout: Duration,
    ) -> Result<Self> {
        tokio::task::spawn_blocking(move || {
            let root = root.canonicalize()?;
            let lock_path = if lock_path.is_absolute() {
                lock_path
            } else {
                root.join(lock_path)
            };
            let lock = Lockfile::read(&lock_path)?;
            ferrite_npm::project_node_modules(&root.join(".ferrite/npm"), &lock)?;
            let mut wrapper = tempfile::Builder::new()
                .prefix("ferrite-compiler-")
                .suffix(".mjs")
                .tempfile()?;
            use std::io::Write as _;
            wrapper.write_all(include_bytes!("compiler_worker.mjs"))?;
            wrapper.as_file().sync_all()?;
            let wrapper = wrapper.into_temp_path();
            let host = NodeAdapterHost::spawn_with_timeout(node_path, timeout)?;
            let entry = url::Url::from_file_path(&wrapper).map_err(|()| {
                FerriteError::Build(
                    "compiler worker path cannot be represented as a file URL".into(),
                )
            })?;
            host.register_plugin("framework-compiler", entry.as_str())?;
            Ok(Self {
                host: Arc::new(host),
                root,
                lock,
                lock_path,
                _wrapper: wrapper,
            })
        })
        .await
        .map_err(|error| {
            FerriteError::Build(format!("compiler host initialization failed: {error}"))
        })?
    }
    /// Stable compiler graph identity used by shared pipeline caches.
    pub fn cache_identity(&self) -> String {
        ferrite_core::Hash::of_str(
            &serde_json::to_string(&self.lock).expect("lock graph serializes"),
        )
        .0
    }
    pub fn process_id(&self) -> Result<u32> {
        self.host.process_id()
    }
    pub fn cancel(&self) {
        self.host.shutdown();
    }
    pub async fn compile(&self, mut request: CompileRequest) -> Result<CompileResult> {
        if Lockfile::read(&self.lock_path)? != self.lock {
            return Err(FerriteError::Build("compiler dependency graph changed; explicitly recreate the compiler host after installing".into()));
        }
        if request.module && matches!(request.framework, Framework::Vue) {
            return Err(FerriteError::Build(
                "Vue module compilation is unavailable; use a .vue component".into(),
            ));
        }
        let name = request.framework.name();
        let package = self.lock.find(name).ok_or_else(|| {
            FerriteError::Build(format!(
                "project has no unambiguous locked {name} dependency; run ferrite add {name}"
            ))
        })?;
        let supported = match request.framework {
            Framework::Vue => crate::registry::VUE_NODE.framework_version,
            Framework::Svelte => crate::registry::SVELTE_NODE.framework_version,
        };
        if package.version != supported {
            return Err(FerriteError::Build(format!("{name}@{} compiler host is unvalidated; this host profile currently validates {supported}", package.version)));
        }
        let package_dir = self.root.join(".ferrite/npm/packages").join(package.id());
        let mut input_map = None;
        if request.module
            && request
                .filename
                .extension()
                .is_some_and(|extension| extension == "ts")
        {
            use ferrite_transform::JsCompiler;
            let result = ferrite_transform::OxcCompiler::new(Default::default()).transform(
                ferrite_transform::TransformRequest::new(
                    request.filename.to_string_lossy(),
                    request.source,
                    ModuleType::Ts,
                ),
            )?;
            request.source = result.code;
            input_map = result.map;
        }
        let value = self.host.call_export("framework-compiler", "compile", serde_json::json!({
            "framework": request.framework, "packageDir": package_dir, "version": package.version, "filename": request.filename, "source": request.source,
            "target": request.target, "development": request.development, "module": request.module,
            "scopeId": ferrite_core::Hash::of_str(&request.filename.strip_prefix(&self.root).unwrap_or(&request.filename).to_string_lossy()).short(8)
        })).await?;
        let output: WorkerResult = serde_json::from_value(value).map_err(|error| {
            FerriteError::Build(format!("compiler result protocol mismatch: {error}"))
        })?;
        let mut code = String::new();
        let mut maps = Vec::new();
        let mut offsets = Vec::new();
        let mut line = 0;
        for piece in output.pieces {
            maps.push(
                piece
                    .map
                    .map(|map| serde_json::to_string(&map))
                    .transpose()?
                    .unwrap_or_else(|| {
                        "{\"version\":3,\"sources\":[],\"names\":[],\"mappings\":\"A\"}".into()
                    }),
            );
            offsets.push(line);
            code.push_str(&piece.code);
            code.push('\n');
            line += piece.code.bytes().filter(|byte| *byte == b'\n').count() as u32 + 1;
        }
        let decoded = maps
            .iter()
            .map(|map| {
                oxc_sourcemap::SourceMap::from_json_string(map).map_err(|error| {
                    FerriteError::Build(format!("compiler returned invalid source map: {error}"))
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let pairs: Vec<_> = decoded.iter().zip(offsets).collect();
        let map = if pairs.is_empty() {
            None
        } else {
            Some(
                oxc_sourcemap::ConcatSourceMapBuilder::from_sourcemaps(&pairs)
                    .into_owned_sourcemap()
                    .to_json_string(),
            )
        };
        let map = match (map, input_map) {
            (Some(outer), Some(inner)) => Some(ferrite_transform::chain_source_maps(
                &outer,
                &inner.mappings,
            )?),
            (map, _) => map,
        };
        Ok(CompileResult {
            code,
            map: map.map(SourceMap::external),
            module_type: if output.language == "ts" {
                ModuleType::Ts
            } else {
                ModuleType::Js
            },
            css: output.css,
            dependencies: output.dependencies,
            diagnostics: output.diagnostics,
            compiler_version: output.compiler_version,
        })
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
}
