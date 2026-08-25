//! Deterministic configuration sources and merged configuration values.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

use crate::{Diagnostic, Error, MADS020, Result};

/// A redacted configuration document containing supported value shapes.
#[derive(Clone, Default, Eq, PartialEq)]
pub struct ConfigDocument {
    scalars: BTreeMap<String, String>,
    string_arrays: BTreeMap<String, Vec<String>>,
}

impl fmt::Debug for ConfigDocument {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConfigDocument")
            .field("entries", &self.len())
            .field("values", &"[REDACTED]")
            .finish()
    }
}

impl ConfigDocument {
    /// Creates an empty configuration document.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a configuration document from scalar values.
    pub fn from_scalars(values: BTreeMap<String, String>) -> Self {
        Self {
            scalars: values,
            string_arrays: BTreeMap::new(),
        }
    }

    /// Inserts a scalar, replacing any value at the same key.
    pub fn insert_scalar(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        self.string_arrays.remove(&key);
        self.scalars.insert(key, value.into());
    }

    /// Inserts a string array, replacing any value at the same key.
    pub fn insert_string_array<I, V>(&mut self, key: impl Into<String>, values: I)
    where
        I: IntoIterator<Item = V>,
        V: Into<String>,
    {
        let key = key.into();
        self.scalars.remove(&key);
        self.string_arrays
            .insert(key, values.into_iter().map(Into::into).collect());
    }

    /// Returns the number of configuration entries across all value shapes.
    pub fn len(&self) -> usize {
        self.scalars.len() + self.string_arrays.len()
    }

    /// Returns whether the document has no configuration entries.
    pub fn is_empty(&self) -> bool {
        self.scalars.is_empty() && self.string_arrays.is_empty()
    }
}

/// A source of string configuration values.
pub trait ConfigSource: Send + Sync {
    /// Returns the source name used for attribution.
    fn name(&self) -> &str;

    /// Loads the source's configuration values.
    #[allow(clippy::result_large_err)]
    fn load(&self) -> Result<BTreeMap<String, String>>;

    /// Loads all supported configuration value shapes.
    #[allow(clippy::result_large_err)]
    fn load_document(&self) -> Result<ConfigDocument> {
        self.load().map(ConfigDocument::from_scalars)
    }
}

/// A fixed, named set of configuration values.
#[derive(Clone, Eq, PartialEq)]
pub struct MapSource {
    name: String,
    scalars: BTreeMap<String, String>,
    string_arrays: BTreeMap<String, Vec<String>>,
}

impl fmt::Debug for MapSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MapSource")
            .field("name", &self.name)
            .field("entries", &(self.scalars.len() + self.string_arrays.len()))
            .field("values", &"[REDACTED]")
            .finish()
    }
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
            scalars: values
                .into_iter()
                .map(|(key, value)| (key.into(), value.into()))
                .collect(),
            string_arrays: BTreeMap::new(),
        }
    }

    /// Adds a string array, replacing any scalar at the same key.
    pub fn with_string_array<I, V>(mut self, key: impl Into<String>, values: I) -> Self
    where
        I: IntoIterator<Item = V>,
        V: Into<String>,
    {
        let key = key.into();
        self.scalars.remove(&key);
        self.string_arrays
            .insert(key, values.into_iter().map(Into::into).collect());
        self
    }
}

impl ConfigSource for MapSource {
    fn name(&self) -> &str {
        &self.name
    }

    fn load(&self) -> Result<BTreeMap<String, String>> {
        Ok(self.scalars.clone())
    }

    fn load_document(&self) -> Result<ConfigDocument> {
        Ok(ConfigDocument {
            scalars: self.scalars.clone(),
            string_arrays: self.string_arrays.clone(),
        })
    }
}

/// An environment-variable configuration source.
#[derive(Clone, Eq, PartialEq)]
pub struct EnvSource {
    prefix: String,
    variables: Vec<(OsString, OsString)>,
}

impl fmt::Debug for EnvSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EnvSource")
            .field("prefix", &self.prefix)
            .field("entries", &self.variables.len())
            .field("values", &"[REDACTED]")
            .finish()
    }
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

/// A TOML configuration file source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TomlSource {
    path: PathBuf,
    name: String,
}

impl TomlSource {
    /// Creates a TOML source backed by `path`.
    pub fn file(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let name = path.to_string_lossy().into_owned();
        Self { path, name }
    }
}

impl ConfigSource for TomlSource {
    fn name(&self) -> &str {
        &self.name
    }

    fn load(&self) -> Result<BTreeMap<String, String>> {
        let input = std::fs::read_to_string(&self.path).map_err(|source| {
            Error::with_source(
                Diagnostic::new(
                    MADS020,
                    "configuration file could not be read",
                    format!("could not read configuration file `{}`", self.name),
                )
                .with_subject(&self.name),
                source,
            )
        })?;
        let parsed = input.parse::<toml::Value>().map_err(|_| {
            Error::new(
                Diagnostic::new(
                    MADS020,
                    "configuration file could not be parsed",
                    format!("configuration file `{}` is not valid TOML", self.name),
                )
                .with_subject(&self.name),
            )
        })?;
        let document = input.parse::<toml_edit::DocumentMut>().map_err(|_| {
            Error::new(
                Diagnostic::new(
                    MADS020,
                    "configuration file could not be parsed",
                    format!("configuration file `{}` is not valid TOML", self.name),
                )
                .with_subject(&self.name),
            )
        })?;
        if let Some(key) = inline_table_key("", document.as_table()) {
            return Err(unsupported_value(&key, "inline table"));
        }
        let mut output = BTreeMap::new();
        flatten_toml("", parsed, &mut output)?;
        Ok(output)
    }
}

fn inline_table_key(prefix: &str, table: &toml_edit::Table) -> Option<String> {
    for (segment, item) in table {
        let key = if prefix.is_empty() {
            segment.to_owned()
        } else {
            format!("{prefix}.{segment}")
        };
        match item {
            toml_edit::Item::Value(toml_edit::Value::InlineTable(_)) => return Some(key),
            toml_edit::Item::Table(nested) => {
                if let Some(key) = inline_table_key(&key, nested) {
                    return Some(key);
                }
            }
            toml_edit::Item::None
            | toml_edit::Item::Value(_)
            | toml_edit::Item::ArrayOfTables(_) => {}
        }
    }
    None
}

fn flatten_toml(
    prefix: &str,
    value: toml::Value,
    output: &mut BTreeMap<String, String>,
) -> Result<()> {
    match value {
        toml::Value::Table(table) => {
            if prefix.is_empty() && table.is_empty() {
                return Ok(());
            }
            for (segment, value) in table {
                let key = if prefix.is_empty() {
                    segment
                } else {
                    format!("{prefix}.{segment}")
                };
                flatten_toml(&key, value, output)?;
            }
            Ok(())
        }
        toml::Value::String(value) => insert_scalar(prefix, value, output),
        toml::Value::Integer(value) => insert_scalar(prefix, value.to_string(), output),
        toml::Value::Float(value) => insert_scalar(prefix, value.to_string(), output),
        toml::Value::Boolean(value) => insert_scalar(prefix, value.to_string(), output),
        toml::Value::Array(_) => Err(unsupported_value(prefix, "array")),
        toml::Value::Datetime(_) => Err(unsupported_value(prefix, "datetime")),
    }
}

fn insert_scalar(key: &str, value: String, output: &mut BTreeMap<String, String>) -> Result<()> {
    if key.is_empty() {
        return Err(unsupported_value(key, "root scalar"));
    }
    output.insert(key.to_owned(), value);
    Ok(())
}

fn unsupported_value(key: &str, kind: &str) -> Error {
    Error::new(
        Diagnostic::new(
            MADS020,
            "unsupported configuration value",
            format!("configuration value has unsupported type `{kind}`"),
        )
        .with_subject(key),
    )
}

/// A dotenv file whose variables are available for configuration interpolation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DotenvSource {
    path: PathBuf,
    required: bool,
    name: String,
}

impl DotenvSource {
    /// Creates a dotenv source that is ignored when the file does not exist.
    pub fn optional(path: impl Into<PathBuf>) -> Self {
        Self::new(path.into(), false)
    }

    /// Creates a dotenv source that fails when the file does not exist.
    pub fn required(path: impl Into<PathBuf>) -> Self {
        Self::new(path.into(), true)
    }

    fn new(path: PathBuf, required: bool) -> Self {
        let name = path.to_string_lossy().into_owned();
        Self {
            path,
            required,
            name,
        }
    }
}

/// A configuration value and the source that supplied it.
#[derive(Clone, Eq, PartialEq)]
pub struct ConfigValue {
    value: String,
    source: String,
}

#[derive(Clone, Eq, PartialEq)]
struct ConfigStringArrayValue {
    value: Vec<String>,
    source: String,
}

impl fmt::Debug for ConfigStringArrayValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConfigStringArrayValue")
            .field("elements", &self.value.len())
            .field("source", &self.source)
            .finish()
    }
}

impl fmt::Debug for ConfigValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConfigValue")
            .field("value", &"[REDACTED]")
            .field("source", &self.source)
            .finish()
    }
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
    string_arrays: BTreeMap<String, ConfigStringArrayValue>,
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

    /// Returns a string-array configuration value by key.
    pub fn get_string_array(&self, key: &str) -> Option<&[String]> {
        self.string_arrays
            .get(key)
            .map(|value| value.value.as_slice())
    }

    /// Returns the source name for a string-array configuration key.
    pub fn source_of_string_array(&self, key: &str) -> Option<&str> {
        self.string_arrays
            .get(key)
            .map(|value| value.source.as_str())
    }

    /// Iterates over string-array keys and values in lexical key order.
    pub fn iter_string_arrays(&self) -> impl Iterator<Item = (&str, &[String])> {
        self.string_arrays
            .iter()
            .map(|(key, value)| (key.as_str(), value.value.as_slice()))
    }

    /// Returns the number of configuration values.
    pub fn len(&self) -> usize {
        self.values.len() + self.string_arrays.len()
    }

    /// Returns whether the configuration has no values.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty() && self.string_arrays.is_empty()
    }
}

/// Builds a configuration by merging ordered sources.
#[derive(Default)]
pub struct ConfigBuilder {
    dotenv_sources: Vec<DotenvSource>,
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

    /// Adds a dotenv source used only for exact configuration interpolation.
    pub fn dotenv(mut self, source: DotenvSource) -> Self {
        self.dotenv_sources.push(source);
        self
    }

    /// Loads and merges all sources in insertion order.
    #[allow(clippy::result_large_err)]
    pub fn build(self) -> Result<Config> {
        self.build_with_environment(std::env::vars_os())
    }

    fn build_with_environment<I, K, V>(self, environment: I) -> Result<Config>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<OsString>,
        V: Into<OsString>,
    {
        let mut variables = BTreeMap::new();
        for source in self.dotenv_sources {
            let iterator = match dotenvy::from_path_iter(&source.path) {
                Ok(iterator) => iterator,
                Err(error) if !source.required && error.not_found() => continue,
                Err(error) => return Err(dotenv_open_error(&source, error)),
            };
            for entry in iterator {
                let (key, value) = entry.map_err(|_| dotenv_parse_error(&source))?;
                variables.insert(key, value);
            }
        }
        for (key, value) in environment {
            let key = key.into();
            let value = value.into();
            if let (Some(key), Some(value)) = (key.to_str(), value.to_str()) {
                variables.insert(key.to_owned(), value.to_owned());
            }
        }

        let mut values = BTreeMap::new();
        let mut string_arrays = BTreeMap::new();
        for source in self.sources {
            let source_name = source.name().to_owned();
            let document = source.load_document()?;
            for (key, value) in document.scalars {
                string_arrays.remove(&key);
                values.insert(
                    key,
                    ConfigValue {
                        value,
                        source: source_name.clone(),
                    },
                );
            }
            for (key, value) in document.string_arrays {
                values.remove(&key);
                string_arrays.insert(
                    key,
                    ConfigStringArrayValue {
                        value,
                        source: source_name.clone(),
                    },
                );
            }
        }
        for (key, value) in &mut values {
            if let Some(variable) = exact_variable_name(&value.value) {
                let resolved = variables.get(variable).ok_or_else(|| {
                    Error::new(
                        Diagnostic::new(
                            MADS020,
                            "configuration variable is missing",
                            format!("configuration variable `{variable}` is not defined"),
                        )
                        .with_subject(key),
                    )
                })?;
                value.value.clone_from(resolved);
            }
        }
        Ok(Config {
            values,
            string_arrays,
        })
    }
}

fn dotenv_open_error(source: &DotenvSource, error: dotenvy::Error) -> Error {
    match error {
        dotenvy::Error::Io(error) => Error::with_source(
            Diagnostic::new(
                MADS020,
                "configuration variable file could not be read",
                format!(
                    "could not read configuration variable file `{}`",
                    source.name
                ),
            )
            .with_subject(&source.name),
            error,
        ),
        _ => dotenv_parse_error(source),
    }
}

fn dotenv_parse_error(source: &DotenvSource) -> Error {
    Error::new(
        Diagnostic::new(
            MADS020,
            "configuration variable file could not be parsed",
            format!(
                "configuration variable file `{}` is not valid dotenv syntax",
                source.name
            ),
        )
        .with_subject(&source.name),
    )
}

fn exact_variable_name(value: &str) -> Option<&str> {
    let variable = value.strip_prefix("${")?.strip_suffix('}')?;
    (!variable.is_empty()
        && !variable.contains("${")
        && !variable.contains('}')
        && !variable.contains(":-"))
    .then_some(variable)
}

fn normalize_environment_key(key: &str) -> String {
    key.to_ascii_lowercase().replace("__", ".")
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;

    #[cfg(unix)]
    use std::os::unix::ffi::OsStringExt;

    use super::{ConfigBuilder, DotenvSource, TomlSource};

    #[test]
    fn optional_dotenv_resolves_exact_placeholders_without_becoming_config() {
        let directory = tempfile::tempdir().unwrap();
        let dotenv = directory.path().join(".env");
        let toml = directory.path().join("mads.toml");
        fs::write(&dotenv, "DATABASE_URL=postgres://dotenv-value\n").unwrap();
        fs::write(&toml, "[database]\nurl = \"${DATABASE_URL}\"\n").unwrap();

        let config = ConfigBuilder::new()
            .dotenv(DotenvSource::optional(&dotenv))
            .source(TomlSource::file(&toml))
            .build_with_environment(std::iter::empty::<(OsString, OsString)>())
            .unwrap();

        assert_eq!(config.get("database.url"), Some("postgres://dotenv-value"));
        assert_eq!(config.get("DATABASE_URL"), None);
        assert_eq!(config.source_of("database.url"), toml.to_str());
    }

    #[test]
    fn unicode_process_environment_overrides_dotenv_variables() {
        let directory = tempfile::tempdir().unwrap();
        let dotenv = directory.path().join(".env");
        fs::write(&dotenv, "DATABASE_URL=postgres://dotenv-value\n").unwrap();

        let config = ConfigBuilder::new()
            .dotenv(DotenvSource::required(dotenv))
            .source(super::MapSource::new(
                "test",
                [("database.url", "${DATABASE_URL}")],
            ))
            .build_with_environment([("DATABASE_URL", "postgres://process-value")])
            .unwrap();

        assert_eq!(config.get("database.url"), Some("postgres://process-value"));
    }

    #[cfg(unix)]
    #[test]
    fn non_unicode_process_environment_is_ignored_for_interpolation() {
        let directory = tempfile::tempdir().unwrap();
        let dotenv = directory.path().join(".env");
        fs::write(&dotenv, "DATABASE_URL=postgres://dotenv-value\n").unwrap();
        let invalid = OsString::from_vec(vec![0xFF]);

        let config = ConfigBuilder::new()
            .dotenv(DotenvSource::required(dotenv))
            .source(super::MapSource::new(
                "test",
                [("database.url", "${DATABASE_URL}")],
            ))
            .build_with_environment([
                (invalid.clone(), OsString::from("ignored")),
                (OsString::from("DATABASE_URL"), invalid),
            ])
            .unwrap();

        assert_eq!(config.get("database.url"), Some("postgres://dotenv-value"));
    }
}
