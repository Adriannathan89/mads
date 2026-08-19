#[mads::routes]
trait Route {
    #[mads::get("/")]
    async fn index(&self);
}

#[mads::controller(routes = [Route])]
struct Controller<T> {
    value: T,
}

fn main() {}
