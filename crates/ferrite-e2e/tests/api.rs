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

    shutdown.abort();
}
