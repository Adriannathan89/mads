//! Conventional HTTP configuration loading and server decision contracts.

#![cfg(feature = "http")]
#![allow(missing_docs)]

use mads_common::__private::{
    enable_automatic_server_for_test, load_standard_config_from_for_test,
    server_binding_address_for_test,
};
use mads_common::core::{
    AutoConfigurationReport, AutoConfigurationStatus, Config, ConfigBuilder, EnvSource, MADS020,
    Mads, MadsBuilder, MapSource, Module,
};

const SERVER_ID: &str = "mads.common.http.server";

mod routed {
    #[mads_common::routes]
    pub trait RoutedRoutes {
        #[mads_common::get("/health")]
        async fn health(&self) -> &'static str;
    }

    #[mads_common::controller(routes = [RoutedRoutes])]
    pub struct RoutedController;

    impl RoutedRoutes for RoutedController {
        async fn health(&self) -> &'static str {
            "healthy"
        }
    }

    #[mads_common::core::module]
    pub struct RoutedApp;
}

mod empty {
    #[mads_common::core::module]
    pub struct EmptyApp;
}

fn config(values: impl IntoIterator<Item = (&'static str, &'static str)>) -> Config {
    ConfigBuilder::new()
        .source(MapSource::new("test", values))
        .build()
        .unwrap()
}

fn automatic_builder<M: Module>(config: Config) -> MadsBuilder {
    let mut builder = Mads::builder_with_config(config);
    builder.root::<M>().unwrap();
    assert!(enable_automatic_server_for_test(&mut builder));
    builder
}

fn report<'a>(reports: &'a [AutoConfigurationReport]) -> &'a AutoConfigurationReport {
    reports
        .iter()
        .find(|report| report.identifier() == SERVER_ID)
        .expect("the HTTP server auto-configuration must be registered")
}

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

#[tokio::test]
async fn server_automatic_mode_uses_defaults_and_contributes_a_binding() {
    let application = automatic_builder::<routed::RoutedApp>(Config::empty())
        .build()
        .await
        .unwrap();

    assert_eq!(
        server_binding_address_for_test(&application).unwrap(),
        ("127.0.0.1".to_owned(), 3000)
    );
    assert_eq!(
        report(application.auto_configurations()).status(),
        AutoConfigurationStatus::Active,
    );
}

#[tokio::test]
async fn server_automatic_mode_accepts_hostnames_ip_addresses_and_explicit_ports() {
    for (host, port) in [
        ("api.internal", "4101"),
        ("192.0.2.10", "4102"),
        ("2001:db8::10", "4103"),
    ] {
        let application = automatic_builder::<routed::RoutedApp>(config([
            ("server.host", host),
            ("server.port", port),
        ]))
        .build()
        .await
        .unwrap();

        assert_eq!(
            server_binding_address_for_test(&application).unwrap(),
            (host.to_owned(), port.parse().unwrap())
        );
        assert_eq!(
            report(application.auto_configurations()).status(),
            AutoConfigurationStatus::Active,
        );
        assert_eq!(
            report(application.auto_configurations())
                .configuration()
                .iter()
                .map(|evidence| evidence.key())
                .collect::<Vec<_>>(),
            ["server.host", "server.port"]
        );
        assert!(
            report(application.auto_configurations())
                .configuration()
                .iter()
                .all(|evidence| evidence.source() == Some("test"))
        );
    }
}

#[test]
fn server_invalid_automatic_configuration_is_failed_and_redacted() {
    for (key, value, redaction_marker) in [
        ("server.host", "", None),
        ("server.host", " \t ", None),
        ("server.host", "private\nserver", Some("private\nserver")),
        ("server.port", "not-a-port", Some("not-a-port")),
        ("server.port", "0", None),
        ("server.port", "65536", Some("65536")),
    ] {
        let analysis = automatic_builder::<routed::RoutedApp>(config([(key, value)])).analyze();
        let server = report(analysis.auto_configurations());
        let diagnostics = analysis
            .diagnostics()
            .iter()
            .map(ToString::to_string)
            .collect::<String>();

        assert!(!analysis.is_valid());
        assert_eq!(server.status(), AutoConfigurationStatus::Failed);
        assert_eq!(server.reason_code().as_str(), "invalid_configuration");
        assert!(
            analysis
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code() == MADS020)
        );
        assert!(diagnostics.contains(key));
        assert!(diagnostics.contains("test"));
        if let Some(redaction_marker) = redaction_marker {
            assert!(!format!("{server:?}").contains(redaction_marker));
            assert!(!diagnostics.contains(redaction_marker));
        }
    }
}

#[tokio::test]
async fn server_automatic_mode_skips_before_parsing_when_the_root_has_no_routes() {
    let application = automatic_builder::<empty::EmptyApp>(config([
        ("server.host", "private.example"),
        ("server.port", "0"),
    ]))
    .build()
    .await
    .unwrap();

    let server = report(application.auto_configurations());
    assert_eq!(server.status(), AutoConfigurationStatus::Skipped);
    assert_eq!(server.reason_code().as_str(), "no_managed_routes");
    assert!(server_binding_address_for_test(&application).is_err());
}

#[tokio::test]
async fn low_level_builder_overrides_the_server_decision_without_parsing_server_values() {
    let configured_host = "private.example";
    let configured_port = "0";
    let mut builder = Mads::builder_with_config(config([
        ("server.host", configured_host),
        ("server.port", configured_port),
    ]));
    builder.root::<routed::RoutedApp>().unwrap();
    let application = builder.build().await.unwrap();

    let server = report(application.auto_configurations());
    assert_eq!(server.status(), AutoConfigurationStatus::Overridden);
    assert_eq!(server.reason_code().as_str(), "explicit_listener");
    let debug = format!("{server:?}");
    assert!(!debug.contains(configured_host));
    assert!(!debug.contains(configured_port));
}
