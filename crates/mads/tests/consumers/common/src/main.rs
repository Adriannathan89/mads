//! Verifies controller expansion through a direct common dependency.

#[mads_common::routes]
trait Routes {
    #[mads_common::get("/")]
    async fn index(&self);
}

#[mads_common::controller(routes = [Routes])]
struct Controller;

impl Routes for Controller {
    async fn index(&self) {}
}

fn main() {}
