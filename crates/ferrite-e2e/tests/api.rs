//! API client tests (no browser needed).

use ferrite_e2e::{ApiClient, E2eError};

async fn serve() -> (String, tokio::task::AbortHandle) {
    let app = axum::Router::new()
        .route(
            "/json",
            axum::routing::get(|| async { axum::Json(serde_json::json!({"hello": "api"})) }),
        )
        .route(
            "/echo",
            axum::routing::post(|body: axum::body::Bytes| async move { body })
                .put(|body: axum::body::Bytes| async move { body }),
        )
        .route(
            "/headers",
            axum::routing::get(|headers: axum::http::HeaderMap| async move {
                headers
                    .get("x-api-key")
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("absent")
                    .to_string()
            }),
        )
        .route(
            "/gone",
            axum::routing::delete(axum::http::StatusCode::NO_CONTENT),
        )
        .route(
            "/patch",
            axum::routing::patch(|body: axum::body::Bytes| async move { body }),
        )
        .route(
            "/form",
            axum::routing::post(
                |axum::Form(fields): axum::Form<Vec<(String, String)>>| async move {
                    axum::Json(fields)
                },
            ),
        )
        .route(
            "/search",
            axum::routing::get(
                |axum::extract::Query(params): axum::extract::Query<
                    std::collections::HashMap<String, String>,
                >| async move { params.get("q").cloned().unwrap_or_default() },
            ),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, app).await });
    (url, task.abort_handle())
}

#[tokio::test]
async fn api_client_round_trip() {
    let (base, shutdown) = serve().await;
    let client = ApiClient::with_base_url(&base);

    let get = client.get("json").await.unwrap();
    assert!(get.ok());
    assert_eq!(get.status(), 200);
    assert_eq!(
        get.json::<serde_json::Value>().unwrap(),
        serde_json::json!({"hello": "api"})
    );
    assert!(get.header("content-type").unwrap().contains("json"));
    assert!(get.text().contains("api"));

    let post = client
        .post_json("/echo", &serde_json::json!({"n": 1}))
        .await
        .unwrap();
    assert_eq!(
        post.json::<serde_json::Value>().unwrap(),
        serde_json::json!({"n": 1})
    );

    let put = client
        .put_json("echo", &serde_json::json!([1, 2]))
        .await
        .unwrap();
    assert_eq!(
        put.json::<serde_json::Value>().unwrap(),
        serde_json::json!([1, 2])
    );

    let delete = client.delete("gone").await.unwrap();
    assert_eq!(delete.status(), 204);
    assert!(delete.ok());

    // Errors do not throw; default headers are sent.
    let missing = client.get("nope").await.unwrap();
    assert_eq!(missing.status(), 404);
    assert!(!missing.ok());
    let keyed = ApiClient::with_base_url(&base).with_header("x-api-key", "secret");
    assert_eq!(keyed.get("headers").await.unwrap().text(), "secret");

    // Absolute URLs bypass the base; relative paths need one.
    let direct = ApiClient::new().get(&format!("{base}json")).await.unwrap();
    assert!(direct.ok());
    let err = ApiClient::new().get("json").await.unwrap_err();
    assert!(matches!(err, E2eError::Config(_)), "{err}");

    // PATCH, HEAD, forms, and query pairs.
    let patch = client
        .patch_json("patch", &serde_json::json!({"p": true}))
        .await
        .unwrap();
    assert_eq!(
        patch.json::<serde_json::Value>().unwrap(),
        serde_json::json!({"p": true})
    );
    let head = client.head("json").await.unwrap();
    assert_eq!(head.status(), 200);
    assert!(head.bytes().is_empty());
    assert!(head.header("content-type").unwrap().contains("json"));
    let form = client
        .post_form("form", &[("user", "ada"), ("role", "dev")])
        .await
        .unwrap();
    assert_eq!(
        form.json::<serde_json::Value>().unwrap(),
        serde_json::json!([["user", "ada"], ["role", "dev"]])
    );
    let search = client
        .get_with_query("search", &[("q", "hello world")])
        .await
        .unwrap();
    assert_eq!(search.text(), "hello world");
    let post_query = client
        .post_json_with_query("echo", &serde_json::json!({"n": 2}), &[("q", "x")])
        .await
        .unwrap();
    assert_eq!(
        post_query.json::<serde_json::Value>().unwrap(),
        serde_json::json!({"n": 2})
    );

    // fetch() covers arbitrary verbs; post_bytes sets the content type.
    let fetched = client.fetch("GET", "json").await.unwrap();
    assert!(fetched.ok());
    assert_eq!(
        fetched.json::<serde_json::Value>().unwrap(),
        serde_json::json!({"hello": "api"})
    );
    let raw = client
        .post_bytes("echo", b"bytes-here", "application/octet-stream")
        .await
        .unwrap();
    assert_eq!(raw.bytes(), b"bytes-here");

    shutdown.abort();
}

#[tokio::test]
async fn download_save_and_delete() {
    use ferrite_e2e::Download;

    let dir = std::env::temp_dir().join(format!("ferrite-dl-unit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("orig.bin");
    std::fs::write(&src, [1u8, 2, 3]).unwrap();

    let download = Download::from_path(src.clone());
    assert_eq!(download.suggested_filename, "orig.bin");
    let saved = download
        .save_as(dir.join("nested").join("copy.bin"))
        .await
        .unwrap();
    assert_eq!(std::fs::read(&saved).unwrap(), vec![1u8, 2, 3]);
    download.delete().await.unwrap();
    assert!(!src.exists());
    // Deleting twice stays quiet.
    download.delete().await.unwrap();

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn api_options_cookies_redirects_and_payloads() {
    use ferrite_e2e::{ApiClientOptions, ApiRequestOptions, MultipartField};
    use std::time::Duration;
    let app = axum::Router::new()
        .route("/cookie", axum::routing::get(|| async { ([("set-cookie", "token=api; Path=/; HttpOnly")], "set") }))
        .route("/inspect", axum::routing::get(|headers: axum::http::HeaderMap| async move {
            axum::Json(serde_json::json!({"cookie":headers.get("cookie").and_then(|v| v.to_str().ok()), "key":headers.get("x-api-key").and_then(|v| v.to_str().ok())}))
        }))
        .route("/slow", axum::routing::get(|| async { tokio::time::sleep(Duration::from_millis(150)).await; "slow" }))
        .route("/redirect", axum::routing::get(|| async { axum::response::Redirect::temporary("/cookie") }))
        .route("/echo", axum::routing::post(|headers: axum::http::HeaderMap, body: axum::body::Bytes| async move { ([("content-type", headers.get("content-type").unwrap().to_str().unwrap().to_string())], body) }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = ApiClient::with_options(ApiClientOptions {
        base_url: Some(format!("{base}/nested/path")),
        headers: vec![("x-api-key".into(), "default".into())],
        ..Default::default()
    })
    .unwrap();
    let response = client.get("/redirect").await.unwrap();
    assert_eq!(response.url(), format!("{base}/cookie"));
    assert_eq!(response.status_text(), "OK");
    let response = client
        .fetch_with(
            "GET",
            "/inspect",
            ApiRequestOptions {
                headers: vec![("x-api-key".into(), "override".into())],
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let json: serde_json::Value = response.json().unwrap();
    assert_eq!(json["cookie"], "token=api");
    assert_eq!(json["key"], "override");
    assert_eq!(
        client.cookie_header("/inspect").unwrap().as_deref(),
        Some("token=api")
    );
    let no_redirect = ApiClient::with_options(ApiClientOptions {
        max_redirects: 0,
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        no_redirect
            .get(&format!("{base}/redirect"))
            .await
            .unwrap()
            .status(),
        307
    );
    let error = client
        .fetch_with(
            "GET",
            "/missing",
            ApiRequestOptions {
                fail_on_status_code: true,
                ..Default::default()
            },
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("404"));
    assert!(client
        .fetch_with(
            "GET",
            "/slow",
            ApiRequestOptions {
                timeout: Some(Duration::from_millis(10)),
                ..Default::default()
            }
        )
        .await
        .is_err());
    let response = client
        .fetch_with(
            "POST",
            "/echo",
            ApiRequestOptions {
                multipart: Some(vec![MultipartField {
                    name: "file".into(),
                    bytes: b"upload contents".to_vec(),
                    filename: Some("test.txt".into()),
                    content_type: Some("text/plain".into()),
                }]),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(response
        .header("content-type")
        .unwrap()
        .contains("multipart/form-data"));
    assert!(response.text().contains("filename=\"test.txt\""));
    assert!(response.text().contains("upload contents"));
    assert!(client
        .fetch_with(
            "POST",
            "/echo",
            ApiRequestOptions {
                body: Some(vec![]),
                json: Some(serde_json::json!({})),
                ..Default::default()
            }
        )
        .await
        .is_err());
    server.abort();
}
