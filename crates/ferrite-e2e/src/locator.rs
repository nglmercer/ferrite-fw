//! Selectors (`css`, `text=`, `xpath=`, `role=`) and locator actions.

use std::time::Duration;

use crate::error::{E2eError, E2eResult};
use crate::page::{ClickOptions, ElementState, Page};

/// A parsed selector with an engine and a body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selector {
    raw: String,
    engine: Engine,
    body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Engine {
    Css,
    Text,
    XPath,
    Role,
}

impl Selector {
    /// Parse `css`, `text=`, `xpath=`, or `role=` selectors.
    #[must_use]
    pub fn parse(raw: String) -> Self {
        let (engine, body) = if let Some(body) = raw.strip_prefix("text=") {
            (Engine::Text, body.to_string())
        } else if let Some(body) = raw.strip_prefix("xpath=") {
            (Engine::XPath, body.to_string())
        } else if let Some(body) = raw.strip_prefix("role=") {
            (Engine::Role, body.to_string())
        } else if let Some(body) = raw.strip_prefix("css=") {
            (Engine::Css, body.to_string())
        } else {
            (Engine::Css, raw.clone())
        };
        Self { raw, engine, body }
    }

    /// Original selector text.
    #[must_use]
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// JS that resolves to `Element[]` for this selector.
    fn resolve_js(&self) -> String {
        let body = serde_json::to_string(&self.body).unwrap_or_default();
        match self.engine {
            Engine::Css => format!("[...document.querySelectorAll({body})]"),
            Engine::XPath => format!(
                "(() => {{ const out = []; const it = document.evaluate({body}, \
                 document, null, XPathResult.ORDERED_NODE_SNAPSHOT_TYPE, null); \
                 for (let i = 0; i < it.snapshotLength; i++) {{ \
                 const n = it.snapshotItem(i); \
                 if (n instanceof Element) out.push(n); }} return out; }})()"
            ),
            Engine::Text => format!(
                "(() => {{ const needle = {body}.toLowerCase(); \
                 const els = [...document.querySelectorAll('body *')]; \
                 const hits = els.filter(el => \
                 (el.textContent || '').toLowerCase().includes(needle)); \
                 hits.sort((a, b) => \
                 (a.textContent || '').length - (b.textContent || '').length); \
                 return hits.slice(0, 20); }})()"
            ),
            Engine::Role => {
                let (role, name) = parse_role(&self.body);
                let role_json = serde_json::to_string(&role).unwrap_or_default();
                let name_json = serde_json::to_string(&name).unwrap_or_default();
                format!(
                    "(() => {{ const byRole = {{ button: 'button,[role=\"button\"],\
                     input[type=\"button\"],input[type=\"submit\"]', \
                     link: 'a[href],[role=\"link\"]', \
                     textbox: 'input[type=\"text\"],input:not([type]),textarea,\
                     [role=\"textbox\"]', checkbox: 'input[type=\"checkbox\"],\
                     [role=\"checkbox\"]', radio: 'input[type=\"radio\"],\
                     [role=\"radio\"]', heading: 'h1,h2,h3,h4,h5,h6,\
                     [role=\"heading\"]', img: 'img,[role=\"img\"]', \
                     listbox: 'select,[role=\"listbox\"]', \
                     option: 'option,[role=\"option\"]' }}; \
                     const sel = byRole[{role_json}] || ('[role=' + {role_json} + ']'); \
                     let els = [...document.querySelectorAll(sel)]; \
                     const want = {name_json}.toLowerCase(); \
                     if (want) els = els.filter(el => \
                     ((el.getAttribute('aria-label') || el.textContent || '')\
                     .toLowerCase().includes(want))); \
                     return els; }})()"
                )
            }
        }
    }

    /// JS expression evaluating to an [`ElementState`] snapshot.
    #[must_use]
    pub fn state_expression(&self) -> String {
        let resolve = self.resolve_js();
        format!(
            "(() => {{ const els = {resolve}; const first = els[0]; \
             const visible = el => {{ if (!el) return false; \
             const s = getComputedStyle(el); const r = el.getBoundingClientRect(); \
             return s.display !== 'none' && s.visibility !== 'hidden' \
             && r.width > 0 && r.height > 0; }}; \
             const rects = els.slice(0, 20).map(el => {{ \
             const r = el.getBoundingClientRect(); \
             return {{ x: r.x, y: r.y, width: r.width, height: r.height }}; }}); \
             return {{ count: els.length, \
             visible: visible(first), \
             enabled: first ? !first.disabled : false, \
             checked: first ? Boolean(first.checked) : false, \
             text: first ? (first.textContent || '').trim() : '', \
             value: first && 'value' in first ? String(first.value) : '', \
             rects }}; }})()"
        )
    }

    /// JS expression performing `action` on the first match.
    ///
    /// Returns `{ ok: true, ... }` or `{ ok: false, error }`.
    #[must_use]
    pub fn action_expression(&self, action: &str, argument: Option<&str>) -> String {
        let resolve = self.resolve_js();
        let arg = serde_json::to_string(argument.unwrap_or_default()).unwrap_or_default();
        let fire = "el.dispatchEvent(new Event('input', { bubbles: true })); \
                    el.dispatchEvent(new Event('change', { bubbles: true }));";
        let body = match action {
            "scroll" => "el.scrollIntoView({ block: 'center' }); return { ok: true };",
            "click" => "el.scrollIntoView({ block: 'center' }); el.click(); return { ok: true };",
            "focus" => "el.focus(); return { ok: true };",
            "blur" => "el.blur(); return { ok: true };",
            "fill" => &format!(
                "el.focus(); \
                 if ('value' in el) {{ el.value = {arg}; {fire} \
                 return {{ ok: true }}; }} \
                 return {{ ok: false, error: 'not fillable' }};"
            ),
            "clear" => &format!(
                "el.focus(); \
                 if ('value' in el) {{ el.value = ''; {fire} \
                 return {{ ok: true }}; }} \
                 return {{ ok: false, error: 'not clearable' }};"
            ),
            "check" => &format!(
                "const want = {arg} === 'true'; \
                 if (el.checked === want) return {{ ok: true }}; \
                 el.click(); return {{ ok: el.checked === want }};"
            ),
            "select" => &format!(
                "if (!(el instanceof HTMLSelectElement)) \
                 return {{ ok: false, error: 'not a select' }}; \
                 el.value = {arg}; {fire} \
                 return {{ ok: el.value === {arg} }};"
            ),
            _ => "return { ok: false, error: 'unknown action' };",
        };
        format!(
            "(() => {{ const els = {resolve}; const el = els[0]; \
             if (!el) return {{ ok: false, error: 'no matching element' }}; \
             {body} }})()"
        )
    }
}

/// Parse `role[name="x"]` into (role, name).
fn parse_role(body: &str) -> (String, String) {
    let Some(bracket) = body.find('[') else {
        return (body.trim().to_string(), String::new());
    };
    let role = body[..bracket].trim().to_string();
    let rest = &body[bracket..];
    let name = rest
        .strip_prefix("[name=")
        .and_then(|s| s.strip_suffix(']'))
        .map(|s| s.trim_matches('"').trim_matches('\'').to_string())
        .unwrap_or_default();
    (role, name)
}

/// Options for locator actions.
#[derive(Debug, Clone, Default)]
pub struct LocatorOptions {
    /// Action timeout (defaults to the page timeout).
    pub timeout: Option<Duration>,
}

/// A lazy handle to DOM element(s): actions auto-wait for the target.
#[derive(Clone)]
pub struct Locator {
    page: Page,
    selector: Selector,
}

impl Locator {
    pub(crate) fn new(page: Page, selector: Selector) -> Self {
        Self { page, selector }
    }

    /// Underlying selector text.
    #[must_use]
    pub fn selector(&self) -> &str {
        self.selector.raw()
    }

    /// Number of matching elements.
    pub async fn count(&self) -> E2eResult<usize> {
        Ok(self.page.query_state(&self.selector).await?.count)
    }

    /// Current state snapshot.
    pub async fn state(&self) -> E2eResult<ElementState> {
        self.page.query_state(&self.selector).await
    }

    /// Wait until at least one match exists.
    pub async fn wait_for(&self, timeout: Duration) -> E2eResult<()> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if let Ok(state) = self.page.query_state(&self.selector).await {
                if state.count > 0 {
                    return Ok(());
                }
            }
            if tokio::time::Instant::now() > deadline {
                return Err(E2eError::Timeout(
                    timeout.as_millis().min(u128::from(u64::MAX)) as u64,
                    format!("wait for `{}`", self.selector.raw()),
                ));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    async fn ready_state(&self, options: &LocatorOptions) -> E2eResult<ElementState> {
        let timeout = options.timeout.unwrap_or_else(|| self.page.timeout());
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let state = self.page.query_state(&self.selector).await?;
            if state.count > 0 && state.visible {
                return Ok(state);
            }
            if tokio::time::Instant::now() > deadline {
                let why = if state.count == 0 {
                    "no matching element"
                } else {
                    "element not visible"
                };
                return Err(E2eError::Locator {
                    selector: self.selector.raw().to_string(),
                    message: format!("{why} within {}ms", timeout.as_millis()),
                });
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    fn center(state: &ElementState) -> Option<(f64, f64)> {
        state
            .rects
            .first()
            .map(|r| (r.x + r.width / 2.0, r.y + r.height / 2.0))
    }

    /// Click the element (trusted mouse input by default).
    pub async fn click(&self) -> E2eResult<()> {
        self.click_with_options(ClickOptions::default()).await
    }

    /// Click with explicit options.
    pub async fn click_with_options(&self, options: ClickOptions) -> E2eResult<()> {
        let locator_options = LocatorOptions { timeout: None };
        if options.force {
            self.page.action(&self.selector, "click", None).await?;
            return Ok(());
        }
        self.page.action(&self.selector, "scroll", None).await?;
        let state = self.ready_state(&locator_options).await?;
        match Self::center(&state) {
            Some((x, y)) => {
                self.page
                    .mouse_click(x, y, options.click_count.max(1))
                    .await?;
                Ok(())
            }
            None => Err(E2eError::Locator {
                selector: self.selector.raw().to_string(),
                message: "element has no bounding box".to_string(),
            }),
        }
    }

    /// Double-click the element.
    pub async fn dblclick(&self) -> E2eResult<()> {
        self.click_with_options(ClickOptions {
            force: false,
            click_count: 2,
        })
        .await
    }

    /// Hover the element.
    pub async fn hover(&self) -> E2eResult<()> {
        self.page.action(&self.selector, "scroll", None).await?;
        let state = self.ready_state(&LocatorOptions { timeout: None }).await?;
        match Self::center(&state) {
            Some((x, y)) => self.page.mouse_move(x, y).await,
            None => Err(E2eError::Locator {
                selector: self.selector.raw().to_string(),
                message: "element has no bounding box".to_string(),
            }),
        }
    }

    /// Focus the element.
    pub async fn focus(&self) -> E2eResult<()> {
        self.page.action(&self.selector, "focus", None).await?;
        Ok(())
    }

    /// Screenshot just this element (PNG bytes).
    pub async fn screenshot(&self) -> E2eResult<Vec<u8>> {
        self.page.action(&self.selector, "scroll", None).await?;
        let state = self.ready_state(&LocatorOptions { timeout: None }).await?;
        match state.rects.first() {
            Some(rect) => self.page.screenshot_clip(rect, None).await,
            None => Err(E2eError::Locator {
                selector: self.selector.raw().to_string(),
                message: "element has no bounding box".to_string(),
            }),
        }
    }

    /// Fill an input/textarea/select with text (replaces the value).
    pub async fn fill(&self, text: &str) -> E2eResult<()> {
        self.page.action(&self.selector, "fill", Some(text)).await?;
        Ok(())
    }

    /// Type text char-by-char with trusted input (keeps existing value).
    pub async fn press_sequentially(&self, text: &str) -> E2eResult<()> {
        self.click().await?;
        self.page.insert_text(text).await?;
        // Notify frameworks that poll for input events.
        self.page
            .evaluate_value(
                "(() => { const el = document.activeElement; if (!el) return false; \
                 el.dispatchEvent(new Event('input', { bubbles: true })); \
                 el.dispatchEvent(new Event('change', { bubbles: true })); \
                 return true; })()",
            )
            .await?;
        Ok(())
    }

    /// Press a key while the element is focused.
    pub async fn press(&self, key: &str) -> E2eResult<()> {
        self.click().await?;
        self.page.press_key(key).await
    }

    /// Clear an input/textarea.
    pub async fn clear(&self) -> E2eResult<()> {
        self.page.action(&self.selector, "clear", None).await?;
        Ok(())
    }

    /// Check a checkbox/radio.
    pub async fn check(&self) -> E2eResult<()> {
        self.page
            .action(&self.selector, "check", Some("true"))
            .await?;
        Ok(())
    }

    /// Uncheck a checkbox.
    pub async fn uncheck(&self) -> E2eResult<()> {
        self.page
            .action(&self.selector, "check", Some("false"))
            .await?;
        Ok(())
    }

    /// Select an `<option>` by value.
    pub async fn select_option(&self, value: &str) -> E2eResult<()> {
        self.page
            .action(&self.selector, "select", Some(value))
            .await?;
        Ok(())
    }

    /// Read trimmed text content.
    pub async fn text(&self) -> E2eResult<String> {
        Ok(self.page.query_state(&self.selector).await?.text)
    }

    /// Read the form value.
    pub async fn input_value(&self) -> E2eResult<String> {
        Ok(self.page.query_state(&self.selector).await?.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engines_parse() {
        assert_eq!(Selector::parse("h1".to_string()).engine, Engine::Css);
        assert_eq!(Selector::parse("css=.a".to_string()).engine, Engine::Css);
        assert_eq!(
            Selector::parse("text=hello".to_string()).engine,
            Engine::Text
        );
        assert_eq!(
            Selector::parse("xpath=//div".to_string()).engine,
            Engine::XPath
        );
        assert_eq!(
            Selector::parse("role=button".to_string()).engine,
            Engine::Role
        );
    }

    #[test]
    fn role_name_parses() {
        assert_eq!(parse_role("button"), ("button".to_string(), String::new()));
        assert_eq!(
            parse_role("button[name=\"save\"]"),
            ("button".to_string(), "save".to_string())
        );
    }

    #[test]
    fn expressions_embed_escaped_selector() {
        let selector = Selector::parse("text=he\"llo".to_string());
        let expression = selector.state_expression();
        assert!(expression.contains("querySelectorAll('body *')"));
        assert!(expression.contains("he\\\"llo"));
        let css = Selector::parse(".a".to_string());
        assert!(css
            .state_expression()
            .contains("document.querySelectorAll(\".a\")"));
    }

    #[test]
    fn unknown_action_reports_error() {
        let selector = Selector::parse("h1".to_string());
        assert!(selector
            .action_expression("nope", None)
            .contains("unknown action"));
    }
}
