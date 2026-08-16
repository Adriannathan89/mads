//! Integration tests for deterministic configuration merging.

use std::collections::BTreeMap;
#[cfg(unix)]
use std::ffi::OsString;
#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;

use mads_core::{
    ConfigBuilder, ConfigSource, Diagnostic, EnvSource, Error, MADS020, MapSource, Result,
};

#[test]
fn later_sources_override_values_and_retain_attribution() {
    let defaults = MapSource::new("defaults", [("server.port", "3000")]);
    let environment = EnvSource::from_iter(
        "MADS_",
        [("MADS_SERVER__PORT", "8080"), ("IGNORED", "value")],
    );

    let config = ConfigBuilder::new()
        .source(defaults)
        .source(environment)
        .build()
        .expect("configuration should build");

    assert_eq!(config.get("server.port"), Some("8080"));
    assert_eq!(config.source_of("server.port"), Some("environment"));
    assert_eq!(config.get("ignored"), None);
}

struct BrokenSource;

impl ConfigSource for BrokenSource {
    fn name(&self) -> &str {
        "broken"
    }

    fn load(&self) -> Result<BTreeMap<String, String>> {
        Err(Error::new(Diagnostic::new(
            MADS020,
            "configuration source failed",
            "broken source could not load",
        )))
    }
}

#[test]
fn source_failures_preserve_their_diagnostic_code() {
    let error = ConfigBuilder::new()
        .source(BrokenSource)
        .build()
        .expect_err("broken source should fail to build");

    assert_eq!(error.code(), MADS020);
}

#[test]
fn iter_returns_keys_in_lexical_order() {
    let config = ConfigBuilder::new()
        .source(MapSource::new("first", [("zeta", "1"), ("alpha", "2")]))
        .source(MapSource::new("second", [("middle", "3")]))
        .build()
        .expect("configuration should build");
    let keys: Vec<_> = config.iter().map(|(key, _)| key).collect();

    assert_eq!(keys, ["alpha", "middle", "zeta"]);
}

#[test]
fn normalized_environment_key_collisions_use_the_later_variable() {
    let config = ConfigBuilder::new()
        .source(EnvSource::from_iter(
            "MADS_",
            [("MADS_SERVER__PORT", "3000"), ("MADS_server__port", "8080")],
        ))
        .build()
        .expect("configuration should build");

    assert_eq!(config.get("server.port"), Some("8080"));
    assert_eq!(config.source_of("server.port"), Some("environment"));
}

#[cfg(unix)]
#[test]
fn environment_source_ignores_non_unicode_names_and_values() {
    let invalid = OsString::from_vec(vec![0xFF]);
    let config = ConfigBuilder::new()
        .source(EnvSource::from_iter(
            "MADS_",
            [
                (OsString::from("MADS_VALID"), OsString::from("value")),
                (invalid.clone(), OsString::from("ignored")),
                (OsString::from("MADS_INVALID_VALUE"), invalid),
            ],
        ))
        .build()
        .expect("configuration should ignore non-Unicode variables");

    assert_eq!(config.get("valid"), Some("value"));
    assert_eq!(config.len(), 1);
}
