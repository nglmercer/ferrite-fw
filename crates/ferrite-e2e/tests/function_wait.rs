//! Document-side predicate scheduling, captured results and teardown.
use ferrite_e2e::*;
use serde_json::{json, Value};
use std::time::{Duration, Instant};
const WAIT: Duration = Duration::from_secs(3);
fn options() -> FunctionWaitOptions {
    FunctionWaitOptions::default().timeout(WAIT)
}
async fn browsers() -> Vec<Browser> {
    let mut result = Vec::new();
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let path = match kind {
            BrowserKind::Chromium => find_chromium(None),
            BrowserKind::Firefox => find_firefox(None),
        };
        if let Some(path) = path {
            let browser = Browser::launch(LaunchOptions::default().browser(kind).executable(path))
                .await
                .expect("installed engine must launch");
            eprintln!(
                "Function waits native {}: {}",
                kind.name(),
                browser.version().await.unwrap()
            );
            result.push(browser);
        } else {
            eprintln!("Function waits browser absent: {}", kind.name());
        }
    }
    if std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_some() {
        assert_eq!(result.len(), 2);
    }
    result
}
async fn empty(page: &Page) {
    assert_eq!(
        page.evaluate_value("globalThis[Symbol.for('ferrite.functionWaits')]?.size || 0")
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn document_polling_arguments_values_handles_errors_and_frames() {
    let reference: Value = serde_json::from_str(include_str!(
        "../../../scripts/e2e-conformance/function-reference.json"
    ))
    .unwrap();
    assert_eq!(reference["playwright"], "1.63.0");
    for browser in browsers().await {
        let page = browser.new_page().await.unwrap();
        page.set_content("<p>wait</p><iframe name='child' srcdoc='<p>frame</p>'></iframe>")
            .await
            .unwrap();
        for case in reference["cases"].as_array().unwrap() {
            page.evaluate_value("window.calls=0; window.stamps=[]; true")
                .await
                .unwrap();
            let polling = if case["polling"] == "raf" {
                FunctionPolling::AnimationFrame
            } else {
                FunctionPolling::Interval(Duration::from_millis(case["polling"].as_u64().unwrap()))
            };
            let value = page
                .wait_for_function_value(
                    case["expression"].as_str().unwrap(),
                    &case["argument"],
                    options().polling(polling),
                )
                .await
                .unwrap();
            assert_eq!(value, case["result"], "{case}");
            empty(&page).await;
        }
        page.evaluate_value("window.calls=0; window.stamps=[]; true")
            .await
            .unwrap();
        let value = page.wait_for_function_value("arg => { window.stamps.push(performance.now()); return ++window.calls === arg.count && {count:window.calls}; }", &json!({"count":3}), options().polling(FunctionPolling::Interval(Duration::from_millis(65)))).await.unwrap();
        assert_eq!(value, json!({"count":3}));
        let stamps: Vec<f64> = page.evaluate("window.stamps").await.unwrap();
        assert!(stamps.windows(2).all(|s| s[1] - s[0] >= 55.0));
        tokio::time::sleep(Duration::from_millis(90)).await;
        assert_eq!(
            page.evaluate_value("window.calls").await.unwrap(),
            3,
            "successful predicate is never re-evaluated"
        );
        page.evaluate_value("window.calls=0; window.rafs=0; window.nativeRAF=requestAnimationFrame; window.requestAnimationFrame=fn=>{++window.rafs; return window.nativeRAF(fn)}; true").await.unwrap();
        assert_eq!(
            page.wait_for_function_value("() => ++window.calls>=3 && {raf:true}", &(), options())
                .await
                .unwrap(),
            json!({"raf":true})
        );
        assert_eq!(page.evaluate_value("window.rafs").await.unwrap(), 2);
        page.evaluate_value("window.requestAnimationFrame=window.nativeRAF; true")
            .await
            .unwrap();
        let (first, second) = tokio::join!(
            page.wait_for_function_value(
                "x => new Promise(resolve=>setTimeout(()=>resolve({result:x}),70))",
                &11,
                options()
            ),
            page.wait_for_function_value("x => Promise.resolve({result:x})", &22, options())
        );
        assert_eq!(first.unwrap(), json!({"result":11}));
        assert_eq!(second.unwrap(), json!({"result":22}));
        empty(&page).await;
        let handle = page.wait_for_function_handle("() => { const value={answer:42}; value.self=value; window.kept=value; return value; }", &(), options()).await.unwrap();
        assert!(handle
            .evaluate::<bool>("value => value === window.kept && value.self === value")
            .await
            .unwrap());
        let answer = handle.get_property("answer").await.unwrap();
        assert_eq!(answer.json_value::<i64>().await.unwrap(), 42);
        handle.dispose().await.unwrap();
        for expression in [
            "() => { throw new Error('predicate exploded'); }",
            "() => Promise.reject(new Error('promise exploded'))",
            "() => { const x={}; x.self=x; return x; }",
        ] {
            let started = Instant::now();
            assert!(
                matches!(page.wait_for_function_value(expression,&(),options()).await,Err(E2eError::Cdp {method,..}) if method=="wait_for_function")
            );
            assert!(started.elapsed() < Duration::from_secs(1));
            empty(&page).await;
        }
        assert!(matches!(
            page.wait_for_function_value(
                "true",
                &(),
                options().polling(FunctionPolling::Interval(Duration::ZERO))
            )
            .await,
            Err(E2eError::Config(_))
        ));
        let frame = page
            .document_frames()
            .await
            .unwrap()
            .into_iter()
            .find(|f| f.parent_id().is_some())
            .unwrap();
        assert_eq!(
            frame
                .wait_for_function_value(
                    "arg => Promise.resolve({text:document.querySelector('p').textContent, arg})",
                    &json!([1, 2]),
                    options()
                )
                .await
                .unwrap(),
            json!({"text":"frame","arg":[1,2]})
        );
        page.evaluate_value("setTimeout(()=>document.querySelector('iframe').remove(),80); true")
            .await
            .unwrap();
        let detached = frame.wait_for_function_value("false", &(), options()).await;
        assert!(detached.is_err());
        assert!(
            !matches!(detached, Err(E2eError::Timeout(_, _))),
            "detached frame fails directly"
        );
        empty(&page).await;
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn function_wait_budgets_cancellation_navigation_and_disposal() {
    for browser in browsers().await {
        let page = browser.new_page().await.unwrap();
        page.set_content("<p>first</p>").await.unwrap();
        let before = page.timeout();
        let started = Instant::now();
        assert!(matches!(
            page.wait_for_function_value(
                "() => new Promise(()=>{})",
                &(),
                options().timeout(Duration::from_millis(100))
            )
            .await,
            Err(E2eError::Timeout(100, _))
        ));
        assert!(started.elapsed() < Duration::from_secs(1));
        empty(&page).await;
        assert_eq!(page.timeout(), before);
        page.evaluate_value("window.calls=0; true").await.unwrap();
        let token = CancellationToken::new();
        let (wait, ()) = tokio::join!(
            page.wait_for_function_value(
                "() => { ++window.calls; return false; }",
                &(),
                options()
                    .timeout(Duration::ZERO)
                    .cancellation(token.clone())
            ),
            async {
                tokio::time::sleep(Duration::from_millis(90)).await;
                token.cancel();
            }
        );
        assert!(matches!(wait, Err(E2eError::Cancelled(_))));
        empty(&page).await;
        let calls = page.evaluate_value("window.calls").await.unwrap();
        tokio::time::sleep(Duration::from_millis(90)).await;
        assert_eq!(page.evaluate_value("window.calls").await.unwrap(), calls);
        // Abort the Rust future itself, proving the drop guard reclaims its timer.
        let task_page = page.clone();
        let task = tokio::spawn(async move {
            task_page
                .wait_for_function_value("false", &(), options().timeout(Duration::ZERO))
                .await
        });
        page.wait_for_function(
            "globalThis[Symbol.for('ferrite.functionWaits')]?.size===1",
            WAIT,
        )
        .await
        .unwrap();
        task.abort();
        let _ = task.await;
        page.wait_for_function(
            "globalThis[Symbol.for('ferrite.functionWaits')]?.size===0",
            WAIT,
        )
        .await
        .unwrap();
        // New documents get a new task and the original enclosing deadline.
        let (wait, navigation) = tokio::join!(
            page.wait_for_function_value(
                "() => location.href.includes('data:text/html') && {after:true}",
                &(),
                options()
            ),
            async {
                tokio::time::sleep(Duration::from_millis(70)).await;
                page.goto("data:text/html,<p>after</p>").await
            }
        );
        navigation.unwrap();
        assert_eq!(wait.unwrap(), json!({"after":true}));
        empty(&page).await;
        let token = CancellationToken::new();
        token.cancel();
        assert!(matches!(
            page.wait_for_function_value("true", &(), options().cancellation(token))
                .await,
            Err(E2eError::Cancelled(_))
        ));
        empty(&page).await;
        let (wait, closed) = tokio::join!(
            page.wait_for_function_value("false", &(), options().timeout(Duration::ZERO)),
            async {
                tokio::time::sleep(Duration::from_millis(60)).await;
                page.close().await
            }
        );
        assert!(matches!(wait, Err(E2eError::Cancelled(_))));
        closed.unwrap();
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn function_wait_enclosing_runner_budget_and_retry() {
    for browser in browsers().await {
        let output = tempfile::tempdir().unwrap();
        let report = Runner::default()
            .test_timeout(Duration::from_millis(500))
            .retries(1)
            .output_dir(output.path().display().to_string())
            .run(
                &browser,
                vec![test("function wait enclosing budget", |page| async move {
                    page.wait_for_function_value(
                        "false",
                        &(),
                        FunctionWaitOptions::default().timeout(Duration::ZERO),
                    )
                    .await?;
                    Ok(())
                })],
            )
            .await;
        assert_eq!(report.results[0].attempts, 2);
        assert_eq!(report.failed(), 1);
        assert!(report.results[0]
            .attempt_results
            .iter()
            .all(|a| !a.errors.is_empty()));
        browser.close().await.unwrap();
    }
}
