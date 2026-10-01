//! Configuration bridge between `ferrite e2e` and consumer test processes.

use crate::{E2eConfig, E2eError, E2eResult};

/// Read the complete CLI configuration, with legacy environment overrides.
/// Invalid configuration fails explicitly rather than silently using defaults.
pub fn config_from_env() -> E2eResult<E2eConfig> {
    config_from_variables(|name| std::env::var(name).ok())
}

fn config_from_variables(read: impl Fn(&str) -> Option<String>) -> E2eResult<E2eConfig> {
    let mut config = match read("FERRITE_E2E_CONFIG") {
        Some(json) => serde_json::from_str(&json)?,
        None => E2eConfig::default(),
    };
    for (name, field) in [
        ("BROWSER", &mut config.browser),
        ("REPORTER", &mut config.reporter),
        ("VIDEO", &mut config.video),
        ("OUTPUT_DIR", &mut config.output_dir),
        ("PRESERVE_OUTPUT", &mut config.preserve_output),
        ("UPDATE_SNAPSHOTS", &mut config.update_snapshots),
    ] {
        let variable = if name == "UPDATE_SNAPSHOTS" {
            "FERRITE_UPDATE_SNAPSHOTS".to_string()
        } else {
            format!("FERRITE_E2E_{name}")
        };
        if let Some(value) = read(&variable) {
            *field = value;
        }
    }
    if let Some(value) = read("FERRITE_E2E_RUN_NAME") {
        config.run_name = Some(value);
    }
    if let Some(value) = read("FERRITE_E2E_METADATA") {
        config.metadata = serde_json::from_str(&value)?;
    }
    if let Some(value) = read("FERRITE_E2E_SLOW_TESTS") {
        config.report_slow_tests = serde_json::from_str(&value)?;
    }
    if let Some(base) = read("FERRITE_E2E_BASE_URL") {
        config.base_url = Some(base);
    }
    for (name, field) in [
        ("FAIL_ON_FLAKY_TESTS", &mut config.fail_on_flaky_tests),
        ("FORBID_ONLY", &mut config.forbid_only),
    ] {
        if let Some(value) = read(&format!("FERRITE_E2E_{name}")) {
            *field = match value.to_ascii_lowercase().as_str() {
                "true" | "1" => true,
                "false" | "0" => false,
                _ => {
                    return Err(E2eError::Config(format!(
                        "invalid FERRITE_E2E_{name}: {value:?}"
                    )))
                }
            };
        }
    }
    for name in [
        "WORKERS",
        "RETRIES",
        "TIMEOUT_MS",
        "EXPECT_TIMEOUT_MS",
        "VIDEO_FPS",
        "GLOBAL_TIMEOUT_MS",
        "MAX_FAILURES",
        "CLEANUP_TIMEOUT_MS",
    ] {
        if let Some(value) = read(&format!("FERRITE_E2E_{name}")) {
            let n: u64 = value
                .parse()
                .map_err(|_| E2eError::Config(format!("invalid FERRITE_E2E_{name}: {value:?}")))?;
            match name {
                "WORKERS" => {
                    config.workers = usize::try_from(n)
                        .map_err(|_| E2eError::Config("workers overflow".into()))?
                }
                "RETRIES" => {
                    config.retries =
                        u32::try_from(n).map_err(|_| E2eError::Config("retries overflow".into()))?
                }
                "VIDEO_FPS" => {
                    config.video_fps = u32::try_from(n)
                        .map_err(|_| E2eError::Config("video_fps overflow".into()))?
                }
                "TIMEOUT_MS" => config.timeout_ms = n,
                "GLOBAL_TIMEOUT_MS" => config.global_timeout_ms = n,
                "CLEANUP_TIMEOUT_MS" => config.cleanup_timeout_ms = n,
                "MAX_FAILURES" => {
                    config.max_failures = usize::try_from(n)
                        .map_err(|_| E2eError::Config("max_failures overflow".into()))?
                }
                _ => config.expect_timeout_ms = n,
            }
        }
    }
    for (name, field) in [
        ("FILTER", &mut config.filter),
        ("GREP", &mut config.grep),
        ("GREP_INVERT", &mut config.grep_invert),
        ("SNAPSHOT_DIR", &mut config.snapshot_dir),
    ] {
        let variable = if name == "SNAPSHOT_DIR" {
            "FERRITE_SNAPSHOT_DIR".into()
        } else {
            format!("FERRITE_E2E_{name}")
        };
        if let Some(value) = read(&variable).filter(|value| !value.is_empty()) {
            *field = Some(value);
        }
    }
    if let Some(value) = read("FERRITE_E2E_REPEAT_EACH") {
        config.repeat_each = value
            .parse()
            .map_err(|_| E2eError::Config(format!("invalid FERRITE_E2E_REPEAT_EACH: {value:?}")))?;
    }
    if let Some(value) = read("FERRITE_E2E_PROJECT") {
        config.selected_projects = value
            .split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_string)
            .collect();
    }
    if let Some(value) = read("FERRITE_E2E_SHARD") {
        let (index, total) = value
            .split_once('/')
            .ok_or_else(|| E2eError::Config(format!("invalid FERRITE_E2E_SHARD: {value:?}")))?;
        config.shard = Some((
            index
                .trim()
                .parse()
                .map_err(|_| E2eError::Config("invalid shard index".into()))?,
            total
                .trim()
                .parse()
                .map_err(|_| E2eError::Config("invalid shard total".into()))?,
        ));
    }
    if let Some(template) = read("FERRITE_SNAPSHOT_PATH_TEMPLATE") {
        config.snapshot_path_template = Some(template);
    }
    validate_config(&config)?;
    Ok(config)
}

pub(crate) fn validate_config(config: &E2eConfig) -> E2eResult<()> {
    crate::BrowserKind::parse(&config.browser)?;
    if let Some(options) = config.report_slow_tests {
        options.validate().map_err(E2eError::Config)?;
    }
    crate::OutputRetention::parse(&config.preserve_output)?;
    crate::VideoMode::parse(&config.video)?;
    crate::SnapshotUpdate::parse(&config.update_snapshots)?;
    for template in config.snapshot_path_template.iter().chain(
        config
            .projects
            .iter()
            .filter_map(|p| p.snapshot_path_template.as_ref()),
    ) {
        crate::snapshot_path::validate_template(template)?;
    }
    let paths = std::iter::once(config.output_dir.as_str())
        .chain(config.snapshot_dir.as_deref())
        .chain(config.projects.iter().flat_map(|project| {
            project
                .output_dir
                .as_deref()
                .into_iter()
                .chain(project.snapshot_dir.as_deref())
        }));
    if paths.into_iter().any(str::is_empty) {
        return Err(E2eError::Config(
            "artifact/snapshot directory cannot be empty".into(),
        ));
    }
    if let Some((index, total)) = config.shard {
        if index == 0 || total == 0 || index > total {
            return Err(E2eError::Config(
                "shard index must be within 1..=total".into(),
            ));
        }
    }
    let mut names = std::collections::HashSet::new();
    for project in &config.projects {
        if project.name.trim().is_empty() || !names.insert(project.name.clone()) {
            return Err(E2eError::Config(format!(
                "empty or duplicate project name {:?}",
                project.name
            )));
        }
        if let Some(browser) = &project.browser {
            crate::BrowserKind::parse(browser)?;
        }
    }
    // Selection is validated at the run boundary: consumers may register named
    // projects with Runner::project after loading shared/environment settings.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_environment_preserves_null_empty_and_validates_limits() {
        let config = config_from_variables(|name| match name {
            "FERRITE_E2E_CONFIG" => Some(
                r#"{"run_name":"base","metadata":{"origin":"base"},"report_slow_tests":{"max":5}}"#
                    .into(),
            ),
            "FERRITE_E2E_RUN_NAME" => Some("".into()),
            "FERRITE_E2E_METADATA" => Some("{}".into()),
            "FERRITE_E2E_SLOW_TESTS" => Some("null".into()),
            _ => None,
        })
        .unwrap();
        assert_eq!(config.run_name.as_deref(), Some(""));
        assert!(config.metadata.unwrap().is_empty());
        assert!(config.report_slow_tests.is_none());
        let cleared =
            config_from_variables(|name| (name == "FERRITE_E2E_METADATA").then(|| "null".into()))
                .unwrap();
        assert!(cleared.metadata.is_none());
        for (key, value) in [
            ("FERRITE_E2E_METADATA", "[]"),
            ("FERRITE_E2E_SLOW_TESTS", r#"{"max":1001}"#),
            ("FERRITE_E2E_SLOW_TESTS", r#"{"threshold_ms":-1}"#),
            (
                "FERRITE_E2E_CONFIG",
                r#"{"report_slow_tests":{"max":1001}}"#,
            ),
        ] {
            assert!(
                config_from_variables(|name| (name == key).then(|| value.into())).is_err(),
                "{key}: {value}"
            );
        }
    }

    #[test]
    fn snapshot_templates_survive_json_environment_and_validate_all_projects() {
        let config=config_from_variables(|name|match name {
            "FERRITE_E2E_CONFIG"=>Some(r#"{"snapshot_path_template":"{snapshotDir}/{arg}{ext}","projects":[{"name":"desktop","snapshot_path_template":"{testDir}/{projectName}/{arg}{ext}"}]}"#.into()),
            "FERRITE_SNAPSHOT_PATH_TEMPLATE"=>Some("{snapshotDir}/{browserName}/{platform}/{arg}{ext}".into()),
            _=>None,
        }).unwrap();
        assert_eq!(
            config.snapshot_path_template.as_deref(),
            Some("{snapshotDir}/{browserName}/{platform}/{arg}{ext}")
        );
        assert_eq!(
            config.projects[0].snapshot_path_template.as_deref(),
            Some("{testDir}/{projectName}/{arg}{ext}")
        );
        for bad in ["", "{unknown}", "{arg"] {
            assert!(config_from_variables(
                |name| (name == "FERRITE_SNAPSHOT_PATH_TEMPLATE").then(|| bad.into())
            )
            .is_err());
            assert!(config_from_variables(|name|(name=="FERRITE_E2E_CONFIG").then(||serde_json::json!({"projects":[{"name":"desktop","snapshot_path_template":bad}]}).to_string())).is_err());
        }
        let old = config_from_variables(|name| (name == "FERRITE_E2E_CONFIG").then(|| "{}".into()))
            .unwrap();
        assert!(old.snapshot_path_template.is_none());
    }

    #[test]
    fn ci_policy_bridge_validates_booleans_and_explicit_false_overrides_json() {
        for (value, expected) in [
            ("true", true),
            ("1", true),
            ("TRUE", true),
            ("false", false),
            ("0", false),
        ] {
            let config = config_from_variables(|name| match name {
                "FERRITE_E2E_CONFIG" => {
                    Some(r#"{"fail_on_flaky_tests":true,"forbid_only":true}"#.into())
                }
                "FERRITE_E2E_FAIL_ON_FLAKY_TESTS" | "FERRITE_E2E_FORBID_ONLY" => Some(value.into()),
                _ => None,
            })
            .unwrap();
            assert_eq!(config.fail_on_flaky_tests, expected);
            assert_eq!(config.forbid_only, expected);
        }
        for name in ["FERRITE_E2E_FAIL_ON_FLAKY_TESTS", "FERRITE_E2E_FORBID_ONLY"] {
            for value in ["", "yes", "2", " true "] {
                let error =
                    config_from_variables(|variable| (variable == name).then(|| value.into()))
                        .unwrap_err();
                assert!(error.to_string().contains(name));
            }
        }
        let historical =
            config_from_variables(|name| (name == "FERRITE_E2E_CONFIG").then(|| "{}".into()))
                .unwrap();
        assert!(!historical.fail_on_flaky_tests && !historical.forbid_only);
    }

    #[test]
    fn project_selection_paths_and_repetitions_survive_bridge() {
        let json = serde_json::json!({
            "repeat_each": 3, "filter": "", "grep": "json", "snapshot_dir": "baseline",
            "projects": [{"name":"desktop", "browser":"firefox", "grep_invert":"skip",
                "repeat_each":0, "retries":0, "timeout_ms":0, "output_dir":"desktop-output",
                "snapshot_dir":"desktop-baseline", "viewport":{"width":600,"height":400}}],
            "selected_projects":["desktop"], "shard":[1,2]
        })
        .to_string();
        let config = config_from_variables(|name| match name {
            "FERRITE_E2E_CONFIG" => Some(json.clone()),
            "FERRITE_E2E_REPEAT_EACH" => Some("5".into()),
            "FERRITE_E2E_GREP" => Some("legacy".into()),
            "FERRITE_E2E_GREP_INVERT" => Some("excluded".into()),
            "FERRITE_E2E_FILTER" => Some("".into()),
            "FERRITE_SNAPSHOT_DIR" => Some("legacy-baseline".into()),
            "FERRITE_E2E_SHARD" => Some(" 2 / 3 ".into()),
            "FERRITE_E2E_PROJECT" => Some(" desktop, ".into()),
            _ => None,
        })
        .unwrap();
        assert_eq!(config.repeat_each, 5);
        assert_eq!(config.filter.as_deref(), Some(""));
        assert_eq!(config.grep.as_deref(), Some("legacy"));
        assert_eq!(config.grep_invert.as_deref(), Some("excluded"));
        assert_eq!(config.snapshot_dir.as_deref(), Some("legacy-baseline"));
        assert_eq!(config.shard, Some((2, 3)));
        assert_eq!(config.selected_projects, ["desktop"]);
        let project = &config.projects[0];
        assert_eq!(project.repeat_each, Some(0));
        assert_eq!(project.retries, Some(0));
        assert_eq!(project.timeout_ms, Some(0));
        assert_eq!(project.viewport.as_ref().unwrap().width, 600);
        let runner = crate::Runner::try_from_config(&config).unwrap();
        drop(runner);
        let manual = config_from_variables(|name| {
            (name == "FERRITE_E2E_PROJECT").then(|| "registered-in-rust".into())
        })
        .unwrap();
        assert!(manual.projects.is_empty());
        let _runner = crate::Runner::try_from_config(&manual)
            .unwrap()
            .project(crate::Project::new("registered-in-rust"));
        let empty = config_from_variables(|name| {
            matches!(name, "FERRITE_E2E_FILTER" | "FERRITE_E2E_GREP_INVERT").then(String::new)
        })
        .unwrap();
        assert!(empty.filter.is_none());
        assert!(empty.grep_invert.is_none());
    }

    #[test]
    fn invalid_project_selection_paths_and_counts_fail_before_run() {
        for json in [
            r#"{"projects":[{"name":""}]}"#,
            r#"{"projects":[{"name":"x"},{"name":"x"}]}"#,
            r#"{"projects":[{"name":"x","browser":"webkit"}]}"#,
            r#"{"projects":[{"name":"x","output_dir":""}]}"#,
            r#"{"snapshot_dir":""}"#,
            r#"{"shard":[0,2]}"#,
            r#"{"shard":[3,2]}"#,
        ] {
            assert!(
                config_from_variables(|name| (name == "FERRITE_E2E_CONFIG").then(|| json.into()))
                    .is_err(),
                "{json}"
            );
        }
        for (name, value) in [
            ("FERRITE_E2E_REPEAT_EACH", "4294967296"),
            ("FERRITE_E2E_REPEAT_EACH", "-1"),
            ("FERRITE_E2E_SHARD", "1/x"),
            ("FERRITE_E2E_SHARD", "1/0"),
        ] {
            assert!(config_from_variables(|key| (key == name).then(|| value.into())).is_err());
        }
        let mut invalid = E2eConfig::default();
        invalid.projects.push(crate::E2eProjectConfig {
            name: "bad".into(),
            browser: Some("webkit".into()),
            ..Default::default()
        });
        assert!(crate::Runner::try_from_config(&invalid).is_err());
        // The non-fallible constructor remains available; run reports the validation error.
        let _ = crate::Runner::from_config(&invalid);
    }

    #[test]
    fn runner_limits_survive_json_and_legacy_overrides() {
        let config = config_from_variables(|name| match name {
            "FERRITE_E2E_CONFIG" => Some(
                r#"{"global_timeout_ms":800,"max_failures":2,"cleanup_timeout_ms":250}"#.into(),
            ),
            "FERRITE_E2E_MAX_FAILURES" => Some("3".into()),
            _ => None,
        })
        .unwrap();
        assert_eq!(config.global_timeout_ms, 800);
        assert_eq!(config.max_failures, 3);
        assert_eq!(config.cleanup_timeout_ms, 250);
        let default = E2eConfig::default();
        assert_eq!(default.global_timeout_ms, 0);
        assert_eq!(default.max_failures, 0);
        assert_eq!(default.cleanup_timeout_ms, 5000);
    }
    #[test]
    fn complete_config_preserved_and_legacy_overrides_win() {
        let json = serde_json::json!({"browser":"firefox","headless":false,"args":["--custom"],"user_agent":"agent","proxy_server":"http://proxy:8080","viewport":{"width":800,"height":600},"expect_timeout_ms":900,"slow_mo_ms":12}).to_string();
        let config = config_from_variables(|key| match key {
            "FERRITE_E2E_CONFIG" => Some(json.clone()),
            "FERRITE_E2E_WORKERS" => Some("3".into()),
            _ => None,
        })
        .unwrap();
        assert_eq!(config.browser, "firefox");
        assert!(!config.headless);
        assert_eq!(config.args, ["--custom"]);
        assert_eq!(config.workers, 3);
        assert_eq!(config.expect_timeout_ms, 900);
        assert_eq!(config.viewport.unwrap().width, 800);
        assert_eq!(config.user_agent.as_deref(), Some("agent"));
        assert_eq!(config.slow_mo_ms, 12);
    }
    #[test]
    fn invalid_and_overflowing_configuration_fails() {
        for (key, value) in [
            ("FERRITE_E2E_CONFIG", "bad json"),
            ("FERRITE_E2E_WORKERS", "many"),
            ("FERRITE_E2E_RETRIES", "4294967296"),
            ("FERRITE_E2E_BROWSER", "unsupported"),
        ] {
            assert!(config_from_variables(|name| (name == key).then(|| value.into())).is_err());
        }
    }
}
