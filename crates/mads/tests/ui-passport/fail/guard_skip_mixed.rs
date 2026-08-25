use mads::common::*;

struct UserPrincipal;

#[routes]
#[guard(strategy = "jwt", principal = UserPrincipal)]
trait UserRoutes {
    #[get("/profile")]
    #[guard(skip, roles(any = ["user"]))]
    async fn profile(&self);
}

fn main() {}
