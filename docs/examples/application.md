# MADS.rs 0.3 User CRUD Example

This is a complete HTTP slice for the API available in v0.3. It uses an
in-memory controller to keep the example focused on typed routing, extraction,
responses, and the runtime. Persistence, Diesel, request validation, and
domain-error policy are deliberately not implied by this example.

## Dependencies

```toml
[package]
name = "user-api"
version = "0.1.0"
edition = "2024"

[dependencies]
mads = "0.3"
serde = { version = "1", features = ["derive"] }
```

## `src/main.rs`

```rust,ignore
use mads::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize)]
struct User {
    id: u64,
    name: String,
}

#[derive(Deserialize)]
struct CreateUser {
    name: String,
}

#[derive(Deserialize)]
struct UpdateUser {
    name: String,
}

#[derive(Deserialize)]
struct UserQuery {
    page: Option<u64>,
}

#[mads::routes(prefix = "/users")]
trait UserRoutes {
    #[mads::get("/")]
    async fn list_users(&self, query: Query<UserQuery>) -> Json<Vec<User>>;

    #[mads::get("/:id")]
    async fn get_user(&self, id: Path<u64>) -> HttpResult<Json<User>>;

    #[mads::post("/")]
    async fn create_user(&self, input: Json<CreateUser>) -> Created<Json<User>>;

    #[mads::put("/:id")]
    async fn replace_user(&self, id: Path<u64>, input: Json<UpdateUser>) -> Json<User>;

    #[mads::delete("/:id")]
    async fn delete_user(&self, id: Path<u64>) -> NoContent;
}

#[mads::controller(routes = [UserRoutes])]
struct UserController;

impl UserRoutes for UserController {
    async fn list_users(&self, Query(query): Query<UserQuery>) -> Json<Vec<User>> {
        let page = query.page.unwrap_or(1);
        Json(vec![User {
            id: page,
            name: "Ada".to_owned(),
        }])
    }

    async fn get_user(&self, Path(id): Path<u64>) -> HttpResult<Json<User>> {
        if id == 0 {
            return Err(HttpError::not_found("user was not found"));
        }
        Ok(Json(User { id, name: "Ada".to_owned() }))
    }

    async fn create_user(&self, Json(input): Json<CreateUser>) -> Created<Json<User>> {
        Created(Json(User { id: 1, name: input.name }))
    }

    async fn replace_user(&self, Path(id): Path<u64>, Json(input): Json<UpdateUser>) -> Json<User> {
        Json(User { id, name: input.name })
    }

    async fn delete_user(&self, Path(_id): Path<u64>) -> NoContent {
        NoContent
    }
}

#[mads::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let application = Mads::builder().build().await?;
    serve(application, "127.0.0.1:3000").await?;
    Ok(())
}
```

The controller is application-scoped. The generated registrar captures it
while building the router, so route methods have only their HTTP arguments and
no framework state parameter. `HttpResult` is for expected HTTP delivery
errors; construction and server bootstrap retain `mads::core::Result`-based
framework errors internally.

## Test it without a listener

`build_router` validates every route and returns an Axum router. Test the full
generated path in-process with Tower:

```rust,ignore
use axum::{body::Body, http::{Request, StatusCode}};
use mads::prelude::*;
use tower::ServiceExt;

let application = Mads::builder().build().await?;
let response = build_router(&application)?
    .oneshot(Request::builder().uri("/users/7").body(Body::empty())?)
    .await?;
assert_eq!(response.status(), StatusCode::OK);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Use `mads::common::axum` when a native extractor, response, router operation,
or Tower layer is preferable. MADS forwards these primitives rather than
wrapping their behavior.

## Runtime policy relevant to this API

MADS uses `/:id` declarations and translates only validated paths to Axum 0.8
syntax. GET also answers HEAD, OPTIONS is not synthesized, unsupported methods
use Axum's 405 and `Allow` behavior, static paths win over parameter paths,
and trailing slashes are strict: `/users` and `/users/` differ. Missing paths
use Axum's default 404 response. Invalid or conflicting catalog metadata
returns `MADS030` before a router is constructed or a listener is bound.
