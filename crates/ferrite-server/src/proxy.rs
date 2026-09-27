//! Dev/preview HTTP proxy (Vite `server.proxy`).
//!
//! Prefix rules forward matching requests to a target origin with the
//! path preserved; plugin-registered rules (see
//! [`ferrite_plugin::PreviewControl`]) and `[server] proxy` config entries
//! share this forwarder.

use ferrite_plugin::ProxyRule;
use std::cmp::Reverse;
use std::collections::HashMap;

/// Build longest-prefix-first rules from `[server] proxy` config.
#[must_use]
pub fn rules_from_config(proxy: &HashMap<String, String>) -> Vec<ProxyRule> {
    let mut rules: Vec<ProxyRule> = proxy
        .iter()
        .map(|(prefix, target)| ProxyRule {
            prefix: prefix.clone(),
            target: target.clone(),
        })
        .collect();
    rules.sort_by_key(|rule| Reverse(rule.prefix.len()));
    rules
}

/// First matching rule for `path`, if any.
#[must_use]
pub fn match_proxy<'a>(rules: &'a [ProxyRule], path: &str) -> Option<&'a ProxyRule> {
    rules.iter().find(|rule| rule.matches(path))
}

/// Hop-by-hop headers, never forwarded.
const HOP_HEADERS: [&str; 8] = [
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
];

/// Forward one request to `target_url` (status + headers + bytes preserved).
pub async fn forward(
    client: &reqwest::Client,
    method: &axum::http::Method,
    target_url: &str,
    headers: &axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> Result<axum::response::Response, reqwest::Error> {
    let mut outgoing = client.request(method.clone(), target_url);
    for (name, value) in headers {
        if name.as_str() == "host" || HOP_HEADERS.contains(&name.as_str()) {
            continue;
        }
        outgoing = outgoing.header(name, value);
    }
    if !body.is_empty() {
        outgoing = outgoing.body(body.to_vec());
    }
    let upstream = outgoing.send().await?;
    let status = axum::http::StatusCode::from_u16(upstream.status().as_u16())
        .unwrap_or(axum::http::StatusCode::BAD_GATEWAY);
    let mut out_headers = axum::http::HeaderMap::new();
    for (name, value) in upstream.headers() {
        if name.as_str() == "content-length" || HOP_HEADERS.contains(&name.as_str()) {
            continue;
        }
        out_headers.insert(name, value.clone());
    }
    let bytes = upstream.bytes().await?;
    let mut response = axum::response::Response::new(axum::body::Body::from(bytes));
    *response.status_mut() = status;
    *response.headers_mut() = out_headers;
    Ok(response)
}
