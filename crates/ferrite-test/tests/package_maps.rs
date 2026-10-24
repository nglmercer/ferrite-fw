//! Checked package maps through the real compiler, bundler and embedded renderer.
#[cfg(feature = "napi-vm")]
#[tokio::test]
async fn emitted_import_maps_use_owner_edges_and_environment_conditions() {
    use ferrite::runtime::{JsRuntime, RuntimeEnvironment};
    let project = ferrite_test::TempProject::new(&[
        ("index.html", "<script type='module' src='/src/main.js'></script>"),
        ("src/main.js", "import {compute} from 'owner'; globalThis.value = compute();"),
        ("src/entry-server.js", "import {compute} from 'owner'; export function render() { return '<h1>'+compute()+'</h1>'; }"),
        (".ferrite/npm/packages/owner@1.0.0/package.json", r##"{"name":"owner","version":"1.0.0","exports":{".":{"browser":"./client.js","import":["../invalid.js","./server.js"]}},"imports":{"#math":"dep/feature","#local":"./local.js"}}"##),
        (".ferrite/npm/packages/owner@1.0.0/client.js", "export function compute() { return 'CLIENT_MAP'; }"),
        (".ferrite/npm/packages/owner@1.0.0/server.js", "import {value} from '#math'; import {local} from '#local'; export function compute() { return value + local; }"),
        (".ferrite/npm/packages/owner@1.0.0/local.js", "export const local = 2;"),
        (".ferrite/npm/packages/dep@1.0.0/package.json", r#"{"name":"dep","version":"1.0.0","exports":{"./feature":"./feature.js"}}"#),
        (".ferrite/npm/packages/dep@1.0.0/feature.js", "export const value = 10;"),
        (".ferrite/npm/packages/dep@2.0.0/package.json", r#"{"name":"dep","version":"2.0.0","exports":{"./feature":"./feature.js"}}"#),
        (".ferrite/npm/packages/dep@2.0.0/feature.js", "export const value = 20;"),
    ]);
    let dep = |version: &str| ferrite::npm::LockedPackage {
        name: "dep".into(),
        version: version.into(),
        source: "npm".into(),
        ..Default::default()
    };
    let owner = ferrite::npm::LockedPackage {
        name: "owner".into(),
        version: "1.0.0".into(),
        source: "npm".into(),
        dependencies: [("dep".into(), "dep@2.0.0".into())].into(),
        ..Default::default()
    };
    let lock = ferrite::npm::Lockfile {
        importers: [(
            ".".into(),
            ferrite::npm::LockedImporter {
                specifiers: [
                    ("owner".into(), "1.0.0".into()),
                    ("dep".into(), "1.0.0".into()),
                ]
                .into(),
                dependencies: [
                    ("owner".into(), owner.id()),
                    ("dep".into(), "dep@1.0.0".into()),
                ]
                .into(),
            },
        )]
        .into(),
        package: vec![owner, dep("1.0.0"), dep("2.0.0")],
        ..Default::default()
    };
    lock.write(&project.root.join("ferrite.lock")).unwrap();
    for hoist in [false, true] {
        let mut config = project.resolve_config_mode("production");
        config.runtime.backend = "napi-vm".into();
        config.build.scope_hoist = hoist;
        let reports = ferrite::Builder::new(config, vec![])
            .build_app()
            .await
            .unwrap();
        let client = &reports
            .iter()
            .find(|report| report.env == "client")
            .unwrap()
            .out_dir;
        let client_code = std::fs::read_dir(client.join("assets"))
            .unwrap()
            .filter_map(|entry| {
                let path = entry.unwrap().path();
                (path.extension().is_some_and(|ext| ext == "js"))
                    .then(|| std::fs::read_to_string(path).unwrap())
            })
            .collect::<String>();
        assert!(client_code.contains("CLIENT_MAP"), "{client_code}");
        let server = &reports
            .iter()
            .find(|report| report.env == "ssr")
            .unwrap()
            .out_dir;
        let artifact = ferrite::SsrRendererArtifact::read(server).unwrap();
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
        assert!(
            matches!(result, ferrite::runtime::JsValue::String(value) if value == "<h1>22</h1>")
        );
    }
}
