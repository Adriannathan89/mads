#[mads::routes]
trait Route {
    #[mads::get("/")]
    async fn index(&self);
}

#[mads::controller(routes = [Route])]
struct Controller(usize);

fn main() {}
