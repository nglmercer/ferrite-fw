//! Confirm native disappearance rather than treating close acknowledgment as disposal.
use crate::{operation::Deadline, E2eResult};
use std::{future::Future, time::Duration};

pub(crate) async fn confirmed_close<C, Q, F>(
    close: C,
    mut absent: Q,
    timeout: Duration,
) -> E2eResult<()>
where
    C: Future<Output = E2eResult<()>>,
    Q: FnMut() -> F,
    F: Future<Output = E2eResult<bool>>,
{
    let budget = Deadline::cleanup(timeout);
    let mut close_error = budget.run("native close command", close).await.err();
    loop {
        match budget
            .run("confirm native target disappearance", absent())
            .await
        {
            Ok(true) => return Ok(()),
            Ok(false) => {
                if let Some(error) = close_error.take() {
                    return Err(error);
                }
            }
            Err(error) => return Err(close_error.take().unwrap_or(error)),
        }
        budget
            .run("wait for native target disappearance", async {
                tokio::time::sleep(Duration::from_millis(20)).await;
                Ok(())
            })
            .await?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::E2eError;
    #[tokio::test]
    async fn acknowledgments_require_disappearance_and_errors_remain_visible() {
        let mut observations = std::collections::VecDeque::from([false, false, true]);
        confirmed_close(
            async { Ok(()) },
            || std::future::ready(Ok(observations.pop_front().unwrap())),
            Duration::from_secs(1),
        )
        .await
        .unwrap();
        assert!(observations.is_empty());
        let rejected = || E2eError::Cdp {
            method: "close".into(),
            message: "rejected".into(),
        };
        assert!(
            matches!(confirmed_close(async { Err(rejected()) }, || std::future::ready(Ok(false)), Duration::from_secs(1)).await, Err(E2eError::Cdp { message, .. }) if message == "rejected")
        );
        // An authoritative absence also resolves a concurrent external close.
        confirmed_close(
            async { Err(rejected()) },
            || std::future::ready(Ok(true)),
            Duration::from_secs(1),
        )
        .await
        .unwrap();
        assert!(matches!(
            confirmed_close(
                async { Ok(()) },
                || std::future::ready(Err(E2eError::Disconnected("lost query".into()))),
                Duration::from_secs(1)
            )
            .await,
            Err(E2eError::Disconnected(_))
        ));
    }
    #[tokio::test]
    async fn close_and_confirmation_share_one_budget() {
        let result = confirmed_close(
            async {
                tokio::time::sleep(Duration::from_millis(10)).await;
                Ok(())
            },
            || async {
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok(false)
            },
            Duration::from_millis(20),
        )
        .await;
        assert!(matches!(result, Err(E2eError::Timeout(20, _))));
    }
}
