//! HAR 1.2 export for recorded network traffic (Playwright `recordHar`).

use serde_json::{json, Value};

use crate::page::RecordedRequest;

/// Build a HAR 1.2 document from recorded requests.
///
/// Bodies are never embedded (equivalent to Playwright's content-`omit`
/// mode): `content.size` is `-1` (unknown) and only headers, timings and
/// MIME types are exported.
pub(crate) fn har_json(requests: &[RecordedRequest]) -> Value {
    let entries: Vec<Value> = requests.iter().map(har_entry).collect();
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
fn har_entry(request: &RecordedRequest) -> Value {
    let started = request.started_ms.unwrap_or(0);
    let time = request.duration_ms.unwrap_or(0);
    let (path, query) = split_query(&request.url);
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
            "content": { "size": -1, "mimeType": request.mime_type },
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
fn iso8601(epoch_ms: u64) -> String {
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
}
