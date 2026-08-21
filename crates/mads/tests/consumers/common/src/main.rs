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

async fn build_application() -> mads_common::core::Result<()> {
    let application = mads_common::core::Mads::builder().build().await?;
    let _router = mads_common::build_router(&application)?;
    Ok(())
}

fn main() {
    let _ = build_application;
}
