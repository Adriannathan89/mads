//! Deterministic configuration sources and merged configuration values.

use std::collections::BTreeMap;
use std::ffi::OsString;

use crate::Result;

/// A source of string configuration values.
pub trait ConfigSource: Send + Sync {
    /// Returns the source name used for attribution.
    fn name(&self) -> &str;

    /// Loads the source's configuration values.
    #[allow(clippy::result_large_err)]
    fn load(&self) -> Result<BTreeMap<String, String>>;
}

/// A fixed, named set of configuration values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MapSource {
    name: String,
    values: BTreeMap<String, String>,
}

impl MapSource {
    /// Creates a named source from an iterable of key-value pairs.
    pub fn new<I, K, V>(name: impl Into<String>, values: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        Self {
            name: name.into(),
            values: values
                .into_iter()
                .map(|(key, value)| (key.into(), value.into()))
                .collect(),
        }
    }
}

impl ConfigSource for MapSource {
    fn name(&self) -> &str {
        &self.name
    }

    fn load(&self) -> Result<BTreeMap<String, String>> {
        Ok(self.values.clone())
    }
}

/// An environment-variable configuration source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvSource {
    prefix: String,
    variables: Vec<(OsString, OsString)>,
}

impl EnvSource {
    /// Creates a source that reads the current process environment.
    pub fn new(prefix: impl Into<String>) -> Self {
        Self::from_iter(prefix, std::env::vars_os())
    }

    /// Creates a source from environment variables supplied by the caller.
    pub fn from_iter<I, K, V>(prefix: impl Into<String>, variables: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<OsString>,
        V: Into<OsString>,
    {
        Self {
            prefix: prefix.into(),
            variables: variables
                .into_iter()
                .map(|(key, value)| (key.into(), value.into()))
                .collect(),
        }
    }
}

impl ConfigSource for EnvSource {
    fn name(&self) -> &str {
        "environment"
    }

    fn load(&self) -> Result<BTreeMap<String, String>> {
        let values = self
            .variables
            .iter()
            .filter_map(|(key, value)| {
                let key = key.to_str()?.strip_prefix(&self.prefix)?;
                let value = value.to_str()?;
                Some((normalize_environment_key(key), value.to_owned()))
            })
            .collect();
        Ok(values)
    }
}

/// A configuration value and the source that supplied it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigValue {
    value: String,
    source: String,
}

impl ConfigValue {
    /// Returns the string value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the name of the source that supplied the value.
    pub fn source(&self) -> &str {
        &self.source
    }
}

/// Configuration values merged in source insertion order.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Config {
    values: BTreeMap<String, ConfigValue>,
}

impl Config {
    /// Creates an empty configuration.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Returns a configuration value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(ConfigValue::value)
    }

    /// Returns the source name for a configuration key.
    pub fn source_of(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(ConfigValue::source)
    }

    /// Iterates over keys and their attributed values in lexical key order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &ConfigValue)> {
        self.values.iter().map(|(key, value)| (key.as_str(), value))
    }

    /// Returns the number of configuration values.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns whether the configuration has no values.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// Builds a configuration by merging ordered sources.
#[derive(Default)]
pub struct ConfigBuilder {
    sources: Vec<Box<dyn ConfigSource>>,
}

impl ConfigBuilder {
    /// Creates an empty configuration builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a source whose values override earlier source values.
    pub fn source<S>(mut self, source: S) -> Self
    where
        S: ConfigSource + 'static,
    {
        self.sources.push(Box::new(source));
        self
    }

    /// Loads and merges all sources in insertion order.
    #[allow(clippy::result_large_err)]
    pub fn build(self) -> Result<Config> {
        let mut values = BTreeMap::new();
        for source in self.sources {
            let source_name = source.name().to_owned();
            for (key, value) in source.load()? {
                values.insert(
                    key,
                    ConfigValue {
                        value,
                        source: source_name.clone(),
                    },
                );
            }
        }
        Ok(Config { values })
    }
}

fn normalize_environment_key(key: &str) -> String {
    key.to_ascii_lowercase().replace("__", ".")
}
