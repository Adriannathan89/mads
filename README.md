# MADS.rs

MADS.rs 0.3 is a Rust application framework with a framework-neutral core and
an executable Axum HTTP runtime. It validates every declared route before it
builds a router, starts application lifecycle hooks, or binds a socket.

## Crates and boundaries

- `mads-core` owns application construction, providers, lifecycle, diagnostics,
  and route catalog inputs. It intentionally has no Axum, Tower, Hyper, HTTP,
  or `mads-common` dependency.
- `mads-common` is the v0.3 Axum adapter. It owns route validation, typed route
  registration, extractors, response wrappers, router construction, and
  serving.
- `mads` is the stable facade. Its default `common` feature exposes the HTTP
  runtime; disable default features for the core-only boundary.

```toml
[dependencies]
mads = "0.3"
serde = { version = "1", features = ["derive"] }
```

MADS.rs supports Rust 1.85 and uses Rust edition 2024.

## A typed HTTP route

`#[mads::routes]` records immutable metadata and emits a typed registration
adapter. `#[mads::controller]` resolves the managed controller from the
application once while the router is built; handlers do not receive manual
`State<AppState>` or perform per-request provider resolution.

```rust,ignore
use mads::prelude::*;

#[derive(Clone, serde::Serialize)]
struct User {
    id: u64,
}

#[mads::routes(prefix = "/users")]
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
async fn main() {
    let application = Mads::builder().build().await?;
    let router = build_router(&application)?;

    // Compose `router` directly with Axum or Tower, or serve the application.
    let _ = router;
    serve(application, "127.0.0.1:3000").await?;
    Ok::<(), Box<dyn std::error::Error>>(())
}
```

`build_router(&application)` is the entry point for in-process tests and
native router composition. `serve(application, address)` validates, starts
lifecycle hooks, binds the listener, serves requests, and attempts lifecycle
shutdown on every path after startup.

## Extractors and responses

The prelude exports the standard HTTP types:

- `Path<T>`, `Query<T>`, `Json<T>`, `Header<T>`, and `Request` are direct Axum
  or axum-extra extractors. Native extractors are also supported through
  `mads::common::axum`.
- `HttpResult<T>` is `Result<T, HttpError>` for handler delivery errors.
  Framework construction and bootstrap continue to use `mads::core::Result`;
  the bare core `Result` is deliberately not in the prelude.
- `Created<T>` responds with 201, `NoContent` with an empty 204 response, and
  `HttpError::{bad_request, not_found, conflict, internal}` return stable JSON
  errors with 400, 404, 409, and 500 statuses respectively.

The public `mads::common::axum` re-export is an intentional escape hatch for
native `Router`, `IntoResponse`, extractors, middleware, and response APIs.
Use Tower layers and services directly when a framework wrapper would get in
the way.

## Route and HTTP policy

MADS route metadata uses `/:parameter`; the validated adapter translates it to
Axum 0.8 syntax only while registering the route. The adapter keeps Axum path
checks enabled. Invalid metadata and conflicts fail with `MADS030` before
router construction.

- GET also handles HEAD, with Axum removing the response body.
- MADS does not synthesize OPTIONS handlers; unsupported methods use Axum's
  405 response and `Allow` header, including HEAD when GET exists.
- Static and parameter routes may coexist, and a static route wins.
- `/users` and `/users/` are different request paths. Declarations with a
  non-root trailing slash are rejected; v0.3 does not redirect or normalize.
- Missing paths retain Axum's default 404 behavior.

## Test a router in-process

Use Tower's `ServiceExt::oneshot` against the state-complete router. This tests
the real generated adapter without opening a TCP listener:

```rust,ignore
use axum::{body::Body, http::Request};
use mads::prelude::*;
use tower::ServiceExt;

let application = Mads::builder().build().await?;
let response = build_router(&application)?
    .oneshot(Request::builder().uri("/users/7").body(Body::empty())?)
    .await?;
assert_eq!(response.status(), axum::http::StatusCode::OK);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Current scope

Version 0.3 provides executable HTTP routing, the basic response model, and
direct Axum/Tower composition. It does not provide persistence, Diesel
integration, database configuration, request validation/error normalization,
custom domain-error registries, middleware abstractions, automatic
trailing-slash redirects, or generated OPTIONS handlers. Persistence and the
broader error policy remain future work rather than available behavior.

## Development

Run the release checks locally:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
cargo test --workspace --all-features --doc
cargo llvm-cov --workspace --all-features --ignore-filename-regex '(^|/)tests/ui/' --fail-under-lines 85
```

The MSRV gate uses the lockfile resolution:

```sh
cargo +1.85.0 test --locked --workspace --all-features
```
