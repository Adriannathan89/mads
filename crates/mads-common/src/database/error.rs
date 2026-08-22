//! Normalized database errors.

use std::fmt;

/// The result type used by database integration APIs.
pub type DatabaseResult<T> = std::result::Result<T, DatabaseError>;

/// The stable class of a database integration error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DatabaseErrorKind {
    /// Database configuration is missing or invalid.
    Configuration,
    /// A database connection could not be acquired from the pool.
    Pool,
    /// A blocking database operation could not run.
    Interaction,
    /// A database query failed.
    Query,
    /// A database migration failed.
    Migration,
}

/// A normalized error from a database integration operation.
#[non_exhaustive]
pub enum DatabaseError {
    /// Database configuration was missing or invalid.
    Configuration {
        /// A redacted structural description of the invalid configuration.
        message: String,
        /// The underlying cause, when one is safe to retain.
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    /// A connection could not be acquired from the PostgreSQL pool.
    Pool(deadpool_diesel::postgres::PoolError),
    /// A blocking database operation failed.
    Interaction(deadpool_diesel::InteractError),
    /// A Diesel query failed.
    Query(diesel::result::Error),
    /// A Diesel migration failed.
    Migration(Box<dyn std::error::Error + Send + Sync>),
}

impl DatabaseError {
    pub(super) fn configuration(message: impl Into<String>) -> Self {
        Self::Configuration {
            message: message.into(),
            source: None,
        }
    }

    pub(super) fn configuration_with_source<E>(message: impl Into<String>, source: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Configuration {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Returns the stable class of this error.
    pub const fn kind(&self) -> DatabaseErrorKind {
        match self {
            Self::Configuration { .. } => DatabaseErrorKind::Configuration,
            Self::Pool(_) => DatabaseErrorKind::Pool,
            Self::Interaction(_) => DatabaseErrorKind::Interaction,
            Self::Query(_) => DatabaseErrorKind::Query,
            Self::Migration(_) => DatabaseErrorKind::Migration,
        }
    }
}

impl fmt::Debug for DatabaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DatabaseError")
            .field("kind", &self.kind())
            .finish()
    }
}

impl fmt::Display for DatabaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration { message, .. } => {
                write!(formatter, "database configuration is invalid: {message}")
            }
            Self::Pool(_) => formatter.write_str("database connection could not be acquired"),
            Self::Interaction(_) => formatter.write_str("database blocking operation failed"),
            Self::Query(_) => formatter.write_str("database query failed"),
            Self::Migration(_) => formatter.write_str("database migration failed"),
        }
    }
}

impl std::error::Error for DatabaseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Configuration { source, .. } => source
                .as_deref()
                .map(|source| source as &(dyn std::error::Error + 'static)),
            Self::Pool(source) => Some(source),
            Self::Interaction(source) => Some(source),
            Self::Query(source) => Some(source),
            Self::Migration(source) => Some(source.as_ref()),
        }
    }
}
