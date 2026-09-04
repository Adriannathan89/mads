use mads::prelude::*;

#[mads::routes]
trait HealthRoutes {
    #[mads::get("/health")]
    async fn health(&self) -> &'static str;
}

#[mads::controller(routes = [HealthRoutes])]
struct HealthController;

impl HealthRoutes for HealthController {
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
