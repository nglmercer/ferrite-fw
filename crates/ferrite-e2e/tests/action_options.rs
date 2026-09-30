use ferrite_e2e::*;
use serde_json::{json, Value};
use std::time::Duration;
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
                "action native {}: {}",
                kind.name(),
                browser.version().await.unwrap()
            );
            result.push(browser);
        } else {
            eprintln!("action browser absent: {}", kind.name());
        }
    }
    if std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_some() {
        assert_eq!(result.len(), 2);
    }
    result
}
const FIXTURE: &str = r#"<style>body{margin:0}button{position:absolute;left:30px;top:40px;width:120px;height:80px;padding:10px;border:4px solid black}#drag{position:absolute;left:40px;top:160px;width:60px;height:40px;background:red}#target{position:absolute;left:160px;top:160px;width:80px;height:40px;background:green}#mask{position:absolute;left:75px;top:65px;width:30px;height:40px;background:blue;z-index:2}input{position:absolute;left:220px;top:40px}</style><button id='button'>ready</button><div id='mask'></div><input id='check' type='checkbox'><div id='drag'></div><div id='target'></div><script>
window.input=[];window.dragMoves=0;
for(const type of ['mousedown','mouseup','click','mousemove','keydown','keyup'])document.addEventListener(type,e=>{input.push({type,x:e.clientX||0,y:e.clientY||0,shift:e.shiftKey,control:e.ctrlKey,alt:e.altKey,meta:e.metaKey,buttons:e.buttons||0,target:e.target.id});if(type==='mousemove' && e.buttons)dragMoves++;});
</script>"#;
#[tokio::test]
async fn positions_modifiers_trials_checks_drag_and_frame_coordinates() {
    for browser in browsers().await {
        eprintln!("checking actions {}", browser.kind().name());
        let page = browser.new_page().await.unwrap();
        page.set_viewport(Viewport {
            width: 640,
            height: 480,
        })
        .await
        .unwrap();
        page.set_content(FIXTURE).await.unwrap();
        let button = page.locator("#button");
        // Covered center does not block an explicitly uncovered padding point.
        button
            .click_with_options(
                ClickOptions::default()
                    .position(8.0, 12.0)
                    .modifiers(&[KeyboardModifier::Shift, KeyboardModifier::Control]),
            )
            .await
            .unwrap();
        let events: Vec<Value> = page.evaluate("input").await.unwrap();
        let click = events.iter().find(|e| e["type"] == "click").unwrap();
        assert_eq!(click["x"], 42);
        assert_eq!(click["y"], 56);
        assert_eq!(click["shift"], true);
        assert_eq!(click["control"], true);
        page.evaluate_value("input=[]; true").await.unwrap();
        button
            .click_with_options(
                ClickOptions::default()
                    .position(8.0, 12.0)
                    .modifiers(&[KeyboardModifier::Alt])
                    .trial(true),
            )
            .await
            .unwrap();
        button
            .hover_with_options(
                ActionOptions::default()
                    .position(8.0, 12.0)
                    .modifiers(&[KeyboardModifier::Meta])
                    .trial(true),
            )
            .await
            .unwrap();
        page.locator("#check")
            .check_with_options(ActionOptions::default().trial(true))
            .await
            .unwrap();
        page.locator("#drag")
            .drag_to_with_options(
                &page.locator("#target"),
                DragOptions::default()
                    .action(ActionOptions::default().trial(true))
                    .steps(4),
            )
            .await
            .unwrap();
        assert_eq!(
            page.evaluate_value("input.length").await.unwrap(),
            0,
            "trials send no mouse/key input"
        );
        assert!(!page.locator("#check").is_checked().await.unwrap());
        button
            .hover_with_options(
                ActionOptions::default()
                    .position(12.0, 14.0)
                    .modifiers(&[KeyboardModifier::Alt]),
            )
            .await
            .unwrap();
        let events: Vec<Value> = page.evaluate("input").await.unwrap();
        assert!(events
            .iter()
            .any(|e| e["type"] == "mousemove" && e["alt"] == true));
        page.locator("#check")
            .check_with_options(ActionOptions::default().modifiers(&[KeyboardModifier::Shift]))
            .await
            .unwrap();
        assert!(page.locator("#check").is_checked().await.unwrap());
        page.locator("#check")
            .uncheck_with_options(ActionOptions::default())
            .await
            .unwrap();
        assert!(!page.locator("#check").is_checked().await.unwrap());
        page.locator("#drag")
            .drag_to_with_options(
                &page.locator("#target"),
                DragOptions::default()
                    .action(
                        ActionOptions::default()
                            .position(3.0, 5.0)
                            .modifiers(&[KeyboardModifier::Control]),
                    )
                    .target_position(12.0, 8.0)
                    .steps(4),
            )
            .await
            .unwrap();
        assert!(page.evaluate::<u32>("dragMoves").await.unwrap() > 0);
        assert!(matches!(
            button
                .click_with_options(ClickOptions::default().position(f64::NAN, 0.0))
                .await,
            Err(E2eError::Config(_))
        ));
        assert!(button
            .click_with_options(ClickOptions::default().position(500.0, 0.0))
            .await
            .unwrap_err()
            .to_string()
            .contains("padding box"));
        let original = page.timeout();
        assert!(matches!(
            button
                .click_with_options(ClickOptions::default().timeout(Duration::from_millis(100)))
                .await,
            Err(E2eError::Timeout(..))
        ));
        assert_eq!(
            page.timeout(),
            original,
            "option timeout cannot mutate the page default"
        );
        page.evaluate_value("document.querySelector('#mask').remove(); true")
            .await
            .unwrap();
        page.key_down("Shift").await.unwrap();
        button
            .click_with_options(
                ClickOptions::default()
                    .modifiers(&[KeyboardModifier::Shift, KeyboardModifier::Alt]),
            )
            .await
            .unwrap();
        page.evaluate_value("input=[]; true").await.unwrap();
        button.click().await.unwrap();
        let events: Vec<Value> = page.evaluate("input").await.unwrap();
        let click = events.iter().find(|e| e["type"] == "click").unwrap();
        assert_eq!(click["shift"], true);
        assert_eq!(
            click["alt"], false,
            "action-acquired modifiers released, existing Shift preserved"
        );
        page.key_up("Shift").await.unwrap();
        page.set_content("<iframe id='frame' style='position:absolute;left:50px;top:60px;border:2px solid black' srcdoc=\"<body style='margin:0'><button id='inner' style='margin:10px;width:80px;height:40px;border:3px solid black'>inner</button><script>onclick=e=>window.point=[e.clientX,e.clientY,e.shiftKey]</script>\"></iframe>").await.unwrap();
        page.frame_locator("#frame")
            .locator("#inner")
            .click_with_options(
                ClickOptions::default()
                    .position(5.0, 7.0)
                    .modifiers(&[KeyboardModifier::Shift]),
            )
            .await
            .unwrap();
        assert_eq!(
            page.evaluate_value("frames[0].point").await.unwrap(),
            json!([18, 20, true])
        );
        page.evaluate_value("document.querySelector('#frame').style.transformOrigin='0 0';document.querySelector('#frame').style.transform='scale(.5)'; true").await.unwrap();
        let inner = page.frame_locator("#frame").locator("#inner");
        let state = inner.state().await.unwrap();
        let rect = &state.rects[0];
        assert_eq!(rect.x, 56.0);
        assert_eq!(rect.width, 40.0);
        inner
            .click_with_options(ClickOptions::default().position(5.0, 7.0))
            .await
            .unwrap();
        assert_eq!(
            page.evaluate_value("frames[0].point").await.unwrap(),
            json!([18, 20, false])
        );
        page.close().await.unwrap();
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn canceled_pointer_action_releases_input_and_modifiers() {
    for browser in browsers().await {
        eprintln!("checking actions {}", browser.kind().name());
        let page = browser.new_page().await.unwrap();
        page.set_content(FIXTURE).await.unwrap();
        page.evaluate_value("document.querySelector('#mask').remove(); true")
            .await
            .unwrap();
        let cancellation = CancellationToken::new();
        let locator = page
            .locator("#button")
            .with_cancellation(cancellation.clone());
        let options = ClickOptions {
            delay: Duration::from_secs(10),
            modifiers: vec![KeyboardModifier::Shift, KeyboardModifier::Alt],
            timeout: Some(Duration::ZERO),
            ..Default::default()
        };
        let action = locator.click_with_options(options);
        let cancel = async {
            page.wait_for_function(
                "input.some(e=>e.type==='mousedown')",
                Duration::from_secs(3),
            )
            .await
            .unwrap();
            cancellation.cancel();
        };
        let (result, ()) = tokio::join!(action, cancel);
        assert!(matches!(result, Err(E2eError::Cancelled(_))));
        page.wait_for_function(
            "input.some(e=>e.type==='keyup' && !e.shift && !e.alt)",
            Duration::from_secs(3),
        )
        .await
        .unwrap();
        page.evaluate_value("input=[]; true").await.unwrap();
        page.locator("#button").click().await.unwrap();
        let events: Vec<Value> = page.evaluate("input").await.unwrap();
        let click = events.iter().find(|e| e["type"] == "click").unwrap();
        assert_eq!(click["shift"], false);
        assert_eq!(click["alt"], false);
        assert!(events
            .iter()
            .filter(|e| e["type"] == "mousemove")
            .all(|e| e["buttons"] == 0));
        page.evaluate_value("input=[]; true").await.unwrap();
        let result = page
            .locator("#button")
            .click_with_options(ClickOptions {
                delay: Duration::from_secs(10),
                modifiers: vec![KeyboardModifier::Shift],
                // Leave native readiness/input acquisition time inside the
                // budget so this tests timeout while input is actually held.
                timeout: Some(Duration::from_secs(2)),
                ..Default::default()
            })
            .await;
        assert!(matches!(result, Err(E2eError::Timeout(..))));
        let acquired: Vec<Value> = page.evaluate("input").await.unwrap();
        assert!(acquired
            .iter()
            .any(|e| e["type"] == "keydown" && e["shift"] == true));
        assert!(acquired.iter().any(|e| e["type"] == "mousedown"));
        page.wait_for_function(
            "input.some(e=>e.type==='keyup' && !e.shift)",
            Duration::from_secs(3),
        )
        .await
        .unwrap();
        page.evaluate_value("input=[]; true").await.unwrap();
        page.locator("#button").click().await.unwrap();
        let events: Vec<Value> = page.evaluate("input").await.unwrap();
        assert!(events
            .iter()
            .filter(|e| e["type"] == "click")
            .all(|e| e["shift"] == false));
        page.close().await.unwrap();
        browser.close().await.unwrap();
    }
}
