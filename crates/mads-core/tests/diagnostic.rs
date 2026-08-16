//! Integration tests for structured MADS diagnostics.

use std::fmt;

use mads_core::{Diagnostic, Error, MADS001, SourceLocation};

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
