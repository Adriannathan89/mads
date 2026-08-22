# MADS.rs — Clean Architecture with v0.4 Persistence

MADS belongs at the composition and delivery edge. Domain and application code
should not depend on Axum, Diesel, or HTTP response types.

```text
HTTP / MADS delivery
        ↓
Infrastructure (Diesel repository)
        ↓
Application use cases and ports
        ↓
Domain
```

## Available in v0.4

`Database` and `Database::run` are available now. A repository can accept the
managed database and keep native Diesel details inside infrastructure:

```rust,ignore
#[mads::repository]
struct DieselUserRepository {
    database: mads::Database,
}

impl DieselUserRepository {
    async fn find(&self, id: i64) -> mads::DatabaseResult<Option<User>> {
        self.database
            .run(move |connection| {
                // Native Diesel select/query code belongs here.
                users::table.find(id).first(connection).optional()
            })
            .await
    }
}
```

The composition root loads `mads.toml` plus optional `.env`, resolves
`DatabaseConfig`, and explicitly calls
`builder.database(DatabaseBootstrap::new(database_config))`. If startup
migrations are enabled, it provides
`DatabaseBootstrap::new(database_config).with_migrations(MIGRATIONS)` instead.
The database URL stays as `${DATABASE_URL}` in tracked configuration; real
values belong in ignored `.env` locally or in production process variables.

Delivery code maps application outcomes to HTTP deliberately. A failed
`Database::run` does **not** automatically become an HTTP error response.

## Project shape

```text
src/
├── domain/                 # entities and domain rules
├── application/            # use cases and repository-port traits
├── infrastructure/         # Diesel schemas/models/repositories
├── delivery/http/          # MADS route traits and controllers
└── main.rs                 # ConfigBuilder + DatabaseBootstrap composition root
```

The application layer owns an ordinary Rust repository-port trait. The
infrastructure implementation owns its Diesel schema/query types, and the
controller depends on application-facing behavior rather than moving database
types into the domain.

## Target/future APIs

The following are intentionally **not** v0.4 APIs:

- `#[repository(as = Port)]` trait-binding syntax;
- zero-bootstrap database auto-configuration; and
- a MADS database test DSL.

They remain targets for later milestones. v0.4 also does not provide automatic
validation or automatic HTTP error normalization.
