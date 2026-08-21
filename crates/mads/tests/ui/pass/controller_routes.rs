//! Confirms supported route traits and managed controllers compile.

#![deny(missing_docs)]

use mads::common::{Json, Path};

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
    async fn get_user(&self, id: Path<i64>) -> String;

    /// Compile-time-disabled route used to verify `cfg` propagation.
    #[cfg(any())]
    #[mads::get("/cfg-disabled")]
    async fn cfg_disabled(&self);

    /// Compile-time-disabled route used to verify `cfg_attr` propagation.
    #[cfg_attr(all(), cfg(any()))]
    #[mads::get("/cfg-attr-disabled")]
    async fn cfg_attr_disabled(&self);
}

/// Command route contract.
#[mads::routes]
pub trait CommandRoutes {
    /// Creates one user.
    #[mads::post("/users")]
    async fn create_user(&self, id: Json<i64>) -> String;

    /// Updates one user.
    #[mads::put("/users/:id")]
    async fn update_user(&self, id: Path<i64>) -> String;

    /// Patches one user.
    #[mads::patch("/users/:id")]
    async fn patch_user(&self, id: Path<i64>) -> String;

    /// Deletes one user.
    #[mads::delete("/users/:id")]
    async fn delete_user(&self, id: Path<i64>) -> String;
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
    async fn get_user(&self, Path(id): Path<i64>) -> String {
        let _query = &self.query;
        id.to_string()
    }
}

impl CommandRoutes for UserController {
    async fn create_user(&self, Json(id): Json<i64>) -> String {
        let _command = &self.command;
        id.to_string()
    }

    async fn update_user(&self, Path(id): Path<i64>) -> String {
        id.to_string()
    }

    async fn patch_user(&self, Path(id): Path<i64>) -> String {
        id.to_string()
    }

    async fn delete_user(&self, Path(id): Path<i64>) -> String {
        id.to_string()
    }
}

fn main() {}
