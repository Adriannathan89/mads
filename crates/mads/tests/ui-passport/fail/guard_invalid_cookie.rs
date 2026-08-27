use mads::common::*;

struct UserPrincipal;

#[routes]
#[guard(
    strategy = "jwt",
    principal = UserPrincipal,
    source = cookie("bad;name"),
)]
trait UserRoutes {
    #[get("/profile")]
    async fn profile(&self);
}

fn main() {}
