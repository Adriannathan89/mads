# MADS.rs — Clean Architecture CRUD User Example

## Goal

Dokumen ini menunjukkan bagaimana MADS dapat digunakan untuk User CRUD dengan **Clean Architecture**, sambil menjaga domain dan application rules tidak bergantung pada Axum, Diesel, atau detail HTTP/database.

Prinsip dependency:

```text
                 outer layers

HTTP / MADS delivery
        │
        ▼
Infrastructure adapters
        │
        ▼
Application use cases
        │
        ▼
Domain

                 inner layers
```

Source-code dependency selalu mengarah ke dalam.

MADS dipakai terutama sebagai **composition/runtime framework pada outer layer**, bukan sebagai dependency wajib domain.

---

## 1. Target API

```text
GET    /users
GET    /users/:id
POST   /users
PUT    /users/:id
DELETE /users/:id
```

Runtime implementation:

```text
HTTP
 ↓
User Routes
 ↓
User Use Cases
 ↓
UserRepositoryPort
 ↑
DieselUserRepository
 ↓
MADS Database
 ↓
Diesel
 ↓
PostgreSQL
```

Perhatikan inversion di repository boundary:

```text
Application owns UserRepositoryPort
Infrastructure implements it
```

---

## 2. Project Structure

```text
user-api/
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
    │
    ├── domain/
    │   └── user/
    │       ├── mod.rs
    │       ├── entity.rs
    │       └── error.rs
    │
    ├── application/
    │   └── user/
    │       ├── mod.rs
    │       ├── dto.rs
    │       ├── ports.rs
    │       └── use_cases.rs
    │
    ├── infrastructure/
    │   └── persistence/
    │       └── user/
    │           ├── mod.rs
    │           ├── schema.rs
    │           ├── model.rs
    │           └── diesel_repository.rs
    │
    └── delivery/
        └── http/
            └── user/
                ├── mod.rs
                ├── input.rs
                └── routes.rs
```

Layer responsibilities:

```text
domain          → enterprise/domain rules
application     → use cases + ports
infrastructure  → Diesel implementation
HTTP delivery   → MADS routes / request-response mapping
app/main        → composition root
```

---

## 3. Cargo.toml

Conceptual target:

```toml
[package]
name = "user-api"
version = "0.1.0"
edition = "2024"

[dependencies]
mads = "1"
serde = { version = "1", features = ["derive"] }
```

The MADS facade/common feature set provides normal Axum/Diesel integration for the standard path.

Strict projects may split each architecture layer into separate Rust crates later, but this example uses modules to keep the example readable.

---

## 4. Configuration

`mads.toml`:

```toml
[app]
name = "clean-user-api"

[server]
host = "127.0.0.1"
port = 3000

[database]
url = "${DATABASE_URL}"
pool_size = 10
migrate = true
```

Environment:

```bash
export DATABASE_URL="postgres://postgres:postgres@localhost/clean_user_api"
```

Infrastructure configuration stays outside domain/application.

---

# DOMAIN LAYER

## 5. Domain Entity

`src/domain/user/entity.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub id: i64,
    pub email: String,
    pub name: String,
}

impl User {
    pub fn new(id: i64, email: String, name: String) -> Result<Self, UserDomainError> {
        if name.trim().len() < 2 {
            return Err(UserDomainError::InvalidName);
        }

        if !email.contains('@') {
            return Err(UserDomainError::InvalidEmail);
        }

        Ok(Self { id, email, name })
    }
}
```

Tidak ada:

```text
mads
axum
diesel
HTTP status
Database
```

Domain entity dapat diuji sebagai pure Rust.

---

## 6. Domain Errors

`src/domain/user/error.rs`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum UserDomainError {
    #[error("invalid user email")]
    InvalidEmail,

    #[error("invalid user name")]
    InvalidName,
}
```

Domain error tidak mengetahui `404`, `409`, atau response JSON.

---

## 7. Domain Module

`src/domain/user/mod.rs`:

```rust
mod entity;
mod error;

pub use entity::*;
pub use error::*;
```

---

# APPLICATION LAYER

## 8. Application DTOs

`src/application/user/dto.rs`:

```rust
#[derive(Debug, Clone)]
pub struct CreateUserCommand {
    pub email: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct UpdateUserCommand {
    pub email: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ListUsersQuery {
    pub page: u32,
    pub limit: u32,
}
```

DTO application tidak perlu menjadi HTTP `Json<T>`.

---

## 9. Repository Port

`src/application/user/ports.rs`:

```rust
use crate::domain::user::User;

#[async_trait::async_trait]
pub trait UserRepositoryPort: Send + Sync {
    async fn list(&self, page: u32, limit: u32) -> anyhow::Result<Vec<User>>;

    async fn find(&self, id: i64) -> anyhow::Result<Option<User>>;

    async fn find_by_email(&self, email: &str) -> anyhow::Result<Option<User>>;

    async fn create(&self, email: String, name: String) -> anyhow::Result<User>;

    async fn update(
        &self,
        id: i64,
        email: Option<String>,
        name: Option<String>,
    ) -> anyhow::Result<Option<User>>;

    async fn delete(&self, id: i64) -> anyhow::Result<bool>;
}
```

> Catatan: exact async-trait strategy dapat mengikuti kemampuan Rust/MADS saat v1 diimplementasikan. Yang penting adalah arah dependency: interface dimiliki application layer.

---

## 10. Application Errors

Application layer dapat memiliki use-case errors sendiri:

```rust
#[derive(Debug, thiserror::Error)]
pub enum UserApplicationError {
    #[error("user not found")]
    NotFound,

    #[error("email already exists")]
    EmailAlreadyExists,

    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
}
```

HTTP mapping dilakukan di delivery layer.

---

## 11. User Use Cases

`src/application/user/use_cases.rs`:

```rust
use crate::domain::user::User;

use super::{
    CreateUserCommand,
    ListUsersQuery,
    UpdateUserCommand,
    UserApplicationError,
    UserRepositoryPort,
};

pub struct UserUseCases<R>
where
    R: UserRepositoryPort,
{
    repository: R,
}

impl<R> UserUseCases<R>
where
    R: UserRepositoryPort,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn list(
        &self,
        query: ListUsersQuery,
    ) -> Result<Vec<User>, UserApplicationError> {
        self.repository
            .list(query.page, query.limit)
            .await
            .map_err(Into::into)
    }

    pub async fn find(&self, id: i64) -> Result<User, UserApplicationError> {
        self.repository
            .find(id)
            .await?
            .ok_or(UserApplicationError::NotFound)
    }

    pub async fn create(
        &self,
        command: CreateUserCommand,
    ) -> Result<User, UserApplicationError> {
        if self
            .repository
            .find_by_email(&command.email)
            .await?
            .is_some()
        {
            return Err(UserApplicationError::EmailAlreadyExists);
        }

        self.repository
            .create(command.email, command.name)
            .await
            .map_err(Into::into)
    }

    pub async fn update(
        &self,
        id: i64,
        command: UpdateUserCommand,
    ) -> Result<User, UserApplicationError> {
        if let Some(email) = &command.email {
            if let Some(existing) = self.repository.find_by_email(email).await? {
                if existing.id != id {
                    return Err(UserApplicationError::EmailAlreadyExists);
                }
            }
        }

        self.repository
            .update(id, command.email, command.name)
            .await?
            .ok_or(UserApplicationError::NotFound)
    }

    pub async fn delete(&self, id: i64) -> Result<(), UserApplicationError> {
        if !self.repository.delete(id).await? {
            return Err(UserApplicationError::NotFound);
        }

        Ok(())
    }
}
```

Notice that this application layer still has no Axum or Diesel API.

---

## 12. Application Module

`src/application/user/mod.rs`:

```rust
mod dto;
mod ports;
mod use_cases;

pub use dto::*;
pub use ports::*;
pub use use_cases::*;
```

---

# INFRASTRUCTURE LAYER

## 13. Diesel Schema

`src/infrastructure/persistence/user/schema.rs`:

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

---

## 14. Persistence Model

Persistence representation may be different from domain representation.

`src/infrastructure/persistence/user/model.rs`:

```rust
use super::schema::users;
use crate::domain::user::User;

#[derive(Queryable, Selectable)]
#[diesel(table_name = users)]
pub struct UserRow {
    pub id: i64,
    pub email: String,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<UserRow> for User {
    fn from(row: UserRow) -> Self {
        Self {
            id: row.id,
            email: row.email,
            name: row.name,
        }
    }
}

#[derive(Insertable)]
#[diesel(table_name = users)]
pub struct NewUserRow {
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

Clean Architecture tidak memaksa domain entity menggunakan Diesel derives.

---

## 15. Diesel Repository Adapter

`src/infrastructure/persistence/user/diesel_repository.rs`:

```rust
use diesel::prelude::*;
use diesel::OptionalExtension;
use mads::prelude::*;

use crate::{
    application::user::UserRepositoryPort,
    domain::user::User,
};

use super::{
    model::{NewUserRow, UserChangeset, UserRow},
    schema::users,
};

#[repository(as = UserRepositoryPort)]
pub struct DieselUserRepository {
    db: Database,
}

#[async_trait::async_trait]
impl UserRepositoryPort for DieselUserRepository {
    async fn list(&self, page: u32, limit: u32) -> anyhow::Result<Vec<User>> {
        let offset = ((page - 1) * limit) as i64;
        let limit = limit as i64;

        let rows = self.db
            .run(move |conn| {
                users::table
                    .order(users::id.asc())
                    .offset(offset)
                    .limit(limit)
                    .select(UserRow::as_select())
                    .load::<UserRow>(conn)
            })
            .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn find(&self, id: i64) -> anyhow::Result<Option<User>> {
        let row = self.db
            .run(move |conn| {
                users::table
                    .find(id)
                    .select(UserRow::as_select())
                    .first::<UserRow>(conn)
                    .optional()
            })
            .await?;

        Ok(row.map(Into::into))
    }

    async fn find_by_email(&self, email: &str) -> anyhow::Result<Option<User>> {
        let email = email.to_owned();

        let row = self.db
            .run(move |conn| {
                users::table
                    .filter(users::email.eq(email))
                    .select(UserRow::as_select())
                    .first::<UserRow>(conn)
                    .optional()
            })
            .await?;

        Ok(row.map(Into::into))
    }

    async fn create(&self, email: String, name: String) -> anyhow::Result<User> {
        let row = self.db
            .run(move |conn| {
                diesel::insert_into(users::table)
                    .values(NewUserRow { email, name })
                    .returning(UserRow::as_returning())
                    .get_result::<UserRow>(conn)
            })
            .await?;

        Ok(row.into())
    }

    async fn update(
        &self,
        id: i64,
        email: Option<String>,
        name: Option<String>,
    ) -> anyhow::Result<Option<User>> {
        let row = self.db
            .run(move |conn| {
                diesel::update(users::table.find(id))
                    .set(UserChangeset { email, name })
                    .returning(UserRow::as_returning())
                    .get_result::<UserRow>(conn)
                    .optional()
            })
            .await?;

        Ok(row.map(Into::into))
    }

    async fn delete(&self, id: i64) -> anyhow::Result<bool> {
        let affected = self.db
            .run(move |conn| {
                diesel::delete(users::table.find(id)).execute(conn)
            })
            .await?;

        Ok(affected > 0)
    }
}
```

The exact `#[repository(as = ...)]` syntax is a **target MADS API**: it means that this infrastructure adapter satisfies the application-owned repository port.

MADS can therefore resolve:

```text
UserRepositoryPort
       ↑
DieselUserRepository
       ↓
Database
```

without the application layer importing Diesel.

---

## 16. Infrastructure Module

`src/infrastructure/persistence/user/mod.rs`:

```rust
mod diesel_repository;
mod model;
mod schema;

pub use diesel_repository::*;
```

---

# DELIVERY LAYER

## 17. HTTP Inputs

HTTP request models belong in delivery because validation/deserialization can be transport-specific.

`src/delivery/http/user/input.rs`:

```rust
use mads::prelude::*;
use serde::Deserialize;

#[derive(Debug, Deserialize, Input)]
pub struct CreateUserRequest {
    #[validate(email)]
    pub email: String,

    #[validate(length(min = 2, max = 120))]
    pub name: String,
}

#[derive(Debug, Deserialize, Input)]
pub struct UpdateUserRequest {
    #[validate(email)]
    pub email: Option<String>,

    #[validate(length(min = 2, max = 120))]
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, Input)]
pub struct ListUsersRequest {
    #[validate(range(min = 1))]
    pub page: Option<u32>,

    #[validate(range(min = 1, max = 100))]
    pub limit: Option<u32>,
}
```

Mapping to application command happens at the boundary.

---

## 18. HTTP Response DTO

Optional transport DTO:

```rust
use serde::Serialize;

use crate::domain::user::User;

#[derive(Serialize)]
pub struct UserResponse {
    pub id: i64,
    pub email: String,
    pub name: String,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            email: user.email,
            name: user.name,
        }
    }
}
```

Domain entity does not need `Serialize` solely because HTTP needs JSON.

---

## 19. HTTP-facing Application Service Adapter

For strict Clean Architecture, MADS-specific injection can live in outer-layer composition. One target pattern is a MADS service wrapping the pure use case with a resolved repository port:

```rust
use mads::prelude::*;

use crate::{
    application::user::{UserRepositoryPort, UserUseCases},
    infrastructure::persistence::user::DieselUserRepository,
};

#[service]
pub struct UserApplicationService {
    repository: DieselUserRepository,
}

impl UserApplicationService {
    fn use_cases(&self) -> UserUseCases<&DieselUserRepository> {
        UserUseCases::new(&self.repository)
    }
}
```

An even more advanced final MADS implementation can inject a trait capability directly, allowing the wrapper to depend on `Inject<dyn UserRepositoryPort>` instead of the concrete Diesel adapter. The architectural goal remains the same: the application use-case implementation itself is framework-independent.

---

## 20. HTTP Error Mapping

Delivery maps application semantics to HTTP:

```rust
fn map_user_error(error: UserApplicationError) -> mads::Error {
    match error {
        UserApplicationError::NotFound => NotFound::new("user").into(),
        UserApplicationError::EmailAlreadyExists => {
            Conflict::new("email already exists").into()
        }
        UserApplicationError::Unexpected(error) => error.into(),
    }
}
```

This keeps:

```text
404 / 409 → delivery concern
```

rather than domain concern.

---

## 21. Routes

`src/delivery/http/user/routes.rs`:

```rust
use mads::prelude::*;

use crate::application::user::{
    CreateUserCommand,
    ListUsersQuery,
    UpdateUserCommand,
};

use super::{
    CreateUserRequest,
    ListUsersRequest,
    UpdateUserRequest,
    UserResponse,
    UserApplicationService,
};

#[get("/")]
pub async fn list_users(
    query: Query<ListUsersRequest>,
    users: UserApplicationService,
) -> Result<Json<Vec<UserResponse>>> {
    let result = users
        .use_cases()
        .list(ListUsersQuery {
            page: query.page.unwrap_or(1),
            limit: query.limit.unwrap_or(20),
        })
        .await
        .map_err(map_user_error)?;

    Ok(Json(result.into_iter().map(Into::into).collect()))
}

#[get("/:id")]
pub async fn get_user(
    id: Path<i64>,
    users: UserApplicationService,
) -> Result<Json<UserResponse>> {
    let user = users
        .use_cases()
        .find(*id)
        .await
        .map_err(map_user_error)?;

    Ok(Json(user.into()))
}

#[post("/")]
pub async fn create_user(
    request: Json<CreateUserRequest>,
    users: UserApplicationService,
) -> Result<Created<UserResponse>> {
    let request = request.into_inner();

    let user = users
        .use_cases()
        .create(CreateUserCommand {
            email: request.email,
            name: request.name,
        })
        .await
        .map_err(map_user_error)?;

    Ok(Created(user.into()))
}

#[put("/:id")]
pub async fn update_user(
    id: Path<i64>,
    request: Json<UpdateUserRequest>,
    users: UserApplicationService,
) -> Result<Json<UserResponse>> {
    let request = request.into_inner();

    let user = users
        .use_cases()
        .update(
            *id,
            UpdateUserCommand {
                email: request.email,
                name: request.name,
            },
        )
        .await
        .map_err(map_user_error)?;

    Ok(Json(user.into()))
}

#[delete("/:id")]
pub async fn delete_user(
    id: Path<i64>,
    users: UserApplicationService,
) -> Result<NoContent> {
    users
        .use_cases()
        .delete(*id)
        .await
        .map_err(map_user_error)?;

    Ok(NoContent)
}
```

HTTP handlers are intentionally thin:

```text
extract request
  ↓
map to command/query
  ↓
execute use case
  ↓
map result/error to HTTP
```

Business rules do not live in routes.

---

## 22. HTTP Module

`src/delivery/http/user/mod.rs`:

```rust
use mads::prelude::*;

mod input;
mod response;
mod routes;
mod service_adapter;

pub use input::*;
pub use response::*;
pub use service_adapter::*;

#[module(path = "/users")]
pub struct UserHttpModule;
```

Route registration is derived from MADS route metadata rather than a manual list.

---

# COMPOSITION ROOT

## 23. App Module

`src/app.rs`:

```rust
use mads::prelude::*;

use crate::delivery::http::user::UserHttpModule;

#[module(imports = [UserHttpModule])]
pub struct AppModule;
```

The composition root is allowed to know outer-layer details.

MADS resolves approximately:

```text
UserHttpModule
  ↓
UserApplicationService
  ↓
DieselUserRepository
  ↓
Database
  ↓
DieselDatabaseAutoConfiguration
```

while the pure application use cases only know `UserRepositoryPort`.

---

## 24. Main

`src/main.rs`:

```rust
use mads::prelude::*;

mod app;
mod application;
mod delivery;
mod domain;
mod infrastructure;

use app::AppModule;

#[mads::main]
async fn main() {
    Mads::run::<AppModule>().await;
}
```

No standard-path bootstrap for:

```text
Tokio
Axum Router
AppState
Arc
Diesel pool
repository construction
service construction
route list
```

---

## 25. Final Dependency Diagram

At compile/source level:

```text
┌───────────────────────────────────┐
│              Domain               │
│ User / domain rules / errors      │
└─────────────────▲─────────────────┘
                  │
┌─────────────────┴─────────────────┐
│            Application            │
│ Commands / Use Cases / Ports      │
└────────────▲──────────────▲───────┘
             │              │ implements
             │              │
┌────────────┴───────┐  ┌───┴─────────────────────┐
│   HTTP Delivery    │  │     Infrastructure      │
│ MADS route macros  │  │ DieselUserRepository    │
└────────────┬───────┘  └───────────┬─────────────┘
             │                      │
             └──────────┬───────────┘
                        ▼
                 MADS Composition
```

At runtime:

```text
HTTP Request
    ↓
MADS / Axum Route
    ↓
UserApplicationService
    ↓
UserUseCases
    ↓
UserRepositoryPort
    ↓
DieselUserRepository
    ↓
Database
    ↓
Diesel
    ↓
PostgreSQL
```

---

## 26. Why This Is Cleaner Than the Simple CRUD Example

Simple MADS architecture may intentionally be:

```text
Route
 ↓
UserService
 ↓
UserRepository
 ↓
Database
```

That is appropriate for many applications.

Clean Architecture adds stronger boundaries:

```text
Route
 ↓
Application Use Case
 ↓
Repository Port
 ↑
Infrastructure Adapter
```

Trade-off:

```text
Simple architecture
  + less code
  + faster CRUD development
  - application layer can know concrete repository types

Clean Architecture
  + business rules independent from MADS/Axum/Diesel
  + persistence replaceable behind ports
  + easier isolated use-case tests
  - more types and mapping code
```

MADS should support both instead of forcing one style.

---

## 27. Pure Application Test

Because use cases are not tied to MADS, they can be tested without starting MADS.

```rust
struct FakeUserRepository {
    // in-memory data
}

#[async_trait::async_trait]
impl UserRepositoryPort for FakeUserRepository {
    // fake methods
}

#[tokio::test]
async fn creating_duplicate_email_fails() {
    let repository = FakeUserRepository::with_user(...);
    let users = UserUseCases::new(repository);

    let result = users
        .create(CreateUserCommand {
            email: "john@example.com".into(),
            name: "John".into(),
        })
        .await;

    assert!(matches!(
        result,
        Err(UserApplicationError::EmailAlreadyExists)
    ));
}
```

This is a major benefit of keeping application logic inward-facing.

---

## 28. MADS Integration Test

Outer integration can still use MADS test graph:

```rust
#[mads::test]
async fn get_user_endpoint(client: TestClient) {
    client
        .get("/users/1")
        .send()
        .await
        .assert_status(200);
}
```

Use both test levels:

```text
Domain/Application tests → fast, pure
Infrastructure tests     → Diesel integration
HTTP/MADS tests          → full application graph
```

---

## 29. Architecture Rule for MADS Projects

A useful rule is:

> **MADS may compose the application, but the domain should not require MADS to exist.**

For strict Clean Architecture:

```text
domain          must not import mads/axum/diesel
application     must not import axum/diesel
infrastructure  may import diesel + mads Database integration
delivery        may import mads/common + HTTP concepts
composition     may know all outer implementations
```

This preserves dependency inversion while still benefiting from MADS auto-wiring.

---

## 30. Final Clean Architecture Experience

Developer explicitly writes the architecture that matters:

```text
UserUseCases depends on UserRepositoryPort
DieselUserRepository implements UserRepositoryPort
User HTTP module exposes routes
AppModule imports User HTTP module
```

MADS handles the infrastructure ceremony:

```text
provider discovery
construction order
shared ownership
Database auto-configuration
Diesel pool
Axum router
route registry
startup
validation plumbing
framework diagnostics
```

The final goal is not to hide architecture.

It is to remove the plumbing around architecture.

> **Clean Architecture defines the boundaries. MADS wires the runtime.**
