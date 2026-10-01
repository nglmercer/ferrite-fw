use ferrite_e2e::*;
use serde_json::{json, Value};

async fn browsers() -> Vec<Browser> {
    let mut result = Vec::new();
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let path = match kind {
            BrowserKind::Chromium => find_chromium(None),
            BrowserKind::Firefox => find_firefox(None),
        };
        if let Some(path) = path {
            result.push(
                Browser::launch(LaunchOptions::default().browser(kind).executable(path))
                    .await
                    .unwrap(),
            );
        }
    }
    if std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_some() {
        assert_eq!(result.len(), 2);
    }
    result
}
fn flatten(value: &Value) -> Vec<&Value> {
    let mut result = Vec::new();
    for node in value.as_array().unwrap() {
        result.push(node);
        if let Some(children) = node.get("children") {
            result.extend(flatten(children));
        }
    }
    result
}
const HTML: &str = r#"<section id="root" role="region" aria-label="Root" style="position:absolute;left:20.4px;top:30.4px;width:140.4px"><div><div role="group" aria-label="Nested"><h2>Heading</h2><button aria-pressed="true">Push</button><input id="mixed" type="checkbox" aria-label="Mixed"></div></div><div id="shadow"></div><button hidden>Hidden</button></section>"#;

#[tokio::test]
async fn depth_states_shadow_and_assertions() {
    for browser in browsers().await {
        let page = browser.new_page().await.unwrap();
        page.set_content(HTML).await.unwrap();
        page.evaluate_value("document.querySelector('#mixed').indeterminate=true;document.querySelector('#shadow').attachShadow({mode:'open'}).innerHTML='<div role=group aria-label=Shadow><button disabled>Inside</button></div>';true").await.unwrap();
        let root = page.locator("#root");
        let options = AriaSnapshotOptions::default();
        let full = root.aria_snapshot_json_with(options).await.unwrap();
        let nodes = flatten(&full);
        assert_eq!(nodes.len(), 7);
        assert!(nodes.iter().any(|node| node["checked"] == "mixed"));
        assert!(nodes.iter().any(|node| node["disabled"] == true));
        assert!(nodes.iter().all(|node| node.get("box").is_none()));
        assert_eq!(
            full,
            root.aria_snapshot_json_with(AriaSnapshotOptions {
                depth: -1,
                ..options
            })
            .await
            .unwrap()
        );
        let depth = AriaSnapshotOptions {
            depth: 1,
            ..options
        };
        let limited = root.aria_snapshot_json_with(depth).await.unwrap();
        assert_eq!(flatten(&limited).len(), 3);
        assert!(limited[0]["children"][0].get("children").is_none());
        let plain = AriaSnapshotOptions {
            states: false,
            boxes: true,
            ..options
        };
        let value = root.aria_snapshot_json_with(plain).await.unwrap();
        for node in flatten(&value) {
            assert!(node.get("box").is_some());
            for key in ["checked", "pressed", "disabled", "level"] {
                assert!(node.get(key).is_none());
            }
        }
        assert_eq!(value[0]["box"]["x"], 20);
        assert_eq!(value[0]["box"]["y"], 30);
        assert_eq!(value[0]["box"]["width"], 140);
        let text = root.aria_snapshot_with(plain).await.unwrap();
        root.expect()
            .aria_snapshot_with(&text, plain)
            .await
            .unwrap();
        let text = page.aria_snapshot_with(depth).await.unwrap();
        page.expect()
            .aria_snapshot_with(&text, depth)
            .await
            .unwrap();
        assert_eq!(limited, page.aria_snapshot_json_with(depth).await.unwrap());
        assert!(matches!(
            root.expect()
                .aria_snapshot_with(
                    "",
                    AriaSnapshotOptions {
                        max_nodes: 0,
                        ..options
                    }
                )
                .await,
            Err(E2eError::Config(_))
        ));
        let short = page.with_timeout(std::time::Duration::from_millis(50));
        assert!(matches!(
            short.locator("#absent").aria_snapshot_with(options).await,
            Err(E2eError::Timeout(..))
        ));
        let token = CancellationToken::new();
        token.cancel();
        let cancelled = page.with_cancellation(token);
        assert!(matches!(
            cancelled.aria_snapshot_json_with(options).await,
            Err(E2eError::Cancelled(_))
        ));
        assert!(matches!(
            cancelled.locator("#root").aria_snapshot_with(options).await,
            Err(E2eError::Cancelled(_))
        ));
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn deterministic_safety_limits() {
    for browser in browsers().await {
        let page = browser.new_page().await.unwrap();
        page.set_content("<main aria-label='long name'><div><button>One</button><button>Two</button></div></main>").await.unwrap();
        let options = AriaSnapshotOptions {
            max_nodes: 2,
            max_name_chars: 4,
            ..Default::default()
        };
        let actual = page.aria_snapshot_json_with(options).await.unwrap();
        assert_eq!(actual[0]["name"], "long…");
        assert_eq!(
            actual[0]["children"][1],
            json!({"role":"truncated","name":"Role node limit"})
        );
        assert_eq!(actual, page.aria_snapshot_json_with(options).await.unwrap());
        let actual = page
            .aria_snapshot_json_with(AriaSnapshotOptions {
                max_dom_nodes: 1,
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(
            actual,
            json!([{"role":"truncated","name":"DOM visit limit"}])
        );
        page.set_content("<main aria-label='😀😀'></main>")
            .await
            .unwrap();
        let unicode = page
            .aria_snapshot_json_with(AriaSnapshotOptions {
                max_name_chars: 1,
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(unicode[0]["name"], "…");
        page.evaluate_value("let p=document.body;p.innerHTML='';for(let i=0;i<100;i++){const n=document.createElement('div');n.setAttribute('role','group');n.setAttribute('aria-label','node');p.append(n);p=n;}true").await.unwrap();
        let actual = page
            .aria_snapshot_json_with(Default::default())
            .await
            .unwrap();
        let nodes = flatten(&actual);
        assert_eq!(nodes.len(), 62);
        assert_eq!(nodes.last().unwrap()["name"], "Safety depth limit");
        assert!(page
            .aria_snapshot_with(Default::default())
            .await
            .unwrap()
            .contains("Safety depth limit"));
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn same_origin_frame_boxes_and_scrolling() {
    for browser in browsers().await {
        let page = browser.new_page().await.unwrap();
        page.set_content(r#"<iframe style="position:absolute;left:210px;top:40px;border:2px solid" srcdoc="<div role='group' aria-labelledby='label' id='root' style='position:absolute;left:20px;top:40px;width:100px;height:60px'><span id='label'>Frame</span><button>Child</button></div>"></iframe>"#).await.unwrap();
        let root = page.frame_locator("iframe").locator("#root");
        let options = AriaSnapshotOptions {
            boxes: true,
            ..Default::default()
        };
        let value = root.aria_snapshot_json_with(options).await.unwrap();
        assert_eq!(value[0]["name"], "Frame");
        assert_eq!(
            value[0]["box"],
            json!({"x":20,"y":40,"width":100,"height":60})
        );
        root.expect()
            .aria_snapshot_with(&root.aria_snapshot_with(options).await.unwrap(), options)
            .await
            .unwrap();
        page.set_content("<main aria-label='scroll' style='position:absolute;left:20px;top:300px;width:100px;height:60px'></main><div style='height:2000px'></div>").await.unwrap();
        page.evaluate_value("window.scrollTo(0,100);true")
            .await
            .unwrap();
        let value = page
            .locator("main")
            .aria_snapshot_json_with(options)
            .await
            .unwrap();
        assert_eq!(value[0]["box"]["y"], 200);
        browser.close().await.unwrap();
    }
}
