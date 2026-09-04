use mads::prelude::*;

#[mads::routes]
trait FirstRoutes {
    #[mads::get("/duplicate")]
    async fn first(&self);
}

#[mads::routes]
trait SecondRoutes {
    #[mads::get("/duplicate")]
    async fn second(&self);
}

#[mads::controller(routes = [FirstRoutes, SecondRoutes])]
struct InvalidRoutesController;

impl FirstRoutes for InvalidRoutesController {
    async fn first(&self) {}
}

impl SecondRoutes for InvalidRoutesController {
    async fn second(&self) {}
}

#[mads::module]
struct InvalidRoutesModule;

#[mads::main]
async fn main() -> Result<(), HttpRuntimeError> {
    Mads::run::<InvalidRoutesModule>().await
}
