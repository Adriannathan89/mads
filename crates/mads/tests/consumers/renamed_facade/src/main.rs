//! Verifies attribute expansion through a renamed facade-only dependency.

use framework::prelude::*;

#[module]
struct AppModule;

#[repository]
struct RenamedRepository {
    database: Database,
}

fn consume_repository(repository: &RenamedRepository) {
    let _ = &repository.database;
}

fn database_config() -> framework::common::DatabaseResult<DatabaseConfig> {
    DatabaseConfig::new("postgres://localhost/renamed")
}

fn diesel_backend(_: std::marker::PhantomData<framework::diesel::pg::Pg>) {}

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

async fn build_application() -> framework::core::Result<()> {
    let application = Mads::builder().build().await?;
    let _router = build_router(&application)?;
    Ok(())
}

fn main() {
    let _ = build_application;
    let _ = database_config;
    let _ = diesel_backend;
    let _ = consume_repository;
}
