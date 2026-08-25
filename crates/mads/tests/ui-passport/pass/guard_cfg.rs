use mads::common::*;

#[cfg(any())]
struct DisabledClaims;

#[cfg(any())]
fn disabled_predicate(_: &ClaimsPrincipal<DisabledClaims>) -> bool {
    true
}

#[routes]
#[guard(
    strategy = "jwt",
    principal = ClaimsPrincipal<DisabledClaims>,
    predicate = disabled_predicate,
)]
trait ConditionallyGuardedRoutes {
    #[cfg(any())]
    #[get("/disabled")]
    async fn disabled(&self);
}

fn main() {}
