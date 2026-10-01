//! Versioned, evidence-based framework capabilities shared by adapters.

/// Registry schema version; independent of framework package versions.
pub const SCHEMA_VERSION: u32 = 1;

/// A capability is tested only after the entire acceptance profile executes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Support {
    Tested,
    Experimental,
    UpstreamManaged,
    Unavailable,
}

/// Framework compiler requirements and current rendering evidence.
#[derive(Debug)]
pub struct FrameworkDescriptor {
    pub name: &'static str,
    pub extensions: &'static [&'static str],
    pub compiler_package: Option<&'static str>,
    pub client: Support,
    pub ssr: Support,
    /// Versions for which clean-directory acceptance tests have passed.
    pub tested_versions: &'static [&'static str],
}

pub const FRAMEWORKS: &[FrameworkDescriptor] = &[
    FrameworkDescriptor {
        name: "react",
        extensions: &["jsx", "tsx"],
        compiler_package: None,
        client: Support::Experimental,
        ssr: Support::Unavailable,
        tested_versions: &[],
    },
    FrameworkDescriptor {
        name: "vue",
        extensions: &["vue"],
        compiler_package: Some("vue/compiler-sfc"),
        client: Support::Unavailable,
        ssr: Support::Unavailable,
        tested_versions: &[],
    },
    FrameworkDescriptor {
        name: "svelte",
        extensions: &["svelte", "svelte.js", "svelte.ts"],
        compiler_package: Some("svelte/compiler"),
        client: Support::Unavailable,
        ssr: Support::Unavailable,
        tested_versions: &[],
    },
];

pub fn descriptor(name: &str) -> Option<&'static FrameworkDescriptor> {
    FRAMEWORKS.iter().find(|framework| framework.name == name)
}

/// Match path suffixes only; query values and lookalike filenames cannot claim ownership.
pub fn owns_component(framework: &str, id: &str) -> bool {
    let path = id.split(['?', '#']).next().unwrap_or(id);
    descriptor(framework).is_some_and(|framework| {
        framework.extensions.iter().any(|extension| {
            path.strip_suffix(extension)
                .is_some_and(|prefix| prefix.ends_with('.'))
        })
    })
}

pub(crate) fn compiler_unavailable(framework: &str, id: &str) -> ferrite_core::FerriteError {
    let compiler = descriptor(framework)
        .and_then(|framework| framework.compiler_package)
        .unwrap_or(framework);
    ferrite_core::FerriteError::Build(format!(
        "cannot compile {id}: {framework} requires project-matched {compiler} on a validated compiler host; this build has no such adapter. Use a supported client JavaScript entry or the framework's upstream toolchain"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn component_ownership_is_precise() {
        assert!(owns_component("vue", "/App.vue?vue&type=template"));
        assert!(owns_component("svelte", "/counter.svelte.ts"));
        for id in ["/App.vue.js", "/file.js?name=App.vue", "/App.vue-backup"] {
            assert!(!owns_component("vue", id), "{id}");
        }
        assert!(!owns_component("unknown", "/App.vue"));
    }

    #[test]
    fn unavailable_compilers_produce_actionable_errors() {
        for framework in ["vue", "svelte"] {
            let error = compiler_unavailable(framework, "/App").to_string();
            assert!(error.contains(descriptor(framework).unwrap().compiler_package.unwrap()));
            assert!(error.contains("validated compiler host"));
            assert_eq!(descriptor(framework).unwrap().client, Support::Unavailable);
        }
    }
}
