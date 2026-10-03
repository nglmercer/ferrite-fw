//! Framework adapters and evidence-based capability registry.
//! Vue/Svelte plugins require explicit integration; official Node compiler calls are experimental.
//! React Refresh remains experimental.

pub mod compiler_host;
pub mod doctor;
pub mod hosted;
pub mod react;
pub mod registry;
pub mod scaffold;
pub mod svelte;
pub mod vue;

pub use hosted::HostedFrameworkPlugin;
pub use react::ReactPlugin;
pub use svelte::SveltePlugin;
pub use vue::VuePlugin;

/// Join an importer-relative specifier onto an absolute importer id,
/// preserving any `?query`. Lexical only (no fs access).
pub(crate) fn join_relative(importer: &str, specifier: &str) -> String {
    let (spec_path, query) = match specifier.split_once('?') {
        Some((path, query)) => (path, format!("?{query}")),
        None => (specifier, String::new()),
    };
    let base = importer.split('?').next().unwrap_or(importer);
    let dir = base.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("");
    let mut parts: Vec<&str> = dir.split('/').filter(|part| !part.is_empty()).collect();
    for part in spec_path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    format!("/{joined}{query}", joined = parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::join_relative;

    #[test]
    fn joins_relative_specs() {
        assert_eq!(join_relative("/src/a.ts", "./b.vue"), "/src/b.vue");
        assert_eq!(
            join_relative("/src/a.ts", "../b.vue?vue&type=script"),
            "/b.vue?vue&type=script"
        );
        assert_eq!(join_relative("/a.ts", "./d/./e.vue"), "/d/e.vue");
    }
}
