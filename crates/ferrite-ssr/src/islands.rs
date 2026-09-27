//! SSR islands.

/// An SSR island (§49): server-rendered HTML + selective hydration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Island {
    /// Component name.
    pub name: String,
    /// Serialized props.
    pub props: serde_json::Value,
    /// Server-rendered HTML.
    pub html: String,
}

/// Render an island placeholder div.
#[must_use]
pub fn island_tag(island: &Island) -> String {
    let props = serde_json::to_string(&island.props).unwrap_or_else(|_| "{}".to_string());
    let escaped = props.replace('"', "&quot;");
    format!(
        "<div data-ferrite-island=\"{}\" data-props=\"{}\">{}</div>",
        island.name, escaped, island.html
    )
}

/// Client hydration scanner for islands (§49).
#[must_use]
pub fn island_hydration_script() -> &'static str {
    r#"for (const el of document.querySelectorAll("[data-ferrite-island]")) {
  const name = el.getAttribute("data-ferrite-island");
  const props = JSON.parse(el.getAttribute("data-props") || "{}");
  import(`/islands/${name}.js`).then((mod) => mod.hydrate?.(el, props));
}
"#
}
