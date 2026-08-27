//! Conventional HTTP configuration loading contracts.

#![cfg(feature = "http")]

use mads_common::__private::load_standard_config_from_for_test;
use mads_common::core::{EnvSource, MADS020};

fn environment(values: impl IntoIterator<Item = (&'static str, &'static str)>) -> EnvSource {
    EnvSource::from_iter("MADS_", values)
}

#[test]
fn conventional_sources_apply_dotenv_toml_and_environment_precedence() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join(".env"), "APP_PORT=3100\n").unwrap();
    std::fs::write(
        root.path().join("mads.toml"),
        "[server]\nport = \"${APP_PORT}\"\nhost = \"127.0.0.1\"\n",
    )
    .unwrap();

    let config = load_standard_config_from_for_test(
        root.path(),
        environment([("MADS_SERVER__PORT", "3200")]),
    )
    .unwrap();

    assert_eq!(config.get("server.port"), Some("3200"));
    assert_eq!(config.source_of("server.port"), Some("environment"));
    assert_eq!(config.get("server.host"), Some("127.0.0.1"));
    assert_eq!(config.get("APP_PORT"), None);
}

#[test]
fn conventional_sources_allow_both_optional_files_to_be_absent() {
    let root = tempfile::tempdir().unwrap();

    let config = load_standard_config_from_for_test(root.path(), environment([])).unwrap();

    assert!(config.is_empty());
}

#[test]
fn conventional_sources_reject_a_malformed_present_dotenv_file() {
    let root = tempfile::tempdir().unwrap();
    let dotenv = root.path().join(".env");
    let sentinel = "postgres://dotenv-secret";
    std::fs::write(&dotenv, format!("BROKEN='unterminated {sentinel}\n")).unwrap();

    let error = load_standard_config_from_for_test(root.path(), environment([])).unwrap_err();

    assert_eq!(error.code(), MADS020);
    assert!(error.to_string().contains(dotenv.to_str().unwrap()));
    assert!(!error.to_string().contains(sentinel));
}

#[test]
fn conventional_sources_reject_a_malformed_present_toml_file() {
    let root = tempfile::tempdir().unwrap();
    let toml = root.path().join("mads.toml");
    let sentinel = "postgres://toml-secret";
    std::fs::write(&toml, format!("[server\nport = \"{sentinel}\"\n")).unwrap();

    let error = load_standard_config_from_for_test(root.path(), environment([])).unwrap_err();

    assert_eq!(error.code(), MADS020);
    assert!(error.to_string().contains(toml.to_str().unwrap()));
    assert!(!error.to_string().contains(sentinel));
}

#[test]
fn conventional_sources_do_not_search_parent_directories() {
    let parent = tempfile::tempdir().unwrap();
    std::fs::write(
        parent.path().join("mads.toml"),
        "[server]\nport = \"3100\"\n",
    )
    .unwrap();
    let child = parent.path().join("child");
    std::fs::create_dir(&child).unwrap();

    let config = load_standard_config_from_for_test(&child, environment([])).unwrap();

    assert!(config.is_empty());
}
