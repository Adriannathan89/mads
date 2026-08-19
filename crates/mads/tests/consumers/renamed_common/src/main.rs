//! Verifies controller expansion through a renamed common dependency.

#[web::routes]
trait Routes {
    #[web::get("/")]
    async fn index(&self);
}

#[web::controller(routes = [Routes])]
struct Controller;

impl Routes for Controller {
    async fn index(&self) {}
}

fn main() {}
