use mads::{common::*, core};

#[derive(serde::Deserialize)]
struct UserClaims {
    user_id: u64,
}

struct UserPrincipal(u64);

impl PassportPrincipal for UserPrincipal {
    fn has_role(&self, _role: &str) -> bool {
        false
    }

    fn has_permission(&self, _permission: &str) -> bool {
        false
    }
}

#[core::service]
struct UserLookup;

#[core::service]
struct AppJwtStrategy {
    users: UserLookup,
}

#[passport_strategy(name = "jwt")]
impl PassportStrategy for AppJwtStrategy {
    type Claims = UserClaims;
    type Principal = UserPrincipal;
    const TOKEN_KIND: JwtTokenKind = JwtTokenKind::Access;

    async fn validate(
        &self,
        _context: &PassportContext<'_>,
        claims: &JwtClaims<Self::Claims>,
    ) -> PassportResult<Self::Principal> {
        let _ = &self.users;
        Ok(UserPrincipal(claims.custom.user_id))
    }
}

fn main() {}
