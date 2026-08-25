//! Verifies main expansion through a renamed facade-only dependency.

use framework::prelude::*;

#[repository]
struct MainRepository {
    database: Database,
}

fn consume_repository(repository: &MainRepository) {
    let _ = &repository.database;
}

#[framework::main]
async fn main() {
    let config = framework::core::ConfigBuilder::new()
        .source(framework::core::MapSource::new(
            "consumer",
            [("database.url", "postgres://localhost/renamed")],
        ))
        .build()
        .unwrap();
    let analysis = Mads::builder_with_config(config).analyze();

    assert_eq!(
        analysis.auto_configurations()[0].status(),
        AutoConfigurationStatus::Active,
    );
    assert_eq!(
        analysis.graph().provider::<Database>().unwrap().origin(),
        ProviderOrigin::AutoConfiguration,
    );

    let _ = consume_repository;
}
