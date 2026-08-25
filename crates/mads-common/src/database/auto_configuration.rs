//! Official Diesel database auto-configuration conditions and provisioning.

use std::any::TypeId;

use mads_core::__private::{
    AutoConfigurationApplyContext, AutoConfigurationContext, AutoConfigurationContribution,
    AutoConfigurationDescriptor, AutoConfigurationEvaluation,
};
use mads_core::{
    AutoConfigurationConfigEvidence, AutoConfigurationReasonCode, Diagnostic, Error, Result,
    SourceLocation,
};

use super::lifecycle::{DatabaseLifecycle, DatabaseMigrations, database_framework_error};
use super::{Database, DatabaseConfig, MADS101};

/// Stable identifier for the official Diesel database auto-configuration.
pub(super) const DATABASE_AUTO_CONFIGURATION_ID: &str = "mads.common.database.diesel";

const DATABASE_TYPE_NAME: &str = "mads_common::database::pool::Database";

fn database_type_id() -> TypeId {
    TypeId::of::<Database>()
}

fn evaluate(context: &AutoConfigurationContext<'_>) -> AutoConfigurationEvaluation {
    if context.has_provider::<Database>()
        || mads_core::Catalog::providers()
            .iter()
            .any(|provider| provider.type_id() == database_type_id())
    {
        return AutoConfigurationEvaluation::overridden(
            AutoConfigurationReasonCode::new("user_override"),
            "an application provider overrides the database default",
            context.requirements::<Database>(),
            Vec::new(),
        );
    }

    let requirements = context.requirements::<Database>();
    if requirements.is_empty() {
        return AutoConfigurationEvaluation::skipped(
            AutoConfigurationReasonCode::new("requirement_absent"),
            "no provider requires the database default",
            requirements,
            configuration_evidence(context),
        );
    }

    let configuration = configuration_evidence(context);
    let missing_url = context.config().source_of("database.url").is_none();
    let config = match DatabaseConfig::from_config(context.config()) {
        Ok(config) => config,
        Err(source) => {
            let reason = if missing_url {
                "missing_configuration"
            } else {
                "invalid_configuration"
            };
            return AutoConfigurationEvaluation::failed(
                AutoConfigurationReasonCode::new(reason),
                "database auto-configuration could not read its configuration",
                requirements,
                configuration,
                database_framework_error(
                    MADS101,
                    "database auto-configuration is invalid",
                    "database configuration failed",
                    source,
                ),
            );
        }
    };

    if config.migrate_on_startup() && context.input::<DatabaseMigrations>().is_none() {
        return AutoConfigurationEvaluation::failed(
            AutoConfigurationReasonCode::new("missing_migration_source"),
            "database startup migrations require an embedded source",
            requirements,
            configuration,
            Error::new(Diagnostic::new(
                MADS101,
                "database auto-configuration is invalid",
                "database.migrate requires an embedded migration source",
            )),
        );
    }

    AutoConfigurationEvaluation::active(
        AutoConfigurationReasonCode::new("conditions_matched"),
        "Database is required and configured",
        requirements,
        configuration,
    )
}

fn apply(context: &AutoConfigurationApplyContext<'_>) -> Result<AutoConfigurationContribution> {
    let config = DatabaseConfig::from_config(context.config()).map_err(provisioning_error)?;
    let database = Database::from_config(&config).map_err(provisioning_error)?;
    let migrations = context.input::<DatabaseMigrations>().cloned();

    Ok(
        AutoConfigurationContribution::new(database).with_lifecycle_hook(DatabaseLifecycle::new(
            config.migrate_on_startup(),
            migrations,
        )),
    )
}

fn configuration_evidence(
    context: &AutoConfigurationContext<'_>,
) -> Vec<AutoConfigurationConfigEvidence> {
    ["database.url", "database.pool_size", "database.migrate"]
        .into_iter()
        .map(|key| AutoConfigurationConfigEvidence::new(key, context.config().source_of(key)))
        .collect()
}

fn provisioning_error(source: super::DatabaseError) -> Error {
    database_framework_error(
        MADS101,
        "database auto-configuration failed",
        "provisioning_failed",
        source,
    )
}

mads_core::__private::inventory::submit! {
    AutoConfigurationDescriptor::new(
        DATABASE_AUTO_CONFIGURATION_ID,
        DATABASE_TYPE_NAME,
        database_type_id,
        SourceLocation::new(file!(), line!(), column!()),
        evaluate,
        apply,
    )
}
