//! Confirms supported route traits and managed controllers compile.

#![deny(missing_docs)]

/// Query application service.
#[mads::service]
pub struct QueryUsecase;

/// Command application service.
#[mads::service]
pub struct CommandUsecase;

/// Query route contract.
#[mads::routes(prefix = "/users")]
pub trait QueryRoutes {
    /// Gets one user.
    #[mads::get("/:id")]
    async fn get_user(&self, id: i64) -> i64;
}

/// Command route contract.
#[mads::routes]
pub trait CommandRoutes {
    /// Creates one user.
    #[mads::post("/users")]
    async fn create_user(&self, id: i64) -> i64;

    /// Updates one user.
    #[mads::put("/users/:id")]
    async fn update_user(&self, id: i64) -> i64;

    /// Patches one user.
    #[mads::patch("/users/:id")]
    async fn patch_user(&self, id: i64) -> i64;

    /// Deletes one user.
    #[mads::delete("/users/:id")]
    async fn delete_user(&self, id: i64) -> i64;
}

/// Controller with multiple managed dependencies and route contracts.
#[allow(dead_code)]
#[mads::controller(routes = [QueryRoutes, CommandRoutes])]
pub struct UserController {
    /// Query behavior.
    query: QueryUsecase,
    /// Command behavior.
    command: CommandUsecase,
}

impl QueryRoutes for UserController {
    async fn get_user(&self, id: i64) -> i64 {
        let _query = &self.query;
        id
    }
}

impl CommandRoutes for UserController {
    async fn create_user(&self, id: i64) -> i64 {
        let _command = &self.command;
        id
    }

    async fn update_user(&self, id: i64) -> i64 {
        id
    }

    async fn patch_user(&self, id: i64) -> i64 {
        id
    }

    async fn delete_user(&self, id: i64) -> i64 {
        id
    }
}

fn main() {}
