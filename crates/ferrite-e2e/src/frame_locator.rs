//! Lazy iframe selection for same-origin documents, including nested frames.

use crate::{GetByRoleOptions, Locator, Page, Selector};

/// Resolve an iframe afresh for every action or assertion, so replacements do
/// not leave stale frame IDs. Cross-origin evaluation requires the explicit
/// protocol-backed [`crate::Frame`] API.
#[derive(Clone, Debug)]
pub struct FrameLocator {
    page: Page,
    selector: Selector,
    owner_description: Option<String>,
}

impl FrameLocator {
    pub(crate) fn new(page: Page, selector: Selector) -> Self {
        Self {
            page,
            selector,
            owner_description: None,
        }
    }

    pub(crate) fn with_owner_description(mut self, description: Option<String>) -> Self {
        self.owner_description = description;
        self
    }

    fn content_page(&self) -> Page {
        let mut page = self.page.clone();
        page.lazy_frames.push(self.selector.clone());
        page
    }

    /// Locator for the iframe element in its parent document.
    pub fn owner(&self) -> Locator {
        let owner = Locator::new(self.page.clone(), self.selector.clone());
        match &self.owner_description {
            Some(description) => owner.describe(description),
            None => owner,
        }
    }

    pub fn first(&self) -> Self {
        Self::new(self.page.clone(), self.owner().first().selector_value())
    }
    pub fn last(&self) -> Self {
        Self::new(self.page.clone(), self.owner().last().selector_value())
    }
    pub fn nth(&self, index: usize) -> Self {
        Self::new(self.page.clone(), self.owner().nth(index).selector_value())
    }

    /// Locate a nested frame.
    pub fn frame_locator(&self, selector: &str) -> Self {
        Self::new(self.content_page(), Selector::parse(selector.to_string()))
    }
    pub fn locator(&self, selector: &str) -> Locator {
        self.content_page().locator(selector)
    }
    pub fn get_by_role(&self, role: &str, name: &str) -> Locator {
        self.content_page().get_by_role(role, name)
    }
    pub fn get_by_role_with(&self, role: &str, options: GetByRoleOptions) -> Locator {
        self.content_page().get_by_role_with(role, options)
    }
    pub fn get_by_text(&self, text: &str) -> Locator {
        self.content_page().get_by_text(text)
    }
    pub fn get_by_label(&self, text: &str) -> Locator {
        self.content_page().get_by_label(text)
    }
    pub fn get_by_test_id(&self, id: &str) -> Locator {
        self.content_page().get_by_test_id(id)
    }
    pub fn get_by_placeholder(&self, text: &str) -> Locator {
        self.content_page().get_by_placeholder(text)
    }
    pub fn get_by_alt(&self, text: &str) -> Locator {
        self.content_page().get_by_alt(text)
    }
    pub fn get_by_title(&self, text: &str) -> Locator {
        self.content_page().get_by_title(text)
    }
}
