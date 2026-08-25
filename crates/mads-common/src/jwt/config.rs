//! Passport JWT configuration parsing.

use std::collections::HashSet;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use mads_core::Config;

use super::{JwtAlgorithm, JwtError, JwtErrorKind, JwtResult};

const DEFAULT_MAX_TOKEN_BYTES: usize = 8_192;

enum PassportKeyMode {
    SimpleHmac { secret: Arc<[u8]> },
}

/// Immutable Passport JWT key description and common validation policy.
pub struct PassportConfig {
    mode: PassportKeyMode,
    algorithms: Vec<JwtAlgorithm>,
    issuer: Option<String>,
    audiences: Vec<String>,
    clock_skew_seconds: u64,
    max_token_bytes: usize,
}

impl PassportConfig {
    /// Parses and validates Passport JWT configuration.
    ///
    /// This release accepts a top-level `passport.secret` with exactly one
    /// HMAC algorithm. Named key rings are parsed separately.
    pub fn from_config(config: &Config) -> JwtResult<Self> {
        reject_named_key_fields(config)?;

        let secret = required_scalar(config, "passport.secret")?;
        let algorithms = match optional_string_array(config, "passport.algorithms")? {
            Some(values) => deduplicate_algorithms(values)?,
            None => vec![JwtAlgorithm::Hs256],
        };
        let algorithm = match algorithms.as_slice() {
            [algorithm @ (JwtAlgorithm::Hs256 | JwtAlgorithm::Hs384 | JwtAlgorithm::Hs512)] => {
                *algorithm
            }
            _ => return Err(invalid_configuration()),
        };
        if secret.len() < minimum_hmac_secret_bytes(algorithm) {
            return Err(invalid_configuration());
        }

        let issuer = optional_scalar(config, "passport.issuer")?.map(str::to_owned);
        let audiences = match optional_string_array(config, "passport.audiences")? {
            Some(values) => deduplicate_strings(values),
            None => Vec::new(),
        };
        let clock_skew_seconds =
            parse_optional_u64(config, "passport.clock_skew_seconds")?.unwrap_or_default();
        let max_token_bytes = parse_optional_usize(config, "passport.max_token_bytes")?
            .unwrap_or(DEFAULT_MAX_TOKEN_BYTES);
        if max_token_bytes == 0 {
            return Err(invalid_configuration());
        }

        Ok(Self {
            mode: PassportKeyMode::SimpleHmac {
                secret: Arc::from(secret.as_bytes()),
            },
            algorithms,
            issuer,
            audiences,
            clock_skew_seconds,
            max_token_bytes,
        })
    }

    /// Returns the configured algorithm allowlist in deterministic order.
    pub fn algorithms(&self) -> &[JwtAlgorithm] {
        &self.algorithms
    }

    /// Returns the configured issuer, when issuer validation is enabled.
    pub fn issuer(&self) -> Option<&str> {
        self.issuer.as_deref()
    }

    /// Returns the configured audiences in deterministic order.
    pub fn audiences(&self) -> &[String] {
        &self.audiences
    }

    /// Returns the allowed validation clock skew in seconds.
    pub const fn clock_skew_seconds(&self) -> u64 {
        self.clock_skew_seconds
    }

    /// Returns the maximum accepted encoded-token size in bytes.
    pub const fn max_token_bytes(&self) -> usize {
        self.max_token_bytes
    }
}

impl fmt::Debug for PassportConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let key_mode = match &self.mode {
            PassportKeyMode::SimpleHmac { secret } => {
                debug_assert!(!secret.is_empty());
                "simple-hmac"
            }
        };
        formatter
            .debug_struct("PassportConfig")
            .field("key_mode", &key_mode)
            .field("algorithms", &self.algorithms)
            .field("has_issuer", &self.issuer.is_some())
            .field("audience_count", &self.audiences.len())
            .field("clock_skew_seconds", &self.clock_skew_seconds)
            .field("max_token_bytes", &self.max_token_bytes)
            .finish()
    }
}

fn reject_named_key_fields(config: &Config) -> JwtResult<()> {
    let is_named = |key: &str| key == "passport.active_key" || key.starts_with("passport.keys.");
    if config.iter().any(|(key, _)| is_named(key))
        || config.iter_string_arrays().any(|(key, _)| is_named(key))
    {
        return Err(invalid_configuration());
    }
    Ok(())
}

fn required_scalar<'a>(config: &'a Config, key: &str) -> JwtResult<&'a str> {
    optional_scalar(config, key)?.ok_or_else(invalid_configuration)
}

fn optional_scalar<'a>(config: &'a Config, key: &str) -> JwtResult<Option<&'a str>> {
    if config.get_string_array(key).is_some() {
        return Err(invalid_configuration());
    }
    match config.get(key) {
        Some("") => Err(invalid_configuration()),
        value => Ok(value),
    }
}

fn optional_string_array<'a>(config: &'a Config, key: &str) -> JwtResult<Option<&'a [String]>> {
    if config.get(key).is_some() {
        return Err(invalid_configuration());
    }
    match config.get_string_array(key) {
        Some(values) if values.is_empty() || values.iter().any(String::is_empty) => {
            Err(invalid_configuration())
        }
        values => Ok(values),
    }
}

fn parse_optional_u64(config: &Config, key: &str) -> JwtResult<Option<u64>> {
    optional_scalar(config, key)?
        .map(|value| value.parse().map_err(|_| invalid_configuration()))
        .transpose()
}

fn parse_optional_usize(config: &Config, key: &str) -> JwtResult<Option<usize>> {
    optional_scalar(config, key)?
        .map(|value| value.parse().map_err(|_| invalid_configuration()))
        .transpose()
}

fn deduplicate_algorithms(values: &[String]) -> JwtResult<Vec<JwtAlgorithm>> {
    let mut seen = HashSet::new();
    let mut algorithms = Vec::new();
    for value in values {
        let algorithm = JwtAlgorithm::from_str(value).map_err(|_| invalid_configuration())?;
        if seen.insert(algorithm) {
            algorithms.push(algorithm);
        }
    }
    Ok(algorithms)
}

fn deduplicate_strings(values: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .iter()
        .filter(|value| seen.insert(value.as_str()))
        .cloned()
        .collect()
}

const fn minimum_hmac_secret_bytes(algorithm: JwtAlgorithm) -> usize {
    match algorithm {
        JwtAlgorithm::Hs256 => 32,
        JwtAlgorithm::Hs384 => 48,
        JwtAlgorithm::Hs512 => 64,
        JwtAlgorithm::Rs256
        | JwtAlgorithm::Rs384
        | JwtAlgorithm::Rs512
        | JwtAlgorithm::Es256
        | JwtAlgorithm::Es384 => 0,
    }
}

const fn invalid_configuration() -> JwtError {
    JwtError::new(JwtErrorKind::InvalidConfiguration)
}
