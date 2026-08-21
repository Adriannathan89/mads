//! Verifies attribute expansion through a facade-only dependency.

use mads::prelude::*;

#[module]
struct AppModule;

#[repository]
struct Repository;

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

async fn build_application() -> mads::core::Result<()> {
    let application = Mads::builder().build().await?;
    let _router = build_router(&application)?;
    Ok(())
}

fn main() {
    let _ = build_application;
}
