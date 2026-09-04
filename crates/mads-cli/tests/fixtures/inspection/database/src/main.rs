use mads::prelude::*;

#[derive(Clone)]
struct DatabaseConsumer;

#[mads::provider]
fn database_consumer(_database: Database) -> DatabaseConsumer {
    if let Ok(marker) = std::env::var("MADS_TEST_CONSTRUCTION_MARKER") {
        let _ = std::fs::write(marker, "constructed");
    }
    DatabaseConsumer
}

#[mads::routes(prefix = "/database")]
trait DatabaseRoutes {
    #[mads::get("/health")]
    async fn health(&self) -> &'static str;
}

#[mads::controller(routes = [DatabaseRoutes])]
struct DatabaseController {
    _database: DatabaseConsumer,
}

impl DatabaseRoutes for DatabaseController {
    async fn health(&self) -> &'static str {
        "healthy"
    }
}

#[mads::module]
struct AppModule;

#[mads::main]
async fn main() -> Result<(), HttpRuntimeError> {
    Mads::run::<AppModule>().await
}
