//! Strict, application-wide CORS configuration.

use std::any::TypeId;
use std::fmt;
use std::time::Duration;

use axum::http::{HeaderName, HeaderValue, Method, Uri};
use mads_core::__private::{
    AutoConfigurationApplyContext, AutoConfigurationContext, AutoConfigurationContribution,
    AutoConfigurationDescriptor, AutoConfigurationEvaluation,
};
use mads_core::{
    AutoConfigurationConfigEvidence, AutoConfigurationReasonCode, Config, Diagnostic, Error,
    MADS020, MadsBuilder, Result, SourceLocation,
};
use tower_http::cors::{Any, CorsLayer};

use crate::http_scope::HttpApplicationScope;
use crate::server_config::HttpRuntimeMode;

pub(crate) const CORS_AUTO_CONFIGURATION_ID: &str = "mads.common.http.cors";

const CORS_PLAN_TYPE_NAME: &str = "mads_common::cors::CorsPlan";
const CORS_KEYS: [&str; 6] = [
    "server.cors.origins",
    "server.cors.methods",
    "server.cors.allowed_headers",
    "server.cors.exposed_headers",
    "server.cors.credentials",
    "server.cors.max_age_seconds",
];

#[derive(Clone)]
enum CorsValues<T> {
    Any,
    List(Vec<T>),
}

impl<T> CorsValues<T> {
    fn count(&self) -> usize {
        match self {
            Self::Any => 0,
            Self::List(values) => values.len(),
        }
    }

    fn is_any(&self) -> bool {
        matches!(self, Self::Any)
    }
}

#[derive(Clone)]
pub(crate) struct CorsPlan {
    origins: CorsValues<HeaderValue>,
    methods: Vec<Method>,
    allowed_headers: CorsValues<HeaderName>,
    exposed_headers: CorsValues<HeaderName>,
    credentials: bool,
    max_age: Option<Duration>,
}

impl CorsPlan {
    fn from_config(config: &Config) -> Result<Self> {
        let origins = parse_origins(config)?;
        let methods = parse_methods(config)?;
        let allowed_headers = parse_headers(config, "server.cors.allowed_headers")?;
        let exposed_headers = parse_headers(config, "server.cors.exposed_headers")?;
        let credentials = parse_credentials(config)?;
        let max_age = parse_max_age(config)?;

        if credentials && (origins.is_any() || allowed_headers.is_any() || exposed_headers.is_any())
        {
            return Err(invalid_cors_configuration(
                "server.cors.credentials",
                config.source_of("server.cors.credentials"),
                "must not enable credentials with wildcard origins or headers",
            ));
        }

        Ok(Self {
            origins,
            methods,
            allowed_headers,
            exposed_headers,
            credentials,
            max_age,
        })
    }

    #[allow(dead_code)]
    pub(crate) fn layer(&self) -> CorsLayer {
        let mut layer = CorsLayer::new()
            .allow_methods(self.methods.clone())
            .allow_credentials(self.credentials);

        layer = match &self.origins {
            CorsValues::Any => layer.allow_origin(Any),
            CorsValues::List(values) => layer.allow_origin(values.clone()),
        };
        layer = match &self.allowed_headers {
            CorsValues::Any => layer.allow_headers(Any),
            CorsValues::List(values) => layer.allow_headers(values.clone()),
        };
        layer = match &self.exposed_headers {
            CorsValues::Any => layer.expose_headers(Any),
            CorsValues::List(values) => layer.expose_headers(values.clone()),
        };
        if let Some(max_age) = self.max_age {
            layer = layer.max_age(max_age);
        }
        layer
    }
}

impl fmt::Debug for CorsPlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CorsPlan")
            .field("origin_count", &self.origins.count())
            .field("method_count", &self.methods.len())
            .field("allowed_header_count", &self.allowed_headers.count())
            .field("exposed_header_count", &self.exposed_headers.count())
            .field("credentials", &self.credentials)
            .field("has_max_age", &self.max_age.is_some())
            .field("values", &"[REDACTED]")
            .finish()
    }
}

pub(crate) fn enable_automatic_cors(builder: &mut MadsBuilder) -> bool {
    builder.__auto_configuration_input(CORS_AUTO_CONFIGURATION_ID, HttpRuntimeMode::Automatic)
}

fn cors_plan_type_id() -> TypeId {
    TypeId::of::<CorsPlan>()
}

fn evaluate(context: &AutoConfigurationContext<'_>) -> AutoConfigurationEvaluation {
    if !cors_is_present(context.config()) {
        return AutoConfigurationEvaluation::skipped(
            AutoConfigurationReasonCode::new("configuration_absent"),
            "CORS configuration is absent",
            Vec::new(),
            cors_evidence(context),
        );
    }

    if context.input::<HttpRuntimeMode>().is_some() {
        let Some(module_graph) = context.module_graph() else {
            return no_managed_routes(context);
        };
        let http = match HttpApplicationScope::for_module_graph(Some(module_graph)) {
            Ok(http) => http,
            Err(error) => {
                return AutoConfigurationEvaluation::failed(
                    AutoConfigurationReasonCode::new("http_scope_failed"),
                    "the selected application HTTP scope could not be analyzed",
                    Vec::new(),
                    cors_evidence(context),
                    error,
                );
            }
        };
        if !http.has_routes() {
            return no_managed_routes(context);
        }
    }

    match CorsPlan::from_config(context.config()) {
        Ok(_) => AutoConfigurationEvaluation::active(
            AutoConfigurationReasonCode::new("conditions_matched"),
            "CORS configuration conditions matched",
            Vec::new(),
            cors_evidence(context),
        ),
        Err(error) => AutoConfigurationEvaluation::failed(
            AutoConfigurationReasonCode::new("invalid_configuration"),
            "CORS configuration is invalid",
            Vec::new(),
            cors_evidence(context),
            error,
        ),
    }
}

fn no_managed_routes(context: &AutoConfigurationContext<'_>) -> AutoConfigurationEvaluation {
    AutoConfigurationEvaluation::skipped(
        AutoConfigurationReasonCode::new("no_managed_routes"),
        "the selected application has no managed HTTP routes",
        Vec::new(),
        cors_evidence(context),
    )
}

fn apply(context: &AutoConfigurationApplyContext<'_>) -> Result<AutoConfigurationContribution> {
    Ok(AutoConfigurationContribution::new(CorsPlan::from_config(
        context.config(),
    )?))
}

fn cors_is_present(config: &Config) -> bool {
    config.contains_table("server.cors")
        || config
            .iter()
            .any(|(key, _)| key.starts_with("server.cors."))
        || config
            .iter_string_arrays()
            .any(|(key, _)| key.starts_with("server.cors."))
}

fn parse_origins(config: &Config) -> Result<CorsValues<HeaderValue>> {
    parse_wildcard_or_list(config, "server.cors.origins", true, parse_origin)
}

fn parse_origin(
    raw: &str,
    key: &'static str,
    source: Option<&str>,
    position: usize,
) -> Result<HeaderValue> {
    let uri = raw.parse::<Uri>().map_err(|_| {
        invalid_cors_array_element(key, source, position, "must be a canonical HTTP origin")
    })?;
    let has_explicit_path_or_query = uri
        .path_and_query()
        .is_some_and(|path_and_query| path_and_query.as_str() != "/" || raw.ends_with('/'));
    if !matches!(uri.scheme_str(), Some("http" | "https"))
        || uri.authority().is_none()
        || has_explicit_path_or_query
        || raw.contains('#')
        || raw.contains('@')
        || raw.eq_ignore_ascii_case("null")
    {
        return Err(invalid_cors_array_element(
            key,
            source,
            position,
            "must be a canonical HTTP or HTTPS origin without paths, queries, fragments, or user information",
        ));
    }
    HeaderValue::from_str(raw).map_err(|_| {
        invalid_cors_array_element(key, source, position, "must be a valid HTTP header value")
    })
}

fn parse_methods(config: &Config) -> Result<Vec<Method>> {
    let key = "server.cors.methods";
    let Some(raw_methods) = config.get_string_array(key) else {
        return Err(invalid_cors_configuration(
            key,
            config.source_of(key),
            "must be a nonempty string array",
        ));
    };
    if raw_methods.is_empty() {
        return Err(invalid_cors_configuration(
            key,
            source_for_key(config, key),
            "must be a nonempty string array",
        ));
    }

    let source = source_for_key(config, key);
    let mut methods = Vec::new();
    for (index, raw) in raw_methods.iter().enumerate() {
        let position = index + 1;
        if raw == "*" {
            return Err(invalid_cors_array_element(
                key,
                source,
                position,
                "must not contain a wildcard method",
            ));
        }
        let normalized = raw.to_ascii_uppercase();
        let method = Method::from_bytes(normalized.as_bytes()).map_err(|_| {
            invalid_cors_array_element(key, source, position, "must contain valid HTTP methods")
        })?;
        if !methods.contains(&method) {
            methods.push(method);
        }
    }
    Ok(methods)
}

fn parse_headers(config: &Config, key: &'static str) -> Result<CorsValues<HeaderName>> {
    parse_wildcard_or_list(config, key, false, |raw, key, source, position| {
        HeaderName::from_bytes(raw.as_bytes()).map_err(|_| {
            invalid_cors_array_element(
                key,
                source,
                position,
                "must contain valid HTTP header names",
            )
        })
    })
}

fn parse_wildcard_or_list<T>(
    config: &Config,
    key: &'static str,
    required: bool,
    parse: impl Fn(&str, &'static str, Option<&str>, usize) -> Result<T>,
) -> Result<CorsValues<T>>
where
    T: PartialEq,
{
    if let Some(value) = config.get(key) {
        if value == "*" {
            return Ok(CorsValues::Any);
        }
        return Err(invalid_cors_configuration(
            key,
            source_for_key(config, key),
            "must be the scalar wildcard \"*\" or a string array",
        ));
    }

    let Some(raw_values) = config.get_string_array(key) else {
        if required {
            return Err(invalid_cors_configuration(
                key,
                source_for_key(config, key),
                "must be the scalar wildcard \"*\" or a nonempty string array",
            ));
        }
        return Ok(CorsValues::List(Vec::new()));
    };
    if required && raw_values.is_empty() {
        return Err(invalid_cors_configuration(
            key,
            source_for_key(config, key),
            "must be the scalar wildcard \"*\" or a nonempty string array",
        ));
    }

    let source = source_for_key(config, key);
    let mut values = Vec::new();
    for (index, raw) in raw_values.iter().enumerate() {
        let position = index + 1;
        if raw == "*" {
            return Err(invalid_cors_array_element(
                key,
                source,
                position,
                "must not contain a wildcard inside a string array",
            ));
        }
        let value = parse(raw, key, source, position)?;
        if !values.contains(&value) {
            values.push(value);
        }
    }
    Ok(CorsValues::List(values))
}

fn parse_credentials(config: &Config) -> Result<bool> {
    let key = "server.cors.credentials";
    if config.get_string_array(key).is_some() {
        return Err(invalid_cors_configuration(
            key,
            source_for_key(config, key),
            "must be exactly true or false",
        ));
    }
    match config.get(key) {
        None => Ok(false),
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        Some(_) => Err(invalid_cors_configuration(
            key,
            source_for_key(config, key),
            "must be exactly true or false",
        )),
    }
}

fn parse_max_age(config: &Config) -> Result<Option<Duration>> {
    let key = "server.cors.max_age_seconds";
    if config.get_string_array(key).is_some() {
        return Err(invalid_cors_configuration(
            key,
            source_for_key(config, key),
            "must be an unsigned integer number of seconds",
        ));
    }
    let Some(value) = config.get(key) else {
        return Ok(None);
    };
    let seconds = value.parse::<u64>().map_err(|_| {
        invalid_cors_configuration(
            key,
            source_for_key(config, key),
            "must be an unsigned integer number of seconds",
        )
    })?;
    Ok(Some(Duration::from_secs(seconds)))
}

fn source_for_key<'a>(config: &'a Config, key: &str) -> Option<&'a str> {
    config
        .source_of(key)
        .or_else(|| config.source_of_string_array(key))
}

fn invalid_cors_configuration(key: &'static str, source: Option<&str>, rule: &str) -> Error {
    Error::new(
        Diagnostic::new(
            MADS020,
            "CORS configuration is invalid",
            format!("{key} {rule}; source: {}", safe_source_label(source)),
        )
        .with_subject(key)
        .with_suggestion("configure valid application-wide CORS settings"),
    )
}

fn invalid_cors_array_element(
    key: &'static str,
    source: Option<&str>,
    position: usize,
    rule: &str,
) -> Error {
    invalid_cors_configuration(
        key,
        source,
        &format!("element at position {position} {rule}"),
    )
}

fn safe_source_label(source: Option<&str>) -> &'static str {
    match source {
        Some("defaults") => "defaults",
        Some("environment") => "environment",
        Some("mads.toml") => "mads.toml",
        Some("test") => "test",
        _ => "[REDACTED]",
    }
}

fn cors_evidence(context: &AutoConfigurationContext<'_>) -> Vec<AutoConfigurationConfigEvidence> {
    CORS_KEYS
        .into_iter()
        .map(|key| AutoConfigurationConfigEvidence::new(key, source_for_key(context.config(), key)))
        .collect()
}

mads_core::__private::inventory::submit! {
    AutoConfigurationDescriptor::new(
        CORS_AUTO_CONFIGURATION_ID,
        CORS_PLAN_TYPE_NAME,
        cors_plan_type_id,
        SourceLocation::new(file!(), line!(), column!()),
        evaluate,
        apply,
    )
}
