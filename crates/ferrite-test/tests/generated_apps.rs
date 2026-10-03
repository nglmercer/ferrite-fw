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
            // Exercise a non-default lock across installer, compiler host,
            // resolver and production pipeline, with no fallback lock present.
            let lock_path = destination.join(if node_enabled && language == "ts" {
                let config = destination.join("ferrite.toml");
                let mut source = std::fs::read_to_string(&config).unwrap();
                source.push_str("\n[npm]\nlockfile = 'selected.lock'\n");
                std::fs::write(config, source).unwrap();
                std::fs::rename(
                    destination.join("ferrite.lock"),
                    destination.join("selected.lock"),
                )
                .unwrap();
                "selected.lock"
            } else {
                "ferrite.lock"
            });
            let lock = std::fs::read(&lock_path).unwrap();
            assert_eq!(
                std::fs::read_link(destination.join("node_modules")).unwrap(),
                PathBuf::from(".ferrite/npm/node_modules")
            );
            if node_enabled {
                let graph = ferrite::npm::Lockfile::read(&lock_path).unwrap();
                let identity = &graph.importers["."].dependencies[*framework];
                let actual = destination
                    .join("node_modules")
                    .join(framework)
                    .canonicalize()
                    .unwrap();
                assert_eq!(
                    actual,
                    destination
                        .join(".ferrite/npm/packages")
                        .join(identity)
                        .canonicalize()
                        .unwrap(),
                    "editor resolution must use the same concrete importer edge before dev starts"
                );
            }

            command(
                &binary,
                &destination,
                &["install", "--frozen-lockfile"],
                node_enabled,
            )
            .await;
            assert_eq!(std::fs::read(&lock_path).unwrap(), lock);
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
            let invalid = match *framework {
                "vue" => "<script setup>const broken = ;</script><template><button>broken</button></template>",
                "svelte" => "<script>const broken = ;</script><button>broken</button>",
                _ => "const broken = ;",
            };
            std::fs::write(&main, invalid).unwrap();
            page.wait_for_function(
                "document.querySelector('#ferrite-error-overlay')?.textContent.length > 0",
                Duration::from_secs(15),
            )
            .await
            .unwrap_or_else(|error| {
                panic!(
                    "{framework}/{language}: {error}; errors={:?}; console={:?}",
                    page.page_errors(),
                    page.console_messages()
                )
            });
            let overlay: String = page
                .evaluate("document.querySelector('#ferrite-error-overlay').textContent")
                .await
                .unwrap();
            assert!(
                overlay.contains(main.file_name().unwrap().to_str().unwrap()),
                "{overlay}"
            );
            assert_eq!(
                page.evaluate::<String>("document.querySelector('#counter').textContent")
                    .await
                    .unwrap(),
                "count: 1",
                "invalid source must keep the running application"
            );

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

async fn dependency_acceptance(kind: BrowserKind) {
    let binary = cli();
    let project = ferrite_test::TempProject::new(&[
        ("index.html", "<html><head><link rel='icon' href='data:,'></head><body><button id='counter'>loading</button><span id='other'></span><script type='module' src='/main.js'></script></body></html>"),
        ("dep.js", "export const step = 1; if (import.meta.hot) import.meta.hot.dispose(() => { globalThis.disposals = (globalThis.disposals || 0) + 1; });"),
        ("main.js", "import {step} from './dep.js'; import './other.js'; globalThis.parentRuns = (globalThis.parentRuns || 0) + 1; globalThis.session = Math.random(); let increment = step; let count = 0; const button = document.querySelector('#counter'); const render = () => button.textContent = `count: ${count}, step: ${increment}`; render(); button.onclick = () => { count += increment; render(); }; if (import.meta.hot) { import.meta.hot.accept(['./dep.js'], ([next]) => { increment = next.step; render(); }); import.meta.hot.dispose(() => { throw new Error('accepting parent was disposed'); }); }"),
        ("other.js", "import {step} from './dep.js'; globalThis.otherRuns = (globalThis.otherRuns || 0) + 1; document.querySelector('#other').textContent = `other: ${step}`; if (import.meta.hot) import.meta.hot.accept('./dep.js', next => { document.querySelector('#other').textContent = `other: ${next.step}`; globalThis.otherCallbacks = (globalThis.otherCallbacks || 0) + 1; });"),
    ]);
    let executable = match kind {
        BrowserKind::Chromium => ferrite_e2e::find_chromium(None),
        BrowserKind::Firefox => ferrite_e2e::find_firefox(None),
    }
    .expect("dependency acceptance requires a real browser");
    let browser = Browser::launch(
        LaunchOptions::default()
            .browser(kind)
            .executable(executable),
    )
    .await
    .unwrap();
    let (mut dev, url) = server(&binary, &project.root, "dev", false).await;
    let page = browser.new_page().await.unwrap();
    page.goto(&url).await.unwrap();
    page.wait_for_function("document.querySelector('#counter')?.textContent === 'count: 0, step: 1' && document.querySelector('#other')?.textContent === 'other: 1'", Duration::from_secs(10)).await.unwrap_or_else(|error| panic!("{error}: errors={:?}; console={:?}", page.page_errors(), page.console_messages()));
    let session: f64 = page.evaluate("globalThis.session").await.unwrap();
    page.locator("#counter").click().await.unwrap();
    let mut count = 1;
    std::fs::write(project.root.join("dep.js"), "export const step = ;").unwrap();
    page.wait_for_function(
        "document.querySelector('#ferrite-error-overlay')?.textContent.includes('/dep.js')",
        Duration::from_secs(10),
    )
    .await
    .unwrap_or_else(|error| {
        panic!(
            "{error}; errors={:?}; console={:?}",
            page.page_errors(),
            page.console_messages()
        )
    });
    assert_eq!(
        page.evaluate::<f64>("globalThis.session").await.unwrap(),
        session
    );
    assert_eq!(
        page.evaluate::<u32>("globalThis.disposals || 0")
            .await
            .unwrap(),
        0,
        "invalid compilation must not dispose live modules"
    );
    assert_eq!(
        page.evaluate::<String>("document.querySelector('#counter').textContent")
            .await
            .unwrap(),
        "count: 1, step: 1"
    );

    for step in [2, 3] {
        std::fs::write(project.root.join("dep.js"), format!("export const step = {step}; if (import.meta.hot) import.meta.hot.dispose(() => {{ globalThis.disposals = (globalThis.disposals || 0) + 1; }});")).unwrap();
        page.wait_for_function(&format!("document.querySelector('#counter')?.textContent === 'count: {count}, step: {step}' && document.querySelector('#other')?.textContent === 'other: {step}'"), Duration::from_secs(10)).await.unwrap_or_else(|error| panic!("{error}: errors={:?}; console={:?}", page.page_errors(), page.console_messages()));
        assert!(page
            .evaluate::<bool>("!document.querySelector('#ferrite-error-overlay')")
            .await
            .unwrap());
        assert_eq!(
            page.evaluate::<f64>("globalThis.session").await.unwrap(),
            session,
            "document must not reload"
        );
        assert_eq!(
            page.evaluate::<u32>("globalThis.parentRuns").await.unwrap(),
            1
        );
        assert_eq!(
            page.evaluate::<u32>("globalThis.otherRuns").await.unwrap(),
            1
        );
        assert_eq!(
            page.evaluate::<u32>("globalThis.disposals").await.unwrap(),
            step - 1,
            "shared dependency is disposed once"
        );
        assert_eq!(
            page.evaluate::<u32>("globalThis.otherCallbacks")
                .await
                .unwrap(),
            step - 1
        );
        page.locator("#counter").click().await.unwrap();
        count += step;
        page.wait_for_function(&format!("document.querySelector('#counter')?.textContent === 'count: {count}, step: {step}'"), Duration::from_secs(5)).await.unwrap();
    }
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
    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires freshly built CLI and real Chromium"]
async fn chromium_dependency_acceptance() {
    dependency_acceptance(BrowserKind::Chromium).await;
}
#[tokio::test]
#[ignore = "requires freshly built CLI and real Firefox"]
async fn firefox_dependency_acceptance() {
    dependency_acceptance(BrowserKind::Firefox).await;
}

async fn transitive_self_acceptance(kind: BrowserKind) {
    let binary = cli();
    let project = ferrite_test::TempProject::new(&[
        ("ferrite.toml", "[npm]\ndev_strategy = 'import-map'\n[resolve.alias]\nleaf = './leaf.js'\n"),
        ("index.html", "<html><head><link rel='icon' href='data:,'></head><body><button id='counter'>loading</button><span id='factor'></span><script type='module' src='/main.js'></script></body></html>"),
        ("leaf.js", "import './mid.js'; export const step = 1; globalThis.leafRuns = (globalThis.leafRuns || 0) + 1;"),
        ("mid.js", "export {step} from 'leaf'; globalThis.midRuns = (globalThis.midRuns || 0) + 1;"),
        ("boundary.js", "import {step} from './mid.js'; export const factor = step; globalThis.factor = factor; document.querySelector('#factor').textContent = `step: ${factor}`; globalThis.boundaryRuns = (globalThis.boundaryRuns || 0) + 1; if (import.meta.hot) { import.meta.hot.accept(next => { globalThis.factor = next.factor; globalThis.callbacks = (globalThis.callbacks || 0) + 1; }); import.meta.hot.dispose(() => { globalThis.disposals = (globalThis.disposals || 0) + 1; }); }"),
        ("main.js", "import './boundary.js'; globalThis.mainRuns = (globalThis.mainRuns || 0) + 1; globalThis.session = Math.random(); let count = 0; const button = document.querySelector('#counter'); button.textContent = `count: ${count}`; button.onclick = () => { count += globalThis.factor; button.textContent = `count: ${count}`; };"),
    ]);
    let executable = match kind {
        BrowserKind::Chromium => ferrite_e2e::find_chromium(None),
        BrowserKind::Firefox => ferrite_e2e::find_firefox(None),
    }
    .expect("transitive acceptance requires a real browser");
    let browser = Browser::launch(
        LaunchOptions::default()
            .browser(kind)
            .executable(executable),
    )
    .await
    .unwrap();
    let (mut dev, url) = server(&binary, &project.root, "dev", false).await;
    let page = browser.new_page().await.unwrap();
    page.goto(&url).await.unwrap();
    page.wait_for_function(
        "document.querySelector('#counter')?.textContent === 'count: 0' && globalThis.factor === 1",
        Duration::from_secs(10),
    )
    .await
    .unwrap();
    let session: f64 = page.evaluate("globalThis.session").await.unwrap();
    page.locator("#counter").click().await.unwrap();
    let mut count = 1;
    std::fs::write(project.root.join("leaf.js"), "export const step = ;").unwrap();
    page.wait_for_function(
        "document.querySelector('#ferrite-error-overlay')?.textContent.includes('/leaf.js')",
        Duration::from_secs(10),
    )
    .await
    .unwrap_or_else(|error| {
        panic!(
            "{error}; errors={:?}; console={:?}",
            page.page_errors(),
            page.console_messages()
        )
    });
    assert_eq!(
        page.evaluate::<f64>("globalThis.session").await.unwrap(),
        session
    );
    assert_eq!(
        page.evaluate::<u32>("globalThis.disposals || 0")
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        page.evaluate::<String>("document.querySelector('#counter').textContent")
            .await
            .unwrap(),
        "count: 1"
    );

    for step in [2, 3] {
        std::fs::write(
            project.root.join("leaf.js"),
            format!(
                "export const step = {step}; globalThis.leafRuns = (globalThis.leafRuns || 0) + 1;"
            ),
        )
        .unwrap();
        page.wait_for_function(&format!("globalThis.factor === {step} && globalThis.callbacks === {} && document.querySelector('#factor')?.textContent === 'step: {step}'", step - 1), Duration::from_secs(10)).await.unwrap_or_else(|error| panic!("{error}; errors={:?}; console={:?}", page.page_errors(), page.console_messages()));
        assert!(page
            .evaluate::<bool>("!document.querySelector('#ferrite-error-overlay')")
            .await
            .unwrap());
        assert_eq!(
            page.evaluate::<f64>("globalThis.session").await.unwrap(),
            session
        );
        assert_eq!(
            page.evaluate::<u32>("globalThis.mainRuns").await.unwrap(),
            1
        );
        for runs in ["leafRuns", "midRuns", "boundaryRuns"] {
            assert_eq!(
                page.evaluate::<u32>(&format!("globalThis.{runs}"))
                    .await
                    .unwrap(),
                step
            );
        }
        assert_eq!(
            page.evaluate::<u32>("globalThis.disposals").await.unwrap(),
            step - 1
        );
        assert_eq!(
            page.evaluate::<String>("document.querySelector('#counter').textContent")
                .await
                .unwrap(),
            format!("count: {count}")
        );
        page.locator("#counter").click().await.unwrap();
        count += step;
        page.wait_for_function(
            &format!("document.querySelector('#counter')?.textContent === 'count: {count}'"),
            Duration::from_secs(5),
        )
        .await
        .unwrap();
    }
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
    browser.close().await.unwrap();
}
#[tokio::test]
#[ignore = "requires freshly built CLI and real Chromium"]
async fn chromium_transitive_self_acceptance() {
    transitive_self_acceptance(BrowserKind::Chromium).await;
}
#[tokio::test]
#[ignore = "requires freshly built CLI and real Firefox"]
async fn firefox_transitive_self_acceptance() {
    transitive_self_acceptance(BrowserKind::Firefox).await;
}
