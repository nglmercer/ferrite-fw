//! Versioned, evidence-based framework capabilities shared by adapters.

/// Registry schema version; independent of framework package versions.
pub const SCHEMA_VERSION: u32 = 5;

/// Shared package-boundary search for physical JSX/TSX source files.
pub fn jsx_manifest_candidates(
    root: &std::path::Path,
    file: &std::path::Path,
) -> Vec<std::path::PathBuf> {
    let Some(parent) = file.parent() else {
        return Vec::new();
    };
    let mut candidates = Vec::new();
    for directory in parent.ancestors() {
        let manifest = directory.join("package.json");
        let exists = manifest.exists();
        candidates.push(manifest);
        if exists || directory == root {
            break;
        }
    }
    candidates
}

#[derive(Debug, serde::Serialize)]
pub struct JsxOwnership {
    pub selection: &'static str,
    pub status: &'static str,
    pub declared_owners: Vec<&'static str>,
    /// A lowering choice is not proof of a functioning framework adapter.
    pub lowering: Option<String>,
}

pub fn jsx_ownership(
    config: &ferrite_config::ResolvedConfig,
    manifest: &serde_json::Value,
) -> JsxOwnership {
    let declared_owners = jsx_owners(manifest);
    let (selection, status, lowering) = if let Some(framework) = &config.framework {
        if framework.enabled.iter().any(|name| name == "react") {
            ("explicit-framework", "selected", Some("react".into()))
        } else {
            ("explicit-framework", "unowned", None)
        }
    } else if config.react.import_source.is_some() || config.react.factory.is_some() {
        (
            "explicit-jsx-settings",
            "configured-lowering-unverified",
            config
                .react
                .import_source
                .clone()
                .or_else(|| config.react.factory.clone()),
        )
    } else if declared_owners.len() > 1 {
        ("manifest", "ambiguous", None)
    } else if declared_owners
        .first()
        .is_some_and(|owner| *owner != "react")
    {
        ("manifest", "unavailable", None)
    } else if declared_owners.is_empty() {
        ("legacy-default", "selected", Some("react".into()))
    } else {
        ("manifest", "selected", Some("react".into()))
    };
    JsxOwnership {
        selection,
        status,
        declared_owners,
        lowering,
    }
}

/// Direct package markers for JSX ownership detection, including unavailable
/// adapters. Detection is not evidence of framework/compiler support.
pub const JSX_OWNER_PACKAGES: &[&str] = &["react", "preact", "solid-js", "@builder.io/qwik"];

/// Detect package-local owners, excluding transitive dependencies and type packages.
pub fn jsx_owners(manifest: &serde_json::Value) -> Vec<&'static str> {
    JSX_OWNER_PACKAGES
        .iter()
        .copied()
        .filter(|name| {
            ["dependencies", "devDependencies", "peerDependencies"]
                .iter()
                .any(|section| {
                    manifest[*section]
                        .as_object()
                        .is_some_and(|entries| entries.contains_key(*name))
                })
        })
        .collect()
}

/// A capability is tested only after the entire acceptance profile executes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Support {
    Tested,
    Experimental,
    UpstreamManaged,
    Unavailable,
}

impl Support {
    pub fn label(self) -> &'static str {
        match self {
            Self::Tested => "tested",
            Self::Experimental => "experimental",
            Self::UpstreamManaged => "upstream-managed",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Compiler-only evidence. This does not advertise rendering, HMR or SSR runtime support.
#[derive(Debug)]
pub struct CompilerProfile {
    pub host: &'static str,
    pub framework_version: &'static str,
    pub client: Support,
    pub server: Support,
    pub requires_explicit_enable: bool,
    pub conformance_fixture: &'static str,
}

pub const VUE_NODE: CompilerProfile = CompilerProfile {
    host: "node",
    framework_version: "3.5.22",
    client: Support::Experimental,
    server: Support::Experimental,
    requires_explicit_enable: true,
    conformance_fixture: "compiler_host::actual_project_matched_compilers_client_server_and_runes",
};
pub const SVELTE_NODE: CompilerProfile = CompilerProfile {
    host: "node",
    framework_version: "5.39.6",
    client: Support::Experimental,
    server: Support::Experimental,
    requires_explicit_enable: true,
    conformance_fixture: "compiler_host::actual_project_matched_compilers_client_server_and_runes",
};

/// Framework compiler requirements and current rendering evidence.
#[derive(Debug)]
pub struct FrameworkDescriptor {
    pub name: &'static str,
    pub extensions: &'static [&'static str],
    pub template_variants: &'static [crate::scaffold::TemplateProfile],
    pub compiler_profiles: &'static [CompilerProfile],
    pub compiler_package: Option<&'static str>,
    pub client: Support,
    pub ssr: Support,
    /// Versions for which clean-directory acceptance tests have passed.
    pub tested_versions: &'static [&'static str],
}

pub const FRAMEWORKS: &[FrameworkDescriptor] = &[
    FrameworkDescriptor {
        name: "vanilla",
        extensions: &[],
        template_variants: crate::scaffold::TEMPLATES,
        compiler_profiles: &[],
        compiler_package: None,
        client: Support::Experimental,
        ssr: Support::Unavailable,
        tested_versions: &[],
    },
    FrameworkDescriptor {
        name: "react",
        extensions: &["jsx", "tsx"],
        template_variants: crate::scaffold::REACT_TEMPLATES,
        compiler_profiles: &[],
        compiler_package: None,
        client: Support::Experimental,
        ssr: Support::Unavailable,
        tested_versions: &[],
    },
    FrameworkDescriptor {
        name: "vue",
        extensions: &["vue"],
        template_variants: crate::scaffold::VUE_TEMPLATES,
        compiler_profiles: &[VUE_NODE],
        compiler_package: Some("vue/compiler-sfc"),
        client: Support::Experimental,
        ssr: Support::Experimental,
        tested_versions: &[],
    },
    FrameworkDescriptor {
        name: "svelte",
        extensions: &["svelte", "svelte.js", "svelte.ts"],
        template_variants: crate::scaffold::SVELTE_TEMPLATES,
        compiler_profiles: &[SVELTE_NODE],
        compiler_package: Some("svelte/compiler"),
        client: Support::Experimental,
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
        "cannot compile {id}: {framework} requires project-matched {compiler} on a validated compiler host; no compiler host has been explicitly configured for this plugin. Use a supported client JavaScript entry or the framework's upstream toolchain"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jsx_owner_markers_are_direct_and_do_not_advertise_adapters() {
        let manifest = serde_json::json!({
            "dependencies": {"react": "19", "preact": "10"},
            "peerDependencies": {"solid-js": "1"},
            "devDependencies": {"@builder.io/qwik": "1", "react": "19"}
        });
        assert_eq!(jsx_owners(&manifest), JSX_OWNER_PACKAGES);
        assert!(jsx_owners(&serde_json::json!({
            "dependencies": {"@types/react": "19", "vue": "3", "svelte": "5"},
            "description": "react preact solid-js"
        }))
        .is_empty());
        for owner in ["preact", "solid-js", "@builder.io/qwik"] {
            assert!(descriptor(owner).is_none());
        }
    }

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
            assert_eq!(
                descriptor(framework).unwrap().ssr,
                if framework == "vue" {
                    Support::Experimental
                } else {
                    Support::Unavailable
                }
            );
            assert_eq!(descriptor(framework).unwrap().client, Support::Experimental);
        }
    }
}
