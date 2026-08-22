//! Explicit database bootstrap and application lifecycle integration.

use std::{fmt, sync::Mutex};

use diesel_migrations::EmbeddedMigrations;
use mads_core::{
    ApplicationContext, Diagnostic, Error, LifecycleFuture, LifecycleHook, MadsBuilder, Result,
};

use super::{Database, DatabaseConfig, DatabaseError, DatabaseErrorKind, MADS100};

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
/// Database registration remains explicit in v0.4; v0.5 may add
/// auto-configuration, but this extension never detects requirements or backs
/// off from an existing [`Database`] provider.
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
}

impl MadsBuilderDatabaseExt for MadsBuilder {
    #[allow(clippy::result_large_err)]
    fn database(&mut self, bootstrap: DatabaseBootstrap) -> Result<&mut Self> {
        bootstrap.validate()?;
        let migrate_on_startup = bootstrap.config.migrate_on_startup();
        let database = Database::from_config(&bootstrap.config)
            .map_err(|source| database_core_error("database pool creation failed", source))?;
        let migrations = bootstrap.migrations;

        self.provide(database)?;
        self.lifecycle_hook(DatabaseLifecycle {
            migrate_on_startup,
            migrations: Mutex::new(migrations),
        });
        Ok(self)
    }
}

struct DatabaseLifecycle {
    migrate_on_startup: bool,
    migrations: Mutex<Option<EmbeddedMigrations>>,
}

impl DatabaseLifecycle {
    #[allow(clippy::result_large_err)]
    fn take_migrations(&self) -> Result<EmbeddedMigrations> {
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

impl LifecycleHook for DatabaseLifecycle {
    fn name(&self) -> &str {
        "database"
    }

    fn start<'a>(&'a self, context: &'a ApplicationContext) -> LifecycleFuture<'a> {
        Box::pin(async move {
            let database = context.resolve::<Database>()?;

            if let Err(source) = database.check().await {
                database.close();
                return Err(database_core_error(
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
                    return Err(database_core_error(
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

fn database_core_error(operation: &'static str, source: DatabaseError) -> Error {
    Error::with_source(
        Diagnostic::new(MADS100, "database bootstrap failed", operation),
        DatabaseCoreSource::new(source),
    )
}

/// Retains a redacted database-error summary for core error sources.
///
/// `deadpool_diesel::InteractError` deliberately stores a `Send` panic payload
/// that is not `Sync`. Core error sources are `Send + Sync`, so this adapter
/// preserves a stable kind and redacted display message without weakening
/// either public boundary or exposing that payload.
struct DatabaseCoreSource {
    kind: DatabaseErrorKind,
    message: String,
}

impl DatabaseCoreSource {
    fn new(source: DatabaseError) -> Self {
        Self {
            kind: source.kind(),
            message: source.to_string(),
        }
    }
}

impl fmt::Display for DatabaseCoreSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl fmt::Debug for DatabaseCoreSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DatabaseCoreSource")
            .field("kind", &self.kind)
            .finish()
    }
}

impl std::error::Error for DatabaseCoreSource {}
