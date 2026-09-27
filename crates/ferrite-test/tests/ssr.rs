//! Vite compat: SSR.

use ferrite_test::TempProject;

mod common;

use common::dev_server;

// --- SSR -------------------------------------------------------------------------------

#[tokio::test]
async fn ssr_load_module_traverses() {
    let project = TempProject::new(&[
        (
            "src/entry-server.ts",
            "import { render } from \"./view\";\nexport { render };\n",
        ),
        (
            "src/view.ts",
            "export function render(url: string): string {\n  return url;\n}\n",
        ),
    ]);
    let server = dev_server(&project).await;
    let module = server
        .ssr_load_module("/src/entry-server.ts")
        .await
        .unwrap();
    assert_eq!(module.id, "/src/entry-server.ts");
    assert!(module.dependencies.iter().any(|dep| dep.contains("view")));
    assert!(!module.code.contains("/@ferrite/client"), "{}", module.code);
}

#[test]
fn ssr_externalization_rules() {
    let config = ferrite::config::SsrConfig {
        external: vec!["pg".to_string()],
        no_external: vec!["my-esm-package".to_string()],
        ..Default::default()
    };
    assert!(ferrite::ssr::is_external("pg", &config));
    assert!(!ferrite::ssr::is_external("my-esm-package", &config));
    assert!(!ferrite::ssr::is_external("./local.ts", &config));
}
