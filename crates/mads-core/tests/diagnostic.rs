//! Integration tests for structured MADS diagnostics.

use std::fmt;

use mads_core::{Diagnostic, Error, MADS001, MADS002, MADS003, MADS005, MADS006, SourceLocation};

#[test]
fn graph_diagnostic_codes_are_stable() {
    assert_eq!(MADS002.as_str(), "MADS002");
    assert_eq!(MADS005.as_str(), "MADS005");
    assert_eq!(MADS006.as_str(), "MADS006");
}

#[test]
fn aggregated_errors_preserve_order_and_primary_code() {
    let primary = Diagnostic::new(MADS002, "ambiguous provider", "two clocks exist");
    let missing = Diagnostic::new(MADS003, "unresolved dependency", "database is missing");
    let error = Error::from_diagnostics(primary, [missing]);

    assert_eq!(error.code(), MADS002);
    assert_eq!(error.diagnostics().len(), 2);
    assert_eq!(error.diagnostic(), &error.diagnostics()[0]);

    let rendered = error.to_string();
    assert!(rendered.find("MADS002").unwrap() < rendered.find("MADS003").unwrap());
    assert!(rendered.contains("\n\nerror[MADS003]"));
}

#[test]
fn renders_a_structured_diagnostic() {
    let diagnostic = Diagnostic::new(
        MADS001,
        "duplicate provider",
        "UserService is registered twice",
    )
    .with_subject("UserService")
    .with_location(SourceLocation::new("src/users.rs", 12, 3))
    .with_suggestion("remove one provider declaration");
    let error = Error::new(diagnostic);

    assert_eq!(error.diagnostic().title(), "duplicate provider");
    assert_eq!(
        error.diagnostic().message(),
        "UserService is registered twice"
    );
    assert_eq!(error.diagnostic().subject(), Some("UserService"));
    assert_eq!(
        error.diagnostic().location(),
        Some(SourceLocation::new("src/users.rs", 12, 3))
    );
    assert_eq!(
        error.diagnostic().suggestions(),
        ["remove one provider declaration"]
    );

    assert_eq!(error.code(), MADS001);
    assert!(
        error
            .to_string()
            .contains("error[MADS001]: duplicate provider")
    );
    assert!(error.to_string().contains("src/users.rs:12:3"));
    assert!(
        error
            .to_string()
            .contains("help: remove one provider declaration")
    );
}

#[test]
fn retains_a_source_error() {
    #[derive(Debug)]
    struct UnderlyingError;

    impl fmt::Display for UnderlyingError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("underlying failure")
        }
    }

    impl std::error::Error for UnderlyingError {}

    let diagnostic = Diagnostic::new(MADS001, "duplicate provider", "registration failed");
    let error = Error::with_source(diagnostic, UnderlyingError);

    assert!(std::error::Error::source(&error).is_some());
}

#[test]
fn renders_multiple_suggestions_in_insertion_order() {
    let diagnostic = Diagnostic::new(
        MADS001,
        "duplicate provider",
        "UserService is registered twice",
    )
    .with_suggestion("remove one provider declaration")
    .with_suggestion("rename the duplicate provider");
    let rendered = Error::new(diagnostic).to_string();

    let first = rendered
        .find("help: remove one provider declaration")
        .expect("first suggestion should render");
    let second = rendered
        .find("help: rename the duplicate provider")
        .expect("second suggestion should render");
    assert!(first < second);
}
