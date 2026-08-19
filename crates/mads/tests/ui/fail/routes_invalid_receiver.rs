#[mads::routes]
trait Routes {
    #[mads::get("/")]
    async fn index(&mut self);
}

fn main() {}
