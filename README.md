# MADS.rs

MADS.rs 0.5.5 is a Rust application framework with a framework-neutral core, an
Axum HTTP runtime, and an explainable PostgreSQL/Diesel conditional default.
It validates every declared route before it starts lifecycle hooks, checks the
database, or binds a socket.

## Crates and boundaries

- `mads-core` owns construction, providers, lifecycle, diagnostics, and
  generic scalar TOML/dotenv configuration, plus official conditional-default
  evaluation and redacted inspection reports. It has no database or HTTP
  dependency.
- `mads-common` owns route validation, Axum delivery, cookies, JWT/Passport,
  the official Diesel default, PostgreSQL pools, database infrastructure
  lifecycle, and migration execution.
- `mads` is the stable facade. Its default `common` feature exposes the HTTP
  runtime and persistence; disable default features for the core-only boundary.

```toml
[dependencies]
mads = { version = "0.5.5", features = ["jwt", "cookies"] }
serde = { version = "1", features = ["derive"] }

[dev-dependencies]
tower = { version = "0.5", features = ["util"] }
```

MADS.rs supports Rust 1.85 and uses Rust edition 2024.

The default `common` feature remains the compatibility aggregate for HTTP and
PostgreSQL; it does not silently enable authentication. For an HTTP API without
Diesel, use `default-features = false` with `features = ["http", "jwt",
"cookies", "runtime-tokio"]`. `jwt` alone does not pull in Axum, while
`cookies` includes HTTP. Passport strategies and Bearer guards require `http +
jwt`; cookie guards additionally require `cookies`.

## Explicit configuration, zero database bootstrap

Commit configuration shape, never a connection secret:

```toml
# mads.toml
[database]
url = "${DATABASE_URL}"
pool_size = 10 # default: 10
migrate = true # default: false
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
`MADS_DATABASE__MIGRATE`). MADS does not auto-load `.env`, `mads.toml`, or
`MADS_*` variables; applications assemble this configuration explicitly.

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
let mut builder = Mads::builder_with_config(config);
builder.database_migrations(MIGRATIONS)?;
let application = builder.build().await?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

This is zero **database** bootstrap, not zero application configuration. A
provider that directly requires `Database` activates the linked default only
after the complete catalog, explicit configuration, and virtual graph all
validate. `database_migrations` separately registers one embedded source; it
does not create a pool, connect, or run migrations. It is required only when
`database.migrate = true`; existing pending embedded migrations then run after
readiness, and no pending migrations are a successful no-op. MADS never
generates migration SQL, derives schema changes, or auto-loads a migration
directory.

Inspect the retained, redacted decision records without exposing configuration
values:

```rust,ignore
for report in application.auto_configurations() {
    println!(
        "{} {:?} {}",
        report.identifier(),
        report.status(),
        report.reason_code().as_str(),
    );
}
```

`DatabaseBootstrap` remains the explicit native Diesel override. It backs off
the conditional default completely and contributes its database lifecycle as
framework infrastructure. An application-provided `Database` instead owns its
complete readiness, migration, and shutdown lifecycle:

```rust,ignore
use mads::{core::{ConfigBuilder, MapSource}, prelude::*};

let config = ConfigBuilder::new()
    .source(MapSource::new(
        "application",
        [("database.url", "postgres://localhost/mads")],
    ))
    .build()?;
let database = DatabaseConfig::from_config(&config)?;
let mut builder = Mads::builder_with_config(config);
builder.database(DatabaseBootstrap::new(database))?;
let application = builder.build().await?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Use the direct `mads::diesel` and
`mads::diesel_migrations` re-exports when native Diesel APIs are the right
tool.

Listener addresses remain explicit, for example
`serve(application, "127.0.0.1:3000")`. HTTP host/port auto-binding is deferred
to v0.5.6.

## Passport configuration and JWT profiles

Configuration is never loaded implicitly. Dotenv sources provide interpolation
values, and ordinary sources merge from first to last; a later scalar or string
array replaces an earlier value at the same key completely. Process variables
override dotenv values during `${NAME}` interpolation. `EnvSource` is
scalar-only, so arrays such as `algorithms` and `audiences` belong in TOML or a
programmatic `ConfigDocument`/`MapSource`.

```toml
# mads.toml
[passport]
secret = "${JWT_SECRET}"
algorithms = ["HS256"]
issuer = "https://auth.example.com"
audiences = ["mads-api"]
```

Simple `secret` mode permits one HMAC algorithm: HS256 by default, or one of
HS384/HS512 when explicitly selected. Minimum secret sizes are 32/48/64 bytes.
For rotation, configure a named key ring; the active key signs and all retained
keys verify by `kid`:

```toml
[passport]
active_key = "2026-08"
algorithms = ["RS256"]

[passport.keys."2026-08"]
algorithm = "RS256"
private_key_file = "keys/current-private.pem"
public_key_file = "keys/current-public.pem"

[passport.keys."2026-07"]
algorithm = "RS256"
public_key_file = "keys/previous-public.pem"
```

MADS supports HS256/384/512, RS256/384/512, and ES256/384. The configured
allowlist—not an untrusted token header—selects eligible algorithms, and every
named key is bound to one algorithm. Relative paths from TOML resolve beside
that TOML file; paths from environment or programmatic sources resolve from the
process working directory.

```rust,ignore
use std::time::Duration;
use mads::prelude::*;

let access = jwt.sign(
    UserClaims { user_id: 7 },
    JwtSignOptions::access(Duration::from_secs(900)).subject("7"),
)?;
let refresh = jwt.sign(
    UserClaims { user_id: 7 },
    JwtSignOptions::refresh(Duration::from_secs(604_800)).subject("7"),
)?;
let verified_access = jwt.verify::<UserClaims>(&access, JwtValidation::access())?;
let verified_refresh = jwt.verify::<UserClaims>(&refresh, JwtValidation::refresh())?;
```

Access and refresh tokens have different protected `typ` values and
`token_use` claims. They are not interchangeable. Unverified decode APIs are
inspection-only and must never authenticate a request.

## Managed strategies, principals, and guards

A custom strategy is both a managed provider and an annotated
`PassportStrategy` implementation. Framework signature, registered-claim, and
token-kind verification always happens before `validate`; the strategy sees
verified claims and a credential-sanitized, read-only `PassportContext`.

```rust,ignore
#[derive(PassportPrincipal)]
struct UserPrincipal {
    user_id: u64,
    #[roles]
    roles: Vec<String>,
    #[permissions]
    permissions: std::collections::BTreeSet<String>,
}

#[service]
struct AppJwtStrategy { users: UserService }

#[passport_strategy(name = "jwt")]
impl PassportStrategy for AppJwtStrategy {
    type Claims = UserClaims;
    type Principal = UserPrincipal;
    const TOKEN_KIND: JwtTokenKind = JwtTokenKind::Access;

    async fn validate(
        &self,
        context: &PassportContext<'_>,
        claims: &JwtClaims<Self::Claims>,
    ) -> PassportResult<Self::Principal> {
        self.users.authenticate_current(context, claims.custom.user_id).await
    }
}
```

`jwt` is the built-in access strategy and can authorize directly as
`ClaimsPrincipal<C>`. A custom `jwt` strategy overrides it. `jwt-refresh` is not
built in: applications define it with `JwtTokenKind::Refresh` and own any
persistence, rotation, reuse detection, and revocation.

```rust,ignore
fn owns_profile(principal: &UserPrincipal) -> bool { principal.user_id == 7 }

#[routes(prefix = "/users")]
#[guard(
    strategy = "jwt",
    principal = UserPrincipal,
    source = bearer,
    roles(any = ["user", "admin"]),
)]
trait UserRoutes {
    #[get("/profile")]
    #[guard(
        permissions(all = ["profile:read"]),
        predicate = owns_profile,
    )]
    async fn profile(
        &self,
        principal: Authenticated<UserPrincipal>,
        token: VerifiedToken<UserClaims>,
    ) -> HttpResult<Json<Profile>>;

    #[post("/login")]
    #[guard(skip)]
    async fn login(&self) -> HttpResult<Json<LoginResponse>>;
}
```

Trait policies inherit. A method replaces only fields it supplies;
`#[guard(skip)]` is the sole opt-out. Roles, permissions, and predicates are
ANDed; `any`/`all` controls matching inside one role or permission clause, and
every predicate must be a synchronous `fn(&UserPrincipal) -> bool`. A guard
uses exactly one source. With `cookies`, select
`source = cookie("refresh_token")`; there is no Bearer fallback.

Authentication and strategy rejection map to generic `401 Unauthorized` with
`WWW-Authenticate: Bearer`, authorization policy failures to `403 Forbidden`,
and operational failures to `500 Internal Server Error`. Ordinary malformed
cookie extraction remains `400 Bad Request`; a missing, malformed, or duplicate
guard cookie is a generic `401`.

Cookie jars compose with response tuples and emit checked `Set-Cookie` headers:

```rust,ignore
let cookie = Cookie::build(("refresh_token", refresh))
    .path("/")
    .http_only(true)
    .secure(true)
    .same_site(SameSite::Strict)
    .max_age(cookie::time::Duration::days(7))
    .build();
Ok((jar.add(cookie), Json(response)))
```

For native Axum routes, apply a typed `PassportGuard<P>` Tower layer. This is a
runtime escape hatch, not static MADS guard metadata, so it cannot activate JWT
auto-configuration. Before `PassportGuard::build()`, a managed provider must
directly require `JwtService`, or the builder must explicitly provide a
concrete `JwtService`; otherwise construction fails with `MADS131`.

See the complete [Passport/JWT example](docs/examples/passport_jwt.md) and the
[v0.5.5 security and release notes](docs/importance/version_0.5.5/passport-jwt-and-cookies.md).

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

Version 0.5.5 provides PostgreSQL-only Diesel conditional defaults, explicit
scalar/array configuration, cookies, typed access/refresh JWTs, managed Passport
strategies, principals, and route/native guards. It does **not** implement login
or credential validation, refresh endpoints, refresh persistence/rotation/
revocation, password hashing, CSRF, CORS, HTTP auto-binding, remote JWKS, JWE,
or module scoping. Database errors are not automatically mapped to HTTP
responses; applications choose their delivery policy.

v0.5.6 owns CORS and HTTP auto-binding. v0.6.0 will restrict currently global
strategy descriptors to root-module-reachable providers and enforce export
eligibility; v0.5.5 deliberately does not implement that scoping.

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
PostgreSQL 16 database and use the commands in the [v0.5 requirements](docs/importance/version_0.5/auto-configuration.md).
