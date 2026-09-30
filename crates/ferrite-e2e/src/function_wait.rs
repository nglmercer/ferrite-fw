use crate::{CancellationToken, E2eError, E2eResult, JSHandle, Page};
use serde_json::Value;
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

/// Predicate scheduling happens in the document, independently of protocol reads.
#[derive(Debug, Clone, Copy, Default)]
pub enum FunctionPolling {
    #[default]
    AnimationFrame,
    /// Nonzero whole milliseconds, up to the browser's signed 32-bit timer limit.
    Interval(Duration),
}

/// Options for a function/expression wait. JSON arguments are passed to functions.
/// Zero disables the local timeout; caller and enclosing cancellation still apply.
#[derive(Debug, Clone, Default)]
pub struct FunctionWaitOptions {
    pub polling: FunctionPolling,
    pub timeout: Option<Duration>,
    pub cancellation: Option<CancellationToken>,
}
impl FunctionWaitOptions {
    pub fn polling(mut self, polling: FunctionPolling) -> Self {
        self.polling = polling;
        self
    }
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
    pub fn cancellation(mut self, token: CancellationToken) -> Self {
        self.cancellation = Some(token);
        self
    }
}

pub(crate) enum ResultValue {
    Json(Value),
    Handle(Box<JSHandle>),
}
const MAP: &str = "globalThis[Symbol.for('ferrite.functionWaits')]";
static NEXT: AtomicU64 = AtomicU64::new(1);

/// Armed before installation: even a dropped registration command is cleaned up.
struct Cleanup {
    page: Page,
    id: String,
    armed: bool,
}
impl Cleanup {
    async fn clean(&mut self) {
        let expression = format!("(() => {{ const m = {MAP}; const s = m?.get({}); if (s) {{ s.stop(); m.delete({}); }} return true; }})()", self.id, self.id);
        let _ = self.page.evaluate_value(&expression).await;
        self.armed = false;
    }
}
impl Drop for Cleanup {
    fn drop(&mut self) {
        if self.armed {
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                let mut cleanup = Self {
                    page: self.page.clone(),
                    id: self.id.clone(),
                    armed: true,
                };
                runtime.spawn(async move {
                    cleanup.clean().await;
                });
            }
        }
    }
}

pub(crate) async fn wait(
    page: &Page,
    expression: &str,
    argument: Value,
    options: FunctionWaitOptions,
    remote: bool,
) -> E2eResult<ResultValue> {
    let interval = match options.polling {
        FunctionPolling::AnimationFrame => Value::Null,
        FunctionPolling::Interval(duration) => {
            if duration.as_millis() == 0
                || duration.as_millis() > i32::MAX as u128
                || duration.subsec_nanos() % 1_000_000 != 0
            {
                return Err(E2eError::Config(
                    "function polling interval must be whole milliseconds in 1..=2147483647".into(),
                ));
            }
            Value::from(duration.as_millis() as u64)
        }
    };
    let timeout = options.timeout.unwrap_or_else(|| page.timeout());
    let mut scoped = page.with_timeout(timeout);
    if let Some(token) = options.cancellation {
        scoped = scoped.with_cancellation(token);
    }
    let id = serde_json::to_string(&format!("wait-{}", NEXT.fetch_add(1, Ordering::Relaxed)))?;
    let mut cleanup = Cleanup {
        page: page
            .with_timeout(Duration::from_millis(750))
            .with_cancellation(CancellationToken::new()),
        id: id.clone(),
        armed: true,
    };
    let install = format!(
        "({})({}, {}, {}, {}, {})",
        include_str!("function_wait.js"),
        id,
        serde_json::to_string(expression)?,
        argument,
        interval,
        remote
    );
    let query = format!("(() => {{ const s = {MAP}?.get({id}); return s ? {{status:s.status, json:s.json, error:s.error}} : {{status:'missing'}}; }})()");
    let result = scoped.run_operation(crate::operation::Deadline::new(timeout).run(
        "wait_for_function", async {
            loop {
                let observed = scoped.evaluate_value(&query).await;
                let observed = match observed {
                    Ok(value) => value,
                    Err(error) if crate::url_wait::navigation_replaced_realm(&error) => {
                        tokio::time::sleep(Duration::from_millis(25)).await;
                        continue;
                    }
                    Err(error) => return Err(error),
                };
                match observed["status"].as_str() {
                    Some("missing") => {
                        match scoped.evaluate_value(&install).await {
                            Ok(_) => {},
                            Err(error) if crate::url_wait::navigation_replaced_realm(&error) => {},
                            Err(error) => return Err(error),
                        }
                    }
                    Some("error") => return Err(E2eError::Cdp { method: "wait_for_function".into(), message: observed["error"].as_str().unwrap_or("predicate failed").into() }),
                    Some("ready") => {
                        if remote {
                            let take = format!("(() => {{ const s = {MAP}?.get({id}); if (!s || s.status !== 'ready') throw new Error('function wait result document was replaced'); return s.value; }})()");
                            return scoped.evaluate_handle(&take).await.map(|handle| ResultValue::Handle(Box::new(handle)));
                        }
                        return serde_json::from_str(observed["json"].as_str().unwrap_or("null")).map(ResultValue::Json).map_err(E2eError::Json);
                    }
                    _ => {},
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        },
    )).await;
    cleanup.clean().await;
    result
}
