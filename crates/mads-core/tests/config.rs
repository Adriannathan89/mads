//! Integration tests for deterministic configuration merging.

use std::collections::BTreeMap;
#[cfg(unix)]
use std::ffi::OsString;
use std::fs;
#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;

use mads_core::{
    ConfigBuilder, ConfigDocument, ConfigSource, Diagnostic, DotenvSource, EnvSource, Error,
    MADS020, MapSource, Result, TomlSource,
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

#[test]
fn map_source_inserts_string_arrays_without_changing_scalar_access() {
    let config = ConfigBuilder::new()
        .source(
            MapSource::new("base", [("passport.issuer", "issuer")])
                .with_string_array("passport.algorithms", ["HS256", "RS256"]),
        )
        .build()
        .unwrap();

    assert_eq!(config.get("passport.issuer"), Some("issuer"));
    assert_eq!(
        config.get_string_array("passport.algorithms"),
        Some(["HS256".to_owned(), "RS256".to_owned()].as_slice()),
    );
    assert_eq!(
        config.source_of_string_array("passport.algorithms"),
        Some("base")
    );
    assert_eq!(config.get("passport.algorithms"), None);
}

#[test]
fn later_source_replaces_an_entry_across_value_shapes() {
    let array_wins = ConfigBuilder::new()
        .source(MapSource::new("first", [("value", "scalar")]))
        .source(
            MapSource::new("second", std::iter::empty::<(&str, &str)>())
                .with_string_array("value", ["array"]),
        )
        .build()
        .unwrap();
    assert_eq!(array_wins.get("value"), None);
    assert_eq!(array_wins.get_string_array("value").unwrap(), ["array"]);

    let scalar_wins = ConfigBuilder::new()
        .source(
            MapSource::new("first", std::iter::empty::<(&str, &str)>())
                .with_string_array("value", ["array"]),
        )
        .source(MapSource::new("second", [("value", "scalar")]))
        .build()
        .unwrap();
    assert_eq!(scalar_wins.get("value"), Some("scalar"));
    assert_eq!(scalar_wins.get_string_array("value"), None);
    assert_eq!(scalar_wins.source_of("value"), Some("second"));
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

#[test]
fn toml_source_flattens_scalar_tables_and_allows_environment_override() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("mads.toml");
    fs::write(
        &path,
        r#"
[database]
url = "postgres://file-value"
pool_size = 10
migrate = false
ratio = 1.5
"#,
    )
    .unwrap();

    let config = ConfigBuilder::new()
        .source(TomlSource::file(&path))
        .source(EnvSource::from_iter(
            "MADS_",
            [("MADS_DATABASE__POOL_SIZE", "12")],
        ))
        .build()
        .unwrap();

    assert_eq!(config.get("database.url"), Some("postgres://file-value"));
    assert_eq!(config.get("database.pool_size"), Some("12"));
    assert_eq!(config.get("database.migrate"), Some("false"));
    assert_eq!(config.get("database.ratio"), Some("1.5"));
    assert_eq!(config.source_of("database.url"), path.to_str());
    assert_eq!(config.source_of("database.pool_size"), Some("environment"));
    assert_eq!(
        config.iter().map(|(key, _)| key).collect::<Vec<_>>(),
        [
            "database.migrate",
            "database.pool_size",
            "database.ratio",
            "database.url"
        ]
    );
}

#[test]
fn toml_string_arrays_interpolate_each_exact_element() {
    let directory = tempfile::tempdir().unwrap();
    let dotenv = directory.path().join(".env");
    let toml = directory.path().join("mads.toml");
    fs::write(&dotenv, "PRIMARY=HS256\nSECONDARY=RS256\n").unwrap();
    fs::write(
        &toml,
        "[passport]\nalgorithms = [\"${PRIMARY}\", \"${SECONDARY}\", \"literal-${PRIMARY}\"]\n",
    )
    .unwrap();

    let config = ConfigBuilder::new()
        .dotenv(DotenvSource::required(dotenv))
        .source(TomlSource::file(toml))
        .build()
        .unwrap();

    assert_eq!(
        config.get_string_array("passport.algorithms").unwrap(),
        ["HS256", "RS256", "literal-${PRIMARY}"],
    );
}

#[test]
fn toml_rejects_every_non_string_array_without_leaking_values() {
    for value in [
        "[1, 2]",
        "[true, false]",
        "[1.0]",
        "[1979-05-27T07:32:00Z]",
        "[\"ok\", 7]",
        "[[\"nested-secret\"]]",
        "[{ token = \"table-secret\" }]",
    ] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mads.toml");
        fs::write(&path, format!("[passport]\nalgorithms = {value}\n")).unwrap();
        let error = ConfigBuilder::new()
            .source(TomlSource::file(path))
            .build()
            .unwrap_err();
        let report = error.to_string();
        assert_eq!(error.code(), MADS020);
        assert!(report.contains("passport.algorithms"));
        assert!(report.contains("non-string array"));
        assert!(!report.contains("nested-secret"));
        assert!(!report.contains("table-secret"));
    }
}

#[test]
fn missing_array_variable_names_only_the_key_and_variable() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("mads.toml");
    fs::write(
        &path,
        "[passport]\nalgorithms = [\"sibling-secret\", \"${MISSING_ALGORITHM}\"]\n",
    )
    .unwrap();

    let error = ConfigBuilder::new()
        .source(TomlSource::file(path))
        .build()
        .unwrap_err();
    let report = error.to_string();

    assert_eq!(error.code(), MADS020);
    assert!(report.contains("passport.algorithms"));
    assert!(report.contains("MISSING_ALGORITHM"));
    assert!(!report.contains("${MISSING_ALGORITHM}"));
    assert!(!report.contains("sibling-secret"));
}

#[test]
fn legacy_toml_load_remains_scalar_only() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("mads.toml");
    fs::write(&path, "[passport]\nalgorithms = [\"HS256\"]\n").unwrap();

    let error = TomlSource::file(path).load().unwrap_err();

    assert_eq!(error.code(), MADS020);
    assert!(error.to_string().contains("passport.algorithms"));
    assert!(error.to_string().contains("array"));
    assert!(!error.to_string().contains("HS256"));
}

#[test]
fn missing_toml_reports_only_its_path() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("missing.toml");
    let error = ConfigBuilder::new()
        .source(TomlSource::file(&path))
        .build()
        .unwrap_err();
    let report = error.to_string();

    assert_eq!(error.code(), MADS020);
    assert!(report.contains(path.to_str().unwrap()));
    assert!(report.contains("configuration file could not be read"));
}

#[test]
fn malformed_toml_is_redacted_and_has_no_parser_source() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("mads.toml");
    let sentinel = "postgres://malformed-secret";
    fs::write(&path, format!("[database\nurl = \"{sentinel}\"\n")).unwrap();
    let error = ConfigBuilder::new()
        .source(TomlSource::file(&path))
        .build()
        .unwrap_err();

    assert_eq!(error.code(), MADS020);
    assert!(error.to_string().contains(path.to_str().unwrap()));
    assert!(!error.to_string().contains(sentinel));
    assert!(std::error::Error::source(&error).is_none());
}

#[test]
fn unsupported_toml_values_name_the_key_without_exposing_secrets() {
    for (value, kind) in [
        ("[5432]", "array"),
        ("1979-05-27T07:32:00Z", "datetime"),
        ("{ url = \"postgres://inline-secret\" }", "inline table"),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mads.toml");
        fs::write(&path, format!("[database]\nports = {value}\n")).unwrap();
        let error = ConfigBuilder::new()
            .source(TomlSource::file(&path))
            .build()
            .unwrap_err();
        let report = error.to_string();

        assert_eq!(error.code(), MADS020, "{kind}");
        assert!(report.contains("database.ports"), "{kind}");
        assert!(report.contains("unsupported configuration value"), "{kind}");
        assert!(!report.contains("postgres://inline-secret"), "{kind}");
    }
}

#[test]
fn toml_inline_table_detection_respects_quoted_keys_and_multiline_strings() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("mads.toml");
    fs::write(
        &path,
        r#"
[database]
"url=primary" = "postgres://valid"
description = '''
this scalar contains = { inline-looking text }
'''
"#,
    )
    .unwrap();

    let config = ConfigBuilder::new()
        .source(TomlSource::file(path))
        .build()
        .unwrap();

    assert_eq!(config.get("database.url=primary"), Some("postgres://valid"));
    assert_eq!(
        config.get("database.description"),
        Some("this scalar contains = { inline-looking text }\n")
    );
}

#[test]
fn inline_table_after_commented_header_reports_complete_flattened_subject() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("mads.toml");
    fs::write(
        &path,
        "[database.connection] # production connection\nsettings = { mode = \"secret\" }\n",
    )
    .unwrap();

    let error = ConfigBuilder::new()
        .source(TomlSource::file(path))
        .build()
        .unwrap_err();
    let report = error.to_string();

    assert_eq!(error.code(), MADS020);
    assert!(report.contains("database.connection.settings"));
    assert!(!report.contains("secret"));
}

#[test]
fn missing_optional_dotenv_is_ignored() {
    let directory = tempfile::tempdir().unwrap();
    let config = ConfigBuilder::new()
        .dotenv(DotenvSource::optional(directory.path().join(".env")))
        .source(MapSource::new("test", [("server.port", "3000")]))
        .build()
        .unwrap();

    assert_eq!(config.get("server.port"), Some("3000"));
}

#[test]
fn required_dotenv_and_present_directory_fail_safely() {
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("missing.env");
    let error = ConfigBuilder::new()
        .dotenv(DotenvSource::required(&missing))
        .build()
        .unwrap_err();
    assert_eq!(error.code(), MADS020);
    assert!(error.to_string().contains(missing.to_str().unwrap()));

    let error = ConfigBuilder::new()
        .dotenv(DotenvSource::required(directory.path()))
        .build()
        .unwrap_err();
    assert_eq!(error.code(), MADS020);
    assert!(
        error
            .to_string()
            .contains(directory.path().to_str().unwrap())
    );
}

#[test]
fn malformed_dotenv_does_not_expose_the_line_or_parser_source() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join(".env");
    let sentinel = "postgres://dotenv-secret";
    fs::write(&path, format!("BROKEN='unterminated {sentinel}\n")).unwrap();
    let error = ConfigBuilder::new()
        .dotenv(DotenvSource::required(&path))
        .build()
        .unwrap_err();

    assert_eq!(error.code(), MADS020);
    assert!(error.to_string().contains(path.to_str().unwrap()));
    assert!(!error.to_string().contains(sentinel));
    assert!(std::error::Error::source(&error).is_none());
}

#[test]
fn later_dotenv_wins_and_variables_do_not_become_configuration() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("first.env");
    let second = directory.path().join("second.env");
    fs::write(&first, "DATABASE_URL=postgres://first\n").unwrap();
    fs::write(&second, "DATABASE_URL=postgres://second\n").unwrap();

    let config = ConfigBuilder::new()
        .dotenv(DotenvSource::required(first))
        .dotenv(DotenvSource::required(second))
        .source(MapSource::new(
            "test",
            [("database.url", "${DATABASE_URL}")],
        ))
        .build()
        .unwrap();

    assert_eq!(config.get("database.url"), Some("postgres://second"));
    assert_eq!(config.get("DATABASE_URL"), None);
    assert_eq!(config.source_of("database.url"), Some("test"));
}

#[test]
fn missing_exact_variable_names_key_and_variable_without_config_value() {
    let sentinel = "postgres://must-not-leak";
    let error = ConfigBuilder::new()
        .source(MapSource::new(
            "test",
            [
                ("database.url", "${MISSING_DATABASE_URL}"),
                ("secret", sentinel),
            ],
        ))
        .build()
        .unwrap_err();
    let report = error.to_string();

    assert_eq!(error.code(), MADS020);
    assert!(report.contains("database.url"));
    assert!(report.contains("MISSING_DATABASE_URL"));
    assert!(!report.contains(sentinel));
    assert!(!report.contains("${MISSING_DATABASE_URL}"));
}

#[test]
fn interpolation_is_exact_and_non_recursive() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join(".env");
    fs::write(&path, "NAME=value\nNESTED='${SECOND}'\nSECOND=resolved\n").unwrap();
    let config = ConfigBuilder::new()
        .dotenv(DotenvSource::required(path))
        .source(MapSource::new(
            "test",
            [
                ("embedded", "prefix-${NAME}"),
                ("fallback", "${NAME:-fallback}"),
                ("nested", "${NESTED}"),
            ],
        ))
        .build()
        .unwrap();

    assert_eq!(config.get("embedded"), Some("prefix-${NAME}"));
    assert_eq!(config.get("fallback"), Some("${NAME:-fallback}"));
    assert_eq!(config.get("nested"), Some("${SECOND}"));
}

#[test]
fn debug_redacts_all_configuration_values() {
    let sentinel = "sentinel-database-secret";
    let map = MapSource::new("defaults", [("database.url", sentinel)]);
    let environment = EnvSource::from_iter("MADS_", [("MADS_DATABASE__URL", sentinel)]);
    let config = ConfigBuilder::new()
        .source(map.clone())
        .source(environment.clone())
        .build()
        .unwrap();
    let value = config.iter().next().unwrap().1;

    for rendered in [
        format!("{map:?}"),
        format!("{environment:?}"),
        format!("{config:?}"),
        format!("{value:?}"),
    ] {
        assert!(!rendered.contains(sentinel));
        assert!(rendered.contains("[REDACTED]"));
    }
}

#[test]
fn debug_redacts_document_and_resolved_string_array_values() {
    let sentinel = "array-secret-sentinel";
    let mut document = ConfigDocument::new();
    document.insert_string_array("passport.keys", [sentinel]);
    let config = ConfigBuilder::new()
        .source(
            MapSource::new("passport", std::iter::empty::<(&str, &str)>())
                .with_string_array("passport.keys", [sentinel]),
        )
        .build()
        .unwrap();

    for rendered in [format!("{document:?}"), format!("{config:?}")] {
        assert!(!rendered.contains(sentinel));
    }
}
