//! Verifies main expansion through a facade-only dependency.

use mads::prelude::*;

#[repository]
struct MainRepository {
    database: Database,
}

fn consume_repository(repository: &MainRepository) {
    let _ = &repository.database;
}

#[mads::main]
async fn main() {
    let config = mads::core::ConfigBuilder::new()
        .source(mads::core::MapSource::new(
            "consumer",
            [("database.url", "postgres://localhost/mads")],
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
