//! Svelte compilation through the official self-contained compiler inside rquickjs.
#![cfg(feature = "rquickjs")]

use crate::compiler_host::{
    selected_compiler_package, CompileDiagnostic, CompileRequest, CompileResult, Framework,
    FrameworkCompiler,
};
use async_trait::async_trait;
use ferrite_core::{FerriteError, ModuleType, Result, SourceMap};
use ferrite_npm::Lockfile;
use ferrite_runtime::quickjs::{QuickJsOptions, QuickJsRuntime};
use ferrite_runtime::{
    js_value_to_json, CompiledModule, CompiledModuleGraph, JsHandle, JsRuntime, JsValue,
    RuntimeEnvironment,
};
use serde::Deserialize;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Persistent embedded Svelte compiler. The upstream compiler bundle is data
/// inside Ferrite's Rust process; no Node executable or external compiler
/// process is started.
pub struct EmbeddedSvelteCompilerHost {
    runtime: QuickJsRuntime,
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

fn embedded_compressed_bundle(bytes: &[u8], description: &str) -> Result<String> {
    let mut bundle = String::new();
    flate2::read::GzDecoder::new(bytes)
        .read_to_string(&mut bundle)
        .map_err(|error| {
            FerriteError::Build(format!("{description} cannot be decoded: {error}"))
        })?;
    Ok(bundle)
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
        if !lock.package.iter().any(|package| package.name == "svelte") {
            return Err(FerriteError::Build(
                "embedded Svelte compilation requires locked svelte".into(),
            ));
        }
        let bundle = embedded_compressed_bundle(
            include_bytes!("embedded_compilers/svelte-compiler-5.39.6.js.gz"),
            "svelte@5.39.6 embedded compiler",
        )?;
        let cache_identity = ferrite_core::Hash::of_str(
            &serde_json::json!({
                "lock": lock,
                "host": "rquickjs-embedded-svelte",
                "compilerWrapper": include_str!("embedded_svelte_worker.mjs"),
                "compilerBundle": ferrite_core::Hash::of_str(&bundle).0,
            })
            .to_string(),
        )
        .0;
        let mut options = QuickJsOptions::default();
        options.request_timeout_ms = timeout.as_millis() as u64;
        let runtime = QuickJsRuntime::new(options);
        let graph = CompiledModuleGraph {
            entry: "/embedded-svelte-worker.mjs".into(),
            modules: vec![
                CompiledModule {
                    id: "/svelte-compiler.mjs".into(),
                    code: bundle,
                    url: Some("ferrite-embedded://svelte/compiler-5.39.6.js".into()),
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
            "module": request.module,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler_host::CompileTarget;
    use ferrite_npm::{LockedImporter, LockedPackage};

    fn locked_project() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        let package = LockedPackage {
            name: "svelte".into(),
            version: "5.39.6".into(),
            source: "npm".into(),
            ..Default::default()
        };
        let directory = root.path().join(".ferrite/npm/packages").join(package.id());
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("package.json"),
            r#"{"name":"svelte","version":"5.39.6"}"#,
        )
        .unwrap();
        let mut importer = LockedImporter::default();
        importer.specifiers.insert("svelte".into(), "5.39.6".into());
        importer.dependencies.insert("svelte".into(), package.id());
        let transitive = LockedPackage {
            name: "svelte".into(),
            version: "5.0.0".into(),
            source: "npm".into(),
            ..Default::default()
        };
        let mut lock = Lockfile::default();
        lock.package.push(package);
        lock.package.push(transitive);
        lock.importers.insert(".".into(), importer);
        lock.write(&root.path().join("ferrite.lock")).unwrap();
        root
    }

    #[tokio::test]
    async fn embedded_svelte_compiles_rune_modules_as_modules() {
        let root = locked_project();
        let host = EmbeddedSvelteCompilerHost::new(
            root.path().into(),
            root.path().join("ferrite.lock"),
            Duration::from_secs(30),
        )
        .await
        .unwrap();
        let result = host
            .compile(CompileRequest {
                framework: Framework::Svelte,
                filename: root.path().join("counter.svelte.ts"),
                source: "let count: number = $state(0); export function increment() { count++; return count; }".into(),
                target: CompileTarget::Client,
                development: true,
                module: true,
            })
            .await
            .unwrap();
        assert!(result.code.contains("increment"), "{}", result.code);
        assert!(!result.code.contains(": number"), "{}", result.code);

        let component = host
            .compile(CompileRequest {
                framework: Framework::Svelte,
                filename: root.path().join("App.svelte"),
                source: "<script>let count = $state(0);</script><button onclick={() => count++}>{count}</button>".into(),
                target: CompileTarget::Client,
                development: true,
                module: false,
            })
            .await
            .unwrap();
        assert!(
            !component
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "invalid_original_source_map_position"),
            "valid negative source-map deltas must not be discarded: {:?}",
            component.diagnostics
        );
        let component_map = component.map.unwrap();
        let map = oxc_sourcemap::SourceMap::from_json_string(&component_map.mappings).unwrap();
        assert!(
            map.get_tokens()
                .any(|token| token.get_source_id().is_some()),
            "valid original positions must survive"
        );
    }
}
