# MADS.rs

MADS.rs is a layered Rust workspace for a clean-architecture application
foundation. Version 0.1 intentionally favors explicit construction over implicit
dependency-graph behavior.

## Crate diagram

```text
mads-cli ──> mads ──> mads-core ──> mads-core-macros
                  ├─> mads-common (optional, reserved)
                  └─> mads-extra  (optional, reserved)

mads-common-macros (reserved proc-macro boundary)
```

- `mads-core` provides framework-neutral application construction, lifecycle
  contracts, configuration, provider metadata, and canonical core attributes.
- `mads-core-macros` implements the core procedural attributes.
- `mads-common` and `mads-common-macros` reserve future standard backend and
  route-integration surfaces.
- `mads-extra` reserves future post-v1 extension capabilities.
- `mads` is the stable public facade; applications should depend on this crate.
- `mads-cli` installs the `mads` development command.

The facade enables `common` and `runtime-tokio` by default. Disable default
features to depend on only the core boundary:

```toml
[dependencies]
mads = { version = "0.1", default-features = false }
```

MADS.rs supports Rust 1.85 and uses the Rust 2024 edition.

## Application construction

Use the facade-qualified attributes as the canonical macro syntax:
`#[mads::main]`, `#[mads::module]`, `#[mads::provider]`,
`#[mads::repository]`, and `#[mads::service]`. The `mads::prelude` module also
collects these attributes with the application-facing core types for ergonomic
imports.

In v0.1, construction is deliberately explicit: create configuration, insert
the dependencies a managed provider needs, construct that provider, build the
application, then start and shut it down.

```rust
use mads::prelude::*;

#[derive(Clone)]
struct Greeting(String);

#[mads::service]
struct Greeter {
    greeting: Greeting,
}

#[mads::main]
async fn main() {
    let config = ConfigBuilder::new()
        .build()
        .expect("an empty configuration is valid");
    let mut builder = Mads::builder_with_config(config);

    builder
        .provide(Greeting("hello".to_owned()))
        .expect("the greeting dependency should be inserted");
    builder
        .construct::<Greeter>()
        .await
        .expect("the managed provider should be constructed explicitly");

    let mut application = builder.build();
    application.start().await.expect("the application should start");
    application
        .shutdown()
        .await
        .expect("the application should shut down");
}
```

## CLI foundation

Use the CLI to report the implemented and reserved boundaries:

```bash
mads foundation
```

The command reports `core: available`, while `common` and `extra` remain
reserved. Run `mads --help` for the complete foundation command surface.

## Deferred features

Version 0.1 does not yet provide HTTP routing, database integrations, automatic
dependency-graph construction, or route/database-specific attributes. Those
surfaces remain intentionally reserved rather than exposed as incomplete APIs.

## Development

Use the configured stable Rust toolchain and run:

```bash
cargo check --workspace --all-features
```
