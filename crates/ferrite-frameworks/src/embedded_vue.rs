//! Vue compilation through the official browser compiler inside rquickjs.
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
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Persistent embedded Vue compiler. The upstream browser ESM compiler is data
/// inside Ferrite's Rust process; no Node executable or external compiler
/// process is started.
pub struct EmbeddedVueCompilerHost {
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

impl EmbeddedVueCompilerHost {
    pub async fn new(root: PathBuf, lock_path: PathBuf, timeout: Duration) -> Result<Self> {
        let root = root.canonicalize()?;
        let lock_path = if lock_path.is_absolute() {
            lock_path
        } else {
            root.join(lock_path)
        };
        let lock = Lockfile::read(&lock_path)?;
        if !lock
            .package
            .iter()
            .any(|package| package.name == "@vue/compiler-sfc")
        {
            return Err(FerriteError::Build(
                "embedded Vue compilation requires locked @vue/compiler-sfc".into(),
            ));
        }
        let bundle = embedded_compressed_bundle(
            include_bytes!("embedded_compilers/vue-compiler-sfc-3.5.22.js.gz"),
            "@vue/compiler-sfc@3.5.22 embedded browser compiler",
        )?;
        let cache_identity = ferrite_core::Hash::of_str(
            &serde_json::json!({
                "lock": lock,
                "host": "rquickjs-embedded-vue",
                "compilerWrapper": include_str!("embedded_vue_worker.mjs"),
                "compilerBundle": ferrite_core::Hash::of_str(&bundle).0,
            })
            .to_string(),
        )
        .0;
        let mut options = QuickJsOptions::default();
        options.request_timeout_ms = timeout.as_millis() as u64;
        let runtime = QuickJsRuntime::new(options);
        let graph = CompiledModuleGraph {
            entry: "/embedded-vue-worker.mjs".into(),
            modules: vec![
                CompiledModule {
                    id: "/vue-compiler.mjs".into(),
                    code: bundle,
                    url: Some("ferrite-embedded://vue/compiler-sfc-3.5.22.js".into()),
                },
                CompiledModule {
                    id: "/embedded-vue-worker.mjs".into(),
                    code: include_str!("embedded_vue_worker.mjs").into(),
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
impl FrameworkCompiler for EmbeddedVueCompilerHost {
    fn cache_identity(&self) -> String {
        self.cache_identity.clone()
    }
    fn root(&self) -> &Path {
        &self.root
    }
    fn host_kind(&self) -> &'static str {
        "embedded"
    }
    async fn compile(&self, request: CompileRequest) -> Result<CompileResult> {
        if Lockfile::read(&self.lock_path)? != self.lock {
            return Err(FerriteError::Build(
                "compiler dependency graph changed; explicitly recreate the compiler host after installing".into(),
            ));
        }
        if !matches!(request.framework, Framework::Vue) {
            return Err(FerriteError::Build(
                "embedded compilation currently supports Vue only".into(),
            ));
        }
        if request.module {
            return Err(FerriteError::Build(
                "Vue module compilation is unavailable; use a .vue component".into(),
            ));
        }
        let package = selected_compiler_package(&self.lock, &self.root, &request.filename, "vue")?;
        let supported = crate::registry::VUE_NODE.framework_version;
        if package.version != supported {
            return Err(FerriteError::Build(format!(
                "vue@{} compiler host is unvalidated; this host profile currently validates {supported}",
                package.version
            )));
        }
        let files = collect_type_files(&self.root, &request.filename, &request.source)?;
        let scope_id = ferrite_core::Hash::of_str(
            &request
                .filename
                .strip_prefix(&self.root)
                .unwrap_or(&request.filename)
                .to_string_lossy(),
        )
        .short(8);
        let input = serde_json::json!({
            "filename": request.filename,
            "source": request.source,
            "target": request.target,
            "development": request.development,
            "scopeId": scope_id,
            "files": files,
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
        finish_output(output)
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

fn finish_output(output: WorkerResult) -> Result<CompileResult> {
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

fn collect_type_files(
    root: &Path,
    filename: &Path,
    source: &str,
) -> Result<BTreeMap<String, String>> {
    let mut files = BTreeMap::new();
    let mut pending = vec![filename.to_path_buf()];
    let mut seen = BTreeSet::new();
    let mut sources = BTreeMap::from([(filename.to_path_buf(), source.to_string())]);
    while let Some(file) = pending.pop() {
        if files.len() >= 64 || !seen.insert(file.clone()) {
            continue;
        }
        let Some(text) = sources.get(&file).cloned() else {
            continue;
        };
        for specifier in relative_imports(&text) {
            for candidate in resolve_typescript_candidate(&file, &specifier) {
                if !candidate.starts_with(root) || seen.contains(&candidate) {
                    continue;
                }
                if let Ok(text) = std::fs::read_to_string(&candidate) {
                    if text.len() > 1024 * 1024 {
                        return Err(FerriteError::Build(format!(
                            "Vue type dependency {} exceeds the embedded compiler input limit",
                            candidate.display()
                        )));
                    }
                    files.insert(ferrite_core::normalize_path(&candidate), text.clone());
                    sources.insert(candidate.clone(), text);
                    pending.push(candidate);
                    break;
                }
            }
        }
    }
    Ok(files)
}

fn relative_imports(source: &str) -> Vec<String> {
    let mut imports = Vec::new();
    for prefix in ["from '", "from \"", "import '", "import \""] {
        let quote = prefix.chars().last().unwrap();
        let mut rest = source;
        while let Some(start) = rest.find(prefix) {
            let after = &rest[start + prefix.len()..];
            if let Some(end) = after.find(quote) {
                let specifier = &after[..end];
                if specifier.starts_with('.') {
                    imports.push(specifier.to_string());
                }
                rest = &after[end + 1..];
            } else {
                break;
            }
        }
    }
    imports
}

fn resolve_typescript_candidate(importer: &Path, specifier: &str) -> Vec<PathBuf> {
    let base = importer
        .parent()
        .unwrap_or_else(|| Path::new("/"))
        .join(specifier);
    let mut candidates = vec![base.clone()];
    for extension in ["ts", "tsx", "d.ts", "mts", "cts"] {
        let mut path = base.as_os_str().to_os_string();
        path.push(".");
        path.push(extension);
        candidates.push(PathBuf::from(path));
    }
    for extension in ["ts", "tsx", "d.ts"] {
        candidates.push(base.join(format!("index.{extension}")));
    }
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler_host::CompileTarget;
    use ferrite_npm::{LockedImporter, LockedPackage};
    use std::time::Duration;

    fn locked_project() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        let package = LockedPackage {
            name: "@vue/compiler-sfc".into(),
            version: "3.5.22".into(),
            source: "npm".into(),
            ..Default::default()
        };
        let directory = root.path().join(".ferrite/npm/packages").join(package.id());
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("package.json"),
            r#"{"name":"@vue/compiler-sfc","version":"3.5.22"}"#,
        )
        .unwrap();
        let vue = LockedPackage {
            name: "vue".into(),
            version: "3.5.22".into(),
            source: "npm".into(),
            ..Default::default()
        };
        let mut importer = LockedImporter::default();
        importer
            .specifiers
            .insert("@vue/compiler-sfc".into(), "3.5.22".into());
        importer.specifiers.insert("vue".into(), "3.5.22".into());
        importer.dependencies.insert("vue".into(), vue.id());
        let mut lock = Lockfile::default();
        lock.package.push(package);
        lock.package.push(vue);
        lock.importers.insert(".".into(), importer);
        lock.write(&root.path().join("ferrite.lock")).unwrap();
        root
    }

    #[tokio::test]
    async fn embedded_vue_compiles_component_through_rquickjs() {
        let root = locked_project();
        let host = EmbeddedVueCompilerHost::new(
            root.path().into(),
            root.path().join("ferrite.lock"),
            Duration::from_secs(30),
        )
        .await
        .unwrap();
        let result = host
            .compile(CompileRequest {
                framework: Framework::Vue,
                filename: root.path().join("App.vue"),
                source: "<script setup>import { ref } from 'vue'; const count = ref(0);</script><template><button @click=\"count++\">{{ count }}</button></template><style scoped>button { color: red; }</style>".into(),
                target: CompileTarget::Client,
                development: true,
                module: false,
            })
            .await
            .unwrap();
        assert!(result.code.contains("count"), "{}", result.code);
        assert!(
            result.css.iter().any(|css| css.code.contains("button")),
            "{:?}",
            result.css
        );
    }

    #[test]
    fn typescript_candidates_preserve_dotted_basenames() {
        let candidates = resolve_typescript_candidate(Path::new("/src/App.vue"), "./props.types");
        assert!(candidates.contains(&PathBuf::from("/src/props.types.ts")));
        assert!(!candidates.contains(&PathBuf::from("/src/props.ts")));
    }

    #[test]
    fn typescript_candidates_cover_extensionless_and_index_imports() {
        let candidates = resolve_typescript_candidate(Path::new("/src/App.vue"), "./types/props");
        assert!(candidates.contains(&PathBuf::from("/src/types/props.ts")));
        assert!(candidates.contains(&PathBuf::from("/src/types/props/index.d.ts")));
    }
}
