//! Database command execution for project-local Diesel migrations.

use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

use mads::{
    Database, DatabaseConfig, DatabaseError,
    core::{ConfigBuilder, DotenvSource, EnvSource, TomlSource},
    diesel_migrations::{FileBasedMigrations, MigrationError},
};

use crate::command::DatabaseCommand;

#[allow(dead_code)]
mod schema;

/// A database-enabled project whose migration source is loaded on demand.
pub(crate) struct LoadedDatabaseProject {
    root: PathBuf,
    database: DatabaseConfig,
}

impl LoadedDatabaseProject {
    /// Returns the selected package root.
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    /// Opens the configured database connection boundary.
    pub(crate) fn connect(&self) -> Result<Database, CliError> {
        Database::from_config(&self.database).map_err(CliError::database)
    }

    /// Loads the selected package's file-based migration source.
    pub(crate) fn migrations(&self) -> Result<FileBasedMigrations, CliError> {
        let path = self.root().join("migrations");
        FileBasedMigrations::from_path(&path).map_err(|error| {
            CliError::from_error(
                format!(
                    "migration directory `{}` could not be loaded",
                    path.display()
                ),
                error,
            )
        })
    }
}

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

/// Loads database configuration for the selected package root.
pub(crate) fn load_project(root: &Path) -> Result<LoadedDatabaseProject, CliError> {
    let config = ConfigBuilder::new()
        .dotenv(DotenvSource::optional(root.join(".env")))
        .source(TomlSource::file(root.join("mads.toml")))
        .source(EnvSource::new("MADS_"))
        .build()
        .map_err(|error| CliError::from_error(error.to_string(), error))?;
    let database = DatabaseConfig::from_config(&config)
        .map_err(|error| CliError::from_error(error.to_string(), error))?;

    Ok(LoadedDatabaseProject {
        root: root.to_owned(),
        database,
    })
}

/// Loads one project and executes its requested database operation.
pub(crate) async fn execute(
    command: DatabaseCommand,
    root: &Path,
) -> Result<Vec<String>, CliError> {
    let project = load_project(root)?;
    let migrations = match command {
        DatabaseCommand::Migrate | DatabaseCommand::Rollback | DatabaseCommand::Status => {
            Some(project.migrations()?)
        }
        DatabaseCommand::Help => None,
    };
    let database = project.connect()?;

    let result = match command {
        DatabaseCommand::Migrate => database
            .run_pending_migrations(migrations.expect("migrate should load migrations"))
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
            }),
        DatabaseCommand::Rollback => database
            .revert_last_migration(migrations.expect("rollback should load migrations"))
            .await
            .map(|report| {
                report
                    .versions()
                    .iter()
                    .map(|version| format!("reverted {version}"))
                    .collect()
            }),
        DatabaseCommand::Status => database
            .migration_status(migrations.expect("status should load migrations"))
            .await
            .map(|status| {
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

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::{TempDir, tempdir};

    use super::load_project;

    #[test]
    fn loading_a_database_project_does_not_require_a_migrations_directory() {
        let root = database_project_without_migrations();
        let project = load_project(root.path()).unwrap();

        assert_eq!(project.root(), root.path());
        assert!(project.migrations().is_err());
    }

    fn database_project_without_migrations() -> TempDir {
        let root = tempdir().expect("temporary project should be created");
        fs::write(
            root.path().join("mads.toml"),
            "[database]\nurl = \"postgres://localhost/mads\"\n",
        )
        .expect("project TOML should be written");
        root
    }
}
