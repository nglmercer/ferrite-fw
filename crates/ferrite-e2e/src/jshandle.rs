//! Remote JavaScript object references (Playwright `JSHandle`).

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::driver::Driver;
use crate::error::E2eResult;

/// A reference to a JavaScript value living in the page.
///
/// Handles keep the remote value alive until [`JSHandle::dispose`]; clones
/// share the same remote reference. Primitive results (numbers, strings,
/// booleans, `null`) are inlined and need no disposal.
#[derive(Clone)]
pub struct JSHandle {
    driver: Driver,
    /// Remote reference (CDP `objectId` / BiDi `handle`); `None` for inlined
    /// primitives.
    remote_id: Option<String>,
    /// Inlined primitive value.
    value: Option<Value>,
}

impl std::fmt::Debug for JSHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JSHandle")
            .field("remote_id", &self.remote_id)
            .field("primitive", &self.is_primitive())
            .finish_non_exhaustive()
    }
}

impl JSHandle {
    pub(crate) fn new(driver: Driver, remote_id: Option<String>, value: Option<Value>) -> Self {
        Self {
            driver,
            remote_id,
            value,
        }
    }

    /// Whether this handle wraps an inlined primitive (no remote reference).
    #[must_use]
    pub fn is_primitive(&self) -> bool {
        self.remote_id.is_none()
    }

    /// Serialize the value to JSON (Playwright `jsonValue()`).
    pub async fn json_value<T: DeserializeOwned>(&self) -> E2eResult<T> {
        let value = self
            .driver
            .handle_json_value(self.remote_id.as_deref(), self.value.clone())
            .await?;
        Ok(serde_json::from_value(value)?)
    }

    /// A handle to the named property (Playwright `getProperty()`).
    pub async fn get_property(&self, name: &str) -> E2eResult<JSHandle> {
        self.driver
            .handle_get_property(self.remote_id.as_deref(), self.value.clone(), name)
            .await
    }

    /// Run `function` with this value as its first argument and return the
    /// JSON result (Playwright `handle.evaluate(fn)`).
    pub async fn evaluate<T: DeserializeOwned>(&self, function: &str) -> E2eResult<T> {
        let value = self
            .driver
            .handle_evaluate(self.remote_id.as_deref(), self.value.clone(), function)
            .await?;
        Ok(serde_json::from_value(value)?)
    }

    /// Release the remote reference (Playwright `dispose()`); a no-op for
    /// inlined primitives.
    pub async fn dispose(&self) -> E2eResult<()> {
        self.driver.handle_dispose(self.remote_id.as_deref()).await
    }
}
