use mads::common::*;

struct UserPrincipal;

#[routes]
#[guard(
    strategy = "jwt",
    principal = UserPrincipal,
    roles(any = ["user"], all = ["admin"]),
)]
trait UserRoutes {
    #[get("/profile")]
    async fn profile(&self);
}

fn main() {}
