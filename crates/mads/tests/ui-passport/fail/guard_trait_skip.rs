use mads::common::*;

#[routes]
#[guard(skip)]
trait UserRoutes {
    #[get("/profile")]
    async fn profile(&self);
}

fn main() {}
