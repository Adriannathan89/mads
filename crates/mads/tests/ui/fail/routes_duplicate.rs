#[mads::routes(prefix = "/users")]
trait Routes {
    #[mads::get("/:id")]
    async fn first(&self);

    #[mads::get("/:id")]
    async fn second(&self);
}

fn main() {}
