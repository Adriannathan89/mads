use mads::common::*;

struct UserPrincipal;

impl PassportPrincipal for UserPrincipal {
    fn has_role(&self, _role: &str) -> bool {
        false
    }

    fn has_permission(&self, _permission: &str) -> bool {
        false
    }
}

#[routes]
#[guard(strategy = "jwt", principal = UserPrincipal, roles(any = []))]
trait UserRoutes {
    #[get("/profile")]
    async fn profile(&self);
}

fn main() {}
