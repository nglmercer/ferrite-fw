use crate::{E2eError, E2eResult};

/// Options for bounded DOM accessibility snapshots (not a native accessibility tree).
/// Positive `depth` counts role nodes, with roots at zero; zero/negative means no
/// requested depth limit. Transparent wrappers do not consume role depth.
/// A safety depth of 60, node/DOM visit budgets and name length limits always apply.
/// Exhausted safety budgets append a `truncated` node; requested depth simply
/// omits descendants. Names are truncated in UTF-16 code units with an ellipsis.
/// Traversal budgets do not preempt native layout or accessible-name computation.
/// The existing no-options methods retain their historical unbounded output.
/// Boxes are rounded CSS pixels in the root's own frame viewport.
#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AriaSnapshotOptions {
    pub depth: i32,
    pub boxes: bool,
    /// Include supported state fields and heading levels. Defaults to true.
    pub states: bool,
    pub max_nodes: usize,
    pub max_dom_nodes: usize,
    pub max_name_chars: usize,
}
impl Default for AriaSnapshotOptions {
    fn default() -> Self {
        Self {
            depth: 0,
            boxes: false,
            states: true,
            max_nodes: 1000,
            max_dom_nodes: 10000,
            max_name_chars: 4096,
        }
    }
}
impl AriaSnapshotOptions {
    pub(crate) fn json(self) -> E2eResult<String> {
        if self.max_nodes == 0 || self.max_dom_nodes == 0 || self.max_name_chars == 0 {
            return Err(E2eError::Config(
                "ARIA snapshot budgets must be positive".into(),
            ));
        }
        if [self.max_nodes, self.max_dom_nodes, self.max_name_chars]
            .iter()
            .any(|value| *value as u128 > 9_007_199_254_740_991)
        {
            return Err(E2eError::Config(
                "ARIA snapshot budgets must be JavaScript safe integers".into(),
            ));
        }
        serde_json::to_string(&self).map_err(|error| E2eError::Config(error.to_string()))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn options_validate_and_serialize() {
        let options = AriaSnapshotOptions::default();
        assert!(options.json().unwrap().contains("\"maxDomNodes\":10000"));
        assert!(AriaSnapshotOptions {
            max_nodes: 0,
            ..options
        }
        .json()
        .is_err());
        assert!(AriaSnapshotOptions {
            max_dom_nodes: 0,
            ..options
        }
        .json()
        .is_err());
        assert!(AriaSnapshotOptions {
            max_name_chars: 0,
            ..options
        }
        .json()
        .is_err());
        assert!(AriaSnapshotOptions {
            depth: -1,
            ..options
        }
        .json()
        .is_ok());
    }
}
