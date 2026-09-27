//! Server-function RPC transport.

use ferrite_core::FerriteError;
use ferrite_core::Result;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

/// Server-function RPC transport (§50).
pub const RPC_ROUTE_PREFIX: &str = "/_ferrite/rpc/";

/// RPC encodings (§50).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpcEncoding {
    /// JSON (default).
    Json,
    /// MessagePack.
    MessagePack,
    /// CBOR.
    Cbor,
}

impl RpcEncoding {
    /// Wire content type.
    #[must_use]
    pub fn content_type(self) -> &'static str {
        match self {
            Self::Json => "application/json",
            Self::MessagePack => "application/msgpack",
            Self::Cbor => "application/cbor",
        }
    }

    /// Detect the encoding from a `Content-Type` header value (parameters
    /// ignored). Missing or unknown types fall back to JSON.
    #[must_use]
    pub fn from_content_type(content_type: Option<&str>) -> Self {
        match content_type.map(|raw| raw.split(';').next().unwrap_or("").trim()) {
            Some("application/msgpack" | "application/x-msgpack") => Self::MessagePack,
            Some("application/cbor") => Self::Cbor,
            _ => Self::Json,
        }
    }

    /// Decode RPC arguments from wire bytes.
    pub fn decode(self, bytes: &[u8]) -> Result<serde_json::Value> {
        match self {
            Self::Json => serde_json::from_slice(bytes)
                .map_err(|error| FerriteError::Ssr(format!("bad JSON rpc args: {error}"))),
            Self::MessagePack => rmp_serde::from_slice(bytes)
                .map_err(|error| FerriteError::Ssr(format!("bad MessagePack rpc args: {error}"))),
            Self::Cbor => ciborium::from_reader(bytes)
                .map_err(|error| FerriteError::Ssr(format!("bad CBOR rpc args: {error}"))),
        }
    }

    /// Encode an RPC result to wire bytes.
    pub fn encode(self, value: &serde_json::Value) -> Result<Vec<u8>> {
        match self {
            Self::Json => serde_json::to_vec(value)
                .map_err(|error| FerriteError::Ssr(format!("cannot encode JSON rpc: {error}"))),
            Self::MessagePack => rmp_serde::to_vec(value).map_err(|error| {
                FerriteError::Ssr(format!("cannot encode MessagePack rpc: {error}"))
            }),
            Self::Cbor => {
                let mut bytes = Vec::new();
                ciborium::into_writer(value, &mut bytes).map_err(|error| {
                    FerriteError::Ssr(format!("cannot encode CBOR rpc: {error}"))
                })?;
                Ok(bytes)
            }
        }
    }
}

/// An RPC invocation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RpcRequest {
    /// Function hash.
    pub hash: String,
    /// JSON-encoded arguments.
    pub args: serde_json::Value,
}

/// Registry of server functions (`hash` → handler).
#[derive(Clone, Default)]
pub struct RpcRegistry {
    /// Handlers.
    handlers: HashMap<String, Arc<dyn Fn(serde_json::Value) -> RpcBoxFuture + Send + Sync>>,
}

pub(crate) type RpcBoxFuture =
    Pin<Box<dyn std::future::Future<Output = Result<serde_json::Value>> + Send>>;

impl RpcRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a handler for `hash`.
    pub fn register<F, Fut>(&mut self, hash: impl Into<String>, handler: F)
    where
        F: Fn(serde_json::Value) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<serde_json::Value>> + Send + 'static,
    {
        self.handlers.insert(
            hash.into(),
            Arc::new(move |args| Box::pin(handler(args)) as RpcBoxFuture),
        );
    }

    /// Invoke a handler.
    pub async fn invoke(&self, request: &RpcRequest) -> Result<serde_json::Value> {
        let handler = self.handlers.get(&request.hash).ok_or_else(|| {
            FerriteError::Ssr(format!("unknown server function `{}`", request.hash))
        })?;
        handler(request.args.clone()).await
    }
}

impl std::fmt::Debug for RpcRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RpcRegistry")
            .field("hashes", &self.handlers.keys().collect::<Vec<_>>())
            .finish()
    }
}
