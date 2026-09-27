//! Vite compat: environment kinds.

use ferrite::EnvironmentKind;

// --- env kinds ------------------------------------------------------------------------------------------

#[test]
fn environment_conditions_follow_spec() {
    assert!(EnvironmentKind::Ssr.is_ssr());
    assert!(!EnvironmentKind::Client.is_ssr());
    assert!(EnvironmentKind::Client
        .default_conditions()
        .contains(&"browser".to_string()));
}
