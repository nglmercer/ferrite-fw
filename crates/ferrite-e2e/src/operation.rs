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

#[derive(Clone, Copy)]
pub(crate) struct Deadline {
    end: Option<tokio::time::Instant>,
    timeout: Duration,
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
