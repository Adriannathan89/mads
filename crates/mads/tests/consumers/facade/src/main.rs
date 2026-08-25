//! Verifies attribute expansion through a facade-only dependency.

use mads::prelude::*;

#[module]
struct AppModule;

#[repository]
struct Repository {
    database: Database,
}

#[routes]
trait Routes {
    #[get("/")]
    async fn index(&self);
}

#[controller(routes = [Routes])]
struct Controller;

impl Routes for Controller {
    async fn index(&self) {}
}

fn inspect_auto_configuration() {
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
}

fn main() {
    let _ = inspect_auto_configuration;
}
