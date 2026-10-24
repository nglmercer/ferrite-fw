//! Live local dependency identities through development and emitted SSR graphs.
#[cfg(feature = "napi-vm")]
#[tokio::test]
async fn local_sources_build_render_and_update_without_reinstalling() {
    use ferrite::ssr::SsrAdapter;
    use std::sync::Arc;
    let project = ferrite_test::TempProject::new(&[
        ("package.json", r#"{"workspaces":["packages/*"],"dependencies":{"app":"workspace:*"}}"#),
        ("index.html", "<main><!--ssr-outlet--></main><script type='module' src='/src/main.ts'></script>"),
        ("src/main.ts", "import { value } from 'app'; globalThis.value = value;"),
        ("src/entry-server.ts", "import { value } from 'app'; export function render() { return '<h1>'+value+'</h1>'; }"),
        ("packages/app/package.json", r#"{"name":"app","version":"1.0.0","exports":"./index.ts","dependencies":{"shared":"workspace:*","offset":"file:../../local"}}"#),
        ("packages/app/index.ts", "import {value as shared} from 'shared'; import {offset} from 'offset'; export const value: number = shared + offset;"),
        ("packages/shared/package.json", r#"{"name":"shared","version":"1.0.0","exports":"./index.ts"}"#),
        ("packages/shared/index.ts", "export const value: number = 40;"),
        ("local/package.json", r#"{"name":"actual-offset","version":"1.0.0","exports":"./index.ts"}"#),
        ("local/index.ts", "export const offset: number = 2;"),
    ]);
    let npm = project.root.join(".ferrite/npm");
    let installer = ferrite::npm::Installer::new(
        ferrite::npm::RegistryClient::new("http://127.0.0.1:9", npm.join("metadata")).unwrap(),
        npm,
    );
    let mut lock = ferrite::npm::Lockfile::default();
    installer
        .install_project(&project.root, &mut lock, false)
        .await
        .unwrap();
    lock.write(&project.root.join("ferrite.lock")).unwrap();
    ferrite::npm::project_editor_dependencies(&project.root, &lock).unwrap();
    let mut config = project.resolve_config_mode("development");
    config.runtime.backend = "napi-vm".into();
    let server = ferrite::DevServer::new_without_watcher(config.clone(), vec![])
        .await
        .unwrap();
    let adapter =
        ferrite::create_dev_ssr_adapter(&server, &config, "<main><!--ssr-outlet--></main>")
            .await
            .unwrap();
    async fn render(adapter: &dyn SsrAdapter) -> String {
        adapter
            .render(
                ferrite::ssr::SsrHttpRequest {
                    method: "GET".into(),
                    uri: "/".into(),
                    headers: vec![],
                    body: vec![],
                },
                Default::default(),
            )
            .await
            .unwrap()
            .into_string()
            .await
            .unwrap()
    }
    assert!(render(adapter.as_ref()).await.contains("<h1>42</h1>"));
    std::fs::write(
        project.root.join("packages/shared/index.ts"),
        "export const value: number = 50;",
    )
    .unwrap();
    assert!(render(adapter.as_ref()).await.contains("<h1>52</h1>"));
    let before = lock.clone();
    installer
        .install_project(&project.root, &mut lock, true)
        .await
        .unwrap();
    assert_eq!(lock, before);
    let mut config = project.resolve_config_mode("production");
    config.runtime.backend = "napi-vm".into();
    let reports = ferrite::Builder::new(config, vec![])
        .build_app()
        .await
        .unwrap();
    let server = reports.iter().find(|report| report.env == "ssr").unwrap();
    let artifact = ferrite::SsrRendererArtifact::read(&server.out_dir).unwrap();
    let runtime = Arc::new(ferrite::runtime::napi_vm::NapiVmRuntime::with_defaults());
    let adapter = artifact
        .into_adapter(runtime, "<main><!--ssr-outlet--></main>".into(), "/")
        .unwrap();
    assert!(render(adapter.as_ref()).await.contains("<h1>52</h1>"));
}
