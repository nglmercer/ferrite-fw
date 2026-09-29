//! Direct context cookie operations; never open a scratch page.
use crate::{
    bidi::bytes_to_string,
    browser::Backend,
    error::{E2eError, E2eResult},
    page::Cookie,
};
use serde_json::Value;
use std::time::Duration;
fn url_host(url: &str) -> Option<String> {
    reqwest::Url::parse(url).ok()?.host_str().map(str::to_owned)
}
pub(crate) async fn cookies(
    backend: &Backend,
    id: Option<&str>,
    timeout: Duration,
) -> E2eResult<Vec<Cookie>> {
    match backend {
        Backend::Cdp(conn) => {
            let mut params = serde_json::json!({});
            if let Some(id) = id {
                params["browserContextId"] = Value::String(id.to_string());
            }
            let cookies = conn
                .call(None, "Storage.getCookies", params, timeout)
                .await?;
            let list = cookies
                .get("cookies")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            serde_json::from_value(Value::Array(list)).map_err(E2eError::Json)
        }
        Backend::Bidi { conn, .. } => {
            let result = conn
            .call(
                "storage.getCookies",
                serde_json::json!({
                    "partition": { "type": "storageKey", "userContext": id.unwrap_or("default") },
                }),
                timeout,
            )
            .await?;
            let mut cookies = Vec::new();
            if let Some(list) = result.get("cookies").and_then(Value::as_array) {
                for cookie in list {
                    cookies.push(Cookie {
                        name: cookie["name"].as_str().unwrap_or_default().to_string(),
                        value: bytes_to_string(&cookie["value"]),
                        domain: cookie["domain"].as_str().map(str::to_string),
                        path: cookie["path"].as_str().map(str::to_string),
                        http_only: cookie["httpOnly"].as_bool().unwrap_or(false),
                        secure: cookie["secure"].as_bool().unwrap_or(false),
                        // Firefox also reports an unspecified/default policy.
                        // It is not one of the portable SameSite attribute values.
                        same_site: match cookie["sameSite"].as_str() {
                            Some("strict") => Some("Strict".into()),
                            Some("lax") => Some("Lax".into()),
                            Some("none") => Some("None".into()),
                            _ => None,
                        },
                        expires: cookie["expiry"]
                            .as_i64()
                            .or_else(|| cookie["expiry"].as_str().and_then(|raw| raw.parse().ok())),
                    });
                }
            }
            Ok(cookies)
        }
    }
}
pub(crate) async fn add_cookies(
    backend: &Backend,
    id: Option<&str>,
    timeout: Duration,
    cookies: &[Cookie],
    url: &str,
) -> E2eResult<()> {
    match backend {
        Backend::Cdp(conn) => {
            let cookies: Vec<Value> = cookies.iter().map(|cookie| {
            let mut params = serde_json::json!({"name":cookie.name,"value":cookie.value,"url":url,"httpOnly":cookie.http_only,"secure":cookie.secure});
            if let Some(policy)=&cookie.same_site {params["sameSite"]=Value::String(policy.clone());}
            if let Some(domain) = &cookie.domain { params["domain"] = Value::String(domain.clone()); }
            if let Some(path) = &cookie.path { params["path"] = Value::String(path.clone()); }
            if let Some(expires) = cookie.expires.filter(|expires| *expires >= 0) { params["expires"] = Value::from(expires); }
            params
        }).collect();
            let mut params = serde_json::json!({"cookies":cookies});
            if let Some(id) = id {
                params["browserContextId"] = Value::String(id.to_string());
            }
            conn.call(None, "Storage.setCookies", params, timeout)
                .await?;
            Ok(())
        }
        Backend::Bidi { conn, .. } => {
            let host = url_host(url);
            for cookie in cookies {
                let mut params = serde_json::json!({
                    "name": cookie.name,
                    "value": { "type": "string", "value": cookie.value },
                    "path": cookie.path.as_deref().unwrap_or("/"),
                });
                if let Some(domain) = cookie.domain.as_deref().or(host.as_deref()) {
                    params["domain"] = Value::String(domain.to_string());
                }
                if cookie.secure {
                    params["secure"] = Value::Bool(true);
                }
                if cookie.http_only {
                    params["httpOnly"] = Value::Bool(true);
                }
                if let Some(expiry) = cookie.expires.filter(|expiry| *expiry >= 0) {
                    params["expiry"] = Value::from(expiry);
                }
                if let Some(policy) = &cookie.same_site {
                    params["sameSite"] = Value::String(policy.to_ascii_lowercase());
                }
                // RFC6265 expiry deletes cookies instead of setting an expired value.
                if cookie.expires.is_some_and(|expiry| {
                    expiry >= 0
                        && expiry
                            <= std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs() as i64
                }) {
                    conn.call("storage.deleteCookies",serde_json::json!({"filter":{"name":cookie.name,"domain":params["domain"],"path":params["path"]},"partition":{"type":"storageKey","userContext":id.unwrap_or("default")}}),timeout).await?;
                    continue;
                }
                conn
                .call(
                    "storage.setCookie",
                    serde_json::json!({
                        "cookie": params,
                        "partition": { "type": "storageKey", "userContext": id.unwrap_or("default") },
                    }),
                    timeout,
                )
                .await?;
            }
            Ok(())
        }
    }
}
pub(crate) async fn clear_cookies(
    backend: &Backend,
    id: Option<&str>,
    timeout: Duration,
) -> E2eResult<()> {
    match backend {
        Backend::Cdp(conn) => {
            let mut params = serde_json::json!({});
            if let Some(id) = id {
                params["browserContextId"] = Value::String(id.to_string());
            }
            conn.call(None, "Storage.clearCookies", params, timeout)
                .await?;
            Ok(())
        }
        Backend::Bidi { conn, .. } => {
            conn.call(
                "storage.deleteCookies",
                serde_json::json!({
                    "partition": { "type": "storageKey", "userContext": id.unwrap_or("default") },
                }),
                timeout,
            )
            .await?;
            Ok(())
        }
    }
}

pub(crate) async fn clear_filtered_cookies(
    backend: &Backend,
    id: Option<&str>,
    timeout: Duration,
    filter: &crate::CookieFilter,
) -> E2eResult<()> {
    if filter.is_empty() {
        return clear_cookies(backend, id, timeout).await;
    }
    match backend {
        Backend::Cdp(conn) => {
            let mut params = serde_json::json!({});
            if let Some(id) = id {
                params["browserContextId"] = Value::String(id.into());
            }
            let result = conn
                .call(None, "Storage.getCookies", params.clone(), timeout)
                .await?;
            let mut expired = Vec::new();
            for cookie in result["cookies"]
                .as_array()
                .ok_or_else(|| E2eError::Config("native cookies unavailable".into()))?
            {
                if !filter.matches(
                    cookie["name"].as_str().unwrap_or_default(),
                    cookie["domain"].as_str().unwrap_or_default(),
                    cookie["path"].as_str().unwrap_or_default(),
                ) {
                    continue;
                }
                if cookie["partitionKeyOpaque"].as_bool() == Some(true) {
                    return Err(E2eError::Config(
                        "filtered deletion of opaque partition cookies is unsupported".into(),
                    ));
                }
                // Expire only selected native keys; never clear and rebuild the
                // whole store (which loses metadata and unrelated concurrent writes).
                let mut value = serde_json::Map::new();
                for key in [
                    "name",
                    "value",
                    "domain",
                    "path",
                    "secure",
                    "httpOnly",
                    "sameSite",
                    "priority",
                    "sameParty",
                    "sourceScheme",
                    "sourcePort",
                    "partitionKey",
                ] {
                    if let Some(field) = cookie.get(key) {
                        value.insert(key.into(), field.clone());
                    }
                }
                value.insert("expires".into(), Value::from(1));
                expired.push(Value::Object(value));
            }
            if !expired.is_empty() {
                params["cookies"] = Value::Array(expired);
                conn.call(None, "Storage.setCookies", params, timeout)
                    .await?;
            }
        }
        Backend::Bidi { conn, .. } => {
            for cookie in cookies(backend, id, timeout).await? {
                if !filter.matches(
                    &cookie.name,
                    cookie.domain.as_deref().unwrap_or_default(),
                    cookie.path.as_deref().unwrap_or_default(),
                ) {
                    continue;
                }
                conn.call(
                    "storage.deleteCookies",
                    serde_json::json!({
                        "filter":{"name":cookie.name,"domain":cookie.domain,"path":cookie.path},
                        "partition":{"type":"storageKey","userContext":id.unwrap_or("default")}
                    }),
                    timeout,
                )
                .await?;
            }
        }
    }
    Ok(())
}
