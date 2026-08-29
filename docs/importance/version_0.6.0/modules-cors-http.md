# v0.6.0 Modules, CORS, and HTTP Runtime

## Standard application boundary

MADS v0.6.0 makes a root module and conventional startup the standard
application path:

```rust
use mads::prelude::*;

#[module]
pub struct UserHttpModule;

#[module(imports = [UserHttpModule])]
pub struct AppModule;

#[mads::main]
async fn main() -> Result<(), HttpRuntimeError> {
    Mads::run::<AppModule>().await
}
```

The argument-free HTTP module owns the descriptors declared in its Rust module.
`AppModule` is imports-only: it selects the application scope without becoming
a second route or provider manifest.

## Scope and visibility

Descriptors belong to their nearest annotated Rust namespace. A selected module
may use private or restricted items that it owns. Across modules, a provider or
strategy must come from a directly imported module and be plain, unrestricted
`pub`. Imports are not transitive: when `A` imports `B` and `B` imports `C`,
`A` must also import `C` before using `C`'s public providers. There is no
separate export array.

## Conventional startup

`Mads::run` reads its process working directory in this order:

1. optional `.env` for interpolation only;
2. optional `mads.toml` as ordinary configuration;
3. final `MADS_*` environment overrides.

Both configuration files may be absent. The default server address is
`127.0.0.1:3000` (`server.host` and `server.port` respectively). A malformed
or unreadable present file is a bootstrap failure, and conventional startup
does not search parent directories.

## CORS

CORS is opt-in and is validated before lifecycle startup. List-valued settings
use TOML arrays:

```toml
[server.cors]
origins = ["https://app.example.com"]
methods = ["GET", "POST"]
allowed_headers = ["authorization", "content-type"]
exposed_headers = ["x-request-id"]
credentials = false
max_age_seconds = 600
```

Wildcard-capable settings use a scalar wildcard, not a one-element list:

```toml
[server.cors]
origins = "*"
methods = ["GET", "POST"]
allowed_headers = "*"
exposed_headers = "*"
```

Wildcard origins or headers cannot be combined with credentials. CORS controls
browser response access; it is not authentication and it is not CSRF
protection. Applications must still authenticate requests and install their own
CSRF policy where cross-site state changes are possible.

## Explicit HTTP composition

Use the low-level builder when configuration, embedded migrations, lifecycle
hooks, binding, or a native Axum router must be explicit. The explicit address
can use port zero, and native routes are merged into the raw router before it
is served:

```rust,ignore
let mut builder = Mads::builder_with_config(config);
builder.root::<AppModule>()?;
builder.database_migrations(MIGRATIONS)?;
let application = builder.build().await?;
let router = build_router(&application)?.merge(native_router);
serve_router(application, router, "127.0.0.1:0").await?;
```

For direct in-process use, merge native routes first and then call
`configure_router(&application, router)`. Do not configure a router before
passing it to `serve_router`: `serve_router` accepts the raw router and applies
the configured CORS layer itself.

## Release verification commands

The v0.6.0 release is verified with these commands:

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
cargo check -p mads --no-default-features
cargo check -p mads --no-default-features --features http,runtime-tokio
cargo check -p mads --no-default-features --features database
cargo check -p mads --no-default-features --features jwt
cargo check -p mads --no-default-features --features cookies
cargo +1.85.0 check --workspace --all-features
cargo llvm-cov --workspace --all-features --ignore-filename-regex '(^|/)tests/ui/' --fail-under-lines 85
```

The documentation acceptance scan also checks the standard entry point and
ensures that `v0.5.6` remains only in historical supersession context.
