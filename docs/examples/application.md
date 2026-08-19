# MADS.rs — Final CRUD User Application Example

This document illustrates what a complete User CRUD application should feel like when the final MADS vision is implemented.

The example intentionally shows **application-facing code**, not the internal code MADS would generate for Axum state, dependency construction, Diesel connection pooling, routing, or runtime startup.

---

## 1. Target Application

The API exposes:

```text
GET    /users
GET    /users/:id
POST   /users
PUT    /users/:id
DELETE /users/:id
```

Architecture:

```text
HTTP Route
    ↓
UserController
    ↓
UserService
    ↓
UserRepository
    ↓
Database
    ↓
Diesel
    ↓
PostgreSQL
```

MADS derives the dependency wiring from Rust types.

---

## 2. Project Structure

```text
user-api/
│
├── Cargo.toml
├── mads.toml
├── migrations/
│   └── 0001_create_users/
│       ├── up.sql
│       └── down.sql
│
└── src/
    ├── main.rs
    ├── app.rs
    └── users/
        ├── mod.rs
        ├── schema.rs
        ├── model.rs
        ├── input.rs
        ├── repository.rs
        ├── service.rs
        └── controller.rs
```

---

## 3. Cargo.toml

The final facade is intended to keep application dependencies small.

```toml
[package]
name = "user-api"
version = "0.1.0"
edition = "2024"

[dependencies]
mads = "0.1"
serde = { version = "1", features = ["derive"] }
```

Diesel, Axum, Tokio, and the framework integration can be provided through the MADS facade/default feature set for the common path.

Applications that need advanced direct Diesel or Axum APIs may still declare those crates explicitly.

---

## 4. Application Configuration

`mads.toml`:

```toml
[app]
name = "user-api"

[server]
host = "127.0.0.1"
port = 3000

[database]
url = "${DATABASE_URL}"
pool_size = 10
migrate = true

[logging]
level = "info"
```

Environment:

```bash
export DATABASE_URL="postgres://postgres:postgres@localhost/user_api"
```

MADS can infer:

```text
Database required by UserRepository
        +
Diesel capability available
        +
database.url configured
        ↓
DieselDatabaseAutoConfiguration
```

No pool creation is required in `main.rs`.

---

## 5. Migration

`migrations/0001_create_users/up.sql`:

```sql
CREATE TABLE users (
    id BIGSERIAL PRIMARY KEY,
    email VARCHAR(255) NOT NULL UNIQUE,
    name VARCHAR(120) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

`migrations/0001_create_users/down.sql`:

```sql
DROP TABLE users;
```

Run manually:

```bash
mads db migrate
```

or let startup migration run because:

```toml
[database]
migrate = true
```

---

## 6. Users Module

`src/users/mod.rs`:

```rust
use mads::prelude::*;

mod input;
mod model;
mod repository;
mod schema;
mod service;
mod controller;

pub use input::*;
pub use model::*;
pub use repository::*;
pub use service::*;
pub use controller::*;

#[module]
pub struct UserModule;
```

The module is an architecture boundary.

MADS associates the route/service/repository metadata belonging to this module without requiring a manual list such as:

```rust
services = [UserService]
repositories = [UserRepository]
routes = [UserRoutes]
```

---

## 7. Diesel Schema

`src/users/schema.rs`:

```rust
diesel::table! {
    users (id) {
        id -> Int8,
        email -> Varchar,
        name -> Varchar,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}
```

MADS does not replace Diesel schema/query functionality.

---

## 8. User Model

`src/users/model.rs`:

```rust
use mads::prelude::*;
use serde::Serialize;

use super::schema::users;

#[derive(Debug, Clone, Serialize, Queryable, Selectable)]
#[diesel(table_name = users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct User {
    pub id: i64,
    pub email: String,
    pub name: String,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Insertable)]
#[diesel(table_name = users)]
pub struct NewUser {
    pub email: String,
    pub name: String,
}

#[derive(AsChangeset)]
#[diesel(table_name = users)]
pub struct UserChangeset {
    pub email: Option<String>,
    pub name: Option<String>,
}
```

The exact re-export names such as `DateTime`, `Queryable`, or `Insertable` can evolve with the final MADS prelude. The important part is that MADS does not invent a second ORM model system on top of Diesel.

---

## 9. Request Inputs

`src/users/input.rs`:

```rust
use mads::prelude::*;
use serde::Deserialize;

#[derive(Debug, Deserialize, Input)]
pub struct CreateUser {
    #[validate(email)]
    pub email: String,

    #[validate(length(min = 2, max = 120))]
    pub name: String,
}

#[derive(Debug, Deserialize, Input)]
pub struct UpdateUser {
    #[validate(email)]
    pub email: Option<String>,

    #[validate(length(min = 2, max = 120))]
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, Input)]
pub struct UserListQuery {
    #[validate(range(min = 1))]
    pub page: Option<u32>,

    #[validate(range(min = 1, max = 100))]
    pub limit: Option<u32>,
}
```

MADS performs request deserialization and validation before calling the route handler.

---

## 10. Repository

`src/users/repository.rs`:

```rust
use diesel::prelude::*;
use diesel::OptionalExtension;
use mads::prelude::*;

use super::{
    model::{NewUser, User, UserChangeset},
    schema::users,
};

#[repository]
pub struct UserRepository {
    db: Database,
}

impl UserRepository {
    pub async fn list(
        &self,
        page: u32,
        limit: u32,
    ) -> Result<Vec<User>> {
        let offset = ((page - 1) * limit) as i64;
        let limit = limit as i64;

        self.db
            .run(move |conn| {
                users::table
                    .order(users::id.asc())
                    .offset(offset)
                    .limit(limit)
                    .select(User::as_select())
                    .load(conn)
            })
            .await
    }

    pub async fn find(&self, id: i64) -> Result<Option<User>> {
        self.db
            .run(move |conn| {
                users::table
                    .find(id)
                    .select(User::as_select())
                    .first(conn)
                    .optional()
            })
            .await
    }

    pub async fn find_by_email(
        &self,
        email: String,
    ) -> Result<Option<User>> {
        self.db
            .run(move |conn| {
                users::table
                    .filter(users::email.eq(email))
                    .select(User::as_select())
                    .first(conn)
                    .optional()
            })
            .await
    }

    pub async fn create(&self, input: NewUser) -> Result<User> {
        self.db
            .run(move |conn| {
                diesel::insert_into(users::table)
                    .values(input)
                    .returning(User::as_returning())
                    .get_result(conn)
            })
            .await
    }

    pub async fn update(
        &self,
        id: i64,
        changes: UserChangeset,
    ) -> Result<Option<User>> {
        self.db
            .run(move |conn| {
                diesel::update(users::table.find(id))
                    .set(changes)
                    .returning(User::as_returning())
                    .get_result(conn)
                    .optional()
            })
            .await
    }

    pub async fn delete(&self, id: i64) -> Result<bool> {
        self.db
            .run(move |conn| {
                let affected = diesel::delete(users::table.find(id))
                    .execute(conn)?;

                Ok(affected > 0)
            })
            .await
    }
}
```

Notice what is absent:

```text
Arc<UserRepository>
State<AppState>
Pool<ConnectionManager<_>> in every service
manual repository registration
manual connection acquisition in routes
```

The repository only declares:

```rust
db: Database
```

MADS resolves the implementation through database auto-configuration.

---

## 11. User Service

`src/users/service.rs`:

```rust
use mads::prelude::*;

use super::{
    CreateUser,
    NewUser,
    UpdateUser,
    User,
    UserChangeset,
    UserRepository,
};

#[service]
pub struct UserService {
    users: UserRepository,
}

impl UserService {
    pub async fn list(
        &self,
        page: u32,
        limit: u32,
    ) -> Result<Vec<User>> {
        self.users.list(page, limit).await
    }

    pub async fn find(&self, id: i64) -> Result<User> {
        self.users
            .find(id)
            .await?
            .ok_or_else(|| NotFound::new("user"))
    }

    pub async fn create(&self, input: CreateUser) -> Result<User> {
        if self
            .users
            .find_by_email(input.email.clone())
            .await?
            .is_some()
        {
            return Err(Conflict::new("email already exists").into());
        }

        self.users
            .create(NewUser {
                email: input.email,
                name: input.name,
            })
            .await
    }

    pub async fn update(
        &self,
        id: i64,
        input: UpdateUser,
    ) -> Result<User> {
        if let Some(email) = &input.email {
            if let Some(existing) = self
                .users
                .find_by_email(email.clone())
                .await?
            {
                if existing.id != id {
                    return Err(Conflict::new("email already exists").into());
                }
            }
        }

        let changes = UserChangeset {
            email: input.email,
            name: input.name,
        };

        self.users
            .update(id, changes)
            .await?
            .ok_or_else(|| NotFound::new("user"))
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        if !self.users.delete(id).await? {
            return Err(NotFound::new("user").into());
        }

        Ok(())
    }
}
```

The service declares its dependency:

```rust
users: UserRepository
```

That type is enough for MADS to construct the service graph.

---

## 12. Controller

`src/users/controller.rs`:

```rust
use mads::prelude::*;

use super::{
    CreateUser,
    UpdateUser,
    User,
    UserListQuery,
    UserService,
};

#[routes(prefix = "/users")]
pub trait UserRoutes {
    #[get("/")]
    async fn list_users(&self, query: Query<UserListQuery>) -> Result<Json<Vec<User>>>;

    #[get("/:id")]
    async fn get_user(&self, id: Path<i64>) -> Result<Json<User>>;

    #[post("/")]
    async fn create_user(&self, input: Json<CreateUser>) -> Result<Created<User>>;

    #[put("/:id")]
    async fn update_user(
        &self,
        id: Path<i64>,
        input: Json<UpdateUser>,
    ) -> Result<Json<User>>;

    #[delete("/:id")]
    async fn delete_user(&self, id: Path<i64>) -> Result<NoContent>;
}

#[controller(routes = [UserRoutes])]
pub struct UserController {
    users: UserService,
}

impl UserRoutes for UserController {
    async fn list_users(&self, query: Query<UserListQuery>) -> Result<Json<Vec<User>>> {
        let page = query.page.unwrap_or(1);
        let limit = query.limit.unwrap_or(20);
        Ok(Json(self.users.list(page, limit).await?))
    }

    async fn get_user(&self, id: Path<i64>) -> Result<Json<User>> {
        Ok(Json(self.users.find(*id).await?))
    }

    async fn create_user(&self, input: Json<CreateUser>) -> Result<Created<User>> {
        Ok(Created(self.users.create(input.into_inner()).await?))
    }

    async fn update_user(
        &self,
        id: Path<i64>,
        input: Json<UpdateUser>,
    ) -> Result<Json<User>> {
        let user = self.users.update(*id, input.into_inner()).await?;
        Ok(Json(user))
    }

    async fn delete_user(&self, id: Path<i64>) -> Result<NoContent> {
        self.users.delete(*id).await?;
        Ok(NoContent)
    }
}
```

The route trait defines the HTTP contract. The controller field declares its
single application dependency:

```text
Path<i64>           → HTTP path extractor
Query<UserListQuery> → HTTP query extractor
Json<CreateUser>    → HTTP body + validation
UserService         → controller dependency
```

There is no manual `State<AppState>` extraction. MADS constructs
`UserRepository`, then `UserService`, then `UserController`.

`#[routes]` validates the reusable route contract, while normal Rust trait
checking ensures that `UserController` implements every endpoint with matching
signatures. The foundation also retains deterministic framework-neutral route
metadata; v0.3 adds Axum adapters and router construction.

---

## 13. Root Application Module

`src/app.rs`:

```rust
use mads::prelude::*;

use crate::users::UserModule;

#[module(
    imports = [UserModule]
)]
pub struct AppModule;
```

The root module describes high-level architecture.

It does **not** repeat every repository, service, and route.

---

## 14. Main

`src/main.rs`:

```rust
use mads::prelude::*;

mod app;
mod users;

use app::AppModule;

#[mads::main]
async fn main() {
    Mads::run::<AppModule>().await;
}
```

There is no standard-path code for:

```text
Tokio runtime configuration
Axum Router creation
TCP listener creation
Diesel pool construction
application state creation
Arc wrapping
repository construction
service construction
route registration
migration bootstrap
error-to-response mapping
```

---

## 15. What MADS Builds

From the application code, MADS derives an internal graph resembling:

```text
AppModule
│
└── UserModule
    │
    └── UserController [/users]
        ├── GET /
        ├── GET /:id
        ├── POST /
        ├── PUT /:id
        ├── DELETE /:id
        └── UserService
            └── UserRepository
                └── Database
                    └── DieselPool
```

Combined route table:

```text
GET     /users
GET     /users/:id
POST    /users
PUT     /users/:id
DELETE  /users/:id
```

---

## 16. Startup Sequence

When the developer runs:

```bash
mads dev
```

MADS can perform:

```text
load mads.toml
      ↓
resolve environment variables
      ↓
read AppModule metadata
      ↓
discover UserModule metadata
      ↓
build dependency graph
      ↓
see UserRepository requires Database
      ↓
activate Diesel database auto-configuration
      ↓
create connection pool
      ↓
run pending migrations
      ↓
construct UserRepository
      ↓
construct UserService
      ↓
construct UserController
      ↓
register Axum routes
      ↓
start server
```

Example console output:

```text
MADS Development Server

✓ configuration loaded
✓ dependency graph validated
✓ Diesel/PostgreSQL configured
✓ database connected
✓ 1 migration applied
✓ 1 module loaded
✓ 5 routes registered

GET     /users
GET     /users/:id
POST    /users
PUT     /users/:id
DELETE  /users/:id

http://127.0.0.1:3000
```

---

## 17. Create User

Request:

```http
POST /users
Content-Type: application/json

{
  "email": "john@example.com",
  "name": "John Doe"
}
```

Flow:

```text
HTTP request
    ↓
Json<CreateUser>
    ↓
deserialization
    ↓
validation
    ↓
UserController::create_user
    ↓
UserService::create
    ↓
UserRepository::find_by_email
    ↓
UserRepository::create
    ↓
Diesel
    ↓
PostgreSQL
```

Response:

```http
HTTP/1.1 201 Created
Content-Type: application/json
```

```json
{
  "id": 1,
  "email": "john@example.com",
  "name": "John Doe",
  "created_at": "2026-08-15T10:00:00Z",
  "updated_at": "2026-08-15T10:00:00Z"
}
```

Duplicate email:

```http
HTTP/1.1 409 Conflict
```

```json
{
  "error": "conflict",
  "message": "email already exists"
}
```

---

## 18. List Users

Request:

```http
GET /users?page=1&limit=20
```

Response:

```json
[
  {
    "id": 1,
    "email": "john@example.com",
    "name": "John Doe",
    "created_at": "2026-08-15T10:00:00Z",
    "updated_at": "2026-08-15T10:00:00Z"
  }
]
```

A future pagination abstraction could wrap this in metadata, but the simple API does not require that feature to prove the core framework model.

---

## 19. Get User

Request:

```http
GET /users/1
```

Response:

```json
{
  "id": 1,
  "email": "john@example.com",
  "name": "John Doe",
  "created_at": "2026-08-15T10:00:00Z",
  "updated_at": "2026-08-15T10:00:00Z"
}
```

Missing user:

```http
HTTP/1.1 404 Not Found
```

```json
{
  "error": "not_found",
  "message": "user not found"
}
```

---

## 20. Update User

Request:

```http
PUT /users/1
Content-Type: application/json

{
  "name": "John Smith"
}
```

Response:

```json
{
  "id": 1,
  "email": "john@example.com",
  "name": "John Smith",
  "created_at": "2026-08-15T10:00:00Z",
  "updated_at": "2026-08-15T10:10:00Z"
}
```

---

## 21. Delete User

Request:

```http
DELETE /users/1
```

Response:

```http
HTTP/1.1 204 No Content
```

Deleting a missing user returns the same MADS-level `404` representation used elsewhere.

---

## 22. Validation Failure

Request:

```http
POST /users
Content-Type: application/json

{
  "email": "invalid-email",
  "name": "J"
}
```

MADS validates the `Input` before invoking `UserController`, which delegates to
`UserService`.

Response:

```http
HTTP/1.1 422 Unprocessable Entity
```

```json
{
  "error": "validation_failed",
  "fields": {
    "email": [
      "invalid email"
    ],
    "name": [
      "minimum length is 2"
    ]
  }
}
```

The exact status code and final validation response schema can remain a framework design choice, but the important developer experience is that route handlers do not manually repeat validation plumbing.

---

## 23. Database Configuration Failure

Suppose `DATABASE_URL` is absent.

MADS should not expose only an indirect Diesel or trait-bound failure.

Example diagnostic:

```text
error[MADS101]: database auto-configuration failed

Database is required by UserRepository.

Dependency path:

GET /users
└── UserController
    └── UserService
        └── UserRepository
            └── Database

MADS selected:
  DieselDatabaseAutoConfiguration

Missing configuration:
  database.url

help:
  set DATABASE_URL

or define:

  [database]
  url = "${DATABASE_URL}"
```

---

## 24. Dependency Failure

If `UserRepository` were removed while `UserService` still required it:

```text
error[MADS003]: unresolved dependency

UserService requires UserRepository,
but no visible provider was found.

Dependency graph:

GET /users/:id
└── UserController
    └── UserService
        └── UserRepository  ← missing

help:
  define UserRepository with #[repository]
  or import a module that exports it.
```

---

## 25. Inspecting the Graph

Run:

```bash
mads graph
```

Example:

```text
AppModule
└── UserModule
    ├── UserRepository
    │   └── Database
    │       └── DieselPool [PostgreSQL]
    │
    ├── UserService
    │   └── UserRepository
    │
    └── UserController [/users]
        ├── GET /
        ├── GET /:id
        ├── POST /
        ├── PUT /:id
        ├── DELETE /:id
        └── UserService
```

This makes automatic wiring inspectable rather than mysterious.

---

## 26. Inspecting Auto-Configuration

Run:

```bash
mads doctor
```

Example:

```text
MADS Auto Configuration Report

ACTIVE

✓ HttpServerAutoConfiguration
    Axum HTTP runtime available

✓ DieselDatabaseAutoConfiguration
    Database required by UserRepository
    database.url configured
    PostgreSQL backend selected

✓ ValidationAutoConfiguration
    Input-derived request validation detected

SKIPPED

○ RedisAutoConfiguration
    Redis capability not installed

○ MailAutoConfiguration
    Mailer not required
```

---

## 27. Service Test

The same dependency graph can be used for application tests.

```rust
#[mads::test]
async fn create_user(users: UserService) {
    let user = users
        .create(CreateUser {
            email: "john@example.com".into(),
            name: "John Doe".into(),
        })
        .await
        .unwrap();

    assert_eq!(user.email, "john@example.com");
}
```

The test does not need to manually construct `UserRepository` and `UserService` if the MADS test graph can resolve them.

---

## 28. HTTP Test

```rust
#[mads::test]
async fn get_user_endpoint(
    client: TestClient,
    users: UserService,
) {
    let created = users
        .create(CreateUser {
            email: "john@example.com".into(),
            name: "John Doe".into(),
        })
        .await
        .unwrap();

    client
        .get(format!("/users/{}", created.id))
        .send()
        .await
        .assert_status(200)
        .assert_json_path("$.email", "john@example.com");
}
```

The exact assertion API can evolve. The intended experience is that MADS owns application test bootstrapping while still allowing lower-level Axum testing when desired.

---

## 29. Custom Database Override

The default CRUD example uses Diesel auto-configuration.

An advanced application can override the database provider:

```rust
#[provider]
async fn database(config: CustomDbConfig) -> Result<Database> {
    // construct custom database integration
}
```

Once this provider exists:

```text
Custom Database provider detected
        ↓
DieselDatabaseAutoConfiguration backs off
```

`UserRepository` remains unchanged:

```rust
#[repository]
struct UserRepository {
    db: Database,
}
```

This demonstrates the intended auto-configuration back-off rule.

---

## 30. Using Native Axum When Needed

MADS should not block advanced HTTP features.

A controller route may directly use an Axum extractor if required. An optional
route contract can extend the controller:

```rust
use axum::extract::Multipart;

#[routes(prefix = "/users")]
trait UserUploadRoutes {
    #[post("/avatar")]
    async fn upload_avatar(&self, multipart: Multipart) -> Result<NoContent>;
}

// Add `UserUploadRoutes` to `#[controller(routes = [...])]`.
impl UserUploadRoutes for UserController {
    async fn upload_avatar(&self, multipart: Multipart) -> Result<NoContent> {
        // ...
        Ok(NoContent)
    }
}
```

The rest of the MADS dependency graph remains available.

---

## 31. Using Native Diesel When Needed

Repository code can directly use Diesel:

```rust
use diesel::prelude::*;

self.db
    .run(move |conn| {
        users::table
            .filter(users::email.like("%@example.com"))
            .select(User::as_select())
            .load(conn)
    })
    .await
```

MADS handles integration and lifecycle, not a replacement query language.

---

## 32. Final CRUD Developer Experience

The application developer primarily writes:

```text
User model
    ↓
CreateUser / UpdateUser input
    ↓
UserRepository
    ↓
UserService
    ↓
UserController
    ↓
HTTP routes
```

MADS handles the common infrastructure path:

```text
Axum router
Tokio runtime
shared application state
service ownership
repository ownership
Diesel pool
configuration loading
migration startup
route registration
dependency construction
error mapping
validation plumbing
framework diagnostics
```

The central value proposition is visible in the final dependency declaration:

```rust
#[repository]
struct UserRepository {
    db: Database,
}

#[service]
struct UserService {
    users: UserRepository,
}

#[controller(routes = [UserRoutes])]
struct UserController {
    users: UserService,
}
```

From those Rust types, MADS can derive:

```text
UserController
    ↓
UserService
    ↓
UserRepository
    ↓
Database
    ↓
Diesel
```

That is the intended final MADS experience:

> **Declare what your application needs. MADS wires the rest.**
