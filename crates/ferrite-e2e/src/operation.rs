//! Shared deadlines and cooperative cancellation for browser and HTTP work.
use crate::{E2eError, E2eResult};
use std::{future::Future, sync::Arc, time::Duration};

/// A reusable cancellation signal. Clones observe the same reason, including
/// cancellation sent before an operation starts. Canceling twice keeps the first reason.
#[derive(Clone, Debug)]
pub struct CancellationToken {
    signal: Arc<tokio::sync::watch::Sender<Option<String>>>,
}
impl Default for CancellationToken {
    fn default() -> Self {
        Self {
            signal: Arc::new(tokio::sync::watch::channel(None).0),
        }
    }
}
impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.cancel_with_reason("operation canceled");
    }
    pub fn cancel_with_reason(&self, reason: impl Into<String>) {
        let reason = reason.into();
        self.signal.send_if_modified(|value| {
            if value.is_some() {
                false
            } else {
                *value = Some(reason);
                true
            }
        });
    }
    pub fn reason(&self) -> Option<String> {
        self.signal.borrow().clone()
    }
    pub fn is_cancelled(&self) -> bool {
        self.reason().is_some()
    }
    pub async fn cancelled(&self) -> String {
        let mut receiver = self.signal.subscribe();
        let reason = receiver
            .wait_for(|value| value.is_some())
            .await
            .expect("cancellation sender lives with the token")
            .clone()
            .unwrap();
        reason
    }
    pub(crate) fn check(&self) -> E2eResult<()> {
        match self.reason() {
            Some(reason) => Err(E2eError::Cancelled(reason)),
            None => Ok(()),
        }
    }
    pub fn run<'a, T: 'a>(
        &'a self,
        future: impl Future<Output = E2eResult<T>> + 'a,
    ) -> impl Future<Output = E2eResult<T>> + 'a {
        // Box before constructing the returned future: boxing inside async fn
        // leaves its generic input inline until the first poll.
        let future = Box::pin(future);
        async move {
            self.check()?;
            tokio::select! {biased;
                reason=self.cancelled()=>Err(E2eError::Cancelled(reason)),
                result=future=>result,
            }
        }
    }
}

/// Overrides for a wait/evaluation. `None` uses its default; zero disables timeout.
#[derive(Debug, Clone, Default)]
pub struct OperationOptions {
    pub timeout: Option<Duration>,
    pub cancellation: Option<CancellationToken>,
}

/// Own one disposal operation independently of any caller waiting for it.
/// Native close failures are replayed to concurrent/repeated callers. Closing
/// is not an ordinary cancellable action: dropping a wait must not drop work
/// that already marked a native resource closed.
#[derive(Clone)]
pub(crate) struct SharedClose {
    started: Arc<std::sync::atomic::AtomicBool>,
    result: Arc<tokio::sync::watch::Sender<Option<Result<(), CloseFailure>>>>,
}

#[derive(Clone)]
enum CloseFailure {
    Timeout(u64, String),
    Disconnected(String),
    Cancelled(String),
    Cdp { method: String, message: String },
    Other(String),
}
impl CloseFailure {
    fn capture(error: E2eError) -> Self {
        match error {
            E2eError::Timeout(ms, detail) => Self::Timeout(ms, detail),
            E2eError::Disconnected(detail) => Self::Disconnected(detail),
            E2eError::Cancelled(detail) => Self::Cancelled(detail),
            E2eError::Cdp { method, message } => Self::Cdp { method, message },
            E2eError::Config(message) => Self::Other(message),
            error => Self::Other(error.to_string()),
        }
    }
    fn error(self) -> E2eError {
        match self {
            Self::Timeout(ms, detail) => E2eError::Timeout(ms, detail),
            Self::Disconnected(detail) => E2eError::Disconnected(detail),
            Self::Cancelled(detail) => E2eError::Cancelled(detail),
            Self::Cdp { method, message } => E2eError::Cdp { method, message },
            Self::Other(detail) => E2eError::Config(detail),
        }
    }
}
impl Default for SharedClose {
    fn default() -> Self {
        Self {
            started: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            result: Arc::new(tokio::sync::watch::channel(None).0),
        }
    }
}
impl SharedClose {
    pub(crate) fn run<'a>(
        &'a self,
        future: impl Future<Output = E2eResult<()>> + Send + 'static,
    ) -> impl Future<Output = E2eResult<()>> + 'a {
        // Keep resource owners out of every enclosing runner future's layout,
        // including the initial state before its first poll.
        let future = Box::pin(future);
        async move {
            if !self.started.swap(true, std::sync::atomic::Ordering::AcqRel) {
                let result = self.result.clone();
                tokio::spawn(async move {
                    use futures::FutureExt;
                    let outcome = match std::panic::AssertUnwindSafe(future).catch_unwind().await {
                        Ok(outcome) => outcome.map_err(CloseFailure::capture),
                        Err(_) => Err(CloseFailure::Other("native disposal panicked".into())),
                    };
                    result.send_replace(Some(outcome));
                });
            }
            let mut receiver = self.result.subscribe();
            let outcome = receiver
                .wait_for(|value| value.is_some())
                .await
                .expect("disposal sender lives with every waiting handle")
                .as_ref()
                .unwrap()
                .clone();
            outcome.map_err(CloseFailure::error)
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Deadline {
    end: Option<tokio::time::Instant>,
    timeout: Duration,
    ready_cleanup: bool,
}
impl Deadline {
    pub(crate) fn new(timeout: Duration) -> Self {
        Self {
            end: if timeout.is_zero() {
                None
            } else {
                tokio::time::Instant::now().checked_add(timeout)
            },
            timeout,
            ready_cleanup: false,
        }
    }
    /// One enclosing cleanup clock. Even after exhaustion, remaining operations
    /// get one poll to release immediately ready resources. Pending work is
    /// reported as timed out without giving it another asynchronous budget.
    pub(crate) fn cleanup(timeout: Duration) -> Self {
        Self {
            ready_cleanup: true,
            ..Self::new(timeout)
        }
    }
    /// A local override may shorten, but never renew or extend, the enclosing
    /// deadline. Zero disables only the local limit.
    pub(crate) fn with_limit(self, timeout: Option<Duration>) -> Self {
        let Some(timeout) = timeout else {
            return self;
        };
        let local = Self::new(timeout);
        match (self.end, local.end) {
            (_, None) => self,
            (Some(outer), Some(inner)) if outer <= inner => self,
            _ => Self {
                ready_cleanup: self.ready_cleanup,
                ..local
            },
        }
    }
    pub(crate) fn expired(self) -> bool {
        self.end
            .is_some_and(|end| tokio::time::Instant::now() >= end)
    }
    pub(crate) async fn elapsed(self) {
        match self.end {
            Some(end) => tokio::time::sleep_until(end).await,
            None => std::future::pending().await,
        }
    }
    pub(crate) fn run<'a, T: 'a>(
        self,
        description: impl Into<String>,
        future: impl Future<Output = E2eResult<T>> + 'a,
    ) -> impl Future<Output = E2eResult<T>> + 'a {
        let future = Box::pin(future);
        let description = description.into();
        async move {
            if self.ready_cleanup {
                return tokio::select! { biased;
                    result=future=>result,
                    ()=self.elapsed()=>Err(E2eError::Timeout(self.timeout.as_millis().min(u128::from(u64::MAX)) as u64,description)),
                };
            }
            tokio::select! {
                result=future=>result,
                ()=self.elapsed()=>Err(E2eError::Timeout(self.timeout.as_millis().min(u128::from(u64::MAX)) as u64,description)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn shared_disposal_survives_dropped_wait_replays_errors_and_releases_ownership() {
        use futures::FutureExt;
        let close = SharedClose::default();
        let owner = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let weak = Arc::downgrade(&owner);
        let count = owner.clone();
        let (release, released) = tokio::sync::oneshot::channel();
        assert!(close
            .run(async move {
                released.await.unwrap();
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Err(E2eError::Cdp {
                    method: "native.close".into(),
                    message: "failure".into(),
                })
            })
            .now_or_never()
            .is_none());
        let unexpected = || async { panic!("disposal ran twice") };
        release.send(()).unwrap();
        let (first, second) = tokio::join!(close.run(unexpected()), close.run(unexpected()));
        for error in [first.unwrap_err(), second.unwrap_err()] {
            assert!(
                matches!(error, E2eError::Cdp { method, message } if method == "native.close" && message == "failure")
            );
        }
        assert_eq!(owner.load(std::sync::atomic::Ordering::SeqCst), 1);
        drop(owner);
        assert!(
            weak.upgrade().is_none(),
            "completed disposal must not retain its captured owner"
        );
        assert!(matches!(
            close.run(unexpected()).await,
            Err(E2eError::Cdp { .. })
        ));

        let panicking = SharedClose::default();
        let result = panicking.run(async { panic!("native failure") }).await;
        assert!(
            matches!(result, Err(E2eError::Config(message)) if message == "native disposal panicked")
        );
        assert!(panicking.run(async { Ok(()) }).await.is_err());
    }
    #[tokio::test]
    async fn cleanup_limit_is_shared_and_exhaustion_still_polls_ready_release() {
        let deadline = Deadline::cleanup(Duration::from_millis(20));
        let started = tokio::time::Instant::now();
        let result = deadline
            .with_limit(Some(Duration::ZERO))
            .run("first cleanup", std::future::pending::<E2eResult<()>>())
            .await;
        assert!(matches!(result, Err(E2eError::Timeout(20, _))));
        let polls = std::sync::atomic::AtomicUsize::new(0);
        for label in ["second cleanup", "third cleanup"] {
            let result = deadline
                .with_limit(Some(Duration::from_secs(1)))
                .run(
                    label,
                    std::future::poll_fn(|_| {
                        polls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        std::task::Poll::<E2eResult<()>>::Pending
                    }),
                )
                .await;
            assert!(matches!(result, Err(E2eError::Timeout(20, detail)) if detail == label));
        }
        assert_eq!(polls.load(std::sync::atomic::Ordering::SeqCst), 2);
        assert_eq!(
            deadline
                .run("ready dependency", async { Ok(42) })
                .await
                .unwrap(),
            42
        );
        assert!(started.elapsed() < Duration::from_millis(200));
    }

    #[tokio::test]
    async fn cleanup_operation_limits_shorten_without_consuming_the_outer_clock() {
        let outer = Deadline::cleanup(Duration::from_millis(100));
        let result = outer
            .with_limit(Some(Duration::from_millis(10)))
            .run("local", std::future::pending::<E2eResult<()>>())
            .await;
        assert!(matches!(result, Err(E2eError::Timeout(10, _))));
        assert!(!outer.expired());
        assert_eq!(outer.run("later", async { Ok(1) }).await.unwrap(), 1);
        let unlimited = Deadline::cleanup(Duration::ZERO);
        assert!(matches!(
            unlimited
                .with_limit(Some(Duration::from_millis(10)))
                .run("finite override", std::future::pending::<E2eResult<()>>())
                .await,
            Err(E2eError::Timeout(10, _))
        ));
    }
    #[tokio::test]
    async fn zero_disables_deadline_and_cancel_wakes_every_waiter() {
        assert_eq!(
            Deadline::new(Duration::ZERO)
                .run("zero", async {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    Ok(42)
                })
                .await
                .unwrap(),
            42
        );
        let token = CancellationToken::new();
        let first = token.clone();
        let second = token.clone();
        let a = tokio::spawn(async move { first.cancelled().await });
        let b = tokio::spawn(async move { second.cancelled().await });
        token.cancel_with_reason("stop");
        token.cancel_with_reason("later");
        assert_eq!(a.await.unwrap(), "stop");
        assert_eq!(b.await.unwrap(), "stop");
        assert!(matches!(
            token.run(async { Ok(()) }).await,
            Err(E2eError::Cancelled(_))
        ));
    }
}
