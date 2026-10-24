//! Flatten embedded ECMA-426 index-map sections before map composition.
use ferrite_core::{FerriteError, Result};
use oxc_sourcemap::{SourceMap, Token};
use serde_json::Value;
use std::borrow::Cow;

type Position = (u32, u32);
struct Leaf {
    map: SourceMap<'static>,
    start: Position,
    end: Option<Position>,
}
fn invalid(message: impl std::fmt::Display) -> FerriteError {
    FerriteError::Transform {
        id: "source-map".into(),
        message: format!("invalid indexed source map: {message}"),
    }
}
fn position(value: &Value) -> Result<Position> {
    let number = |key| {
        value
            .get(key)
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| invalid("section offsets require nonnegative u32 line and column"))
    };
    Ok((number("line")?, number("column")?))
}
fn offset(parent: Position, child: Position) -> Result<Position> {
    Ok((
        parent
            .0
            .checked_add(child.0)
            .ok_or_else(|| invalid("line offset overflow"))?,
        if child.0 == 0 {
            parent
                .1
                .checked_add(child.1)
                .ok_or_else(|| invalid("column offset overflow"))?
        } else {
            child.1
        },
    ))
}
fn collect(
    value: &Value,
    start: Position,
    end: Option<Position>,
    depth: usize,
    leaves: &mut Vec<Leaf>,
) -> Result<()> {
    if depth > 64 {
        return Err(invalid("section nesting exceeds 64 levels"));
    }
    if value.get("version").and_then(Value::as_u64) != Some(3) {
        return Err(invalid("only version 3 maps are supported"));
    }
    if let Some(sections) = value.get("sections") {
        if [
            "sources",
            "sourcesContent",
            "names",
            "mappings",
            "sourceRoot",
        ]
        .iter()
        .any(|key| value.get(key).is_some())
        {
            return Err(invalid(
                "an indexed source map cannot also contain regular mapping fields",
            ));
        }
        let sections = sections
            .as_array()
            .ok_or_else(|| invalid("sections must be an array"))?;
        let positions: Vec<_> = sections
            .iter()
            .map(|section| {
                position(
                    section
                        .get("offset")
                        .ok_or_else(|| invalid("section offset is required"))?,
                )
                .and_then(|child| offset(start, child))
            })
            .collect::<Result<_>>()?;
        for pair in positions.windows(2) {
            if pair[0] > pair[1] {
                return Err(invalid("sections must have ordered nonoverlapping offsets"));
            }
        }
        for (index, section) in sections.iter().enumerate() {
            if section.get("url").is_some() {
                return Err(invalid("external URL sections require explicit fetching by the producing plugin; return an embedded map section"));
            }
            let next = positions.get(index + 1).copied().or(end);
            if end.is_some_and(|end| positions[index] > end) {
                return Err(invalid("section exceeds its enclosing section"));
            }
            let map = section
                .get("map")
                .ok_or_else(|| invalid("embedded section map is required"))?;
            // Even an empty section terminates mappings from the previous one.
            leaves.push(Leaf {
                map: SourceMap::default(),
                start: positions[index],
                end: next,
            });
            collect(map, positions[index], next, depth + 1, leaves)?;
        }
    } else {
        let json = value.to_string();
        let map = SourceMap::from_json_string(&json)
            .map_err(invalid)?
            .into_owned();
        leaves.push(Leaf { map, start, end });
    }
    Ok(())
}

/// Validate a regular map or flatten embedded indexed sections into regular v3.
/// Offset columns apply only to the section's first generated line. Source
/// contents, names, roots and ignore lists survive without deduplication loss.
pub fn flatten_source_map(json: &str) -> Result<String> {
    let value: Value = serde_json::from_str(json).map_err(invalid)?;
    if value.get("sections").is_none() {
        SourceMap::from_json_string(json).map_err(invalid)?;
        return Ok(json.to_string());
    }
    let mut leaves = Vec::new();
    collect(&value, (0, 0), None, 0, &mut leaves)?;
    let mut sources = Vec::new();
    let mut contents = Vec::new();
    let mut names = Vec::new();
    let mut tokens = Vec::new();
    let mut ignored = Vec::new();
    for leaf in leaves {
        let source_offset = u32::try_from(sources.len()).map_err(invalid)?;
        let name_offset = u32::try_from(names.len()).map_err(invalid)?;
        let source_count = leaf.map.get_sources().len() as u32;
        let name_count = leaf.map.get_names().len() as u32;
        let root = leaf.map.get_source_root().unwrap_or("");
        for source in leaf.map.get_sources() {
            let path = if root.is_empty() || source.starts_with('/') || source.contains("://") {
                source.to_string()
            } else {
                format!("{}/{}", root.trim_end_matches('/'), source)
            };
            sources.push(Cow::Owned(path));
        }
        for index in 0..source_count {
            contents.push(
                leaf.map
                    .get_source_content(index)
                    .map(|content| Cow::Owned(content.to_string())),
            );
        }
        names.extend(
            leaf.map
                .get_names()
                .map(|name| Cow::Owned(name.to_string())),
        );
        if let Some(indices) = leaf.map.get_x_google_ignore_list() {
            for index in indices {
                if *index >= source_count {
                    return Err(invalid("ignore-list source index is out of bounds"));
                }
                ignored.push(source_offset + index);
            }
        }
        tokens.push(Token::new(leaf.start.0, leaf.start.1, 0, 0, None, None));
        for token in leaf.map.get_tokens() {
            let point = offset(leaf.start, (token.get_dst_line(), token.get_dst_col()))?;
            if leaf.end.is_some_and(|end| point > end) {
                return Err(invalid("section mappings overlap the following section"));
            }
            let source = token
                .get_source_id()
                .map(|index| {
                    if index < source_count {
                        Ok(index + source_offset)
                    } else {
                        Err(invalid("source index is out of bounds"))
                    }
                })
                .transpose()?;
            let name = token
                .get_name_id()
                .map(|index| {
                    if index < name_count {
                        Ok(index + name_offset)
                    } else {
                        Err(invalid("name index is out of bounds"))
                    }
                })
                .transpose()?;
            tokens.push(Token::new(
                point.0,
                point.1,
                token.get_src_line(),
                token.get_src_col(),
                source,
                name,
            ));
        }
    }
    tokens.sort_by_key(|token| (token.get_dst_line(), token.get_dst_col()));
    // Keep the last token at each position: actual section mappings override
    // the boundary's unmapped marker, never the other way around.
    tokens.reverse();
    tokens.dedup_by_key(|token| (token.get_dst_line(), token.get_dst_col()));
    tokens.reverse();
    let mut map = SourceMap::new(
        value
            .get("file")
            .and_then(Value::as_str)
            .map(|file| Cow::Owned(file.into())),
        names,
        None,
        sources,
        contents,
        tokens.into_boxed_slice(),
        None,
    );
    if !ignored.is_empty() {
        map.set_x_google_ignore_list(ignored);
    }
    if let Some(id) = value.get("debugId").and_then(Value::as_str) {
        map.set_debug_id(id);
    }
    Ok(map.to_json_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_offsets_contents_names_and_unmapped_boundaries_survive_flattening() {
        let leaf = serde_json::json!({"version":3,"sourceRoot":"src","sources":["one.ts"],"sourcesContent":["one"],"names":["name"],"mappings":"AAAAA;AACA","x_google_ignoreList":[0]});
        let indexed = serde_json::json!({"version":3,"file":"out.js","sections":[{"offset":{"line":1,"column":7},"map":{"version":3,"sections":[{"offset":{"line":0,"column":3},"map":leaf}]}},{"offset":{"line":4,"column":2},"map":{"version":3,"sources":[],"names":[],"mappings":""}}]});
        let json = flatten_source_map(&indexed.to_string()).unwrap();
        let map = SourceMap::from_json_string(&json).unwrap();
        assert_eq!(map.get_source(0), Some("src/one.ts"));
        assert_eq!(map.get_source_content(0), Some("one"));
        assert_eq!(map.get_name(0), Some("name"));
        let lookup = map.generate_lookup_table();
        assert_eq!(
            map.lookup_source_view_token(&lookup, 1, 10)
                .unwrap()
                .get_source(),
            Some("src/one.ts")
        );
        assert_eq!(
            map.lookup_source_view_token(&lookup, 2, 0)
                .unwrap()
                .get_src_line(),
            1
        );
        assert!(map
            .lookup_source_view_token(&lookup, 4, 2)
            .unwrap()
            .get_source()
            .is_none());
        assert_eq!(map.get_x_google_ignore_list(), Some(&[0][..]));
    }
    #[test]
    fn equal_section_boundaries_use_the_later_mapping_and_compose() {
        let leaf = |source: &str| serde_json::json!({"version":3,"sources":[source],"sourcesContent":["original"],"names":[],"mappings":"AAAA"});
        let indexed = serde_json::json!({"version":3,"sections":[
            {"offset":{"line":0,"column":0},"map":leaf("first.ts")},
            {"offset":{"line":0,"column":0},"map":leaf("last.ts")}
        ]});
        let flat = flatten_source_map(&indexed.to_string()).unwrap();
        let outer = serde_json::json!({"version":3,"sources":["intermediate.js"],"names":[],"mappings":"AAAA"});
        let composed = crate::chain_source_maps(&outer.to_string(), &flat).unwrap();
        let map = SourceMap::from_json_string(&composed).unwrap();
        let lookup = map.generate_lookup_table();
        assert_eq!(
            map.lookup_source_view_token(&lookup, 0, 0)
                .unwrap()
                .get_source(),
            Some("last.ts")
        );
    }
    #[test]
    fn invalid_hybrid_order_overflow_and_external_sections_fail_explicitly() {
        for value in [
            serde_json::json!({"version":3,"sections":[],"mappings":""}),
            serde_json::json!({"version":3,"sections":[{"offset":{"line":0,"column":0},"url":"map.json"}]}),
            serde_json::json!({"version":3,"sections":[{"offset":{"line":-1,"column":0},"map":{}}]}),
            serde_json::json!({"version":3,"sections":[{"offset":{"line":1,"column":0},"map":{"version":3,"sources":[],"names":[],"mappings":""}},{"offset":{"line":0,"column":0},"map":{}}]}),
        ] {
            assert!(flatten_source_map(&value.to_string()).is_err(), "{value}");
        }
    }
}
