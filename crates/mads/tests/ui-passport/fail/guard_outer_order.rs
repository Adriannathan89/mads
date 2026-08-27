use mads::common::*;

struct UserPrincipal;

#[guard(strategy = "jwt", principal = UserPrincipal)]
#[routes]
trait UserRoutes {
    #[get("/profile")]
    async fn profile(&self);
}

fn main() {}
