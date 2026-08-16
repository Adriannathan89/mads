//! Confirms supported module and managed-provider declarations compile.

#![deny(missing_docs)]

/// Application module used by the compile fixture.
#[mads::module]
pub struct AppModule;

/// Dependency-free repository used by the service.
#[mads::repository]
pub struct UserRepository;

/// Service whose documented public dependency is preserved on its inner value.
#[mads::service]
pub struct UserService {
    /// Repository used by service methods.
    pub repository: UserRepository,
}

impl UserService {
    fn repository(&self) -> &UserRepository {
        &self.repository
    }
}

fn main() {
    let _method: fn(&UserService) -> &UserRepository = UserService::repository;
}
