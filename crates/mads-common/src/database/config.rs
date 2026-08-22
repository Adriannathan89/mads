//! Database configuration.

use std::fmt;

use mads_core::Config;

use super::{DatabaseError, DatabaseResult};

/// Resolved configuration for the PostgreSQL database integration.
pub struct DatabaseConfig {
    url: String,
    pool_size: usize,
    migrate_on_startup: bool,
}

impl fmt::Debug for DatabaseConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DatabaseConfig")
            .field("url", &"[REDACTED]")
            .field("pool_size", &self.pool_size)
            .field("migrate_on_startup", &self.migrate_on_startup)
            .finish()
    }
}

impl DatabaseConfig {
    /// Creates configuration with the default pool size and migration policy.
    pub fn new(url: impl Into<String>) -> DatabaseResult<Self> {
        let url = url.into();
        validate_url(&url)?;
        Ok(Self {
            url,
            pool_size: 10,
            migrate_on_startup: false,
        })
    }

    /// Builds database configuration from already-resolved core configuration.
    pub fn from_config(config: &Config) -> DatabaseResult<Self> {
        let url = config
            .get("database.url")
            .ok_or_else(|| DatabaseError::configuration("database.url is required"))?;
        let mut database = Self::new(url)?;

        if let Some(pool_size) = config.get("database.pool_size") {
            let pool_size = pool_size.parse::<usize>().map_err(|_| {
                DatabaseError::configuration("database.pool_size must be a positive integer")
            })?;
            database = database.with_pool_size(pool_size)?;
        }

        if let Some(migrate) = config.get("database.migrate") {
            database.migrate_on_startup = migrate.parse::<bool>().map_err(|_| {
                DatabaseError::configuration("database.migrate must be true or false")
            })?;
        }

        Ok(database)
    }

    /// Sets the maximum number of connections in the pool.
    pub fn with_pool_size(mut self, pool_size: usize) -> DatabaseResult<Self> {
        if pool_size == 0 {
            return Err(DatabaseError::configuration(
                "database.pool_size must be a positive integer",
            ));
        }
        self.pool_size = pool_size;
        Ok(self)
    }

    /// Sets whether pending migrations run during application startup.
    pub fn with_migrate_on_startup(mut self, enabled: bool) -> Self {
        self.migrate_on_startup = enabled;
        self
    }

    /// Returns the configured maximum pool size.
    pub const fn pool_size(&self) -> usize {
        self.pool_size
    }

    /// Returns whether migrations run during application startup.
    pub const fn migrate_on_startup(&self) -> bool {
        self.migrate_on_startup
    }

    /// Returns the resolved database URL.
    pub fn url(&self) -> &str {
        &self.url
    }
}

fn validate_url(url: &str) -> DatabaseResult<()> {
    if url.is_empty() {
        return Err(DatabaseError::configuration(
            "database.url must not be empty",
        ));
    }
    if exact_variable_name(url).is_some() {
        return Err(DatabaseError::configuration(
            "database.url must be resolved before database configuration is built",
        ));
    }
    Ok(())
}

fn exact_variable_name(value: &str) -> Option<&str> {
    let variable = value.strip_prefix("${")?.strip_suffix('}')?;
    (!variable.is_empty()
        && !variable.contains("${")
        && !variable.contains('}')
        && !variable.contains(":-"))
    .then_some(variable)
}
