//! Shared pipeline source maps, CJS, node shims, TS/JSX and diagnostics.

use ferrite::ModuleId;
use ferrite::ModuleType;
use ferrite_test::assert_contains_all;
use ferrite_test::TempProject;

mod common;

use common::dev_server;

// --- source maps ----------------------------------------------------------------------------

#[test]
fn source_maps_produced_on_request() {
    let compiler = ferrite::transform::OxcCompiler::new(Default::default());
    let mut request = ferrite::transform::TransformRequest::new(
        "/src/a.ts",
        "const x: number = 1;\nexport default x;\n",
        ModuleType::Ts,
    );
    request.sourcemap = true;
    let result = ferrite::transform::JsCompiler::transform(&compiler, request).unwrap();
    let map = result.map.expect("map");
    assert!(map.mappings.contains("mappings") || map.mappings.contains("version"));
}

// --- CJS --------------------------------------------------------------------------------------

#[tokio::test]
async fn cjs_converted_to_esm_wrapper() {
    let project = TempProject::new(&[
        ("dep.cjs", "module.exports = 41;\n"),
        (
            "legacy.cjs",
            "const dep = require(\"./dep.cjs\");\nmodule.exports = { dep };\n",
        ),
    ]);
    let server = dev_server(&project).await;
    let module = server
        .pipeline_module(&ModuleId::new("/legacy.cjs"), None, "client")
        .await
        .unwrap();
    assert_contains_all(
        &module.code,
        &[
            "__ferrite_cjs_require__",
            "?ferrite-cjs-factory",
            "module.exports",
            "export default",
            "__ferrite_value[\"dep\"]",
        ],
    );
    let factory = server
        .pipeline_module(
            &ModuleId::new("/legacy.cjs?ferrite-cjs-factory"),
            None,
            "client",
        )
        .await
        .unwrap();
    assert!(
        factory
            .imports
            .iter()
            .any(|(_, dependency, _)| dependency.0.contains("dep.cjs?ferrite-cjs-factory")),
        "{:?}",
        factory.imports
    );
}

// --- node compat ---------------------------------------------------------------------------------

#[tokio::test]
async fn synchronous_require_of_esm_fails_with_import_hint() {
    let project = TempProject::new(&[
        ("dep.js", "export default 41;\n"),
        ("legacy.cjs", "module.exports = require('./dep.js');\n"),
    ]);
    let server = dev_server(&project).await;
    let factory = server
        .pipeline_module(
            &ModuleId::new("/legacy.cjs?ferrite-cjs-factory"),
            None,
            "client",
        )
        .await
        .unwrap();
    let dependency = factory
        .imports
        .iter()
        .find(|(_, dependency, _)| dependency.0.starts_with("/dep.js?"))
        .expect("literal require must resolve its actual dependency factory");
    let message = server
        .pipeline_module(&dependency.1, None, "client")
        .await
        .unwrap_err()
        .to_string();
    assert!(message.contains("synchronous require(ESM)"), "{message}");
    assert!(message.contains("use ESM imports"), "{message}");
}

#[tokio::test]
async fn node_builtin_shims_in_browser() {
    let project = TempProject::new(&[(
        "src/main.ts",
        "import path from \"node:path\";\nconsole.log(path.sep);\n",
    )]);
    let server = dev_server(&project).await;
    let main = server
        .pipeline_module(&ModuleId::new("/src/main.ts"), None, "client")
        .await
        .unwrap();
    assert!(main.code.contains("/@id/node:path"), "{}", main.code);
    let shim = server
        .pipeline_module(&ModuleId::new("/@id/node:path"), None, "client")
        .await
        .unwrap();
    assert!(shim.code.contains("sep"), "{}", shim.code);
}

// --- TS/JSX ----------------------------------------------------------------------------------------

#[tokio::test]
async fn tsx_automatic_runtime() {
    let project = TempProject::new(&[
        (
            "src/app.tsx",
            "export const App = () => <div className=\"a\">hi</div>;\n",
        ),
        (
            ".ferrite/npm/packages/react@18.0.0/package.json",
            r#"{"name":"react","version":"18.0.0","exports":{".":"./index.js","./jsx-dev-runtime":"./jsx-dev-runtime.js"}}"#,
        ),
        (
            ".ferrite/npm/packages/react@18.0.0/jsx-dev-runtime.js",
            "export function jsxDEV() {}\n",
        ),
    ]);
    let server = dev_server(&project).await;
    let module = server
        .pipeline_module(&ModuleId::new("/src/app.tsx"), None, "client")
        .await
        .unwrap();
    assert!(!module.code.contains("<div"), "{}", module.code);
    assert!(module.code.contains("jsx"), "{}", module.code);
}

// --- errors ------------------------------------------------------------------------------------------

#[tokio::test]
async fn jsx_ownership_uses_package_boundaries_and_invalidates_manifest_changes() {
    let project = TempProject::new(&[
        ("package.json", r#"{"dependencies":{"react":"19.2.0"}}"#),
        ("src/app.jsx", "export const App = () => <div />;"),
        ("packages/new/app.jsx", "export const App = () => <div />;"),
        (
            "packages/widget/package.json",
            r#"{"peerDependencies":{"preact":"10.0.0"}}"#,
        ),
        (
            "packages/widget/app.tsx",
            "export const App = () => <div />;",
        ),
    ]);
    let mut config = project.resolve_config();
    config.react.runtime = "classic".into();
    let server = ferrite::server::DevServer::new_without_watcher(config.clone(), vec![])
        .await
        .unwrap();
    let id = ModuleId::new("/src/app.jsx");
    let first = server.pipeline_module(&id, None, "client").await.unwrap();
    assert!(first.code.contains("React.createElement"));
    assert!(first
        .dependencies
        .iter()
        .any(|path| path.ends_with("package.json")));
    let nested = ModuleId::new("/packages/new/app.jsx");
    server
        .pipeline_module(&nested, None, "client")
        .await
        .unwrap();
    std::fs::write(
        project.root.join("packages/new/package.json"),
        r#"{"devDependencies":{"solid-js":"1.0.0"}}"#,
    )
    .unwrap();
    let error = server
        .pipeline_module(&nested, None, "client")
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("unavailable (solid-js)"), "{error}");
    let error = server
        .pipeline_module(&ModuleId::new("/packages/widget/app.tsx"), None, "client")
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("unavailable (preact)"), "{error}");
    assert!(error.contains("packages/widget/package.json"), "{error}");

    std::fs::write(
        project.root.join("package.json"),
        r#"{"dependencies":{"react":"19.2.0","solid-js":"1.0.0"}}"#,
    )
    .unwrap();
    let error = server
        .pipeline_module(&id, None, "client")
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("ambiguous (react, solid-js)"), "{error}");

    config.framework = Some(ferrite::config::FrameworkConfig {
        enabled: vec!["react".into()],
        compiler_host: Some("native".into()),
        ..Default::default()
    });
    let explicit = ferrite::server::DevServer::new_without_watcher(config, vec![])
        .await
        .unwrap();
    assert!(explicit
        .pipeline_module(&id, None, "client")
        .await
        .unwrap()
        .code
        .contains("React.createElement"));
}

#[tokio::test]
async fn explicit_framework_exclusion_rejects_unowned_jsx_in_all_targets() {
    let project = TempProject::new(&[
        ("src/app.jsx", "export const App = () => <div />;"),
        ("src/app.tsx", "export const App = () => <div />;"),
        ("src/plain.ts", "export const value: number = 1;"),
    ]);
    for (production, owners) in [
        (false, vec![]),
        (true, vec![]),
        (false, vec!["vue".to_string()]),
        (true, vec!["svelte".to_string()]),
    ] {
        let mut config = project.resolve_config();
        config.framework = Some(ferrite::config::FrameworkConfig {
            compiler_host: (!owners.is_empty()).then(|| "node".into()),
            enabled: owners,
            ..Default::default()
        });
        config.is_production = production;
        let server = ferrite::server::DevServer::new_without_watcher(config, vec![])
            .await
            .unwrap();
        for environment in ["client", "ssr"] {
            for path in ["/src/app.jsx", "/src/app.tsx"] {
                let error = server
                    .pipeline_module(&ModuleId::new(path), None, environment)
                    .await
                    .unwrap_err();
                let message = error.to_string();
                assert!(
                    message.contains("JSX has no enabled framework owner"),
                    "{message}"
                );
                assert!(message.contains("compiler_host"), "{message}");
                assert!(message.contains(path), "{message}");
            }
            let plain = server
                .pipeline_module(&ModuleId::new("/src/plain.ts"), None, environment)
                .await
                .unwrap();
            assert!(!plain.code.contains(": number"));
        }
    }
}

#[tokio::test]
async fn parse_errors_carry_diagnostics() {
    let project = TempProject::new(&[("src/bad.ts", "const = ;;;\n")]);
    let server = dev_server(&project).await;
    let error = server
        .pipeline_module(&ModuleId::new("/src/bad.ts"), None, "client")
        .await
        .unwrap_err();
    let diagnostic = error.diagnostic();
    assert_eq!(diagnostic.code, "FERRITE_PARSE_001");
    assert!(diagnostic.id.is_some());
}
