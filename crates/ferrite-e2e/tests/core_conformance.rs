//! Compare shared native semantics with the pinned Playwright reference corpus.
use ferrite_e2e::*;
use serde_json::{json, Value};
use std::time::Duration;
const SHORT: Duration = Duration::from_millis(250);

async fn browsers() -> Vec<Browser> {
    let mut result = Vec::new();
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let executable = match kind {
            BrowserKind::Chromium => find_chromium(None),
            BrowserKind::Firefox => find_firefox(None),
        };
        if let Some(executable) = executable {
            eprintln!(
                "native conformance {}: {}",
                kind.name(),
                executable.display()
            );
            let browser = Browser::launch(
                LaunchOptions::default()
                    .browser(kind)
                    .executable(executable),
            )
            .await
            .expect("installed browser must launch");
            eprintln!(
                "native browser version {}",
                browser.version().await.unwrap()
            );
            result.push(browser);
        } else {
            eprintln!("browser absent: {}", kind.name());
        }
    }
    if std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_some() {
        assert_eq!(result.len(), 2, "both installed browsers are required");
    }
    result
}
async fn server() -> (String, tokio::task::AbortHandle) {
    use axum::{response::Html, routing::get, Router};
    let fixture = include_str!("../../../scripts/e2e-conformance/fixture.html");
    let app = Router::new()
        .route(
            "/redirect",
            get(|| async {
                (
                    axum::http::StatusCode::FOUND,
                    [("location", "/final")],
                    "redirect",
                )
            }),
        )
        .fallback(get(move || async move { Html(fixture) }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (base, task.abort_handle())
}

#[tokio::test]
async fn native_semantics_match_playwright_163_reference() {
    let reference: Value = serde_json::from_str(include_str!(
        "../../../scripts/e2e-conformance/reference.json"
    ))
    .unwrap();
    assert_eq!(reference["playwright"], "1.63.0");
    for mut browser in browsers().await {
        let (base, stop) = server().await;
        browser.set_base_url(Some(base));
        let page = browser.new_page().await.unwrap();
        page.set_viewport(Viewport {
            width: 640,
            height: 480,
        })
        .await
        .unwrap();
        page.goto("/fixture").await.unwrap();
        for (name, init) in [
            ("click", json!({"clientX":17,"ctrlKey":true})),
            ("keydown", json!({"key":"Q","code":"KeyQ"})),
            ("focus", json!({})),
            ("input", json!({"data":"x","inputType":"insertText"})),
            ("pointerdown", json!({"pointerId":7,"pointerType":"pen"})),
            ("probe", json!({})),
        ] {
            page.locator("#event")
                .dispatch_event_with(name, DispatchEventOptions::default().init(init))
                .await
                .unwrap();
        }
        page.locator("#event")
            .dispatch_event_with(
                "input",
                DispatchEventOptions::default()
                    .kind(DomEventKind::Input)
                    .init(json!({"data":"typed","inputType":"insertText"})),
            )
            .await
            .unwrap();
        let events = page.evaluate_value("samples").await.unwrap();
        assert_eq!(
            events,
            reference["event_samples"],
            "{} event constructors/fields differ",
            browser.kind().name()
        );
        page.locator("#shadow-button")
            .dispatch_event_with("shadow-probe", DispatchEventOptions::default())
            .await
            .unwrap();
        page.locator("#shadow-button")
            .dispatch_event_with(
                "shadow-probe",
                DispatchEventOptions::default().composed(false),
            )
            .await
            .unwrap();
        assert_eq!(
            page.evaluate_value("shadowReached").await.unwrap(),
            reference["shadow_reached"]
        );
        let mut assertions = serde_json::Map::new();
        macro_rules! check {
            ($name:literal, $future:expr) => {
                assertions.insert($name.into(), Value::Bool($future.await.is_ok()));
            };
        }
        let text_options = TextAssertionOptions::default();
        let match_options = MatchOptions::default();
        check!(
            "normalized",
            page.locator("#text")
                .expect()
                .timeout(SHORT)
                .text_with(&TextMatcher::exact("A B Hidden"), text_options)
        );
        check!(
            "raw_regex",
            page.locator("#text")
                .expect()
                .timeout(SHORT)
                .text_with(&TextMatcher::regex("\\n").unwrap(), text_options)
        );
        check!(
            "rendered",
            page.locator("#text").expect().timeout(SHORT).text_with(
                &TextMatcher::exact("a b"),
                text_options.use_inner_text(true).ignore_case(true)
            )
        );
        check!(
            "ordered_mixed",
            page.locator(".items p").expect().timeout(SHORT).texts_with(
                &[
                    "Alpha".into(),
                    TextMatcher::regex("^Beta$").unwrap(),
                    "Gamma".into()
                ],
                text_options
            )
        );
        check!(
            "subset",
            page.locator(".items p")
                .expect()
                .timeout(SHORT)
                .contains_texts_with(
                    &["Al".into(), TextMatcher::regex("Gamma").unwrap()],
                    text_options
                )
        );
        check!(
            "reversed_subset",
            page.locator(".items p")
                .expect()
                .timeout(SHORT)
                .contains_texts_with(&["Gamma".into(), "Alpha".into()], text_options)
        );
        check!(
            "class_order",
            page.locator("#classes")
                .expect()
                .timeout(SHORT)
                .class_with(&"two one".into(), match_options)
        );
        check!(
            "class_duplicates",
            page.locator("#duplicate")
                .expect()
                .timeout(SHORT)
                .class_with(&"one one".into(), match_options)
        );
        check!(
            "class_list",
            page.locator(".items p")
                .expect()
                .timeout(SHORT)
                .classes_with(
                    &[
                        "one alpha".into(),
                        TextMatcher::regex("two").unwrap(),
                        "three gamma".into()
                    ],
                    match_options
                )
        );
        check!(
            "class_list_exact",
            page.locator(".items p")
                .expect()
                .timeout(SHORT)
                .classes_with(
                    &[
                        "alpha one".into(),
                        TextMatcher::regex("two").unwrap(),
                        "gamma three".into()
                    ],
                    match_options
                )
        );
        check!(
            "contains_class_tokens",
            page.locator("#classes")
                .expect()
                .timeout(SHORT)
                .contains_class_tokens(&["two", "one"])
        );
        check!(
            "contains_class_list",
            page.locator(".items p")
                .expect()
                .timeout(SHORT)
                .contains_class_tokens_list(&["one", "two beta", "gamma"])
        );
        check!(
            "values_mixed",
            page.locator("#values").expect().timeout(SHORT).values_with(
                &["a".into(), TextMatcher::regex("^be").unwrap()],
                match_options
            )
        );
        check!(
            "indeterminate",
            page.locator("#checkbox")
                .expect()
                .timeout(SHORT)
                .checked_with(CheckedOptions::default().indeterminate(true))
        );
        check!(
            "unchecked",
            page.locator("#checkbox")
                .expect()
                .timeout(SHORT)
                .checked_with(CheckedOptions::default().checked(false))
        );
        check!(
            "viewport_low",
            page.locator("#clipped")
                .expect()
                .timeout(SHORT)
                .in_viewport_with(0.4)
        );
        check!(
            "viewport_high",
            page.locator("#clipped")
                .expect()
                .timeout(SHORT)
                .in_viewport_with(0.7)
        );
        check!(
            "accessible_name",
            page.locator("#accessible")
                .expect()
                .timeout(SHORT)
                .accessible_name_with(
                    &TextMatcher::regex("save changes").unwrap(),
                    match_options.ignore_case(true)
                )
        );
        check!(
            "accessible_description",
            page.locator("#accessible")
                .expect()
                .timeout(SHORT)
                .accessible_description_with(
                    &TextMatcher::regex("helpful").unwrap(),
                    match_options.ignore_case(true)
                )
        );
        check!(
            "accessible_error",
            page.locator("#accessible")
                .expect()
                .timeout(SHORT)
                .accessible_error_message_with(
                    &TextMatcher::regex("invalid").unwrap(),
                    match_options.ignore_case(true)
                )
        );
        assert_eq!(
            Value::Object(assertions),
            reference["assertions"],
            "{} assertion semantics differ",
            browser.kind().name()
        );
        let png = page.screenshot(ScreenshotOptions::default()).await.unwrap();
        let image = image::load_from_memory(&png).unwrap().to_rgb8();
        assert_eq!(
            json!({"width":image.width(),"height":image.height(),"background":image.get_pixel(0,0).0}),
            reference["screenshot"]
        );
        page.expose_function("double", |args| json!(args[0].as_i64().unwrap() * 2))
            .await
            .unwrap();
        let mut events = page.subscribe();
        page.goto("/redirect").await.unwrap();
        let mut redirects = Vec::new();
        while let Ok(event) = events.try_recv() {
            if let PageEvent::Response { url, status, .. } = event {
                let path = reqwest::Url::parse(&url).unwrap().path().to_owned();
                if path == "/redirect" || path == "/final" {
                    redirects.push(json!([path, status]));
                }
            }
        }
        assert_eq!(json!(redirects), reference["redirects"]);
        assert_eq!(
            page.evaluate_value("double(21)").await.unwrap(),
            reference["callback_after_navigation"]
        );
        page.goto("/api/item.js").await.unwrap();
        for case in reference["url_matchers"].as_array().unwrap() {
            let matcher = UrlMatcher::glob(case["pattern"].as_str().unwrap()).unwrap();
            let matched = page
                .wait_for_url_matching(&matcher, Duration::from_millis(70))
                .await
                .is_ok();
            assert_eq!(
                matched,
                case["matches"].as_bool().unwrap(),
                "glob case {case}"
            );
        }
        browser.close().await.unwrap();
        stop.abort();
    }
}

#[tokio::test]
async fn assertion_event_retries_validation_cancellation_and_frames() {
    for browser in browsers().await {
        let page = browser.new_page().await.unwrap();
        page.set_content(include_str!(
            "../../../scripts/e2e-conformance/fixture.html"
        ))
        .await
        .unwrap();
        let options = TextAssertionOptions::default();
        assert!(matches!(
            page.locator("#event")
                .dispatch_event_with(
                    "probe",
                    DispatchEventOptions::default()
                        .init(json!([]))
                        .composed(false)
                )
                .await,
            Err(E2eError::Config(_))
        ));
        assert!(matches!(
            page.locator("#checkbox")
                .expect()
                .checked_with(
                    CheckedOptions::default()
                        .checked(false)
                        .indeterminate(false)
                )
                .await,
            Err(E2eError::Config(_))
        ));
        assert!(matches!(
            page.locator("#event")
                .expect()
                .in_viewport_with(f64::NAN)
                .await,
            Err(E2eError::Config(_))
        ));
        let invalid = page
            .locator("#event")
            .expect()
            .timeout(SHORT)
            .not()
            .checked_with(CheckedOptions::default())
            .await
            .unwrap_err();
        assert!(invalid.to_string().contains("checkbox"), "{invalid}");
        let strict = page
            .locator(".items p")
            .expect()
            .timeout(SHORT)
            .not()
            .class_with(&"impossible".into(), MatchOptions::default())
            .await
            .unwrap_err();
        assert!(strict.to_string().contains("strict"), "{strict}");
        page.locator("#disabled")
            .expect()
            .state_with(StateAssertion::Enabled, false)
            .await
            .unwrap();
        page.locator("#enabled")
            .expect()
            .state_with(StateAssertion::Disabled, false)
            .await
            .unwrap();
        page.locator("#absent")
            .expect()
            .state_with(StateAssertion::Attached, false)
            .await
            .unwrap();
        page.locator("#absent")
            .expect()
            .state_with(StateAssertion::Visible, false)
            .await
            .unwrap();
        assert!(page
            .locator("#absent")
            .expect()
            .timeout(SHORT)
            .not()
            .state_with(StateAssertion::Enabled, true)
            .await
            .is_err());
        page.locator("#text")
            .expect()
            .not()
            .text_with(&"different".into(), options)
            .await
            .unwrap();
        page.locator("#text")
            .expect()
            .contains_text_with(
                &TextMatcher::regex("HIDDEN").unwrap(),
                options.ignore_case(true),
            )
            .await
            .unwrap();
        page.locator("#missing-list")
            .expect()
            .texts_with(&[], options)
            .await
            .unwrap();
        page.evaluate_value(
            "setTimeout(() => document.querySelector('#text').textContent = 'Later', 80);true",
        )
        .await
        .unwrap();
        page.locator("#text")
            .expect()
            .text_with(&"Later".into(), options)
            .await
            .unwrap();
        assert!(page
            .locator(".items p")
            .expect()
            .timeout(SHORT)
            .contains_texts_with(&["Alpha".into(), "Alpha".into()], options)
            .await
            .is_err());
        page.evaluate_value("setTimeout(() => { const p = document.createElement('p'); p.textContent = 'Delta'; document.querySelector('.items').appendChild(p); }, 60); true").await.unwrap();
        page.locator(".items p")
            .expect()
            .texts_with(
                &[
                    "Alpha".into(),
                    "Beta".into(),
                    TextMatcher::regex("^Gamma$").unwrap(),
                    "Delta".into(),
                ],
                options,
            )
            .await
            .unwrap();
        page.evaluate_value("document.querySelector('#classes').className = 'one\\u00a0two'; true")
            .await
            .unwrap();
        assert!(page
            .locator("#classes")
            .expect()
            .timeout(SHORT)
            .contains_class_tokens(&["one", "two"])
            .await
            .is_err());
        page.locator("#enabled")
            .expect()
            .state_with(StateAssertion::Empty, true)
            .await
            .unwrap();
        page.locator("#enabled")
            .expect()
            .state_with(StateAssertion::Editable, true)
            .await
            .unwrap();
        page.locator("#enabled").focus().await.unwrap();
        page.locator("#enabled")
            .expect()
            .state_with(StateAssertion::Focused, true)
            .await
            .unwrap();
        page.locator("#checkbox")
            .expect()
            .not()
            .checked_with(CheckedOptions::default().indeterminate(false))
            .await
            .unwrap();
        page.evaluate_value(
            "document.querySelector('#clipped').style.transform = 'translateY(-100px)'; true",
        )
        .await
        .unwrap();
        page.locator("#clipped")
            .expect()
            .not()
            .in_viewport()
            .await
            .unwrap();
        assert!(!page.locator("#clipped").in_viewport().await.unwrap());
        page.evaluate_value("setTimeout(() => document.querySelector('#accessible').setAttribute('aria-label', 'Changed Label'), 60); true").await.unwrap();
        page.locator("#accessible")
            .expect()
            .accessible_name_with(
                &"changed label".into(),
                MatchOptions::default().ignore_case(true),
            )
            .await
            .unwrap();
        page.evaluate_value("document.querySelector('#accessible').removeAttribute('aria-describedby'); document.querySelector('#accessible').setAttribute('aria-errormessage', 'missing-id'); true").await.unwrap();
        page.locator("#accessible")
            .expect()
            .accessible_description_with(&"".into(), MatchOptions::default())
            .await
            .unwrap();
        page.locator("#accessible")
            .expect()
            .not()
            .accessible_error_message_with(
                &TextMatcher::regex("invalid").unwrap(),
                MatchOptions::default().ignore_case(true),
            )
            .await
            .unwrap();
        page.evaluate_value("document.querySelector('#event').addEventListener('legacy', e => window.legacy = { detail: e.detail, constructor: e.constructor.name, bubbles: e.bubbles, cancelable: e.cancelable, composed: e.composed }); document.querySelector('#event').addEventListener('flags', e => { e.preventDefault(); window.flags = [e.bubbles, e.cancelable, e.composed, e.defaultPrevented]; });true").await.unwrap();
        page.locator("#event")
            .dispatch_event("legacy", Some(&json!({"binary":1})))
            .await
            .unwrap();
        assert_eq!(
            page.evaluate_value("legacy").await.unwrap(),
            json!({"detail":{"binary":1},"constructor":"CustomEvent","bubbles":true,"cancelable":false,"composed":false})
        );
        page.locator("#event")
            .dispatch_event_with(
                "flags",
                DispatchEventOptions::default()
                    .bubbles(false)
                    .cancelable(false)
                    .composed(false),
            )
            .await
            .unwrap();
        assert_eq!(
            page.evaluate_value("flags").await.unwrap(),
            json!([false, false, false, false])
        );
        page.set_content("<iframe id=child style='width:120px;height:120px' srcdoc=\"<div style='width:40px;height:40px;overflow:hidden'><div id=inner style='height:80px;width:40px'></div></div>\"></iframe>").await.unwrap();
        page.frame_locator("#child")
            .locator("#inner")
            .expect()
            .in_viewport_with(0.4)
            .await
            .unwrap();
        page.frame_locator("#child")
            .locator("#inner")
            .expect()
            .not()
            .in_viewport_with(0.8)
            .await
            .unwrap();
        let token = CancellationToken::new();
        let scoped = page.with_cancellation(token.clone());
        let locator = scoped.locator("#never");
        let expectation = locator.expect().timeout(Duration::ZERO);
        let expected = TextMatcher::exact("never");
        let (result, _) = tokio::join!(expectation.text_with(&expected, options), async {
            tokio::time::sleep(Duration::from_millis(30)).await;
            token.cancel();
        });
        assert!(matches!(result, Err(E2eError::Cancelled(_))));
        let locator = page.locator("#never");
        let expectation = locator.expect().timeout(Duration::ZERO);
        let (result, close) = tokio::join!(expectation.text_with(&expected, options), async {
            tokio::time::sleep(Duration::from_millis(30)).await;
            page.close().await
        });
        close.unwrap();
        assert!(matches!(
            result,
            Err(E2eError::Cancelled(_) | E2eError::Disconnected(_))
        ));
        let output = tempfile::tempdir().unwrap();
        let report = Runner::default()
            .workers(1)
            .list_progress(false)
            .output_dir(output.path().display().to_string())
            .run(
                &browser,
                vec![test(
                    "options APIs run in a Send test future",
                    |page| async move {
                        page.set_content("<p class='ready visible'>Ready</p>")
                            .await?;
                        page.locator("p")
                            .expect()
                            .text_with(
                                &"ready".into(),
                                TextAssertionOptions::default().ignore_case(true),
                            )
                            .await?;
                        page.locator("p")
                            .expect()
                            .contains_class_tokens(&["visible", "ready"])
                            .await?;
                        page.locator("p").expect().in_viewport_with(0.5).await
                    },
                )],
            )
            .await;
        assert!(report.ok(), "{}", report.to_list());
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn explicit_native_engine_capabilities_and_unsupported_results() {
    for browser in browsers().await {
        let page = browser.new_page().await.unwrap();
        page.set_content("<script>window.covered = () => 42;</script>")
            .await
            .unwrap();
        let operations = [
            (
                "media",
                page.emulate_media(Some(ColorScheme::Dark), None).await,
            ),
            ("locale", page.set_locale("fr-FR").await),
            ("timezone", page.set_timezone("UTC").await),
            (
                "headers",
                page.set_extra_http_headers(&[("x-native", "true")]).await,
            ),
            ("garbage collection", page.request_gc().await),
        ];
        for (name, result) in operations {
            match browser.kind() {
                BrowserKind::Chromium => assert!(result.is_ok(), "{name}: {result:?}"),
                BrowserKind::Firefox => assert!(
                    matches!(result, Err(E2eError::Config(_))),
                    "{name} must report an explicit unsupported configuration: {result:?}"
                ),
            }
        }
        let coverage = page.coverage();
        let result = coverage.start_js_coverage().await;
        if browser.kind() == BrowserKind::Chromium {
            result.unwrap();
            page.evaluate_value("covered()").await.unwrap();
            assert!(!coverage.stop_js_coverage().await.unwrap().is_empty());
            assert_eq!(
                page.evaluate_value("matchMedia('(prefers-color-scheme:dark)').matches")
                    .await
                    .unwrap(),
                true
            );
        } else {
            assert!(matches!(result, Err(E2eError::Config(_))));
        }
        assert_eq!(page.evaluate_value("40 + 2").await.unwrap(), 42);
        browser.close().await.unwrap();
    }
}
