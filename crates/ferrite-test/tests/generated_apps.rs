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
async fn command(binary: &Path, root: &Path, args: &[&str]) {
    let output = tokio::process::Command::new(binary)
        .args(args)
        .current_dir(root)
        .env("PATH", "")
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
async fn server(binary: &Path, root: &Path, mode: &str) -> (tokio::process::Child, String) {
    let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = socket.local_addr().unwrap().port().to_string();
    drop(socket);
    let child = tokio::process::Command::new(binary)
        .args([mode, "--port", &port])
        .current_dir(root)
        .env("PATH", "")
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
async fn acceptance(kind: BrowserKind) {
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
                "vanilla",
                "--language",
                language,
                "--rendering",
                "client",
            ],
        )
        .await;
        let lock = std::fs::read(destination.join("ferrite.lock")).unwrap();
        command(&binary, &destination, &["install", "--frozen-lockfile"]).await;
        assert_eq!(
            std::fs::read(destination.join("ferrite.lock")).unwrap(),
            lock
        );
        let (mut dev, url) = server(&binary, &destination, "dev").await;
        let page = browser.new_page().await.unwrap();
        page.goto(&url).await.unwrap();
        page.locator("#counter").click().await.unwrap();
        page.wait_for_function(
            "document.querySelector('#counter')?.textContent === 'count: 1'",
            Duration::from_secs(10),
        )
        .await
        .unwrap();
        let main = destination.join(format!("src/main.{language}"));
        let source = std::fs::read_to_string(&main).unwrap();
        std::fs::write(
            &main,
            format!(
                "{}\ndocument.body.dataset.revision = 'edited';\n",
                source.replace("count += 1", "count += 2")
            ),
        )
        .unwrap();
        page.wait_for_function("document.body.dataset.revision === 'edited' && document.querySelector('#counter')?.textContent === 'count: 0'", Duration::from_secs(15)).await.unwrap();
        page.locator("#counter").click().await.unwrap();
        page.wait_for_function(
            "document.querySelector('#counter')?.textContent === 'count: 2'",
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
        dev.kill().await.unwrap();
        dev.wait().await.unwrap();
        command(&binary, &destination, &["build", "--scope-hoist"]).await;
        assert!(destination.join("dist/index.html").exists());
        let (mut preview, url) = server(&binary, &destination, "preview").await;
        let page = browser.new_page().await.unwrap();
        page.goto(&url).await.unwrap();
        page.wait_for_function(
            "document.body.dataset.revision === 'edited'",
            Duration::from_secs(10),
        )
        .await
        .unwrap();
        page.locator("#counter").click().await.unwrap();
        page.wait_for_function(
            "document.querySelector('#counter')?.textContent === 'count: 2'",
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
        preview.kill().await.unwrap();
        preview.wait().await.unwrap();
        eprintln!("{kind:?}: vanilla/{language}/client complete with Node absent from CLI PATH");
    }
    browser.close().await.unwrap();
}
#[tokio::test]
#[ignore = "requires freshly built FERRITE_CLI_PATH and real Chromium"]
async fn chromium_generated_profiles() {
    acceptance(BrowserKind::Chromium).await;
}
#[tokio::test]
#[ignore = "requires freshly built FERRITE_CLI_PATH and real Firefox"]
async fn firefox_generated_profiles() {
    acceptance(BrowserKind::Firefox).await;
}
