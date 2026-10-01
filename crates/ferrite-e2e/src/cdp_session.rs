//! Independently owned Chromium target sessions on the shared browser transport.
use crate::{
    cdp::{CdpConnection, CdpEvent},
    driver::Driver,
    operation::{Deadline, SharedClose},
    CancellationToken, E2eError, E2eResult, OperationOptions, Page,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

pub(crate) struct SessionState {
    pub(crate) id: Mutex<Option<String>>,
    pub(crate) closed: CancellationToken,
    pub(crate) native_gone: std::sync::atomic::AtomicBool,
    pub(crate) events: broadcast::Sender<CdpEvent>,
    pub(crate) connection: CdpConnection,
    pub(crate) disposal: SharedClose,
}
impl Drop for SessionState {
    fn drop(&mut self) {
        self.closed
            .cancel_with_reason("last CDP session owner dropped");
        let id = self.id.lock().unwrap_or_else(|e| e.into_inner()).clone();
        if let Some(id) = id {
            self.connection.forget_session(&id);
            if !self.native_gone.load(std::sync::atomic::Ordering::Acquire) {
                self.connection.detach_unobserved(&id);
            }
        }
    }
}
/// Cloneable session owner. Last-owner drop queues native detach; explicit
/// detach waits for native disposal and invalidates all clones and streams.
#[derive(Clone)]
pub struct CdpSession {
    state: Arc<SessionState>,
    owner: Driver,
}
impl CdpSession {
    /// Native flattened session identity; never the Page's internal session.
    pub fn id(&self) -> String {
        self.state
            .id
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .expect("attached session")
    }
    pub fn is_detached(&self) -> bool {
        self.state.closed.is_cancelled()
    }
    pub async fn send(&self, method: &str, params: Value) -> E2eResult<Value> {
        self.send_with(method, params, OperationOptions::default())
            .await
    }
    /// Send under one deadline, including page/context and caller cancellation.
    pub async fn send_with(
        &self,
        method: &str,
        params: Value,
        options: OperationOptions,
    ) -> E2eResult<Value> {
        let owner = match options.cancellation {
            Some(token) => self.owner.clone().with_cancellation(token),
            None => self.owner.clone(),
        };
        let timeout = options.timeout.unwrap_or_else(|| owner.timeout());
        owner
            .run(
                self.state.closed.run(
                    Deadline::new(timeout).run(
                        format!("CDP session {method}"),
                        self.state
                            .connection
                            .call(Some(&self.id()), method, params, timeout),
                    ),
                ),
            )
            .await
    }
    /// Subscribe only to this session. No listener tasks or page owners retained
    /// by the transport; overflow is reported rather than silently skipped.
    pub fn events(&self) -> CdpSessionEvents {
        CdpSessionEvents {
            receiver: self.state.events.subscribe(),
            closed: self.state.closed.clone(),
            owner: self.owner.clone(),
            lost: false,
        }
    }
    /// Idempotent disposal. Native detach continues if a caller drops its wait.
    /// Does not close the target or browser transport.
    pub async fn detach(&self) -> E2eResult<()> {
        self.detach_with(OperationOptions::default()).await
    }
    /// The first caller starts disposal under the selected timeout. Cancelling
    /// or dropping its wait leaves native cleanup running; later waits replay
    /// the same outcome. Zero disables timeout.
    pub async fn detach_with(&self, options: OperationOptions) -> E2eResult<()> {
        let timeout = options.timeout.unwrap_or_else(|| self.owner.timeout());
        let cancellation = options
            .cancellation
            .unwrap_or_else(|| self.owner.caller_cancellation());
        let state = self.state.clone();
        cancellation
            .run(Deadline::new(timeout).run(
                "detach CDP session",
                self.state.disposal.run(async move {
                    if state.closed.is_cancelled() {
                        return Ok(());
                    }
                    state.closed.cancel_with_reason("CDP session detached");
                    let id = state.id.lock().unwrap_or_else(|e| e.into_inner()).clone();
                    let result = state
                        .connection
                        .call(
                            None,
                            "Target.detachFromTarget",
                            json!({"sessionId":id}),
                            timeout,
                        )
                        .await;
                    if let Err(error) = result {
                        if !matches!(error, E2eError::Cdp { .. })
                            || !state.native_gone.load(std::sync::atomic::Ordering::Acquire)
                        {
                            return Err(error);
                        }
                    }
                    state
                        .native_gone
                        .store(true, std::sync::atomic::Ordering::Release);
                    Ok(())
                }),
            ))
            .await
    }
}
/// Bounded stream (256 events), with explicit loss, detach and owner errors.
pub struct CdpSessionEvents {
    receiver: broadcast::Receiver<CdpEvent>,
    closed: CancellationToken,
    owner: Driver,
    lost: bool,
}
impl CdpSessionEvents {
    pub async fn next(&mut self, options: OperationOptions) -> E2eResult<CdpEvent> {
        if self.lost {
            return Err(E2eError::Cancelled(
                "CDP event stream lost observations".into(),
            ));
        }
        let owner = match options.cancellation {
            Some(token) => self.owner.clone().with_cancellation(token),
            None => self.owner.clone(),
        };
        let timeout = options.timeout.unwrap_or_else(|| owner.timeout());
        owner
            .run(
                self.closed
                    .run(Deadline::new(timeout).run("CDP session event", async {
                        match self.receiver.recv().await {
                            Ok(event) => Ok(event),
                            Err(broadcast::error::RecvError::Lagged(count)) => {
                                self.lost = true;
                                Err(E2eError::Config(format!(
                                    "CDP session event stream lost {count} events"
                                )))
                            }
                            Err(broadcast::error::RecvError::Closed) => Err(E2eError::Cancelled(
                                "CDP session event source closed".into(),
                            )),
                        }
                    })),
            )
            .await
    }
}
impl Page {
    pub async fn new_cdp_session(&self) -> E2eResult<CdpSession> {
        self.new_cdp_session_with(OperationOptions::default()).await
    }
    /// Attach an independent target session. Late responses to abandoned
    /// attaches are detached by the connection reader, even after timeout.
    pub async fn new_cdp_session_with(&self, options: OperationOptions) -> E2eResult<CdpSession> {
        let connection = self.driver.cdp_connection()?;
        let page = self.operation_page(&options);
        let state = Arc::new(SessionState {
            id: Mutex::new(None),
            native_gone: std::sync::atomic::AtomicBool::new(false),
            closed: CancellationToken::new(),
            events: broadcast::channel(256).0,
            connection: connection.clone(),
            disposal: SharedClose::default(),
        });
        page.run_operation(
            Deadline::new(page.timeout()).run("attach CDP session", async {
                connection
                    .call_owned(
                        None,
                        "Target.attachToTarget",
                        json!({"targetId":page.target_id(),"flatten":true}),
                        page.timeout(),
                        Some(Arc::downgrade(&state)),
                    )
                    .await?;
                if state.id.lock().unwrap_or_else(|e| e.into_inner()).is_none() {
                    return Err(E2eError::Config(
                        "CDP attach response missing sessionId".into(),
                    ));
                }
                Ok(CdpSession {
                    state: state.clone(),
                    owner: self.driver.clone(),
                })
            }),
        )
        .await
    }
}
