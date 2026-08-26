use mads::prelude::*;

struct User;

impl PassportPrincipal for User {
    fn has_role(&self, _role: &str) -> bool {
        false
    }

    fn has_permission(&self, _permission: &str) -> bool {
        false
    }
}

#[routes]
#[guard(
    strategy = "jwt-refresh",
    principal = User,
    source = cookie("refresh_token"),
)]
trait ProtectedRoutes {
    #[get("/")]
    async fn protected(&self);
}

fn main() {}
