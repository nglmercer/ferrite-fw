//! Owned, bounded native console/error previews without remote handle ownership.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const MAX_ARGUMENTS: usize = 64;
const MAX_PAYLOAD: usize = 32 * 1024;
const MAX_STRING: usize = 4096;
const MAX_FRAMES: usize = 64;

/// A JSON value, a protocol preview, or an explicitly non-JSON/absent value.
/// The tagged wrapper preserves JSON null across serialization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", content = "value", rename_all = "snake_case")]
pub enum ConsoleArgumentValue {
    Json(Value),
    /// Native CDP property previews or BiDi typed values; not a complete object.
    Preview(Value),
    /// Native spelling, including undefined, NaN, Infinity, -0 and bigints.
    Unserializable(String),
    /// The native event supplied no by-value representation.
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConsoleArgument {
    pub kind: String,
    pub subtype: Option<String>,
    pub class_name: Option<String>,
    pub description: Option<String>,
    pub value: ConsoleArgumentValue,
    /// The event referenced a remote object. Its identifier is never retained.
    pub remote_reference: bool,
    pub truncated: bool,
}

/// At most 64 arguments and 32 KiB of serialized preview data per message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConsoleArguments {
    pub values: Vec<ConsoleArgument>,
    pub dropped_arguments: usize,
    pub truncated: bool,
}

/// Native stack entry. Positions are zero-based; unavailable fields stay None.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorStackFrame {
    pub function_name: Option<String>,
    pub url: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub async_stack: bool,
    pub async_description: Option<String>,
}

/// Native error metadata, bounded independently of the legacy rendered text.
/// Constructor class is distinct from a mutable JavaScript Error.name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageErrorInfo {
    pub name: Option<String>,
    pub message: Option<String>,
    pub class_name: Option<String>,
    pub description: Option<String>,
    pub stack: Option<String>,
    pub frames: Vec<ErrorStackFrame>,
    pub thrown: Option<ConsoleArgument>,
    pub truncated: bool,
}

struct Budget {
    bytes: usize,
    nodes: usize,
    truncated: bool,
}
impl Budget {
    fn new() -> Self {
        Self {
            bytes: 16 * 1024,
            nodes: 256,
            truncated: false,
        }
    }
    fn string(&mut self, input: &str) -> String {
        let mut length = 0;
        let mut encoded = 2;
        for ch in input.chars() {
            let cost = match ch {
                '"' | '\\' | '\n' | '\r' | '\t' => 2,
                c if c.is_control() => 6,
                c => c.len_utf8(),
            };
            if length + ch.len_utf8() > MAX_STRING || encoded + cost > self.bytes {
                break;
            }
            length += ch.len_utf8();
            encoded += cost;
        }
        self.bytes = self.bytes.saturating_sub(encoded);
        self.truncated |= length != input.len();
        input[..length].to_owned()
    }
    fn optional(&mut self, input: &Value) -> Option<String> {
        input.as_str().map(|input| self.string(input))
    }
    fn value(&mut self, input: &Value, depth: usize, native: bool) -> Value {
        // BiDi mapping/entry/typed-value wrappers add several encoding levels
        // per JavaScript object. Keep ordinary JSON depth bounded separately.
        if self.nodes == 0 || self.bytes < 8 || depth > if native { 24 } else { 6 } {
            self.truncated = true;
            return json!({"$ferrite":"truncated"});
        }
        self.nodes -= 1;
        self.bytes = self.bytes.saturating_sub(8);
        match input {
            Value::String(s) => Value::String(self.string(s)),
            Value::Array(items) => {
                let mut values = Vec::new();
                for item in items.iter().take(32) {
                    if self.nodes == 0 || self.bytes < 8 {
                        self.truncated = true;
                        break;
                    }
                    values.push(self.value(item, depth + 1, native));
                }
                self.truncated |= values.len() != items.len();
                Value::Array(values)
            }
            Value::Object(items) => {
                let mut values = serde_json::Map::new();
                for (key, item) in items {
                    if native
                        && matches!(
                            key.as_str(),
                            "objectId"
                                | "handle"
                                | "internalId"
                                | "sharedId"
                                | "weakLocalObjectReference"
                        )
                    {
                        continue;
                    }
                    if values.len() == 32 || self.nodes == 0 || self.bytes < 8 {
                        self.truncated = true;
                        break;
                    }
                    let key = self.string(key);
                    values.insert(key, self.value(item, depth + 1, native));
                }
                if native && remote_reference(input) {
                    values.insert("remote_reference".into(), Value::Bool(true));
                }
                if input.get("overflow").and_then(Value::as_bool) == Some(true) {
                    self.truncated = true;
                }
                Value::Object(values)
            }
            other => other.clone(),
        }
    }
}
fn remote_reference(value: &Value) -> bool {
    ["objectId", "handle", "internalId", "sharedId"]
        .iter()
        .any(|key| value.get(key).is_some())
}
fn argument(input: &Value, bidi: bool, budget: &mut Budget) -> ConsoleArgument {
    // Track each argument's truncation separately while keeping one shared budget.
    let earlier = budget.truncated;
    budget.truncated = false;
    let kind = budget
        .optional(&input["type"])
        .unwrap_or_else(|| "unknown".into());
    let subtype = budget.optional(&input["subtype"]);
    let class_name = budget.optional(&input["className"]);
    let description = budget.optional(&input["description"]);
    let value = if kind == "undefined" {
        ConsoleArgumentValue::Unserializable("undefined".into())
    } else if kind == "null" || subtype.as_deref() == Some("null") {
        ConsoleArgumentValue::Json(Value::Null)
    } else if let Some(special) = input["unserializableValue"].as_str() {
        ConsoleArgumentValue::Unserializable(budget.string(special))
    } else if bidi && (kind == "bigint" || (kind == "number" && input["value"].is_string())) {
        input["value"]
            .as_str()
            .map(|value| ConsoleArgumentValue::Unserializable(budget.string(value)))
            .unwrap_or(ConsoleArgumentValue::Unavailable)
    } else if bidi && !matches!(kind.as_str(), "string" | "number" | "boolean") {
        if input.get("value").is_some() {
            ConsoleArgumentValue::Preview(budget.value(input, 0, true))
        } else {
            ConsoleArgumentValue::Unavailable
        }
    } else if let Some(value) = input.get("value") {
        ConsoleArgumentValue::Json(budget.value(value, 0, false))
    } else if let Some(value) = input.get("preview") {
        ConsoleArgumentValue::Preview(budget.value(value, 0, true))
    } else {
        ConsoleArgumentValue::Unavailable
    };
    let truncated = budget.truncated;
    budget.truncated |= earlier;
    ConsoleArgument {
        kind,
        subtype,
        class_name,
        description,
        value,
        remote_reference: remote_reference(input),
        truncated,
    }
}
pub(crate) fn arguments(input: &Value, bidi: bool) -> Option<ConsoleArguments> {
    let input = input.as_array()?;
    let mut budget = Budget::new();
    let mut result = ConsoleArguments {
        values: input
            .iter()
            .take(MAX_ARGUMENTS)
            .map(|value| argument(value, bidi, &mut budget))
            .collect(),
        dropped_arguments: input.len().saturating_sub(MAX_ARGUMENTS),
        truncated: false,
    };
    result.truncated = budget.truncated || result.dropped_arguments != 0;
    while serde_json::to_vec(&result).map_or(true, |data| data.len() > MAX_PAYLOAD)
        && !result.values.is_empty()
    {
        result.values.pop();
        result.dropped_arguments += 1;
        result.truncated = true;
    }
    Some(result)
}
pub(crate) fn error(details: &Value, bidi: bool) -> PageErrorInfo {
    let mut budget = Budget::new();
    let exception = &details["exception"];
    // Only native own string properties establish Error.name/message/stack.
    // A constructor name or formatted error text does not establish these fields.
    let property = |name: &str| {
        exception["preview"]["properties"]
            .as_array()
            .and_then(|properties| {
                properties
                    .iter()
                    .find(|property| property["name"] == name && property["type"] == "string")
            })
            .and_then(|property| property.get("value"))
            .unwrap_or(&Value::Null)
    };
    let name = budget.optional(property("name"));
    let message = budget.optional(property("message"));
    let class_name = budget.optional(&exception["className"]);
    let stack = budget.optional(property("stack"));
    let description = budget.optional(if bidi {
        &details["text"]
    } else {
        exception.get("description").unwrap_or(&details["text"])
    });
    let mut frames = Vec::new();
    let mut current = details.get("stackTrace");
    let mut stack_depth = 0;
    let mut frames_truncated = false;
    while let Some(stack) = current {
        if stack_depth == 8 {
            frames_truncated = true;
            break;
        }
        if let Some(input) = stack["callFrames"].as_array() {
            for frame in input {
                if frames.len() == MAX_FRAMES {
                    frames_truncated = true;
                    break;
                }
                frames.push(ErrorStackFrame {
                    function_name: budget.optional(&frame["functionName"]),
                    url: budget.optional(&frame["url"]),
                    line: frame["lineNumber"]
                        .as_u64()
                        .and_then(|value| value.try_into().ok()),
                    column: frame["columnNumber"]
                        .as_u64()
                        .and_then(|value| value.try_into().ok()),
                    async_stack: stack_depth != 0,
                    async_description: budget.optional(&stack["description"]),
                });
            }
        }
        current = stack.get("parent");
        stack_depth += 1;
    }
    let thrown = (!exception.is_null()).then(|| argument(exception, false, &mut budget));
    let mut result = PageErrorInfo {
        name,
        message,
        class_name,
        description,
        stack,
        frames,
        thrown,
        truncated: budget.truncated || frames_truncated,
    };
    while serde_json::to_vec(&result).map_or(true, |data| data.len() > MAX_PAYLOAD)
        && !result.frames.is_empty()
    {
        result.frames.pop();
        result.truncated = true;
    }
    if serde_json::to_vec(&result).map_or(true, |data| data.len() > MAX_PAYLOAD) {
        result.thrown = None;
        result.truncated = true;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_wire_values_preserve_null_special_values_previews_and_native_error_fields() {
        for (bidi, source) in [
            (false, include_str!("../tests/fixtures/console-cdp.json")),
            (true, include_str!("../tests/fixtures/console-bidi.json")),
        ] {
            let events: Vec<Value> = serde_json::from_str(source).unwrap();
            let values = arguments(&events[0]["params"]["args"], bidi).unwrap();
            assert_eq!(values.values.len(), 12);
            assert_eq!(
                values.values[0].value,
                ConsoleArgumentValue::Json(json!("metadata"))
            );
            assert_eq!(
                values.values[1].value,
                ConsoleArgumentValue::Unserializable("undefined".into())
            );
            assert_eq!(
                values.values[2].value,
                ConsoleArgumentValue::Json(Value::Null)
            );
            for (index, text) in [(3, "NaN"), (4, "Infinity"), (5, "-0")] {
                assert_eq!(
                    values.values[index].value,
                    ConsoleArgumentValue::Unserializable(text.into())
                );
            }
            assert_eq!(
                values.values[6].value,
                ConsoleArgumentValue::Unserializable(if bidi { "12" } else { "12n" }.into())
            );
            assert_eq!(values.values[7].value, ConsoleArgumentValue::Unavailable);
            assert_eq!(values.values[8].value, ConsoleArgumentValue::Unavailable);
            assert!(matches!(
                values.values[9].value,
                ConsoleArgumentValue::Preview(_)
            ));
            assert!(
                !values.truncated,
                "ordinary nested wire values must survive native encoding wrappers"
            );
            if bidi {
                let ConsoleArgumentValue::Preview(preview) = &values.values[10].value else {
                    panic!("typed object preview missing")
                };
                assert_eq!(preview["value"][1][1]["value"][0][1]["value"], true);
            }
            let encoded = serde_json::to_string(&values).unwrap();
            assert!(!encoded.contains("native-reference-"));
            assert!(!encoded.contains("internalId") && !encoded.contains("objectId"));
            assert_eq!(
                serde_json::from_str::<ConsoleArguments>(&encoded).unwrap(),
                values,
                "JSON null must remain present through roundtrip"
            );
            let details = if bidi {
                &events[1]["params"]
            } else {
                &events[1]["params"]["exceptionDetails"]
            };
            let info = error(details, bidi);
            assert!(info
                .description
                .as_ref()
                .unwrap()
                .contains("RenamedError: structured error"));
            assert!(!info.frames.is_empty());
            assert_eq!(info.frames[0].url.as_deref(), Some("metadata-fixture.js"));
            assert_eq!(info.frames[0].line, Some(0));
            if bidi {
                assert!(
                    info.name.is_none()
                        && info.message.is_none()
                        && info.class_name.is_none()
                        && info.thrown.is_none()
                );
            } else {
                assert_eq!(info.name.as_deref(), Some("RenamedError"));
                assert_eq!(info.class_name.as_deref(), Some("TypeError"));
                assert_eq!(info.message.as_deref(), Some("structured error"));
                assert!(info.stack.as_ref().unwrap().contains("metadata-fixture.js"));
            }
            assert_eq!(
                serde_json::from_str::<PageErrorInfo>(&serde_json::to_string(&info).unwrap())
                    .unwrap(),
                info
            );
        }
    }
    #[test]
    fn payload_limits_are_utf8_safe_explicit_and_strip_only_native_ownership() {
        let huge = "\"\n😀".repeat(10000);
        let values = arguments(
            &json!((0..200)
                .map(|_| json!({"type":"string","value":huge}))
                .collect::<Vec<_>>()),
            false,
        )
        .unwrap();
        assert!(values.truncated && values.dropped_arguments >= 136);
        assert!(values.values[0].truncated);
        assert!(serde_json::to_vec(&values).unwrap().len() <= MAX_PAYLOAD);
        let mut nested = json!({"type":"object","internalId":"native-secret"});
        for _ in 0..30 {
            nested = json!({"type":"object","value":[["child",nested]]});
        }
        let values = arguments(&json!([nested]), true).unwrap();
        assert!(values.truncated && values.values[0].truncated);
        assert!(!serde_json::to_string(&values)
            .unwrap()
            .contains("native-secret"));
        let values = arguments(
            &json!([{"type":"object","value":{"handle":"user-value","objectId":"user-property"}}]),
            false,
        )
        .unwrap();
        assert_eq!(
            values.values[0].value,
            ConsoleArgumentValue::Json(json!({"handle":"user-value","objectId":"user-property"})),
            "ordinary JSON keys are not native ownership fields"
        );
        assert!(arguments(&Value::Null, false).is_none());
        assert!(arguments(&json!([]), false).unwrap().values.is_empty());
        let info = error(
            &json!({"text":"native text","stackTrace":{"callFrames":[{"functionName":"a","url":"a.js","lineNumber":0,"columnNumber":0}],"parent":{"description":"timer","callFrames":[{"functionName":"b","url":"b.js","lineNumber":2}]}}}),
            false,
        );
        assert_eq!(info.frames.len(), 2);
        assert!(!info.frames[0].async_stack && info.frames[1].async_stack);
        assert_eq!(info.frames[1].async_description.as_deref(), Some("timer"));
        assert!(info.frames[1].column.is_none());
        let info = error(
            &json!({"text":huge,"stackTrace":{"callFrames":(0..100).map(|_|json!({"functionName":huge,"url":huge,"lineNumber":0})).collect::<Vec<_>>()}}),
            true,
        );
        assert!(info.truncated && info.frames.len() <= MAX_FRAMES);
        assert!(serde_json::to_vec(&info).unwrap().len() <= MAX_PAYLOAD);
    }
    #[test]
    fn absent_native_fields_and_older_console_json_remain_unavailable() {
        let message: crate::ConsoleMessage =
            serde_json::from_value(json!({"kind":"log","text":"old"})).unwrap();
        assert!(message.arguments.is_none() && message.error.is_none());
        let trace: crate::TraceEntry =
            serde_json::from_value(json!({"ts_ms":1,"kind":"console","detail":"old"})).unwrap();
        assert!(trace.console.is_none());
        let trace = crate::TraceEntry {
            ts_ms: 1,
            kind: "console".into(),
            detail: "new".into(),
            console: Some(Box::new(crate::ConsoleMessage {
                kind: "log".into(),
                text: "null".into(),
                location: None,
                timestamp_ms: None,
                page_id: None,
                arguments: arguments(&json!([{"type":"null"}]), true),
                error: None,
            })),
        };
        let trace: crate::TraceEntry =
            serde_json::from_value(serde_json::to_value(trace).unwrap()).unwrap();
        assert_eq!(
            trace.console.unwrap().arguments.unwrap().values[0].value,
            ConsoleArgumentValue::Json(Value::Null)
        );
        let info = error(
            &json!({"exception":{"type":"object","className":"Error","description":"not a native name property"}}),
            false,
        );
        assert!(info.name.is_none() && info.message.is_none());
        assert_eq!(info.class_name.as_deref(), Some("Error"));
        let info = error(
            &json!({"exception":{"type":"string","value":"thrown primitive"}}),
            false,
        );
        assert_eq!(
            info.thrown.unwrap().value,
            ConsoleArgumentValue::Json(json!("thrown primitive"))
        );
    }
}
