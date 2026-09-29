//! Selectors (`css`, `text=`, `xpath=`, `role=`) and locator actions.

use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde_json::Value;

use crate::driver::base64_encode;
use crate::error::{E2eError, E2eResult};
use crate::page::{ClickOptions, ElementState, KeyPressOptions, Page};

/// Attribute `get_by_test_id` matches (default `data-testid`).
static TEST_ID_ATTRIBUTE: OnceLock<Mutex<String>> = OnceLock::new();

fn test_id_cell() -> &'static Mutex<String> {
    TEST_ID_ATTRIBUTE.get_or_init(|| Mutex::new("data-testid".to_string()))
}

/// Set the attribute `get_by_test_id` matches (Playwright
/// `selectors.setTestIdAttribute`). Global: restore the default when done.
pub fn set_test_id_attribute(name: &str) {
    *test_id_cell().lock().unwrap_or_else(|e| e.into_inner()) = name.to_string();
}

/// The attribute `get_by_test_id` currently matches.
#[must_use]
pub fn test_id_attribute() -> String {
    test_id_cell()
        .lock()
        .map(|name| name.clone())
        .unwrap_or_else(|_| "data-testid".to_string())
}

/// A parsed selector with an engine and a body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selector {
    raw: String,
    engine: Engine,
    body: String,
    pick: Pick,
    scope: Option<Box<Selector>>,
    has_text: Vec<String>,
    has_not_text: Vec<String>,
    has: Vec<Selector>,
    has_not: Vec<Selector>,
    strict: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Engine {
    Css,
    Text,
    XPath,
    Role,
    Union(Box<Selector>, Box<Selector>),
    Intersect(Box<Selector>, Box<Selector>),
}

/// Which matches a narrowed selector keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pick {
    /// All matches (actions use the first).
    First,
    /// The last match only.
    Last,
    /// The `index`-th match only.
    Nth(usize),
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
        Self::leaf(raw, engine, body)
    }

    /// A leaf selector (no narrowing, document scope).
    fn leaf(raw: String, engine: Engine, body: String) -> Self {
        Self {
            raw,
            engine,
            body,
            pick: Pick::First,
            scope: None,
            has_text: Vec::new(),
            has_not_text: Vec::new(),
            has: Vec::new(),
            has_not: Vec::new(),
            strict: false,
        }
    }

    /// Match `[<test-id-attr>="id"]` exactly.
    pub(crate) fn test_id(id: &str) -> Self {
        let raw = format!("css=[{}={}]", test_id_attribute(), css_string(id));
        Self::parse(raw)
    }

    /// Match elements containing `text` (case-insensitive).
    pub(crate) fn by_text(text: &str) -> Self {
        Self::parse(format!("text={text}"))
    }

    /// Match an ARIA role, optionally filtered by accessible name.
    pub(crate) fn by_role(role: &str, name: &str) -> Self {
        if name.is_empty() {
            Self::parse(format!("role={role}"))
        } else {
            Self::parse(format!("role={role}[name=\"{name}\"]"))
        }
    }

    /// Match an ARIA role with full predicates.
    pub(crate) fn by_role_with(role: &str, options: &GetByRoleOptions) -> Self {
        let mut body = format!("role={role}");
        if let Some(name) = &options.name {
            body.push_str(&format!("[name=\"{name}\"]"));
        }
        for (key, flag) in [
            ("checked", options.checked),
            ("disabled", options.disabled),
            ("expanded", options.expanded),
            ("pressed", options.pressed),
            ("selected", options.selected),
            ("include-hidden", options.include_hidden),
        ] {
            if let Some(want) = flag {
                body.push_str(&format!("[{key}={want}]"));
            }
        }
        Self::parse(body)
    }

    /// Match a `<label>` by its text (case-insensitive substring).
    pub(crate) fn by_label(text: &str) -> Self {
        Self::parse(format!(
            "xpath=//label[contains(normalize-space(.), {})]",
            xpath_string(text)
        ))
    }

    /// Match an attribute by case-insensitive substring.
    fn by_attribute_contains(tag: &str, attribute: &str, text: &str) -> Self {
        const UPPER: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
        const LOWER: &str = "abcdefghijklmnopqrstuvwxyz";
        Self::parse(format!(
            "xpath=//{tag}[contains(translate(@{attribute},'{UPPER}','{LOWER}'), {})]",
            xpath_string(&text.to_lowercase())
        ))
    }

    /// Match `[placeholder]` by case-insensitive substring.
    pub(crate) fn by_placeholder(text: &str) -> Self {
        Self::by_attribute_contains("*", "placeholder", text)
    }

    /// Match `[alt]` by case-insensitive substring.
    pub(crate) fn by_alt(text: &str) -> Self {
        Self::by_attribute_contains("*", "alt", text)
    }

    /// Match `[title]` by case-insensitive substring.
    pub(crate) fn by_title(text: &str) -> Self {
        Self::by_attribute_contains("*", "title", text)
    }

    /// Original selector text.
    #[must_use]
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// JS that resolves to `Element[]` for this selector.
    fn resolve_js(&self) -> String {
        self.resolve_with(None)
    }

    /// Resolve against `roots` (`None` = whole document).
    ///
    /// An explicit [`Selector::scope`] (locator chaining) wins over inherited
    /// `roots`; combinators distribute `roots` to children without their own
    /// scope.
    fn resolve_with(&self, roots: Option<&str>) -> String {
        let scoped;
        let effective = match &self.scope {
            Some(parent) => {
                scoped = parent.resolve_js();
                Some(scoped.as_str())
            }
            None => roots,
        };
        let matched = match &self.engine {
            Engine::Union(first, second) => {
                let a = first.resolve_with(effective);
                let b = second.resolve_with(effective);
                format!("[...new Set([...({a}), ...({b})])]")
            }
            Engine::Intersect(first, second) => {
                let a = first.resolve_with(effective);
                let b = second.resolve_with(effective);
                format!(
                    "(() => {{ const keep = new Set(({b})); \
                     return ({a}).filter(el => keep.has(el)); }})()"
                )
            }
            leaf => self.resolve_leaf(leaf, effective),
        };
        let mut expression = matched;
        for needle in &self.has_text {
            let query = serde_json::to_string(needle).unwrap_or_default();
            expression = format!(
                "(() => {{ const els = ({expression}); \
                 const q = {query}.toLowerCase(); \
                 return els.filter(el => \
                 (el.textContent || '').toLowerCase().includes(q)); }})()"
            );
        }
        for needle in &self.has_not_text {
            let query = serde_json::to_string(needle).unwrap_or_default();
            expression = format!(
                "(() => {{ const els = ({expression}); \
                 const q = {query}.toLowerCase(); \
                 return els.filter(el => \
                 !(el.textContent || '').toLowerCase().includes(q)); }})()"
            );
        }
        // Inner locators resolve document-wide; containment makes the match
        // relative to each candidate (this also honors chained inners).
        for inner in &self.has {
            let resolve = inner.resolve_js();
            expression = format!(
                "(() => {{ const els = ({expression}); \
                 const inner = new Set(({resolve})); \
                 return els.filter(el => {{ for (const n of inner) {{ \
                 if (n !== el && el.contains(n)) return true; }} \
                 return false; }}); }})()"
            );
        }
        for inner in &self.has_not {
            let resolve = inner.resolve_js();
            expression = format!(
                "(() => {{ const els = ({expression}); \
                 const inner = new Set(({resolve})); \
                 return els.filter(el => {{ for (const n of inner) {{ \
                 if (n !== el && el.contains(n)) return false; }} \
                 return true; }}); }})()"
            );
        }
        match self.pick {
            Pick::First => expression,
            Pick::Last => format!("({expression}).slice(-1)"),
            Pick::Nth(index) => {
                format!("({expression}).slice({index}, {})", index.saturating_add(1))
            }
        }
    }

    /// JS that resolves a leaf engine to `Element[]` against `roots`.
    fn resolve_leaf(&self, engine: &Engine, roots: Option<&str>) -> String {
        let body = serde_json::to_string(&self.body).unwrap_or_default();
        match engine {
            Engine::Css => match roots {
                None => format!("[...document.querySelectorAll({body})]"),
                Some(r) => {
                    format!("[...({r}).flatMap(root => [...root.querySelectorAll({body})])]")
                }
            },
            Engine::XPath => match roots {
                None => format!(
                    "(() => {{ const out = []; const it = document.evaluate({body}, \
                     document, null, XPathResult.ORDERED_NODE_SNAPSHOT_TYPE, null); \
                     for (let i = 0; i < it.snapshotLength; i++) {{ \
                     const n = it.snapshotItem(i); \
                     if (n instanceof Element) out.push(n); }} return out; }})()"
                ),
                Some(r) => format!(
                    "(() => {{ const out = []; for (const root of ({r})) {{ \
                     const it = document.evaluate({body}, root, null, \
                     XPathResult.ORDERED_NODE_SNAPSHOT_TYPE, null); \
                     for (let i = 0; i < it.snapshotLength; i++) {{ \
                     const n = it.snapshotItem(i); \
                     if (n instanceof Element) out.push(n); }} }} return out; }})()"
                ),
            },
            Engine::Text => {
                let candidates = match roots {
                    None => "document.querySelectorAll('body *')".to_string(),
                    Some(r) => format!("({r}).flatMap(root => [...root.querySelectorAll('*')])"),
                };
                format!(
                    "(() => {{ const needle = {body}.toLowerCase(); \
                     const els = [...{candidates}]; \
                     const hits = els.filter(el => \
                     (el.textContent || '').toLowerCase().includes(needle)); \
                     hits.sort((a, b) => \
                     (a.textContent || '').length - (b.textContent || '').length); \
                     return hits.slice(0, 20); }})()"
                )
            }
            Engine::Role => {
                let query = parse_role_full(&self.body);
                let role_json = serde_json::to_string(&query.role).unwrap_or_default();
                let name_json = serde_json::to_string(&query.name).unwrap_or_default();
                let source = match roots {
                    None => "document.querySelectorAll(sel)".to_string(),
                    Some(r) => format!("({r}).flatMap(root => [...root.querySelectorAll(sel)])"),
                };
                let mut filters = format!(
                    "const want = {name_json}.toLowerCase(); \
                     if (want) els = els.filter(el => \
                     ((el.getAttribute('aria-label') || el.textContent || '')\
                     .toLowerCase().includes(want))); ",
                );
                // Each predicate compares a resolved boolean to `want`.
                for (attribute, want) in [
                    ("checked", query.checked),
                    ("disabled", query.disabled),
                    ("expanded", query.expanded),
                    ("pressed", query.pressed),
                    ("selected", query.selected),
                ] {
                    let Some(want) = want else { continue };
                    let resolved = match attribute {
                        "checked" => {
                            "(el.checked === true \
                             || el.getAttribute('aria-checked') === 'true')"
                        }
                        "disabled" => "(el.disabled === true)",
                        "expanded" => "(el.getAttribute('aria-expanded') === 'true')",
                        "pressed" => "(el.getAttribute('aria-pressed') === 'true')",
                        _ => {
                            "(el.selected === true \
                             || el.getAttribute('aria-selected') === 'true')"
                        }
                    };
                    filters.push_str(&format!(
                        "els = els.filter(el => ({resolved}) === {want}); "
                    ));
                }
                if query.include_hidden == Some(false) {
                    filters.push_str(
                        "els = els.filter(el => { const s = getComputedStyle(el); \
                         const r = el.getBoundingClientRect(); \
                         return s.display !== 'none' && s.visibility !== 'hidden' \
                         && r.width > 0 && r.height > 0; }); ",
                    );
                }
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
                     let els = [...{source}]; \
                     {filters}return els; }})()"
                )
            }
            Engine::Union(..) | Engine::Intersect(..) => {
                unreachable!("combinators resolve in resolve_with")
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
             const editable = el => {{ if (!el || el.disabled) return false; \
             const tag = el.tagName; \
             if (tag === 'INPUT') {{ \
             const t = (el.type || 'text').toLowerCase(); \
             if (['hidden','submit','button','reset','checkbox','radio',\
             'file','image','range','color'].includes(t)) return false; \
             return !el.readOnly; }} \
             if (tag === 'TEXTAREA' || tag === 'SELECT') return !el.readOnly; \
             return Boolean(el.isContentEditable); }}; \
             const rects = els.slice(0, 20).map(el => {{ \
             const r = el.getBoundingClientRect(); \
             return {{ x: r.x, y: r.y, width: r.width, height: r.height }}; }}); \
             return {{ count: els.length, \
             visible: visible(first), \
             enabled: first ? !first.disabled : false, \
             checked: first ? Boolean(first.checked) : false, \
             editable: editable(first), \
             focused: first ? document.activeElement === first : false, \
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
            "select_many" => &format!(
                "if (!(el instanceof HTMLSelectElement)) \
                 return {{ ok: false, error: 'not a select' }}; \
                 const wants = JSON.parse({arg}); \
                 const opts = [...el.options]; \
                 if (!el.multiple) opts.forEach(o => o.selected = false); \
                 let matched = 0; \
                 for (const w of wants) {{ \
                 const hit = w.value !== undefined \
                   ? opts.find(o => o.value === w.value) \
                   : w.label !== undefined \
                     ? opts.find(o => (o.label || o.text) === w.label) \
                     : opts[w.index]; \
                 if (hit) {{ hit.selected = true; matched++; }} }} \
                 {fire} \
                 return matched === wants.length \
                   ? {{ ok: true }} \
                   : {{ ok: false, error: 'option not found' }};"
            ),
            "set_input_files" => &format!(
                "if (!(el instanceof HTMLInputElement) || el.type !== 'file') \
                 return {{ ok: false, error: 'not a file input' }}; \
                 const picked = JSON.parse({arg}).map(f => {{ \
                 const bin = atob(f.data); \
                 const bytes = new Uint8Array(bin.length); \
                 for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i); \
                 return new File([bytes], f.name, {{ type: f.mime }}); }}); \
                 const dt = new DataTransfer(); \
                 picked.forEach(f => dt.items.add(f)); \
                 el.files = dt.files; {fire} \
                 return {{ ok: true }};"
            ),
            _ => "return { ok: false, error: 'unknown action' };",
        };
        let guard = if self.strict {
            "if (els.length !== 1) return { ok: false, \
             error: 'strict mode violation: expected exactly one match, got ' + els.length }; "
        } else {
            ""
        };
        format!(
            "(() => {{ const els = {resolve}; {guard}const el = els[0]; \
             if (!el) return {{ ok: false, error: 'no matching element' }}; \
             {body} }})()"
        )
    }
}

/// MIME type guess from a file name (defaults to octet-stream).
fn guess_mime(name: &str) -> &'static str {
    match name
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_lowercase()
        .as_str()
    {
        "txt" | "text" | "md" | "csv" => "text/plain",
        "html" | "htm" => "text/html",
        "json" => "application/json",
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "zip" => "application/zip",
        _ => "application/octet-stream",
    }
}

/// A double-quoted CSS string literal with `\` and `"` escaped.
fn css_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// An XPath string literal, using `concat()` when both quote types appear.
fn xpath_string(value: &str) -> String {
    if !value.contains('"') {
        return format!("\"{value}\"");
    }
    if !value.contains('\'') {
        return format!("'{value}'");
    }
    let mut parts: Vec<String> = Vec::new();
    let mut run = String::new();
    for ch in value.chars() {
        match ch {
            '"' => {
                if !run.is_empty() {
                    parts.push(format!("'{run}'"));
                    run.clear();
                }
                parts.push("'\"'".to_string());
            }
            '\'' => {
                if !run.is_empty() {
                    parts.push(format!("'{run}'"));
                    run.clear();
                }
                parts.push("\"'\"".to_string());
            }
            _ => run.push(ch),
        }
    }
    if !run.is_empty() {
        parts.push(format!("'{run}'"));
    }
    format!("concat({})", parts.join(", "))
}

/// Parse `role[name="x"]` into (role, name).
#[cfg(test)]
fn parse_role(body: &str) -> (String, String) {
    let query = parse_role_full(body);
    (query.role, query.name)
}

/// A parsed `role=` body with ARIA predicates.
#[derive(Debug, Default, PartialEq, Eq)]
struct RoleQuery {
    role: String,
    name: String,
    checked: Option<bool>,
    disabled: Option<bool>,
    expanded: Option<bool>,
    pressed: Option<bool>,
    selected: Option<bool>,
    include_hidden: Option<bool>,
}

/// Parse `role[name="x"][checked=true]...` bodies.
fn parse_role_full(body: &str) -> RoleQuery {
    let mut query = RoleQuery::default();
    let Some(bracket) = body.find('[') else {
        query.role = body.trim().to_string();
        return query;
    };
    query.role = body[..bracket].trim().to_string();
    let mut rest = &body[bracket..];
    while let Some(after_open) = rest.strip_prefix('[') {
        let Some((inner, after)) = after_open.split_once(']') else {
            break;
        };
        let (key, value) = inner.split_once('=').unwrap_or((inner, ""));
        let value = value.trim().trim_matches('"').trim_matches('\'');
        let flag = match value {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        };
        match key.trim() {
            "name" => query.name = value.to_string(),
            "checked" => query.checked = flag,
            "disabled" => query.disabled = flag,
            "expanded" => query.expanded = flag,
            "pressed" => query.pressed = flag,
            "selected" => query.selected = flag,
            "include-hidden" => query.include_hidden = flag,
            _ => {}
        }
        rest = after.trim_start();
        if !rest.starts_with('[') {
            break;
        }
    }
    query
}

/// Options for locator actions.
#[derive(Debug, Clone, Default)]
pub struct LocatorOptions {
    /// Action timeout (defaults to the page timeout).
    pub timeout: Option<Duration>,
}

/// Options for [`Locator::filter_with`].
#[derive(Debug, Clone, Default)]
pub struct FilterOptions {
    /// Keep matches containing this text (case-insensitive substring).
    pub has_text: Option<String>,
    /// Drop matches containing this text.
    pub has_not_text: Option<String>,
    /// Keep matches containing a match of this locator.
    pub has: Option<Locator>,
    /// Drop matches containing a match of this locator.
    pub has_not: Option<Locator>,
}

impl FilterOptions {
    /// Keep matches containing `text`.
    #[must_use]
    pub fn has_text(mut self, text: impl Into<String>) -> Self {
        self.has_text = Some(text.into());
        self
    }

    /// Drop matches containing `text`.
    #[must_use]
    pub fn has_not_text(mut self, text: impl Into<String>) -> Self {
        self.has_not_text = Some(text.into());
        self
    }

    /// Keep matches containing a match of `inner`.
    #[must_use]
    pub fn has(mut self, inner: Locator) -> Self {
        self.has = Some(inner);
        self
    }

    /// Drop matches containing a match of `inner`.
    #[must_use]
    pub fn has_not(mut self, inner: Locator) -> Self {
        self.has_not = Some(inner);
        self
    }
}

/// Options for `get_by_role_with`.
#[derive(Debug, Clone, Default)]
pub struct GetByRoleOptions {
    /// Filter by accessible name (case-insensitive substring).
    pub name: Option<String>,
    /// Filter by checked state.
    pub checked: Option<bool>,
    /// Filter by disabled state.
    pub disabled: Option<bool>,
    /// Filter by expanded state (`aria-expanded`).
    pub expanded: Option<bool>,
    /// Filter by pressed state (`aria-pressed`).
    pub pressed: Option<bool>,
    /// Filter by selected state.
    pub selected: Option<bool>,
    /// `Some(false)` drops hidden matches (default keeps them, matching
    /// the existing `get_by_role` behavior).
    pub include_hidden: Option<bool>,
}

impl GetByRoleOptions {
    /// Filter by accessible name.
    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Filter by checked state.
    #[must_use]
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }

    /// Filter by disabled state.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = Some(disabled);
        self
    }

    /// Filter by expanded state.
    #[must_use]
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = Some(expanded);
        self
    }

    /// Filter by pressed state.
    #[must_use]
    pub fn pressed(mut self, pressed: bool) -> Self {
        self.pressed = Some(pressed);
        self
    }

    /// Filter by selected state.
    #[must_use]
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = Some(selected);
        self
    }

    /// Drop hidden matches when `false`.
    #[must_use]
    pub fn include_hidden(mut self, include: bool) -> Self {
        self.include_hidden = Some(include);
        self
    }
}

/// States for [`Locator::wait_for_state`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitForState {
    /// At least one match exists.
    Attached,
    /// No match exists.
    Detached,
    /// A match exists and the first is visible.
    Visible,
    /// No match exists or the first is hidden.
    Hidden,
}

impl WaitForState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Attached => "attached",
            Self::Detached => "detached",
            Self::Visible => "visible",
            Self::Hidden => "hidden",
        }
    }
}

/// One `<option>` selection for [`Locator::select_options`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectOption {
    /// Match by option value.
    Value(String),
    /// Match by option label (or text).
    Label(String),
    /// Match by option index.
    Index(usize),
}

impl SelectOption {
    /// Match by option value.
    pub fn value(value: impl Into<String>) -> Self {
        Self::Value(value.into())
    }

    /// Match by option label.
    pub fn label(label: impl Into<String>) -> Self {
        Self::Label(label.into())
    }
}

/// A lazy handle to DOM element(s): actions auto-wait for the target.
#[derive(Clone)]
pub struct Locator {
    page: Page,
    selector: Selector,
}

impl std::fmt::Debug for Locator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Locator")
            .field("selector", &self.selector)
            .finish()
    }
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

    /// Scope `sub` to this locator's matches.
    fn scoped(&self, mut sub: Selector) -> Self {
        sub.raw = format!("{} >> {}", self.selector.raw(), sub.raw);
        sub.scope = Some(Box::new(self.selector.clone()));
        Self {
            page: self.page.clone(),
            selector: sub,
        }
    }

    /// Chain a sub-selector scoped to this locator's matches.
    #[must_use]
    pub fn locator(&self, selector: &str) -> Self {
        self.scoped(Selector::parse(selector.to_string()))
    }

    /// Narrow to the first match.
    #[must_use]
    pub fn first(&self) -> Self {
        self.picked(Pick::First, "first".to_string())
    }

    /// Narrow to the last match.
    #[must_use]
    pub fn last(&self) -> Self {
        self.picked(Pick::Last, "last".to_string())
    }

    /// Narrow to the `index`-th match (0-based).
    #[must_use]
    pub fn nth(&self, index: usize) -> Self {
        self.picked(Pick::Nth(index), format!("nth={index}"))
    }

    /// Narrow to one pick (later picks win).
    fn picked(&self, pick: Pick, label: String) -> Self {
        let mut selector = self.selector.clone();
        selector.pick = pick;
        selector.raw = format!("{} >> {label}", selector.raw);
        Self {
            page: self.page.clone(),
            selector,
        }
    }

    /// Fail actions unless exactly one element matches.
    ///
    /// Applies to actions and getters; `count()` still reports all matches.
    #[must_use]
    pub fn strict(&self) -> Self {
        let mut selector = self.selector.clone();
        selector.strict = true;
        selector.raw = format!("{} >> strict", selector.raw);
        Self {
            page: self.page.clone(),
            selector,
        }
    }

    /// Keep only matches containing `has_text` (case-insensitive substring).
    #[must_use]
    pub fn filter(&self, has_text: &str) -> Self {
        let mut selector = self.selector.clone();
        selector.has_text.push(has_text.to_string());
        selector.raw = format!("{} >> has-text={has_text:?}", selector.raw);
        Self {
            page: self.page.clone(),
            selector,
        }
    }

    /// Narrow matches with full options (chainable with [`Locator::filter`]).
    #[must_use]
    pub fn filter_with(&self, options: FilterOptions) -> Self {
        let mut selector = self.selector.clone();
        let mut raw = selector.raw;
        if let Some(text) = options.has_text {
            selector.has_text.push(text.clone());
            raw = format!("{raw} >> has-text={text:?}");
        }
        if let Some(text) = options.has_not_text {
            selector.has_not_text.push(text.clone());
            raw = format!("{raw} >> has-not-text={text:?}");
        }
        if let Some(inner) = options.has {
            raw = format!("{raw} >> has=({})", inner.selector.raw());
            selector.has.push(inner.selector);
        }
        if let Some(inner) = options.has_not {
            raw = format!("{raw} >> has-not=({})", inner.selector.raw());
            selector.has_not.push(inner.selector);
        }
        selector.raw = raw;
        Self {
            page: self.page.clone(),
            selector,
        }
    }

    /// This locator's selector text (debugging, handler matching).
    pub(crate) fn selector_raw(&self) -> &str {
        self.selector.raw()
    }

    /// Matches of either locator (same page required).
    pub fn or_(&self, other: &Locator) -> E2eResult<Self> {
        self.combine(other, true)
    }

    /// Matches of both locators (same page required).
    pub fn and_(&self, other: &Locator) -> E2eResult<Self> {
        self.combine(other, false)
    }

    /// Combine two same-page locators.
    fn combine(&self, other: &Locator, union: bool) -> E2eResult<Self> {
        if self.page.target_id() != other.page.target_id() {
            return Err(E2eError::Locator {
                selector: self.selector.raw().to_string(),
                message: "cannot combine locators from different pages".to_string(),
            });
        }
        let (engine, op) = if union {
            (
                Engine::Union(
                    Box::new(self.selector.clone()),
                    Box::new(other.selector.clone()),
                ),
                "|",
            )
        } else {
            (
                Engine::Intersect(
                    Box::new(self.selector.clone()),
                    Box::new(other.selector.clone()),
                ),
                "&",
            )
        };
        let selector = Selector {
            raw: format!("({} {op} {})", self.selector.raw(), other.selector.raw()),
            engine,
            body: String::new(),
            pick: Pick::First,
            scope: None,
            has_text: Vec::new(),
            has_not_text: Vec::new(),
            has: Vec::new(),
            has_not: Vec::new(),
            strict: false,
        };
        Ok(Self {
            page: self.page.clone(),
            selector,
        })
    }

    /// One locator per match.
    pub async fn all(&self) -> E2eResult<Vec<Self>> {
        let count = self.count().await?;
        Ok((0..count).map(|index| self.nth(index)).collect())
    }

    /// Locate `[data-testid]` within this locator's matches.
    #[must_use]
    pub fn get_by_test_id(&self, id: &str) -> Self {
        self.scoped(Selector::test_id(id))
    }

    /// Locate by text within this locator's matches.
    #[must_use]
    pub fn get_by_text(&self, text: &str) -> Self {
        self.scoped(Selector::by_text(text))
    }

    /// Locate by ARIA role within this locator's matches.
    #[must_use]
    pub fn get_by_role(&self, role: &str, name: &str) -> Self {
        self.scoped(Selector::by_role(role, name))
    }

    /// Locate an ARIA role with full predicates within this locator.
    #[must_use]
    pub fn get_by_role_with(&self, role: &str, options: GetByRoleOptions) -> Self {
        self.scoped(Selector::by_role_with(role, &options))
    }

    /// Locate a `<label>` by text within this locator's matches.
    #[must_use]
    pub fn get_by_label(&self, text: &str) -> Self {
        self.scoped(Selector::by_label(text))
    }

    /// Locate by `[placeholder]` within this locator's matches.
    #[must_use]
    pub fn get_by_placeholder(&self, text: &str) -> Self {
        self.scoped(Selector::by_placeholder(text))
    }

    /// Locate by `[alt]` within this locator's matches.
    #[must_use]
    pub fn get_by_alt(&self, text: &str) -> Self {
        self.scoped(Selector::by_alt(text))
    }

    /// Locate by `[title]` within this locator's matches.
    #[must_use]
    pub fn get_by_title(&self, text: &str) -> Self {
        self.scoped(Selector::by_title(text))
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
        self.wait_for_state(WaitForState::Attached, timeout).await
    }

    /// Wait until the locator reaches `state`.
    pub async fn wait_for_state(&self, state: WaitForState, timeout: Duration) -> E2eResult<()> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if let Ok(current) = self.page.query_state(&self.selector).await {
                let done = match state {
                    WaitForState::Attached => current.count > 0,
                    WaitForState::Detached => current.count == 0,
                    WaitForState::Visible => current.count > 0 && current.visible,
                    WaitForState::Hidden => current.count == 0 || !current.visible,
                };
                if done {
                    return Ok(());
                }
            }
            if tokio::time::Instant::now() > deadline {
                return Err(E2eError::Timeout(
                    timeout.as_millis().min(u128::from(u64::MAX)) as u64,
                    format!(
                        "wait for `{}` to be {}",
                        self.selector.raw(),
                        state.as_str()
                    ),
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
                    .mouse_click_with(
                        x,
                        y,
                        crate::page::MouseClickOptions {
                            button: options.button,
                            click_count: options.click_count.max(1),
                            delay: options.delay,
                        },
                    )
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
            button: crate::page::MouseButton::Left,
            delay: Duration::ZERO,
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

    /// Blur the element.
    pub async fn blur(&self) -> E2eResult<()> {
        self.page.action(&self.selector, "blur", None).await?;
        Ok(())
    }

    /// Tap the element's center via the touchscreen.
    pub async fn tap(&self) -> E2eResult<()> {
        self.page.action(&self.selector, "scroll", None).await?;
        let state = self.ready_state(&LocatorOptions { timeout: None }).await?;
        match Self::center(&state) {
            Some((x, y)) => self.page.touchscreen_tap(x, y).await,
            None => Err(E2eError::Locator {
                selector: self.selector.raw().to_string(),
                message: "element has no bounding box".to_string(),
            }),
        }
    }

    /// Drag the element's center onto `target` in `steps` moves (min 1).
    /// Both locators must live on the same page.
    pub async fn drag_to(&self, target: &Locator, steps: u32) -> E2eResult<()> {
        if self.page.target_id() != target.page.target_id() {
            return Err(E2eError::Locator {
                selector: self.selector.raw().to_string(),
                message: "drag_to needs locators on the same page".to_string(),
            });
        }
        if steps == 0 {
            return Err(E2eError::Config(
                "drag_to needs at least 1 step".to_string(),
            ));
        }
        self.page.action(&self.selector, "scroll", None).await?;
        let from = self.ready_state(&LocatorOptions { timeout: None }).await?;
        let to = target
            .ready_state(&LocatorOptions { timeout: None })
            .await?;
        let (Some((x0, y0)), Some((x1, y1))) = (Self::center(&from), Self::center(&to)) else {
            return Err(E2eError::Locator {
                selector: self.selector.raw().to_string(),
                message: "element has no bounding box".to_string(),
            });
        };
        self.page.mouse_drag((x0, y0), (x1, y1), steps).await
    }

    /// Scroll the element into the center of the viewport.
    pub async fn scroll_into_view(&self) -> E2eResult<()> {
        self.page.action(&self.selector, "scroll", None).await?;
        Ok(())
    }

    /// Dispatch a bubbling `CustomEvent` with an optional JSON `detail`.
    pub async fn dispatch_event(&self, name: &str, detail: Option<&Value>) -> E2eResult<()> {
        let name_json = serde_json::to_string(name).unwrap_or_default();
        let detail_json = detail
            .map(|value| serde_json::to_string(value).unwrap_or_else(|_| "null".to_string()))
            .unwrap_or_else(|| "null".to_string());
        let value = self
            .eval_first(&format!(
                "el.dispatchEvent(new CustomEvent({name_json}, \
                 {{ bubbles: true, detail: {detail_json} }}))"
            ))
            .await?;
        self.require_match(value)
    }

    /// Select the element's text (input value or rendered text).
    pub async fn select_text(&self) -> E2eResult<()> {
        let value = self
            .eval_first(
                "(() => { if (el.select) el.select(); \
                 else getSelection().selectAllChildren(el); return true; })()",
            )
            .await?;
        self.require_match(value)
    }

    /// A null `eval_first` projection means nothing matched: fail loudly.
    fn require_match(&self, value: Value) -> E2eResult<()> {
        if value.is_null() {
            return Err(E2eError::Locator {
                selector: self.selector.raw().to_string(),
                message: "no matching element".to_string(),
            });
        }
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

    /// Press a key with explicit options (down/up delay).
    pub async fn press_with(&self, key: &str, options: KeyPressOptions) -> E2eResult<()> {
        self.click().await?;
        self.page.press_key_with(key, options).await
    }

    /// Type text with a delay between keystrokes.
    pub async fn press_sequentially_with(
        &self,
        text: &str,
        options: KeyPressOptions,
    ) -> E2eResult<()> {
        self.click().await?;
        for ch in text.chars() {
            self.page
                .press_key_with(&ch.to_string(), options.clone())
                .await?;
        }
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

    /// Clear an input/textarea.
    pub async fn clear(&self) -> E2eResult<()> {
        self.page.action(&self.selector, "clear", None).await?;
        Ok(())
    }

    /// Set files on a file input (an empty list clears it). Files are read
    /// from disk (64 MiB total cap) and injected via `DataTransfer`, firing
    /// `input`/`change`.
    pub async fn set_input_files<P: AsRef<Path>>(&self, paths: &[P]) -> E2eResult<()> {
        let mut files = Vec::with_capacity(paths.len());
        let mut total = 0usize;
        for path in paths {
            let path = path.as_ref();
            let bytes = std::fs::read(path).map_err(|error| {
                E2eError::Config(format!("cannot read {}: {error}", path.display()))
            })?;
            total += bytes.len();
            if total > 64 * 1024 * 1024 {
                return Err(E2eError::Config(
                    "set_input_files payload exceeds 64 MiB".to_string(),
                ));
            }
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            files.push(serde_json::json!({
                "name": name,
                "mime": guess_mime(&name),
                "data": base64_encode(&bytes),
            }));
        }
        let payload = serde_json::to_string(&files).map_err(E2eError::Json)?;
        self.page
            .action(&self.selector, "set_input_files", Some(&payload))
            .await?;
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

    /// Select `<option>`s by value, label, or index (every entry must match;
    /// single-selects keep the last match).
    pub async fn select_options(&self, options: &[SelectOption]) -> E2eResult<()> {
        let wants: Vec<Value> = options
            .iter()
            .map(|option| match option {
                SelectOption::Value(value) => serde_json::json!({ "value": value }),
                SelectOption::Label(label) => serde_json::json!({ "label": label }),
                SelectOption::Index(index) => serde_json::json!({ "index": index }),
            })
            .collect();
        let argument = serde_json::to_string(&wants).unwrap_or_default();
        self.page
            .action(&self.selector, "select_many", Some(&argument))
            .await?;
        Ok(())
    }

    /// Values of the selected `<option>`s (empty when not a select).
    pub async fn selected_options(&self) -> E2eResult<Vec<String>> {
        let value = self
            .eval_first(
                "el instanceof HTMLSelectElement ? [...el.selectedOptions].map(o => o.value) : []",
            )
            .await?;
        serde_json::from_value(value).map_err(E2eError::Json)
    }

    /// Read trimmed text content.
    pub async fn text(&self) -> E2eResult<String> {
        Ok(self.page.query_state(&self.selector).await?.text)
    }

    /// Read the form value.
    pub async fn input_value(&self) -> E2eResult<String> {
        Ok(self.page.query_state(&self.selector).await?.value)
    }

    /// Evaluate `projection` (an expression over `el`) on the first match.
    ///
    /// Returns [`Value::Null`] when nothing matches.
    async fn eval_first(&self, projection: &str) -> E2eResult<Value> {
        let resolve = self.selector.resolve_js();
        let guard = if self.selector.strict {
            "if (els.length !== 1) throw new Error('strict mode violation: \
             expected exactly one match, got ' + els.length); "
        } else {
            ""
        };
        let expression = format!(
            "(() => {{ const els = {resolve}; {guard}const el = els[0]; \
             if (!el) return null; return ({projection}); }})()"
        );
        self.page.evaluate_value(&expression).await
    }

    /// Read an attribute (`None` when the element or attribute is missing).
    pub async fn attribute(&self, name: &str) -> E2eResult<Option<String>> {
        let name_json = serde_json::to_string(name).unwrap_or_default();
        let value = self
            .eval_first(&format!("el.getAttribute({name_json})"))
            .await?;
        Ok(value.as_str().map(str::to_string))
    }

    /// Read a computed CSS property (`None` when the element is missing).
    pub async fn css_value(&self, property: &str) -> E2eResult<Option<String>> {
        let property_json = serde_json::to_string(property).unwrap_or_default();
        let value = self
            .eval_first(&format!(
                "getComputedStyle(el).getPropertyValue({property_json})"
            ))
            .await?;
        Ok(value.as_str().map(str::to_string))
    }

    /// Read a DOM property as JSON (`None` when the element is missing or
    /// the property is `null`/`undefined`).
    pub async fn js_property(&self, name: &str) -> E2eResult<Option<Value>> {
        let name_json = serde_json::to_string(name).unwrap_or_default();
        let value = self.eval_first(&format!("el[{name_json}]")).await?;
        if value.is_null() {
            Ok(None)
        } else {
            Ok(Some(value))
        }
    }

    /// Read innerHTML (`None` when the element is missing).
    pub async fn inner_html(&self) -> E2eResult<Option<String>> {
        let value = self.eval_first("el.innerHTML").await?;
        Ok(value.as_str().map(str::to_string))
    }

    /// Whether the first match intersects the viewport.
    pub async fn in_viewport(&self) -> E2eResult<bool> {
        let value = self
            .eval_first(
                "(() => { const r = el.getBoundingClientRect(); \
                 return r.bottom > 0 && r.right > 0 \
                 && r.top < innerHeight && r.left < innerWidth; })()",
            )
            .await?;
        Ok(value.as_bool().unwrap_or(false))
    }

    /// Accessible name (`aria-label`, `alt`, then text).
    ///
    /// An approximation of the full accessible-name computation.
    pub async fn accessible_name(&self) -> E2eResult<Option<String>> {
        let value = self
            .eval_first(
                "(el.getAttribute('aria-label') \
                 || el.getAttribute('alt') \
                 || (el.textContent || '').trim() || null)",
            )
            .await?;
        Ok(value.as_str().map(str::to_string))
    }

    /// Accessible description (`aria-describedby` targets, then `title`).
    ///
    /// An approximation of the full accessible-description computation.
    pub async fn accessible_description(&self) -> E2eResult<Option<String>> {
        let value = self
            .eval_first(
                "(() => { const ids = (el.getAttribute('aria-describedby') || '') \
                 .split(/\\s+/).filter(Boolean); \
                 const text = ids.map(id => document.getElementById(id)) \
                 .filter(Boolean).map(target => target.textContent.trim()) \
                 .join(' '); \
                 return text || el.getAttribute('title') || null; })()",
            )
            .await?;
        Ok(value.as_str().map(str::to_string))
    }

    /// Whether the first match is visible (immediate, no retry).
    pub async fn is_visible(&self) -> E2eResult<bool> {
        Ok(self.page.query_state(&self.selector).await?.visible)
    }

    /// Whether the first match is hidden or absent (immediate, no retry).
    pub async fn is_hidden(&self) -> E2eResult<bool> {
        let state = self.page.query_state(&self.selector).await?;
        Ok(state.count == 0 || !state.visible)
    }

    /// Whether the first match is enabled (immediate, no retry).
    pub async fn is_enabled(&self) -> E2eResult<bool> {
        Ok(self.page.query_state(&self.selector).await?.enabled)
    }

    /// Whether the first match is disabled (immediate, no retry).
    pub async fn is_disabled(&self) -> E2eResult<bool> {
        Ok(!self.page.query_state(&self.selector).await?.enabled)
    }

    /// Whether the first match is checked (immediate, no retry).
    pub async fn is_checked(&self) -> E2eResult<bool> {
        Ok(self.page.query_state(&self.selector).await?.checked)
    }

    /// Whether the first match is editable (immediate, no retry).
    pub async fn is_editable(&self) -> E2eResult<bool> {
        Ok(self.page.query_state(&self.selector).await?.editable)
    }

    /// Whether the first match is focused (immediate, no retry).
    pub async fn is_focused(&self) -> E2eResult<bool> {
        Ok(self.page.query_state(&self.selector).await?.focused)
    }

    /// Whether at least one element matches (immediate, no retry).
    pub async fn is_attached(&self) -> E2eResult<bool> {
        Ok(self.page.query_state(&self.selector).await?.count > 0)
    }

    /// Bounding box of the first match (`None` when nothing matches).
    pub async fn bounding_box(&self) -> E2eResult<Option<crate::page::ElementRect>> {
        Ok(self
            .page
            .query_state(&self.selector)
            .await?
            .rects
            .into_iter()
            .next())
    }

    /// Outline the first match with a red box for a moment (debugging).
    pub async fn highlight(&self) -> E2eResult<()> {
        let value = self
            .eval_first(
                "(() => { el.style.outline = '2px solid #ff0000'; \
                 el.style.outlineOffset = '1px'; return true; })()",
            )
            .await?;
        self.require_match(value)
    }

    /// Check or uncheck a checkbox to reach `checked`.
    pub async fn set_checked(&self, checked: bool) -> E2eResult<()> {
        if checked {
            self.check().await
        } else {
            self.uncheck().await
        }
    }

    /// Run `function` with the first match as its argument.
    ///
    /// `function` is a JS function expression, e.g. `(el) => el.id`.
    pub async fn evaluate<T: serde::de::DeserializeOwned>(&self, function: &str) -> E2eResult<T> {
        let value = self.eval_first(&format!("(({function}))(el)")).await?;
        if value.is_null() {
            return Err(E2eError::Locator {
                selector: self.selector.raw().to_string(),
                message: "no matching element".to_string(),
            });
        }
        Ok(serde_json::from_value(value)?)
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
    fn test_id_attribute_round_trips() {
        assert_eq!(test_id_attribute(), "data-testid");
        set_test_id_attribute("data-qa");
        assert_eq!(test_id_attribute(), "data-qa");
        assert_eq!(Selector::test_id("save").raw, "css=[data-qa=\"save\"]");
        set_test_id_attribute("data-testid");
        assert_eq!(test_id_attribute(), "data-testid");
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
    fn role_predicates_parse() {
        let query = parse_role_full(
            "checkbox[name=\"agree\"][checked=true][disabled=false][include-hidden=true]",
        );
        assert_eq!(query.role, "checkbox");
        assert_eq!(query.name, "agree");
        assert_eq!(query.checked, Some(true));
        assert_eq!(query.disabled, Some(false));
        assert_eq!(query.include_hidden, Some(true));
        assert_eq!(query.expanded, None);
        // Malformed bodies degrade instead of panicking.
        let query = parse_role_full("button[name=\"oops");
        assert_eq!(query.role, "button");
        assert_eq!(query.name, "");
        let query = parse_role_full("link[wat=1][pressed=nope]");
        assert_eq!(query.role, "link");
        assert_eq!(query.pressed, None);
    }

    #[test]
    fn role_with_resolves_predicates() {
        let selector = Selector::by_role_with(
            "checkbox",
            &GetByRoleOptions::default()
                .checked(true)
                .include_hidden(false),
        );
        assert!(selector.raw.contains("[checked=true]"), "{}", selector.raw);
        let expression = selector.resolve_js();
        assert!(expression.contains("aria-checked"), "{expression}");
        assert!(expression.contains("getComputedStyle"), "{expression}");
        // Plain roles keep the old shape (no predicate filters).
        let plain = Selector::by_role("button", "");
        assert!(!plain.resolve_js().contains("aria-pressed"));
    }

    #[test]
    fn filter_with_resolves() {
        let mut selector = Selector::parse("li".to_string());
        selector.has_not_text.push("gone".to_string());
        selector.has.push(Selector::parse("button".to_string()));
        selector
            .has_not
            .push(Selector::parse("css=.gone".to_string()));
        let expression = selector.resolve_js();
        assert!(expression.contains("!(el.textContent"), "{expression}");
        assert!(expression.contains("el.contains(n)"), "{expression}");
        assert!(
            expression.contains("querySelectorAll(\"button\")"),
            "{expression}"
        );
        assert!(expression.contains(".gone"), "{expression}");
    }

    #[test]
    fn select_many_action_matches_variants() {
        let selector = Selector::parse("css=select".to_string());
        let expression = selector.action_expression("select_many", Some("[{\"label\":\"B\"}]"));
        assert!(expression.contains("JSON.parse"), "{expression}");
        assert!(expression.contains("o.label"), "{expression}");
        assert!(expression.contains("option not found"), "{expression}");
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
    fn state_snapshot_covers_editable_and_focused() {
        let expression = Selector::parse("#a".to_string()).state_expression();
        assert!(expression.contains("editable: editable(first)"));
        assert!(expression.contains("document.activeElement === first"));
    }

    #[test]
    fn unknown_action_reports_error() {
        let selector = Selector::parse("h1".to_string());
        assert!(selector
            .action_expression("nope", None)
            .contains("unknown action"));
    }

    #[test]
    fn css_strings_escape() {
        assert_eq!(css_string("plain"), "\"plain\"");
        assert_eq!(css_string("a\"b"), "\"a\\\"b\"");
        assert_eq!(css_string("a\\b"), "\"a\\\\b\"");
    }

    #[test]
    fn xpath_strings_escape() {
        assert_eq!(xpath_string("plain"), "\"plain\"");
        assert_eq!(xpath_string("a\"b"), "'a\"b'");
        assert_eq!(xpath_string("a'b"), "\"a'b\"");
        assert_eq!(xpath_string("a\"b'c"), "concat('a', '\"', 'b', \"'\", 'c')");
    }

    #[test]
    fn by_builders_map() {
        assert_eq!(Selector::test_id("go").raw(), "css=[data-testid=\"go\"]");
        assert_eq!(Selector::by_text("hi").raw(), "text=hi");
        assert_eq!(Selector::by_role("button", "").raw(), "role=button");
        assert_eq!(
            Selector::by_role("button", "save").raw(),
            "role=button[name=\"save\"]"
        );
        assert!(Selector::by_label("Name")
            .raw()
            .starts_with("xpath=//label["));
        assert!(Selector::by_placeholder("ENTER")
            .raw()
            .contains("@placeholder"));
        assert!(Selector::by_placeholder("ENTER")
            .raw()
            .contains("\"enter\""));
        assert!(Selector::by_alt("logo").raw().contains("@alt"));
        assert!(Selector::by_title("docs").raw().contains("@title"));
    }

    #[test]
    fn pick_narrows_resolution() {
        let base = Selector::parse(".a".to_string());
        let mut nth = base.clone();
        nth.pick = Pick::Nth(2);
        assert!(nth.resolve_js().ends_with(".slice(2, 3)"));
        let mut last = base.clone();
        last.pick = Pick::Last;
        assert!(last.resolve_js().ends_with(".slice(-1)"));
        assert!(!base.resolve_js().contains(".slice("));
    }

    #[test]
    fn filter_and_scope_shape() {
        let mut filtered = Selector::parse(".a".to_string());
        filtered.has_text.push("hi".to_string());
        let expression = filtered.resolve_js();
        assert!(expression.contains("toLowerCase().includes(q)"));
        let mut scoped = Selector::parse(".b".to_string());
        scoped.scope = Some(Box::new(Selector::parse("form".to_string())));
        let expression = scoped.resolve_js();
        assert!(expression.contains("flatMap"));
        assert!(expression.contains("querySelectorAll(\"form\")"));
    }

    #[test]
    fn strict_guards_actions() {
        let mut selector = Selector::parse(".a".to_string());
        selector.strict = true;
        assert!(selector
            .action_expression("click", None)
            .contains("strict mode violation"));
        let loose = Selector::parse(".a".to_string());
        assert!(!loose
            .action_expression("click", None)
            .contains("strict mode violation"));
    }

    #[test]
    fn combinators_resolve() {
        let combo = Selector {
            raw: "(a | b)".to_string(),
            engine: Engine::Union(
                Box::new(Selector::parse("#a".to_string())),
                Box::new(Selector::parse("#b".to_string())),
            ),
            body: String::new(),
            pick: Pick::First,
            scope: None,
            has_text: Vec::new(),
            has_not_text: Vec::new(),
            has: Vec::new(),
            has_not: Vec::new(),
            strict: false,
        };
        let expression = combo.resolve_js();
        assert!(expression.contains("new Set"));
        assert!(expression.contains("#a"));
        assert!(expression.contains("#b"));
        let both = Selector {
            raw: "(a & b)".to_string(),
            engine: Engine::Intersect(
                Box::new(Selector::parse("#a".to_string())),
                Box::new(Selector::parse("#b".to_string())),
            ),
            body: String::new(),
            pick: Pick::First,
            scope: None,
            has_text: Vec::new(),
            has_not_text: Vec::new(),
            has: Vec::new(),
            has_not: Vec::new(),
            strict: false,
        };
        let expression = both.resolve_js();
        assert!(expression.contains("keep.has(el)"));
    }
}
