//! Selectors (`css`, `text=`, `xpath=`, `role=`) and locator actions.

use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde_json::Value;

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
    exact: bool,
    regex: Option<String>,
    visible: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Engine {
    Label,
    Attribute(String),
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
    All,
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
            pick: Pick::All,
            scope: None,
            has_text: Vec::new(),
            has_not_text: Vec::new(),
            has: Vec::new(),
            has_not: Vec::new(),
            strict: false,
            exact: false,
            regex: None,
            visible: None,
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
        let mut selector = Self::leaf(format!("label={text}"), Engine::Label, text.into());
        selector.body = text.into();
        selector
    }

    fn by_attribute_contains(_tag: &str, attribute: &str, text: &str) -> Self {
        Self::leaf(
            format!("{attribute}={text}"),
            Engine::Attribute(attribute.into()),
            text.into(),
        )
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

    pub(crate) fn is_strict(&self) -> bool {
        self.strict
    }

    /// Original selector text.
    #[must_use]
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// JS that resolves to `Element[]` for this selector.
    pub(crate) fn resolve_js(&self) -> String {
        format!(
            "(() => {{ const f = {}; return {}; }})()",
            include_str!("dom.js"),
            self.resolve_with(None)
        )
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
                scoped = parent.resolve_with(roots);
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
        for inner in &self.has {
            let resolve = inner.resolve_with(Some("[el]"));
            expression = format!("({expression}).filter(el => ({resolve}).length > 0)");
        }
        for inner in &self.has_not {
            let resolve = inner.resolve_with(Some("[el]"));
            expression = format!("({expression}).filter(el => ({resolve}).length === 0)");
        }
        if let Some(visible) = self.visible {
            expression = format!("({expression}).filter(el => f.hidden(el) !== {visible})");
        }
        match self.pick {
            Pick::All => expression,
            Pick::First => format!("({expression}).slice(0, 1)"),
            Pick::Last => format!("({expression}).slice(-1)"),
            Pick::Nth(index) => {
                format!("({expression}).slice({index}, {})", index.saturating_add(1))
            }
        }
    }

    /// JS that resolves a leaf engine to `Element[]` against `roots`.
    fn resolve_leaf(&self, engine: &Engine, roots: Option<&str>) -> String {
        let body = serde_json::to_string(&self.body).unwrap();
        let roots = roots.unwrap_or("[document]");
        let regex = serde_json::to_string(&self.regex).unwrap();
        let all = format!("[...new Set(({roots}).flatMap(root => f.query(root, '*')))]");
        let exact = self.exact;
        match engine {
            Engine::Css => format!("[...new Set(({roots}).flatMap(root => f.query(root, {body})))]"),
            Engine::XPath => format!("(() => {{ const out = []; for (const root of ({roots})) {{ const it = document.evaluate({body}, root, null, XPathResult.ORDERED_NODE_SNAPSHOT_TYPE, null); for (let i = 0; i < it.snapshotLength; i++) {{ const n = it.snapshotItem(i); if (n instanceof Element) out.push(n); }} }} return [...new Set(out)]; }})()"),
            Engine::Text => format!("(() => {{ const hits = ({all}).filter(el => !el.matches('script,style,noscript') && f.matches(el.matches('input[type=button],input[type=submit]') ? el.value : el.textContent || el.shadowRoot?.textContent, {body}, {exact}, {regex})); return hits.filter(el => !hits.some(child => child !== el && el.contains(child))); }})()"),
            Engine::Label => format!("({all}).filter(el => el.matches('input,textarea,select,button,meter,progress,output') && (f.matches(f.name(el), {body}, {exact}, {regex}) || [...(el.labels || [])].some(label => f.matches(label.textContent, {body}, {exact}, {regex}))))"),
            Engine::Attribute(attribute) => {
                let attribute = serde_json::to_string(attribute).unwrap();
                format!("({all}).filter(el => el.hasAttribute({attribute}) && f.matches(el.getAttribute({attribute}), {body}, {exact}, {regex}))")
            }
            Engine::Role => {
                let query = parse_role_full(&self.body);
                let role = serde_json::to_string(&query.role).unwrap();
                let name = serde_json::to_string(&query.name).unwrap();
                let mut filters = format!("({all}).filter(el => f.role(el) === {role})");
                if !query.name.is_empty() || self.regex.is_some() { filters.push_str(&format!(".filter(el => f.matches(f.name(el), {name}, {exact}, {regex}))")); }
                if query.include_hidden != Some(true) { filters.push_str(".filter(el => !f.ariaHidden(el))"); }
                for (attribute, flag) in [("checked",query.checked),("disabled",query.disabled),("expanded",query.expanded),("pressed",query.pressed),("selected",query.selected)] {
                    if let Some(flag) = flag {
                        let value = match attribute { "disabled" => "f.disabled(el)".to_string(), "checked" => "(el.checked === true || el.getAttribute('aria-checked') === 'true')".into(), "selected" => "(el.selected === true || el.getAttribute('aria-selected') === 'true')".into(), _ => format!("(el.getAttribute('aria-{attribute}') === 'true')") };
                        filters.push_str(&format!(".filter(el => {value} === {flag})"));
                    }
                }
                filters
            }
            Engine::Union(_, _) | Engine::Intersect(_, _) => unreachable!(),
        }
    }

    /// JS expression evaluating to an [`ElementState`] snapshot.
    #[must_use]
    pub fn state_expression(&self) -> String {
        let resolve = self.resolve_js();
        format!(
            "(() => {{ const f = {}; const els = {resolve}; const first = els[0]; \
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
             const r = f.box(el); \
             return r; }}); \
             return {{ count: els.length, \
             visible: visible(first), \
             enabled: first ? !f.disabled(first) : false, \
             receives_events: first ? f.receives(first) : false, \
             checked: first ? Boolean(first.checked) : false, \
             editable: editable(first), \
             focused: first ? document.activeElement === first : false, \
             text: first ? (first.textContent || '').trim() : '', \
             value: first && 'value' in first ? String(first.value) : '', \
             rects }}; }})()",
            include_str!("dom.js")
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
            "fill" | "clear" => &format!(
                "el.focus(); const text = {arg}; \
                 if (el.isContentEditable) {{ el.textContent = text; {fire} return {{ ok: true }}; }} \
                 if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) {{ \
                 const proto = el instanceof HTMLInputElement ? HTMLInputElement.prototype : HTMLTextAreaElement.prototype; \
                 Object.getOwnPropertyDescriptor(proto, 'value').set.call(el, text); {fire} return {{ ok: true }}; }} \
                 return {{ ok: false, error: 'not fillable' }};"
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
                 const source = JSON.parse({arg}); \
                 if (source.length > 1 && !el.multiple) \
                 return {{ ok: false, error: 'multiple files require a multiple input' }}; \
                 const picked = source.map(f => {{ \
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

#[cfg(test)]
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
    /// Include hidden matches when `Some(true)`; hidden matches are excluded
    /// by default.
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
    description: Option<String>,
}

tokio::task_local! {
    static LOCATOR_DIAGNOSTIC: bool;
}

impl std::fmt::Display for Locator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.description().unwrap_or_else(|| self.selector()))
    }
}

impl std::fmt::Debug for Locator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Locator")
            .field("selector", &self.selector)
            .field("description", &self.description)
            .finish()
    }
}

impl Locator {
    pub(crate) fn new(page: Page, mut selector: Selector) -> Self {
        selector.strict = true;
        Self {
            page,
            selector,
            description: None,
        }
    }

    /// Describe this locator without changing its selector or the original handle.
    /// An empty description clears the label. Derived selectors clear labels;
    /// cloning and timeout/cancellation decorators preserve them.
    #[must_use]
    pub fn describe(&self, description: impl Into<String>) -> Self {
        let description = description.into();
        let mut locator = self.clone();
        locator.description = (!description.is_empty()).then_some(description);
        locator
    }

    /// Custom description, or None when no label is set.
    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// Return an otherwise identical locator without a description.
    #[must_use]
    pub fn clear_description(&self) -> Self {
        self.describe("")
    }

    pub(crate) fn diagnostic_step<'a, T: Send + 'a>(
        &'a self,
        title: impl Into<String>,
        category: crate::StepCategory,
        future: impl std::future::Future<Output = E2eResult<T>> + Send + 'a,
    ) -> futures::future::BoxFuture<'a, E2eResult<T>> {
        Box::pin(self.diagnostic_step_local(title.into(), category, future))
    }

    pub(crate) async fn diagnostic_step_local<T>(
        &self,
        title: impl Into<String>,
        category: crate::StepCategory,
        future: impl std::future::Future<Output = E2eResult<T>>,
    ) -> E2eResult<T> {
        let title = title.into();
        let boundary = self.description.is_some()
            && !LOCATOR_DIAGNOSTIC
                .try_with(|active| *active)
                .unwrap_or(false);
        let title = match self.description() {
            Some(description) => format!("{title} [description: {description:?}]"),
            None => title,
        };
        self.page
            .auto_step_local(title.clone(), category, async {
                if !boundary {
                    return future.await;
                }
                let result = LOCATOR_DIAGNOSTIC
                    .scope(true, future)
                    .await
                    .map_err(|error| error.with_context(&title));
                self.page.record_locator_diagnostic(
                    serde_json::json!({
                        "operation":title,
                        "description":self.description(),
                        "selector":self.selector(),
                        "error":result.as_ref().err().map(|error| serde_json::json!({
                            "code":error.code(),"message":error.to_string()
                        }))
                    })
                    .to_string(),
                );
                result
            })
            .await
    }

    /// Apply a caller cancellation signal to operations on this clone.
    pub fn with_cancellation(&self, token: crate::CancellationToken) -> Self {
        Self {
            page: self.page.with_cancellation(token),
            selector: self.selector.clone(),
            description: self.description.clone(),
        }
    }
    /// Override action, protocol and wait defaults for this clone; zero disables them.
    pub fn with_timeout(&self, timeout: Duration) -> Self {
        Self {
            page: self.page.with_timeout(timeout),
            selector: self.selector.clone(),
            description: self.description.clone(),
        }
    }
    pub(crate) fn selector_value(&self) -> Selector {
        self.selector.clone()
    }

    /// Convert this iframe locator to a lazy frame locator.
    pub fn content_frame(&self) -> crate::FrameLocator {
        crate::FrameLocator::new(self.page.clone(), self.selector.clone())
            .with_owner_description(self.description.clone())
    }

    /// Locate an iframe inside this locator's matches.
    pub fn frame_locator(&self, selector: &str) -> crate::FrameLocator {
        self.locator(selector).content_frame()
    }

    /// Page owning this locator.
    pub fn page(&self) -> Page {
        self.page.owning_page()
    }

    /// Restrict matching to visible elements.
    pub fn visible(&self) -> Self {
        let mut out = self.clone();
        out.description = None;
        out.selector.visible = Some(true);
        out
    }

    /// Match normalized text exactly (applies to text/label/attribute/role locators).
    pub fn exact(&self) -> Self {
        let mut out = self.clone();
        out.description = None;
        out.selector.exact = true;
        out
    }

    /// Match text using a JavaScript regular expression.
    pub fn matching(&self, pattern: &str) -> Self {
        let mut out = self.clone();
        out.description = None;
        out.selector.regex = Some(pattern.into());
        out
    }

    /// Evaluate with the element and a JSON-serializable argument.
    pub async fn evaluate_with_arg<T: serde::de::DeserializeOwned, A: serde::Serialize>(
        &self,
        function: &str,
        argument: &A,
    ) -> E2eResult<T> {
        self.diagnostic_step_local(
            format!("locator.evaluate_with_arg {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator evaluate_with_arg `{}`", self.selector.raw()),
                        async {
                            self.evaluate(&format!(
                                "el => ({function})(el, {})",
                                serde_json::to_string(argument)?
                            ))
                            .await
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Evaluate a function against every matched element.
    pub async fn evaluate_all<T: serde::de::DeserializeOwned>(
        &self,
        function: &str,
    ) -> E2eResult<T> {
        self.diagnostic_step_local(
            format!("locator.evaluate_all {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator evaluate_all `{}`", self.selector.raw()),
                        async {
                            self.page
                                .evaluate(&format!("({function})({})", self.selector.resolve_js()))
                                .await
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Raw textContent values, without trimming.
    pub async fn all_text_contents(&self) -> E2eResult<Vec<String>> {
        self.diagnostic_step(
            format!("locator.all_text_contents {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator all_text_contents `{}`", self.selector.raw()),
                        async {
                            self.evaluate_all("els => els.map(el => el.textContent || '')")
                                .await
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Rendered innerText values.
    pub async fn all_inner_texts(&self) -> E2eResult<Vec<String>> {
        self.diagnostic_step(
            format!("locator.all_inner_texts {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator all_inner_texts `{}`", self.selector.raw()),
                        async {
                            self.evaluate_all("els => els.map(el => el.innerText)")
                                .await
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Read raw textContent of the unique element.
    pub async fn text_content(&self) -> E2eResult<Option<String>> {
        self.diagnostic_step(
            format!("locator.text_content {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator text_content `{}`", self.selector.raw()),
                        async {
                            Ok(self
                                .eval_first("el.textContent")
                                .await?
                                .as_str()
                                .map(str::to_string))
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Read rendered innerText of the unique element.
    pub async fn inner_text(&self) -> E2eResult<Option<String>> {
        self.diagnostic_step(
            format!("locator.inner_text {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator inner_text `{}`", self.selector.raw()),
                        async {
                            Ok(self
                                .eval_first("el.innerText")
                                .await?
                                .as_str()
                                .map(str::to_string))
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Remove outlines created by highlight().
    pub async fn hide_highlight(&self) -> E2eResult<()> {
        self.diagnostic_step(format!("locator.hide_highlight {}", self.selector.raw()), crate::StepCategory::Action, async {
        self.page.run_operation(crate::operation::Deadline::new(self.page.timeout()).run(format!("locator hide_highlight `{}`",self.selector.raw()), async {
        self.evaluate_all::<Value>("els => { els.forEach(el => {el.style.outline = ''; el.style.outlineOffset = '';}); return true; }").await?;
        Ok(())

 })).await

        }).await
    }

    /// Capture the structured ARIA tree rooted at this element.
    pub async fn aria_snapshot_json(&self) -> E2eResult<Value> {
        self.diagnostic_step(
            format!("locator.aria_snapshot_json {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator aria_snapshot_json `{}`", self.selector.raw()),
                        async {
                            self.eval_first(&format!("({}).aria(el)", include_str!("dom.js")))
                                .await
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Capture indented ARIA snapshot text.
    pub async fn aria_snapshot(&self) -> E2eResult<String> {
        self.diagnostic_step(
            format!("locator.aria_snapshot {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator aria_snapshot `{}`", self.selector.raw()),
                        async {
                            self.evaluate(&format!(
                        "el => {{ const f = {}; return f.render(f.aria(el)).join('\\n'); }}",
                        include_str!("dom.js")
                    ))
                            .await
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Bounded DOM tree rooted at this element; boxes use its own frame viewport.
    pub async fn aria_snapshot_json_with(
        &self,
        options: crate::AriaSnapshotOptions,
    ) -> E2eResult<Value> {
        let options = options.json()?;
        self.diagnostic_step(
            format!("locator.aria_snapshot_json {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        "bounded ARIA snapshot",
                        async {
                            let text: String = self
                                .evaluate(&format!(
                                    "el => JSON.stringify(({}).ariaBounded(el, {options}))",
                                    include_str!("dom.js")
                                ))
                                .await?;
                            serde_json::from_str(&text).map_err(|error| {
                                crate::E2eError::Config(format!(
                                    "invalid ARIA snapshot JSON: {error}"
                                ))
                            })
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Bounded exact indented text, including selected boxes/state and truncation markers.
    pub async fn aria_snapshot_with(
        &self,
        options: crate::AriaSnapshotOptions,
    ) -> E2eResult<String> {
        let options = options.json()?;
        self.diagnostic_step(format!("locator.aria_snapshot {}", self.selector.raw()), crate::StepCategory::Action, async {
            self.page.run_operation(crate::operation::Deadline::new(self.page.timeout()).run("bounded ARIA snapshot", async {
                self.evaluate(&format!("el => {{ const f = {}; return f.render(f.ariaBounded(el, {options})).join('\\n'); }}", include_str!("dom.js"))).await
            })).await
        }).await
    }

    /// Wait for an element-scoped function to become truthy.
    pub async fn wait_for_function(&self, function: &str, timeout: Duration) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.wait_for_function {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(timeout).run(
                        format!(
                            "wait for locator wait_for_function `{}`",
                            self.selector.raw()
                        ),
                        async {
                            let scoped = self.with_timeout(timeout);
                            let deadline = crate::operation::Deadline::new(timeout);
                            loop {
                                if scoped
                                    .evaluate::<bool>(&format!(
                                        "async el => Boolean(await ({function})(el))"
                                    ))
                                    .await
                                    .unwrap_or(false)
                                {
                                    return Ok(());
                                }
                                if deadline.expired() {
                                    return Err(E2eError::Timeout(
                                        timeout.as_millis() as u64,
                                        format!("locator function: {}", scoped.selector()),
                                    ));
                                }
                                tokio::time::sleep(Duration::from_millis(50)).await;
                            }
                        },
                    ))
                    .await
            },
        )
        .await
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
        sub.strict = true;
        Self {
            page: self.page.clone(),
            selector: sub,
            description: None,
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
            description: None,
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
            description: None,
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
            description: None,
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
            description: None,
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
            let error = E2eError::Locator {
                selector: self.selector.raw().to_string(),
                message: "cannot combine locators from different pages".to_string(),
            };
            return Err(match self.description() {
                Some(description) => error.with_context(&format!(
                    "locator.{} {} [description: {description:?}]",
                    if union { "or_" } else { "and_" },
                    self.selector()
                )),
                None => error,
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
            pick: Pick::All,
            scope: None,
            has_text: Vec::new(),
            has_not_text: Vec::new(),
            has: Vec::new(),
            has_not: Vec::new(),
            strict: true,
            exact: false,
            regex: None,
            visible: None,
        };
        Ok(Self {
            page: self.page.clone(),
            selector,
            description: None,
        })
    }

    /// One locator per match.
    pub async fn all(&self) -> E2eResult<Vec<Self>> {
        self.diagnostic_step(
            format!("locator.all {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator all `{}`", self.selector.raw()),
                        async {
                            let count = self.count().await?;
                            Ok((0..count).map(|index| self.nth(index)).collect())
                        },
                    ))
                    .await
            },
        )
        .await
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
        self.diagnostic_step(
            format!("locator.count {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(
                        crate::operation::Deadline::new(self.page.timeout())
                            .run(format!("locator count `{}`", self.selector.raw()), async {
                                Ok(self.page.query_state(&self.selector).await?.count)
                            }),
                    )
                    .await
            },
        )
        .await
    }

    async fn single_state(&self) -> E2eResult<ElementState> {
        self.page
            .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                format!("locator single_state `{}`", self.selector.raw()),
                async {
                    let state = self.page.query_state(&self.selector).await?;
                    if self.selector.strict && state.count > 1 {
                        return Err(E2eError::Locator {
                            selector: self.selector().into(),
                            message: "strict mode violation: multiple elements match".into(),
                        });
                    }
                    Ok(state)
                },
            ))
            .await
    }

    /// Current state snapshot.
    pub async fn state(&self) -> E2eResult<ElementState> {
        self.diagnostic_step(
            format!("locator.state {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator state `{}`", self.selector.raw()),
                        async {
                            self.page.run_locator_handlers().await?;
                            self.page.query_state(&self.selector).await
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Wait until at least one match exists.
    pub async fn wait_for(&self, timeout: Duration) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.wait_for {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(timeout).run(
                        format!("wait for locator wait_for `{}`", self.selector.raw()),
                        async { self.wait_for_state(WaitForState::Attached, timeout).await },
                    ))
                    .await
            },
        )
        .await
    }

    /// Wait until the locator reaches `state`.
    pub async fn wait_for_state(&self, state: WaitForState, timeout: Duration) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.wait_for_state {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(timeout).run(
                        format!("wait for locator wait_for_state `{}`", self.selector.raw()),
                        async {
                            let scoped = self.with_timeout(timeout);
                            let deadline = crate::operation::Deadline::new(timeout);
                            loop {
                                if let Ok(current) = scoped.page.query_state(&scoped.selector).await
                                {
                                    let done = match state {
                                        WaitForState::Attached => current.count > 0,
                                        WaitForState::Detached => current.count == 0,
                                        WaitForState::Visible => {
                                            current.count > 0 && current.visible
                                        }
                                        WaitForState::Hidden => {
                                            current.count == 0 || !current.visible
                                        }
                                    };
                                    if done {
                                        return Ok(());
                                    }
                                }
                                if deadline.expired() {
                                    return Err(E2eError::Timeout(
                                        timeout.as_millis().min(u128::from(u64::MAX)) as u64,
                                        format!(
                                            "wait for `{}` to be {}",
                                            scoped.selector.raw(),
                                            state.as_str()
                                        ),
                                    ));
                                }
                                tokio::time::sleep(Duration::from_millis(50)).await;
                            }
                        },
                    ))
                    .await
            },
        )
        .await
    }

    async fn ready_state(
        &self,
        options: &LocatorOptions,
        enabled: bool,
        receives: bool,
    ) -> E2eResult<ElementState> {
        self.page
            .run_operation(
                crate::operation::Deadline::new(
                    options.timeout.unwrap_or_else(|| self.page.timeout()),
                )
                .run(
                    format!("locator ready_state `{}`", self.selector.raw()),
                    async {
                        let timeout = options.timeout.unwrap_or_else(|| self.page.timeout());
                        let deadline = crate::operation::Deadline::new(timeout);
                        let mut previous = None;
                        loop {
                            self.page.run_locator_handlers().await?;
                            let state = self.page.query_state(&self.selector).await?;
                            if self.selector.strict && state.count > 1 {
                                return Err(E2eError::Locator {
                                    selector: self.selector.raw().into(),
                                    message: "strict mode violation: multiple elements match"
                                        .into(),
                                });
                            }
                            if state.count == 1
                                && state.visible
                                && (!enabled || state.enabled)
                                && (!receives || state.receives_events)
                            {
                                let rect = state.rects.first().cloned();
                                if previous == rect {
                                    return Ok(state);
                                }
                                previous = rect;
                            } else {
                                previous = None;
                            }
                            if deadline.expired() {
                                return Err(E2eError::Locator {
                    selector: self.selector.raw().into(),
                    message: "element not actionable: missing, hidden, disabled, moving or covered"
                        .into(),
                });
                            }
                            tokio::time::sleep(Duration::from_millis(50)).await;
                        }
                    },
                ),
            )
            .await
    }

    fn center(state: &ElementState) -> Option<(f64, f64)> {
        state
            .rects
            .first()
            .map(|r| (r.x + r.width / 2.0, r.y + r.height / 2.0))
    }

    /// Click the element (trusted mouse input by default).
    pub async fn click(&self) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.click {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(
                        crate::operation::Deadline::new(self.page.timeout())
                            .run(format!("locator click `{}`", self.selector.raw()), async {
                                self.click_with_options(ClickOptions::default()).await
                            }),
                    )
                    .await
            },
        )
        .await
    }

    /// Click with position, modifiers, trial readiness and a scoped timeout.
    pub async fn click_with_options(&self, options: ClickOptions) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.click_with_options {}", self.selector()),
            crate::StepCategory::Action,
            async {
                let locator = options
                    .timeout
                    .map(|timeout| self.with_timeout(timeout))
                    .unwrap_or_else(|| self.clone());
                locator
                    .diagnostic_step(
                        format!("locator.click_with_options {}", self.selector.raw()),
                        crate::StepCategory::Action,
                        locator.page.run_operation(
                            crate::operation::Deadline::new(locator.page.timeout()).run(
                                format!("locator click_with_options `{}`", self.selector.raw()),
                                async {
                                    let point = locator
                                        .ready_point(&options.action_options(), true)
                                        .await?;
                                    if options.trial {
                                        return Ok(());
                                    }
                                    let mut input = crate::action_options::InputGuard::new(
                                        locator.page.clone(),
                                    );
                                    input.press(&locator.page, &options.modifiers).await?;
                                    input.mouse(options.button, point.0, point.1);
                                    let result = locator
                                        .page
                                        .mouse_click_with(
                                            point.0,
                                            point.1,
                                            crate::MouseClickOptions {
                                                button: options.button,
                                                click_count: options.click_count.max(1),
                                                delay: options.delay,
                                            },
                                        )
                                        .await;
                                    if result.is_ok() {
                                        input.mouse_completed();
                                    }
                                    let cleanup = input.release().await;
                                    result.and(cleanup)
                                },
                            ),
                        ),
                    )
                    .await
            },
        )
        .await
    }

    async fn ready_point(
        &self,
        options: &crate::ActionOptions,
        enabled: bool,
    ) -> E2eResult<(f64, f64)> {
        if options
            .position
            .is_some_and(|p| !p.x.is_finite() || !p.y.is_finite() || p.x < 0.0 || p.y < 0.0)
        {
            return Err(E2eError::Config(
                "action position must be finite and nonnegative".into(),
            ));
        }
        self.page.action(&self.selector, "scroll", None).await?;
        loop {
            let state = if options.force {
                self.page.query_state(&self.selector).await?
            } else {
                self.ready_state(&LocatorOptions { timeout: None }, enabled, false)
                    .await?
            };
            if self.selector.strict && state.count > 1 {
                return Err(E2eError::Locator {
                    selector: self.selector.raw().into(),
                    message: "strict mode violation: multiple elements match".into(),
                });
            }
            if state.count != 1 {
                return Err(E2eError::Locator {
                    selector: self.selector.raw().into(),
                    message: "no matching actionable element".into(),
                });
            }
            let position = options
                .position
                .map(|p| serde_json::json!({"x":p.x,"y":p.y}))
                .unwrap_or(Value::Null);
            let point=self.eval_first(&format!(r#"(() => {{
                const position={position}; const r=el.getBoundingClientRect();
                if(!r.width || !r.height) throw new Error('element has no bounding box');
                const axisAligned=e=>{{const transform=e.ownerDocument.defaultView.getComputedStyle(e).transform;
                    if(transform==='none')return;const m=new e.ownerDocument.defaultView.DOMMatrixReadOnly(transform);
                    if(!m.is2D || m.b || m.c || m.a<=0 || m.d<=0)throw new Error('rotated or reflected action coordinates are unsupported');}};
                if(position) axisAligned(el);
                if(position && (position.x>el.clientWidth || position.y>el.clientHeight))throw new Error('action position is outside the padding box');
                const sx=el.offsetWidth ? r.width/el.offsetWidth : 1,sy=el.offsetHeight ? r.height/el.offsetHeight : 1;
                let x=position ? r.x+(el.clientLeft+position.x)*sx : Math.max(0,Math.min(innerWidth-1,r.x+r.width/2));
                let y=position ? r.y+(el.clientTop+position.y)*sy : Math.max(0,Math.min(innerHeight-1,r.y+r.height/2));
                const hit=(doc,x,y)=>{{let h=doc.elementFromPoint(x,y);while(h?.shadowRoot){{const child=h.shadowRoot.elementFromPoint(x,y);if(!child || child===h)break;h=child;}}return h;}};
                let target=hit(document,x,y),receives=!!target && (target===el || el.contains(target));
                let w=window;
                while(w.parent!==w){{const frame=w.frameElement;if(!frame)throw new Error('cross-origin action coordinates are unsupported');axisAligned(frame);
                    const fr=frame.getBoundingClientRect(),fx=fr.width/frame.offsetWidth,fy=fr.height/frame.offsetHeight;
                    x=fr.x+(frame.clientLeft+x)*fx;y=fr.y+(frame.clientTop+y)*fy;
                    target=hit(frame.ownerDocument,x,y);receives=receives && target===frame;w=w.parent;
                }}
                return {{x,y,receives}};
            }})()"#)).await?;
            if options.force || point["receives"].as_bool() == Some(true) {
                return Ok((
                    point["x"]
                        .as_f64()
                        .ok_or_else(|| E2eError::Config("action point unavailable".into()))?,
                    point["y"]
                        .as_f64()
                        .ok_or_else(|| E2eError::Config("action point unavailable".into()))?,
                ));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Double-click the element.
    pub async fn dblclick(&self) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.dblclick {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator dblclick `{}`", self.selector.raw()),
                        async {
                            self.click_with_options(ClickOptions {
                                force: false,
                                click_count: 2,
                                button: crate::page::MouseButton::Left,
                                delay: Duration::ZERO,
                                ..Default::default()
                            })
                            .await
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Hover with native input.
    pub async fn hover(&self) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.hover {}", self.selector()),
            crate::StepCategory::Action,
            async {
                self.hover_with_options(crate::ActionOptions::default())
                    .await
            },
        )
        .await
    }
    pub async fn hover_with_options(&self, options: crate::ActionOptions) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.hover_with_options {}", self.selector()),
            crate::StepCategory::Action,
            async {
                let locator = options
                    .timeout
                    .map(|timeout| self.with_timeout(timeout))
                    .unwrap_or_else(|| self.clone());
                locator
                    .diagnostic_step(
                        format!("locator.hover {}", self.selector.raw()),
                        crate::StepCategory::Action,
                        locator.page.run_operation(
                            crate::operation::Deadline::new(locator.page.timeout()).run(
                                format!("locator hover `{}`", self.selector.raw()),
                                async {
                                    let point = locator.ready_point(&options, false).await?;
                                    if options.trial {
                                        return Ok(());
                                    }
                                    let mut input = crate::action_options::InputGuard::new(
                                        locator.page.clone(),
                                    );
                                    input.press(&locator.page, &options.modifiers).await?;
                                    let result = locator.page.mouse_move(point.0, point.1).await;
                                    let cleanup = input.release().await;
                                    result.and(cleanup)
                                },
                            ),
                        ),
                    )
                    .await
            },
        )
        .await
    }

    /// Focus the element.
    pub async fn focus(&self) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.focus {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator focus `{}`", self.selector.raw()),
                        async {
                            self.page.action(&self.selector, "focus", None).await?;
                            Ok(())
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Blur the element.
    pub async fn blur(&self) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.blur {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator blur `{}`", self.selector.raw()),
                        async {
                            self.page.action(&self.selector, "blur", None).await?;
                            Ok(())
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Tap the element's center via the touchscreen.
    pub async fn tap(&self) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.tap {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator tap `{}`", self.selector.raw()),
                        async {
                            self.page.action(&self.selector, "scroll", None).await?;
                            let state = self
                                .ready_state(&LocatorOptions { timeout: None }, true, true)
                                .await?;
                            match Self::center(&state) {
                                Some((x, y)) => self.page.touchscreen_tap(x, y).await,
                                None => Err(E2eError::Locator {
                                    selector: self.selector.raw().to_string(),
                                    message: "element has no bounding box".to_string(),
                                }),
                            }
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Drag the element to a target on the same page, with at least one move.
    pub async fn drag_to(&self, target: &Locator, steps: u32) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.drag_to {}", self.selector()),
            crate::StepCategory::Action,
            async {
                self.drag_to_with_options(target, crate::DragOptions::default().steps(steps))
                    .await
            },
        )
        .await
    }
    pub async fn drag_to_with_options(
        &self,
        target: &Locator,
        options: crate::DragOptions,
    ) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.drag_to_with_options {}", self.selector()),
            crate::StepCategory::Action,
            async {
                if self.page.target_id() != target.page.target_id() {
                    return Err(E2eError::Config(
                        "drag_to needs locators on the same page".into(),
                    ));
                }
                if options.steps == 0 {
                    return Err(E2eError::Config("drag_to needs at least 1 step".into()));
                }
                let locator = options
                    .action
                    .timeout
                    .map(|timeout| self.with_timeout(timeout))
                    .unwrap_or_else(|| self.clone());
                let target = target.with_timeout(locator.page.timeout());
                locator
                    .diagnostic_step(
                        format!("locator.drag_to {}", self.selector.raw()),
                        crate::StepCategory::Action,
                        locator.page.run_operation(
                            crate::operation::Deadline::new(locator.page.timeout()).run(
                                format!("locator drag_to `{}`", self.selector.raw()),
                                async {
                                    let from = locator.ready_point(&options.action, false).await?;
                                    let mut target_options = options.action.clone();
                                    target_options.position = options.target_position;
                                    let to = target.ready_point(&target_options, false).await?;
                                    if options.action.trial {
                                        return Ok(());
                                    }
                                    let mut input = crate::action_options::InputGuard::new(
                                        locator.page.clone(),
                                    );
                                    input
                                        .press(&locator.page, &options.action.modifiers)
                                        .await?;
                                    input.mouse(crate::MouseButton::Left, to.0, to.1);
                                    let result =
                                        locator.page.mouse_drag(from, to, options.steps).await;
                                    if result.is_ok() {
                                        input.mouse_completed();
                                    }
                                    let cleanup = input.release().await;
                                    result.and(cleanup)
                                },
                            ),
                        ),
                    )
                    .await
            },
        )
        .await
    }

    /// Scroll the element into the center of the viewport.
    pub async fn scroll_into_view(&self) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.scroll_into_view {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator scroll_into_view `{}`", self.selector.raw()),
                        async {
                            self.page.action(&self.selector, "scroll", None).await?;
                            Ok(())
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Dispatch a bubbling `CustomEvent` with an optional JSON `detail`.
    pub async fn dispatch_event(&self, name: &str, detail: Option<&Value>) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.dispatch_event {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator dispatch_event `{}`", self.selector.raw()),
                        async {
                            let name_json = serde_json::to_string(name).unwrap_or_default();
                            let detail_json = detail
                                .map(|value| {
                                    serde_json::to_string(value)
                                        .unwrap_or_else(|_| "null".to_string())
                                })
                                .unwrap_or_else(|| "null".to_string());
                            let value = self
                                .eval_first(&format!(
                                    "el.dispatchEvent(new CustomEvent({name_json}, \
                 {{ bubbles: true, detail: {detail_json} }}))"
                                ))
                                .await?;
                            self.require_match(value)
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Dispatch a synthetic event with native constructor/init semantics.
    /// Successful dispatch includes events canceled by preventDefault().
    pub async fn dispatch_event_with(
        &self,
        name: &str,
        options: crate::DispatchEventOptions,
    ) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.dispatch_event_with {}", self.selector()),
            crate::StepCategory::Action,
            async {
        if !options.init.is_object() {
            return Err(E2eError::Config("event init must be a JSON object".into()));
        }
        let name = serde_json::to_string(name)?;
        let init = serde_json::to_string(&options.init)?;
        let constructor = serde_json::to_string(options.kind.constructor())?;
        self.diagnostic_step(
            format!("locator.dispatch_event_with {}", self.selector()),
            crate::StepCategory::Action,
            async {
                let value = self.eval_first(&format!(r#"(() => {{
                    const type = {name}, init = Object.assign({{ bubbles: true, cancelable: true, composed: true }}, {init});
                    let name = {constructor};
                    if (name === 'auto') {{
                        if (/^(?:click|dblclick|auxclick|contextmenu|mouse(?:down|up|move|over|out|enter|leave))$/.test(type)) name = 'MouseEvent';
                        else if (/^(?:keydown|keyup|keypress)$/.test(type)) name = 'KeyboardEvent';
                        else if (/^(?:focus|blur|focusin|focusout)$/.test(type)) name = 'FocusEvent';
                        else if (/^(?:pointer(?:down|up|move|over|out|enter|leave|cancel)|gotpointercapture|lostpointercapture)$/.test(type)) name = 'PointerEvent';
                        else if (type === 'wheel') name = 'WheelEvent';
                        else if (/^(?:dragstart|drag|dragenter|dragleave|dragover|drop|dragend)$/.test(type)) name = 'DragEvent';
                        else name = 'Event';
                    }}
                    const Constructor = el.ownerDocument.defaultView[name];
                    if (typeof Constructor !== 'function') throw new Error('unsupported DOM event constructor ' + name);
                    el.dispatchEvent(new Constructor(type, init));
                    return true;
                }})()"#)).await?;
                self.require_match(value)
            }
        ).await

            },
        ).await
    }

    /// Native intersection ratio, including clipping ancestors and the owning
    /// frame's viewport. The observer is disconnected after its first sample.
    pub async fn intersection_ratio(&self) -> E2eResult<f64> {
        self.diagnostic_step(
            format!("locator.intersection_ratio {}", self.selector()),
            crate::StepCategory::Action,
            async {
        self.evaluate("el => new Promise(resolve => { const observer = new el.ownerDocument.defaultView.IntersectionObserver(entries => { observer.disconnect(); resolve(entries[0].intersectionRatio); }); observer.observe(el); })")
            .await

            },
        ).await
    }

    /// Select the element's text (input value or rendered text).
    pub async fn select_text(&self) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.select_text {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator select_text `{}`", self.selector.raw()),
                        async {
                            let value = self
                                .eval_first(
                                    "(() => { if (el.select) el.select(); \
                 else getSelection().selectAllChildren(el); return true; })()",
                                )
                                .await?;
                            self.require_match(value)
                        },
                    ))
                    .await
            },
        )
        .await
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

    /// Screenshot this element (PNG bytes).
    pub async fn screenshot(&self) -> E2eResult<Vec<u8>> {
        self.screenshot_with(crate::ScreenshotOptions::default())
            .await
    }

    /// Capture this element with masks, scale, background and temporary style controls.
    /// full_page and clip are rejected because the element defines the region.
    pub async fn screenshot_with(&self, options: crate::ScreenshotOptions) -> E2eResult<Vec<u8>> {
        self.diagnostic_step(
            format!("locator.screenshot {}", self.selector.raw()),
            crate::StepCategory::Action,
            crate::screenshot::capture(
                &self.page,
                options,
                crate::screenshot::Source::Element(Box::new(self.clone())),
            ),
        )
        .await
    }

    pub(crate) async fn screenshot_rect(&self) -> E2eResult<crate::ElementRect> {
        self.page.action(&self.selector, "scroll", None).await?;
        let state = self
            .ready_state(&LocatorOptions { timeout: None }, false, false)
            .await?;
        state
            .rects
            .first()
            .cloned()
            .ok_or_else(|| E2eError::Locator {
                selector: self.selector.raw().into(),
                message: "element has no bounding box".into(),
            })
    }

    /// Fill an input/textarea/select with text (replaces the value).
    pub async fn fill(&self, text: &str) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.fill {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator fill `{}`", self.selector.raw()),
                        async {
                            self.page.action(&self.selector, "fill", Some(text)).await?;
                            Ok(())
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Type text char-by-char with trusted input (keeps existing value).
    pub async fn press_sequentially(&self, text: &str) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.press_sequentially {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator press_sequentially `{}`", self.selector.raw()),
                        async {
                            self.press_sequentially_with(text, KeyPressOptions::default())
                                .await
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Press a key while the element is focused.
    pub async fn press(&self, key: &str) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.press {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator press `{}`", self.selector.raw()),
                        async {
                            self.focus().await?;
                            self.page.press_key(key).await
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Press a key with explicit options (down/up delay).
    pub async fn press_with(&self, key: &str, options: KeyPressOptions) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.press_with {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator press_with `{}`", self.selector.raw()),
                        async {
                            self.focus().await?;
                            self.page.press_key_with(key, options).await
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Type text with a delay between keystrokes.
    pub async fn press_sequentially_with(
        &self,
        text: &str,
        options: KeyPressOptions,
    ) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.press_sequentially_with {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator press_sequentially_with `{}`", self.selector.raw()),
                        async {
                            self.focus().await?;
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
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Clear an input/textarea.
    pub async fn clear(&self) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.clear {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator clear `{}`", self.selector.raw()),
                        async {
                            self.page.action(&self.selector, "clear", None).await?;
                            Ok(())
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Set files on a file input (an empty list clears it). Files are read
    /// from disk (64 MiB total cap) and injected via `DataTransfer`, firing
    /// `input`/`change`.
    pub async fn set_input_files<P: AsRef<Path>>(&self, paths: &[P]) -> E2eResult<()> {
        self.diagnostic_step_local(
            format!("locator.set_input_files {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator set_input_files `{}`", self.selector.raw()),
                        async {
                            let mut files = Vec::with_capacity(paths.len());
                            let mut total = 0usize;
                            for path in paths {
                                let path = path.as_ref();
                                let metadata = tokio::fs::metadata(path).await.map_err(|e| {
                                    E2eError::Config(format!("cannot read {}: {e}", path.display()))
                                })?;
                                if !metadata.is_file() {
                                    return Err(E2eError::Config(format!(
                                        "upload {} is not a file",
                                        path.display()
                                    )));
                                }
                                if metadata.len()
                                    > crate::file_payload::MAX_UPLOAD_BYTES.saturating_sub(total)
                                        as u64
                                {
                                    return Err(E2eError::Config(
                                        "set_input_files payload exceeds 64 MiB".into(),
                                    ));
                                }
                                let bytes = tokio::fs::read(path).await.map_err(|e| {
                                    E2eError::Config(format!("cannot read {}: {e}", path.display()))
                                })?;
                                total = total.checked_add(bytes.len()).ok_or_else(|| {
                                    E2eError::Config("upload size overflow".into())
                                })?;
                                if total > crate::file_payload::MAX_UPLOAD_BYTES {
                                    return Err(E2eError::Config(
                                        "set_input_files payload exceeds 64 MiB".into(),
                                    ));
                                }
                                let name = path
                                    .file_name()
                                    .map(|n| n.to_string_lossy().into_owned())
                                    .unwrap_or_default();
                                files.push(crate::FilePayload::new(
                                    &name,
                                    guess_mime(&name),
                                    bytes,
                                ));
                            }
                            self.set_input_file_payloads(&files).await
                        },
                    ))
                    .await
            },
        )
        .await
    }
    /// Upload generated bytes with explicit filenames and MIME types (64 MiB
    /// total cap). Multiple files require a multiple input; an empty list clears.
    pub async fn set_input_file_payloads(&self, files: &[crate::FilePayload]) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.set_input_file_payloads {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator set_input_file_payloads `{}`", self.selector.raw()),
                        async {
                            let payload = crate::file_payload::encode_payloads(files)?;
                            self.page
                                .action(&self.selector, "set_input_files", Some(&payload))
                                .await?;
                            Ok(())
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Check a checkbox/radio.
    pub async fn check(&self) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.check {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(
                        crate::operation::Deadline::new(self.page.timeout())
                            .run(format!("locator check `{}`", self.selector.raw()), async {
                                self.change_checked(true).await
                            }),
                    )
                    .await
            },
        )
        .await
    }

    /// Uncheck a checkbox.
    pub async fn uncheck(&self) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.uncheck {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator uncheck `{}`", self.selector.raw()),
                        async { self.change_checked(false).await },
                    ))
                    .await
            },
        )
        .await
    }

    async fn change_checked(&self, checked: bool) -> E2eResult<()> {
        self.set_checked_with_options(checked, crate::ActionOptions::default())
            .await
    }
    pub async fn check_with_options(&self, options: crate::ActionOptions) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.check_with_options {}", self.selector()),
            crate::StepCategory::Action,
            async { self.set_checked_with_options(true, options).await },
        )
        .await
    }
    pub async fn uncheck_with_options(&self, options: crate::ActionOptions) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.uncheck_with_options {}", self.selector()),
            crate::StepCategory::Action,
            async { self.set_checked_with_options(false, options).await },
        )
        .await
    }
    pub async fn set_checked_with_options(
        &self,
        checked: bool,
        options: crate::ActionOptions,
    ) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.set_checked_with_options {}", self.selector()),
            crate::StepCategory::Action,
            async {
        let locator = options
            .timeout
            .map(|timeout| self.with_timeout(timeout))
            .unwrap_or_else(|| self.clone());
        locator.diagnostic_step(format!("locator.set_checked {}",self.selector.raw()),crate::StepCategory::Action,
            locator.page.run_operation(crate::operation::Deadline::new(locator.page.timeout()).run(format!("locator set_checked `{}`",self.selector.raw()),async {
                let current=locator.eval_first("(() => {if(!(el instanceof HTMLInputElement) || !['checkbox','radio'].includes(el.type))throw new Error('element is not a checkbox or radio');return el.checked;})()").await?;
                if current.as_bool()==Some(checked) && !options.trial {return Ok(());}
                locator.click_with_options(ClickOptions{force:options.force,position:options.position,modifiers:options.modifiers,trial:options.trial,timeout:None,..Default::default()}).await?;
                if !options.trial && locator.is_checked().await?!=checked {return Err(E2eError::Config(format!("click did not set {} to checked={checked}",self.selector())));}
                Ok(())
            }))
        ).await

            },
        ).await
    }

    /// Select an `<option>` by value.
    pub async fn select_option(&self, value: &str) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.select_option {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator select_option `{}`", self.selector.raw()),
                        async {
                            self.page
                                .action(&self.selector, "select", Some(value))
                                .await?;
                            Ok(())
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Select `<option>`s by value, label, or index (every entry must match;
    /// single-selects keep the last match).
    pub async fn select_options(&self, options: &[SelectOption]) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.select_options {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator select_options `{}`", self.selector.raw()),
                        async {
                            let wants: Vec<Value> = options
                                .iter()
                                .map(|option| match option {
                                    SelectOption::Value(value) => {
                                        serde_json::json!({ "value": value })
                                    }
                                    SelectOption::Label(label) => {
                                        serde_json::json!({ "label": label })
                                    }
                                    SelectOption::Index(index) => {
                                        serde_json::json!({ "index": index })
                                    }
                                })
                                .collect();
                            let argument = serde_json::to_string(&wants).unwrap_or_default();
                            self.page
                                .action(&self.selector, "select_many", Some(&argument))
                                .await?;
                            Ok(())
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Values of the selected `<option>`s (empty when not a select).
    pub async fn selected_options(&self) -> E2eResult<Vec<String>> {
        self.diagnostic_step(
            format!("locator.selected_options {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator selected_options `{}`", self.selector.raw()),
                        async {
                            let value = self
            .eval_first(
                "el instanceof HTMLSelectElement ? [...el.selectedOptions].map(o => o.value) : []",
            )
            .await?;
                            serde_json::from_value(value).map_err(E2eError::Json)
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Read trimmed text content.
    pub async fn text(&self) -> E2eResult<String> {
        self.diagnostic_step(
            format!("locator.text {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(
                        crate::operation::Deadline::new(self.page.timeout())
                            .run(format!("locator text `{}`", self.selector.raw()), async {
                                self.evaluate("el => (el.textContent || '').trim()").await
                            }),
                    )
                    .await
            },
        )
        .await
    }

    /// Read the form value of the unique input.
    pub async fn input_value(&self) -> E2eResult<String> {
        self.diagnostic_step(format!("locator.input_value {}", self.selector.raw()), crate::StepCategory::Action, async {
        self.page.run_operation(crate::operation::Deadline::new(self.page.timeout()).run(format!("locator input_value `{}`",self.selector.raw()), async {
        self.evaluate("el => { if (!el.matches('input,textarea,select')) throw new Error('not an input element'); return el.value; }").await

 })).await

        }).await
    }

    /// Evaluate `projection` (an expression over `el`) on the first match.
    ///
    /// Returns [`Value::Null`] when nothing matches.
    async fn eval_first(&self, projection: &str) -> E2eResult<Value> {
        self.page
            .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                format!("locator eval_first `{}`", self.selector.raw()),
                async {
                    let timeout = self.page.timeout();
                    let deadline = crate::operation::Deadline::new(timeout);
                    loop {
                        self.page.run_locator_handlers().await?;
                        let state = self.single_state().await?;
                        if state.count > 0 {
                            break;
                        }
                        if deadline.expired() {
                            return Err(E2eError::Timeout(
                                timeout.as_millis().min(u64::MAX as u128) as u64,
                                format!("waiting for {}", self.selector()),
                            ));
                        }
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
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
                },
            ))
            .await
    }

    /// Wait for one element and read an attribute (`None` if the attribute is absent).
    pub async fn attribute(&self, name: &str) -> E2eResult<Option<String>> {
        self.diagnostic_step(
            format!("locator.attribute {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator attribute `{}`", self.selector.raw()),
                        async {
                            let name_json = serde_json::to_string(name).unwrap_or_default();
                            let value = self
                                .eval_first(&format!("el.getAttribute({name_json})"))
                                .await?;
                            Ok(value.as_str().map(str::to_string))
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Wait for one element and read a computed CSS property.
    pub async fn css_value(&self, property: &str) -> E2eResult<Option<String>> {
        self.diagnostic_step(
            format!("locator.css_value {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator css_value `{}`", self.selector.raw()),
                        async {
                            let property_json = serde_json::to_string(property).unwrap_or_default();
                            let value = self
                                .eval_first(&format!(
                                    "getComputedStyle(el).getPropertyValue({property_json})"
                                ))
                                .await?;
                            Ok(value.as_str().map(str::to_string))
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Wait for one element and read a DOM property as JSON (`None` when the
    /// property is `null`/`undefined`).
    pub async fn js_property(&self, name: &str) -> E2eResult<Option<Value>> {
        self.diagnostic_step(
            format!("locator.js_property {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator js_property `{}`", self.selector.raw()),
                        async {
                            let name_json = serde_json::to_string(name).unwrap_or_default();
                            let value = self.eval_first(&format!("el[{name_json}]")).await?;
                            if value.is_null() {
                                Ok(None)
                            } else {
                                Ok(Some(value))
                            }
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Wait for one element and read its innerHTML.
    pub async fn inner_html(&self) -> E2eResult<Option<String>> {
        self.diagnostic_step(
            format!("locator.inner_html {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator inner_html `{}`", self.selector.raw()),
                        async {
                            let value = self.eval_first("el.innerHTML").await?;
                            Ok(value.as_str().map(str::to_string))
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Whether the first match intersects the viewport.
    pub async fn in_viewport(&self) -> E2eResult<bool> {
        self.diagnostic_step(
            format!("locator.in_viewport {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator in_viewport `{}`", self.selector.raw()),
                        async { Ok(self.intersection_ratio().await? > 0.0) },
                    ))
                    .await
            },
        )
        .await
    }

    /// Accessible name (`aria-label`, `alt`, then text).
    ///
    /// An approximation of the full accessible-name computation.
    pub async fn accessible_name(&self) -> E2eResult<Option<String>> {
        self.diagnostic_step(
            format!("locator.accessible_name {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator accessible_name `{}`", self.selector.raw()),
                        async {
                            let value = self
                                .eval_first(&format!("({}).name(el)", include_str!("dom.js")))
                                .await?;
                            Ok(value.as_str().map(str::to_string))
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Accessible description from referenced elements, aria-description or title.
    pub async fn accessible_description(&self) -> E2eResult<Option<String>> {
        self.diagnostic_step(
            format!("locator.accessible_description {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator accessible_description `{}`", self.selector.raw()),
                        async {
                            let value = self
                                .eval_first(&format!(
                                    "({}).description(el)",
                                    include_str!("dom.js")
                                ))
                                .await?;
                            Ok(value.as_str().map(str::to_string))
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Implicit or explicit ARIA role.
    pub async fn role(&self) -> E2eResult<Option<String>> {
        self.diagnostic_step(
            format!("locator.role {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator role `{}`", self.selector.raw()),
                        async {
                            let value = self
                                .eval_first(&format!("({}).role(el)", include_str!("dom.js")))
                                .await?;
                            Ok(value.as_str().map(str::to_string))
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Accessible error message referenced by aria-errormessage.
    pub async fn accessible_error_message(&self) -> E2eResult<Option<String>> {
        self.diagnostic_step(format!("locator.accessible_error_message {}", self.selector.raw()), crate::StepCategory::Action, async {
        self.page.run_operation(crate::operation::Deadline::new(self.page.timeout()).run(format!("locator accessible_error_message `{}`",self.selector.raw()), async {
        let value = self.eval_first("(el.getAttribute('aria-errormessage') || '').split(/\\s+/).map(id => document.getElementById(id)?.textContent || '').join(' ').trim()").await?;
        Ok(value.as_str().map(str::to_string))

 })).await

        }).await
    }

    /// Whether the first match is visible (immediate, no retry).
    pub async fn is_visible(&self) -> E2eResult<bool> {
        self.diagnostic_step(
            format!("locator.is_visible {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator is_visible `{}`", self.selector.raw()),
                        async { Ok(self.single_state().await?.visible) },
                    ))
                    .await
            },
        )
        .await
    }

    /// Whether the first match is hidden or absent (immediate, no retry).
    pub async fn is_hidden(&self) -> E2eResult<bool> {
        self.diagnostic_step(
            format!("locator.is_hidden {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator is_hidden `{}`", self.selector.raw()),
                        async {
                            let state = self.single_state().await?;
                            Ok(state.count == 0 || !state.visible)
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Whether the first match is enabled (immediate, no retry).
    pub async fn is_enabled(&self) -> E2eResult<bool> {
        self.diagnostic_step(
            format!("locator.is_enabled {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator is_enabled `{}`", self.selector.raw()),
                        async { Ok(self.single_state().await?.enabled) },
                    ))
                    .await
            },
        )
        .await
    }

    /// Whether the first match is disabled (immediate, no retry).
    pub async fn is_disabled(&self) -> E2eResult<bool> {
        self.diagnostic_step(
            format!("locator.is_disabled {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator is_disabled `{}`", self.selector.raw()),
                        async { Ok(!self.single_state().await?.enabled) },
                    ))
                    .await
            },
        )
        .await
    }

    /// Whether the first match is checked (immediate, no retry).
    pub async fn is_checked(&self) -> E2eResult<bool> {
        self.diagnostic_step(
            format!("locator.is_checked {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator is_checked `{}`", self.selector.raw()),
                        async { Ok(self.single_state().await?.checked) },
                    ))
                    .await
            },
        )
        .await
    }

    /// Whether the first match is editable (immediate, no retry).
    pub async fn is_editable(&self) -> E2eResult<bool> {
        self.diagnostic_step(
            format!("locator.is_editable {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator is_editable `{}`", self.selector.raw()),
                        async { Ok(self.single_state().await?.editable) },
                    ))
                    .await
            },
        )
        .await
    }

    /// Whether the first match is focused (immediate, no retry).
    pub async fn is_focused(&self) -> E2eResult<bool> {
        self.diagnostic_step(
            format!("locator.is_focused {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator is_focused `{}`", self.selector.raw()),
                        async { Ok(self.single_state().await?.focused) },
                    ))
                    .await
            },
        )
        .await
    }

    /// Whether at least one element matches (immediate, no retry).
    pub async fn is_attached(&self) -> E2eResult<bool> {
        self.diagnostic_step(
            format!("locator.is_attached {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator is_attached `{}`", self.selector.raw()),
                        async { Ok(self.single_state().await?.count > 0) },
                    ))
                    .await
            },
        )
        .await
    }

    /// Bounding box of the first match (`None` when nothing matches).
    pub async fn bounding_box(&self) -> E2eResult<Option<crate::page::ElementRect>> {
        self.diagnostic_step(
            format!("locator.bounding_box {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator bounding_box `{}`", self.selector.raw()),
                        async { Ok(self.single_state().await?.rects.into_iter().next()) },
                    ))
                    .await
            },
        )
        .await
    }

    /// Outline the first match with a red box for a moment (debugging).
    pub async fn highlight(&self) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.highlight {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator highlight `{}`", self.selector.raw()),
                        async {
                            let value = self
                                .eval_first(
                                    "(() => { el.style.outline = '2px solid #ff0000'; \
                 el.style.outlineOffset = '1px'; return true; })()",
                                )
                                .await?;
                            self.require_match(value)
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Check or uncheck a checkbox to reach `checked`.
    pub async fn set_checked(&self, checked: bool) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.set_checked {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator set_checked `{}`", self.selector.raw()),
                        async {
                            if checked {
                                self.check().await
                            } else {
                                self.uncheck().await
                            }
                        },
                    ))
                    .await
            },
        )
        .await
    }

    /// Run `function` with the first match as its argument.
    ///
    /// `function` is a JS function expression, e.g. `(el) => el.id`.
    pub async fn evaluate<T: serde::de::DeserializeOwned>(&self, function: &str) -> E2eResult<T> {
        self.diagnostic_step_local(
            format!("locator.evaluate {}", self.selector.raw()),
            crate::StepCategory::Action,
            async {
                self.page
                    .run_operation(crate::operation::Deadline::new(self.page.timeout()).run(
                        format!("locator evaluate `{}`", self.selector.raw()),
                        async {
                            let value = self.eval_first(&format!("(({function}))(el)")).await?;
                            if value.is_null() {
                                return Err(E2eError::Locator {
                                    selector: self.selector.raw().to_string(),
                                    message: "no matching element".to_string(),
                                });
                            }
                            Ok(serde_json::from_value(value)?)
                        },
                    ))
                    .await
            },
        )
        .await
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
        assert!(expression.contains("([el]).flatMap"), "{expression}");
        assert!(
            expression.contains("f.query(root, \"button\")"),
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
        assert!(expression.contains("f.query(root, '*')"));
        assert!(expression.contains("he\\\"llo"));
        let css = Selector::parse(".a".to_string());
        assert!(css.state_expression().contains("f.query(root, \".a\")"));
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
        assert!(matches!(Selector::by_label("Name").engine, Engine::Label));
        assert!(
            matches!(Selector::by_placeholder("ENTER").engine, Engine::Attribute(ref name) if name == "placeholder")
        );
        assert!(
            matches!(Selector::by_alt("logo").engine, Engine::Attribute(ref name) if name == "alt")
        );
        assert!(
            matches!(Selector::by_title("docs").engine, Engine::Attribute(ref name) if name == "title")
        );
    }

    #[test]
    fn pick_narrows_resolution() {
        let base = Selector::parse(".a".to_string());
        let mut nth = base.clone();
        nth.pick = Pick::Nth(2);
        assert!(nth.resolve_js().contains(".slice(2, 3)"));
        let mut last = base.clone();
        last.pick = Pick::Last;
        assert!(last.resolve_js().contains(".slice(-1)"));
        // Name truncation inside the shared DOM helper also uses String.slice.
        // A pick slices the parenthesized selector result.
        assert!(!base.resolve_js().contains(").slice("));
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
        assert!(expression.contains("f.query(root, \"form\")"));
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
            pick: Pick::All,
            scope: None,
            has_text: Vec::new(),
            has_not_text: Vec::new(),
            has: Vec::new(),
            has_not: Vec::new(),
            strict: false,
            exact: false,
            regex: None,
            visible: None,
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
            pick: Pick::All,
            scope: None,
            has_text: Vec::new(),
            has_not_text: Vec::new(),
            has: Vec::new(),
            has_not: Vec::new(),
            strict: false,
            exact: false,
            regex: None,
            visible: None,
        };
        let expression = both.resolve_js();
        assert!(expression.contains("keep.has(el)"));
    }
}
