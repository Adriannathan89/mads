# MADS.rs Phase 0 and v0.1 Foundation Design

## Purpose

This specification defines the initial implementation of MADS.rs. It covers Phase 0, Repository and Architecture Foundation, and v0.1.0, Core Runtime Foundation, from `final_timelinev1.md`.

The implementation establishes a clean, compiler-enforced workspace architecture and a small framework-neutral runtime. It intentionally stops before the v0.2 dependency graph and before the v0.3 HTTP runtime.

## Goals

The foundation must provide:

- a Rust 2024 workspace with MSRV 1.85;
- compiler-enforced dependency direction;
- a public `mads` facade and prelude;
- deterministic static metadata for modules and managed providers;
- `#[mads::main]`, `#[mads::module]`, `#[mads::service]`, `#[mads::repository]`, and `#[mads::provider]`;
- a basic application-scoped provider registry;
- explicit v0.1 construction and bootstrap operations;
- configuration and lifecycle foundations;
- structured framework diagnostics;
- stable, MSRV, lint, documentation, test, and coverage automation;
- Rust documentation on every Rust source file and every public item.

## Non-Goals

The implementation must not include:

- dependency graph planning or automatic topological construction;
- missing-edge or dependency-cycle validation;
- Axum, HTTP routing, route macros, or HTTP extractors;
- Diesel, database pools, or migrations;
- `mads.toml`, typed configuration derives, or secret handling;
- request or transient provider scopes;
- Redis, caching, or rate limiting;
- a dynamic runtime-reflection container.

Those capabilities remain assigned to their milestones in `final_timelinev1.md`.

## Workspace Architecture

The workspace contains these crates:

```text
crates/
├── mads/                 Public facade and prelude
├── mads-core/            Framework-neutral runtime and application semantics
├── mads-core-macros/     Core procedural-macro implementations
├── mads-common/          Standard backend integration boundary
├── mads-common-macros/   Reserved HTTP procedural-macro implementation boundary
├── mads-cli/             Developer CLI foundation
└── mads-extra/           Reserved post-v1 capability boundary
```

The dependency rules are:

- `mads-core` does not depend on Axum, Diesel, `mads-common`, or `mads-extra`;
- procedural-macro crates contain parsing and generation, not runtime semantics;
- generated core macro code targets public or doc-hidden contracts in `mads-core`;
- `mads-common` depends inward on `mads-core`;
- `mads-extra` may depend inward on core abstractions but is disabled by default;
- `mads-cli` is an outer adapter and may depend on the public framework surface;
- `mads` composes and re-exports the supported public API.

The common, common-macros, and extra crates are documented boundary shells during this milestone. They do not expose no-op route, database, cache, or policy APIs.

## Public API and Macro Ownership

Canonical documentation uses fully qualified attributes:

```rust
#[mads::module]
pub struct AppModule;

#[mads::repository]
pub struct UserRepository {
    database: Database,
}

#[mads::service]
pub struct UserService {
    users: UserRepository,
}
```

Core attributes are implemented in `mads-core-macros`, re-exported from `mads-core`, and re-exported again from `mads`. This is the Rust equivalent of a package barrel export: implementation is physically valid for procedural macros while public ownership remains conceptual.

Bare attributes are also available after `use mads::prelude::*`, but they are not the canonical documentation style.

`mads-common-macros` owns future `#[get]`, `#[post]`, `#[put]`, `#[patch]`, and `#[delete]` implementations. These are not implemented as placeholders in v0.1.

## Core Components

`mads-core` is divided into focused units:

```text
descriptor    Static module, provider, and dependency metadata
catalog       Inventory collection and deterministic ordering
registry      Type-indexed application-scoped provider storage
context       Read-only provider and configuration access
builder       Explicit v0.1 construction and bootstrap operations
lifecycle     Application states and startup/shutdown hooks
config        Configuration source and merge foundation
diagnostic    Structured diagnostics, framework errors, and Result
runtime       Optional Tokio-backed main bootstrap
```

Each unit has a narrow public contract and can be tested without knowing its internals.

## Metadata and Discovery

The core attribute macros generate static metadata for managed types and functions. Provider metadata contains:

- provider kind: service, repository, or provider;
- stable Rust type name;
- concrete `TypeId` access through a function;
- declared dependency type metadata;
- source file, line, and column where available;
- a generated constructor entry point where the declaration supports construction.

Descriptors are submitted through `inventory`. Collection is sorted explicitly by stable descriptor fields, so linker registration order never becomes observable behavior.

`#[mads::module]` submits module metadata. v0.1 supports the unit-struct declaration required by the core foundation. Module imports, exports, visibility, route prefixes, and module graph validation remain scheduled for v0.6.

## Managed Provider Construction

`#[mads::service]` and `#[mads::repository]` support non-generic structs with named dependency fields, plus dependency-free unit structs. They generate:

- deterministic dependency metadata in declaration order;
- a doc-hidden construction implementation;
- application-scoped shared-handle behavior;
- inventory registration;
- compile-time diagnostics for unsupported declaration shapes.

`#[mads::provider]` supports synchronous or asynchronous free functions with concrete dependency parameters and a concrete return type. Provider results may be direct values or the MADS result form. The macro generates equivalent descriptor and constructor metadata.

The v0.1 builder performs construction explicitly. It does not infer a safe global construction order. This keeps the milestone honest while ensuring v0.2 can consume the existing descriptors to create a topological plan without redesigning the macro surface.

## Registry and Application Context

The provider registry stores one value per concrete Rust `TypeId` as an `Arc<dyn Any + Send + Sync>`. It:

- returns typed shared handles;
- rejects duplicate insertion;
- reports unresolved lookup as a structured MADS error;
- requires managed values to be `Send + Sync + 'static`;
- becomes immutable when the application enters the running state.

`ApplicationContext` provides read-only access to the frozen registry and merged configuration. Framework internals may retain shared ownership, while normal future application handler signatures remain free of visible `Arc<T>` and `State<T>` plumbing.

## Lifecycle

The application lifecycle is:

```text
Created → Starting → Running → Stopping → Stopped
```

Invalid transitions return structured errors. Startup hooks execute in registration order. Shutdown hooks execute in reverse registration order. If startup fails, already-started resources are shut down before the original startup error is returned.

Only application scope exists in v0.1. Request and transient lifecycles are explicit non-goals.

## Runtime Feature

`mads-core` exposes Tokio-backed bootstrap only through a `runtime-tokio` feature. The `mads` facade enables that feature by default.

`#[mads::main]` validates that it is attached to a supported async main function and expands through the core runtime contract. `mads-core` remains free from Axum and HTTP assumptions. Consumers that do not want the default runtime can disable facade default features.

## Configuration Foundation

The configuration subsystem provides:

- a `ConfigSource` contract;
- deterministic source merging;
- later sources overriding earlier sources;
- programmatic key/value sources;
- an environment source with the `MADS_` prefix;
- retained source names for diagnostics.

File configuration, interpolation, typed derives, validation schemas, and secret-safe values are deferred. This milestone establishes contracts rather than a large configuration product.

## Diagnostics and Error Handling

Framework operations return `mads_core::Result<T>`. Errors contain a structured diagnostic with:

- a stable MADS code;
- title and explanatory message;
- optional subject or type;
- optional source location;
- optional underlying cause;
- zero or more suggestions.

Initial diagnostics cover duplicate providers, unresolved providers, invalid lifecycle transitions, lifecycle hook failures, and configuration source failures. Compiler errors produced by procedural macros identify the unsupported source construct and a supported alternative.

Errors implement Rust's standard error contracts and preserve source chains when an underlying error exists.

## CLI Foundation

`mads-cli` provides a minimal executable with help, version output, and a foundation health/check command. It does not claim future commands such as graph inspection, route inspection, doctor, development watching, or database migration support.

## Documentation Policy

Every Rust source file begins with meaningful `//!` crate or module documentation. Every public item has `///` documentation. Publishable crates enable `#![deny(missing_docs)]`, and generated implementation details are hidden from rustdoc.

Workspace documentation explains:

- crate boundaries and dependency direction;
- supported features and default features;
- the explicit v0.1 construction boundary;
- supported macro inputs and compile diagnostics;
- deferred milestone functionality.

Examples in documentation are compiled as doctests where practical.

## Testing Strategy

Implementation follows test-driven development.

Unit tests cover:

- registry insertion and typed resolution;
- duplicate and unresolved provider diagnostics;
- deterministic catalog ordering;
- configuration precedence and source attribution;
- lifecycle transitions, ordering, rollback, and failure behavior;
- diagnostic formatting.

Procedural-macro tests use `trybuild` for supported declarations and invalid service, repository, provider, module, and main inputs.

Integration tests cover:

- inventory discovery across crates;
- explicit provider construction;
- application-scoped shared identity;
- facade and prelude re-exports;
- Tokio runtime feature behavior;
- CLI foundation behavior.

Generated expansion and compile-test fixtures are excluded from the LLVM line-coverage denominator where instrumentation cannot represent them consistently. Their behavior remains enforced by compile tests.

## Continuous Integration

GitHub Actions runs:

```text
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test with Rust 1.85
cargo doc --workspace --all-features with rustdoc warnings denied
cargo llvm-cov --workspace --all-features --fail-under-lines 85
```

The workspace uses Edition 2024 and declares `rust-version = "1.85"`. Dependency versions must remain compatible with that MSRV.

## Acceptance Criteria

Phase 0 and v0.1 are complete when:

1. The workspace builds and tests on stable Rust and Rust 1.85.
2. Cargo manifests enforce the approved dependency direction.
3. All supported core macros generate deterministic metadata.
4. Invalid macro declarations produce focused compiler diagnostics.
5. The registry and explicit builder path retain application-scoped providers.
6. Configuration precedence and lifecycle behavior match this specification.
7. The `mads` facade exposes canonical `#[mads::...]` attributes.
8. Every Rust file and public API is documented.
9. Formatting, Clippy, tests, doctests, and rustdoc checks pass.
10. Workspace LLVM line coverage is at least 85 percent.
11. HTTP, Diesel, dependency graph planning, and post-v1 policies have not leaked into the foundation.

## Evolution to v0.2

The v0.2 graph engine will consume the provider and dependency descriptors established here. It will add provider nodes, dependency edges, deterministic topological construction, missing and duplicate validation, ambiguous provider detection, and cycle diagnostics.

The v0.1 public macro syntax and descriptor meaning must not need to change for that work.
