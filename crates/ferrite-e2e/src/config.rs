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
    if let Some(base) = read("FERRITE_E2E_BASE_URL") {
        config.base_url = Some(base);
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
    crate::BrowserKind::parse(&config.browser)?;
    crate::VideoMode::parse(&config.video)?;
    crate::SnapshotUpdate::parse(&config.update_snapshots)?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

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
