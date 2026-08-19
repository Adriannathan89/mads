#[mads::routes]
trait Routes {
    #[mads::get("/health\0check")]
    async fn health(&self);
}

fn main() {}
