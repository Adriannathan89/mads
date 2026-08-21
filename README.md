# MADS.rs

MADS.rs is a layered Rust workspace for a clean-architecture application
foundation. Version 0.2 provides deterministic, side-effect-free provider-graph
analysis and automatic application-scoped construction.

## Crate diagram

```text
mads-cli ──> mads ──> mads-core ──> mads-core-macros
                  ├─> mads-common (optional, contract macros)
                  └─> mads-extra  (optional, reserved)

mads-common ──> mads-common-macros
```

- `mads-core` provides framework-neutral application construction, lifecycle
  contracts, configuration, provider metadata, and canonical core attributes.
- `mads-core-macros` implements the core procedural attributes.
- `mads-common` and `mads-common-macros` provide compile-time route contracts,
  deterministic static route metadata, and managed controllers. HTTP execution
  remains reserved for v0.3.
- `mads-extra` reserves future post-v1 extension capabilities.
- `mads` is the stable public facade; applications should depend on this crate.
- `mads-cli` installs the `mads` development command.

The facade enables `common` and `runtime-tokio` by default. Disable default
features to depend on only the core boundary:

```toml
[dependencies]
mads = { version = "0.2", default-features = false }
```

MADS.rs supports Rust 1.85 and uses the Rust 2024 edition.

## Application construction

Use the facade-qualified attributes as the canonical macro syntax:
`#[mads::main]`, `#[mads::module]`, `#[mads::provider]`,
`#[mads::repository]`, and `#[mads::service]`. With the default `common`
feature, it also exports `#[mads::routes]`, `#[mads::controller]`, and the HTTP
verb attributes. The `mads::prelude` module collects these attributes with the
application-facing core types for ergonomic imports.

MADS.rs analyzes the complete static provider catalog before construction. It
validates duplicate bindings, ambiguous outputs, unresolved dependencies, and
cycles without invoking constructors. A valid graph is then constructed
sequentially in deterministic dependency order.

```rust,ignore
use mads::prelude::*;

#[derive(Clone)]
struct Greeting(String);

#[mads::service]
struct Greeter {
    greeting: Greeting,
}

#[mads::main]
async fn main() {
    let mut builder = Mads::builder();

    builder
        .provide(Greeting("hello".to_owned()))
        .expect("the external value should be inserted");

    let mut application = builder
        .build()
        .await
        .expect("the complete provider graph should validate and build");
    application.start().await.expect("the application should start");
    application
        .shutdown()
        .await
        .expect("the application should shut down");
}
```

A controller can depend on any number of managed services or use cases. Route
traits make the controller contract compiler-checked, retain canonical
method/path metadata, and reject conflicts before controller construction:

```rust,ignore
use mads::prelude::*;

#[service]
struct GetUserUsecase;

#[service]
struct DeleteUserUsecase;

#[routes(prefix = "/users")]
trait UserRoutes {
    #[get("/:id")]
    async fn get_user(&self, id: i64) -> Result<i64>;

    #[delete("/:id")]
    async fn delete_user(&self, id: i64) -> Result<()>;
}

#[controller(routes = [UserRoutes])]
struct UserController {
    get_user: GetUserUsecase,
    delete_user: DeleteUserUsecase,
}

impl UserRoutes for UserController {
    async fn get_user(&self, id: i64) -> Result<i64> {
        Ok(id)
    }

    async fn delete_user(&self, _id: i64) -> Result<()> {
        Ok(())
    }
}

// `build().await` constructs these providers in dependency order.
```

## Graph analysis and inspection

Call `builder.analyze()` to inspect the complete catalog without running a
constructor. `GraphAnalysis` exposes immutable provider nodes, resolved
dependency edges, diagnostics, and a deterministic `ConstructionPlan` when the
graph is valid. After a successful build, use `application.graph()` and
`application.construction_plan()` to inspect the validated graph and the plan
that ran.

Every unambiguous provider is included, even when no other provider references
it. Invalid duplicate or ambiguous declarations are represented by diagnostics
rather than individual effective provider nodes. Values inserted with `provide`
override one matching static provider and are recorded as public
application-wide values. `construct::<T>()` remains a manual escape hatch;
manually constructed providers are not constructed again by `build()`.

Provider declaration visibility is recorded as descriptive metadata: `pub`
providers are public and inherited or restricted visibility is private. MADS.rs
does not enforce visibility until module semantics are introduced.

The graph and construction diagnostics are stable:

- `MADS001`: exact duplicate provider descriptor.
- `MADS002`: ambiguous provider binding.
- `MADS003`: unresolved dependency.
- `MADS005`: dependency cycle.
- `MADS006`: provider construction failure.

## CLI foundation

Use the CLI to report the implemented and reserved boundaries:

```bash
mads foundation
```

The command reports the core and common contract surfaces as available. The
graph data model and runtime analysis are implemented, but final `mads graph`
rendering is deferred. The common HTTP runtime and `extra` remain reserved. Run
`mads --help` for the complete foundation command surface.

## Deferred features

Version 0.2 does not yet provide HTTP execution, Axum registration, extractors,
Diesel or other database integrations, module imports or exports, trait
bindings, qualifiers, additional scopes, or final CLI graph rendering. Route
metadata is available through `mads::common::RouteCatalog`; runtime routing
remains reserved for v0.3.

## Development

CI uses stable Rust for formatting, linting, tests, documentation, and coverage.
Before submitting a change, run the same stable checks locally:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
cargo tree -p mads-core --edges normal
cargo llvm-cov --workspace --all-features --ignore-filename-regex '(^|/)tests/ui/' --fail-under-lines 85
```

The coverage command requires `cargo-llvm-cov` and the
`llvm-tools-preview` Rust component. It enforces at least 85 percent line
coverage while excluding only trybuild UI fixtures.

CI separately verifies the minimum supported Rust version (MSRV), Rust 1.85.0.
When that toolchain is installed locally, run:

```sh
rustup run 1.85.0 cargo test --workspace --all-features
```
