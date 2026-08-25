//! Explicit database bootstrap and application lifecycle integration.

use std::{
    fmt,
    sync::{Arc, Mutex},
};

use diesel::{migration::MigrationSource, pg::Pg};
use diesel_migrations::EmbeddedMigrations;
use mads_core::{
    ApplicationContext, Diagnostic, Error, LifecycleFuture, LifecycleHook, MadsBuilder, Result,
};

use super::{Database, DatabaseConfig, DatabaseError, DatabaseErrorKind, MADS100, MADS101};

/// The explicit database value registered with an application builder.
///
/// Its debug representation redacts the database configuration, including the
/// connection URL.
pub struct DatabaseBootstrap {
    config: DatabaseConfig,
    migrations: Option<EmbeddedMigrations>,
}

impl fmt::Debug for DatabaseBootstrap {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DatabaseBootstrap")
            .field("config", &self.config)
            .field("has_embedded_migrations", &self.migrations.is_some())
            .finish()
    }
}

impl DatabaseBootstrap {
    /// Creates an explicit database bootstrap value without embedded migrations.
    pub fn new(config: DatabaseConfig) -> Self {
        Self {
            config,
            migrations: None,
        }
    }

    /// Attaches embedded migrations that may run during application startup.
    pub fn with_migrations(mut self, migrations: EmbeddedMigrations) -> Self {
        self.migrations = Some(migrations);
        self
    }

    #[allow(clippy::result_large_err)]
    fn validate(&self) -> Result<()> {
        if self.config.migrate_on_startup() && self.migrations.is_none() {
            return Err(Error::new(Diagnostic::new(
                MADS100,
                "database bootstrap is invalid",
                "database.migrate requires an embedded migration source",
            )));
        }
        Ok(())
    }
}

/// Registers an explicitly configured database and its lifecycle hook.
///
/// Version 0.5 ships the official Diesel conditional default.
/// [`DatabaseBootstrap`] remains the explicit override: this extension
/// registers the provided database and its infrastructure lifecycle hook. It
/// does not perform auto-configuration requirement detection or back-off;
/// those belong to the official default.
///
/// # Example
///
/// ```ignore
/// use mads_common::{DatabaseBootstrap, DatabaseConfig, MadsBuilderDatabaseExt};
/// use mads_core::Mads;
///
/// const MIGRATIONS: diesel_migrations::EmbeddedMigrations =
///     diesel_migrations::embed_migrations!("migrations");
///
/// let config = DatabaseConfig::new("postgres://localhost/mads")?;
/// let mut builder = Mads::builder();
/// builder.database(DatabaseBootstrap::new(config).with_migrations(MIGRATIONS))?;
/// let application = builder.build().await?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// Startup checks database readiness before later lifecycle hooks and before
/// the HTTP listener is bound. When [`DatabaseConfig::migrate_on_startup`] is
/// enabled, an embedded migration source is required and pending migrations
/// run immediately after that readiness check. Calling this extension twice,
/// or after manually providing a [`Database`], returns the existing duplicate
/// provider error.
pub trait MadsBuilderDatabaseExt {
    /// Provides the database and registers its lifecycle hook.
    ///
    /// # Errors
    ///
    /// Returns `MADS100` when the bootstrap value is invalid or the pool cannot
    /// be created, and preserves the core duplicate-provider diagnostic when a
    /// database has already been provided.
    #[allow(clippy::result_large_err)]
    fn database(&mut self, bootstrap: DatabaseBootstrap) -> Result<&mut Self>;

    /// Registers the one embedded migration source for the official database default.
    ///
    /// This creates no pool and affects only the official
    /// `mads.common.database.diesel` auto-configuration. Explicit
    /// [`DatabaseBootstrap`] values continue to own migrations supplied with
    /// [`DatabaseBootstrap::with_migrations`].
    ///
    /// # Errors
    ///
    /// Returns `MADS101` when another embedded migration source has already
    /// been registered for the official database default.
    #[allow(clippy::result_large_err)]
    fn database_migrations(&mut self, migrations: EmbeddedMigrations) -> Result<&mut Self>;
}

impl MadsBuilderDatabaseExt for MadsBuilder {
    #[allow(clippy::result_large_err)]
    fn database(&mut self, bootstrap: DatabaseBootstrap) -> Result<&mut Self> {
        bootstrap.validate()?;
        let migrate_on_startup = bootstrap.config.migrate_on_startup();
        let database = Database::from_config(&bootstrap.config).map_err(|source| {
            database_framework_error(
                MADS100,
                "database bootstrap failed",
                "database pool creation failed",
                source,
            )
        })?;
        let migrations = bootstrap.migrations;

        self.provide(database)?;
        self.__infrastructure_lifecycle_hook(
            "mads.common.database.diesel",
            DatabaseLifecycle::new(migrate_on_startup, migrations.map(DatabaseMigrations::new)),
        );
        Ok(self)
    }

    #[allow(clippy::result_large_err)]
    fn database_migrations(&mut self, migrations: EmbeddedMigrations) -> Result<&mut Self> {
        if !self.__auto_configuration_input(
            super::auto_configuration::DATABASE_AUTO_CONFIGURATION_ID,
            DatabaseMigrations::new(migrations),
        ) {
            return Err(Error::new(Diagnostic::new(
                MADS101,
                "database auto-configuration is invalid",
                "one embedded migration source is already registered",
            )));
        }
        Ok(self)
    }
}

pub(super) struct DatabaseLifecycle {
    migrate_on_startup: bool,
    migrations: Mutex<Option<DatabaseMigrations>>,
}

impl DatabaseLifecycle {
    pub(super) fn new(migrate_on_startup: bool, migrations: Option<DatabaseMigrations>) -> Self {
        Self {
            migrate_on_startup,
            migrations: Mutex::new(migrations),
        }
    }

    #[allow(clippy::result_large_err)]
    fn take_migrations(&self) -> Result<DatabaseMigrations> {
        let mut migrations = self.migrations.lock().map_err(|_| {
            Error::new(Diagnostic::new(
                MADS100,
                "database bootstrap failed",
                "database startup migration source is unavailable",
            ))
        })?;
        migrations.take().ok_or_else(|| {
            Error::new(Diagnostic::new(
                MADS100,
                "database bootstrap failed",
                "database startup migration source is unavailable",
            ))
        })
    }
}

/// Cloneable ownership for the one embedded migration source registered by an
/// official database auto-configuration.
///
/// Diesel 2.2 deliberately makes [`EmbeddedMigrations`] neither `Copy` nor
/// `Clone`. The builder retains the source for repeated analysis while the
/// lifecycle hook needs to own it after application construction, so this
/// adapter shares the immutable embedded source without duplicating it.
#[derive(Clone)]
pub(super) struct DatabaseMigrations(Arc<EmbeddedMigrations>);

impl DatabaseMigrations {
    pub(super) fn new(migrations: EmbeddedMigrations) -> Self {
        Self(Arc::new(migrations))
    }
}

impl MigrationSource<Pg> for DatabaseMigrations {
    fn migrations(
        &self,
    ) -> diesel::migration::Result<Vec<Box<dyn diesel::migration::Migration<Pg>>>> {
        self.0.migrations()
    }
}

impl LifecycleHook for DatabaseLifecycle {
    fn name(&self) -> &str {
        "database"
    }

    fn start<'a>(&'a self, context: &'a ApplicationContext) -> LifecycleFuture<'a> {
        Box::pin(async move {
            let database = context.resolve::<Database>()?;

            if let Err(source) = database.check().await {
                database.close();
                return Err(database_framework_error(
                    MADS100,
                    "database bootstrap failed",
                    "database readiness check failed",
                    source,
                ));
            }

            if self.migrate_on_startup {
                let migrations = match self.take_migrations() {
                    Ok(migrations) => migrations,
                    Err(error) => {
                        database.close();
                        return Err(error);
                    }
                };
                if let Err(source) = database.run_pending_migrations(migrations).await {
                    database.close();
                    return Err(database_framework_error(
                        MADS100,
                        "database bootstrap failed",
                        "database startup migration failed",
                        source,
                    ));
                }
            }

            Ok(())
        })
    }

    fn stop<'a>(&'a self, context: &'a ApplicationContext) -> LifecycleFuture<'a> {
        Box::pin(async move {
            context.resolve::<Database>()?.close();
            Ok(())
        })
    }
}

pub(super) fn database_framework_error(
    code: mads_core::DiagnosticCode,
    title: &'static str,
    operation: &'static str,
    source: DatabaseError,
) -> Error {
    Error::with_source(
        Diagnostic::new(code, title, operation),
        DatabaseCoreSource::from(source),
    )
}

/// Retains the core-safe parts of a database error for core error sources.
///
/// `deadpool_diesel::InteractError` deliberately stores a `Send` panic payload
/// that is not `Sync`. Core error sources are `Send + Sync`, so this type
/// retains every other variant's safe underlying source and omits only that
/// interaction payload.
enum DatabaseCoreSource {
    Configuration {
        message: String,
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    Pool(deadpool_diesel::postgres::PoolError),
    Query(diesel::result::Error),
    Migration(Box<dyn std::error::Error + Send + Sync>),
    Interaction,
}

impl From<DatabaseError> for DatabaseCoreSource {
    fn from(source: DatabaseError) -> Self {
        match source {
            DatabaseError::Configuration { message, source } => {
                Self::Configuration { message, source }
            }
            DatabaseError::Pool(source) => Self::Pool(source),
            DatabaseError::Interaction(_) => Self::Interaction,
            DatabaseError::Query(source) => Self::Query(source),
            DatabaseError::Migration(source) => Self::Migration(source),
        }
    }
}

impl DatabaseCoreSource {
    const fn kind(&self) -> DatabaseErrorKind {
        match self {
            Self::Configuration { .. } => DatabaseErrorKind::Configuration,
            Self::Pool(_) => DatabaseErrorKind::Pool,
            Self::Query(_) => DatabaseErrorKind::Query,
            Self::Migration(_) => DatabaseErrorKind::Migration,
            Self::Interaction => DatabaseErrorKind::Interaction,
        }
    }
}

impl fmt::Display for DatabaseCoreSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration { message, .. } => {
                write!(formatter, "database configuration is invalid: {message}")
            }
            Self::Pool(_) => formatter.write_str("database connection could not be acquired"),
            Self::Query(_) => formatter.write_str("database query failed"),
            Self::Migration(_) => formatter.write_str("database migration failed"),
            Self::Interaction => formatter.write_str("database blocking operation failed"),
        }
    }
}

impl fmt::Debug for DatabaseCoreSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DatabaseCoreSource")
            .field("kind", &self.kind())
            .finish()
    }
}

impl std::error::Error for DatabaseCoreSource {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Configuration { source, .. } => source
                .as_deref()
                .map(|source| source as &(dyn std::error::Error + 'static)),
            Self::Pool(source) => Some(source),
            Self::Query(source) => Some(source),
            Self::Migration(source) => Some(source.as_ref()),
            Self::Interaction => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error as StdError;

    use super::*;

    #[test]
    fn core_error_preserves_safe_configuration_cause() {
        let error = database_framework_error(
            MADS100,
            "database bootstrap failed",
            "database configuration failed",
            DatabaseError::Configuration {
                message: "database.url is invalid".to_owned(),
                source: Some(Box::new(std::io::Error::other("configuration cause"))),
            },
        );

        let database_source = StdError::source(&error).unwrap();
        assert_eq!(
            database_source.to_string(),
            "database configuration is invalid: database.url is invalid"
        );
        assert_eq!(
            StdError::source(database_source).unwrap().to_string(),
            "configuration cause"
        );
    }

    #[test]
    fn core_error_preserves_query_cause() {
        let error = database_framework_error(
            MADS100,
            "database bootstrap failed",
            "database query failed",
            DatabaseError::Query(diesel::result::Error::NotFound),
        );

        let database_source = StdError::source(&error).unwrap();
        assert_eq!(database_source.to_string(), "database query failed");
        assert!(
            StdError::source(database_source)
                .unwrap()
                .downcast_ref::<diesel::result::Error>()
                .is_some()
        );
    }

    #[test]
    fn core_error_preserves_migration_cause() {
        let error = database_framework_error(
            MADS100,
            "database bootstrap failed",
            "database migration failed",
            DatabaseError::Migration(Box::new(std::io::Error::other("migration cause"))),
        );

        let database_source = StdError::source(&error).unwrap();
        assert_eq!(database_source.to_string(), "database migration failed");
        assert_eq!(
            StdError::source(database_source).unwrap().to_string(),
            "migration cause"
        );
    }

    #[tokio::test]
    async fn core_error_preserves_pool_cause() {
        let database =
            Database::from_config(&DatabaseConfig::new("postgres://localhost/mads").unwrap())
                .unwrap();
        database.close();
        let database_error = database.check().await.unwrap_err();

        assert_eq!(database_error.kind(), DatabaseErrorKind::Pool);
        let error = database_framework_error(
            MADS100,
            "database bootstrap failed",
            "database readiness check failed",
            database_error,
        );
        let database_source = StdError::source(&error).unwrap();
        assert_eq!(
            database_source.to_string(),
            "database connection could not be acquired"
        );
        assert!(StdError::source(database_source).is_some());
    }

    #[test]
    fn interaction_payload_is_redacted_from_core_error_sources() {
        let secret = "mads-lifecycle-interaction-secret";
        let error = database_framework_error(
            MADS100,
            "database bootstrap failed",
            "database query failed",
            DatabaseError::Interaction(deadpool_diesel::InteractError::Panic(Box::new(
                secret.to_owned(),
            ))),
        );

        let database_source = StdError::source(&error).unwrap();
        assert_eq!(
            database_source.to_string(),
            "database blocking operation failed"
        );
        assert!(StdError::source(database_source).is_none());
        let output = format!("{error}\n{error:?}\n{database_source}\n{database_source:?}");
        assert!(!output.contains(secret));
    }
}
