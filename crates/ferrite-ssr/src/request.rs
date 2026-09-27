//! SSR request/response types.

use bytes::Bytes;
use ferrite_core::FerriteError;
use ferrite_core::Result;
use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;

/// An SSR HTTP request (framework-agnostic).
#[derive(Debug, Clone)]
pub struct SsrHttpRequest {
    /// Method (`GET`, ...).
    pub method: String,
    /// Full URI.
    pub uri: String,
    /// Headers.
    pub headers: Vec<(String, String)>,
    /// Body bytes.
    pub body: Vec<u8>,
}

/// SSR render context.
#[derive(Debug, Clone, Default)]
pub struct SsrContext {
    /// Request URL.
    pub url: String,
    /// Request headers.
    pub headers: HashMap<String, String>,
    /// CSP nonce, when configured.
    pub nonce: Option<String>,
    /// Preload files for this route (from the manifest).
    pub preload: Vec<String>,
}

/// Rendered body: full string or byte stream (§48).
pub enum RenderBody {
    /// Complete HTML.
    Full(String),
    /// Streamed HTML.
    Stream(Pin<Box<dyn Stream<Item = Result<Bytes>> + Send>>),
}

impl std::fmt::Debug for RenderBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Full(html) => f.debug_tuple("Full").field(&html.len()).finish(),
            Self::Stream(_) => f.debug_tuple("Stream").finish(),
        }
    }
}

/// SSR response.
#[derive(Debug)]
pub struct SsrResponse {
    /// Status code.
    pub status: u16,
    /// Headers.
    pub headers: Vec<(String, String)>,
    /// Body.
    pub body: RenderBody,
}

impl SsrResponse {
    /// HTML response.
    #[must_use]
    pub fn html(html: impl Into<String>) -> Self {
        Self {
            status: 200,
            headers: vec![(
                "content-type".to_string(),
                "text/html; charset=utf-8".to_string(),
            )],
            body: RenderBody::Full(html.into()),
        }
    }

    /// Streaming HTML response from chunks.
    #[must_use]
    pub fn stream(chunks: Vec<String>) -> Self {
        let stream = futures::stream::iter(chunks.into_iter().map(|chunk| Ok(Bytes::from(chunk))));
        Self {
            status: 200,
            headers: vec![(
                "content-type".to_string(),
                "text/html; charset=utf-8".to_string(),
            )],
            body: RenderBody::Stream(Box::pin(stream)),
        }
    }

    /// Collect the body into a string (buffers streams).
    pub async fn into_string(self) -> Result<String> {
        match self.body {
            RenderBody::Full(html) => Ok(html),
            RenderBody::Stream(mut stream) => {
                use futures::StreamExt as _;
                let mut output = Vec::new();
                while let Some(chunk) = stream.next().await {
                    output.extend_from_slice(&chunk?);
                }
                String::from_utf8(output)
                    .map_err(|error| FerriteError::Ssr(format!("non-utf8 stream: {error}")))
            }
        }
    }
}
