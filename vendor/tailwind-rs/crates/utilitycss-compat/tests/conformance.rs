#![allow(missing_docs)]

use utilitycss_compat::{parse_fixtures_json, run_fixtures};
use utilitycss_compiler::{Compiler, CompilerConfig};

#[test]
fn native_fixture_file_matches_the_compiler() {
    let fixtures =
        parse_fixtures_json(include_str!("fixtures/native.json")).expect("fixture JSON is valid");
    let compiler = Compiler::new(CompilerConfig::new());
    let report = run_fixtures(&compiler, &fixtures);

    assert_eq!(report.failed, 0, "compatibility fixtures failed: {report:?}");
}
