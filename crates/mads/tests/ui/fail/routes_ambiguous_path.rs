#[mads::routes(prefix = "/users/")]
trait Routes {
    #[mads::get("//:id")]
    async fn get_user(&self);
}

fn main() {}
