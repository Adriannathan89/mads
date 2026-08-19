#[mads::routes]
trait Routes {
    #[mads::get]
    async fn index(&self);
}

fn main() {}
