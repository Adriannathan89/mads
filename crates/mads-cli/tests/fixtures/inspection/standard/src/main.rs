use mads::prelude::*;

#[derive(Clone)]
struct Marker;

#[mads::provider]
fn marker_provider() -> Marker {
    if let Ok(marker) = std::env::var("MADS_TEST_CONSTRUCTION_MARKER") {
        let _ = std::fs::write(marker, "constructed");
    }
    Marker
}

#[mads::routes(prefix = "/users")]
trait UserRoutes {
    #[mads::get("/:id")]
    async fn get_user(&self) -> &'static str;

    #[mads::post("/")]
    async fn create_user(&self) -> &'static str;
}

#[mads::controller(routes = [UserRoutes])]
struct UserController {
    _marker: Marker,
}

impl UserRoutes for UserController {
    async fn get_user(&self) -> &'static str {
        "user"
    }

    async fn create_user(&self) -> &'static str {
        "created"
    }
}

#[mads::module]
struct AppModule;

#[mads::main]
async fn main() -> Result<(), HttpRuntimeError> {
    Mads::run::<AppModule>().await
}
