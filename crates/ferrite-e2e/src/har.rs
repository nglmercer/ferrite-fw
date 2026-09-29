//! HAR 1.2 export and replay for recorded network traffic (Playwright
//! `recordHar` / `routeFromHAR`).

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::{E2eError, E2eResult};
use crate::page::RecordedRequest;

/// Response-body handling for HAR export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HarContentMode {
    /// Omit bodies (`content.size` is `-1`).
    #[default]
    Omit,
    /// Embed bodies as base64 `content.text` (entries without a captured
    /// body export as in `Omit`).
    Embed,
}

/// Build a HAR 1.2 document from recorded requests (content-`omit` mode).
pub(crate) fn har_json(requests: &[RecordedRequest]) -> Value {
    har_json_with(requests, HarContentMode::Omit)
}

/// Build a HAR 1.2 document with an explicit content mode.
pub(crate) fn har_json_with(requests: &[RecordedRequest], mode: HarContentMode) -> Value {
    let entries: Vec<Value> = requests
        .iter()
        .map(|request| har_entry(request, mode))
        .collect();
    json!({
        "log": {
            "version": "1.2",
            "creator": { "name": "ferrite", "version": env!("CARGO_PKG_VERSION") },
            "pages": [{ "startedDateTime": entries.first()
                .and_then(|e| e.get("startedDateTime")).cloned()
                .unwrap_or(Value::String(iso8601(0))),
                "id": "page_1", "title": "page",
                "pageTimings": { "onContentLoad": -1, "onLoad": -1 } }],
            "entries": entries,
        }
    })
}

/// One HAR entry from a recorded request/response pair.
fn har_entry(request: &RecordedRequest, mode: HarContentMode) -> Value {
    let started = request.started_ms.unwrap_or(0);
    let time = request.duration_ms.unwrap_or(0);
    let (path, query) = split_query(&request.url);
    let content = match (mode, request.body.as_ref()) {
        (HarContentMode::Embed, Some(body)) => {
            use base64::Engine as _;
            json!({
                "size": body.len(),
                "mimeType": request.mime_type,
                "text": base64::engine::general_purpose::STANDARD.encode(body),
                "encoding": "base64",
            })
        }
        _ => json!({ "size": -1, "mimeType": request.mime_type }),
    };
    json!({
        "pageref": "page_1",
        "startedDateTime": iso8601(started),
        "time": time,
        "request": {
            "method": request.method,
            "url": request.url,
            "httpVersion": "HTTP/1.1",
            "cookies": [],
            "headers": header_objects(&request.headers),
            "queryString": query,
            "headersSize": -1,
            "bodySize": request.post_data.as_ref().map_or(0, String::len),
            "postData": request.post_data.as_ref().map(|text| json!({
                "mimeType": content_type(&request.headers),
                "text": text,
            })).unwrap_or(Value::Null),
            "comment": path,
        },
        "response": {
            "status": request.status,
            "statusText": request.status_text,
            "httpVersion": "HTTP/1.1",
            "cookies": [],
            "headers": header_objects(&request.response_headers),
            "content": content,
            "redirectURL": "",
            "headersSize": -1,
            "bodySize": -1,
        },
        "cache": {},
        "timings": {
            "send": 0,
            "wait": time,
            "receive": 0,
        },
    })
}

/// Header pairs as HAR `{name, value}` objects.
fn header_objects(headers: &[(String, String)]) -> Vec<Value> {
    headers
        .iter()
        .map(|(name, value)| json!({ "name": name, "value": value }))
        .collect()
}

/// Request `Content-Type` header value (`""` when absent).
fn content_type(headers: &[(String, String)]) -> &str {
    headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
        .map(|(_, value)| value.as_str())
        .unwrap_or("")
}

/// Split `url` into (path, HAR query objects); values stay percent-encoded.
fn split_query(url: &str) -> (String, Vec<Value>) {
    let path = url.split(['?', '#']).next().unwrap_or(url).to_string();
    let query = url.split('?').nth(1).map_or_else(Vec::new, |rest| {
        let rest = rest.split('#').next().unwrap_or(rest);
        rest.split('&')
            .filter(|pair| !pair.is_empty())
            .map(|pair| {
                let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
                json!({ "name": name, "value": value })
            })
            .collect()
    });
    (path, query)
}

/// Format epoch milliseconds as `YYYY-MM-DDTHH:MM:SS.sssZ` (no date crates).
pub(crate) fn iso8601(epoch_ms: u64) -> String {
    let secs = epoch_ms / 1000;
    let ms = epoch_ms % 1000;
    let days = (secs / 86_400) as i64;
    let time = secs % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{ms:03}Z",
        time / 3600,
        (time % 3600) / 60,
        time % 60,
    )
}

/// Days since 1970-01-01 to (year, month, day) (Howard Hinnant's algorithm).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

/// One replayable HAR response (method + exact URL match).
#[derive(Debug, Clone)]
pub struct HarReplayEntry {
    /// HTTP method (uppercased).
    pub method: String,
    /// Exact request URL.
    pub url: String,
    /// Response status.
    pub status: u16,
    /// Response status text.
    pub status_text: String,
    /// Response headers.
    pub headers: Vec<(String, String)>,
    /// Response body bytes (empty when the HAR omits content).
    pub body: Vec<u8>,
}

impl HarReplayEntry {
    /// Body decoded as UTF-8 (`None` when empty or non-UTF-8).
    #[must_use]
    pub fn body_text(&self) -> Option<String> {
        if self.body.is_empty() {
            return None;
        }
        String::from_utf8(self.body.clone()).ok()
    }
}

/// A parsed HAR 1.2 file for [`Page::route_from_har`](crate::page::Page::route_from_har).
#[derive(Debug, Clone, Default)]
pub struct HarFile {
    entries: Vec<HarReplayEntry>,
}

impl HarFile {
    /// Load and decode a HAR file (fails loudly on corrupt JSON/bodies).
    ///
    /// Entries with status `0` (failed requests) are skipped: there is no
    /// response to replay.
    pub fn load(path: impl AsRef<Path>) -> E2eResult<Self> {
        let raw = std::fs::read_to_string(path.as_ref()).map_err(|error| {
            E2eError::Config(format!(
                "cannot read HAR {}: {error}",
                path.as_ref().display()
            ))
        })?;
        let parsed: HarRaw = serde_json::from_str(&raw).map_err(|error| {
            E2eError::Config(format!(
                "cannot parse HAR {}: {error}",
                path.as_ref().display()
            ))
        })?;
        let mut entries = Vec::with_capacity(parsed.log.entries.len());
        for entry in parsed.log.entries {
            if entry.response.status == 0 {
                continue;
            }
            entries.push(entry.decode()?);
        }
        Ok(Self { entries })
    }

    /// Replayable entries (file order).
    #[must_use]
    pub fn entries(&self) -> &[HarReplayEntry] {
        &self.entries
    }

    /// Entries as a `(METHOD, url)` lookup (first entry wins on duplicates).
    pub(crate) fn lookup(&self) -> HashMap<(String, String), HarReplayEntry> {
        let mut map = HashMap::new();
        for entry in &self.entries {
            map.entry((entry.method.clone(), entry.url.clone()))
                .or_insert_with(|| entry.clone());
        }
        map
    }
}

/// Minimal HAR 1.2 shape (unknown fields ignored).
#[derive(Debug, Deserialize)]
struct HarRaw {
    #[serde(default)]
    log: HarLogRaw,
}

#[derive(Debug, Default, Deserialize)]
struct HarLogRaw {
    #[serde(default)]
    entries: Vec<HarEntryRaw>,
}

#[derive(Debug, Deserialize)]
struct HarEntryRaw {
    request: HarRequestRaw,
    response: HarResponseRaw,
}

#[derive(Debug, Deserialize)]
struct HarRequestRaw {
    #[serde(default)]
    method: String,
    #[serde(default)]
    url: String,
}

#[derive(Debug, Deserialize)]
struct HarResponseRaw {
    #[serde(default)]
    status: u16,
    #[serde(default, rename = "statusText")]
    status_text: String,
    #[serde(default)]
    headers: Vec<HarHeaderRaw>,
    #[serde(default)]
    content: HarContentRaw,
}

#[derive(Debug, Deserialize)]
struct HarHeaderRaw {
    #[serde(default)]
    name: String,
    #[serde(default)]
    value: String,
}

#[derive(Debug, Default, Deserialize)]
struct HarContentRaw {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    encoding: Option<String>,
}

impl HarEntryRaw {
    fn decode(&self) -> E2eResult<HarReplayEntry> {
        let body = match (
            self.response.content.text.as_deref(),
            self.response.content.encoding.as_deref(),
        ) {
            (None, _) => Vec::new(),
            (Some(text), Some("base64")) => {
                use base64::Engine as _;
                base64::engine::general_purpose::STANDARD
                    .decode(text)
                    .map_err(|error| {
                        E2eError::Config(format!(
                            "cannot decode HAR body for {} {}: {error}",
                            self.request.method, self.request.url
                        ))
                    })?
            }
            (Some(text), _) => text.as_bytes().to_vec(),
        };
        Ok(HarReplayEntry {
            method: self.request.method.to_ascii_uppercase(),
            url: self.request.url.clone(),
            status: self.response.status,
            status_text: self.response.status_text.clone(),
            headers: self
                .response
                .headers
                .iter()
                .map(|header| (header.name.clone(), header.value.clone()))
                .collect(),
            body,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_boundaries_format() {
        assert_eq!(iso8601(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(iso8601(1_000), "1970-01-01T00:00:01.000Z");
        assert_eq!(iso8601(1_726_627_200_123), "2024-09-18T02:40:00.123Z");
        assert_eq!(iso8601(1_893_456_000_000), "2030-01-01T00:00:00.000Z");
    }

    #[test]
    fn query_splits() {
        let (path, query) = split_query("http://x.test/a?b=1&c#frag");
        assert_eq!(path, "http://x.test/a");
        assert_eq!(
            query,
            vec![
                json!({ "name": "b", "value": "1" }),
                json!({ "name": "c", "value": "" }),
            ]
        );
        let (path, query) = split_query("http://x.test/a");
        assert_eq!(path, "http://x.test/a");
        assert!(query.is_empty());
    }

    fn write_har(name: &str, text: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("ferrite-har-{name}-{}", std::process::id()));
        std::fs::write(&path, text).unwrap();
        path
    }

    #[test]
    fn har_load_decodes_bodies() {
        let path = write_har(
            "load",
            r#"{"log": {"entries": [
                {"request": {"method": "get", "url": "http://x.test/a"},
                 "response": {"status": 200, "statusText": "OK",
                   "headers": [{"name": "X-A", "value": "1"}],
                   "content": {"text": "aGVsbG8=", "encoding": "base64"}}},
                {"request": {"method": "POST", "url": "http://x.test/b"},
                 "response": {"status": 201, "statusText": "Created",
                   "headers": [],
                   "content": {"text": "plain"}}},
                {"request": {"method": "GET", "url": "http://x.test/c"},
                 "response": {"status": 200, "statusText": "OK",
                   "headers": [], "content": {}}},
                {"request": {"method": "GET", "url": "http://x.test/dead"},
                 "response": {"status": 0}}
            ]}}"#,
        );
        let file = HarFile::load(&path).unwrap();
        // Status-0 entries are skipped.
        assert_eq!(file.entries().len(), 3);
        assert_eq!(file.entries()[0].method, "GET");
        assert_eq!(file.entries()[0].body, b"hello");
        assert_eq!(
            file.entries()[0].headers,
            vec![("X-A".to_string(), "1".to_string())]
        );
        assert_eq!(file.entries()[1].body, b"plain");
        assert!(file.entries()[2].body.is_empty());
        let lookup = file.lookup();
        assert_eq!(lookup.len(), 3);
        assert_eq!(
            lookup[&("GET".to_string(), "http://x.test/a".to_string())].status,
            200
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn har_load_is_loud() {
        let missing = std::env::temp_dir().join("ferrite-har-nope.json");
        let _ = std::fs::remove_file(&missing);
        assert!(HarFile::load(&missing).is_err());
        let bad = write_har("bad", "not json{{{");
        assert!(HarFile::load(&bad).is_err());
        let bad_body = write_har(
            "badbody",
            r#"{"log": {"entries": [
                {"request": {"method": "GET", "url": "http://x.test/a"},
                 "response": {"status": 200, "content": {"text": "!!!", "encoding": "base64"}}}
            ]}}"#,
        );
        let error = HarFile::load(&bad_body).unwrap_err();
        assert!(error.to_string().contains("decode"), "{error}");
        let _ = std::fs::remove_file(&bad);
        let _ = std::fs::remove_file(&bad_body);
    }

    #[test]
    fn har_embed_includes_bodies() {
        let with_body = RecordedRequest {
            method: "GET".to_string(),
            url: "http://x.test/a".to_string(),
            status: 200,
            headers: Vec::new(),
            post_data: None,
            duration_ms: Some(3),
            status_text: "OK".to_string(),
            mime_type: "application/json".to_string(),
            response_headers: Vec::new(),
            started_ms: Some(1_700_000_000_000),
            request_id: None,
            body: Some(br#"{"real":true}"#.to_vec()),
            body_truncated: false,
        };
        let embedded = har_json_with(std::slice::from_ref(&with_body), HarContentMode::Embed);
        let content = &embedded["log"]["entries"][0]["response"]["content"];
        assert_eq!(content["encoding"], json!("base64"));
        assert_eq!(content["size"], json!(13));
        use base64::Engine as _;
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(content["text"].as_str().unwrap())
            .unwrap();
        assert_eq!(decoded, br#"{"real":true}"#);
        // Omit mode and body-less entries keep the unknown-size shape.
        let omitted = har_json(std::slice::from_ref(&with_body));
        assert_eq!(
            omitted["log"]["entries"][0]["response"]["content"]["size"],
            json!(-1)
        );
        let mut bare = with_body.clone();
        bare.body = None;
        let embedded = har_json_with(std::slice::from_ref(&bare), HarContentMode::Embed);
        assert_eq!(
            embedded["log"]["entries"][0]["response"]["content"]["size"],
            json!(-1)
        );
    }
}
