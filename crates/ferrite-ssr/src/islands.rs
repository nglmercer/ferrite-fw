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
    let escaped = escape_attribute(&props);
    let name = escape_attribute(&island.name);
    format!(
        "<div data-ferrite-island=\"{}\" data-props=\"{}\">{}</div>",
        name, escaped, island.html
    )
}

/// Legacy flat `/islands/<name>.js` scanner. Configured SSR registries use the
/// selective hydration runtime instead.
#[must_use]
pub fn island_hydration_script() -> &'static str {
    r#"for (const el of document.querySelectorAll('[data-ferrite-island]')) {
  const name = el.getAttribute('data-ferrite-island');
  Promise.resolve().then(async () => {
    if (!name || !/^[A-Za-z0-9_-]+$/.test(name)) throw new Error('Invalid island name');
    const props = JSON.parse(el.getAttribute('data-props') || '{}');
    const mod = await import('/islands/' + encodeURIComponent(name) + '.js');
    if (typeof mod.hydrate !== 'function') throw new Error('Missing island hydrate export');
    await mod.hydrate(el, props);
  }).catch(error => el.dispatchEvent(new CustomEvent('ferrite:island-error', {bubbles:true, detail:error})));
}"#
}

/// Host-generated registries prepend statically discoverable module loaders.
#[doc(hidden)]
pub fn island_registry_runtime() -> &'static str {
    include_str!("islands_client.js")
}

fn escape_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\'', "&#39;")
}

/// Emit one selectively hydrated root. Server HTML is trusted renderer output;
/// component names and JSON props are escaped for HTML attributes.
pub fn island_tag_with_strategy(island: &Island, strategy: &str) -> ferrite_core::Result<String> {
    if !matches!(
        strategy,
        "load" | "idle" | "visible" | "interaction" | "manual"
    ) {
        return Err(ferrite_core::FerriteError::Ssr(format!(
            "unknown island hydration strategy `{strategy}`"
        )));
    }
    Ok(island_tag(island).replacen(
        " data-props=",
        &format!(" data-ferrite-hydrate=\"{strategy}\" data-props="),
        1,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn island_attributes_escape_entities_quotes_and_html_without_changing_server_nodes() {
        let island = Island {
            name: "name\"<&".into(),
            props: serde_json::json!({"label":"&quot; <safe> & 😀"}),
            html: "<button>SSR</button>".into(),
        };
        let html = island_tag_with_strategy(&island, "interaction").unwrap();
        assert!(html.contains("name&quot;&lt;&amp;"));
        assert!(html.contains("&amp;quot; &lt;safe&gt; &amp; 😀"));
        assert!(html.contains("data-ferrite-hydrate=\"interaction\""));
        assert!(html.ends_with("<button>SSR</button></div>"));
        assert!(island_tag_with_strategy(&island, "unknown").is_err());
    }
}
