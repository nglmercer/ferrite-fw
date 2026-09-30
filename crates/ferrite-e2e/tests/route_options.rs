//! Native real-fetch and fulfillment options, context affinity and budgets.
use ferrite_e2e::*;
#[path = "common/tls.rs"]
mod tls;
use serde_json::{json, Value};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
async fn fixture() -> (String, tokio::task::AbortHandle) {
    let app=axum::Router::new()
        .route("/",axum::routing::get(||async{"fixture"}))
        .route("/echo",axum::routing::any(|method:axum::http::Method,headers:axum::http::HeaderMap,body:axum::body::Bytes|async move{
            let value=|name:&str|headers.get(name).and_then(|v|v.to_str().ok()).unwrap_or("").to_string();
            ([("set-cookie","fetched=1; Path=/"),("x-source","original")],axum::Json(json!({"method":method.as_str(),"bytes":body.to_vec(),"body":String::from_utf8_lossy(&body),"cookie":value("cookie"),"extra":value("x-extra"),"auth":value("authorization"),"type":value("content-type")})))
        }))
        .route("/redirect",axum::routing::get(||async{(axum::http::StatusCode::FOUND,[("location","/echo"),("set-cookie","redirected=1; Path=/")],"redirect")}))
        .route("/slow",axum::routing::get(||async{tokio::time::sleep(Duration::from_millis(150)).await;"slow"}))
        .route("/binary",axum::routing::get(||async{([( "content-type","application/octet-stream"),("set-cookie","binary=1; Path=/"),("x-source","original")],vec![0u8,255,128,13,10])}));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (base, handle.abort_handle())
}
async fn browsers(base: &str) -> Vec<Browser> {
    browsers_with_tls(base, false).await
}
async fn browsers_with_tls(base: &str, ignore_https_errors: bool) -> Vec<Browser> {
    let mut result = Vec::new();
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let executable = match kind {
            BrowserKind::Chromium => find_chromium(None),
            BrowserKind::Firefox => find_firefox(None),
        };
        if let Some(executable) = executable {
            let mut browser = Browser::launch(LaunchOptions {
                ignore_https_errors,
                ..LaunchOptions::default()
                    .browser(kind)
                    .executable(executable)
            })
            .await
            .unwrap();
            browser.set_base_url(Some(base.into()));
            eprintln!(
                "route options {}: {}",
                kind.name(),
                browser.version().await.unwrap()
            );
            result.push(browser);
        }
    }
    if std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_some() {
        assert_eq!(result.len(), 2);
    }
    result
}
#[tokio::test]
async fn native_fetch_context_overrides_original_body_and_fulfillment() {
    let (base, stop) = fixture().await;
    for browser in browsers(&base).await {
        let kind = browser.kind();
        let context = browser
            .new_context(ContextOptions {
                extra_http_headers: if kind == BrowserKind::Chromium {
                    vec![
                        ("x-extra".into(), "context".into()),
                        ("authorization".into(), "Basic fixture".into()),
                    ]
                } else {
                    vec![]
                },
                ..Default::default()
            })
            .await
            .unwrap();
        context
            .add_cookies(
                &[Cookie {
                    name: "before".into(),
                    value: "1".into(),
                    domain: None,
                    path: Some("/".into()),
                    http_only: false,
                    secure: false,
                    same_site: None,
                    expires: None,
                }],
                &base,
            )
            .await
            .unwrap();
        let page = context.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        let retained = Arc::new(Mutex::new(None));
        let stored = retained.clone();
        page.route_with_handler("**/echo", move |info| {
            let stored = stored.clone();
            async move {
                *stored.lock().unwrap() = Some(info.clone());
                let response = info
                    .fetch_with(RouteFetchOptions {
                        method: Some("put".into()),
                        body: Some(vec![0, 255, 10]),
                        headers: Some({
                            let mut headers = vec![
                                ("content-type".into(), "application/octet-stream".into()),
                                ("content-length".into(), "999".into()),
                            ];
                            if kind == BrowserKind::Firefox {
                                headers.extend([
                                    ("x-extra".into(), "override".into()),
                                    ("authorization".into(), "Basic fixture".into()),
                                ]);
                            }
                            headers
                        }),
                        ..Default::default()
                    })
                    .await?;
                let value: Value = response.json()?;
                assert_eq!(value["method"], "PUT");
                assert_eq!(
                    value["bytes"],
                    reference("fetch-overrides")["details"]["bytes"]
                );
                assert_eq!(
                    value["type"],
                    reference("fetch-overrides")["details"]["type"]
                );
                assert!(value["cookie"].as_str().unwrap().contains("before=1"));
                assert_eq!(
                    value["extra"],
                    if kind == BrowserKind::Chromium {
                        "context"
                    } else {
                        "override"
                    }
                );
                assert_eq!(value["auth"], "Basic fixture");
                info.fulfill_with(RouteFulfillOptions {
                    response: Some(response),
                    status: Some(201),
                    json: Some(json!({"changed":true})),
                    ..Default::default()
                })
                .await
            }
        })
        .await
        .unwrap();
        let value:Value=page.evaluate("fetch('/echo',{method:'POST',body:'original'}).then(async r=>({status:r.status,type:r.headers.get('content-type'),value:await r.json()}))").await.unwrap();
        assert_eq!(
            value,
            json!({"status":201,"type":"application/json","value":{"changed":true}})
        );
        assert!(context
            .cookies()
            .await
            .unwrap()
            .iter()
            .any(|c| c.name == "fetched"));
        page.unroute_all().await.unwrap();
        let kind = browser.kind();
        page.route_with_handler("**/echo",move|info|async move {
            if kind==BrowserKind::Chromium {
                assert_eq!(info.body_state(),RouteBodyState::Captured);
                let response=info.fetch().await?;
                let value:Value=response.json()?;
                assert_eq!(value["bytes"],json!([0,255,128,13,10]));
                info.fulfill_with(RouteFulfillOptions {response:Some(response),..Default::default()}).await
            } else {
                assert_eq!(info.body_state(),RouteBodyState::Unavailable);
                assert!(matches!(info.fetch().await,Err(E2eError::Config(message))if message.contains("body is unavailable")));
                Ok(RouteAction::fulfill(200,"unavailable","text/plain"))
            }
        }).await.unwrap();
        let result:String=page.evaluate("fetch('/echo',{method:'POST',body:new Uint8Array([0,255,128,13,10])}).then(r=>r.text())").await.unwrap();
        assert!(result.contains(if kind == BrowserKind::Chromium {
            "bytes"
        } else {
            "unavailable"
        }));
        page.unroute_all().await.unwrap();
        page.route_with_handler("**/echo", |info| async move {
            let response = info
                .fetch_with(RouteFetchOptions {
                    url: Some("/echo".into()),
                    method: Some("post".into()),
                    headers: Some(vec![]),
                    json: Some(json!({"changed":true})),
                    ..Default::default()
                })
                .await?;
            let value: Value = response.json()?;
            assert_eq!(
                value["method"],
                reference("fetch-json")["details"]["method"]
            );
            assert_eq!(value["bytes"], reference("fetch-json")["details"]["bytes"]);
            assert_eq!(value["type"], reference("fetch-json")["details"]["type"]);
            info.fulfill_with(RouteFulfillOptions {
                response: Some(response),
                ..Default::default()
            })
            .await
        })
        .await
        .unwrap();
        let _: Value = page
            .evaluate("fetch('/echo',{method:'POST',body:'original'}).then(r=>r.json())")
            .await
            .unwrap();
        page.unroute_all().await.unwrap();
        let binary_url = format!("{base}/binary");
        page.route_with_handler("**/echo", move |info| {
            let binary_url = binary_url.clone();
            async move {
                let response = info
                    .fetch_with(RouteFetchOptions {
                        url: Some(binary_url),
                        headers: Some(vec![]),
                        ..Default::default()
                    })
                    .await?;
                info.fulfill_with(RouteFulfillOptions {
                    response: Some(response),
                    ..Default::default()
                })
                .await
            }
        })
        .await
        .unwrap();
        let bytes: Vec<u8> = page
            .evaluate(
                "fetch('/echo').then(r=>r.arrayBuffer()).then(b=>Array.from(new Uint8Array(b)))",
            )
            .await
            .unwrap();
        assert_eq!(bytes, vec![0, 255, 128, 13, 10]);
        page.close().await.unwrap();
        let info = retained.lock().unwrap().take().unwrap();
        assert!(matches!(
            info.fetch_with(RouteFetchOptions {
                body: Some(vec![]),
                ..Default::default()
            })
            .await,
            Err(E2eError::Cancelled(_))
        ));
        context.close().await.unwrap();
        browser.close().await.unwrap();
    }
    stop.abort();
}
#[tokio::test]
async fn detached_fetch_redirects_and_fulfillment_validation_files_and_cancellation() {
    let (base, stop) = fixture().await;
    let info = RouteInfo::new(format!("{base}/redirect"), "GET", vec![], None);
    let response = info
        .fetch_with(RouteFetchOptions {
            max_redirects: Some(0),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(response.status(), 302);
    assert_eq!(response.text(), "redirect");
    assert!(matches!(
        info.fetch_with(RouteFetchOptions {
            body: Some(vec![]),
            json: Some(json!({})),
            ..Default::default()
        })
        .await,
        Err(E2eError::Config(_))
    ));
    assert!(matches!(
        info.fetch_with(RouteFetchOptions {
            url: Some("file:///tmp/x".into()),
            ..Default::default()
        })
        .await,
        Err(E2eError::Config(_))
    ));
    let api = ApiClient::with_options(ApiClientOptions {
        base_url: Some(base.clone()),
        headers: vec![
            ("content-type".into(), "application/x-default".into()),
            ("content-length".into(), "999".into()),
        ],
        ..Default::default()
    })
    .unwrap();
    let value: Value = api
        .fetch_with(
            "post",
            "/echo",
            ApiRequestOptions {
                json: Some(json!({"changed":true})),
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(value["method"], "POST");
    assert_eq!(value["type"], "application/x-default");
    assert_eq!(value["body"], "{\"changed\":true}");
    let slow = RouteInfo::new(format!("{base}/slow"), "GET", vec![], None);
    assert!(matches!(
        slow.fetch_with(RouteFetchOptions {
            timeout: Some(Duration::from_millis(20)),
            ..Default::default()
        })
        .await,
        Err(E2eError::Timeout(_, _))
    ));
    assert_eq!(
        slow.fetch_with(RouteFetchOptions {
            timeout: Some(Duration::ZERO),
            ..Default::default()
        })
        .await
        .unwrap()
        .text(),
        "slow"
    );
    let token = CancellationToken::new();
    token.cancel();
    assert!(matches!(
        slow.fetch_with(RouteFetchOptions {
            cancellation: Some(token.clone()),
            ..Default::default()
        })
        .await,
        Err(E2eError::Cancelled(_))
    ));
    assert!(matches!(
        RouteAction::fulfill_with(RouteFulfillOptions {
            cancellation: Some(token),
            ..Default::default()
        })
        .await,
        Err(E2eError::Cancelled(_))
    ));
    for options in [
        RouteFulfillOptions {
            status: Some(999),
            ..Default::default()
        },
        RouteFulfillOptions {
            body: Some(vec![]),
            json: Some(Value::Null),
            ..Default::default()
        },
        RouteFulfillOptions {
            headers: Some(vec![("x".into(), "unsafe\r\nvalue".into())]),
            ..Default::default()
        },
        RouteFulfillOptions {
            status_text: Some("bad\nphrase".into()),
            ..Default::default()
        },
    ] {
        assert!(RouteAction::fulfill_with(options).await.is_err());
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fixture.json");
    tokio::fs::write(&path, b"{\"file\":true}").await.unwrap();
    let action = RouteAction::fulfill_with(RouteFulfillOptions {
        path: Some(path.clone()),
        timeout: Some(Duration::ZERO),
        headers: Some(vec![
            ("content-type".into(), "text/plain".into()),
            ("set-cookie".into(), "one=1".into()),
            ("Set-Cookie".into(), "two=2".into()),
        ]),
        ..Default::default()
    })
    .await
    .unwrap();
    let RouteAction::Fulfill { headers, body, .. } = action else {
        panic!()
    };
    assert_eq!(body, b"{\"file\":true}");
    assert_eq!(
        headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
            .unwrap()
            .1,
        "application/json"
    );
    assert_eq!(
        headers
            .iter()
            .filter(|(name, _)| name.eq_ignore_ascii_case("set-cookie"))
            .count(),
        2
    );
    assert!(matches!(
        RouteAction::fulfill_with(RouteFulfillOptions {
            path: Some(dir.path().join("missing")),
            ..Default::default()
        })
        .await,
        Err(E2eError::Io(_))
    ));
    assert!(matches!(
        RouteAction::fulfill_with(RouteFulfillOptions {
            path: Some(dir.path().to_path_buf()),
            ..Default::default()
        }).await,
        Err(E2eError::Config(message)) if message.contains("regular file")
    ));
    #[cfg(unix)]
    {
        let socket = dir.path().join("socket.txt");
        let _listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        assert!(matches!(
            RouteAction::fulfill_with(RouteFulfillOptions {path:Some(socket),..Default::default()}).await,
            Err(E2eError::Config(message)) if message.contains("regular file")
        ));
    }
    stop.abort();
}

fn reference(name: &str) -> Value {
    let data: Value = serde_json::from_str(include_str!(
        "../../../scripts/e2e-conformance/route-options-reference.json"
    ))
    .unwrap();
    assert_eq!(data["playwright"], "1.63.0");
    data["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == name)
        .unwrap()
        .clone()
}

#[tokio::test]
async fn pinned_native_cross_origin_fulfillment_and_explicit_cors_headers() {
    let (base, stop) = fixture().await;
    let cross = format!("{}/cors-target", base.replace("127.0.0.1", "localhost"));
    for browser in browsers(&base).await {
        let context = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        let page = context.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        for explicit in [false, true] {
            page.route_with_handler("**/cors-target", move |info| async move {
                assert!(info
                    .headers
                    .iter()
                    .any(|(name, _)| name.eq_ignore_ascii_case("origin")));
                let mut headers = vec![(
                    "access-control-expose-headers".into(),
                    "Access-Control-Allow-Origin, Access-Control-Allow-Credentials, Vary".into(),
                )];
                if explicit {
                    headers.push(("access-control-allow-origin".into(), "*".into()));
                }
                info.fulfill_with(RouteFulfillOptions {
                    body: Some(b"cors bytes".to_vec()),
                    headers: Some(headers),
                    ..Default::default()
                })
                .await
            })
            .await
            .unwrap();
            let value:Value=page.evaluate(&format!(
                "fetch({},{{credentials:'{}'}}).then(async r=>({{status:r.status,bytes:Array.from(new Uint8Array(await r.arrayBuffer())),originAccepted:r.headers.get('access-control-allow-origin')==={},credentials:r.headers.get('access-control-allow-credentials'),vary:r.headers.get('vary')}}))",
                json!(cross), if explicit {"omit"} else {"include"}, if explicit {"'*'"} else {"location.origin"}
            )).await.unwrap();
            let name = if explicit {
                "cors-explicit-headers"
            } else {
                "cors-contextual-fulfill"
            };
            assert_eq!(
                value,
                reference(name)["result"],
                "{}: {name}",
                browser.kind().name()
            );
            page.unroute_all().await.unwrap();
        }
        context.close().await.unwrap();
        browser.close().await.unwrap();
    }
    stop.abort();
}

#[tokio::test]
async fn retained_route_fetch_and_fulfillment_observe_native_transport_loss() {
    let (base, stop) = fixture().await;
    let _stop = AbortOnDrop(stop);
    for browser in browsers(&base).await {
        let context = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        let page = context.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        let retained = Arc::new(Mutex::new(None));
        let stored = retained.clone();
        page.route_with_handler("**/probe", move |info| {
            *stored.lock().unwrap() = Some(info);
            async { Ok(RouteAction::fulfill(200, "probe", "text/plain")) }
        })
        .await
        .unwrap();
        page.evaluate_value("fetch('/probe').then(r=>r.text())")
            .await
            .unwrap();
        let info = retained.lock().unwrap().take().unwrap();
        page.unroute_all().await.unwrap();
        let (result, ()) = tokio::join!(
            info.fetch_with(RouteFetchOptions {
                url: Some(format!("{base}/slow")),
                timeout: Some(Duration::ZERO),
                ..Default::default()
            }),
            async {
                tokio::time::sleep(Duration::from_millis(30)).await;
                if let Some(connection) = browser.cdp() {
                    connection.close();
                } else {
                    browser.bidi().unwrap().close();
                }
            }
        );
        assert!(
            matches!(result, Err(E2eError::Disconnected(_))),
            "{result:?}"
        );
        assert!(matches!(info.fetch().await, Err(E2eError::Disconnected(_))));
        assert!(matches!(
            info.fulfill_with(RouteFulfillOptions::default()).await,
            Err(E2eError::Disconnected(_))
        ));
        browser.close().await.unwrap();
    }
}
#[tokio::test]
async fn pinned_native_fulfillment_precedence_and_duplicate_cookies() {
    let (base, stop) = fixture().await;
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("fixture.txt");
    tokio::fs::write(&file, b"file bytes").await.unwrap();
    for browser in browsers(&base).await {
        let context = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        let page = context.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        for (name, options) in [
            ("response-inherit", RouteFulfillOptions::default()),
            (
                "response-status",
                RouteFulfillOptions {
                    status: Some(201),
                    ..Default::default()
                },
            ),
            (
                "response-body",
                RouteFulfillOptions {
                    body: Some(b"new body longer".to_vec()),
                    ..Default::default()
                },
            ),
            (
                "response-headers",
                RouteFulfillOptions {
                    headers: Some(vec![("x-only".into(), "replacement".into())]),
                    ..Default::default()
                },
            ),
            (
                "json-header-precedence",
                RouteFulfillOptions {
                    json: Some(json!({"changed":true})),
                    headers: Some(vec![("content-type".into(), "text/plain".into())]),
                    ..Default::default()
                },
            ),
            (
                "json-false",
                RouteFulfillOptions {
                    json: Some(json!(false)),
                    headers: Some(vec![("content-type".into(), "text/plain".into())]),
                    ..Default::default()
                },
            ),
            (
                "json-null",
                RouteFulfillOptions {
                    json: Some(Value::Null),
                    headers: Some(vec![("content-type".into(), "text/plain".into())]),
                    ..Default::default()
                },
            ),
            (
                "json-explicit-type",
                RouteFulfillOptions {
                    json: Some(json!({"changed":true})),
                    headers: Some(vec![("content-type".into(), "text/plain".into())]),
                    content_type: Some("application/x-fixture".into()),
                    ..Default::default()
                },
            ),
            (
                "file-header-precedence",
                RouteFulfillOptions {
                    path: Some(file.clone()),
                    headers: Some(vec![(
                        "content-type".into(),
                        "application/octet-stream".into(),
                    )]),
                    ..Default::default()
                },
            ),
            (
                "file-over-body",
                RouteFulfillOptions {
                    path: Some(file.clone()),
                    body: Some(b"ignored".to_vec()),
                    ..Default::default()
                },
            ),
            (
                "file-over-json",
                RouteFulfillOptions {
                    path: Some(file.clone()),
                    json: Some(json!({"ignored":true})),
                    ..Default::default()
                },
            ),
        ] {
            let client = context.request();
            let url = format!("{base}/binary");
            page.route_with_handler("**/target", move |info| {
                let client = client.clone();
                let url = url.clone();
                let mut options = options.clone();
                async move {
                    options.response = Some(client.get(&url).await?);
                    info.fulfill_with(options).await
                }
            })
            .await
            .unwrap();
            let result:Value=page.evaluate("fetch('/target').then(async r=>({status:r.status,type:r.headers.get('content-type'),length:r.headers.get('content-length'),source:r.headers.get('x-source'),only:r.headers.get('x-only'),bytes:Array.from(new Uint8Array(await r.arrayBuffer()))}))").await.unwrap();
            assert_eq!(
                result,
                reference(name)["result"],
                "{}: {name}",
                browser.kind().name()
            );
            page.unroute_all().await.unwrap();
        }
        page.route_with_handler("**/target", |info| async move {
            info.fulfill_with(RouteFulfillOptions {
                body: Some(b"cookies".to_vec()),
                headers: Some(vec![
                    ("set-cookie".into(), "one=1; Path=/".into()),
                    ("Set-Cookie".into(), "two=2; Path=/".into()),
                ]),
                ..Default::default()
            })
            .await
        })
        .await
        .unwrap();
        assert_eq!(
            page.evaluate::<String>("fetch('/target').then(r=>r.text())")
                .await
                .unwrap(),
            "cookies"
        );
        let cookies = context.cookies().await.unwrap();
        assert!(cookies.iter().any(|c| c.name == "one" && c.value == "1"));
        assert!(cookies.iter().any(|c| c.name == "two" && c.value == "2"));
        context.close().await.unwrap();
        browser.close().await.unwrap();
    }
    stop.abort();
}

struct AbortOnDrop(tokio::task::AbortHandle);
impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}
async fn reset_fixture(
    resets: usize,
) -> (String, Arc<std::sync::atomic::AtomicUsize>, AbortOnDrop) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = calls.clone();
    let task = tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = [0; 8192];
            let _ = stream.read(&mut bytes).await;
            if counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < resets {
                stream.shutdown().await.ok();
                continue;
            }
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\nfixture",
                )
                .await
                .ok();
            stream.shutdown().await.ok();
        }
    });
    (url, calls, AbortOnDrop(task.abort_handle()))
}
#[tokio::test]
async fn native_fetch_budgets_retries_caller_cancellation_page_and_context_disposal() {
    let (base, stop) = fixture().await;
    let _stop = AbortOnDrop(stop);
    for browser in browsers(&base).await {
        let context = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        let mut page = context.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        let retained = Arc::new(Mutex::new(None));
        let stored = retained.clone();
        page.route_with_handler("**/probe", move |info| {
            *stored.lock().unwrap() = Some(info.clone());
            async { Ok(RouteAction::fulfill(200, "probe", "text/plain")) }
        })
        .await
        .unwrap();
        assert_eq!(
            page.evaluate::<String>("fetch('/probe').then(r=>r.text())")
                .await
                .unwrap(),
            "probe"
        );
        let info = retained.lock().unwrap().take().unwrap();
        page.unroute_all().await.unwrap();
        page.set_timeout(Duration::from_millis(25));
        assert!(matches!(
            info.fetch_with(RouteFetchOptions {
                url: Some(format!("{base}/slow")),
                ..Default::default()
            })
            .await,
            Err(E2eError::Timeout(_, _))
        ));
        assert_eq!(
            info.fetch_with(RouteFetchOptions {
                url: Some(format!("{base}/slow")),
                timeout: Some(Duration::ZERO),
                ..Default::default()
            })
            .await
            .unwrap()
            .text(),
            "slow"
        );
        page.set_timeout(Duration::ZERO);
        assert_eq!(
            info.fetch_with(RouteFetchOptions {
                url: Some(format!("{base}/slow")),
                ..Default::default()
            })
            .await
            .unwrap()
            .text(),
            "slow"
        );
        let response = info
            .fetch_with(RouteFetchOptions {
                url: Some(format!("{base}/redirect")),
                max_redirects: Some(0),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            reference("fetch-no-redirect")["details"]["status"]
                .as_u64()
                .unwrap() as u16
        );
        assert_eq!(
            response.text(),
            reference("fetch-no-redirect")["details"]["body"]
        );
        assert!(context
            .cookies()
            .await
            .unwrap()
            .iter()
            .any(|c| c.name == "redirected"));
        let (url, calls, _reset) = reset_fixture(1).await;
        assert_eq!(
            info.fetch_with(RouteFetchOptions {
                url: Some(url),
                max_retries: 1,
                ..Default::default()
            })
            .await
            .unwrap()
            .text(),
            "fixture"
        );
        assert_eq!(
            calls.load(std::sync::atomic::Ordering::SeqCst) as u64,
            reference("fetch-reset-retry")["details"]["requests"]
                .as_u64()
                .unwrap()
        );
        let cancellation = CancellationToken::new();
        let cancel = cancellation.clone();
        let (result, ()) = tokio::join!(
            info.fetch_with(RouteFetchOptions {
                url: Some(format!("{base}/slow")),
                cancellation: Some(cancellation),
                ..Default::default()
            }),
            async {
                tokio::time::sleep(Duration::from_millis(30)).await;
                cancel.cancel();
            }
        );
        assert!(matches!(result, Err(E2eError::Cancelled(_))));
        let (result, ()) = tokio::join!(
            info.fetch_with(RouteFetchOptions {
                url: Some(format!("{base}/slow")),
                ..Default::default()
            }),
            async {
                tokio::time::sleep(Duration::from_millis(30)).await;
                page.close().await.unwrap();
            }
        );
        assert!(matches!(result, Err(E2eError::Cancelled(_))));
        assert!(matches!(
            info.fulfill_with(RouteFulfillOptions::default()).await,
            Err(E2eError::Cancelled(_))
        ));
        context.close().await.unwrap();
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn native_fetch_runner_retries_release_callback_state_and_context_lifetimes() {
    let (base, stop) = fixture().await;
    let _stop = AbortOnDrop(stop);
    for browser in browsers(&base).await {
        let witnesses = Arc::new(Mutex::new(Vec::new()));
        let observed = witnesses.clone();
        let output = tempfile::tempdir().unwrap();
        let report = Runner::default()
            .test_timeout(Duration::from_millis(850))
            .retries(1)
            .output_dir(output.path().display().to_string())
            .run(
                &browser,
                vec![test_with_context(
                    "route fetch enclosing retry",
                    move |ctx| {
                        let observed = observed.clone();
                        async move {
                            ctx.page.goto("/").await?;
                            ctx.page
                                .route_with_handler("**/probe", move |info| {
                                    let observed = observed.clone();
                                    async move {
                                        let (url, calls, _stop) = reset_fixture(usize::MAX).await;
                                        observed.lock().unwrap().push(Arc::downgrade(&calls));
                                        let response = info
                                            .fetch_with(RouteFetchOptions {
                                                url: Some(url),
                                                timeout: Some(Duration::ZERO),
                                                max_retries: 100,
                                                ..Default::default()
                                            })
                                            .await?;
                                        info.fulfill_with(RouteFulfillOptions {
                                            response: Some(response),
                                            ..Default::default()
                                        })
                                        .await
                                    }
                                })
                                .await?;
                            ctx.page
                                .evaluate_value("fetch('/probe').then(r=>r.text())")
                                .await
                                .map(|_| ())
                        }
                    },
                )],
            )
            .await;
        assert_eq!(report.failed(), 1);
        assert_eq!(report.results[0].attempts, 2);
        assert_eq!(
            witnesses.lock().unwrap().len(),
            2,
            "both retries reached the route fetch"
        );
        tokio::time::timeout(Duration::from_secs(1), async {
            while witnesses
                .lock()
                .unwrap()
                .iter()
                .any(|weak| weak.upgrade().is_some())
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("canceled callback futures release their listeners and state");
        let context = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        let page = context.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        let retained = Arc::new(Mutex::new(None));
        let stored = retained.clone();
        page.route_with_handler("**/probe", move |info| {
            *stored.lock().unwrap() = Some(info);
            async { Ok(RouteAction::fulfill(200, "probe", "text/plain")) }
        })
        .await
        .unwrap();
        page.evaluate_value("fetch('/probe').then(r=>r.text())")
            .await
            .unwrap();
        let info = retained.lock().unwrap().take().unwrap();
        let (result, ()) = tokio::join!(
            info.fetch_with(RouteFetchOptions {
                url: Some(format!("{base}/slow")),
                timeout: Some(Duration::ZERO),
                ..Default::default()
            }),
            async {
                tokio::time::sleep(Duration::from_millis(30)).await;
                context.clone().close().await.unwrap();
            }
        );
        assert!(matches!(result, Err(E2eError::Cancelled(_))));
        assert!(matches!(info.fetch().await, Err(E2eError::Cancelled(_))));
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn native_fetch_inherits_tls_proxy_and_supported_context_credentials() {
    let mut tls = tls::tls_fixture().await;
    let (base, stop) = fixture().await;
    let _stop = AbortOnDrop(stop);
    for ignore_https_errors in [false, true] {
        for browser in browsers_with_tls(&base, ignore_https_errors).await {
            let context = browser
                .new_context(ContextOptions {
                    ignore_https_errors,
                    ..Default::default()
                })
                .await
                .unwrap();
            let page = context.new_page().await.unwrap();
            page.goto("/").await.unwrap();
            let url = tls.url.clone();
            page.route_with_handler("**/probe", move |info| {
                let url = url.clone();
                async move {
                    match info
                        .fetch_with(RouteFetchOptions {
                            url: Some(url),
                            ..Default::default()
                        })
                        .await
                    {
                        Ok(response) => {
                            assert!(ignore_https_errors);
                            assert_eq!(response.status(), 200);
                            info.fulfill_with(RouteFulfillOptions {
                                response: Some(response),
                                ..Default::default()
                            })
                            .await
                        }
                        Err(E2eError::Http(_)) if !ignore_https_errors => {
                            info.fulfill_with(RouteFulfillOptions {
                                body: Some(b"tls rejected".to_vec()),
                                ..Default::default()
                            })
                            .await
                        }
                        other => panic!("unexpected route TLS result: {other:?}"),
                    }
                }
            })
            .await
            .unwrap();
            let body: String = page
                .evaluate("fetch('/probe').then(r=>r.text())")
                .await
                .unwrap();
            if ignore_https_errors {
                assert!(body.to_lowercase().contains("<html>"));
            } else {
                assert_eq!(body, "tls rejected");
            }
            context.close().await.unwrap();
            if browser.kind() == BrowserKind::Chromium && !ignore_https_errors {
                let context = browser
                    .new_context(ContextOptions {
                        http_credentials: Some(HttpCredentials::new("user", "pass")),
                        ..Default::default()
                    })
                    .await
                    .unwrap();
                let page = context.new_page().await.unwrap();
                page.goto("/").await.unwrap();
                let echo = format!("{base}/echo");
                page.route_with_handler("**/probe", move |info| {
                    let echo = echo.clone();
                    async move {
                        let response = info
                            .fetch_with(RouteFetchOptions {
                                url: Some(echo),
                                headers: Some(vec![]),
                                ..Default::default()
                            })
                            .await?;
                        let value: Value = response.json()?;
                        assert_eq!(value["auth"], "Basic dXNlcjpwYXNz");
                        info.fulfill_with(RouteFulfillOptions {
                            response: Some(response),
                            ..Default::default()
                        })
                        .await
                    }
                })
                .await
                .unwrap();
                let _: Value = page
                    .evaluate("fetch('/probe').then(r=>r.json())")
                    .await
                    .unwrap();
                context.close().await.unwrap();
            }
            browser.close().await.unwrap();
        }
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy = format!("http://{}", listener.local_addr().unwrap());
    let app = axum::Router::new().fallback(axum::routing::any(
        |request: axum::extract::Request| async move {
            axum::Json(json!({"proxied":true,"target":request.uri().to_string()}))
        },
    ));
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let _stop_proxy = AbortOnDrop(task.abort_handle());
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let executable = match kind {
            BrowserKind::Chromium => find_chromium(None),
            BrowserKind::Firefox => find_firefox(None),
        };
        let Some(executable) = executable else {
            assert!(std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_none());
            continue;
        };
        let mut options = LaunchOptions::default()
            .browser(kind)
            .executable(executable);
        options.proxy_server = Some(proxy.clone());
        let mut browser = Browser::launch(options).await.unwrap();
        browser.set_base_url(Some(base.clone()));
        let context = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        let page = context.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        let target = format!("{base}/echo");
        let expected = target.clone();
        page.route_with_handler("**/probe", move |info| {
            let target = target.clone();
            async move {
                let response = info
                    .fetch_with(RouteFetchOptions {
                        url: Some(target),
                        ..Default::default()
                    })
                    .await?;
                info.fulfill_with(RouteFulfillOptions {
                    response: Some(response),
                    ..Default::default()
                })
                .await
            }
        })
        .await
        .unwrap();
        let value: Value = page
            .evaluate("fetch('/probe').then(r=>r.json())")
            .await
            .unwrap();
        assert_eq!(
            value,
            json!({"proxied":true,"target":expected}),
            "{} context proxy",
            kind.name()
        );
        context.close().await.unwrap();
        browser.close().await.unwrap();
    }
    tls.child.kill().await.unwrap();
}
