//! Fixture smoke test for the native CLI adapter.

use std::process::Command;

#[test]
fn build_command_compiles_a_fixture() {
    let fixture = format!("{}/tests/fixtures/basic.html", env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(env!("CARGO_BIN_EXE_utilitycss-cli"))
        .args(["build", "--pretty", "--stats", &fixture])
        .output()
        .expect("CLI binary should start");

    assert!(output.status.success(), "CLI failed: {}", String::from_utf8_lossy(&output.stderr));
    let css = String::from_utf8(output.stdout).expect("CLI CSS should be UTF-8");
    assert!(css.contains(".flex {"));
    assert!(css.contains("padding: 1rem;"));
    assert!(css.contains("@media (min-width: 768px)"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("stats:"));
}

#[test]
fn config_file_changes_theme_and_output_mode() {
    let fixture = format!("{}/tests/fixtures/basic.html", env!("CARGO_MANIFEST_DIR"));
    let config = format!("{}/tests/fixtures/custom.json", env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(env!("CARGO_BIN_EXE_utilitycss-cli"))
        .args(["build", "--config", &config, &fixture])
        .output()
        .expect("CLI binary should start");

    assert!(output.status.success(), "CLI failed: {}", String::from_utf8_lossy(&output.stderr));
    let css = String::from_utf8(output.stdout).expect("CLI CSS should be UTF-8");
    assert!(css.contains("background-color: #123456;"));
    assert!(css.contains(".hover\\:bg-red-500:hover"));
}

#[test]
fn build_command_uses_swc_for_tsx_helpers() {
    let fixture = format!("{}/tests/fixtures/basic.tsx", env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(env!("CARGO_BIN_EXE_utilitycss-cli"))
        .args(["build", "--minify", &fixture])
        .output()
        .expect("CLI binary should start");

    assert!(output.status.success(), "CLI failed: {}", String::from_utf8_lossy(&output.stderr));
    let css = String::from_utf8(output.stdout).expect("CLI CSS should be UTF-8");
    assert!(css.contains(".flex{display:flex;}"));
    assert!(css.contains(".p-4{padding:1rem;}"));
}
