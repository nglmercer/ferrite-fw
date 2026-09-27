//! Shared `@apply` fixture conformance for the Rust stylesheet layer.

use std::{fs, path::Path};

use serde::Deserialize;
use utilitycss_compiler::{Compiler, CompilerConfig};
use utilitycss_span::SourceId;
use utilitycss_stylesheet::{transform_stylesheet, StylesheetInput};

#[derive(Debug, Deserialize)]
struct Fixture {
    input: String,
    expected: String,
    diagnostics: Vec<String>,
}

#[test]
fn shared_apply_fixtures_match_the_rust_semantic_layer() {
    let fixture_directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/apply");
    let mut paths = fs::read_dir(fixture_directory)
        .expect("shared apply fixture directory exists")
        .map(|entry| entry.expect("fixture entry is readable").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "json"))
        .collect::<Vec<_>>();
    paths.sort();

    for path in paths {
        let fixture: Fixture =
            serde_json::from_str(&fs::read_to_string(&path).expect("fixture source is readable"))
                .expect("fixture JSON is valid");
        let mut compiler = Compiler::new(CompilerConfig::new());
        let output = transform_stylesheet(
            &mut compiler,
            StylesheetInput::new(SourceId::new(path.to_string_lossy()), fixture.input),
        );
        let diagnostics = output
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.code().to_string())
            .collect::<Vec<_>>();

        assert_eq!(output.css(), fixture.expected, "fixture {} CSS", path.display());
        assert_eq!(diagnostics, fixture.diagnostics, "fixture {} diagnostics", path.display());
    }
}
