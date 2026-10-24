//! Route dispatch executes emitted graphs; unrelated lazy modules never evaluate.
#![cfg(feature = "napi-vm")]
use ferrite::ssr::{SsrAdapter, SsrContext, SsrHttpRequest};
use std::sync::Arc;
fn project() -> ferrite_test::TempProject {
    ferrite_test::TempProject::new(&[
        ("index.html", "<html><head></head><body><!--ssr-outlet--><script type='module' src='/src/main.js'></script></body></html>"),
        ("src/main.js", "globalThis.client=true;"),
        ("src/default.ts", "export function render(url, request) {return {html:`fallback ${request.method} ${url}`,status:202,headers:[['x-route','fallback']]};}"),
        ("src/admin.ts", "globalThis.adminEvaluations=(globalThis.adminEvaluations||0)+1; export function render(url) {return `<h1>admin ${url} ${globalThis.adminEvaluations}</h1>`;}"),
        ("src/exact.ts", "export function render() {return '<h1>exact</h1>';}"),
        ("src/broken.ts", "throw new Error('selected route failed'); export function render() {return 'never';}"),
    ])
}
fn config(project: &ferrite_test::TempProject) -> ferrite::ResolvedConfig {
    let mut config = project.resolve_config_mode("production");
    config.runtime.backend = "napi-vm".into();
    config.base = "/app/".into();
    config.ssr.entry = Some("src/default.ts".into());
    config.ssr.entries = [
        ("/admin/*", "src/admin.ts"),
        ("/admin/report.json", "src/exact.ts"),
        ("/admin/reports/*", "src/exact.ts"),
        ("/broken", "src/broken.ts"),
    ]
    .into_iter()
    .map(|(route, entry)| ferrite::config::SsrRouteEntry {
        shell: None,
        route: route.into(),
        entry: entry.into(),
    })
    .collect();
    config
}
async fn render(adapter: &dyn SsrAdapter, url: &str) -> ferrite::Result<ferrite::ssr::SsrResponse> {
    adapter
        .render(
            SsrHttpRequest {
                method: "POST".into(),
                uri: url.into(),
                headers: vec![],
                body: vec![],
            },
            SsrContext {
                url: url.into(),
                ..Default::default()
            },
        )
        .await
}
async fn assertions(adapter: &dyn SsrAdapter) {
    let response = render(adapter, "/app/other?x=1").await.unwrap();
    assert_eq!(response.status, 202);
    assert!(response
        .headers
        .contains(&("x-route".into(), "fallback".into())));
    assert!(response
        .into_string()
        .await
        .unwrap()
        .contains("fallback POST /app/other?x=1"));
    for path in ["/app/admin", "/app/admin/nested"] {
        assert!(render(adapter, path)
            .await
            .unwrap()
            .into_string()
            .await
            .unwrap()
            .contains(&format!("admin {path} 1")));
    }
    assert!(render(adapter, "/app/admin/report.json")
        .await
        .unwrap()
        .into_string()
        .await
        .unwrap()
        .contains("<h1>exact</h1>"));
    assert!(render(adapter, "/app/admin/reports/a")
        .await
        .unwrap()
        .into_string()
        .await
        .unwrap()
        .contains("<h1>exact</h1>"));
    assert!(render(adapter, "/app/admin-other")
        .await
        .unwrap()
        .into_string()
        .await
        .unwrap()
        .contains("fallback"));
    assert!(render(adapter, "/app/broken")
        .await
        .unwrap_err()
        .to_string()
        .contains("selected route failed"));
    assert!(render(adapter, "/app/admin")
        .await
        .unwrap()
        .into_string()
        .await
        .unwrap()
        .contains("admin /app/admin 1"));
}
#[tokio::test]
async fn route_entries_dev_build_and_prerender_share_dispatch() {
    let project = project();
    let config = config(&project);
    let server = ferrite::DevServer::new_without_watcher(config.clone(), vec![])
        .await
        .unwrap();
    let adapter =
        ferrite::create_dev_ssr_adapter(&server, &config, "<main><!--ssr-outlet--></main>")
            .await
            .unwrap();
    assertions(adapter.as_ref()).await;
    std::fs::write(
        project.root.join("src/exact.ts"),
        "export function render(){return '<h1>updated</h1>'}",
    )
    .unwrap();
    assert!(render(adapter.as_ref(), "/app/admin/report.json")
        .await
        .unwrap()
        .into_string()
        .await
        .unwrap()
        .contains("updated"));
    for scope_hoist in [false, true] {
        let mut config = config.clone();
        config.build.scope_hoist = scope_hoist;
        let builder = ferrite::Builder::new(config, vec![]);
        let reports = builder.build_app().await.unwrap();
        let artifact = ferrite::SsrRendererArtifact::read(&reports[1].out_dir).unwrap();
        let built = artifact
            .into_adapter(
                Arc::new(ferrite::runtime::napi_vm::NapiVmRuntime::with_defaults()),
                "<!--ssr-outlet-->".into(),
                "/app/",
            )
            .unwrap();
        assert!(render(built.as_ref(), "/app/admin")
            .await
            .unwrap()
            .into_string()
            .await
            .unwrap()
            .contains("admin /app/admin 1"));
        assert!(render(built.as_ref(), "/app/admin/report.json")
            .await
            .unwrap()
            .into_string()
            .await
            .unwrap()
            .contains("updated"));
        let report = builder
            .prerender(&["/admin/report.json".into(), "/admin".into()])
            .await
            .unwrap();
        assert_eq!(report.routes.len(), 2);
        assert!(
            std::fs::read_to_string(project.root.join("dist/admin/report.json/index.html"))
                .unwrap()
                .contains("updated")
        );
    }
}
#[tokio::test]
async fn no_fallback_returns_404_and_route_modules_are_ssr_only() {
    let project = project();
    let mut config = config(&project);
    config.ssr.entry = None;
    let server = ferrite::DevServer::new_without_watcher(config.clone(), vec![])
        .await
        .unwrap();
    assert!(server
        .resolve_entry("/@ferrite/ssr-router", "client")
        .await
        .is_err());
    let adapter = ferrite::create_dev_ssr_adapter(&server, &config, "<!--ssr-outlet-->")
        .await
        .unwrap();
    assert_eq!(
        render(adapter.as_ref(), "/app/missing")
            .await
            .unwrap()
            .status,
        404
    );
    assert!(!config.ssr.matches_route("/src/admin.ts"));
    assert!(config.ssr.matches_route("/admin/report.json"));
}
