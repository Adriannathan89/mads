use mads::common::*;

struct UserPrincipal;

#[routes]
#[guard(strategy = "JWT", principal = UserPrincipal)]
trait UserRoutes {
    #[get("/profile")]
    async fn profile(&self);
}

fn main() {}
