//! Database command execution for project-local Diesel migrations.

use std::{error::Error, fmt, path::Path};

use mads::{
    Database, DatabaseConfig, DatabaseError,
    core::{ConfigBuilder, DotenvSource, EnvSource, TomlSource},
    diesel_migrations::{FileBasedMigrations, MigrationError},
};

use crate::command::DatabaseCommand;

/// A CLI-safe failure which retains its original cause for programmatic inspection.
pub(crate) struct CliError {
    message: String,
    source: Box<dyn Error>,
}

impl CliError {
    fn from_error(message: impl Into<String>, source: impl Error + 'static) -> Self {
        Self {
            message: message.into(),
            source: Box::new(source),
        }
    }

    fn database(error: DatabaseError) -> Self {
        if contains_no_migration(&error) {
            return Self::from_error("no migration is available to revert", error);
        }
        Self::from_error(error.to_string(), error)
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl fmt::Debug for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CliError")
            .field("message", &self.message)
            .field("source", &"[REDACTED]")
            .finish()
    }
}

impl Error for CliError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}

/// Loads one project and executes its requested database operation.
pub(crate) async fn execute(
    command: DatabaseCommand,
    root: &Path,
) -> Result<Vec<String>, CliError> {
    let config = ConfigBuilder::new()
        .dotenv(DotenvSource::optional(root.join(".env")))
        .source(TomlSource::file(root.join("mads.toml")))
        .source(EnvSource::new("MADS_"))
        .build()
        .map_err(|error| CliError::from_error(error.to_string(), error))?;
    let database_config = DatabaseConfig::from_config(&config)
        .map_err(|error| CliError::from_error(error.to_string(), error))?;
    let migrations = FileBasedMigrations::from_path(root.join("migrations")).map_err(|error| {
        CliError::from_error(
            format!(
                "migration directory `{}` could not be loaded",
                root.join("migrations").display()
            ),
            error,
        )
    })?;
    let database = Database::from_config(&database_config).map_err(CliError::database)?;

    let result = match command {
        DatabaseCommand::Migrate => {
            database
                .run_pending_migrations(migrations)
                .await
                .map(|report| {
                    if report.is_empty() {
                        vec!["database is up to date".to_owned()]
                    } else {
                        report
                            .versions()
                            .iter()
                            .map(|version| format!("applied {version}"))
                            .collect()
                    }
                })
        }
        DatabaseCommand::Rollback => {
            database
                .revert_last_migration(migrations)
                .await
                .map(|report| {
                    report
                        .versions()
                        .iter()
                        .map(|version| format!("reverted {version}"))
                        .collect()
                })
        }
        DatabaseCommand::Status => database.migration_status(migrations).await.map(|status| {
            let mut lines = status
                .applied()
                .iter()
                .map(|version| format!("applied {version}"))
                .collect::<Vec<_>>();
            lines.extend(
                status
                    .pending()
                    .iter()
                    .map(|version| format!("pending {version}")),
            );
            lines.push(format!(
                "summary: {} applied, {} pending",
                status.applied().len(),
                status.pending().len()
            ));
            lines
        }),
        DatabaseCommand::Help => Ok(Vec::new()),
    };
    database.close();

    result.map_err(CliError::database)
}

fn contains_no_migration(error: &DatabaseError) -> bool {
    let mut current = error.source();
    while let Some(source) = current {
        if matches!(
            source.downcast_ref::<MigrationError>(),
            Some(MigrationError::NoMigrationRun)
        ) {
            return true;
        }
        current = source.source();
    }
    false
}
