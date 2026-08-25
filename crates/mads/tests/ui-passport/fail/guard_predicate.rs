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

fn wrong_predicate_argument(_: UserPrincipal) -> bool {
    true
}

fn wrong_predicate_return(_: &UserPrincipal) {}

#[routes]
trait UserRoutes {
    #[get("/argument")]
    #[guard(
        strategy = "jwt",
        principal = UserPrincipal,
        predicate = wrong_predicate_argument,
    )]
    async fn argument(&self);

    #[get("/return")]
    #[guard(
        strategy = "jwt",
        principal = UserPrincipal,
        predicate = wrong_predicate_return,
    )]
    async fn return_type(&self);
}

fn main() {}
