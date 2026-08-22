# MADS.rs

MADS.rs 0.4 is a Rust application framework with a framework-neutral core, an
Axum HTTP runtime, and explicit PostgreSQL/Diesel persistence. It validates
every declared route before it builds a router, starts lifecycle hooks, checks
the database, or binds a socket.

## Crates and boundaries

- `mads-core` owns construction, providers, lifecycle, diagnostics, and
  generic scalar TOML/dotenv configuration. It has no database or HTTP
  dependency.
- `mads-common` owns route validation, Axum delivery, explicit Diesel database
  bootstrap, PostgreSQL pools, and migration execution.
- `mads` is the stable facade. Its default `common` feature exposes the HTTP
  runtime and persistence; disable default features for the core-only boundary.

```toml
[dependencies]
mads = "0.4"
serde = { version = "1", features = ["derive"] }

[dev-dependencies]
tower = { version = "0.5", features = ["util"] }
```

MADS.rs supports Rust 1.85 and uses Rust edition 2024.

## Configure PostgreSQL explicitly

Commit configuration shape, never a connection secret:

```toml
# mads.toml
[database]
url = "${DATABASE_URL}"
pool_size = 10 # default: 10
migrate = false # default: false
```

Use the tracked [`.env.example`](.env.example) as a local template, copy it to
the ignored `.env`, and put real secrets in process variables in CI and
production. The repository safety pattern is:

```gitignore
.env
.env.*
!.env.example
```

Dotenv loading is generic: `redis.url = "${REDIS_URL}"` needs no
database-specific loader in a future integration. Dotenv files are read into a
temporary interpolation map and never mutate the process environment. Process
variables win over dotenv variables. Configuration sources merge in order, so
the final `EnvSource::new("MADS_")` makes `MADS_DATABASE__URL` a direct
`database.url` override (and likewise `MADS_DATABASE__POOL_SIZE` and
`MADS_DATABASE__MIGRATE`).

```rust,ignore
use mads::{
    core::{ConfigBuilder, DotenvSource, EnvSource, TomlSource},
    diesel_migrations::{EmbeddedMigrations, embed_migrations},
    prelude::*,
};

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

let config = ConfigBuilder::new()
    .dotenv(DotenvSource::optional(".env"))
    .source(TomlSource::file("mads.toml"))
    .source(EnvSource::new("MADS_"))
    .build()?;
let database = DatabaseConfig::from_config(&config)?;
let mut builder = Mads::builder_with_config(config);
builder.database(
    DatabaseBootstrap::new(database).with_migrations(MIGRATIONS),
)?;
let application = builder.build().await?;
serve(application, "127.0.0.1:3000").await?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

`DatabaseBootstrap` is deliberate: it provides one shared `Database` and a
lifecycle hook. Startup checks PostgreSQL, then runs configured embedded
migrations, before listener binding. Invalid routes still prevent any database
connection attempt. `Database::run` sends each synchronous Diesel operation
through the managed pool's blocking interaction boundary. Use the direct
`mads::diesel` and `mads::diesel_migrations` re-exports when native Diesel APIs
are the right tool.

## CLI migrations

From a project root containing `mads.toml` and `migrations/`:

```text
mads db migrate   # apply pending file migrations
mads db rollback  # revert the latest migration from this source
mads db status    # report applied and pending versions
```

`mads db migrate` prints `applied <version>` for work performed or `database is
up to date`; `rollback` prints `reverted <version>`; `status` prints individual
versions plus an applied/pending summary. Invalid command syntax exits with 2;
configuration, pool, or migration failures exit with 1.

## A typed HTTP route

`#[mads::routes]` records immutable metadata and emits a typed registration
adapter. `#[mads::controller]` resolves the managed controller once while the
router is built; handlers do not receive manual `State<AppState>` or perform
per-request provider resolution.

```rust,no_run
use mads::prelude::*;

#[derive(Clone, serde::Serialize)]
struct User {
    id: u64,
}

#[mads::routes(prefix = "/readme-users")]
trait UserRoutes {
    #[mads::get("/:id")]
    async fn get_user(&self, id: Path<u64>) -> HttpResult<Json<User>>;
}

#[mads::controller(routes = [UserRoutes])]
struct UserController;

impl UserRoutes for UserController {
    async fn get_user(&self, Path(id): Path<u64>) -> HttpResult<Json<User>> {
        Ok(Json(User { id }))
    }
}

#[mads::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let application = Mads::builder().build().await?;
    serve(application, "127.0.0.1:3000").await?;
    Ok(())
}
```

## Extractors, responses, and routing

The prelude exports `Path<T>`, `Query<T>`, `Json<T>`, `Header<T>`, `Request`,
`HttpResult<T>`, `Created<T>`, `NoContent`, `build_router`, and `serve`.
`mads::common::axum` remains the native Axum escape hatch for extractors,
responses, routers, middleware, and Tower composition.

MADS route metadata uses `/:parameter`; the validated adapter translates it to
Axum 0.8 syntax only while registering the route. Invalid metadata and
conflicts fail with `MADS030` before router construction. GET also handles
HEAD, OPTIONS is not synthesized, static routes win over parameter routes, and
trailing slashes remain strict. Use `build_router(&application)` with Tower's
`ServiceExt::oneshot` for in-process route tests without binding a listener.

## Current scope

Version 0.4 provides explicit PostgreSQL-only Diesel persistence, optional
dotenv interpolation, managed pools, embedded/file migrations, and the
existing typed HTTP runtime. Auto-configuration/back-off, MySQL/SQLite,
request validation, and automatic HTTP error normalization remain future work.
Database errors are not automatically mapped to HTTP responses; applications
choose their delivery policy.

## Development

Run the available release checks locally:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace --all-features --doc
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
cargo +1.85.0 test --locked --workspace --all-features
```

CI also provisions PostgreSQL 16 and runs the ignored database suites plus the
85% line-coverage gate. To run those locally, set `MADS_TEST_DATABASE_URL` to a
PostgreSQL 16 database and use the commands in the [v0.4 requirements](docs/importance/version_0.4/diesel-persistence.md).
