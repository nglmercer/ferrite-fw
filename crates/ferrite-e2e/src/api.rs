//! Standalone HTTP API client (no browser needed).

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::error::{E2eError, E2eResult};

/// HTTP response from [`ApiClient`] (4xx/5xx do not throw; check [`ApiResponse::status`]).
#[derive(Debug, Clone)]
pub struct ApiResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl ApiResponse {
    /// HTTP status code.
    #[must_use]
    pub fn status(&self) -> u16 {
        self.status
    }

    /// True for 2xx statuses.
    #[must_use]
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// Response headers.
    #[must_use]
    pub fn headers(&self) -> &[(String, String)] {
        &self.headers
    }

    /// First response header with `name` (case-insensitive).
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// Raw body bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.body
    }

    /// Body as lossy UTF-8 text.
    #[must_use]
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// Body parsed as JSON.
    pub fn json<T: DeserializeOwned>(&self) -> E2eResult<T> {
        Ok(serde_json::from_slice(&self.body)?)
    }
}

/// Plain HTTP client with an optional base URL and default headers.
#[derive(Debug, Clone)]
pub struct ApiClient {
    client: reqwest::Client,
    base_url: Option<String>,
    headers: Vec<(String, String)>,
}

impl ApiClient {
    /// Client without a base URL (absolute URLs only).
    #[must_use]
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: None,
            headers: Vec::new(),
        }
    }

    /// Client resolving relative paths against `base`.
    #[must_use]
    pub fn with_base_url(base: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: Some(base.into()),
            headers: Vec::new(),
        }
    }

    /// Add a default header sent with every request.
    #[must_use]
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// GET `path` (absolute URL or relative to the base URL).
    pub async fn get(&self, path: &str) -> E2eResult<ApiResponse> {
        let url = self.url(path)?;
        let request = self.client.get(&url);
        self.send(request).await
    }

    /// DELETE `path`.
    pub async fn delete(&self, path: &str) -> E2eResult<ApiResponse> {
        let url = self.url(path)?;
        let request = self.client.delete(&url);
        self.send(request).await
    }

    /// POST `body` as JSON.
    pub async fn post_json(&self, path: &str, body: &Value) -> E2eResult<ApiResponse> {
        let url = self.url(path)?;
        let request = self.client.post(&url).json(body);
        self.send(request).await
    }

    /// PUT `body` as JSON.
    pub async fn put_json(&self, path: &str, body: &Value) -> E2eResult<ApiResponse> {
        let url = self.url(path)?;
        let request = self.client.put(&url).json(body);
        self.send(request).await
    }

    /// PATCH `body` as JSON.
    pub async fn patch_json(&self, path: &str, body: &Value) -> E2eResult<ApiResponse> {
        let url = self.url(path)?;
        let request = self.client.patch(&url).json(body);
        self.send(request).await
    }

    /// HEAD `path` (headers only, no body).
    pub async fn head(&self, path: &str) -> E2eResult<ApiResponse> {
        let url = self.url(path)?;
        let request = self.client.head(&url);
        self.send(request).await
    }

    /// POST form fields (`application/x-www-form-urlencoded`).
    pub async fn post_form(&self, path: &str, fields: &[(&str, &str)]) -> E2eResult<ApiResponse> {
        let url = self.url(path)?;
        let request = self.client.post(&url).form(fields);
        self.send(request).await
    }

    /// GET `path` with URL-encoded query pairs.
    pub async fn get_with_query(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> E2eResult<ApiResponse> {
        let url = self.url(path)?;
        let request = self.client.get(&url).query(query);
        self.send(request).await
    }

    /// POST `body` as JSON with URL-encoded query pairs.
    pub async fn post_json_with_query(
        &self,
        path: &str,
        body: &Value,
        query: &[(&str, &str)],
    ) -> E2eResult<ApiResponse> {
        let url = self.url(path)?;
        let request = self.client.post(&url).query(query).json(body);
        self.send(request).await
    }

    /// Resolve `path` against the base URL (absolute URLs pass through).
    fn url(&self, path: &str) -> E2eResult<String> {
        if path.starts_with("http://") || path.starts_with("https://") {
            return Ok(path.to_string());
        }
        match &self.base_url {
            Some(base) => Ok(format!(
                "{}/{}",
                base.trim_end_matches('/'),
                path.trim_start_matches('/')
            )),
            None => Err(E2eError::Config(format!(
                "relative API path without base_url: {path}"
            ))),
        }
    }

    /// Send a request with default headers; transport errors are `Http`.
    async fn send(&self, request: reqwest::RequestBuilder) -> E2eResult<ApiResponse> {
        let mut request = request;
        for (name, value) in &self.headers {
            request = request.header(name, value);
        }
        let response = request.send().await?;
        let status = response.status().as_u16();
        let mut headers = Vec::new();
        for (name, value) in response.headers() {
            let text = value
                .to_str()
                .map(str::to_string)
                .unwrap_or_else(|_| String::from_utf8_lossy(value.as_bytes()).into_owned());
            headers.push((name.to_string(), text));
        }
        let body = response.bytes().await?.to_vec();
        Ok(ApiResponse {
            status,
            headers,
            body,
        })
    }
}

impl Default for ApiClient {
    fn default() -> Self {
        Self::new()
    }
}
