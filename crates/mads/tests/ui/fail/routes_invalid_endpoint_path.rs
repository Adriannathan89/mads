#[mads::routes]
trait Routes {
    #[mads::get("index")]
    async fn index(&self);
}

fn main() {}
