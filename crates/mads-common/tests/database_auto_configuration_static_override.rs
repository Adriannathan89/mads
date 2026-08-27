//! Static provider precedence over the official Diesel auto-configuration.

#![cfg(feature = "database")]

use mads_common::{Database, DatabaseConfig};
use mads_core::{AutoConfigurationStatus, Mads, ProviderOrigin};

#[mads_core::repository]
struct StaticOverrideRepository {
    database: Database,
}

impl StaticOverrideRepository {
    fn database(&self) -> &Database {
        &self.database
    }
}

#[mads_core::provider]
fn custom_database() -> Database {
    Database::from_config(&DatabaseConfig::new("postgres://localhost/custom").unwrap()).unwrap()
}

#[tokio::test]
async fn static_database_provider_overrides_without_infrastructure_lifecycle_ownership() {
    let builder = Mads::builder();
    let analysis = builder.analyze();
    assert_eq!(
        analysis.auto_configurations()[0].status(),
        AutoConfigurationStatus::Overridden,
    );
    let mut application = builder.build().await.unwrap();

    assert_eq!(
        application.auto_configurations()[0].status(),
        AutoConfigurationStatus::Overridden,
    );
    assert_eq!(
        application.graph().provider::<Database>().unwrap().origin(),
        ProviderOrigin::Provider,
    );

    application.start().await.unwrap();
    application.shutdown().await.unwrap();

    let database = application.context().resolve::<Database>().unwrap();
    assert!(!database.is_closed());
    application
        .context()
        .resolve::<StaticOverrideRepository>()
        .unwrap()
        .database()
        .close();
}

#[tokio::test]
async fn explicitly_constructed_static_database_provider_remains_overridden() {
    let mut builder = Mads::builder();
    builder.construct::<Database>().await.unwrap();

    let analysis = builder.analyze();
    assert_eq!(
        analysis.auto_configurations()[0].status(),
        AutoConfigurationStatus::Overridden,
    );

    builder
        .construct::<StaticOverrideRepository>()
        .await
        .unwrap();
    let application = builder.build().await.unwrap();
    application.context().resolve::<Database>().unwrap().close();
}
