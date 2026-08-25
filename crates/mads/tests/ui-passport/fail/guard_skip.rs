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
trait UserRoutes {
    #[get("/profile")]
    #[guard(skip)]
    async fn profile(&self);
}

fn main() {}
