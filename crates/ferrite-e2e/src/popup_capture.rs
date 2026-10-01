//! Popup observations captured before asynchronous page adoption.
use crate::driver::ConsoleSink;
use crate::{BrowserContext, ConsoleMessage, RequestSnapshot};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, Weak};

pub(crate) const MAX_POPUP_HISTORY: usize = 64;
const MAX_STARTUP_EVENTS: usize = 1024;
const MAX_STARTUP_BYTES: usize = 2 * 1024 * 1024;

/// Adoption outcome, independent of whether native closure was observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PopupAdoption {
    Pending,
    Adopted,
    ClosedBeforeAdoption,
    Failed,
    Cancelled,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn record(id: &str) -> PopupRecord {
        PopupRecord {
            metadata: PopupDiagnostics {
                page_id: id.into(),
                opener_id: "root".into(),
                adoption: PopupAdoption::Pending,
                closed: false,
                error: None,
                truncated: false,
                dropped_events: 0,
                initial_document_replaced: false,
                console: Vec::new(),
                requests: Vec::new(),
            },
            requests: Vec::new(),
            events: 0,
            bytes: 0,
            accept_current: false,
            first_main_commit: false,
            document: None,
        }
    }
    #[test]
    fn startup_limits_are_visible_without_blocking_adopted_page_observations() {
        let mut record = record("popup");
        record.document(Some("doc1"), Some("http://fixture/first"));
        assert!(record.allow(MAX_STARTUP_BYTES));
        assert!(!record.allow(1));
        assert_eq!(record.metadata.dropped_events, 1);
        record.metadata.adoption = PopupAdoption::Adopted;
        assert!(
            record.allow(1),
            "normal adopted page observation must continue after projection truncation"
        );
        let message: ConsoleMessage = serde_json::from_value(
            json!({"kind":"log","text":"must not expand truncated projection"}),
        )
        .unwrap();
        record.console(message);
        assert!(record.metadata.console.is_empty());
        assert_eq!(record.metadata.dropped_events, 2);
        record.document(Some("doc1"), Some("http://fixture/first"));
        assert!(!record.metadata.initial_document_replaced);
        record.document(Some("doc2"), Some("http://fixture/second"));
        assert!(record.metadata.initial_document_replaced);
        assert!(record.allow(1));
        assert_eq!(record.metadata.dropped_events, 2);
        let mut record = super::tests::record("bounded events");
        for _ in 0..MAX_STARTUP_EVENTS {
            assert!(record.allow(0));
        }
        assert!(!record.allow(0));
        assert!(record.metadata.truncated);
        let mut record = super::tests::record("pending-document-replacement");
        record.document(Some("doc1"), Some("http://fixture/first"));
        record.document(Some("doc2"), Some("http://fixture/second"));
        assert!(record.allow(100));
        let message: ConsoleMessage =
            serde_json::from_value(json!({"kind":"log","text":"pre-adoption"})).unwrap();
        record.console(message.clone());
        assert_eq!(
            record.metadata.console.len(),
            1,
            "all pre-adoption documents are retained"
        );
        record.metadata.adoption = PopupAdoption::Adopted;
        assert!(record.allow(100));
        record.console(message);
        assert_eq!(
            record.metadata.console.len(),
            1,
            "the projection freezes after adoption and replacement"
        );
    }
    #[test]
    fn history_eviction_clear_and_request_completion_release_owned_state() {
        let mut history = PopupHistory::default();
        let first = Arc::new(Mutex::new(record("first")));
        let weak = Arc::downgrade(&first);
        history.insert(first);
        for i in 0..MAX_POPUP_HISTORY {
            history.insert(Arc::new(Mutex::new(record(&i.to_string()))));
        }
        assert!(weak.upgrade().is_none());
        assert_eq!(history.snapshot().entries.len(), MAX_POPUP_HISTORY);
        assert_eq!(history.snapshot().dropped_popups, 1);
        let mut log = crate::network::NetworkLog::default();
        let request: crate::RecordedRequest = serde_json::from_value(
            json!({"method":"GET","url":"http://fixture/early","status":0,"request_id":"native"}),
        )
        .unwrap();
        let state = log.start(request, Default::default(), Some("last".into()));
        let weak = Arc::downgrade(&state);
        let mut entry = record("last");
        assert!(entry.allow(100));
        entry.request(state);
        history.insert(Arc::new(Mutex::new(entry)));
        log.finish("native", None);
        drop(log);
        assert!(matches!(
            history.snapshot().entries.last().unwrap().requests[0].completion,
            crate::RequestCompletion::Finished
        ));
        assert!(weak.upgrade().is_some());
        history.clear();
        assert!(weak.upgrade().is_none());
        assert!(history.snapshot().entries.is_empty());
        assert_eq!(history.snapshot().dropped_popups, 0);
        let sink = ConsoleSink::new();
        let request:crate::RecordedRequest=serde_json::from_value(json!({"method":"GET","url":"http://fixture/truncated","status":0,"request_id":"truncated"})).unwrap();
        let state = sink.network_log.lock().unwrap().start(
            request,
            Default::default(),
            Some("truncated".into()),
        );
        let mut record = record("truncated");
        assert!(record.allow(MAX_STARTUP_BYTES));
        record.request(state.clone());
        let capture = PopupCapture {
            sink,
            context_id: None,
            session: None,
            page_id: "truncated".into(),
            owner_cancel: crate::CancellationToken::new(),
            record: Arc::new(Mutex::new(record)),
            lifecycle: Mutex::new(None),
        };
        assert!(!capture.allow(1));
        assert!(
            matches!(
                state.snapshot().completion,
                crate::RequestCompletion::Unavailable(_)
            ),
            "lost pre-adoption native completion must settle explicitly"
        );
        capture.stop("owner cancelled", true);
        capture.failed("late initialization error");
        let metadata = &capture.record.lock().unwrap().metadata;
        assert_eq!(metadata.adoption, PopupAdoption::Cancelled);
        assert_eq!(metadata.error.as_deref(), Some("owner cancelled"));
    }
    #[tokio::test]
    async fn transport_bursts_before_popup_subscription_retain_closed_source_and_release_capture() {
        use futures::{SinkExt, StreamExt};
        for bidi in [false, true] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                let (stream, _) = listener.accept().await.unwrap();
                let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
                while let Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) =
                    socket.next().await
                {
                    let command: serde_json::Value = serde_json::from_str(&text).unwrap();
                    if matches!(
                        command["method"].as_str(),
                        Some("test.popupBurst" | "test.liveBurst" | "test.closeBurst")
                    ) {
                        let mut events = if bidi {
                            vec![
                                json!({"type":"event","method":"browsingContext.contextCreated","params":{"context":"popup","parent":null,"originalOpener":"root","url":"about:blank"}}),
                                json!({"type":"event","method":"log.entryAdded","params":{"type":"console","method":"log","text":"early-popup","source":{"context":"popup"}}}),
                                json!({"type":"event","method":"network.beforeRequestSent","params":{"context":"popup","request":{"request":"native","url":"http://fixture/early","method":"GET","headers":[]}}}),
                                json!({"type":"event","method":"browsingContext.contextDestroyed","params":{"context":"popup"}}),
                            ]
                        } else {
                            vec![
                                json!({"method":"Target.attachedToTarget","params":{"sessionId":"popup-session","targetInfo":{"targetId":"popup","openerId":"root","type":"page"}}}),
                                json!({"sessionId":"popup-session","method":"Runtime.consoleAPICalled","params":{"type":"log","args":[{"type":"string","value":"early-popup"}]}}),
                                json!({"sessionId":"popup-session","method":"Network.requestWillBeSent","params":{"requestId":"native","frameId":"popup-frame","request":{"url":"http://fixture/early","method":"GET","headers":{}}}}),
                                json!({"method":"Target.targetDestroyed","params":{"targetId":"popup"}}),
                            ]
                        };
                        if command["method"] == "test.liveBurst" {
                            events.pop();
                            events = events
                                .into_iter()
                                .map(|event| {
                                    serde_json::from_str(
                                        &event.to_string().replace("popup", "failed"),
                                    )
                                    .unwrap()
                                })
                                .collect();
                        } else if command["method"] == "test.closeBurst" {
                            events = vec![events[1].clone(), events[3].clone()]
                                .into_iter()
                                .map(|event| {
                                    serde_json::from_str(
                                        &event
                                            .to_string()
                                            .replace("popup", "failed")
                                            .replace("early-failed", "late-failed"),
                                    )
                                    .unwrap()
                                })
                                .collect();
                        }
                        for event in events {
                            socket
                                .send(tokio_tungstenite::tungstenite::Message::Text(
                                    event.to_string().into(),
                                ))
                                .await
                                .unwrap();
                        }
                    }
                    if (command["method"] == "Page.getFrameTree"
                        && command["sessionId"] == "failed-session")
                        || (command["method"] == "browsingContext.getTree"
                            && command["params"]["root"] == "failed")
                    {
                        let reply = if bidi {
                            json!({"type":"error","id":command["id"],"error":"unknown error","message":"injected popup initialization failure"})
                        } else {
                            json!({"id":command["id"],"error":{"code":-32000,"message":"injected popup initialization failure"}})
                        };
                        socket
                            .send(tokio_tungstenite::tungstenite::Message::Text(
                                reply.to_string().into(),
                            ))
                            .await
                            .unwrap();
                        continue;
                    }
                    let result = match command["method"].as_str().unwrap() {
                        "browsingContext.create" => {
                            json!({"context":if command["params"]["userContext"] == "owner2" {"root2"} else {"root"}})
                        }
                        "browsingContext.getTree" => {
                            if command["params"]["maxDepth"] == 0 {
                                json!({"contexts":[]})
                            } else {
                                json!({"contexts":[{"context":"root","url":"about:blank","children":[]}]})
                            }
                        }
                        "Target.closeTarget" => json!({"success":true}),
                        "Target.getTargets" => json!({"targetInfos":[]}),
                        "Target.createTarget" => {
                            json!({"targetId":if command["params"]["browserContextId"] == "owner2" {"root2"} else {"root"}})
                        }
                        "Target.attachToTarget" => {
                            json!({"sessionId":if command["params"]["targetId"] == "root2" {"root2-session"} else {"root-session"}})
                        }
                        "Page.getFrameTree" => {
                            json!({"frameTree":{"frame":{"id":"native-root","url":"about:blank","loaderId":"root-document"}}})
                        }
                        _ => json!({}),
                    };
                    let reply = if bidi {
                        json!({"type":"success","id":command["id"],"result":result})
                    } else {
                        json!({"id":command["id"],"result":result})
                    };
                    socket
                        .send(tokio_tungstenite::tungstenite::Message::Text(
                            reply.to_string().into(),
                        ))
                        .await
                        .unwrap();
                }
            });
            let backend = if bidi {
                crate::browser::Backend::Bidi {
                    conn: crate::bidi::BidiConnection::connect(&format!("ws://{address}"))
                        .await
                        .unwrap(),
                    insecure_certs: false,
                }
            } else {
                crate::browser::Backend::Cdp(
                    crate::cdp::CdpConnection::connect(&format!("ws://{address}"))
                        .await
                        .unwrap(),
                )
            };
            let contexts = Arc::new(Mutex::new(Vec::new()));
            let captures = Arc::new(PopupCaptures::new(Arc::downgrade(&contexts)));
            let context = BrowserContext::new(
                backend.clone(),
                Some("owner".into()),
                Default::default(),
                std::time::Duration::ZERO,
                std::time::Duration::from_secs(2),
                None,
                Arc::downgrade(&contexts),
                None,
            );
            contexts.lock().unwrap().push(context.clone());
            match &backend {
                crate::browser::Backend::Cdp(conn) => conn.set_popup_captures(captures.clone()),
                crate::browser::Backend::Bidi { conn, .. } => {
                    conn.set_popup_captures(captures.clone())
                }
            };
            let _root = context.new_page().await.unwrap();
            let mut events = context.subscribe();
            match &backend {
                crate::browser::Backend::Cdp(conn) => {
                    conn.call(
                        None,
                        "test.popupBurst",
                        json!({}),
                        std::time::Duration::from_secs(2),
                    )
                    .await
                    .unwrap();
                }
                crate::browser::Backend::Bidi { conn, .. } => {
                    conn.call(
                        "test.popupBurst",
                        json!({}),
                        std::time::Duration::from_secs(2),
                    )
                    .await
                    .unwrap();
                }
            };
            let history = context.popup_diagnostics();
            assert_eq!(history.entries.len(), 1);
            let popup = &history.entries[0];
            assert_eq!(popup.adoption, PopupAdoption::ClosedBeforeAdoption);
            assert!(popup.closed);
            assert_eq!(popup.console.len(), 1);
            assert_eq!(popup.console[0].page_id.as_deref(), Some("popup"));
            assert_eq!(popup.requests.len(), 1);
            assert_eq!(popup.requests[0].page_id.as_deref(), Some("popup"));
            assert!(matches!(
                popup.requests[0].completion,
                crate::RequestCompletion::Unavailable(_)
            ));
            assert_eq!(context.console_messages().len(), 1);
            let mut closed = 0;
            let mut console = 0;
            while let Ok(event) = events.try_recv() {
                match event.kind() {
                    crate::ContextEventKind::PageClose => closed += 1,
                    crate::ContextEventKind::Console => console += 1,
                    _ => {}
                }
            }
            assert_eq!((console, closed), (1, 1));
            let call = |method: &'static str| {
                let backend = backend.clone();
                async move {
                    match backend {
                        crate::browser::Backend::Cdp(conn) => conn
                            .call(None, method, json!({}), std::time::Duration::from_secs(2))
                            .await
                            .unwrap(),
                        crate::browser::Backend::Bidi { conn, .. } => conn
                            .call(method, json!({}), std::time::Duration::from_secs(2))
                            .await
                            .unwrap(),
                    }
                }
            };
            call("test.liveBurst").await;
            let failed = captures.get("failed").unwrap();
            let result = match &backend {
                crate::browser::Backend::Cdp(conn) => crate::driver::CdpDriver::spawn(
                    conn.clone(),
                    "failed-session".into(),
                    "failed".into(),
                    std::time::Duration::from_secs(2),
                    failed.sink(),
                    Some("owner".into()),
                )
                .await
                .map(|_| ()),
                crate::browser::Backend::Bidi { conn, .. } => crate::driver::BidiDriver::spawn(
                    conn.clone(),
                    "failed".into(),
                    std::time::Duration::from_secs(2),
                    false,
                    failed.sink(),
                    Some("owner".into()),
                )
                .await
                .map(|_| ()),
            };
            let error = result.unwrap_err().to_string();
            assert!(error.contains("injected popup initialization failure"));
            failed.failed(&error);
            let failed_weak = Arc::downgrade(&failed);
            drop(failed);
            call("test.closeBurst").await;
            let history = context.popup_diagnostics();
            let failed = history
                .entries
                .iter()
                .find(|entry| entry.page_id == "failed")
                .unwrap();
            assert_eq!(failed.adoption, PopupAdoption::Failed);
            assert!(failed.closed);
            assert_eq!(
                failed.console.len(),
                2,
                "ingress must survive the failed driver listener"
            );
            assert!(failed.console[1].text.contains("late-failed"));
            assert!(matches!(
                failed.requests[0].completion,
                crate::RequestCompletion::Unavailable(_)
            ));
            tokio::time::timeout(std::time::Duration::from_secs(2), async {
                while failed_weak.upgrade().is_some() {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect(
                "failed initialization must release its listener and capture after native closure",
            );

            let weak = Arc::downgrade(&captures.get("popup").unwrap());
            captures.discard_closed("popup");
            assert!(weak.upgrade().is_none());
            context.clear_popup_diagnostics();
            let resumed = Arc::new(Mutex::new(Vec::<String>::new()));
            if !bidi {
                let resumed = resumed.clone();
                captures.set_resume(Arc::new(move |session| {
                    resumed.lock().unwrap().push(session.into())
                }));
            }
            for index in 0..MAX_POPUP_HISTORY {
                captures.create(
                    &format!("bounded-{index}"),
                    "root",
                    Some(format!("bounded-session-{index}")),
                    !bidi,
                );
            }
            if bidi {
                captures.bidi(
                    &crate::bidi::BidiEvent {
                        method: "script.realmCreated".into(),
                        params: json!({"context":"bounded-0"}),
                    },
                    MAX_STARTUP_BYTES + 1,
                );
            } else {
                captures.cdp(
                    &crate::cdp::CdpEvent {
                        session: Some("bounded-session-0".into()),
                        method: "Page.screencastFrame".into(),
                        params: json!({}),
                    },
                    MAX_STARTUP_BYTES + 1,
                );
            }
            assert!(
                !context.popup_diagnostics().entries[0].truncated,
                "unrelated native traffic must not spend the startup budget"
            );
            let owner2 = BrowserContext::new(
                backend.clone(),
                Some("owner2".into()),
                Default::default(),
                std::time::Duration::ZERO,
                std::time::Duration::from_secs(2),
                None,
                Arc::downgrade(&contexts),
                None,
            );
            contexts.lock().unwrap().push(owner2.clone());
            let _root2 = owner2.new_page().await.unwrap();
            let (page_loss, context_loss, ()) = tokio::join!(
                _root.wait_for_popup(std::time::Duration::ZERO),
                context.wait_for_event(crate::ContextEventKind::Page, std::time::Duration::ZERO),
                async {
                    tokio::task::yield_now().await;
                    captures.create("other-owner", "root2", Some("other-session".into()), !bidi);
                }
            );
            assert!(
                matches!(page_loss, Err(crate::E2eError::Config(message)) if message.contains("observation limit"))
            );
            assert!(
                matches!(context_loss, Err(crate::E2eError::Config(message)) if message.contains("observation limit"))
            );
            // A new wait is a retry with a fresh observation baseline.
            assert!(tokio::time::timeout(
                std::time::Duration::from_millis(10),
                _root.wait_for_popup(std::time::Duration::ZERO)
            )
            .await
            .is_err());
            let (page_lag, context_lag, ()) = tokio::join!(
                _root.wait_for_event(crate::PageEventKind::Request, std::time::Duration::ZERO),
                context.wait_for_event(crate::ContextEventKind::Request, std::time::Duration::ZERO),
                async {
                    tokio::task::yield_now().await;
                    for _ in 0..600 {
                        _root.emit(crate::PageEvent::Console(
                            serde_json::from_value(json!({"kind":"log","text":"buffer flood"}))
                                .unwrap(),
                        ));
                    }
                }
            );
            assert!(
                matches!(page_lag, Err(crate::E2eError::Config(message)) if message.contains("lost"))
            );
            assert!(
                matches!(context_lag, Err(crate::E2eError::Config(message)) if message.contains("lost"))
            );
            assert!(captures.get("bounded-0").is_none());
            let recovery = captures.recovery();
            assert_eq!(recovery.len(), MAX_POPUP_HISTORY);
            assert!(!recovery.iter().any(|(id, _, _)| id == "bounded-0"));
            assert_eq!(recovery.first().unwrap().0, "bounded-1");
            if !bidi {
                assert!(resumed
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|id| id == "bounded-session-0"));
            }
            let history = context.popup_diagnostics();
            assert_eq!(history.entries[0].adoption, PopupAdoption::Pending);
            assert!(history.entries[0].truncated);
            let recovered = captures
                .for_adoption("bounded-0", "root", Some("bounded-session-0".into()), !bidi)
                .unwrap();
            recovered.adopted();
            captures.adopted("bounded-0");
            assert_eq!(
                context.popup_diagnostics().entries[0].adoption,
                PopupAdoption::Adopted
            );
            assert_eq!(
                context.popup_diagnostics().entries.len(),
                MAX_POPUP_HISTORY,
                "recovery must not duplicate history"
            );
            assert!(captures.get("bounded-1").is_none());
            while let Ok(_) | Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) =
                events.try_recv()
            {}
            captures.close_native("bounded-1");
            captures.close_native("bounded-1");
            let closed = captures
                .for_adoption("bounded-1", "root", None, !bidi)
                .unwrap();
            assert!(closed.sink.native_closed());
            assert_eq!(
                closed.record.lock().unwrap().metadata.adoption,
                PopupAdoption::ClosedBeforeAdoption
            );
            let mut close_count = 0;
            while let Ok(event) = events.try_recv() {
                if event.kind() == crate::ContextEventKind::PageClose {
                    close_count += 1;
                }
            }
            assert_eq!(close_count, 1, "evicted native closure forwards only once");
            drop(closed);
            drop(recovered);
            let weak = Arc::downgrade(&captures.get("bounded-3").unwrap());
            let mut network = _root.subscribe_network();
            let (context_loss, network_loss, frame_loss, ()) = tokio::join!(
                context.wait_for_event(crate::ContextEventKind::Request, std::time::Duration::ZERO),
                network.recv(),
                _root.wait_for_event(
                    crate::PageEventKind::FrameAttached,
                    std::time::Duration::ZERO
                ),
                async {
                    tokio::task::yield_now().await;
                    _root
                        .sink
                        .mark_native_observation_lost("injected native listener exit");
                }
            );
            assert!(
                matches!(context_loss, Err(crate::E2eError::Config(message)) if message.contains("source unavailable"))
            );
            assert!(
                matches!(network_loss, Err(crate::E2eError::Config(message)) if message.contains("source unavailable"))
            );
            assert!(
                matches!(frame_loss, Err(crate::E2eError::Config(message)) if message.contains("source unavailable"))
            );
            assert!(tokio::time::timeout(
                std::time::Duration::from_millis(10),
                context.wait_for_event(crate::ContextEventKind::Request, std::time::Duration::ZERO)
            )
            .await
            .is_err());
            assert!(
                matches!(_root.wait_for_event(crate::PageEventKind::Request, std::time::Duration::ZERO).await, Err(crate::E2eError::Config(message)) if message.contains("source unavailable"))
            );
            assert!(
                matches!(_root.wait_for_event(crate::PageEventKind::FrameAttached, std::time::Duration::ZERO).await, Err(crate::E2eError::Config(message)) if message.contains("source unavailable"))
            );
            let mut network = _root.subscribe_network();
            let result =
                tokio::time::timeout(std::time::Duration::from_millis(100), network.recv())
                    .await
                    .expect(
                        "lost native source must settle a disabled-timeout network subscription",
                    );
            assert!(
                matches!(result, Err(crate::E2eError::Config(message)) if message.contains("source unavailable"))
            );

            let matcher = crate::UrlMatcher::glob("**").unwrap();
            let options = crate::OperationOptions {
                timeout: Some(std::time::Duration::ZERO),
                cancellation: None,
            };
            assert!(
                matches!(_root.wait_for_request_handle(&matcher, options.clone()).await, Err(crate::E2eError::Config(message)) if message.contains("source unavailable"))
            );
            assert!(
                matches!(_root.wait_for_response_handle(&matcher, options).await, Err(crate::E2eError::Config(message)) if message.contains("source unavailable"))
            );
            context.close().await.unwrap();
            assert!(
                weak.upgrade().is_none(),
                "context close must release unadopted native capture"
            );
            owner2.close().await.unwrap();
            match backend {
                crate::browser::Backend::Cdp(conn) => conn.close(),
                crate::browser::Backend::Bidi { conn, .. } => conn.close(),
            };
            server.abort();
        }
    }
}

/// Owned startup observations. No live Page, context or remote handles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PopupDiagnostics {
    pub page_id: String,
    pub opener_id: String,
    pub adoption: PopupAdoption,
    /// True only after a native destruction event, not an initialization error.
    pub closed: bool,
    pub error: Option<String>,
    pub truncated: bool,
    pub dropped_events: u64,
    /// A later main-document commit was observed. The projection stops growing
    /// once this is true and adoption has completed.
    pub initial_document_replaced: bool,
    pub console: Vec<ConsoleMessage>,
    /// Requests started before adoption or in the initial document. Completion
    /// can update after adoption and document replacement.
    pub requests: Vec<RequestSnapshot>,
}

/// Bounded context history, including failed and immediately closed popups.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PopupDiagnosticsHistory {
    pub entries: Vec<PopupDiagnostics>,
    pub dropped_popups: u64,
}

pub(crate) struct PopupRecord {
    pub metadata: PopupDiagnostics,
    requests: Vec<Arc<crate::network::RequestState>>,
    events: usize,
    bytes: usize,
    accept_current: bool,
    first_main_commit: bool,
    document: Option<String>,
}
impl PopupRecord {
    pub(crate) fn console(&mut self, message: ConsoleMessage) {
        if !self.window_ended() && self.accept_current {
            self.metadata.console.push(message);
        }
    }
    pub(crate) fn request(&mut self, state: Arc<crate::network::RequestState>) {
        if !self.window_ended() && self.accept_current {
            self.requests.push(state);
        }
    }
    fn snapshot(&self) -> PopupDiagnostics {
        let mut metadata = self.metadata.clone();
        metadata.requests = self
            .requests
            .iter()
            .map(|request| request.snapshot())
            .collect();
        metadata
    }
    fn allow(&mut self, bytes: usize) -> bool {
        self.accept_current = false;
        if self.window_ended() {
            return true;
        }
        if self.events >= MAX_STARTUP_EVENTS || bytes > MAX_STARTUP_BYTES.saturating_sub(self.bytes)
        {
            self.metadata.truncated = true;
            self.metadata.dropped_events = self.metadata.dropped_events.saturating_add(1);
            return self.metadata.adoption == PopupAdoption::Adopted;
        }
        self.events += 1;
        self.bytes += bytes;
        self.accept_current = true;
        true
    }
    fn window_ended(&self) -> bool {
        self.metadata.initial_document_replaced && self.metadata.adoption == PopupAdoption::Adopted
    }
    fn document(&mut self, id: Option<&str>, url: Option<&str>) {
        if url.is_none_or(|url| url == "about:blank") {
            return;
        }
        if self.first_main_commit {
            if id.is_none() || self.document.as_deref() != id {
                self.metadata.initial_document_replaced = true;
            }
        } else {
            self.first_main_commit = true;
            self.document = id.map(str::to_owned);
        }
    }
}

#[derive(Default)]
pub(crate) struct PopupHistory {
    records: VecDeque<Arc<Mutex<PopupRecord>>>,
    dropped: u64,
}
impl PopupHistory {
    fn get(&self, id: &str) -> Option<Arc<Mutex<PopupRecord>>> {
        self.records
            .iter()
            .find(|record| {
                record
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .metadata
                    .page_id
                    == id
            })
            .cloned()
    }
    fn insert(&mut self, record: Arc<Mutex<PopupRecord>>) {
        self.records.push_back(record);
        while self.records.len() > MAX_POPUP_HISTORY {
            self.records.pop_front();
            self.dropped = self.dropped.saturating_add(1);
        }
    }
    pub(crate) fn snapshot(&self) -> PopupDiagnosticsHistory {
        PopupDiagnosticsHistory {
            entries: self
                .records
                .iter()
                .map(|record| record.lock().unwrap_or_else(|e| e.into_inner()).snapshot())
                .collect(),
            dropped_popups: self.dropped,
        }
    }
    pub(crate) fn clear(&mut self) {
        self.records.clear();
        self.dropped = 0;
    }
}

pub(crate) struct PopupCapture {
    pub(crate) sink: ConsoleSink,
    context_id: Option<String>,
    session: Option<String>,
    page_id: String,
    pub(crate) owner_cancel: crate::CancellationToken,
    record: Arc<Mutex<PopupRecord>>,
    lifecycle: Mutex<Option<crate::CancellationToken>>,
}
impl PopupCapture {
    pub(crate) fn sink(self: &Arc<Self>) -> ConsoleSink {
        let mut sink = self.sink.clone();
        sink.popup_capture = Some(self.clone());
        sink
    }
    pub(crate) fn bind_lifecycle(&self, token: crate::CancellationToken) {
        if self.owner_cancel.is_cancelled() || self.sink.native_closed() {
            token.cancel_with_reason("popup owner closed before adoption");
        }
        *self.lifecycle.lock().unwrap_or_else(|e| e.into_inner()) = Some(token);
    }
    pub(crate) fn adopted(&self) {
        let mut record = self.record.lock().unwrap_or_else(|e| e.into_inner());
        if !record.metadata.closed {
            record.metadata.adoption = PopupAdoption::Adopted;
        }
    }
    pub(crate) fn failed(&self, reason: &str) {
        let mut record = self.record.lock().unwrap_or_else(|e| e.into_inner());
        if !record.metadata.closed
            && matches!(
                record.metadata.adoption,
                PopupAdoption::Pending | PopupAdoption::Failed
            )
        {
            record.metadata.adoption = PopupAdoption::Failed;
            record.metadata.error = Some(reason.into());
        }
    }
    fn stop(&self, reason: &str, cancelled: bool) {
        if cancelled {
            let mut record = self.record.lock().unwrap_or_else(|e| e.into_inner());
            if record.metadata.adoption == PopupAdoption::Pending && !record.metadata.closed {
                record.metadata.adoption = PopupAdoption::Cancelled;
                record.metadata.error = Some(reason.into());
            }
        }
        self.sink.stop_popup_observation(reason);
        if let Some(token) = self
            .lifecycle
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
        {
            token.cancel_with_reason(reason);
        }
    }
    fn closed(&self) {
        {
            let mut record = self.record.lock().unwrap_or_else(|e| e.into_inner());
            record.metadata.closed = true;
            if record.metadata.adoption == PopupAdoption::Pending {
                record.metadata.adoption = PopupAdoption::ClosedBeforeAdoption;
            }
        }
        self.sink.emit(crate::PageEvent::Closed);
        self.stop("popup destroyed", false);
    }
    fn allow(&self, bytes: usize) -> bool {
        let allowed = self
            .record
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .allow(bytes);
        if !allowed {
            self.sink
                .close_network("popup startup capture exceeded its native event budget");
        }
        allowed
    }
    pub(crate) fn seed_document(&self, id: Option<&str>, url: Option<&str>) {
        self.record
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .document(id, url);
    }
}

struct Entry {
    pending: Option<Arc<PopupCapture>>,
    capture: Weak<PopupCapture>,
    order: u64,
}
/// Transport ingress owns pending captures; adopted captures have weak ownership.
type NativeResume = Arc<dyn Fn(&str) + Send + Sync>;

pub(crate) struct PopupCaptures {
    contexts: Weak<Mutex<Vec<BrowserContext>>>,
    entries: Mutex<HashMap<String, Entry>>,
    next: std::sync::atomic::AtomicU64,
    resume: Mutex<Option<NativeResume>>,
}
impl PopupCaptures {
    pub(crate) fn new(contexts: Weak<Mutex<Vec<BrowserContext>>>) -> Self {
        Self {
            contexts,
            entries: Mutex::new(HashMap::new()),
            next: std::sync::atomic::AtomicU64::new(0),
            resume: Mutex::new(None),
        }
    }
    fn observation_lost(&self, opener: &str, reason: &str) {
        if let Some((context, page)) = crate::browser::find_owner(&self.contexts, opener) {
            context.popup_observation_lost(reason);
            page.sink.popup_observation_lost(reason);
        }
    }
    pub(crate) fn set_resume(&self, resume: NativeResume) {
        *self.resume.lock().unwrap_or_else(|e| e.into_inner()) = Some(resume);
    }
    fn resume(&self, session: &str) {
        let resume = self
            .resume
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if let Some(resume) = resume {
            resume(session);
        }
    }
    /// Retained pending attachments can reconstruct missed adoption notifications.
    /// The snapshot has at most MAX_POPUP_HISTORY entries and no strong owners.
    pub(crate) fn recovery(&self) -> Vec<(String, String, Option<String>)> {
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let mut pending: Vec<_> = entries
            .values()
            .filter_map(|entry| {
                let capture = entry.pending.as_ref()?;
                let record = capture.record.lock().unwrap_or_else(|e| e.into_inner());
                (record.metadata.adoption == PopupAdoption::Pending && !record.metadata.closed)
                    .then(|| {
                        (
                            entry.order,
                            capture.page_id.clone(),
                            record.metadata.opener_id.clone(),
                            capture.session.clone(),
                        )
                    })
            })
            .collect();
        pending.sort_by_key(|entry| entry.0);
        pending
            .into_iter()
            .map(|(_, id, opener, session)| (id, opener, session))
            .collect()
    }
    fn create(&self, id: &str, opener: &str, session: Option<String>, cdp: bool) {
        if id.is_empty() || opener.is_empty() || self.get(id).is_some() {
            return;
        }
        let owner = crate::browser::find_owner(&self.contexts, opener)
            .map(|(context, _)| context)
            .or_else(|| {
                let capture = self.get(opener)?;
                let contexts = self.contexts.upgrade()?;
                let contexts = contexts.lock().unwrap_or_else(|e| e.into_inner());
                contexts
                    .iter()
                    .find(|context| context.id() == capture.context_id.as_deref())
                    .cloned()
            });
        let Some(owner) = owner.filter(|owner| !owner.is_closed()) else {
            return;
        };
        // A queued adopter can outlive the bounded ingress observation slot.
        // Reuse its owned history so actual adoption can settle independently
        // of capture loss, rather than retaining an invented setup failure.
        let previous = owner
            .popup_history
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id);
        let fresh = previous.is_none();
        let record = previous.unwrap_or_else(|| {
            Arc::new(Mutex::new(PopupRecord {
                metadata: PopupDiagnostics {
                    page_id: id.into(),
                    opener_id: opener.into(),
                    adoption: PopupAdoption::Pending,
                    closed: false,
                    error: None,
                    truncated: false,
                    dropped_events: 0,
                    initial_document_replaced: false,
                    console: Vec::new(),
                    requests: Vec::new(),
                },
                requests: Vec::new(),
                events: 0,
                bytes: 0,
                accept_current: false,
                first_main_commit: false,
                document: None,
            }))
        });
        let mut sink = ConsoleSink::new();
        sink.popup_diagnostics = Some(Arc::downgrade(&record));
        sink.seed_popup(id, cdp);
        if record
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .metadata
            .closed
        {
            sink.seed_popup_closed();
        }
        owner.bind_popup_sink(&sink, id);
        if fresh {
            owner
                .popup_history
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(record.clone());
        }
        let capture = Arc::new(PopupCapture {
            sink,
            context_id: owner.id().map(str::to_owned),
            session,
            page_id: id.into(),
            owner_cancel: owner.lifecycle_cancellation(),
            record,
            lifecycle: Mutex::new(None),
        });
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        entries.retain(|_, entry| entry.capture.strong_count() != 0);
        if entries
            .values()
            .filter(|entry| entry.pending.is_some())
            .count()
            >= MAX_POPUP_HISTORY
        {
            if let Some(oldest) = entries
                .iter()
                .filter(|(_, entry)| entry.pending.is_some())
                .min_by_key(|(_, entry)| entry.order)
                .map(|(id, _)| id.clone())
            {
                if let Some(entry) = entries.remove(&oldest) {
                    if let Some(capture) = entry.capture.upgrade() {
                        {
                            let mut record =
                                capture.record.lock().unwrap_or_else(|e| e.into_inner());
                            record.metadata.truncated = true;
                            record.metadata.error = Some("pending popup observation limit exceeded; initial observations unavailable".into());
                        }
                        let opener = capture
                            .record
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .metadata
                            .opener_id
                            .clone();
                        self.observation_lost(&opener, "pending popup observation limit exceeded; adoption notifications may be lost");
                        capture.stop("pending popup capture limit exceeded", false);
                        // Eviction must not strand a paused native target. Its eventual
                        // adopter can still recover owned history and report actual success.
                        if let Some(session) = capture.session.as_deref() {
                            self.resume(session);
                        }
                    }
                }
            }
        }
        entries.insert(
            id.into(),
            Entry {
                capture: Arc::downgrade(&capture),
                pending: Some(capture),
                order: self.next.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            },
        );
    }
    pub(crate) fn get(&self, id: &str) -> Option<Arc<PopupCapture>> {
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .and_then(|entry| entry.capture.upgrade())
    }
    pub(crate) fn for_adoption(
        &self,
        id: &str,
        opener: &str,
        session: Option<String>,
        cdp: bool,
    ) -> Option<Arc<PopupCapture>> {
        if let Some(capture) = self.get(id) {
            return Some(capture);
        }
        self.create(id, opener, session, cdp);
        self.get(id)
    }
    pub(crate) fn adopted(&self, id: &str) {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = entries.get_mut(id) {
            entry.pending = None;
        }
    }
    pub(crate) fn discard_closed(&self, id: &str) {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        if entries
            .get(id)
            .and_then(|entry| entry.capture.upgrade())
            .is_some_and(|capture| capture.sink.native_closed())
        {
            entries.remove(id);
        }
    }
    pub(crate) fn release_context(&self, id: Option<&str>) {
        let removed = {
            let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
            let ids: Vec<_> = entries
                .iter()
                .filter(|(_, entry)| {
                    entry
                        .capture
                        .upgrade()
                        .is_some_and(|capture| capture.context_id.as_deref() == id)
                })
                .map(|(id, _)| id.clone())
                .collect();
            ids.into_iter()
                .filter_map(|id| entries.remove(&id))
                .collect::<Vec<_>>()
        };
        for entry in removed {
            if let Some(capture) = entry.capture.upgrade() {
                capture.stop("browser context closed", true);
            }
        }
    }
    pub(crate) fn disconnect(&self, reason: &str) {
        let entries = std::mem::take(&mut *self.entries.lock().unwrap_or_else(|e| e.into_inner()));
        for entry in entries.into_values() {
            if let Some(capture) = entry.capture.upgrade() {
                capture.failed(reason);
                // Transport cancellation settles the driver itself; do not
                // turn a disconnected operation into page cancellation.
                capture.sink.stop_popup_observation(reason);
            }
        }
    }
    fn close_native(&self, id: &str) {
        // Native destruction is authoritative even if the adoption pump lags.
        if let Some((_, page)) = crate::browser::find_owner(&self.contexts, id) {
            page.mark_closed();
        }
        let Some(capture) = self.get(id) else {
            // An observation slot may have been evicted while its owned
            // diagnostics remain. Destruction is still an authoritative fact.
            let contexts = self
                .contexts
                .upgrade()
                .map(|contexts| contexts.lock().unwrap_or_else(|e| e.into_inner()).clone())
                .unwrap_or_default();
            for owner in contexts {
                let record = owner
                    .popup_history
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .get(id);
                if let Some(record) = record {
                    let mut record = record.lock().unwrap_or_else(|e| e.into_inner());
                    if record.metadata.closed {
                        continue;
                    }
                    record.metadata.closed = true;
                    if record.metadata.adoption == PopupAdoption::Pending {
                        record.metadata.adoption = PopupAdoption::ClosedBeforeAdoption;
                    }
                    drop(record);
                    let sink = ConsoleSink::new();
                    owner.bind_popup_sink(&sink, id);
                    sink.emit(crate::PageEvent::Closed);
                }
            }
            return;
        };
        capture.closed();
        let terminal = matches!(
            capture
                .record
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .metadata
                .adoption,
            PopupAdoption::Adopted | PopupAdoption::Failed | PopupAdoption::Cancelled
        );
        if terminal {
            self.entries
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(id);
        }
    }
    pub(crate) fn cdp(&self, event: &crate::cdp::CdpEvent, bytes: usize) {
        if event.method == "Target.attachedToTarget"
            && event.session.is_none()
            && event.params["targetInfo"]["type"] == "page"
        {
            if let (Some(id), Some(opener), Some(session)) = (
                event.params["targetInfo"]["targetId"].as_str(),
                event.params["targetInfo"]["openerId"].as_str(),
                event.params["sessionId"].as_str(),
            ) {
                self.create(id, opener, Some(session.into()), true);
                if self.get(id).is_none() {
                    self.resume(session);
                }
            }
        }
        if event.method == "Target.attachedToTarget"
            && event.session.is_none()
            && event.params["targetInfo"]["openerId"].as_str().is_none()
        {
            if let Some(session) = event.params["sessionId"].as_str() {
                self.resume(session);
            }
        }
        if event.method == "Target.targetDestroyed" {
            if let Some(id) = event.params["targetId"].as_str() {
                self.close_native(id);
            }
            return;
        }
        // Execution-world caches, screencast frames and other unrelated native
        // traffic are handled by their own listeners, outside this budget.
        if !event.method.starts_with("Network.")
            && !matches!(
                event.method.as_str(),
                "Runtime.consoleAPICalled"
                    | "Runtime.exceptionThrown"
                    | "Log.entryAdded"
                    | "Fetch.requestPaused"
                    | "Page.frameAttached"
                    | "Page.frameNavigated"
                    | "Page.navigatedWithinDocument"
                    | "Page.frameDetached"
                    | "Page.domContentEventFired"
                    | "Page.loadEventFired"
                    | "Page.javascriptDialogClosed"
            )
        {
            return;
        }
        let captures: Vec<_> = self
            .entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .filter_map(|entry| entry.capture.upgrade())
            .collect();
        for capture in captures {
            if event.session.as_deref() == capture.session.as_deref()
                && event.method == "Page.frameNavigated"
                && event.params["frame"]["parentId"].is_null()
            {
                capture.seed_document(
                    event.params["frame"]["loaderId"].as_str(),
                    event.params["frame"]["url"].as_str(),
                );
            }
            if event.session.as_deref() == capture.session.as_deref()
                && event.session.is_some()
                && !capture.sink.native_closed()
                && capture.allow(bytes)
            {
                capture.sink.observe_popup_cdp(event);
            }
        }
    }
    pub(crate) fn bidi(&self, event: &crate::bidi::BidiEvent, bytes: usize) {
        if event.method == "browsingContext.contextCreated" && event.params["parent"].is_null() {
            if let (Some(id), Some(opener)) = (
                event.params["context"].as_str(),
                event.params["originalOpener"].as_str(),
            ) {
                self.create(id, opener, None, false);
            }
        }
        if !event.method.starts_with("network.")
            && !matches!(
                event.method.as_str(),
                "log.entryAdded"
                    | "browsingContext.contextCreated"
                    | "browsingContext.contextDestroyed"
                    | "browsingContext.navigationCommitted"
                    | "browsingContext.fragmentNavigated"
                    | "browsingContext.historyUpdated"
                    | "browsingContext.domContentLoaded"
                    | "browsingContext.load"
                    | "browsingContext.userPromptClosed"
            )
        {
            return;
        }
        let captures: Vec<_> = self
            .entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .filter_map(|entry| entry.capture.upgrade())
            .collect();
        for capture in captures {
            if event.method == "browsingContext.navigationCommitted"
                && event.context() == Some(&capture.page_id)
            {
                capture.seed_document(
                    event.params["navigation"].as_str(),
                    event.params["url"].as_str(),
                );
            }
            let belongs = if event.method == "browsingContext.contextCreated" {
                event.params["parent"]
                    .as_str()
                    .is_some_and(|parent| capture.sink.contains_frame(parent))
            } else {
                event
                    .context()
                    .is_some_and(|id| id == capture.page_id || capture.sink.contains_frame(id))
            };
            if !belongs {
                continue;
            }
            if event.method == "browsingContext.contextDestroyed"
                && event.context() == Some(&capture.page_id)
            {
                self.close_native(&capture.page_id);
            } else if !capture.sink.native_closed() && capture.allow(bytes) {
                capture.sink.observe_popup_bidi(event);
            }
        }
    }
}
