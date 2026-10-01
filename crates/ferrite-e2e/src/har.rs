//! HAR 1.2 export and replay for recorded network traffic (Playwright
//! `recordHar` / `routeFromHAR`).

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
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
            "redirectURL": request.response_headers.iter().find(|(name,_)| name.eq_ignore_ascii_case("location")).map(|(_,value)| {
                reqwest::Url::parse(&request.url).and_then(|base| base.join(value)).map(|url| url.to_string()).unwrap_or_else(|_| value.clone())
            }).unwrap_or_default(),
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
    /// Request headers used to score duplicate candidates.
    pub request_headers: Vec<(String, String)>,
    /// Original request body, when present.
    pub post_data: Option<Vec<u8>>,
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
        Self::load_checked(path.as_ref(), || Ok(()))
    }
    fn load_checked(path: &Path, mut check: impl FnMut() -> E2eResult<()>) -> E2eResult<Self> {
        use std::io::Read;
        check()?;
        let before = std::fs::metadata(path).map_err(|error| {
            E2eError::Config(format!("cannot read HAR {}: {error}", path.display()))
        })?;
        if !before.is_file() || before.len() > MAX_FILE as u64 {
            return Err(E2eError::Config(
                "HAR must be a regular file of at most 32 MiB".into(),
            ));
        }
        let mut file = std::fs::File::open(path).map_err(|error| {
            E2eError::Config(format!("cannot read HAR {}: {error}", path.display()))
        })?;
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.len() > MAX_FILE as u64 {
            return Err(E2eError::Config(
                "HAR must be a regular file of at most 32 MiB".into(),
            ));
        }
        check()?;
        let mut raw = Vec::new();
        file.by_ref()
            .take(MAX_FILE as u64 + 1)
            .read_to_end(&mut raw)?;
        if raw.len() > MAX_FILE {
            return Err(E2eError::Config(
                "HAR exceeds 32 MiB encoded file limit".into(),
            ));
        }
        check()?;
        let parsed: HarRaw = serde_json::from_slice(&raw).map_err(|error| {
            E2eError::Config(format!("cannot parse HAR {}: {error}", path.display()))
        })?;
        if parsed.log.entries.len() > MAX_ENTRIES {
            return Err(E2eError::Config("HAR exceeds 4096 entry limit".into()));
        }
        let mut entries = Vec::new();
        let mut bytes = 0;
        for entry in parsed.log.entries {
            check()?;
            if entry.response.status == 0 {
                continue;
            }
            let entry = entry.decode()?;
            bytes += entry.retained_size();
            if bytes > MAX_RETAINED {
                return Err(E2eError::Config(
                    "HAR exceeds 16 MiB retained replay limit".into(),
                ));
            }
            entries.push(entry);
        }
        check()?;
        Ok(Self { entries })
    }

    /// Replayable entries (file order).
    #[must_use]
    pub fn entries(&self) -> &[HarReplayEntry] {
        &self.entries
    }

    /// Entries as a `(METHOD, url)` lookup (first entry wins on duplicates).
    #[cfg(test)]
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
    log: HarLogRaw,
}

#[derive(Debug, Default, Deserialize)]
struct HarLogRaw {
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
    #[serde(default)]
    headers: Vec<HarHeaderRaw>,
    #[serde(default, rename = "postData")]
    post_data: Option<HarContentRaw>,
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
    #[serde(default, rename = "redirectURL")]
    redirect_url: String,
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
        if !(100..=599).contains(&self.response.status)
            || self.request.method.is_empty()
            || !self
                .request
                .method
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
        {
            return Err(E2eError::Config(
                "HAR has invalid HTTP status or request method".into(),
            ));
        }
        let url = reqwest::Url::parse(&self.request.url)
            .map_err(|error| E2eError::Config(format!("invalid HAR URL: {error}")))?;
        if !matches!(url.scheme(), "http" | "https") || self.request.url.len() > 16384 {
            return Err(E2eError::Config(
                "HAR requires HTTP(S) URLs of at most 16 KiB".into(),
            ));
        }
        let body = decode_content(&self.response.content).map_err(|error| {
            E2eError::Config(format!(
                "cannot decode HAR body for {} {}: {error}",
                self.request.method, self.request.url
            ))
        })?;
        let post_data = self
            .request
            .post_data
            .as_ref()
            .map(decode_content)
            .transpose()?;
        let mut headers: Vec<_> = self
            .response
            .headers
            .iter()
            .map(|header| (header.name.clone(), header.value.clone()))
            .collect();
        if (300..=399).contains(&self.response.status)
            && self.response.status != 304
            && !self.response.redirect_url.is_empty()
            && !headers
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case("location"))
        {
            headers.push(("Location".into(), self.response.redirect_url.clone()));
        }
        Ok(HarReplayEntry {
            method: self.request.method.to_ascii_uppercase(),
            request_headers: self
                .request
                .headers
                .iter()
                .map(|header| (header.name.clone(), header.value.clone()))
                .collect(),
            post_data,
            url: self.request.url.clone(),
            status: self.response.status,
            status_text: self.response.status_text.clone(),
            headers,
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

const MAX_FILE: usize = 32 * 1024 * 1024;
const MAX_ENTRIES: usize = 4096;
const MAX_RETAINED: usize = 16 * 1024 * 1024;
const MAX_BODY: usize = 1024 * 1024;
/// Behavior for requests selected by the HAR URL filter without a replay match.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum HarNotFound {
    /// Pass to the next route or real network; legacy Ferrite default.
    #[default]
    Fallback,
    /// Fail the request; Playwright's default behavior.
    Abort,
}
fn decode_content(content: &HarContentRaw) -> E2eResult<Vec<u8>> {
    let Some(text) = &content.text else {
        return Ok(Vec::new());
    };
    let limit = if content.encoding.as_deref() == Some("base64") {
        MAX_BODY.div_ceil(3) * 4
    } else {
        MAX_BODY
    };
    if text.len() > limit {
        return Err(E2eError::Config(
            "HAR body exceeds 1 MiB decoded limit".into(),
        ));
    }
    let body = match content.encoding.as_deref() {
        Some("base64") => {
            use base64::Engine;
            base64::engine::general_purpose::STANDARD
                .decode(text)
                .map_err(|error| E2eError::Config(format!("invalid HAR base64: {error}")))?
        }
        None | Some("") => text.as_bytes().to_vec(),
        Some(_) => return Err(E2eError::Config("unsupported HAR content encoding".into())),
    };
    if body.len() > MAX_BODY {
        return Err(E2eError::Config(
            "HAR body exceeds 1 MiB decoded limit".into(),
        ));
    }
    Ok(body)
}
impl HarReplayEntry {
    fn retained_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + (self.headers.len() + self.request_headers.len())
                * std::mem::size_of::<(String, String)>()
            + self.method.len()
            + self.url.len()
            + self.status_text.len()
            + self.body.len()
            + self.post_data.as_ref().map_or(0, Vec::len)
            + self
                .headers
                .iter()
                .chain(&self.request_headers)
                .map(|(name, value)| name.len() + value.len())
                .sum::<usize>()
    }
}
pub(crate) async fn load_async(path: std::path::PathBuf) -> E2eResult<HarFile> {
    crate::snapshot_work::run(move |stop| HarFile::load_checked(&path, || stop.check())).await
}
fn replay_key(method: &str, url: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (method, url).hash(&mut hasher);
    hasher.finish()
}
pub(crate) struct HarReplay {
    entries: Vec<HarReplayEntry>,
    index: HashMap<u64, Vec<usize>>,
    filter: Option<crate::url_matcher::HarFilter>,
    miss: HarNotFound,
}
impl HarReplay {
    pub(crate) fn new(
        file: HarFile,
        filter: Option<crate::url_matcher::HarFilter>,
        miss: HarNotFound,
    ) -> Self {
        let entries: Vec<_> = file
            .entries
            .into_iter()
            .filter(|entry| {
                filter
                    .as_ref()
                    .is_none_or(|filter| filter.matches(&entry.url))
            })
            .collect();
        let mut index: HashMap<_, Vec<_>> = HashMap::new();
        for (position, entry) in entries.iter().enumerate() {
            index
                .entry(replay_key(&entry.method, &entry.url))
                .or_default()
                .push(position);
        }
        Self {
            entries,
            index,
            filter,
            miss,
        }
    }
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }
    pub(crate) fn action(&self, info: &crate::RouteInfo) -> E2eResult<crate::RouteAction> {
        if self
            .filter
            .as_ref()
            .is_some_and(|filter| !filter.matches(&info.url))
        {
            return Ok(crate::RouteAction::Fallback);
        }
        let method = info.method.to_ascii_uppercase();
        if let Some(candidates) = self.index.get(&replay_key(&method, &info.url)) {
            let mut candidates = candidates
                .iter()
                .map(|position| &self.entries[*position])
                .filter(|entry| entry.method == method && entry.url == info.url)
                .peekable();
            if candidates.peek().is_none() {
                return Ok(match self.miss {
                    HarNotFound::Fallback => crate::RouteAction::Fallback,
                    HarNotFound::Abort => crate::RouteAction::Abort,
                });
            }
            let headers: HashSet<_> = info
                .headers
                .iter()
                .map(|(name, value)| (name.to_ascii_lowercase(), value.as_str()))
                .collect();
            if info.body_state() == crate::RouteBodyState::Unavailable {
                return Err(E2eError::Config(
                    "HAR replay cannot match unavailable native request bytes".into(),
                ));
            }
            let mut best = None;
            for entry in candidates {
                if entry.post_data.as_deref().unwrap_or_default()
                    != info.post_data.as_deref().unwrap_or_default()
                {
                    continue;
                }
                let score = entry
                    .request_headers
                    .iter()
                    .filter(|(name, value)| {
                        headers.contains(&(name.to_ascii_lowercase(), value.as_str()))
                    })
                    .count();
                if best.is_none_or(|(_, previous): (&HarReplayEntry, usize)| score > previous) {
                    best = Some((entry, score));
                }
            }
            if let Some((entry, _)) = best {
                let headers = entry
                    .headers
                    .iter()
                    .filter(|(name, _)| {
                        !name.eq_ignore_ascii_case("content-length")
                            && !name.eq_ignore_ascii_case("content-encoding")
                            && !name.eq_ignore_ascii_case("transfer-encoding")
                    })
                    .cloned()
                    .collect();
                return Ok(crate::RouteAction::fulfill_full(
                    entry.status,
                    entry.status_text.clone(),
                    headers,
                    entry.body.clone(),
                ));
            }
        }
        Ok(match self.miss {
            HarNotFound::Fallback => crate::RouteAction::Fallback,
            HarNotFound::Abort => crate::RouteAction::Abort,
        })
    }
}

/// Full metadata or replay-focused entries with diagnostic fields omitted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum HarRecordMode {
    #[default]
    Full,
    Minimal,
}
/// Captured total duration is an approximation; detailed TCP/TLS timing is unavailable.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum HarTimingMode {
    #[default]
    Recorded,
    Omit,
}
#[derive(Debug, Clone, Default)]
pub struct HarExportOptions {
    pub content: HarContentMode,
    pub mode: HarRecordMode,
    pub timing: HarTimingMode,
    pub url_matcher: Option<crate::UrlMatcher>,
    pub operation: crate::OperationOptions,
}
static EXPORTS: std::sync::LazyLock<std::sync::Arc<tokio::sync::Semaphore>> =
    std::sync::LazyLock::new(|| std::sync::Arc::new(tokio::sync::Semaphore::new(2)));
pub(crate) async fn export_permit() -> E2eResult<tokio::sync::OwnedSemaphorePermit> {
    EXPORTS
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| E2eError::Config("HAR export admission ended".into()))
}
pub(crate) fn stage_export(
    path: &Path,
    records: Vec<RecordedRequest>,
    options: &HarExportOptions,
    matcher: Option<crate::UrlMatcher>,
    mut check: impl FnMut() -> E2eResult<()>,
) -> E2eResult<tempfile::NamedTempFile> {
    check()?;
    let records: Vec<_> = records
        .into_iter()
        .filter(|record| {
            matcher
                .as_ref()
                .is_none_or(|matcher| matcher.matches(&record.url))
        })
        .collect();
    let mut document = har_json_with(&records, options.content);
    for entry in document["log"]["entries"]
        .as_array_mut()
        .into_iter()
        .flatten()
    {
        check()?;
        if options.timing == HarTimingMode::Omit {
            entry["time"] = json!(-1);
            entry["timings"] = json!({"send":-1,"wait":-1,"receive":-1});
        }
        if options.mode == HarRecordMode::Minimal {
            let object = entry.as_object_mut().unwrap();
            for field in ["pageref", "startedDateTime", "time", "cache", "timings"] {
                object.remove(field);
            }
            for side in ["request", "response"] {
                let object = entry[side].as_object_mut().unwrap();
                for field in [
                    "cookies",
                    "headersSize",
                    "bodySize",
                    "httpVersion",
                    "comment",
                    "queryString",
                ] {
                    object.remove(field);
                }
            }
        }
    }
    if options.mode == HarRecordMode::Minimal {
        document["log"].as_object_mut().unwrap().remove("pages");
    }
    check()?;
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut stage = tempfile::Builder::new()
        .prefix(".ferrite-har-")
        .tempfile_in(parent)?;
    struct Writer<'a, F> {
        file: &'a mut std::fs::File,
        count: usize,
        check: F,
    }
    impl<F: FnMut() -> E2eResult<()>> std::io::Write for Writer<'_, F> {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            (self.check)().map_err(std::io::Error::other)?;
            if self.count.saturating_add(bytes.len()) > 32 * 1024 * 1024 {
                return Err(std::io::Error::other(
                    "HAR export exceeds 32 MiB encoded limit",
                ));
            }
            let written = std::io::Write::write(self.file, bytes)?;
            self.count += written;
            Ok(written)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            (self.check)().map_err(std::io::Error::other)?;
            std::io::Write::flush(self.file)
        }
    }
    let mut writer = Writer {
        file: stage.as_file_mut(),
        count: 0,
        check: &mut check,
    };
    serde_json::to_writer_pretty(&mut writer, &document)?;
    std::io::Write::flush(&mut writer)?;
    check()?;
    Ok(stage)
}

#[cfg(test)]
mod option_tests {
    use super::*;
    fn parsed(entries: Value) -> HarFile {
        let raw: HarRaw = serde_json::from_value(json!({"log":{"entries":entries}})).unwrap();
        HarFile {
            entries: raw
                .log
                .entries
                .into_iter()
                .map(|entry| entry.decode().unwrap())
                .collect(),
        }
    }
    fn entry(body: &str, header: &str, result: &str) -> Value {
        json!({"request":{"method":"POST","url":"http://host/data","headers":[{"name":"X-Choice","value":header}],"postData":{"text":body}},"response":{"status":200,"headers":[{"name":"Content-Encoding","value":"gzip"},{"name":"Content-Length","value":"999"}],"content":{"text":result}}})
    }
    fn request(body: &[u8]) -> crate::RouteInfo {
        crate::RouteInfo {
            url: "http://host/data".into(),
            method: "POST".into(),
            headers: vec![("x-choice".into(), "two".into())],
            post_data: Some(body.into()),
            body_state: crate::RouteBodyState::Captured,
            owner: None,
        }
    }
    #[test]
    fn duplicate_selection_uses_body_header_score_and_stable_ties() {
        let replay = HarReplay::new(
            parsed(json!([
                entry("a", "one", "first"),
                entry("a", "two", "second"),
                entry("b", "two", "third"),
                entry("a", "two", "tied")
            ])),
            None,
            HarNotFound::Abort,
        );
        assert_eq!(replay.len(), 4);
        for (body, expected) in [(b"a".as_slice(), b"second".as_slice()), (b"b", b"third")] {
            match replay.action(&request(body)).unwrap() {
                crate::RouteAction::Fulfill { body, headers, .. } => {
                    assert_eq!(body, expected);
                    assert!(headers.is_empty());
                }
                other => panic!("{other:?}"),
            }
        }
        assert!(matches!(
            replay.action(&request(b"missing")).unwrap(),
            crate::RouteAction::Abort
        ));
        let mut unavailable = request(b"a");
        unavailable.body_state = crate::RouteBodyState::Unavailable;
        assert!(replay.action(&unavailable).is_err());
    }
    #[test]
    fn empty_abort_filtered_misses_and_fallback_are_distinct() {
        let options = crate::RouteFromHarOptions::default()
            .matching(crate::UrlMatcher::exact("http://host/data"));
        let filter = crate::url_matcher::har_filter(&options, |_| unreachable!()).unwrap();
        let replay = HarReplay::new(HarFile::default(), filter, HarNotFound::Abort);
        assert!(matches!(
            replay.action(&request(b"a")).unwrap(),
            crate::RouteAction::Abort
        ));
        let mut outside = request(b"a");
        outside.url = "http://host/outside".into();
        assert!(matches!(
            replay.action(&outside).unwrap(),
            crate::RouteAction::Fallback
        ));
        let replay = HarReplay::new(HarFile::default(), None, HarNotFound::Fallback);
        assert!(matches!(
            replay.action(&request(b"a")).unwrap(),
            crate::RouteAction::Fallback
        ));
    }
    #[test]
    fn encoded_body_caps_and_unknown_encoding_fail_before_decode() {
        use base64::Engine;
        for length in [MAX_BODY, MAX_BODY + 1] {
            let encoded = base64::engine::general_purpose::STANDARD.encode(vec![255; length]);
            let result = decode_content(&HarContentRaw {
                text: Some(encoded),
                encoding: Some("base64".into()),
            });
            assert_eq!(result.is_ok(), length == MAX_BODY);
        }
        assert!(decode_content(&HarContentRaw {
            text: Some("text".into()),
            encoding: Some("unknown".into())
        })
        .is_err());
        assert!(serde_json::from_value::<HarRaw>(json!({})).is_err());
        assert!(serde_json::from_value::<HarRaw>(json!({"log":{}})).is_err());
    }
    #[tokio::test]
    async fn abandoned_export_keeps_admission_until_worker_releases_inputs() {
        use futures::FutureExt;
        let _serial = crate::snapshot_work::TEST_ADMISSION_LOCK.lock().await;
        let held = export_permit().await.unwrap();
        let worker_permit = export_permit().await.unwrap();
        let (start, started) = tokio::sync::oneshot::channel();
        let (finish, finished) = tokio::sync::oneshot::channel();
        let (release, released) = std::sync::mpsc::channel();
        let mut work = Box::pin(crate::snapshot_work::run(move |_| {
            let permit = worker_permit;
            let _ = start.send(());
            released.recv().unwrap();
            drop(permit);
            let _ = finish.send(());
            Ok(())
        }));
        assert!(work.as_mut().now_or_never().is_none());
        started.await.unwrap();
        drop(work);
        assert_eq!(
            EXPORTS.available_permits(),
            0,
            "abandonment must not admit more cloned export inputs while CPU work continues"
        );
        release.send(()).unwrap();
        finished.await.unwrap();
        assert_eq!(EXPORTS.available_permits(), 1);
        drop(held);
    }

    #[test]
    fn regular_file_and_entry_count_caps_reject_before_replay() {
        let directory = tempfile::tempdir().unwrap();
        assert!(HarFile::load(directory.path()).is_err());
        let path = directory.path().join("large.har");
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(MAX_FILE as u64 + 1).unwrap();
        assert!(HarFile::load(&path)
            .unwrap_err()
            .to_string()
            .contains("32 MiB"));
        let raw = json!({"log":{"entries":vec![entry("a","one","small");MAX_ENTRIES+1]}});
        std::fs::write(&path, serde_json::to_vec(&raw).unwrap()).unwrap();
        assert!(HarFile::load(&path)
            .unwrap_err()
            .to_string()
            .contains("4096"));
    }

    #[test]
    fn redirect_url_is_used_when_location_is_missing() {
        let file = parsed(
            json!([{"request":{"method":"GET","url":"http://host/from"},"response":{"status":302,"redirectURL":"/to"}}]),
        );
        assert_eq!(
            file.entries[0].headers,
            vec![("Location".into(), "/to".into())]
        );
    }
    #[test]
    fn export_staging_omits_timings_and_preserves_destination_on_abandonment() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("out.har");
        std::fs::write(&path, b"original").unwrap();
        let record = RecordedRequest {
            method: "GET".into(),
            url: "http://host/from".into(),
            status: 302,
            response_headers: vec![("Location".into(), "/to".into())],
            headers: vec![],
            post_data: None,
            duration_ms: Some(7),
            status_text: "Found".into(),
            mime_type: "text/plain".into(),
            started_ms: Some(0),
            request_id: None,
            body: Some(vec![0, 255]),
            body_truncated: false,
        };
        let full = stage_export(
            &path,
            vec![record.clone()],
            &HarExportOptions {
                content: HarContentMode::Embed,
                timing: HarTimingMode::Omit,
                ..Default::default()
            },
            None,
            || Ok(()),
        )
        .unwrap();
        let value: Value = serde_json::from_slice(&std::fs::read(full.path()).unwrap()).unwrap();
        assert_eq!(value["log"]["entries"][0]["timings"]["wait"], json!(-1));
        assert_eq!(
            value["log"]["entries"][0]["response"]["redirectURL"],
            json!("http://host/to")
        );
        let minimal = stage_export(
            &path,
            vec![record],
            &HarExportOptions {
                mode: HarRecordMode::Minimal,
                ..Default::default()
            },
            None,
            || Ok(()),
        )
        .unwrap();
        let value: Value = serde_json::from_slice(&std::fs::read(minimal.path()).unwrap()).unwrap();
        assert!(value["log"]["entries"][0].get("timings").is_none());
        assert!(value["log"].get("pages").is_none());
        assert!(
            stage_export(&path, vec![], &HarExportOptions::default(), None, || Err(
                E2eError::Cancelled("abandoned".into())
            ))
            .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"original");
        drop(full);
        drop(minimal);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}
