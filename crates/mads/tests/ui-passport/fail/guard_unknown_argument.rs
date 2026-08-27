use mads::common::*;

struct UserPrincipal;

#[routes]
#[guard(strategy = "jwt", principal = UserPrincipal, audience = "mads")]
trait UserRoutes {
    #[get("/profile")]
    async fn profile(&self);
}

fn main() {}
