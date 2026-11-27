//! Svelte compilation through the official self-contained compiler inside napi-vm.
#![cfg(feature = "napi-vm")]

use crate::compiler_host::{
    selected_compiler_package, CompileDiagnostic, CompileRequest, CompileResult, Framework,
    FrameworkCompiler,
};
use async_trait::async_trait;
use ferrite_core::{FerriteError, ModuleType, Result, SourceMap};
use ferrite_npm::Lockfile;
use ferrite_runtime::napi_vm::{NapiVmOptions, NapiVmRuntime};
use ferrite_runtime::{
    js_value_to_json, CompiledModule, CompiledModuleGraph, JsHandle, JsRuntime, JsValue,
    RuntimeEnvironment,
};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Persistent embedded Svelte compiler. The upstream compiler bundle is data
/// inside Ferrite's Rust process; no Node executable or external compiler
/// process is started.
pub struct EmbeddedSvelteCompilerHost {
    runtime: NapiVmRuntime,
    compile: JsHandle,
    root: PathBuf,
    lock: Lockfile,
    lock_path: PathBuf,
    cache_identity: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkerResult {
    pieces: Vec<Piece>,
    language: String,
    css: Vec<crate::compiler_host::CompileCss>,
    dependencies: Vec<String>,
    diagnostics: Vec<CompileDiagnostic>,
    compiler_version: String,
}
#[derive(Deserialize)]
struct Piece {
    code: String,
    map: Option<serde_json::Value>,
}

impl EmbeddedSvelteCompilerHost {
    pub async fn new(root: PathBuf, lock_path: PathBuf, timeout: Duration) -> Result<Self> {
        let root = root.canonicalize()?;
        let lock_path = if lock_path.is_absolute() {
            lock_path
        } else {
            root.join(lock_path)
        };
        let lock = Lockfile::read(&lock_path)?;
        let compilers: Vec<_> = lock
            .package
            .iter()
            .filter(|package| package.name == "svelte")
            .collect();
        let compiler_package = match compilers.as_slice() {
            [package] => *package,
            [] => {
                return Err(FerriteError::Build(
                    "embedded Svelte compilation requires locked svelte".into(),
                ))
            }
            _ => {
                return Err(FerriteError::Build(
                    "embedded Svelte compilation requires one project-wide svelte version".into(),
                ))
            }
        };
        let supported = crate::registry::SVELTE_NODE.framework_version;
        if compiler_package.version != supported {
            return Err(FerriteError::Build(format!(
                "svelte@{} embedded host is unvalidated; this host validates {supported}",
                compiler_package.version
            )));
        }
        let bundle_path = root
            .join(".ferrite/npm/packages")
            .join(compiler_package.id())
            .join("compiler/index.js");
        let bundle = std::fs::read_to_string(&bundle_path).map_err(|error| {
            FerriteError::Build(format!(
                "embedded Svelte compiler bundle {} cannot be read: {error}; run ferrite install --frozen-lockfile",
                bundle_path.display()
            ))
        })?;
        let cache_identity = ferrite_core::Hash::of_str(
            &serde_json::json!({
                "lock": lock,
                "host": "napi-vm-embedded-svelte",
                "compilerWrapper": include_str!("embedded_svelte_worker.mjs"),
                "compilerBundle": bundle,
            })
            .to_string(),
        )
        .0;
        let mut options = NapiVmOptions::default();
        options.request_timeout_ms = timeout.as_millis() as u64;
        let runtime = NapiVmRuntime::new(options);
        let graph = CompiledModuleGraph {
            entry: "/embedded-svelte-worker.mjs".into(),
            modules: vec![
                CompiledModule {
                    id: "/svelte-compiler.mjs".into(),
                    code: bundle,
                    url: Some(bundle_path.to_string_lossy().into_owned()),
                },
                CompiledModule {
                    id: "/embedded-svelte-worker.mjs".into(),
                    code: include_str!("embedded_svelte_worker.mjs").into(),
                    url: None,
                },
            ],
        };
        let namespace = runtime
            .evaluate_module_graph(
                graph,
                RuntimeEnvironment {
                    ssr: false,
                    request_id: None,
                },
            )
            .await?;
        let compile = namespace.get_function("compile")?.clone();
        Ok(Self {
            runtime,
            compile,
            root,
            lock,
            lock_path,
            cache_identity,
        })
    }

    pub fn cancel(&self) {
        self.runtime.shutdown();
    }
}

#[async_trait]
impl FrameworkCompiler for EmbeddedSvelteCompilerHost {
    fn cache_identity(&self) -> String {
        self.cache_identity.clone()
    }
    fn root(&self) -> &Path {
        &self.root
    }
    fn host_kind(&self) -> &'static str {
        "embedded"
    }
    async fn compile(&self, mut request: CompileRequest) -> Result<CompileResult> {
        if Lockfile::read(&self.lock_path)? != self.lock {
            return Err(FerriteError::Build(
                "compiler dependency graph changed; explicitly recreate the compiler host after installing".into(),
            ));
        }
        if !matches!(request.framework, Framework::Svelte) {
            return Err(FerriteError::Build(
                "embedded compilation currently supports Svelte only in this host".into(),
            ));
        }
        let package =
            selected_compiler_package(&self.lock, &self.root, &request.filename, "svelte")?;
        let supported = crate::registry::SVELTE_NODE.framework_version;
        if package.version != supported {
            return Err(FerriteError::Build(format!(
                "svelte@{} compiler host is unvalidated; this host profile currently validates {supported}",
                package.version
            )));
        }
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
        let input = serde_json::json!({
            "filename": request.filename,
            "source": request.source,
            "target": request.target,
            "development": request.development,
            "version": package.version,
        });
        let output = self
            .runtime
            .call(&self.compile, vec![JsValue::from(input)])
            .await?;
        let output: WorkerResult = serde_json::from_value(integer_json(js_value_to_json(&output)?))
            .map_err(|error| {
                FerriteError::Build(format!(
                    "embedded compiler result protocol mismatch: {error}"
                ))
            })?;
        finish_output(output, input_map)
    }
}

fn integer_json(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Number(number) => {
            if let Some(value) = number.as_f64() {
                if value.fract() == 0.0 && value >= i64::MIN as f64 && value <= i64::MAX as f64 {
                    return serde_json::Value::Number((value as i64).into());
                }
            }
            serde_json::Value::Number(number)
        }
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.into_iter().map(integer_json).collect())
        }
        serde_json::Value::Object(values) => serde_json::Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, integer_json(value)))
                .collect(),
        ),
        value => value,
    }
}

fn finish_output(output: WorkerResult, input_map: Option<SourceMap>) -> Result<CompileResult> {
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
