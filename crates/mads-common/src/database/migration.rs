//! Diesel migration execution and deterministic status reports.

use std::collections::BTreeSet;

use super::{Database, DatabaseResult};
use diesel::{migration::MigrationSource, pg::Pg};

/// A deterministic list of migration versions affected by an operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationReport {
    versions: Vec<String>,
}

impl MigrationReport {
    /// Creates a report from migration versions in sorted, deduplicated order.
    #[doc(hidden)]
    pub fn from_versions(versions: impl IntoIterator<Item = String>) -> Self {
        Self {
            versions: normalize_versions(versions),
        }
    }

    /// Returns the affected migration versions in lexical order.
    pub fn versions(&self) -> &[String] {
        &self.versions
    }

    /// Returns whether no migrations were affected.
    pub fn is_empty(&self) -> bool {
        self.versions.is_empty()
    }
}

/// A deterministic snapshot of migration versions for one source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationStatus {
    applied: Vec<String>,
    pending: Vec<String>,
}

impl MigrationStatus {
    /// Creates a status from applied and pending versions in deterministic order.
    #[doc(hidden)]
    pub fn from_versions(
        applied: impl IntoIterator<Item = String>,
        pending: impl IntoIterator<Item = String>,
    ) -> Self {
        Self {
            applied: normalize_versions(applied),
            pending: normalize_versions(pending),
        }
    }

    /// Returns source migrations already applied to the database in lexical order.
    pub fn applied(&self) -> &[String] {
        &self.applied
    }

    /// Returns source migrations not yet applied to the database in lexical order.
    pub fn pending(&self) -> &[String] {
        &self.pending
    }
}

impl Database {
    /// Applies every pending migration from one migration source.
    ///
    /// # Errors
    ///
    /// Returns a normalized pool, interaction, or migration error. The source
    /// is loaded and every harness operation executes on one pooled connection.
    pub async fn run_pending_migrations<S>(&self, source: S) -> DatabaseResult<MigrationReport>
    where
        S: MigrationSource<Pg> + Send + 'static,
    {
        self.run_migration(move |connection| {
            use diesel_migrations::MigrationHarness;

            let versions = connection
                .run_pending_migrations(source)?
                .into_iter()
                .map(|version| version.to_string())
                .collect::<Vec<_>>();

            Ok(MigrationReport::from_versions(versions))
        })
        .await
    }

    /// Reverts the lexically greatest applied migration from one source.
    ///
    /// This deliberately does not use Diesel's convenience rollback method:
    /// another source may own the database-wide latest migration.
    ///
    /// # Errors
    ///
    /// Returns a normalized pool, interaction, or migration error. In
    /// particular, returns Diesel's `NoMigrationRun` error when this source has
    /// no applied migrations.
    pub async fn revert_last_migration<S>(&self, source: S) -> DatabaseResult<MigrationReport>
    where
        S: MigrationSource<Pg> + Send + 'static,
    {
        self.run_migration(move |connection| {
            use diesel_migrations::MigrationHarness;

            let applied = connection
                .applied_migrations()?
                .into_iter()
                .map(|version| version.to_string())
                .collect::<BTreeSet<_>>();
            let migration = source
                .migrations()?
                .into_iter()
                .filter(|migration| applied.contains(&migration.name().version().to_string()))
                .max_by_key(|migration| migration.name().version().to_string())
                .ok_or_else(|| {
                    Box::new(diesel_migrations::MigrationError::NoMigrationRun)
                        as Box<dyn std::error::Error + Send + Sync>
                })?;
            let version = connection.revert_migration(migration.as_ref())?;

            Ok(MigrationReport::from_versions([version.to_string()]))
        })
        .await
    }

    /// Returns applied and pending migrations for one migration source.
    ///
    /// Applied migrations owned by other sources are excluded. This method does
    /// not execute, apply, or revert migrations.
    ///
    /// # Errors
    ///
    /// Returns a normalized pool, interaction, or migration error.
    pub async fn migration_status<S>(&self, source: S) -> DatabaseResult<MigrationStatus>
    where
        S: MigrationSource<Pg> + Send + 'static,
    {
        self.run_migration(move |connection| {
            use diesel_migrations::MigrationHarness;

            let source_versions = source
                .migrations()?
                .into_iter()
                .map(|migration| migration.name().version().to_string())
                .collect::<BTreeSet<_>>();
            let applied = connection
                .applied_migrations()?
                .into_iter()
                .map(|version| version.to_string())
                .filter(|version| source_versions.contains(version))
                .collect::<BTreeSet<_>>();
            let pending = source_versions
                .difference(&applied)
                .cloned()
                .collect::<Vec<_>>();

            Ok(MigrationStatus::from_versions(applied, pending))
        })
        .await
    }
}

fn normalize_versions(versions: impl IntoIterator<Item = String>) -> Vec<String> {
    versions
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
