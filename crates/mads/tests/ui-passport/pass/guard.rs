use mads::common::*;

struct UserPrincipal;

impl PassportPrincipal for UserPrincipal {
    fn has_role(&self, _role: &str) -> bool {
        true
    }

    fn has_permission(&self, _permission: &str) -> bool {
        true
    }
}

fn owns_profile(_: &UserPrincipal) -> bool {
    true
}

fn may_read(_: &UserPrincipal) -> bool {
    true
}

#[derive(serde::Deserialize)]
struct UserClaims;

impl PassportPrincipal for UserClaims {
    fn has_role(&self, _role: &str) -> bool {
        true
    }

    fn has_permission(&self, _permission: &str) -> bool {
        true
    }
}

#[routes(prefix = "/users")]
#[guard(
    strategy = "jwt",
    principal = UserPrincipal,
    source = cookie("access_token"),
    roles(any = ["user", "admin"]),
    permissions(all = ["profile:base"]),
)]
trait UserRoutes {
    #[get("/profile")]
    #[guard(
        strategy = "jwt-refresh",
        principal = UserPrincipal,
        source = bearer,
        roles(all = ["member"]),
        permissions(any = ["profile:read"]),
        predicate = owns_profile,
    )]
    async fn profile(&self);

    #[post("/login")]
    #[guard(skip)]
    async fn login(&self);
}

#[routes]
trait MethodOnlyRoutes {
    #[get("/method-only")]
    #[guard(
        strategy = "jwt",
        principal = UserPrincipal,
        predicates = [owns_profile, may_read],
    )]
    async fn method_only(&self);
}

#[routes]
#[guard(strategy = "jwt", principal = ClaimsPrincipal<UserClaims>)]
trait BuiltinJwtRoutes {
    #[get("/builtin")]
    async fn builtin(&self);
}

fn main() {}
