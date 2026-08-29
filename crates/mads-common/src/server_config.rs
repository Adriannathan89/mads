//! Conventional configuration loading for the HTTP runtime.

use std::any::TypeId;
use std::fmt;
use std::net::IpAddr;
use std::path::Path;

use mads_core::__private::{
    AutoConfigurationApplyContext, AutoConfigurationContext, AutoConfigurationContribution,
    AutoConfigurationDescriptor, AutoConfigurationEvaluation,
};
use mads_core::{
    AutoConfigurationConfigEvidence, AutoConfigurationReasonCode, Config, ConfigBuilder,
    Diagnostic, DotenvSource, EnvSource, Error, MADS020, MadsBuilder, Result, SourceLocation,
    TomlSource,
};

use crate::http_scope::HttpApplicationScope;

pub(crate) const SERVER_AUTO_CONFIGURATION_ID: &str = "mads.common.http.server";

const SERVER_BINDING_TYPE_NAME: &str = "mads_common::server_config::ServerBinding";

#[derive(Clone, Copy)]
pub(crate) enum HttpRuntimeMode {
    Automatic,
}

#[derive(Clone)]
pub(crate) struct ServerBinding {
    host: String,
    port: u16,
}

impl ServerBinding {
    pub(crate) fn from_config(config: &Config) -> Result<Self> {
        let host = config.get("server.host").unwrap_or("127.0.0.1");
        let host = parse_host(host, config.source_of("server.host"))?;
        let port = config.get("server.port").unwrap_or("3000");
        let port = parse_port(port, config.source_of("server.port"))?;

        Ok(Self { host, port })
    }

    pub(crate) fn host(&self) -> &str {
        &self.host
    }

    pub(crate) const fn port(&self) -> u16 {
        self.port
    }

    pub(crate) fn address(&self) -> (&str, u16) {
        (self.host(), self.port())
    }
}

impl fmt::Debug for ServerBinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

pub(crate) fn enable_automatic_server(builder: &mut MadsBuilder) -> bool {
    builder.__auto_configuration_input(SERVER_AUTO_CONFIGURATION_ID, HttpRuntimeMode::Automatic)
}

#[allow(dead_code)]
pub(crate) fn load_standard_config_from(root: &Path) -> Result<Config> {
    load_standard_config_from_with_environment(root, EnvSource::new("MADS_"))
}

pub(crate) fn load_standard_config_from_with_environment(
    root: &Path,
    environment: EnvSource,
) -> Result<Config> {
    ConfigBuilder::new()
        .dotenv(DotenvSource::optional(root.join(".env")))
        .source(TomlSource::optional(root.join("mads.toml")))
        .source(environment)
        .build()
}

fn server_binding_type_id() -> TypeId {
    TypeId::of::<ServerBinding>()
}

fn evaluate(context: &AutoConfigurationContext<'_>) -> AutoConfigurationEvaluation {
    if context.input::<HttpRuntimeMode>().is_none() {
        return AutoConfigurationEvaluation::overridden(
            AutoConfigurationReasonCode::new("explicit_listener"),
            "the low-level runtime owns its listener address",
            Vec::new(),
            server_evidence(context),
        );
    }

    let Some(module_graph) = context.module_graph() else {
        return AutoConfigurationEvaluation::skipped(
            AutoConfigurationReasonCode::new("no_managed_routes"),
            "the selected application has no managed HTTP routes",
            Vec::new(),
            server_evidence(context),
        );
    };

    let http = match HttpApplicationScope::for_module_graph(Some(module_graph)) {
        Ok(http) => http,
        Err(error) => {
            return AutoConfigurationEvaluation::failed(
                AutoConfigurationReasonCode::new("http_scope_failed"),
                "the selected application HTTP scope could not be analyzed",
                Vec::new(),
                server_evidence(context),
                error,
            );
        }
    };
    if !http.has_routes() {
        return AutoConfigurationEvaluation::skipped(
            AutoConfigurationReasonCode::new("no_managed_routes"),
            "the selected application has no managed HTTP routes",
            Vec::new(),
            server_evidence(context),
        );
    }

    match ServerBinding::from_config(context.config()) {
        Ok(_) => AutoConfigurationEvaluation::active(
            AutoConfigurationReasonCode::new("conditions_matched"),
            "automatic HTTP server conditions matched",
            Vec::new(),
            server_evidence(context),
        ),
        Err(error) => AutoConfigurationEvaluation::failed(
            AutoConfigurationReasonCode::new("invalid_configuration"),
            "automatic HTTP server configuration is invalid",
            Vec::new(),
            server_evidence(context),
            error,
        ),
    }
}

fn apply(context: &AutoConfigurationApplyContext<'_>) -> Result<AutoConfigurationContribution> {
    Ok(AutoConfigurationContribution::new(
        ServerBinding::from_config(context.config())?,
    ))
}

fn parse_host(host: &str, source: Option<&str>) -> Result<String> {
    if host.trim().is_empty() {
        return Err(invalid_server_configuration(
            "server.host",
            source,
            "must not be empty or whitespace-only",
        ));
    }
    if host.chars().any(|character| character.is_ascii_control()) {
        return Err(invalid_server_configuration(
            "server.host",
            source,
            "must not contain ASCII control characters",
        ));
    }

    if let Ok(address) = host.parse::<IpAddr>() {
        return Ok(address.to_string());
    }
    if !is_valid_hostname(host) {
        return Err(invalid_server_configuration(
            "server.host",
            source,
            "must be an IPv4 address, IPv6 address, or valid hostname",
        ));
    }

    Ok(host.to_owned())
}

pub(crate) fn is_valid_hostname(host: &str) -> bool {
    let hostname = host.strip_suffix('.').unwrap_or(host);
    !hostname.is_empty()
        && hostname.len() <= 253
        && hostname.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|character| character.is_ascii_alphanumeric() || character == b'-')
        })
}

fn parse_port(port: &str, source: Option<&str>) -> Result<u16> {
    let Ok(port) = port.parse::<u16>() else {
        return Err(invalid_server_configuration(
            "server.port",
            source,
            "must be an integer in the range 1..=65535",
        ));
    };
    if port == 0 {
        return Err(invalid_server_configuration(
            "server.port",
            source,
            "must be an integer in the range 1..=65535",
        ));
    }

    Ok(port)
}

fn invalid_server_configuration(key: &'static str, source: Option<&str>, rule: &str) -> Error {
    Error::new(
        Diagnostic::new(
            MADS020,
            "automatic HTTP server configuration is invalid",
            format!("{key} {rule}; source: {}", safe_source_label(source)),
        )
        .with_subject(key)
        .with_suggestion("configure a valid automatic HTTP server address"),
    )
}

fn safe_source_label(source: Option<&str>) -> &'static str {
    match source {
        None => "defaults",
        Some("defaults") => "defaults",
        Some("environment") => "environment",
        Some("mads.toml") => "mads.toml",
        Some("test") => "test",
        Some(source)
            if Path::new(source)
                .file_name()
                .is_some_and(|name| name == "mads.toml") =>
        {
            "mads.toml"
        }
        Some(_) => "[REDACTED]",
    }
}

fn server_evidence(context: &AutoConfigurationContext<'_>) -> Vec<AutoConfigurationConfigEvidence> {
    ["server.host", "server.port"]
        .into_iter()
        .map(|key| AutoConfigurationConfigEvidence::new(key, context.config().source_of(key)))
        .collect()
}

mads_core::__private::inventory::submit! {
    AutoConfigurationDescriptor::new(
        SERVER_AUTO_CONFIGURATION_ID,
        SERVER_BINDING_TYPE_NAME,
        server_binding_type_id,
        SourceLocation::new(file!(), line!(), column!()),
        evaluate,
        apply,
    )
}
