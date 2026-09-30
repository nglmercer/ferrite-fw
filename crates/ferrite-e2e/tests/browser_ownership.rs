//! Actual owner access, shared shutdown, final-owner release and native disconnects.
use ferrite_e2e::*;
use serde_json::{json, Value};
use std::{path::PathBuf, time::Duration};

const BUDGET: Duration = Duration::from_secs(15);

fn engines() -> Vec<(BrowserKind, PathBuf)> {
    let engines: Vec<_> = [BrowserKind::Chromium, BrowserKind::Firefox]
        .into_iter()
        .filter_map(|kind| {
            let executable = match kind {
                BrowserKind::Chromium => find_chromium(None),
                BrowserKind::Firefox => find_firefox(None),
            };
            executable.map(|executable| (kind, executable))
        })
        .collect();
    if std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_some() {
        assert_eq!(engines.len(), 2, "both actual browser engines required");
    }
    engines
}

async fn launch(kind: BrowserKind, executable: PathBuf) -> Browser {
    let browser = Browser::launch(
        LaunchOptions::default()
            .browser(kind)
            .executable(executable),
    )
    .await
    .unwrap();
    eprintln!("owner {} {}", kind.name(), browser.version().await.unwrap());
    browser
}

async fn until(mut predicate: impl FnMut() -> bool) {
    tokio::time::timeout(BUDGET, async {
        while !predicate() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("native cleanup/connection state did not settle");
}

fn reference(name: &str) -> Value {
    let reference: Value = serde_json::from_str(include_str!(
        "../../../scripts/e2e-conformance/ownership-reference.json"
    ))
    .unwrap();
    assert_eq!(reference["playwright"], "1.63.0");
    reference["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == name)
        .unwrap()["result"]
        .clone()
}

async fn native_context_ids(browser: &Browser) -> Vec<String> {
    let values = if let Some(cdp) = browser.cdp() {
        cdp.call(None, "Target.getBrowserContexts", json!({}), BUDGET)
            .await
            .unwrap()["browserContextIds"]
            .clone()
    } else {
        let result = browser
            .bidi()
            .unwrap()
            .call("browser.getUserContexts", json!({}), BUDGET)
            .await
            .unwrap();
        json!(result["userContexts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|context| context["userContext"].as_str().unwrap())
            .collect::<Vec<_>>())
    };
    let mut values = values
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    values.sort();
    values
}

#[tokio::test]
async fn native_explicit_default_convenience_and_retained_owner_operations() {
    for (kind, executable) in engines() {
        let mut browser = launch(kind, executable).await;
        let native_before = native_context_ids(&browser).await;
        let missing = tempfile::tempdir().unwrap();
        assert!(browser
            .new_context(
                ContextOptions::default().storage_state(missing.path().join("missing.json"))
            )
            .await
            .is_err());
        assert!(browser.contexts().is_empty());
        assert_eq!(
            native_context_ids(&browser).await,
            native_before,
            "failed context storage setup must release the native context"
        );
        browser.set_base_url(Some("http://127.0.0.1/first/".into()));
        let context = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        let mut owner = context
            .browser()
            .expect("explicit context has its real owner");
        assert_eq!(owner.kind(), kind);
        assert_eq!(owner.debug_port(), browser.debug_port());
        assert_eq!(
            owner.version().await.unwrap(),
            browser.version().await.unwrap()
        );
        owner.set_base_url(Some("http://127.0.0.1/second/".into()));
        assert_eq!(browser.base_url(), owner.base_url());

        let page = browser.new_page().await.unwrap();
        let convenience = page.context().unwrap();
        let convenience_owner = convenience.browser().unwrap();
        assert_eq!(convenience_owner.debug_port(), browser.debug_port());
        assert!(convenience_owner
            .pages()
            .iter()
            .any(|p| p.target_id() == page.target_id()));
        page.close().await.unwrap();
        assert!(convenience.is_closed());

        let default = browser.default_context();
        let default_page = default.new_page().await.unwrap();
        let default_owner = default.browser().unwrap();
        assert!(default_owner
            .default_context()
            .pages()
            .iter()
            .any(|p| p.target_id() == default_page.target_id()));
        default_page.close().await.unwrap();
        let connected = owner.is_connected();
        context.clone().close().await.unwrap();
        assert_eq!(
            json!({"explicitOwner":true,"convenienceOwner":true,
            "connected":connected,"convenienceClosed":convenience.is_closed(),
            "closedContextOwner":context.browser().is_some()}),
            reference("context-owners")
        );
        drop(browser);
        assert!(
            owner.is_connected(),
            "retrieved owner retains the actual process"
        );
        let retained_page = owner.new_page().await.unwrap();
        assert_eq!(
            retained_page.evaluate::<Value>("6 * 7").await.unwrap(),
            json!(42)
        );
        owner.close().await.unwrap();
        assert!(!default_owner.is_connected());
        assert!(default.is_closed() && retained_page.is_closed());
        assert!(default_owner
            .new_context(ContextOptions::default())
            .await
            .is_err());
        default_owner.close().await.unwrap();
    }
}

// Inspect only the process associated with this test's unique debugging port.
// Linux adds actual child reaping/profile release evidence; other platforms
// still verify live owner operations and disconnected native transport state.
#[cfg(target_os = "linux")]
fn native_process(browser: &Browser) -> (PathBuf, PathBuf) {
    let port = browser.debug_port().to_string();
    for process in std::fs::read_dir("/proc").unwrap().flatten() {
        let path = process.path();
        let Ok(bytes) = std::fs::read(path.join("cmdline")) else {
            continue;
        };
        let args: Vec<_> = bytes
            .split(|byte| *byte == 0)
            .filter_map(|arg| std::str::from_utf8(arg).ok())
            // Chrome rewrites /proc argv into one space-separated process title.
            .flat_map(str::split_ascii_whitespace)
            .collect();
        if args.iter().any(|arg| arg.starts_with("--type=")) {
            continue;
        }
        let chromium = args
            .iter()
            .any(|arg| *arg == format!("--remote-debugging-port={port}"));
        let firefox = args
            .windows(2)
            .any(|args| args == ["--remote-debugging-port", port.as_str()]);
        if !chromium && !firefox {
            continue;
        }
        let profile = args
            .iter()
            .find_map(|arg| arg.strip_prefix("--user-data-dir="))
            .or_else(|| {
                args.windows(2)
                    .find(|args| args[0] == "--profile")
                    .map(|args| args[1])
            });
        if let Some(profile) = profile {
            return (path, PathBuf::from(profile));
        }
    }
    panic!("could not find this test's native browser process");
}

#[tokio::test]
async fn native_last_owner_drop_releases_process_profile_and_empty_context_waiters() {
    for (kind, executable) in engines() {
        let browser = launch(kind, executable).await;
        let context = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        let owner = context.browser().unwrap();
        #[cfg(target_os = "linux")]
        let (process, profile) = native_process(&browser);
        let waiting_context = context.clone();
        let waiter = tokio::spawn(async move {
            waiting_context
                .wait_for_event(ContextEventKind::Console, Duration::ZERO)
                .await
        });
        tokio::task::yield_now().await;
        drop(browser);
        assert!(owner.is_connected());
        #[cfg(target_os = "linux")]
        assert!(process.exists() && profile.exists());
        drop(owner);
        assert!(
            context.browser().is_none(),
            "contexts must not own a browser cycle"
        );
        assert!(context.is_closed());
        let error = tokio::time::timeout(BUDGET, waiter)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err();
        assert!(matches!(error, E2eError::Disconnected(_)), "{error}");
        assert!(context.new_page().await.is_err());
        #[cfg(target_os = "linux")]
        {
            let result = tokio::time::timeout(BUDGET, async {
                while process.exists() || profile.exists() {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await;
            assert!(
                result.is_ok(),
                "release failed: process={} stat={:?}; profile={} files={:?}",
                process.display(),
                std::fs::read_to_string(process.join("stat")),
                profile.display(),
                std::fs::read_dir(&profile).map(|entries| entries
                    .flatten()
                    .map(|entry| entry.file_name())
                    .collect::<Vec<_>>())
            );
        }
        context.close().await.unwrap();
    }
}

#[tokio::test]
async fn native_shared_close_survives_future_drop_and_preserves_persistent_profile() {
    for (kind, executable) in engines() {
        let profile = tempfile::tempdir().unwrap();
        let sentinel = profile.path().join("user-owned.txt");
        std::fs::write(&sentinel, "retained").unwrap();
        let browser = Browser::launch(
            LaunchOptions::default()
                .browser(kind)
                .executable(executable)
                .user_data_dir(profile.path()),
        )
        .await
        .unwrap();
        let context = browser.default_context();
        let page = context.new_page().await.unwrap();
        assert!(context.browser().unwrap().is_connected());
        let owner = browser.clone();
        let mut close = Box::pin(owner.close());
        assert!(futures::poll!(&mut close).is_pending());
        assert!(!browser.is_connected() && context.is_closed());
        drop(close); // Shutdown has started, but its caller is canceled.
        let (first, second) =
            tokio::join!(browser.clone().close(), context.browser().unwrap().close());
        first.unwrap();
        second.unwrap();
        assert!(page.is_closed());
        assert_eq!(std::fs::read_to_string(&sentinel).unwrap(), "retained");
        assert_eq!(
            json!({"persistentOwner":true,"disconnected":!browser.is_connected()}),
            reference("persistent-owner")
        );
        context.clone().close().await.unwrap();
        browser.close().await.unwrap();
        assert!(profile.path().exists());
    }
}

async fn unexpected_native_close(browser: &Browser) {
    if let Some(cdp) = browser.cdp() {
        cdp.call(None, "Browser.close", json!({}), BUDGET)
            .await
            .ok();
    } else {
        browser
            .bidi()
            .unwrap()
            .call("browser.close", json!({}), BUDGET)
            .await
            .ok();
    }
    until(|| !browser.is_connected()).await;
}

#[tokio::test]
async fn native_unexpected_disconnect_and_remote_owner_close_leave_accurate_state() {
    for (kind, executable) in engines() {
        let browser = launch(kind, executable).await;
        let empty = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        let page = browser.new_page().await.unwrap();
        let context = page.context().unwrap();
        let owner = context.browser().unwrap();
        let waiting = empty.clone();
        let waiter = tokio::spawn(async move {
            waiting
                .wait_for_event(ContextEventKind::Console, Duration::ZERO)
                .await
        });
        tokio::task::yield_now().await;
        let mut remote_observer = None;
        if kind == BrowserKind::Chromium {
            let remote = Browser::connect(browser.debug_port(), BUDGET)
                .await
                .unwrap();
            let remote_context = remote.new_context(ContextOptions::default()).await.unwrap();
            let remote_owner = remote_context.browser().unwrap();
            remote_owner.close().await.unwrap();
            until(|| !remote.is_connected()).await;
            assert!(remote_context.is_closed());
            assert!(
                browser.is_connected(),
                "closing an attached owner must leave source process running"
            );
            assert_eq!(page.evaluate::<Value>("21 * 2").await.unwrap(), json!(42));
            remote.close().await.unwrap();
            let observer = Browser::connect(browser.debug_port(), BUDGET)
                .await
                .unwrap();
            let observer_context = observer
                .new_context(ContextOptions::default())
                .await
                .unwrap();
            remote_observer = Some((observer, observer_context));
        }
        unexpected_native_close(&browser).await;
        if let Some((observer, observer_context)) = remote_observer {
            until(|| !observer.is_connected()).await;
            assert!(observer_context.is_closed());
            assert!(!observer_context.browser().unwrap().is_connected());
            observer.close().await.unwrap();
        }
        assert!(!owner.is_connected());
        assert!(context.is_closed() && empty.is_closed() && page.is_closed());
        assert!(matches!(
            tokio::time::timeout(BUDGET, waiter).await.unwrap().unwrap(),
            Err(E2eError::Disconnected(_))
        ));
        assert!(empty.new_page().await.is_err());
        assert!(matches!(
            empty
                .wait_for_event(ContextEventKind::Closed, BUDGET)
                .await
                .unwrap(),
            ContextEvent::Closed
        ));
        owner.close().await.unwrap();
        browser.close().await.unwrap();
    }
}
