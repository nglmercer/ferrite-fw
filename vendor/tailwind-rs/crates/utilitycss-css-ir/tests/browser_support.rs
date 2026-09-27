#![allow(missing_docs)]

use serde::Deserialize;
use utilitycss_css_ir::{
    BrowserTarget, CssDeclaration, CssDocument, CssFeature, CssRule, OrderKey,
};

#[derive(Debug, Deserialize)]
struct Fixture {
    target: String,
    declarations: Vec<FixtureDeclaration>,
    expected_required: Vec<String>,
    expected_unsupported: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct FixtureDeclaration {
    property: String,
    value: String,
}

fn feature_names(features: &[CssFeature]) -> Vec<String> {
    features.iter().map(|feature| feature.as_str().to_owned()).collect()
}

#[test]
fn browser_target_support_fixtures() {
    let fixtures: Vec<Fixture> =
        serde_json::from_str(include_str!("fixtures/browser_targets.json"))
            .expect("fixture is valid JSON");
    assert!(!fixtures.is_empty(), "fixture file must define cases");

    for fixture in fixtures {
        let target = BrowserTarget::parse(&fixture.target).expect("fixture target parses");
        let declarations = fixture
            .declarations
            .iter()
            .map(|declaration| CssDeclaration::new(&declaration.property, &declaration.value))
            .collect();
        let mut document = CssDocument::new();
        document.push(CssRule::style(OrderKey::default(), ".fixture", declarations));

        let report = document.browser_support(target);
        assert_eq!(
            feature_names(&report.required),
            fixture.expected_required,
            "required features for {}",
            fixture.target
        );
        assert_eq!(
            feature_names(&report.unsupported),
            fixture.expected_unsupported,
            "unsupported features for {}",
            fixture.target
        );
        assert_eq!(
            report.is_supported(),
            fixture.expected_unsupported.is_empty(),
            "support flag for {}",
            fixture.target
        );
    }
}
