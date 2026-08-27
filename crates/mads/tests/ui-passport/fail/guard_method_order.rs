use mads::common::*;

struct UserPrincipal;

#[routes]
trait UserRoutes {
    #[guard(strategy = "jwt", principal = UserPrincipal)]
    #[get("/profile")]
    async fn profile(&self);
}

fn main() {}
