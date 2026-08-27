use mads::common::*;

struct UserPrincipal;

#[routes]
#[guard(
    strategy = "jwt",
    strategy = "jwt-refresh",
    principal = UserPrincipal,
)]
trait UserRoutes {
    #[get("/profile")]
    async fn profile(&self);
}

fn main() {}
