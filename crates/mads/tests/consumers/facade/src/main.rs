//! Verifies attribute expansion through a facade-only dependency.

#[mads::module]
struct AppModule;

#[mads::repository]
struct Repository;

#[mads::routes]
trait Routes {
    #[mads::get("/")]
    async fn index(&self);
}

#[mads::controller(routes = [Routes])]
struct Controller;

impl Routes for Controller {
    async fn index(&self) {}
}

fn main() {}
