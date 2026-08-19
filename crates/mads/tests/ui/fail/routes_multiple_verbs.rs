#[mads::routes]
trait Routes {
    #[mads::get("/")]
    #[mads::post("/")]
    async fn index(&self);
}

fn main() {}
