//! PostgreSQL connection pooling and blocking Diesel execution.

use std::{error::Error, fmt};

use deadpool_diesel::postgres::{Manager, Pool, Runtime};
use diesel::prelude::*;

use super::{DatabaseConfig, DatabaseError, DatabaseResult};

/// A cloneable, lazily connected PostgreSQL pool.
///
/// Clones share the same pool and close state. Connections are opened only
/// when an operation needs one, and every synchronous Diesel operation is
/// executed on the pool's blocking interaction boundary.
#[derive(Clone)]
pub struct Database {
    pub(super) pool: Pool,
}

impl fmt::Debug for Database {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Database")
            .field("status", &self.status())
            .finish()
    }
}

/// A point-in-time snapshot of a [`Database`] pool.
///
/// Pool counters are eventually consistent while concurrent work is running.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DatabasePoolStatus {
    max_size: usize,
    size: usize,
    available: usize,
    closed: bool,
}

impl DatabasePoolStatus {
    /// Returns the configured maximum number of connections.
    pub const fn max_size(self) -> usize {
        self.max_size
    }

    /// Returns the number of connections currently created by the pool.
    pub const fn size(self) -> usize {
        self.size
    }

    /// Returns the number of idle connections available immediately.
    pub const fn available(self) -> usize {
        self.available
    }

    /// Returns whether the pool is closed to current and future acquisition.
    pub const fn closed(self) -> bool {
        self.closed
    }
}

impl Database {
    /// Builds a lazy PostgreSQL pool from validated database configuration.
    ///
    /// This does not connect to PostgreSQL. A connection is established only
    /// when [`Self::run`] or [`Self::check`] acquires one.
    ///
    /// # Errors
    ///
    /// Returns a configuration error if the pool cannot be configured. The
    /// database URL is never included in the returned error's display or debug
    /// output.
    pub fn from_config(config: &DatabaseConfig) -> DatabaseResult<Self> {
        let manager = Manager::new(config.url().to_owned(), Runtime::Tokio1);
        let pool = Pool::builder(manager)
            .max_size(config.pool_size())
            .build()
            .map_err(|error| {
                DatabaseError::configuration_with_source(
                    "database.pool_size could not configure the pool",
                    error,
                )
            })?;

        Ok(Self { pool })
    }

    /// Runs a synchronous Diesel query through the pool's blocking boundary.
    ///
    /// The operation and its successful result must be `Send + 'static`
    /// because Diesel executes outside the asynchronous runtime's worker.
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseError::Pool`] when a connection cannot be acquired,
    /// [`DatabaseError::Interaction`] when the blocking boundary fails, or
    /// [`DatabaseError::Query`] when Diesel returns a query error.
    pub async fn run<T, F>(&self, operation: F) -> DatabaseResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut diesel::PgConnection) -> diesel::QueryResult<T> + Send + 'static,
    {
        let connection = self.pool.get().await.map_err(DatabaseError::Pool)?;
        normalize_query_result(connection.interact(operation).await)
    }

    /// Verifies that PostgreSQL accepts a simple `SELECT 1` query.
    ///
    /// The query follows the same pool acquisition and blocking execution path
    /// as [`Self::run`].
    ///
    /// # Errors
    ///
    /// Returns the same pool, interaction, and query errors as [`Self::run`].
    pub async fn check(&self) -> DatabaseResult<()> {
        self.run(|connection| {
            diesel::select(1_i32.into_sql::<diesel::sql_types::Integer>())
                .execute(connection)
                .map(|_| ())
        })
        .await
    }

    /// Returns a point-in-time snapshot of the managed connection pool.
    pub fn status(&self) -> DatabasePoolStatus {
        let status = self.pool.status();
        DatabasePoolStatus {
            max_size: status.max_size,
            size: status.size,
            available: status.available,
            closed: self.pool.is_closed(),
        }
    }

    /// Closes the pool to current and future connection acquisition.
    ///
    /// Closing is idempotent and affects every clone of this [`Database`].
    pub fn close(&self) {
        self.pool.close();
    }

    /// Returns whether this shared pool has been closed.
    pub fn is_closed(&self) -> bool {
        self.pool.is_closed()
    }

    #[allow(dead_code)]
    pub(super) async fn run_migration<T, F>(&self, operation: F) -> DatabaseResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut diesel::PgConnection) -> Result<T, Box<dyn Error + Send + Sync>>
            + Send
            + 'static,
    {
        let connection = self.pool.get().await.map_err(DatabaseError::Pool)?;
        normalize_migration_result(connection.interact(operation).await)
    }
}

fn normalize_query_result<T>(
    result: Result<diesel::QueryResult<T>, deadpool_diesel::InteractError>,
) -> DatabaseResult<T> {
    result
        .map_err(DatabaseError::Interaction)?
        .map_err(DatabaseError::Query)
}

fn normalize_migration_result<T>(
    result: Result<Result<T, Box<dyn Error + Send + Sync>>, deadpool_diesel::InteractError>,
) -> DatabaseResult<T> {
    result
        .map_err(DatabaseError::Interaction)?
        .map_err(DatabaseError::Migration)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::DatabaseErrorKind;

    #[test]
    fn query_errors_are_normalized_as_query_errors() {
        let error =
            normalize_query_result::<()>(Ok(Err(diesel::result::Error::NotFound))).unwrap_err();

        assert_eq!(error.kind(), DatabaseErrorKind::Query);
    }

    #[test]
    fn interaction_errors_are_not_normalized_as_query_errors() {
        let error =
            normalize_query_result::<()>(Err(deadpool_diesel::InteractError::Aborted)).unwrap_err();

        assert_eq!(error.kind(), DatabaseErrorKind::Interaction);
    }
}
