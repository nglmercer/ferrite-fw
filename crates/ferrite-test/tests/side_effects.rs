//! Production behavior of package sideEffects metadata, including emitted execution.
use ferrite_test::TempProject;

fn project(npm: bool) -> TempProject {
    let files = [
        ("index.html", "<html><head></head><body><script type=\"module\" src=\"/client.js\"></script></body></html>"),
        ("client.js", "globalThis.ready = true;"),
        ("src/entry-server.js", "import './lib/drop.js'; import './lib/dynamic.js'; import {kept} from './lib/barrel.js'; import {namespace} from './lib/namespace-barrel.js'; import './lib/polyfill.js'; import './unknown.js'; import './lib/theme.css'; export async function render() { const before = (globalThis.effects || []).slice(); const {dynamic} = await import('./lib/dynamic.js'); return JSON.stringify({kept, extra: namespace.extra, dynamic, before, effects: globalThis.effects || []}); }"),
        ("src/lib/package.json", r#"{"name":"effect-fixture","version":"1.0.0","sideEffects":["./polyfill.js","*.css"]}"#),
        ("src/lib/drop.js", "import './child.js'; (globalThis.effects ||= []).push('DROP_ROOT');"),
        ("src/lib/child.js", "(globalThis.effects ||= []).push('DROP_CHILD');"),
        ("src/lib/barrel.js", "export {kept} from './used.js'; export {unused} from './unused.js';"),
        ("src/lib/used.js", "export const kept = 42; export const extra = 11;"),
        ("src/lib/namespace-barrel.js", "export * as namespace from './used.js';"),
        ("src/lib/unused.js", "import './unused.css'; (globalThis.effects ||= []).push('UNUSED_EXPORT'); export const unused = 99;"),
        ("src/lib/unused.css", ".unused-only { color: magenta; }"),
        ("src/lib/polyfill.js", "(globalThis.effects ||= []).push('POLYFILL');"),
        ("src/unknown.js", "(globalThis.effects ||= []).push('UNKNOWN');"),
        ("src/lib/dynamic.js", "(globalThis.effects ||= []).push('DYNAMIC_EVAL'); export const dynamic = 7;"),
        ("src/lib/theme.css", ".kept-theme { color: rgb(128, 0, 0); }"),
    ];
    let files: Vec<(String, String)> = files
        .into_iter()
        .map(|(path, code)| {
            if npm {
                (
                    path.replacen("src/lib/", ".ferrite/npm/packages/effect-fixture@1.0.0/", 1),
                    if path == "src/entry-server.js" {
                        code.replace("./lib/", "effect-fixture/")
                    } else {
                        code.into()
                    },
                )
            } else {
                (path.into(), code.into())
            }
        })
        .collect();
    let inputs: Vec<_> = files
        .iter()
        .map(|(path, code)| (path.as_str(), code.as_str()))
        .collect();
    let project = TempProject::new(&inputs);
    if npm {
        let lock = ferrite::npm::Lockfile {
            importers: [(
                ".".into(),
                ferrite::npm::LockedImporter {
                    specifiers: [("effect-fixture".into(), "1.0.0".into())].into(),
                    dependencies: [("effect-fixture".into(), "effect-fixture@1.0.0".into())].into(),
                },
            )]
            .into(),
            package: vec![ferrite::npm::LockedPackage {
                name: "effect-fixture".into(),
                version: "1.0.0".into(),
                source: "npm".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        lock.write(&project.root.join("ferrite.lock")).unwrap();
    }
    project
}

async fn build(project: &TempProject, treeshake: bool, hoist: bool) -> ferrite::BuildReport {
    let mut config = project.resolve_config_mode("production");
    config.ssr.entry = Some("src/entry-server.js".into());
    config.runtime.backend = "napi-vm".into();
    config.build.minify = treeshake;
    config.build.scope_hoist = hoist;
    ferrite::Builder::new(config, vec![])
        .build_app()
        .await
        .unwrap()
        .into_iter()
        .find(|report| report.env == "ssr")
        .unwrap()
}

#[tokio::test]
async fn package_metadata_prunes_real_modules_and_assets_in_each_production_mode() {
    for (npm, hoist) in [(false, false), (false, true), (true, false), (true, true)] {
        for treeshake in [false, true] {
            let project = project(npm);
            let report = build(&project, treeshake, hoist).await;
            let manifest = ferrite::SsrRendererArtifact::read(&report.out_dir).unwrap();
            let code = manifest
                .graph
                .modules
                .iter()
                .map(|module| module.code.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            assert_eq!(
                code.contains("DROP_ROOT"),
                !treeshake,
                "{hoist}/{treeshake}: {code}"
            );
            assert_eq!(
                code.contains("DROP_CHILD"),
                !treeshake,
                "{hoist}/{treeshake}: {code}"
            );
            assert_eq!(
                code.contains("UNUSED_EXPORT"),
                !treeshake,
                "{hoist}/{treeshake}: {code}"
            );
            assert!(
                code.contains("POLYFILL")
                    && code.contains("UNKNOWN")
                    && code.contains("DYNAMIC_EVAL"),
                "{code}"
            );
            let styles = manifest
                .stylesheets
                .iter()
                .map(|style| std::fs::read_to_string(report.out_dir.join(style)).unwrap())
                .collect::<String>();
            assert!(styles.contains("kept-theme"), "{styles}");
            assert_eq!(styles.contains("unused-only"), !treeshake, "{styles}");
            if treeshake {
                assert!(report.dropped >= 3, "{report:?}");
            }
        }
    }
}

#[cfg(feature = "napi-vm")]
#[tokio::test]
async fn emitted_graph_executes_required_effects_and_preserves_dynamic_boundaries() {
    use ferrite::runtime::{JsRuntime, RuntimeEnvironment};
    for (npm, hoist) in [(false, false), (false, true), (true, false), (true, true)] {
        for treeshake in [false, true] {
            let project = project(npm);
            let report = build(&project, treeshake, hoist).await;
            let artifact = ferrite::SsrRendererArtifact::read(&report.out_dir).unwrap();
            let runtime = ferrite::runtime::napi_vm::NapiVmRuntime::with_defaults();
            let result = runtime
                .invoke_module_graph(
                    artifact.graph,
                    "render",
                    vec![],
                    RuntimeEnvironment {
                        ssr: true,
                        request_id: None,
                    },
                )
                .await
                .unwrap();
            let ferrite::runtime::JsValue::String(result) = result else {
                panic!("renderer must return JSON text")
            };
            let result: serde_json::Value = serde_json::from_str(&result).unwrap();
            assert_eq!(result["kept"], 42);
            assert_eq!(result["extra"], 11);
            assert_eq!(result["dynamic"], 7);
            let effects = result["effects"].as_array().unwrap();
            assert_eq!(
                result["before"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|value| value.as_str() == Some("DYNAMIC_EVAL")),
                !treeshake,
                "{result}"
            );
            for effect in ["POLYFILL", "UNKNOWN", "DYNAMIC_EVAL"] {
                assert_eq!(
                    effects
                        .iter()
                        .filter(|value| value.as_str() == Some(effect))
                        .count(),
                    1,
                    "{result}"
                );
            }
            for effect in ["DROP_ROOT", "DROP_CHILD", "UNUSED_EXPORT"] {
                assert_eq!(
                    effects.iter().any(|value| value.as_str() == Some(effect)),
                    !treeshake,
                    "{result}"
                );
            }
        }
    }
}
