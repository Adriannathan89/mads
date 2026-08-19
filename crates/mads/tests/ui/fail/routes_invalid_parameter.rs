#[mads::routes]
trait Routes {
    #[mads::get("/:9id")]
    async fn get_user(&self);
}

fn main() {}
