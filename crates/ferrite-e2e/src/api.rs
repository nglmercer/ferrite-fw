//! Standalone HTTP API client (no browser needed).

use crate::{BrowserContext, HttpCredentials};
use reqwest::cookie::CookieStore;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::error::{E2eError, E2eResult};

/// HTTP response from [`ApiClient`] (4xx/5xx do not throw; check [`ApiResponse::status`]).
#[derive(Debug, Clone)]
pub struct ApiResponse {
    url: String,
    status_text: String,
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl ApiResponse {
    /// Final URL after redirects.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Canonical HTTP status text.
    pub fn status_text(&self) -> &str {
        &self.status_text
    }

    /// Release the buffered response body.
    pub fn dispose(&mut self) {
        self.body.clear();
        self.body.shrink_to_fit();
    }

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

    /// Native header entries, including duplicate names, with JSON serialization.
    pub fn headers_array(&self) -> Vec<crate::HttpHeader> {
        crate::network::header_array(&self.headers)
    }
    /// All values for a case-insensitive name, preserving duplicate entries.
    pub fn header_values(&self, name: &str) -> Vec<String> {
        crate::network::header_values(&self.headers, name)
    }
    /// Joined values (Set-Cookie uses newline; other names use comma-space).
    pub fn header_value(&self, name: &str) -> Option<String> {
        crate::network::header_value(&self.headers, name)
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

/// When an API client sends configured Basic credentials.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ApiCredentialsSend {
    /// Preemptive Basic authentication (Ferrite's existing default).
    #[default]
    Always,
    /// Send only after a 401 Basic challenge, matching Playwright's default.
    Unauthorized,
}

/// Transport configuration for a standalone or context-linked API client.
#[derive(Clone)]
pub struct ApiClientOptions {
    pub base_url: Option<String>,
    pub headers: Vec<(String, String)>,
    pub timeout: Duration,
    pub max_redirects: usize,
    pub ignore_https_errors: bool,
    pub proxy: Option<String>,
    pub credentials: Option<HttpCredentials>,
    /// Restrict configured credentials to this URL origin (scheme/host/port).
    pub credential_origin: Option<String>,
    pub credential_send: ApiCredentialsSend,
    pub storage_state: Option<crate::StorageState>,
}

impl Default for ApiClientOptions {
    fn default() -> Self {
        Self {
            base_url: None,
            headers: Vec::new(),
            timeout: Duration::from_secs(30),
            max_redirects: 20,
            ignore_https_errors: false,
            proxy: None,
            credentials: None,
            credential_origin: None,
            credential_send: ApiCredentialsSend::Always,
            storage_state: None,
        }
    }
}

/// Per-request options. Body, JSON, form and multipart payloads are mutually exclusive.
#[derive(Debug, Clone, Default)]
pub struct ApiRequestOptions {
    pub headers: Vec<(String, String)>,
    pub query: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    pub json: Option<Value>,
    pub form: Option<Vec<(String, String)>>,
    pub multipart: Option<Vec<MultipartField>>,
    pub timeout: Option<Duration>,
    pub fail_on_status_code: bool,
    /// Retry peer resets before response headers; refused connections, body
    /// failures and HTTP error statuses are never retried.
    pub max_retries: u32,
    /// None inherits the client limit; zero returns redirects without following.
    pub max_redirects: Option<usize>,
    pub cancellation: Option<crate::CancellationToken>,
}

/// In-memory multipart field or file.
#[derive(Debug, Clone)]
pub struct MultipartField {
    pub name: String,
    pub bytes: Vec<u8>,
    pub filename: Option<String>,
    pub content_type: Option<String>,
}

/// Plain HTTP client with an optional base URL and default headers.
#[derive(Clone)]
pub struct ApiClient {
    client: reqwest::Client,
    base_url: Option<String>,
    headers: Vec<(String, String)>,
    jar: Arc<crate::api_cookies::ApiCookieJar>,
    timeout: Duration,
    lifecycle: crate::CancellationToken,
    cancellation: crate::CancellationToken,
    origins: Arc<Mutex<Vec<crate::StorageOrigin>>>,
    linked_request: Arc<tokio::sync::Mutex<()>>,
    context: Option<BrowserContext>,
    credentials: Option<HttpCredentials>,
    credential_origin: Option<reqwest::Url>,
    credential_send: ApiCredentialsSend,
    max_redirects: usize,
}

impl ApiClient {
    pub fn with_cancellation(&self, cancellation: crate::CancellationToken) -> Self {
        let mut client = self.clone();
        client.cancellation = cancellation;
        client
    }
    async fn run<T>(
        &self,
        future: impl std::future::Future<Output = E2eResult<T>>,
    ) -> E2eResult<T> {
        self.lifecycle
            .run(self.cancellation.run(async {
                if let Some(context) = &self.context {
                    context.cancellation_token().run(future).await
                } else {
                    future.await
                }
            }))
            .await
    }
    /// Invalidate all clones and interrupt in-flight requests. Response buffers
    /// already returned remain owned by their individual responses.
    pub fn dispose(&self) {
        self.lifecycle.cancel_with_reason("API client disposed");
        self.jar.clear();
    }
    pub async fn storage_state(&self) -> E2eResult<crate::StorageState> {
        self.run(async {
            if let Some(context) = &self.context {
                return context.storage_state().await;
            }
            Ok(crate::StorageState {
                origin: String::new(),
                local_storage: Default::default(),
                cookies: self.jar.snapshot(),
                origins: self
                    .origins
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone(),
            })
        })
        .await
    }
    pub async fn save_storage_state(&self, path: impl AsRef<std::path::Path>) -> E2eResult<()> {
        tokio::fs::write(
            path,
            serde_json::to_vec_pretty(&self.storage_state().await?)?,
        )
        .await?;
        Ok(())
    }
    pub async fn apply_storage_state(&self, state: &crate::StorageState) -> E2eResult<()> {
        self.run(async {
            if let Some(context) = &self.context {
                context.apply_storage_state(state).await?;
            }
            self.jar.restore(&state.cookies, self.base_url.as_deref())?;
            *self.origins.lock().unwrap_or_else(|e| e.into_inner()) = state.all_origins();
            Ok(())
        })
        .await
    }
    pub async fn load_storage_state(&self, path: impl AsRef<std::path::Path>) -> E2eResult<()> {
        self.apply_storage_state(&serde_json::from_slice(&tokio::fs::read(path).await?)?)
            .await
    }
    /// Client without a base URL (absolute URLs only).
    pub fn new() -> Self {
        Self::with_options(ApiClientOptions::default()).expect("default HTTP client configuration")
    }

    /// Client resolving relative paths with standard URL semantics.
    pub fn with_base_url(base: impl Into<String>) -> Self {
        Self::with_options(ApiClientOptions {
            base_url: Some(base.into()),
            ..ApiClientOptions::default()
        })
        .expect("default HTTP client configuration")
    }

    /// Configure cookies, proxy, TLS, authentication, redirects and timeouts.
    pub fn with_options(options: ApiClientOptions) -> E2eResult<Self> {
        let jar = Arc::new(crate::api_cookies::ApiCookieJar::default());
        if let Some(state) = &options.storage_state {
            jar.restore(&state.cookies, options.base_url.as_deref())?;
        }
        let credential_origin = options
            .credential_origin
            .as_ref()
            .map(|origin| {
                let url = reqwest::Url::parse(origin)
                    .map_err(|e| E2eError::Config(format!("invalid API credential origin: {e}")))?;
                if !matches!(url.scheme(), "http" | "https")
                    || url.host_str().is_none()
                    || url.path() != "/"
                    || url.query().is_some()
                    || url.fragment().is_some()
                    || !url.username().is_empty()
                    || url.password().is_some()
                {
                    return Err(E2eError::Config(
                        "API credential origin must contain only HTTP(S) scheme, host and port"
                            .into(),
                    ));
                }
                Ok(url)
            })
            .transpose()?;
        let mut builder = reqwest::Client::builder()
            .cookie_provider(jar.clone())
            .danger_accept_invalid_certs(options.ignore_https_errors)
            .redirect(reqwest::redirect::Policy::none());
        if let Some(proxy) = &options.proxy {
            builder = builder.proxy(reqwest::Proxy::all(proxy)?);
        }
        Ok(Self {
            client: builder.build()?,
            base_url: options.base_url,
            headers: options.headers,
            jar,
            timeout: options.timeout,
            lifecycle: crate::CancellationToken::new(),
            cancellation: crate::CancellationToken::new(),
            origins: Arc::new(Mutex::new(
                options
                    .storage_state
                    .as_ref()
                    .map(|s| s.all_origins())
                    .unwrap_or_default(),
            )),
            linked_request: Arc::new(tokio::sync::Mutex::new(())),
            context: None,
            credentials: options.credentials,
            credential_origin,
            credential_send: options.credential_send,
            max_redirects: options.max_redirects,
        })
    }

    /// Share browser-context cookies in both directions.
    pub fn with_context(mut self, context: BrowserContext) -> Self {
        self.context = Some(context);
        self
    }

    /// Cookie header for a URL in the standalone client's jar.
    pub fn cookie_header(&self, url: &str) -> E2eResult<Option<String>> {
        let url =
            reqwest::Url::parse(&self.url(url)?).map_err(|e| E2eError::Config(e.to_string()))?;
        Ok(self
            .jar
            .cookies(&url)
            .and_then(|v| v.to_str().ok().map(str::to_string)))
    }

    /// Generic request with per-request payloads, timeout and transport retries.
    pub async fn fetch_with(
        &self,
        method: &str,
        path: &str,
        options: ApiRequestOptions,
    ) -> E2eResult<ApiResponse> {
        let client = options
            .cancellation
            .as_ref()
            .map(|token| self.with_cancellation(token.clone()))
            .unwrap_or_else(|| self.clone());
        // Retries consume the same total budget as headers, cookies and body reads.
        client
            .run(
                crate::operation::Deadline::new(options.timeout.unwrap_or(self.timeout)).run(
                    "API fetch",
                    async {
                        let bodies = [
                            options.body.is_some(),
                            options.json.is_some(),
                            options.form.is_some(),
                            options.multipart.is_some(),
                        ]
                        .into_iter()
                        .filter(|v| *v)
                        .count();
                        if bodies > 1 {
                            return Err(E2eError::Config(
                                "API payload options are mutually exclusive".into(),
                            ));
                        }
                        let method = method
                            .to_ascii_uppercase()
                            .parse::<reqwest::Method>()
                            .map_err(|e| E2eError::Config(e.to_string()))?;
                        let mut request = self
                            .client
                            .request(method.clone(), self.url(path)?)
                            .query(&options.query);
                        // Defaults must participate before payload content-type inference.
                        let mut headers = options.headers.clone();
                        for (name, value) in &self.headers {
                            if !headers
                                .iter()
                                .any(|(key, _)| key.eq_ignore_ascii_case(name))
                            {
                                headers.push((name.clone(), value.clone()));
                            }
                        }
                        for (name, value) in &headers {
                            request = request.header(name, value);
                        }
                        if let Some(timeout) = options.timeout {
                            request = request.timeout(timeout);
                        }
                        if let Some(body) = &options.body {
                            request = request.body(body.clone());
                            if !headers
                                .iter()
                                .any(|(name, _)| name.eq_ignore_ascii_case("content-type"))
                            {
                                request = request.header(
                                    reqwest::header::CONTENT_TYPE,
                                    "application/octet-stream",
                                );
                            }
                        }
                        if let Some(json) = &options.json {
                            request = request.json(json);
                        }
                        if let Some(form) = &options.form {
                            request = request.form(form);
                        }
                        if let Some(fields) = &options.multipart {
                            let mut form = reqwest::multipart::Form::new();
                            for field in fields {
                                let mut part = reqwest::multipart::Part::bytes(field.bytes.clone());
                                if let Some(filename) = &field.filename {
                                    part = part.file_name(filename.clone());
                                }
                                if let Some(content_type) = &field.content_type {
                                    part = part.mime_str(content_type)?;
                                }
                                form = form.part(field.name.clone(), part);
                            }
                            request = request.multipart(form);
                        }
                        client
                            .send_with(
                                request,
                                options.max_redirects.unwrap_or(self.max_redirects),
                                options.max_retries,
                                options.fail_on_status_code,
                            )
                            .await
                    },
                ),
            )
            .await
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

    /// Send `method` to `url` with no extra headers or body
    /// (Playwright `request.fetch()` equivalent).
    pub async fn fetch(&self, method: &str, url: &str) -> E2eResult<ApiResponse> {
        self.request(method, url, &[], None).await
    }

    /// POST raw bytes with an explicit content type.
    pub async fn post_bytes(
        &self,
        path: &str,
        body: &[u8],
        content_type: &str,
    ) -> E2eResult<ApiResponse> {
        self.request(
            "POST",
            path,
            &[("content-type".to_string(), content_type.to_string())],
            Some(body),
        )
        .await
    }

    /// Send any method to `url` with extra headers and an optional body
    /// (full verb coverage: `OPTIONS`, `TRACE`, ...).
    pub async fn request(
        &self,
        method: &str,
        url: &str,
        headers: &[(String, String)],
        body: Option<&[u8]>,
    ) -> E2eResult<ApiResponse> {
        let url = self.url(url)?;
        let method: reqwest::Method = method
            .to_ascii_uppercase()
            .parse()
            .map_err(|error| E2eError::Config(format!("bad API method: {error}")))?;
        let mut request = self.client.request(method, &url);
        for (name, value) in headers {
            request = request.header(name, value);
        }
        if let Some(body) = body {
            request = request.body(body.to_vec());
        }
        self.send(request).await
    }

    /// Resolve `path` against the base URL (absolute URLs pass through).
    pub(crate) fn url(&self, path: &str) -> E2eResult<String> {
        if path.starts_with("http://") || path.starts_with("https://") {
            return Ok(path.to_string());
        }
        match &self.base_url {
            Some(base) => reqwest::Url::parse(base)
                .and_then(|base| base.join(path))
                .map(|url| url.to_string())
                .map_err(|e| E2eError::Config(e.to_string())),
            None => Err(E2eError::Config(format!(
                "relative API path without base_url: {path}"
            ))),
        }
    }

    /// Send through the shared manual redirect/authentication transport.
    async fn send(&self, request: reqwest::RequestBuilder) -> E2eResult<ApiResponse> {
        self.send_with(request, self.max_redirects, 0, false).await
    }
    async fn send_with(
        &self,
        request: reqwest::RequestBuilder,
        max_redirects: usize,
        max_retries: u32,
        fail_on_status: bool,
    ) -> E2eResult<ApiResponse> {
        let mut request = request.build()?;
        let timeout = request.timeout_mut().take().unwrap_or(self.timeout);
        self.run(
            crate::operation::Deadline::new(timeout).run("API request", async {
                for (name, value) in &self.headers {
                    let name = reqwest::header::HeaderName::from_bytes(name.as_bytes())
                        .map_err(|e| E2eError::Config(e.to_string()))?;
                    if !request.headers().contains_key(&name) {
                        request.headers_mut().insert(
                            name,
                            value.parse().map_err(|e| {
                                E2eError::Config(format!("invalid API header: {e}"))
                            })?,
                        );
                    }
                }
                if self.credential_send == ApiCredentialsSend::Always {
                    self.authorize(&mut request)?;
                }
                // API payloads originate in owned memory. Materialize reqwest's
                // multipart stream once so 307/308, auth and retries replay all bytes.
                if let Some(body) = request.body_mut().take() {
                    use http_body_util::BodyExt;
                    let body = body.collect().await?.to_bytes();
                    request.headers_mut().insert(
                        reqwest::header::CONTENT_LENGTH,
                        body.len().to_string().parse().expect("valid byte length"),
                    );
                    *request.body_mut() = Some(body.into());
                }
                let _linked = if self.context.is_some() {
                    Some(self.linked_request.lock().await)
                } else {
                    None
                };
                if let Some(context) = &self.context {
                    self.jar
                        .restore(&context.cookies().await?, self.base_url.as_deref())?;
                }
                let mut attempt = 0;
                loop {
                    let replay = request.try_clone().ok_or_else(|| {
                        E2eError::Config("API request body could not be replayed".into())
                    })?;
                    match self.send_attempt(replay, max_redirects).await {
                        Err(E2eError::Http(error))
                            if retryable_reset(&error) && attempt < max_retries =>
                        {
                            // Match the reference backoff, bounded by the same deadline.
                            let delay = Duration::from_millis(250)
                                .saturating_mul(1u32.checked_shl(attempt).unwrap_or(u32::MAX));
                            attempt += 1;
                            tokio::time::sleep(delay).await;
                        }
                        Ok(response)
                            if fail_on_status && !(200..400).contains(&response.status()) =>
                        {
                            let preview: String = response.text().chars().take(1000).collect();
                            return Err(E2eError::Config(format!(
                                "HTTP {} {} for {}\nResponse text:\n{}",
                                response.status(),
                                response.status_text(),
                                response.url(),
                                preview
                            )));
                        }
                        result => return result,
                    }
                }
            }),
        )
        .await
    }
    fn authorize(&self, request: &mut reqwest::Request) -> E2eResult<bool> {
        if request
            .headers()
            .contains_key(reqwest::header::AUTHORIZATION)
            || self
                .credential_origin
                .as_ref()
                .is_some_and(|origin| origin.origin() != request.url().origin())
        {
            return Ok(false);
        }
        let Some(credentials) = &self.credentials else {
            return Ok(false);
        };
        let header = format!(
            "Basic {}",
            crate::driver::base64_encode(
                format!("{}:{}", credentials.username, credentials.password).as_bytes()
            )
        );
        request.headers_mut().insert(
            reqwest::header::AUTHORIZATION,
            header
                .parse()
                .map_err(|e| E2eError::Config(format!("invalid API credentials: {e}")))?,
        );
        Ok(true)
    }
    async fn flush_cookies(&self) -> E2eResult<()> {
        let changes = self.jar.take_changes();
        if let Some(context) = &self.context {
            for (cookie, url) in changes {
                context.add_cookies(&[cookie], &url).await?;
            }
        }
        Ok(())
    }
    async fn send_attempt(
        &self,
        mut request: reqwest::Request,
        max_redirects: usize,
    ) -> E2eResult<ApiResponse> {
        let mut redirects = 0;
        loop {
            let outgoing = request
                .try_clone()
                .ok_or_else(|| E2eError::Config("API request body could not be replayed".into()))?;
            let response = self.client.execute(outgoing).await;
            // Cookies are accepted at every hop, even if a later redirect/body fails.
            self.flush_cookies().await?;
            let response = response?;
            let status = response.status().as_u16();
            if matches!(status, 301 | 302 | 303 | 307 | 308) && max_redirects != 0 {
                if redirects == max_redirects {
                    return Err(E2eError::Config("Max redirect count exceeded".into()));
                }
                let location = response
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .filter(|value| !value.is_empty());
                if let Some(location) = location {
                    let destination = request
                        .url()
                        .join(location)
                        .map_err(|e| E2eError::Config(format!("invalid redirect URL: {e}")))?;
                    if !matches!(destination.scheme(), "http" | "https") {
                        return Err(E2eError::Config("redirect URL must be HTTP(S)".into()));
                    }
                    let change_method = (matches!(status, 301 | 302)
                        && request.method() == reqwest::Method::POST)
                        || (status == 303
                            && !matches!(
                                *request.method(),
                                reqwest::Method::GET | reqwest::Method::HEAD
                            ));
                    if change_method {
                        *request.method_mut() = reqwest::Method::GET;
                        *request.body_mut() = None;
                        for name in [
                            "content-encoding",
                            "content-language",
                            "content-length",
                            "content-location",
                            "content-type",
                        ] {
                            request.headers_mut().remove(name);
                        }
                    }
                    if request.url().origin() != destination.origin() {
                        request.headers_mut().remove(reqwest::header::AUTHORIZATION);
                    }
                    request.headers_mut().remove(reqwest::header::COOKIE);
                    request.headers_mut().remove(reqwest::header::HOST);
                    *request.url_mut() = destination;
                    redirects += 1;
                    // Drop the redirect response rather than retaining its body/connection.
                    drop(response);
                    continue;
                }
            }
            if status == 401
                && response
                    .headers()
                    .get(reqwest::header::WWW_AUTHENTICATE)
                    .and_then(|value| value.to_str().ok())
                    .is_some_and(|value| value.trim_start().starts_with("Basic"))
                && self.authorize(&mut request)?
            {
                drop(response);
                continue;
            }
            let url = response.url().to_string();
            let status_text = response
                .status()
                .canonical_reason()
                .unwrap_or("")
                .to_string();
            let headers = response
                .headers()
                .iter()
                .map(|(name, value)| {
                    (
                        name.to_string(),
                        String::from_utf8_lossy(value.as_bytes()).into_owned(),
                    )
                })
                .collect();
            let body = response.bytes().await?.to_vec();
            return Ok(ApiResponse {
                url,
                status_text,
                status,
                headers,
                body,
            });
        }
    }
}

impl Default for ApiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ApiClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiClient")
            .field("base_url", &self.base_url)
            .field("context_linked", &self.context.is_some())
            .finish_non_exhaustive()
    }
}

/// Classify actual transport causes; do not retry refusal, TLS, parsing or body errors.
fn retryable_reset(error: &reqwest::Error) -> bool {
    if error.is_timeout() || error.is_body() || error.is_builder() {
        return false;
    }
    let mut cause: Option<&(dyn std::error::Error + 'static)> = Some(error);
    while let Some(error) = cause {
        if let Some(io) = error.downcast_ref::<std::io::Error>() {
            if io.kind() == std::io::ErrorKind::ConnectionReset {
                return true;
            }
        }
        if error
            .downcast_ref::<hyper::Error>()
            .is_some_and(|error| error.is_incomplete_message())
        {
            return true;
        }
        cause = error.source();
    }
    false
}
