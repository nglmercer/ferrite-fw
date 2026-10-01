//! Native frame, document readiness and dialog-close observations.
use crate::PageEvent;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};

/// Native frame identity and its latest available metadata; no live page owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameEvent {
    pub page_id: String,
    pub frame_id: String,
    pub parent_frame_id: Option<String>,
    pub url: Option<String>,
    /// Firefox does not supply a frame name through these native events.
    pub name: Option<String>,
    pub document_id: Option<String>,
    pub is_main_frame: bool,
    /// True for fragment/history navigation; false for other event kinds.
    pub same_document: bool,
    /// Descendants removed by their ancestor's native detach are reported once.
    pub detached_with_parent: bool,
    /// Native detach reason, when provided for this frame itself. CDP `swap`
    /// denotes departure from this target session; OOPIF adoption is unsupported.
    pub detach_reason: Option<String>,
}

/// Native dialog closure. Fields omitted by the backend remain unavailable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DialogClosedInfo {
    pub page_id: String,
    /// Native prompt context on Firefox; CDP does not identify the dialog frame.
    pub frame_id: Option<String>,
    pub accepted: Option<bool>,
    pub user_text: Option<String>,
    pub dialog_type: Option<String>,
}

/// Only live frame metadata is retained. Child-first removal owns a subtree,
/// so later native descendant notifications cannot duplicate its detach events.
#[derive(Clone, Default)]
pub(crate) struct FrameEvents {
    root: Option<String>,
    frames: HashMap<String, FrameEvent>,
    children: HashMap<String, BTreeSet<String>>,
}
impl FrameEvents {
    pub(crate) fn contains(&self, id: &str) -> bool {
        self.frames.contains_key(id)
    }
    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }
    fn metadata(&self, page: &str, id: &str) -> FrameEvent {
        let mut frame = self.frames.get(id).cloned().unwrap_or_else(|| FrameEvent {
            page_id: page.into(),
            frame_id: id.into(),
            parent_frame_id: None,
            url: None,
            name: None,
            document_id: None,
            is_main_frame: self.root.as_deref() == Some(id),
            same_document: false,
            detached_with_parent: false,
            detach_reason: None,
        });
        frame.same_document = false;
        frame.detached_with_parent = false;
        frame.detach_reason = None;
        frame
    }
    fn store(&mut self, frame: FrameEvent) {
        if let Some(previous) = self.frames.get(&frame.frame_id) {
            if previous.parent_frame_id != frame.parent_frame_id {
                if let Some(parent) = &previous.parent_frame_id {
                    if let Some(children) = self.children.get_mut(parent) {
                        children.remove(&frame.frame_id);
                    }
                }
            }
        }
        if let Some(parent) = &frame.parent_frame_id {
            self.children
                .entry(parent.clone())
                .or_default()
                .insert(frame.frame_id.clone());
        }
        self.frames.insert(frame.frame_id.clone(), frame);
    }
    pub(crate) fn seed_bidi(&mut self, page: &str) {
        self.root = Some(page.into());
        self.store(self.metadata(page, page));
    }
    pub(crate) fn seed_bidi_tree(&mut self, page: &str, trees: &Value) {
        let mut pending: Vec<(&Value, Option<String>)> = trees
            .as_array()
            .into_iter()
            .flatten()
            .map(|tree| (tree, None))
            .collect();
        while let Some((tree, parent)) = pending.pop() {
            let Some(id) = tree["context"].as_str() else {
                continue;
            };
            if !self.contains(id) {
                let mut frame = self.metadata(page, id);
                frame.parent_frame_id = tree["parent"].as_str().map(str::to_owned).or(parent);
                frame.url = tree["url"].as_str().map(str::to_owned);
                self.store(frame);
            } else if let Some(frame) = self.frames.get_mut(id) {
                if frame.url.is_none() {
                    frame.url = tree["url"].as_str().map(str::to_owned);
                }
            }
            if let Some(children) = tree["children"].as_array() {
                pending.extend(children.iter().map(|child| (child, Some(id.into()))));
            }
        }
    }
    pub(crate) fn seed_cdp(&mut self, page: &str, tree: &Value) {
        self.root = tree["frame"]["id"].as_str().map(str::to_owned);
        let mut pending = vec![tree];
        while let Some(tree) = pending.pop() {
            let native = &tree["frame"];
            if let Some(id) = native["id"].as_str() {
                if !self.contains(id) {
                    let mut frame = self.metadata(page, id);
                    frame.parent_frame_id = native["parentId"].as_str().map(str::to_owned);
                    frame.url = cdp_url(native);
                    frame.name = native["name"].as_str().map(str::to_owned);
                    frame.document_id = native["loaderId"].as_str().map(str::to_owned);
                    self.store(frame);
                }
            }
            if let Some(children) = tree["childFrames"].as_array() {
                pending.extend(children);
            }
        }
    }
    fn detach(&mut self, id: &str, reason: Option<&str>) -> Vec<PageEvent> {
        if !self.contains(id) {
            return Vec::new();
        }
        let mut pending = vec![id.to_owned()];
        let mut removal = Vec::new();
        while let Some(id) = pending.pop() {
            if let Some(children) = self.children.get(&id) {
                pending.extend(children.iter().cloned());
            }
            removal.push(id);
        }
        let mut events = Vec::new();
        for removed in removal.into_iter().rev() {
            if let Some(mut frame) = self.frames.remove(&removed) {
                if let Some(parent) = &frame.parent_frame_id {
                    if let Some(children) = self.children.get_mut(parent) {
                        children.remove(&removed);
                    }
                }
                self.children.remove(&removed);
                frame.same_document = false;
                frame.detached_with_parent = removed != id;
                frame.detach_reason = if removed == id {
                    reason.map(str::to_owned)
                } else {
                    None
                };
                events.push(PageEvent::FrameDetached(frame));
            }
        }
        events
    }
    pub(crate) fn cdp(&mut self, page: &str, method: &str, params: &Value) -> Vec<PageEvent> {
        let event = match method {
            "Page.frameAttached" => {
                let (Some(id), Some(parent)) =
                    (params["frameId"].as_str(), params["parentFrameId"].as_str())
                else {
                    return Vec::new();
                };
                if self.contains(id) {
                    return Vec::new();
                }
                let mut frame = self.metadata(page, id);
                frame.parent_frame_id = Some(parent.into());
                self.store(frame.clone());
                PageEvent::FrameAttached(frame)
            }
            "Page.frameNavigated" => {
                let native = &params["frame"];
                let Some(id) = native["id"].as_str() else {
                    return Vec::new();
                };
                let parent = native["parentId"].as_str().map(str::to_owned);
                if parent.is_none() {
                    self.root = Some(id.into());
                }
                let mut frame = self.metadata(page, id);
                frame.parent_frame_id = parent;
                frame.is_main_frame = self.root.as_deref() == Some(id);
                frame.url = cdp_url(native);
                frame.name = native["name"].as_str().map(str::to_owned);
                frame.document_id = native["loaderId"].as_str().map(str::to_owned);
                self.store(frame.clone());
                PageEvent::FrameNavigated(frame)
            }
            "Page.navigatedWithinDocument" => {
                let Some(id) = params["frameId"].as_str() else {
                    return Vec::new();
                };
                let mut frame = self.metadata(page, id);
                frame.url = params["url"].as_str().map(str::to_owned);
                self.store(frame.clone());
                frame.same_document = true;
                PageEvent::FrameNavigated(frame)
            }
            "Page.frameDetached" => {
                let Some(id) = params["frameId"].as_str() else {
                    return Vec::new();
                };
                return self.detach(id, params["reason"].as_str());
            }
            "Page.domContentEventFired" | "Page.loadEventFired" => {
                let Some(root) = self.root.as_deref() else {
                    return Vec::new();
                };
                let frame = self.metadata(page, root);
                if method == "Page.domContentEventFired" {
                    PageEvent::DomContentLoaded(frame)
                } else {
                    PageEvent::Load(frame)
                }
            }
            "Page.javascriptDialogClosed" => PageEvent::DialogClosed(DialogClosedInfo {
                page_id: page.into(),
                frame_id: None,
                accepted: params["result"].as_bool(),
                user_text: params["userInput"].as_str().map(str::to_owned),
                dialog_type: None,
            }),
            _ => return Vec::new(),
        };
        vec![event]
    }
    pub(crate) fn bidi(&mut self, page: &str, method: &str, params: &Value) -> Vec<PageEvent> {
        let Some(id) = params["context"].as_str() else {
            return Vec::new();
        };
        let event = match method {
            "browsingContext.contextCreated" => {
                if self.contains(id) {
                    return Vec::new();
                }
                let mut frame = self.metadata(page, id);
                frame.parent_frame_id = params["parent"].as_str().map(str::to_owned);
                frame.url = params["url"].as_str().map(str::to_owned);
                self.store(frame.clone());
                PageEvent::FrameAttached(frame)
            }
            "browsingContext.navigationCommitted"
            | "browsingContext.fragmentNavigated"
            | "browsingContext.historyUpdated" => {
                let mut frame = self.metadata(page, id);
                frame.url = params["url"].as_str().map(str::to_owned);
                let same_document = method != "browsingContext.navigationCommitted";
                if !same_document {
                    frame.document_id = params["navigation"].as_str().map(str::to_owned);
                }
                self.store(frame.clone());
                frame.same_document = same_document;
                PageEvent::FrameNavigated(frame)
            }
            "browsingContext.contextDestroyed" => {
                if self.root.as_deref() == Some(id) {
                    self.clear();
                    return Vec::new();
                }
                return self.detach(id, None);
            }
            "browsingContext.domContentLoaded" | "browsingContext.load" => {
                // Page readiness events describe its main document, not every iframe.
                if self.root.as_deref() != Some(id) {
                    return Vec::new();
                }
                let frame = self.metadata(page, id);
                if method == "browsingContext.domContentLoaded" {
                    PageEvent::DomContentLoaded(frame)
                } else {
                    PageEvent::Load(frame)
                }
            }
            "browsingContext.userPromptClosed" => PageEvent::DialogClosed(DialogClosedInfo {
                page_id: page.into(),
                frame_id: Some(id.into()),
                accepted: params["accepted"].as_bool(),
                user_text: params["userText"].as_str().map(str::to_owned),
                dialog_type: params["type"].as_str().map(str::to_owned),
            }),
            _ => return Vec::new(),
        };
        vec![event]
    }
}
fn cdp_url(frame: &Value) -> Option<String> {
    let mut url = frame["url"].as_str()?.to_owned();
    if let Some(fragment) = frame["urlFragment"].as_str() {
        url.push_str(fragment);
    }
    Some(url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cdp_native_root_identity_fragments_and_subtree_removal_do_not_retarget() {
        let mut frames = FrameEvents::default();
        frames.seed_cdp("page",&json!({"frame":{"id":"root","url":"http://fixture/main","loaderId":"document","name":""}}));
        let ready = frames.cdp("page", "Page.domContentEventFired", &json!({}));
        assert!(
            matches!(&ready[0],PageEvent::DomContentLoaded(frame) if frame.frame_id=="root"&&frame.page_id=="page"&&frame.is_main_frame)
        );
        for (id, parent) in [("outer", "root"), ("nested", "outer")] {
            assert_eq!(
                frames
                    .cdp(
                        "page",
                        "Page.frameAttached",
                        &json!({"frameId":id,"parentFrameId":parent})
                    )
                    .len(),
                1
            );
            assert!(frames
                .cdp(
                    "page",
                    "Page.frameAttached",
                    &json!({"frameId":id,"parentFrameId":parent})
                )
                .is_empty());
        }
        let navigation=frames.cdp("page","Page.frameNavigated",&json!({"frame":{"id":"outer","parentId":"root","url":"http://fixture/child","urlFragment":"#fragment","loaderId":"child-document"}}));
        assert!(
            matches!(&navigation[0],PageEvent::FrameNavigated(frame) if frame.url.as_deref()==Some("http://fixture/child#fragment")&&!frame.same_document)
        );
        for _ in 0..2 {
            let navigation = frames.cdp(
                "page",
                "Page.navigatedWithinDocument",
                &json!({"frameId":"outer","url":"http://fixture/child#fragment"}),
            );
            assert!(
                matches!(&navigation[0],PageEvent::FrameNavigated(frame) if frame.same_document&&frame.document_id.as_deref()==Some("child-document"))
            );
        }
        let detached = frames.cdp(
            "page",
            "Page.frameDetached",
            &json!({"frameId":"outer","reason":"remove"}),
        );
        assert_eq!(detached.len(), 2);
        assert!(
            matches!(&detached[0],PageEvent::FrameDetached(frame) if frame.frame_id=="nested"&&frame.detached_with_parent&&frame.detach_reason.is_none())
        );
        assert!(
            matches!(&detached[1],PageEvent::FrameDetached(frame) if frame.frame_id=="outer"&&!frame.detached_with_parent&&frame.detach_reason.as_deref()==Some("remove"))
        );
        assert!(frames
            .cdp("page", "Page.frameDetached", &json!({"frameId":"nested"}))
            .is_empty());
        assert_eq!(frames.frames.len(), 1);
        assert!(!frames.contains("outer") && !frames.contains("nested"));
    }

    #[test]
    fn bidi_native_tree_tracks_parent_identity_and_only_main_document_readiness() {
        let mut frames = FrameEvents::default();
        frames.seed_bidi("root");
        frames.seed_bidi_tree("root",&json!([{"context":"root","url":"about:blank","children":[{"context":"child","url":"http://fixture/child","children":[]}]}]));
        let navigation = frames.bidi(
            "root",
            "browsingContext.navigationCommitted",
            &json!({"context":"child","url":"http://fixture/new","navigation":"document"}),
        );
        assert!(
            matches!(&navigation[0],PageEvent::FrameNavigated(frame) if frame.parent_frame_id.as_deref()==Some("root")&&frame.name.is_none()&&!frame.is_main_frame)
        );
        assert!(frames
            .bidi("root", "browsingContext.load", &json!({"context":"child"}))
            .is_empty());
        assert_eq!(
            frames
                .bidi("root", "browsingContext.load", &json!({"context":"root"}))
                .len(),
            1
        );
        let detach = frames.bidi(
            "root",
            "browsingContext.contextDestroyed",
            &json!({"context":"child"}),
        );
        assert!(
            matches!(&detach[0],PageEvent::FrameDetached(frame) if frame.frame_id=="child"&&frame.url.as_deref()==Some("http://fixture/new"))
        );
        assert!(frames
            .bidi(
                "root",
                "browsingContext.contextDestroyed",
                &json!({"context":"root"})
            )
            .is_empty());
        assert!(frames.frames.is_empty() && frames.children.is_empty());
    }

    #[test]
    fn dialog_close_preserves_backend_absence_and_missing_frame_ids_produce_no_event() {
        let mut frames = FrameEvents::default();
        let cdp = frames.cdp(
            "page",
            "Page.javascriptDialogClosed",
            &json!({"result":false,"userInput":""}),
        );
        assert!(
            matches!(&cdp[0],PageEvent::DialogClosed(info) if info.accepted==Some(false)&&info.frame_id.is_none()&&info.dialog_type.is_none()&&info.user_text.as_deref()==Some(""))
        );
        let bidi = frames.bidi(
            "page",
            "browsingContext.userPromptClosed",
            &json!({"context":"child","type":"confirm","accepted":true}),
        );
        assert!(
            matches!(&bidi[0],PageEvent::DialogClosed(info) if info.accepted==Some(true)&&info.frame_id.as_deref()==Some("child")&&info.user_text.is_none())
        );
        assert!(frames
            .cdp("page", "Page.frameNavigated", &json!({}))
            .is_empty());
        assert!(frames
            .cdp("page", "Page.frameAttached", &json!({"frameId":"child"}))
            .is_empty());
        assert!(frames
            .bidi("page", "browsingContext.navigationCommitted", &json!({}))
            .is_empty());
    }
}
