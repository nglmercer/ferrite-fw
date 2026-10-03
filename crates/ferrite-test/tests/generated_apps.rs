//! Actual generated-app CLI acceptance. Explicitly execute; never skip missing tools.
use ferrite_e2e::{Browser, BrowserKind, LaunchOptions};
use std::path::{Path, PathBuf};
use std::time::Duration;

fn cli() -> PathBuf {
    std::env::var_os("FERRITE_CLI_PATH")
        .map(PathBuf::from)
        .expect("set FERRITE_CLI_PATH to the freshly built Ferrite CLI")
        .canonicalize()
        .unwrap()
}
async fn command(binary: &Path, root: &Path, args: &[&str], node_enabled: bool) {
    let output = tokio::process::Command::new(binary)
        .args(args)
        .current_dir(root)
        .env(
            "PATH",
            if node_enabled {
                std::env::var_os("PATH").unwrap_or_default()
            } else {
                std::ffi::OsString::new()
            },
        )
        .kill_on_drop(true)
        .output()
        .await
        .unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
async fn server(
    binary: &Path,
    root: &Path,
    mode: &str,
    node_enabled: bool,
) -> (tokio::process::Child, String) {
    let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = socket.local_addr().unwrap().port().to_string();
    drop(socket);
    let child = tokio::process::Command::new(binary)
        .args([mode, "--port", &port])
        .current_dir(root)
        .env(
            "PATH",
            if node_enabled {
                std::env::var_os("PATH").unwrap_or_default()
            } else {
                std::ffi::OsString::new()
            },
        )
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let url = format!("http://127.0.0.1:{port}");
    ferrite_e2e::wait_for_url(&url, Duration::from_secs(15))
        .await
        .unwrap();
    (child, url)
}
async fn acceptance(kind: BrowserKind, frameworks: &[&str]) {
    let binary = cli();
    let executable = match kind {
        BrowserKind::Chromium => ferrite_e2e::find_chromium(None),
        BrowserKind::Firefox => ferrite_e2e::find_firefox(None),
    }
    .expect("generated-app acceptance requires the selected real browser");
    let browser = Browser::launch(
        LaunchOptions::default()
            .browser(kind)
            .executable(executable),
    )
    .await
    .unwrap();
    for framework in frameworks {
        let node_enabled = *framework != "vanilla";
        for language in ["js", "ts"] {
            let project = ferrite_test::TempProject::new(&[]);
            let destination = project.root.join("app");
            command(
                &binary,
                &project.root,
                &[
                    "create",
                    destination.to_str().unwrap(),
                    "--framework",
                    framework,
                    "--language",
                    language,
                    "--rendering",
                    "client",
                    "--compiler-host",
                    if node_enabled { "node" } else { "native" },
                ],
                node_enabled,
            )
            .await;
            let lock = std::fs::read(destination.join("ferrite.lock")).unwrap();
            command(
                &binary,
                &destination,
                &["install", "--frozen-lockfile"],
                node_enabled,
            )
            .await;
            assert_eq!(
                std::fs::read(destination.join("ferrite.lock")).unwrap(),
                lock
            );
            let (mut dev, url) = server(&binary, &destination, "dev", node_enabled).await;
            let page = browser.new_page().await.unwrap();
            page.goto(&url).await.unwrap();
            page.wait_for_function(
                "document.querySelector('#counter')?.textContent === 'count: 0'",
                Duration::from_secs(15),
            )
            .await
            .unwrap_or_else(|error| {
                panic!(
                    "{framework}/{language}: {error}; {:?}; {:?}",
                    page.page_errors(),
                    page.console_messages()
                )
            });
            page.locator("#counter").click().await.unwrap();
            page.wait_for_function(
                "document.querySelector('#counter')?.textContent === 'count: 1'",
                Duration::from_secs(10),
            )
            .await
            .unwrap();
            if node_enabled {
                let color: String = page
                    .evaluate("getComputedStyle(document.querySelector('#counter')).color")
                    .await
                    .unwrap();
                assert_eq!(color, "rgb(128, 0, 0)", "initial generated scoped CSS");
            }
            let main = destination.join(if node_enabled {
                format!("src/App.{framework}")
            } else {
                format!("src/main.{language}")
            });
            let source = std::fs::read_to_string(&main).unwrap();
            let changed = source.replace("count += 1", "count += 2");
            let changed = if node_enabled {
                changed
                    .replace(">count: ", ">edited count: ")
                    .replace("rgb(128, 0, 0)", "rgb(0, 0, 128)")
            } else {
                format!("{changed}\ndocument.body.dataset.revision = 'edited';\n")
            };
            std::fs::write(&main, changed).unwrap();
            let prefix = if node_enabled {
                "edited count"
            } else {
                "count"
            };
            page.wait_for_function(
                &format!("document.querySelector('#counter')?.textContent === '{prefix}: 0'"),
                Duration::from_secs(15),
            )
            .await
            .unwrap();
            page.locator("#counter").click().await.unwrap();
            page.wait_for_function(
                &format!("document.querySelector('#counter')?.textContent === '{prefix}: 2'"),
                Duration::from_secs(10),
            )
            .await
            .unwrap();
            assert!(page.page_errors().is_empty(), "{:?}", page.page_errors());
            assert!(
                page.console_messages()
                    .iter()
                    .all(|message| message.kind != "error"),
                "{:?}",
                page.console_messages()
            );
            if node_enabled {
                let color: String = page
                    .evaluate("getComputedStyle(document.querySelector('#counter')).color")
                    .await
                    .unwrap();
                assert_eq!(color, "rgb(0, 0, 128)", "updated generated scoped CSS");
            }
            dev.kill().await.unwrap();
            dev.wait().await.unwrap();
            command(
                &binary,
                &destination,
                &["build", "--scope-hoist"],
                node_enabled,
            )
            .await;
            assert!(destination.join("dist/index.html").exists());
            for asset in std::fs::read_dir(destination.join("dist/assets")).unwrap() {
                let asset = asset.unwrap().path();
                if asset.extension().is_some_and(|extension| extension == "js") {
                    let code = std::fs::read_to_string(asset).unwrap();
                    assert!(!code.contains("/@ferrite/client"));
                    assert!(!code.contains("__ferrite_create_hot__"));
                    assert!(!code.contains("react-refresh/runtime"));
                }
            }

            let (mut preview, url) = server(&binary, &destination, "preview", node_enabled).await;
            let page = browser.new_page().await.unwrap();
            page.goto(&url).await.unwrap();
            page.wait_for_function(
                &format!("document.querySelector('#counter')?.textContent === '{prefix}: 0'"),
                Duration::from_secs(10),
            )
            .await
            .unwrap();
            page.locator("#counter").click().await.unwrap();
            page.wait_for_function(
                &format!("document.querySelector('#counter')?.textContent === '{prefix}: 2'"),
                Duration::from_secs(10),
            )
            .await
            .unwrap();
            assert!(page.page_errors().is_empty(), "{:?}", page.page_errors());
            assert!(
                page.console_messages()
                    .iter()
                    .all(|message| message.kind != "error"),
                "{:?}",
                page.console_messages()
            );
            if node_enabled {
                let color: String = page
                    .evaluate("getComputedStyle(document.querySelector('#counter')).color")
                    .await
                    .unwrap();
                assert_eq!(color, "rgb(0, 0, 128)", "production extracted scoped CSS");
            }
            preview.kill().await.unwrap();
            preview.wait().await.unwrap();
            eprintln!("{kind:?}: {framework}/{language}/client complete (Node compiler enabled: {node_enabled})");
        }
    }
    browser.close().await.unwrap();
}
#[tokio::test]
#[ignore = "requires freshly built FERRITE_CLI_PATH and real Chromium"]
async fn chromium_generated_profiles() {
    acceptance(BrowserKind::Chromium, &["vanilla"]).await;
}
#[tokio::test]
#[ignore = "requires freshly built FERRITE_CLI_PATH and real Firefox"]
async fn firefox_generated_profiles() {
    acceptance(BrowserKind::Firefox, &["vanilla"]).await;
}

#[tokio::test]
#[ignore = "requires freshly built CLI, Node and real Chromium"]
async fn chromium_generated_components() {
    acceptance(BrowserKind::Chromium, &["svelte", "vue"]).await;
}
#[tokio::test]
#[ignore = "requires freshly built CLI, Node and real Firefox"]
async fn firefox_generated_components() {
    acceptance(BrowserKind::Firefox, &["svelte", "vue"]).await;
}
