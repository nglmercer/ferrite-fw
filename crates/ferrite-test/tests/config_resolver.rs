//! Shared configuration and resolver conformance.

use ferrite::server::DevServer;
use ferrite::ModuleId;
use ferrite_test::assert_contains_all;
use ferrite_test::TempProject;

mod common;

use common::dev_server;

// --- config ------------------------------------------------------------------

#[test]
fn config_defaults_and_precedence() {
    let project = TempProject::new(&[("ferrite.toml", "[server]\nport = 3000\n")]);
    let user = ferrite::load_user_config(&project.root).unwrap();
    assert_eq!(user.server.port, 3000);
    // CLI overrides win.
    let resolved = ferrite::resolve_config(
        user,
        Some(project.root.clone()),
        ferrite::CliOverrides {
            port: Some(4000),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(resolved.server.port, 4000);
    assert_eq!(resolved.base, "/");
}

// --- resolver -----------------------------------------------------------------

#[tokio::test]
async fn resolver_relative_and_alias() {
    let project = TempProject::new(&[
        ("src/main.ts", "import x from \"./x\";\nconsole.log(x);\n"),
        ("src/x.ts", "export default 1;"),
        ("src/deep/y.ts", "export default 2;"),
    ]);
    let mut user = ferrite::UserConfig::default();
    user.resolve
        .alias
        .insert("@".to_string(), "/src".to_string());
    let resolved = ferrite::resolve_config(
        user,
        Some(project.root.clone()),
        ferrite::CliOverrides::default(),
    )
    .unwrap();
    let server = DevServer::new_without_watcher(resolved, vec![])
        .await
        .unwrap();
    let importer = ModuleId::new("/src/main.ts");
    let relative = server
        .pipeline_module(&ModuleId::new("/src/main.ts"), Some(&importer), "client")
        .await
        .unwrap();
    assert!(relative.code.contains("/src/x.ts"), "{}", relative.code);
    let aliased = server.resolve_entry("@/deep/y.ts", "client").await.unwrap();
    assert_eq!(aliased.0, "/src/deep/y.ts");
}

#[tokio::test]
async fn resolver_bare_exports_conditions() {
    let project = TempProject::new(&[
        (
            ".ferrite/npm/packages/greet@1.0.0/package.json",
            r#"{"name":"greet","version":"1.0.0","exports":{".":"./index.js","./feature":"./feature.js"}}"#,
        ),
        (
            ".ferrite/npm/packages/greet@1.0.0/index.js",
            "export default \"hi\";",
        ),
        (
            ".ferrite/npm/packages/greet@1.0.0/feature.js",
            "export const f = 1;",
        ),
        (
            "src/main.ts",
            "import g from \"greet\";\nimport { f } from \"greet/feature\";\nconsole.log(g, f);",
        ),
    ]);
    let server = dev_server(&project).await;
    let main = server
        .pipeline_module(&ModuleId::new("/src/main.ts"), None, "client")
        .await
        .unwrap();
    assert_contains_all(
        &main.code,
        &["/@npm/greet@1.0.0/index.js", "/@npm/greet@1.0.0/feature.js"],
    );
}

#[tokio::test]
async fn resolver_missing_bare_package_fails_loudly() {
    // A bare import no browser can resolve must never be served as-is:
    // the pipeline fails with the install hint instead of a cryptic
    // client-side TypeError.
    let project = TempProject::new(&[(
        "src/main.ts",
        "import { x } from \"no-such-pkg/deep\";\nconsole.log(x);\n",
    )]);
    let server = dev_server(&project).await;
    let error = server
        .pipeline_module(&ModuleId::new("/src/main.ts"), None, "client")
        .await
        .unwrap_err();
    let message = error.to_string();
    assert!(message.contains("no-such-pkg"), "{message}");
    assert!(message.contains("ferrite add no-such-pkg"), "{message}");
}

#[tokio::test]
async fn resolver_disabled_remote_import_fails_with_opt_in_hint() {
    // Disabled remote loading must fail in the shared pipeline rather than
    // bypassing its explicit configuration through browser passthrough.
    let project = TempProject::new(&[(
        "src/main.ts",
        "import x from \"https://esm.example/mod.js\";\nconsole.log(x);\n",
    )]);
    let server = dev_server(&project).await;
    let error = server
        .pipeline_module(&ModuleId::new("/src/main.ts"), None, "client")
        .await
        .unwrap_err();
    let message = error.to_string();
    assert!(message.contains("https://esm.example/mod.js"), "{message}");
    assert!(message.contains("disabled"), "{message}");
    assert!(message.contains("enable `[remote]`"), "{message}");
}
