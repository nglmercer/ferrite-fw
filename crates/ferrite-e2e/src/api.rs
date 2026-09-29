//! Standalone HTTP API client (no browser needed).

use crate::{BrowserContext, Cookie, HttpCredentials};
use reqwest::cookie::CookieStore;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::{sync::Arc, time::Duration};

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
    /// Retry connection failures only; HTTP error statuses are never retried.
    pub max_retries: u32,
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
    jar: Arc<reqwest::cookie::Jar>,
    context: Option<BrowserContext>,
    credentials: Option<HttpCredentials>,
}

impl ApiClient {
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
        let jar = Arc::new(reqwest::cookie::Jar::default());
        let mut builder = reqwest::Client::builder()
            .cookie_provider(jar.clone())
            .danger_accept_invalid_certs(options.ignore_https_errors)
            .redirect(if options.max_redirects == 0 {
                reqwest::redirect::Policy::none()
            } else {
                reqwest::redirect::Policy::limited(options.max_redirects)
            });
        if !options.timeout.is_zero() {
            builder = builder.timeout(options.timeout);
        }
        if let Some(proxy) = &options.proxy {
            builder = builder.proxy(reqwest::Proxy::all(proxy)?);
        }
        Ok(Self {
            client: builder.build()?,
            base_url: options.base_url,
            headers: options.headers,
            jar,
            context: None,
            credentials: options.credentials,
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
            .parse::<reqwest::Method>()
            .map_err(|e| E2eError::Config(e.to_string()))?;
        let mut attempt = 0;
        loop {
            let mut request = self
                .client
                .request(method.clone(), self.url(path)?)
                .query(&options.query);
            for (name, value) in &options.headers {
                request = request.header(name, value);
            }
            if let Some(timeout) = options.timeout.filter(|t| !t.is_zero()) {
                request = request.timeout(timeout);
            }
            if let Some(body) = &options.body {
                request = request.body(body.clone());
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
            match self.send(request).await {
                Err(E2eError::Http(error))
                    if error.is_connect() && attempt < options.max_retries =>
                {
                    attempt += 1;
                }
                Ok(response) if options.fail_on_status_code && response.status() >= 400 => {
                    return Err(E2eError::Config(format!(
                        "HTTP {} {} for {}",
                        response.status(),
                        response.status_text(),
                        response.url()
                    )))
                }
                result => return result,
            }
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
    fn url(&self, path: &str) -> E2eResult<String> {
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

    /// Send a request with default headers; transport errors are `Http`.
    async fn send(&self, request: reqwest::RequestBuilder) -> E2eResult<ApiResponse> {
        let mut request = request.build()?;
        for (name, value) in &self.headers {
            let name = reqwest::header::HeaderName::from_bytes(name.as_bytes())
                .map_err(|e| E2eError::Config(e.to_string()))?;
            if !request.headers().contains_key(&name) {
                request.headers_mut().insert(
                    name,
                    value
                        .parse()
                        .map_err(|e| E2eError::Config(format!("invalid API header: {e}")))?,
                );
            }
        }
        let mut request = reqwest::RequestBuilder::from_parts(self.client.clone(), request);
        if let Some(credentials) = &self.credentials {
            request = request.basic_auth(&credentials.username, Some(&credentials.password));
        }
        let mut request = request.build()?;
        if let Some(context) = &self.context {
            let url = request.url().clone();
            let cookies = context.cookies().await?;
            let header = cookies
                .iter()
                .filter(|c| cookie_matches(c, &url))
                .map(|c| format!("{}={}", c.name, c.value))
                .collect::<Vec<_>>()
                .join("; ");
            if !request.headers().contains_key(reqwest::header::COOKIE) {
                request.headers_mut().insert(
                    reqwest::header::COOKIE,
                    header
                        .parse()
                        .map_err(|e| E2eError::Config(format!("invalid cookie header: {e}")))?,
                );
            }
        }
        let response = self.client.execute(request).await?;
        let url = response.url().to_string();
        let status_text = response
            .status()
            .canonical_reason()
            .unwrap_or("")
            .to_string();
        if let Some(context) = &self.context {
            let cookies: Vec<Cookie> = response
                .headers()
                .get_all(reqwest::header::SET_COOKIE)
                .iter()
                .filter_map(|header| {
                    let parsed = cookie::Cookie::parse(header.to_str().ok()?.to_string()).ok()?;
                    Some(Cookie {
                        name: parsed.name().into(),
                        value: parsed.value().into(),
                        domain: parsed.domain().map(str::to_string),
                        path: parsed
                            .path()
                            .map(str::to_string)
                            .or_else(|| Some(default_cookie_path(response.url()))),
                        http_only: parsed.http_only().unwrap_or(false),
                        secure: parsed.secure().unwrap_or(false),
                        expires: parsed
                            .max_age()
                            .map(|age| {
                                std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs() as i64
                                    + age.whole_seconds()
                            })
                            .or_else(|| {
                                parsed.expires_datetime().map(|time| time.unix_timestamp())
                            }),
                    })
                })
                .collect();
            if !cookies.is_empty() {
                context.add_cookies(&cookies, &url).await?;
            }
        }
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
            url,
            status_text,
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

impl std::fmt::Debug for ApiClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiClient")
            .field("base_url", &self.base_url)
            .field("context_linked", &self.context.is_some())
            .finish_non_exhaustive()
    }
}

fn default_cookie_path(url: &reqwest::Url) -> String {
    url.path()
        .rsplit_once('/')
        .map(|(prefix, _)| if prefix.is_empty() { "/" } else { prefix })
        .unwrap_or("/")
        .to_string()
}

fn cookie_matches(cookie: &Cookie, url: &reqwest::Url) -> bool {
    let host = url.host_str().unwrap_or("");
    let domain = cookie.domain.as_deref().unwrap_or(host);
    let domain_match = if domain.starts_with('.') {
        host == domain.trim_start_matches('.') || host.ends_with(domain)
    } else {
        host == domain
    };
    let path = cookie.path.as_deref().unwrap_or("/");
    let path_match = url.path() == path
        || (url.path().starts_with(path)
            && (path.ends_with('/') || url.path().as_bytes().get(path.len()) == Some(&b'/')));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    domain_match
        && path_match
        && (!cookie.secure || url.scheme() == "https")
        && cookie
            .expires
            .is_none_or(|expiry| expiry < 0 || expiry > now)
}
