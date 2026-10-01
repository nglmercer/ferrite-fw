//! Confirm native disappearance rather than treating close acknowledgment as disposal.
use crate::{operation::Deadline, E2eResult};
use std::future::Future;

pub(crate) async fn confirmed_close<C, Q, F>(
    close: C,
    mut absent: Q,
    budget: Deadline,
) -> E2eResult<()>
where
    C: Future<Output = E2eResult<()>>,
    Q: FnMut() -> F,
    F: Future<Output = E2eResult<bool>>,
{
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
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                Ok(())
            })
            .await?;
    }
}

pub(crate) fn context_absent(value: &serde_json::Value, id: &str, bidi: bool) -> E2eResult<bool> {
    let field = if bidi {
        "userContexts"
    } else {
        "browserContextIds"
    };
    let entries = value[field].as_array().ok_or_else(|| {
        crate::E2eError::Config(format!("native context inventory omitted {field}"))
    })?;
    let mut present = false;
    for entry in entries {
        let native = if bidi {
            entry["userContext"].as_str()
        } else {
            entry.as_str()
        }
        .filter(|id| !id.is_empty())
        .ok_or_else(|| {
            crate::E2eError::Config(
                "native context inventory contains an invalid context ID".into(),
            )
        })?;
        present |= native == id;
    }
    Ok(!present)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::E2eError;
    use std::time::Duration;
    #[test]
    fn context_inventory_requires_valid_native_ids_and_distinguishes_present_from_absent() {
        for bidi in [false, true] {
            let field = if bidi {
                "userContexts"
            } else {
                "browserContextIds"
            };
            let row = |id| {
                if bidi {
                    serde_json::json!({"userContext":id})
                } else {
                    serde_json::json!(id)
                }
            };
            assert!(!context_absent(
                &serde_json::json!({field:[row("owned"),row("other")]}),
                "owned",
                bidi
            )
            .unwrap());
            assert!(
                context_absent(&serde_json::json!({field:[row("other")]}), "owned", bidi).unwrap()
            );
            for invalid in [
                serde_json::Value::Null,
                serde_json::json!({}),
                serde_json::json!({field:[row(""), row("other")]}),
                serde_json::json!({field:[null]}),
            ] {
                assert!(context_absent(&invalid, "owned", bidi).is_err());
            }
        }
    }

    #[tokio::test]
    async fn acknowledgments_require_disappearance_and_errors_remain_visible() {
        let mut observations = std::collections::VecDeque::from([false, false, true]);
        confirmed_close(
            async { Ok(()) },
            || std::future::ready(Ok(observations.pop_front().unwrap())),
            Deadline::cleanup(Duration::from_secs(1)),
        )
        .await
        .unwrap();
        assert!(observations.is_empty());
        let rejected = || E2eError::Cdp {
            method: "close".into(),
            message: "rejected".into(),
        };
        assert!(
            matches!(confirmed_close(async { Err(rejected()) }, || std::future::ready(Ok(false)), Deadline::cleanup(Duration::from_secs(1))).await, Err(E2eError::Cdp { message, .. }) if message == "rejected")
        );
        // An authoritative absence also resolves a concurrent external close.
        confirmed_close(
            async { Err(rejected()) },
            || std::future::ready(Ok(true)),
            Deadline::cleanup(Duration::from_secs(1)),
        )
        .await
        .unwrap();
        assert!(matches!(
            confirmed_close(
                async { Ok(()) },
                || std::future::ready(Err(E2eError::Disconnected("lost query".into()))),
                Deadline::cleanup(Duration::from_secs(1))
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
            Deadline::cleanup(Duration::from_millis(20)),
        )
        .await;
        assert!(matches!(result, Err(E2eError::Timeout(20, _))));
    }
}
