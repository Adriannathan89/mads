//! Verifies attribute expansion through a renamed facade-only dependency.

#[framework::module]
struct AppModule;

#[framework::repository]
struct Repository;

#[framework::routes]
trait Routes {
    #[framework::get("/")]
    async fn index(&self);
}

#[framework::controller(routes = [Routes])]
struct Controller;

impl Routes for Controller {
    async fn index(&self) {}
}

fn main() {}
