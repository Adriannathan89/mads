#[mads::routes]
trait Route {
    #[mads::get("/")]
    async fn index(&self);
}

#[mads::controller(routes = [Route, Route])]
struct Controller;

fn main() {}
