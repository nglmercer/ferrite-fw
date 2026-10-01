//! Opt-in captured bytes, separate from native request/header completion.
use crate::{E2eError, E2eResult, OperationOptions, RequestFailure, Response};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "detail", rename_all = "snake_case")]
pub enum BodyCaptureState {
    #[default]
    NotCaptured,
    Pending,
    Available {
        bytes: usize,
    },
    Truncated {
        limit: usize,
    },
    Unavailable(String),
    Failed(RequestFailure),
}
pub(crate) struct BodySlot {
    generation: Option<u64>,
    bytes: Mutex<Option<Vec<u8>>>,
    pub(crate) state: tokio::sync::watch::Sender<BodyCaptureState>,
}
fn clip(text: &str) -> String {
    let mut end = text.len().min(4096);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].into()
}
impl BodySlot {
    pub(crate) fn new(generation: Option<u64>) -> Self {
        Self {
            generation,
            bytes: Mutex::new(None),
            state: tokio::sync::watch::channel(if generation.is_some() {
                BodyCaptureState::Pending
            } else {
                BodyCaptureState::NotCaptured
            })
            .0,
        }
    }
    pub(crate) fn complete(&self, generation: u64, result: Result<Vec<u8>, BodyCaptureState>) {
        if self.generation != Some(generation) {
            return;
        }
        let mut bytes = self.bytes.lock().unwrap_or_else(|e| e.into_inner());
        if !matches!(*self.state.borrow(), BodyCaptureState::Pending) {
            return;
        }
        let status = match result {
            Ok(body) if body.len() <= crate::driver::MAX_RESPONSE_BODY => {
                let length = body.len();
                *bytes = Some(body);
                BodyCaptureState::Available { bytes: length }
            }
            Ok(_) => BodyCaptureState::Truncated {
                limit: crate::driver::MAX_RESPONSE_BODY,
            },
            Err(BodyCaptureState::Unavailable(reason)) => {
                BodyCaptureState::Unavailable(clip(&reason))
            }
            Err(BodyCaptureState::Failed(failure)) => BodyCaptureState::Failed(RequestFailure {
                error_text: clip(&failure.error_text),
                cancelled: failure.cancelled,
            }),
            Err(status) => status,
        };
        self.state.send_replace(status);
    }
    pub(crate) fn unavailable(&self, reason: &str) {
        let _bytes = self.bytes.lock().unwrap_or_else(|e| e.into_inner());
        if matches!(*self.state.borrow(), BodyCaptureState::Pending) {
            self.state
                .send_replace(BodyCaptureState::Unavailable(clip(reason)));
        }
    }
    pub(crate) fn failed(&self, failure: &RequestFailure) {
        let _bytes = self.bytes.lock().unwrap_or_else(|e| e.into_inner());
        if matches!(
            *self.state.borrow(),
            BodyCaptureState::Pending | BodyCaptureState::NotCaptured
        ) {
            self.state
                .send_replace(BodyCaptureState::Failed(RequestFailure {
                    error_text: clip(&failure.error_text),
                    cancelled: failure.cancelled,
                }));
        }
    }
    pub(crate) fn size(&self) -> usize {
        self.bytes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map_or(0, Vec::len)
            + match &*self.state.borrow() {
                BodyCaptureState::Unavailable(reason) => reason.len(),
                BodyCaptureState::Failed(failure) => failure.error_text.len(),
                _ => 0,
            }
    }
    pub(crate) fn captured_bytes(&self) -> Option<Vec<u8>> {
        self.bytes.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
    fn ready(&self, url: &str) -> Option<E2eResult<Vec<u8>>> {
        let bytes = self.bytes.lock().unwrap_or_else(|e| e.into_inner());
        let status = self.state.borrow().clone();
        match status {
            BodyCaptureState::Pending => None,
            BodyCaptureState::Available { .. } => Some(
                bytes
                    .as_ref()
                    .cloned()
                    .ok_or_else(|| body_error(url, "captured bytes no longer available".into())),
            ),
            status => Some(Err(body_error(
                url,
                format!("captured response body unavailable: {status:?}"),
            ))),
        }
    }
}
fn body_error(url: &str, message: String) -> E2eError {
    E2eError::Network {
        url: url.into(),
        message,
    }
}

/// Check the encoded length before allocating decoded bytes. Protocol ingress is
/// already parsed by the native transport and is outside this retained-body cap.
pub(crate) fn decode_body(got: &serde_json::Value) -> Result<Vec<u8>, BodyCaptureState> {
    let unavailable = |message: &str| BodyCaptureState::Unavailable(clip(message));
    let body = got["body"]
        .as_str()
        .ok_or_else(|| unavailable("native body reply has no string body"))?;
    let bytes = if got["base64Encoded"].as_bool() == Some(true) {
        let max_encoded = crate::driver::MAX_RESPONSE_BODY.div_ceil(3) * 4;
        if body.len() > max_encoded {
            return Err(BodyCaptureState::Truncated {
                limit: crate::driver::MAX_RESPONSE_BODY,
            });
        }
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(body)
            .map_err(|error| {
                unavailable(&format!("invalid native response-body base64: {error}"))
            })?
    } else {
        if body.len() > crate::driver::MAX_RESPONSE_BODY {
            return Err(BodyCaptureState::Truncated {
                limit: crate::driver::MAX_RESPONSE_BODY,
            });
        }
        body.as_bytes().to_vec()
    };
    if bytes.len() > crate::driver::MAX_RESPONSE_BODY {
        Err(BodyCaptureState::Truncated {
            limit: crate::driver::MAX_RESPONSE_BODY,
        })
    } else {
        Ok(bytes)
    }
}

impl Response {
    pub fn body_capture_state(&self) -> BodyCaptureState {
        self.request.state.body.state.borrow().clone()
    }
    /// Opt-in captured bytes, never a refetch. Completed bytes remain usable after close.
    pub async fn body(&self) -> E2eResult<Vec<u8>> {
        self.body_with_options(OperationOptions::default()).await
    }
    pub async fn body_with_options(&self, options: OperationOptions) -> E2eResult<Vec<u8>> {
        if matches!(self.request.page.driver, crate::driver::Driver::Bidi(_)) {
            return Err(E2eError::Config(
                "captured response bodies are not supported on Firefox".into(),
            ));
        }
        if let Some(token) = &options.cancellation {
            token.check()?;
        }
        let body = &self.request.state.body;
        let mut changed = body.state.subscribe();
        if let Some(result) = body.ready(self.url()) {
            return result;
        }
        let timeout = options
            .timeout
            .unwrap_or_else(|| self.request.page.timeout());
        let page = self.request.page.operation_page(&options);
        page.run_operation(crate::operation::Deadline::new(timeout).run(
            "captured response body",
            async {
                loop {
                    if let Some(result) = body.ready(self.url()) {
                        return result;
                    }
                    changed.changed().await.map_err(|_| {
                        E2eError::Disconnected("body capture observation ended".into())
                    })?;
                }
            },
        ))
        .await
    }
    /// UTF-8 with replacement for invalid byte sequences, matching Response.text.
    pub async fn text(&self) -> E2eResult<String> {
        self.text_with_options(OperationOptions::default()).await
    }
    pub async fn text_with_options(&self, options: OperationOptions) -> E2eResult<String> {
        Ok(String::from_utf8_lossy(&self.body_with_options(options).await?).into_owned())
    }
    pub async fn json<T: serde::de::DeserializeOwned>(&self) -> E2eResult<T> {
        self.json_with_options(OperationOptions::default()).await
    }
    pub async fn json_with_options<T: serde::de::DeserializeOwned>(
        &self,
        options: OperationOptions,
    ) -> E2eResult<T> {
        let bytes = self.body_with_options(options).await?;
        serde_json::from_slice(&bytes).map_err(|source| E2eError::Diagnostic {
            context: format!("response.json({:?})", self.url()),
            source: Box::new(E2eError::Json(source)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use serde_json::json;

    #[test]
    fn decoding_distinguishes_empty_missing_invalid_and_oversized() {
        let cap = crate::driver::MAX_RESPONSE_BODY;
        assert_eq!(decode_body(&json!({"body":""})), Ok(vec![]));
        assert!(matches!(
            decode_body(&json!({})),
            Err(BodyCaptureState::Unavailable(_))
        ));
        assert!(matches!(
            decode_body(&json!({"body":"!", "base64Encoded":true})),
            Err(BodyCaptureState::Unavailable(_))
        ));
        for length in [cap, cap + 1] {
            let bytes = vec![255; length];
            let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
            let decoded = decode_body(&json!({"body":encoded,"base64Encoded":true}));
            if length == cap {
                assert_eq!(decoded.unwrap(), bytes);
            } else {
                assert_eq!(decoded, Err(BodyCaptureState::Truncated { limit: cap }));
            }
            let plain = decode_body(&json!({"body":"x".repeat(length)}));
            if length == cap {
                assert_eq!(plain.unwrap().len(), cap);
            } else {
                assert_eq!(plain, Err(BodyCaptureState::Truncated { limit: cap }));
            }
        }
    }

    #[test]
    fn stale_generation_and_stop_cannot_resurrect_pending_bytes() {
        let slot = BodySlot::new(Some(2));
        slot.complete(1, Ok(vec![1]));
        assert!(slot.ready("url").is_none());
        slot.unavailable("stopped");
        slot.complete(2, Ok(vec![2]));
        assert!(slot.ready("url").unwrap().is_err());
        assert_eq!(slot.size(), "stopped".len());
    }

    #[test]
    fn cached_empty_body_survives_close_and_differs_from_uncaptured() {
        let slot = BodySlot::new(Some(2));
        slot.complete(2, Ok(vec![]));
        slot.unavailable("closed");
        assert_eq!(slot.ready("url").unwrap().unwrap(), Vec::<u8>::new());
        assert_eq!(
            *slot.state.borrow(),
            BodyCaptureState::Available { bytes: 0 }
        );
        assert!(BodySlot::new(None).ready("url").unwrap().is_err());
    }

    #[test]
    fn terminal_reasons_are_utf8_safe_and_bounded() {
        let slot = BodySlot::new(Some(1));
        slot.complete(1, Err(BodyCaptureState::Unavailable("🦀".repeat(4096))));
        assert_eq!(slot.size(), 4096);
        let failed = BodySlot::new(None);
        failed.failed(&RequestFailure {
            error_text: "🦀".repeat(4096),
            cancelled: Some(false),
        });
        assert_eq!(failed.size(), 4096);
        let state = failed.state.borrow().clone();
        assert_eq!(
            serde_json::from_value::<BodyCaptureState>(serde_json::to_value(&state).unwrap())
                .unwrap(),
            state
        );
    }
}
