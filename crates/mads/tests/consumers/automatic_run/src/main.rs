use mads::prelude::*;

mod delivery {
    use mads::prelude::*;

    #[routes]
    pub trait HealthRoutes {
        #[get("/health")]
        async fn health(&self) -> &'static str;
    }

    #[controller(routes = [HealthRoutes])]
    pub struct HealthController;

    impl HealthRoutes for HealthController {
        async fn health(&self) -> &'static str {
            "ok"
        }
    }

    #[module]
    pub struct HealthHttpModule;
}

#[module(imports = [delivery::HealthHttpModule])]
struct AppModule;

#[mads::main]
async fn main() -> Result<(), HttpRuntimeError> {
    Mads::run::<AppModule>().await
}
