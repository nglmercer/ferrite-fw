//! napi-vm embedded backend (feature `napi-vm`, **disabled by default**).
//!
//! Runs SSR JavaScript in-process through
//! [napi-vm](https://github.com/nglmercer/napi-vm) (pure-Rust core, no Node
//! required) and loads real `.node` binaries through its in-process Node-API
//! backend (`node-api-host`, via
//! [`Interpreter::enable_native_addons`](https://github.com/nglmercer/napi-vm/blob/main/docs/node-addon-runtime-plan.md)
//! with `RustNodeApiOptions`).
//!
//! Because the interpreter is `!Send`, it lives on a dedicated OS thread;
//! [`NapiVmRuntime`] itself is `Send + Sync` and communicates over a job
//! queue. Only JSON-serializable values cross the bridge; functions cross
//! as [`crate::JsHandle`]s resolved through a guest-side function table.
//!
//! Setup follows the napi-vm contract (`docs/node-addon-runtime-plan.md`,
//! `src/interpreter/native_addon.rs`): one `enable_native_addons` call
//! installs the CommonJS loader, host bridge, and entry together, so this
//! backend never pre-installs a plain loader on the native path. The
//! returned [`NativeAddonRuntime`](https://github.com/nglmercer/napi-vm)
//! handle is retained for startup [`preflight`](https://github.com/nglmercer/napi-vm)
//! checks and for owner-thread [`shutdown`](https://github.com/nglmercer/napi-vm)
//! (finalizers/cleanup hooks) when the worker stops.
//!
//! Scope (inherited from napi-vm): the in-process host covers the **Node-API
//! C ABI** (including napi-rs addons that only use supported Node-API
//! symbols). Addons needing V8, NAN, Node C++, or libuv ABIs stay on
//! napi-vm's Node sidecar backend — napi-vm reports those as explicit
//! errors, never silent mis-execution.

mod jobs;
mod options;
mod runtime;
mod util;
mod worker;

pub use options::{NapiVmInfo, NapiVmOptions, NativeAddonAllow, NAPI_VM_PIN};
pub use runtime::NapiVmRuntime;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::napi_vm::util::{js_string, parse_sha256};
    use crate::{CompiledModule, JsRuntime, JsValue, RuntimeEnvironment};
    use std::path::PathBuf;

    #[test]
    fn js_string_escapes() {
        assert_eq!(js_string("a'b\\c"), "'a\\'b\\\\c'");
        assert_eq!(js_string("x\ny"), "'x\\ny'");
    }

    #[test]
    fn sha256_parses() {
        let hex = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let bytes = parse_sha256(hex).unwrap();
        assert_eq!(bytes[0], 0xe3);
        assert!(parse_sha256("zz").is_err());
        assert!(parse_sha256(&hex[..62]).is_err());
    }

    #[tokio::test]
    async fn evaluates_and_calls_exports() {
        let runtime = NapiVmRuntime::with_defaults();
        let namespace = runtime
            .evaluate_module(
                CompiledModule {
                    id: "entry".to_string(),
                    code: "export const answer = 40 + 2;\nexport function render(name) { return `hello ${name}`; }\n".to_string(),
                    url: None,
                },
                RuntimeEnvironment {
                    ssr: true,
                    request_id: None,
                },
            )
            .await
            .expect("evaluate");
        assert!(
            matches!(namespace.get("answer"), Some(JsValue::Number(value)) if *value == 42.0),
            "{:?}",
            namespace.exports
        );
        let handle = namespace
            .get_function("render")
            .expect("render handle")
            .clone();
        let result = runtime
            .call(&handle, vec![JsValue::String("ferrite".to_string())])
            .await
            .expect("call");
        assert!(
            matches!(result, JsValue::String(ref text) if text == "hello ferrite"),
            "{result:?}"
        );
    }

    #[tokio::test]
    async fn evaluates_explicit_compiled_dependency_graph_without_filesystem_sources() {
        let runtime = NapiVmRuntime::with_defaults();
        let graph = crate::CompiledModuleGraph {
            entry: "/compiled/entry.js".into(),
            modules: vec![
                CompiledModule { id: "/compiled/entry.js".into(), code: "import { value } from '/compiled/dependency.js'; export const answer = value + 1;".into(), url: None },
                CompiledModule { id: "/compiled/dependency.js".into(), code: "export const value = 41;".into(), url: None },
            ],
        };
        let namespace = runtime
            .evaluate_module_graph(
                graph.clone(),
                RuntimeEnvironment {
                    ssr: true,
                    request_id: None,
                },
            )
            .await
            .unwrap();
        assert!(matches!(namespace.get("answer"), Some(JsValue::Number(value)) if *value == 42.0));
        let mut updated = graph.clone();
        updated.modules[1].code = "export const value = 42;".into();
        let namespace = runtime
            .evaluate_module_graph(
                updated,
                RuntimeEnvironment {
                    ssr: true,
                    request_id: None,
                },
            )
            .await
            .unwrap();
        assert!(matches!(namespace.get("answer"), Some(JsValue::Number(value)) if *value == 43.0));
        let mut incomplete = graph.clone();
        incomplete.modules.pop();
        let error = runtime
            .evaluate_module_graph(
                incomplete,
                RuntimeEnvironment {
                    ssr: true,
                    request_id: None,
                },
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("dependency"), "{error}");
        // A failed replacement must not leave the previous dependency available.
        let namespace = runtime
            .evaluate_module_graph(
                graph.clone(),
                RuntimeEnvironment {
                    ssr: true,
                    request_id: None,
                },
            )
            .await
            .unwrap();
        assert!(matches!(namespace.get("answer"), Some(JsValue::Number(value)) if *value == 42.0));
        let mut invalid = graph.clone();
        invalid.modules.push(invalid.modules[0].clone());
        let error = runtime
            .evaluate_module_graph(
                invalid,
                RuntimeEnvironment {
                    ssr: true,
                    request_id: None,
                },
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("duplicate"));
        let mut invalid = graph;
        invalid.entry = "/compiled/missing.js".into();
        let error = runtime
            .evaluate_module_graph(
                invalid,
                RuntimeEnvironment {
                    ssr: true,
                    request_id: None,
                },
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("missing"));
    }

    #[tokio::test]
    async fn awaits_async_exports() {
        let runtime = NapiVmRuntime::with_defaults();
        let namespace = runtime
            .evaluate_module(
                CompiledModule {
                    id: "async-entry".to_string(),
                    code: "export async function fetch() { return { ok: true }; }\n".to_string(),
                    url: None,
                },
                RuntimeEnvironment {
                    ssr: true,
                    request_id: None,
                },
            )
            .await
            .expect("evaluate");
        let handle = namespace.get_function("fetch").expect("handle").clone();
        let result = runtime.call(&handle, Vec::new()).await.expect("call");
        assert!(
            matches!(&result, JsValue::Object(map) if matches!(map.get("ok"), Some(JsValue::Bool(true)))),
            "{result:?}"
        );
    }

    #[tokio::test]
    async fn call_arguments_cross_verbatim() {
        let runtime = NapiVmRuntime::with_defaults();
        let namespace = runtime
            .evaluate_module(
                CompiledModule {
                    id: "echo".to_string(),
                    code: "export function echo(x) { return [typeof x, x]; }\n".to_string(),
                    url: None,
                },
                RuntimeEnvironment {
                    ssr: true,
                    request_id: None,
                },
            )
            .await
            .expect("evaluate");
        let handle = namespace.get_function("echo").expect("handle").clone();
        let result = runtime
            .call(&handle, vec![JsValue::String("ferrite".to_string())])
            .await
            .expect("call");
        assert!(
            matches!(&result, JsValue::Array(items)
                if matches!(&items[..], [JsValue::String(t), JsValue::String(v)] if t == "string" && v == "ferrite")),
            "{result:?}"
        );
    }

    #[tokio::test]
    async fn relative_module_id_rejected_loudly() {
        let runtime = NapiVmRuntime::with_defaults();
        let error = runtime
            .evaluate_module(
                CompiledModule {
                    id: "./relative.js".to_string(),
                    code: "export const x = 1;\n".to_string(),
                    url: None,
                },
                RuntimeEnvironment {
                    ssr: true,
                    request_id: None,
                },
            )
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("must not be relative"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn absolute_path_id_evaluates() {
        let runtime = NapiVmRuntime::with_defaults();
        let namespace = runtime
            .evaluate_module(
                CompiledModule {
                    id: "/tmp/ferrite-smoke/src/entry-server.js".to_string(),
                    code: "export function render(url) { return url; }\n".to_string(),
                    url: None,
                },
                RuntimeEnvironment {
                    ssr: true,
                    request_id: None,
                },
            )
            .await
            .expect("absolute id");
        assert!(namespace.get_function("render").is_ok());
    }

    #[tokio::test]
    async fn reports_guest_errors() {
        let runtime = NapiVmRuntime::with_defaults();
        let error = runtime
            .evaluate_module(
                CompiledModule {
                    id: "broken".to_string(),
                    code: "export const x = ;;;\n".to_string(),
                    url: None,
                },
                RuntimeEnvironment {
                    ssr: true,
                    request_id: None,
                },
            )
            .await
            .unwrap_err();
        assert!(error.to_string().len() > 4, "{error}");
    }

    #[tokio::test]
    async fn native_requires_allowlist() {
        let runtime = NapiVmRuntime::with_defaults();
        let error = runtime
            .require("./addon.node", Some("/app/main.cjs"))
            .await
            .unwrap_err();
        // No loader configured at all: must fail loudly, never silently.
        assert!(error.to_string().len() > 4, "{error}");
    }

    #[tokio::test]
    async fn missing_allowlist_file_fails_loudly() {
        let options = NapiVmOptions {
            native_allow: vec![NativeAddonAllow {
                path: PathBuf::from("/nonexistent-ferrite addon/missing.node"),
                sha256_hex: None,
            }],
            ..Default::default()
        };
        let runtime = NapiVmRuntime::new(options);
        let error = runtime.require("./x", None).await.unwrap_err();
        assert!(
            error.to_string().contains("missing.node"),
            "unexpected error: {error}"
        );
    }

    #[tokio::test]
    async fn preflight_rejects_wrong_format_without_initializing() {
        let dir = std::env::temp_dir().join(format!("ferrite-preflight-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("main.cjs"), "module.exports = {};\n").unwrap();
        std::fs::write(dir.join("fake.node"), b"not-an-elf").unwrap();
        let options = NapiVmOptions {
            roots: vec![dir.clone()],
            entry: Some(dir.join("main.cjs").to_string_lossy().into_owned()),
            native_allow: vec![NativeAddonAllow {
                path: dir.join("fake.node"),
                sha256_hex: None,
            }],
            ..Default::default()
        };
        let runtime = NapiVmRuntime::new(options);
        let error = runtime.require("./main.cjs", None).await.unwrap_err();
        assert!(error.to_string().len() > 8, "unexpected error: {error}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn relative_allow_resolves_against_first_root() {
        let dir = std::env::temp_dir().join(format!("ferrite-relallow-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("main.cjs"), "module.exports = {};\n").unwrap();
        std::fs::write(dir.join("fake.node"), b"not-an-elf").unwrap();
        let options = NapiVmOptions {
            roots: vec![dir.clone()],
            entry: Some(dir.join("main.cjs").to_string_lossy().into_owned()),
            native_allow: vec![NativeAddonAllow {
                path: PathBuf::from("fake.node"),
                sha256_hex: None,
            }],
            ..Default::default()
        };
        let runtime = NapiVmRuntime::new(options);
        let error = runtime.require("./main.cjs", None).await.unwrap_err();
        // Resolution hit the file (no "cannot allow"/not-found); the failure
        // is the later preflight format check.
        assert!(
            !error.to_string().contains("cannot allow")
                && !error.to_string().contains("No such file"),
            "unexpected error: {error}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn bad_napi_ceiling_is_rejected() {
        let dir = std::env::temp_dir().join(format!("ferrite-napiver-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("main.cjs"), "module.exports = {};\n").unwrap();
        std::fs::write(dir.join("fake.node"), b"not-an-elf").unwrap();
        let options = NapiVmOptions {
            roots: vec![dir.clone()],
            entry: Some(dir.join("main.cjs").to_string_lossy().into_owned()),
            native_allow: vec![NativeAddonAllow {
                path: dir.join("fake.node"),
                sha256_hex: None,
            }],
            max_napi_version: 99,
            ..Default::default()
        };
        let runtime = NapiVmRuntime::new(options);
        let error = runtime.require("./main.cjs", None).await.unwrap_err();
        assert!(
            error.to_string().contains("99"),
            "unexpected error: {error}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn unlisted_native_is_rejected() {
        let dir = std::env::temp_dir().join(format!("ferrite-native-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("main.cjs"), "module.exports = {};\n").unwrap();
        std::fs::write(dir.join("evil.node"), b"not-an-elf").unwrap();
        let options = NapiVmOptions {
            roots: vec![dir.clone()],
            entry: Some(dir.join("main.cjs").to_string_lossy().into_owned()),
            ..Default::default()
        };
        let runtime = NapiVmRuntime::new(options);
        let parent = dir.join("main.cjs").to_string_lossy().into_owned();
        let error = runtime
            .require("./evil.node", Some(&parent))
            .await
            .unwrap_err();
        assert!(
            error.to_string().to_lowercase().contains("allow")
                || error.to_string().to_lowercase().contains("denied")
                || error.to_string().to_lowercase().contains("native"),
            "unexpected error: {error}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
