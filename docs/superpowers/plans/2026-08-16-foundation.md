# MADS.rs Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the compiler-enforced MADS.rs workspace foundation and functional v0.1 core runtime defined by the approved foundation specification.

**Architecture:** A layered Cargo workspace keeps framework-neutral semantics in `mads-core`, procedural syntax in dedicated macro crates, outward integrations in `mads-common`, and ergonomic exports in `mads`. Static `inventory` descriptors feed an explicit v0.1 builder; v0.2 will later consume the same metadata for graph planning.

**Tech Stack:** Rust 2024, Rust 1.85 MSRV, Cargo resolver 3, `inventory`, `syn`, `quote`, `proc-macro2`, `proc-macro-crate`, Tokio, `trybuild`, GitHub Actions, and `cargo-llvm-cov`.

## Global Constraints

- Use Edition 2024 and declare `rust-version = "1.85"` in workspace package metadata.
- `mads-core` must not depend on Axum, Diesel, `mads-common`, or `mads-extra`.
- Keep Tokio behind `mads-core/runtime-tokio`; enable it through the `mads` facade default features.
- Canonical attributes are fully qualified: `#[mads::main]`, `#[mads::module]`, `#[mads::service]`, `#[mads::repository]`, and `#[mads::provider]`.
- Do not implement route macros, HTTP, Diesel, graph planning, request/transient scopes, Redis, caching, or rate limiting.
- Every Rust file starts with meaningful `//!` documentation; every public item has `///` documentation.
- Publishable crates use `#![deny(missing_docs)]` and `#![forbid(unsafe_code)]`.
- Use `apply_patch` for hand-written file edits; use `cargo init --vcs none` only for initial crate scaffolding.
- Follow red-green-refactor: observe each new test fail before implementing its behavior.
- Maintain at least 85 percent workspace LLVM line coverage.
- The mounted `.git/` path is not writable or usable. Run repository operations as `git --git-dir=.git-data --work-tree=.`.

## Planned File Map

```text
Cargo.toml                              Workspace members, shared metadata, dependencies, and lints
rust-toolchain.toml                     Stable default toolchain and required components
.gitignore                              Rust build and coverage artifacts
README.md                               Foundation scope, crate map, and quick start
.github/workflows/ci.yml                Stable, MSRV, rustdoc, and coverage gates

crates/mads/src/lib.rs                  Public facade, canonical macro exports, and prelude
crates/mads/tests/facade.rs             Facade and prelude integration tests
crates/mads/tests/ui.rs                 trybuild harness
crates/mads/tests/ui/pass/*.rs          Supported macro declarations
crates/mads/tests/ui/fail/*.rs          Rejected macro declarations
crates/mads/tests/ui/fail/*.stderr      Approved compiler diagnostics

crates/mads-core/src/lib.rs             Core crate exports and private macro support exports
crates/mads-core/src/diagnostic.rs      Diagnostic codes, structured errors, and Result
crates/mads-core/src/config.rs          Configuration sources, values, and merge builder
crates/mads-core/src/registry.rs        Type-indexed application-scoped storage
crates/mads-core/src/context.rs         Construction and immutable application contexts
crates/mads-core/src/descriptor.rs      Module/provider/dependency descriptor contracts
crates/mads-core/src/catalog.rs         Inventory collection and deterministic lookup
crates/mads-core/src/lifecycle.rs       State machine and async lifecycle hooks
crates/mads-core/src/builder.rs         Explicit construction builder and Mads application
crates/mads-core/src/runtime.rs         Tokio bootstrap behind `runtime-tokio`
crates/mads-core/tests/*.rs             Core behavior integration tests

crates/mads-core-macros/src/lib.rs      Proc-macro entry points
crates/mads-core-macros/src/path.rs     Direct-core versus facade expansion path resolution
crates/mads-core-macros/src/module.rs   Module descriptor generation
crates/mads-core-macros/src/managed.rs  Service/repository handle and constructor generation
crates/mads-core-macros/src/provider.rs Provider-function descriptor and constructor generation
crates/mads-core-macros/src/main.rs     Async main validation and runtime expansion

crates/mads-common/src/lib.rs           Documented standard-integration boundary shell
crates/mads-common-macros/src/lib.rs    Documented future HTTP macro boundary shell
crates/mads-extra/src/lib.rs            Documented post-v1 capability boundary shell
crates/mads-cli/src/main.rs             Version/help/foundation-check CLI
crates/mads-cli/tests/cli.rs             CLI process integration tests
```

---

### Task 1: Bootstrap the Cargo workspace and enforce crate boundaries

**Files:**
- Create: `Cargo.toml`
- Create: `rust-toolchain.toml`
- Create: `.gitignore`
- Create: `README.md`
- Create: `crates/mads/Cargo.toml`
- Create: `crates/mads/src/lib.rs`
- Create: `crates/mads-core/Cargo.toml`
- Create: `crates/mads-core/src/lib.rs`
- Create: `crates/mads-core-macros/Cargo.toml`
- Create: `crates/mads-core-macros/src/lib.rs`
- Create: `crates/mads-common/Cargo.toml`
- Create: `crates/mads-common/src/lib.rs`
- Create: `crates/mads-common-macros/Cargo.toml`
- Create: `crates/mads-common-macros/src/lib.rs`
- Create: `crates/mads-cli/Cargo.toml`
- Create: `crates/mads-cli/src/main.rs`
- Create: `crates/mads-extra/Cargo.toml`
- Create: `crates/mads-extra/src/lib.rs`

**Interfaces:**
- Consumes: Approved workspace dependency diagram and MSRV policy.
- Produces: Seven buildable workspace packages; workspace lint inheritance; `mads-core/runtime-tokio`; facade default features.

- [ ] **Step 1: Verify the workspace is absent**

Run: `cargo metadata --no-deps --format-version 1`

Expected: FAIL because the root does not contain `Cargo.toml`.

- [ ] **Step 2: Create crate scaffolds with Cargo**

Run:

```bash
cargo init --lib --name mads --vcs none crates/mads
cargo init --lib --name mads-core --vcs none crates/mads-core
cargo init --lib --name mads-core-macros --vcs none crates/mads-core-macros
cargo init --lib --name mads-common --vcs none crates/mads-common
cargo init --lib --name mads-common-macros --vcs none crates/mads-common-macros
cargo init --bin --name mads-cli --vcs none crates/mads-cli
cargo init --lib --name mads-extra --vcs none crates/mads-extra
```

Expected: each command creates one package without creating nested Git metadata.

- [ ] **Step 3: Define workspace metadata, dependencies, and lints**

Write root `Cargo.toml` with this contract:

```toml
[workspace]
resolver = "3"
members = [
    "crates/mads",
    "crates/mads-core",
    "crates/mads-core-macros",
    "crates/mads-common",
    "crates/mads-common-macros",
    "crates/mads-cli",
    "crates/mads-extra",
]

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.85"
authors = ["MADS.rs contributors"]
repository = "https://github.com/mads-rs/mads"

[workspace.dependencies]
inventory = "0.3"
proc-macro-crate = "3"
proc-macro2 = "1"
quote = "1"
syn = { version = "2", features = ["full", "extra-traits"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread", "sync"] }
trybuild = "1"

[workspace.lints.rust]
missing_docs = "deny"
unsafe_code = "forbid"

[workspace.lints.clippy]
all = "deny"
```

Each package inherits `version`, `edition`, `rust-version`, `authors`, and `repository`, and contains `[lints] workspace = true`.

- [ ] **Step 4: Set package dependencies and features**

Use these manifest edges:

```text
mads-core          → inventory; optional tokio; mads-core-macros re-export
mads-core-macros   → proc-macro2, quote, syn, proc-macro-crate
mads-common        → mads-core
mads               → mads-core, optional mads-common, optional mads-extra
mads-cli           → mads
mads-extra         → mads-core
mads-common-macros → proc-macro crate with no exported attributes yet
```

Set `mads-core` features to:

```toml
[features]
default = []
runtime-tokio = ["dep:tokio"]
```

Set `mads` features to:

```toml
[features]
default = ["common", "runtime-tokio"]
common = ["dep:mads-common"]
runtime-tokio = ["mads-core/runtime-tokio"]
extra = ["dep:mads-extra"]
```

Set the `mads-cli` binary target explicitly so the package remains `mads-cli` while the installed command is `mads`:

```toml
[[bin]]
name = "mads"
path = "src/main.rs"
```

- [ ] **Step 5: Replace generated sources with documented crate shells**

Every `lib.rs` and `main.rs` starts with `//!`. Library crates contain:

```rust
#![deny(missing_docs)]
#![forbid(unsafe_code)]
```

`mads-common`, `mads-common-macros`, and `mads-extra` explicitly document that their scheduled APIs are not implemented in v0.1. The `mads` binary prints `mads 0.1.0` until its command parser is added.

- [ ] **Step 6: Add toolchain and ignore policy**

Write `rust-toolchain.toml`:

```toml
[toolchain]
channel = "stable"
components = ["clippy", "llvm-tools-preview", "rustfmt"]
profile = "minimal"
```

Write `.gitignore` entries for `/target/`, `/coverage/`, `*.profraw`, and `.DS_Store`. Do not add `.git-data/`; it is already excluded by the alternate repository's local exclude file.

- [ ] **Step 7: Verify metadata and the initial build**

Run:

```bash
cargo metadata --no-deps --format-version 1
cargo check --workspace --all-features
cargo tree -p mads-core --edges normal
```

Expected: all commands pass, and the `mads-core` tree contains neither `mads-common`, `mads-extra`, Axum, nor Diesel.

- [ ] **Step 8: Commit the workspace foundation**

```bash
git --git-dir=.git-data --work-tree=. add Cargo.toml rust-toolchain.toml .gitignore README.md crates docs final_timelinev1.md
git --git-dir=.git-data --work-tree=. commit -m "build: bootstrap layered MADS workspace"
```

---

### Task 2: Add structured diagnostics and framework errors

**Files:**
- Create: `crates/mads-core/src/diagnostic.rs`
- Create: `crates/mads-core/tests/diagnostic.rs`
- Modify: `crates/mads-core/src/lib.rs`

**Interfaces:**
- Consumes: Workspace lint policy from Task 1.
- Produces: `DiagnosticCode`, `SourceLocation`, `Diagnostic`, `Error`, `Result<T>`, and initial diagnostic constants.

- [ ] **Step 1: Write failing diagnostic behavior tests**

Create `crates/mads-core/tests/diagnostic.rs` with module docs and assertions equivalent to:

```rust
//! Integration tests for structured MADS diagnostics.

use mads_core::{Diagnostic, Error, MADS001, SourceLocation};

#[test]
fn renders_a_structured_diagnostic() {
    let diagnostic = Diagnostic::new(MADS001, "duplicate provider", "UserService is registered twice")
        .with_subject("UserService")
        .with_location(SourceLocation::new("src/users.rs", 12, 3))
        .with_suggestion("remove one provider declaration");
    let error = Error::new(diagnostic);

    assert_eq!(error.code(), MADS001);
    assert!(error.to_string().contains("error[MADS001]: duplicate provider"));
    assert!(error.to_string().contains("src/users.rs:12:3"));
    assert!(error.to_string().contains("help: remove one provider declaration"));
}
```

- [ ] **Step 2: Run the test and observe the missing API**

Run: `cargo test -p mads-core --test diagnostic`

Expected: FAIL with unresolved imports from `mads_core`.

- [ ] **Step 3: Implement the diagnostic model**

Implement these exact public contracts in `diagnostic.rs`:

```rust
pub struct DiagnosticCode(&'static str);

impl DiagnosticCode {
    pub const fn new(value: &'static str) -> Self;
    pub const fn as_str(self) -> &'static str;
}

pub const MADS001: DiagnosticCode = DiagnosticCode::new("MADS001");
pub const MADS003: DiagnosticCode = DiagnosticCode::new("MADS003");
pub const MADS004: DiagnosticCode = DiagnosticCode::new("MADS004");
pub const MADS010: DiagnosticCode = DiagnosticCode::new("MADS010");
pub const MADS011: DiagnosticCode = DiagnosticCode::new("MADS011");
pub const MADS020: DiagnosticCode = DiagnosticCode::new("MADS020");

pub struct SourceLocation {
    pub file: &'static str,
    pub line: u32,
    pub column: u32,
}

pub struct Diagnostic {
    code: DiagnosticCode,
    title: String,
    message: String,
    subject: Option<String>,
    location: Option<SourceLocation>,
    suggestions: Vec<String>,
}

impl Diagnostic {
    pub fn new(code: DiagnosticCode, title: impl Into<String>, message: impl Into<String>) -> Self;
    pub fn with_subject(self, subject: impl Into<String>) -> Self;
    pub fn with_location(self, location: SourceLocation) -> Self;
    pub fn with_suggestion(self, suggestion: impl Into<String>) -> Self;
    pub const fn code(&self) -> DiagnosticCode;
}

pub struct Error {
    diagnostic: Diagnostic,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl Error {
    pub fn new(diagnostic: Diagnostic) -> Self;
    pub fn with_source<E>(diagnostic: Diagnostic, source: E) -> Self
    where E: std::error::Error + Send + Sync + 'static;
    pub const fn code(&self) -> DiagnosticCode;
    pub const fn diagnostic(&self) -> &Diagnostic;
}

pub type Result<T> = std::result::Result<T, Error>;
```

Derive or implement `Copy`, `Clone`, `Debug`, `Eq`, `Hash`, and display behavior where appropriate. Implement `std::error::Error` for `Error`, returning its optional cause from `source()`.

- [ ] **Step 4: Add focused tests for source chaining and multi-suggestion formatting**

Add one local test error implementing `Display` and `std::error::Error`. Assert `std::error::Error::source(&error).is_some()`, and assert suggestions render one `help:` line each in insertion order.

- [ ] **Step 5: Run diagnostics tests and Clippy**

Run:

```bash
cargo test -p mads-core --test diagnostic
cargo clippy -p mads-core --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 6: Commit diagnostics**

```bash
git --git-dir=.git-data --work-tree=. add crates/mads-core/src crates/mads-core/tests/diagnostic.rs
git --git-dir=.git-data --work-tree=. commit -m "feat(core): add structured diagnostics"
```

---

### Task 3: Implement deterministic configuration sources

**Files:**
- Create: `crates/mads-core/src/config.rs`
- Create: `crates/mads-core/tests/config.rs`
- Modify: `crates/mads-core/src/lib.rs`

**Interfaces:**
- Consumes: `mads_core::{Diagnostic, Error, Result, MADS020}`.
- Produces: `ConfigSource`, `MapSource`, `EnvSource`, `ConfigValue`, `Config`, and `ConfigBuilder`.

- [ ] **Step 1: Write failing merge-precedence tests**

Create `crates/mads-core/tests/config.rs`:

```rust
//! Integration tests for deterministic configuration merging.

use mads_core::{ConfigBuilder, EnvSource, MapSource};

#[test]
fn later_sources_override_values_and_retain_attribution() {
    let defaults = MapSource::new("defaults", [("server.port", "3000")]);
    let environment = EnvSource::from_iter(
        "MADS_",
        [("MADS_SERVER__PORT", "8080"), ("IGNORED", "value")],
    );

    let config = ConfigBuilder::new()
        .source(defaults)
        .source(environment)
        .build()
        .expect("configuration should build");

    assert_eq!(config.get("server.port"), Some("8080"));
    assert_eq!(config.source_of("server.port"), Some("environment"));
    assert_eq!(config.get("ignored"), None);
}
```

- [ ] **Step 2: Run the config test and observe missing imports**

Run: `cargo test -p mads-core --test config`

Expected: FAIL because the configuration types do not exist.

- [ ] **Step 3: Implement configuration contracts**

Implement:

```rust
pub trait ConfigSource: Send + Sync {
    fn name(&self) -> &str;
    fn load(&self) -> Result<std::collections::BTreeMap<String, String>>;
}

pub struct MapSource {
    name: String,
    values: BTreeMap<String, String>,
}
pub struct EnvSource {
    prefix: String,
    variables: Vec<(OsString, OsString)>,
}
pub struct ConfigValue {
    value: String,
    source: String,
}
pub struct Config {
    values: BTreeMap<String, ConfigValue>,
}
pub struct ConfigBuilder {
    sources: Vec<Box<dyn ConfigSource>>,
}
```

Expose:

```rust
MapSource::new(name, values)
EnvSource::new(prefix)
EnvSource::from_iter(prefix, variables)
ConfigBuilder::new()
ConfigBuilder::source(source)
ConfigBuilder::build() -> Result<Config>
Config::empty()
Config::get(key) -> Option<&str>
Config::source_of(key) -> Option<&str>
Config::iter()
Config::len() -> usize
Config::is_empty() -> bool
```

Normalize environment keys by stripping `MADS_`, lowercasing ASCII letters, and converting `__` to `.`. Name `EnvSource` as `environment` for attribution. Ignore variables outside the configured prefix and variables that are not valid Unicode.

- [ ] **Step 4: Add failure and deterministic-order tests**

Define a test `ConfigSource` that returns `Error::new(Diagnostic::new(MADS020, "configuration source failed", "broken source could not load"))`; assert `ConfigBuilder::build()` preserves `MADS020`. Assert `Config::iter()` returns keys in lexical order regardless of source insertion order.

- [ ] **Step 5: Verify config behavior and documentation**

Run:

```bash
cargo test -p mads-core --test config
cargo test -p mads-core --doc
cargo clippy -p mads-core --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 6: Commit configuration foundation**

```bash
git --git-dir=.git-data --work-tree=. add crates/mads-core/src crates/mads-core/tests/config.rs
git --git-dir=.git-data --work-tree=. commit -m "feat(core): add configuration source foundation"
```

---

### Task 4: Implement the application-scoped registry and contexts

**Files:**
- Create: `crates/mads-core/src/registry.rs`
- Create: `crates/mads-core/src/context.rs`
- Create: `crates/mads-core/tests/registry.rs`
- Modify: `crates/mads-core/src/lib.rs`

**Interfaces:**
- Consumes: `Config`, `Diagnostic`, `Error`, `Result`, `MADS001`, and `MADS003`.
- Produces: `ErasedProvider`, `ProviderRegistry`, `ConstructionContext<'a>`, and `ApplicationContext`.

- [ ] **Step 1: Write failing registry identity and error tests**

Create tests equivalent to:

```rust
//! Integration tests for application-scoped provider storage.

use std::sync::Arc;

use mads_core::{Config, ProviderRegistry, MADS001, MADS003};

#[derive(Debug)]
struct Counter;

#[test]
fn resolves_the_same_application_scoped_allocation() {
    let mut registry = ProviderRegistry::new();
    registry.insert(Counter).expect("first insertion should work");

    let first = registry.resolve::<Counter>().expect("provider should resolve");
    let second = registry.resolve::<Counter>().expect("provider should resolve");

    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(registry.insert(Counter).unwrap_err().code(), MADS001);
    assert_eq!(registry.resolve::<String>().unwrap_err().code(), MADS003);
}
```

- [ ] **Step 2: Run the registry test and observe missing types**

Run: `cargo test -p mads-core --test registry`

Expected: FAIL with unresolved registry imports.

- [ ] **Step 3: Implement type-erased storage**

Use:

```rust
pub type ErasedProvider = std::sync::Arc<dyn std::any::Any + Send + Sync>;

pub struct ProviderRegistry {
    values: std::collections::HashMap<std::any::TypeId, ErasedProvider>,
}
```

Expose documented methods:

```rust
pub fn new() -> Self;
pub fn insert<T>(&mut self, value: T) -> Result<()>
where T: Send + Sync + 'static;
pub fn insert_erased(&mut self, type_id: TypeId, type_name: &'static str, value: ErasedProvider) -> Result<()>;
pub fn resolve<T>(&self) -> Result<Arc<T>>
where T: Send + Sync + 'static;
pub fn contains<T>(&self) -> bool
where T: Send + Sync + 'static;
pub fn len(&self) -> usize;
pub fn is_empty(&self) -> bool;
```

Duplicate insertion reports `MADS001`; lookup failure reports `MADS003`. An impossible downcast mismatch reports `MADS004` instead of panicking.

- [ ] **Step 4: Implement mutable construction and immutable application contexts**

Define:

```rust
pub struct ConstructionContext<'a> {
    registry: &'a ProviderRegistry,
    config: &'a Config,
}

pub struct ApplicationContext {
    registry: Arc<ProviderRegistry>,
    config: Arc<Config>,
}
```

Both expose `resolve<T>()` and `config()`. Only `ApplicationContext` is cloneable. Its constructor consumes `ProviderRegistry` and `Config`, making the running registry immutable by type rather than by a mutable boolean flag.

- [ ] **Step 5: Add context immutability and config access tests**

Construct an `ApplicationContext`, clone it, assert both clones resolve pointer-equal providers, and assert `context.config().get("server.port")` returns the merged value. Do not expose a mutable registry accessor.

- [ ] **Step 6: Verify registry and context**

Run:

```bash
cargo test -p mads-core --test registry
cargo clippy -p mads-core --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 7: Commit registry and context**

```bash
git --git-dir=.git-data --work-tree=. add crates/mads-core/src crates/mads-core/tests/registry.rs
git --git-dir=.git-data --work-tree=. commit -m "feat(core): add application provider registry"
```

---

### Task 5: Add static descriptors and deterministic inventory catalog

**Files:**
- Create: `crates/mads-core/src/descriptor.rs`
- Create: `crates/mads-core/src/catalog.rs`
- Create: `crates/mads-core/tests/catalog.rs`
- Modify: `crates/mads-core/src/lib.rs`

**Interfaces:**
- Consumes: `ConstructionContext`, `ErasedProvider`, `Result`, `SourceLocation`, and diagnostic codes.
- Produces: `ProviderKind`, `DependencyDescriptor`, `ProviderDescriptor`, `ModuleDescriptor`, `ProviderFuture<'a>`, `ProviderConstructor`, `Catalog`.

- [ ] **Step 1: Write failing deterministic catalog tests**

In `crates/mads-core/tests/catalog.rs`, submit two descriptors in reverse lexical order:

```rust
//! Integration tests for deterministic descriptor discovery.

use std::any::TypeId;

use mads_core::{Catalog, ModuleDescriptor, SourceLocation};

struct Alpha;
struct Zeta;

fn alpha_type_id() -> TypeId { TypeId::of::<Alpha>() }
fn zeta_type_id() -> TypeId { TypeId::of::<Zeta>() }

inventory::submit! {
    ModuleDescriptor::new("zeta::Module", zeta_type_id, SourceLocation::new(file!(), line!(), column!()))
}

inventory::submit! {
    ModuleDescriptor::new("alpha::Module", alpha_type_id, SourceLocation::new(file!(), line!(), column!()))
}

#[test]
fn modules_are_sorted_by_stable_name() {
    let names: Vec<_> = Catalog::modules().into_iter().map(|item| item.type_name()).collect();
    assert_eq!(names, ["alpha::Module", "zeta::Module"]);
}
```

Add `inventory` as a `mads-core` dev dependency if the re-exported private path is intentionally unavailable to tests.

- [ ] **Step 2: Run the catalog test and observe missing descriptor contracts**

Run: `cargo test -p mads-core --test catalog`

Expected: FAIL with unresolved descriptor imports.

- [ ] **Step 3: Implement descriptor contracts**

Define:

```rust
pub enum ProviderKind { Service, Repository, Provider }

pub type ProviderFuture<'a> = Pin<Box<dyn Future<Output = Result<ErasedProvider>> + Send + 'a>>;
pub type ProviderConstructor = for<'a> fn(&'a ConstructionContext<'a>) -> ProviderFuture<'a>;

pub struct DependencyDescriptor {
    type_name: &'static str,
    type_id: fn() -> TypeId,
}

pub struct ProviderDescriptor {
    kind: ProviderKind,
    type_name: &'static str,
    type_id: fn() -> TypeId,
    dependencies: &'static [DependencyDescriptor],
    location: SourceLocation,
    constructor: ProviderConstructor,
}

pub struct ModuleDescriptor {
    type_name: &'static str,
    type_id: fn() -> TypeId,
    location: SourceLocation,
}
```

Provide `const fn new` constructors and getters for every field. Descriptor constructors store `fn() -> TypeId` because `TypeId::of` cannot be evaluated in all required static contexts on MSRV 1.85.

- [ ] **Step 4: Register inventory collection points and catalog lookups**

In `catalog.rs`, call:

```rust
inventory::collect!(ProviderDescriptor);
inventory::collect!(ModuleDescriptor);
```

Expose:

```rust
pub struct Catalog;

impl Catalog {
    pub fn providers() -> Vec<&'static ProviderDescriptor>;
    pub fn modules() -> Vec<&'static ModuleDescriptor>;
    pub fn provider_for<T>() -> Result<&'static ProviderDescriptor>
    where T: Send + Sync + 'static;
}
```

Sort providers by output type name, provider kind, then file/line/column. Sort modules by type name, then file/line/column. `provider_for<T>()` reports `MADS003` for none and `MADS001` for more than one descriptor.

- [ ] **Step 5: Test provider selection and duplicate reporting**

Submit one provider descriptor with a constructor that returns `Arc::new(Alpha)` as `ErasedProvider`; assert `Catalog::provider_for::<Alpha>()` selects it. Submit two descriptors for a separate output type and assert selection returns `MADS001`.

- [ ] **Step 6: Verify catalog behavior**

Run:

```bash
cargo test -p mads-core --test catalog
cargo clippy -p mads-core --all-targets -- -D warnings
```

Expected: PASS with deterministic ordering independent of submit order.

- [ ] **Step 7: Commit descriptors and catalog**

```bash
git --git-dir=.git-data --work-tree=. add crates/mads-core/src crates/mads-core/tests/catalog.rs crates/mads-core/Cargo.toml
git --git-dir=.git-data --work-tree=. commit -m "feat(core): add deterministic metadata catalog"
```

---

### Task 6: Implement lifecycle state, rollback, and application builder

**Files:**
- Create: `crates/mads-core/src/lifecycle.rs`
- Create: `crates/mads-core/src/builder.rs`
- Create: `crates/mads-core/tests/lifecycle.rs`
- Create: `crates/mads-core/tests/builder.rs`
- Modify: `crates/mads-core/src/lib.rs`

**Interfaces:**
- Consumes: `ApplicationContext`, `Catalog`, `Config`, `ConstructionContext`, `Diagnostic`, `ErasedProvider`, `ProviderRegistry`, `Result`, `MADS010`, and `MADS011`.
- Produces: `LifecycleState`, `LifecycleFuture<'a>`, `LifecycleHook`, `LifecycleManager`, `MadsBuilder`, and `Mads`.

- [ ] **Step 1: Write failing lifecycle order and rollback tests**

Use a test hook holding `Arc<Mutex<Vec<String>>>`. Implement:

```rust
impl LifecycleHook for RecordingHook {
    fn name(&self) -> &str { &self.name }
    fn start<'a>(&'a self, _context: &'a ApplicationContext) -> LifecycleFuture<'a>;
    fn stop<'a>(&'a self, _context: &'a ApplicationContext) -> LifecycleFuture<'a>;
}
```

Register hooks `database` then `worker`. Assert successful events equal:

```text
start:database, start:worker, stop:worker, stop:database
```

Make `worker` fail during startup and assert `stop:database` runs before the startup error is returned.

- [ ] **Step 2: Run lifecycle tests and observe missing APIs**

Run: `cargo test -p mads-core --test lifecycle`

Expected: FAIL with unresolved lifecycle imports.

- [ ] **Step 3: Implement lifecycle contracts and state machine**

Define:

```rust
pub enum LifecycleState { Created, Starting, Running, Stopping, Stopped }
pub type LifecycleFuture<'a> = Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;

pub trait LifecycleHook: Send + Sync {
    fn name(&self) -> &str;
    fn start<'a>(&'a self, context: &'a ApplicationContext) -> LifecycleFuture<'a>;
    fn stop<'a>(&'a self, context: &'a ApplicationContext) -> LifecycleFuture<'a>;
}
```

`LifecycleManager::start` only accepts `Created`; `shutdown` only accepts `Running`. Invalid transitions return `MADS010`. Hook failure wraps the hook name in `MADS011`. Startup rollback calls stop only for successfully started hooks and retains the original start failure.

After successful startup the manager is `Running`; after successful shutdown it is `Stopped`. A startup failure that completes rollback also leaves the manager `Stopped`, so partially started resources cannot be restarted accidentally.

- [ ] **Step 4: Write failing explicit builder tests**

Submit descriptors for `Database` and `Repository`. Make the repository constructor resolve `Database`. Test:

```rust
let mut builder = Mads::builder();
builder.provide(Database::new()).expect("database insertion should work");
builder.construct::<Repository>().await.expect("explicit construction should work");
let app = builder.build();
assert!(app.context().resolve::<Repository>().is_ok());
```

Call `construct::<Repository>()` without first providing `Database`; assert `MADS003`. This proves explicit order without adding graph planning.

- [ ] **Step 5: Implement `MadsBuilder` and `Mads`**

Expose:

```rust
impl MadsBuilder {
    pub fn new(config: Config) -> Self;
    pub fn provide<T>(&mut self, value: T) -> Result<&mut Self>
    where T: Send + Sync + 'static;
    pub async fn construct<T>(&mut self) -> Result<&mut Self>
    where T: Send + Sync + 'static;
    pub fn lifecycle_hook<H>(&mut self, hook: H) -> &mut Self
    where H: LifecycleHook + 'static;
    pub fn build(self) -> Mads;
}

impl Mads {
    pub fn builder() -> MadsBuilder;
    pub fn builder_with_config(config: Config) -> MadsBuilder;
    pub const fn state(&self) -> LifecycleState;
    pub const fn context(&self) -> &ApplicationContext;
    pub async fn start(&mut self) -> Result<()>;
    pub async fn shutdown(&mut self) -> Result<()>;
}
```

`MadsBuilder::new` inserts a clone of `Config` as a resolvable provider. `construct<T>()` selects exactly one descriptor through `Catalog::provider_for<T>()`, executes its constructor against a temporary `ConstructionContext`, then inserts the erased result. It does not recurse into dependencies.

- [ ] **Step 6: Verify lifecycle and builder behavior**

Run:

```bash
cargo test -p mads-core --test lifecycle --test builder
cargo clippy -p mads-core --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 7: Commit lifecycle and builder**

```bash
git --git-dir=.git-data --work-tree=. add crates/mads-core/src crates/mads-core/tests/lifecycle.rs crates/mads-core/tests/builder.rs
git --git-dir=.git-data --work-tree=. commit -m "feat(core): add lifecycle and explicit builder"
```

---

### Task 7: Generate module, service, and repository metadata

**Files:**
- Modify: `crates/mads-core-macros/src/lib.rs`
- Create: `crates/mads-core-macros/src/path.rs`
- Create: `crates/mads-core-macros/src/module.rs`
- Create: `crates/mads-core-macros/src/managed.rs`
- Modify: `crates/mads-core/src/lib.rs`
- Modify: `crates/mads/src/lib.rs`
- Create: `crates/mads/tests/ui.rs`
- Create: `crates/mads/tests/ui/pass/managed.rs`
- Create: `crates/mads/tests/ui/fail/service_tuple.rs`
- Create: `crates/mads/tests/ui/fail/service_generic.rs`
- Create: `crates/mads/tests/ui/fail/module_shape.rs`
- Create after diagnostic capture: corresponding `.stderr` files

**Interfaces:**
- Consumes: descriptor constructors, `ConstructionContext::resolve`, `ErasedProvider`, inventory private re-export, and facade/core crate path resolution.
- Produces: `#[module]`, `#[service]`, and `#[repository]` attribute macros plus generated descriptors and constructors.

- [ ] **Step 1: Write the trybuild harness and fixtures**

The harness contains:

```rust
//! Compile tests for the public MADS attribute macros.

#[test]
fn core_attributes_accept_supported_shapes() {
    trybuild::TestCases::new().pass("tests/ui/pass/*.rs");
}

#[test]
fn core_attributes_reject_unsupported_shapes() {
    trybuild::TestCases::new().compile_fail("tests/ui/fail/*.rs");
}
```

The passing fixture declares a unit module, dependency-free repository, named-field service, inherent service method that accesses its dependency field, and a normal `fn main()`. Failure fixtures use a tuple service, generic service, and non-unit module.

- [ ] **Step 2: Run compile tests and observe missing attributes**

Run: `cargo test -p mads --test ui`

Expected: FAIL because the facade does not export the attributes.

- [ ] **Step 3: Implement expansion-path selection**

In `path.rs`, use `proc_macro_crate::crate_name`:

```rust
pub(crate) fn core_path() -> syn::Path;
```

Resolve direct `mads-core` consumers to `::mads_core`; resolve facade consumers to `::mads::core`; honor renamed dependencies using the returned crate name. Return a `syn::Error` if neither package exists.

- [ ] **Step 4: Implement `#[module]`**

Accept only a non-generic unit struct with no macro arguments. Preserve visibility and documentation attributes. Emit the struct plus an `inventory::submit!` containing `ModuleDescriptor::new(concat!(module_path!(), "::", stringify!(Name)), type_id_fn, SourceLocation::new(file!(), line!(), column!()))`.

Reject fields, generics, or attribute arguments with a span-local `syn::Error` that states the supported `#[mads::module] struct AppModule;` form.

- [ ] **Step 5: Implement shared managed-provider expansion**

Use one `expand_managed(ProviderKind, ItemStruct)` implementation for service and repository attributes. Accept named-field and unit structs without generics. Generate:

```text
doc-hidden inner value struct
cloneable public handle preserving the original name and visibility
Deref from the handle to the inner value
doc-hidden constructor function
dependency descriptors in source declaration order
provider inventory submission
```

The constructor resolves each named field through `ConstructionContext`, clones the resolved dependency value into the inner value, wraps the inner value in `Arc`, then returns an `Arc` of the public handle as `ErasedProvider`. This preserves shared identity for the managed provider while keeping `Arc` out of user declarations.

Reject tuple structs, generics, and unsupported representation attributes with focused `syn::Error` messages.

- [ ] **Step 6: Export attributes from their conceptual owners**

In `mads-core`:

```rust
pub use mads_core_macros::{module, repository, service};

#[doc(hidden)]
pub mod __private {
    pub use inventory;
}
```

In `mads`:

```rust
pub use mads_core as core;
pub use mads_core::{module, repository, service};
```

Add the same attributes to `mads::prelude`.

- [ ] **Step 7: Capture and approve compile diagnostics**

Run: `TRYBUILD=overwrite cargo test -p mads --test ui`

Inspect every generated `.stderr` file. Confirm it names the invalid construct and shows the supported form. Then run `cargo test -p mads --test ui` without overwrite.

Expected: PASS.

- [ ] **Step 8: Add runtime descriptor integration assertions**

In `crates/mads/tests/facade.rs`, declare one module, repository, and service using `#[mads::module]`, `#[mads::repository]`, and `#[mads::service]`. Assert `Catalog::modules()` and `Catalog::providers()` contain their fully qualified names, service dependencies retain source field order, and cloned service handles dereference to the same inner allocation.

- [ ] **Step 9: Verify macros and Clippy**

Run:

```bash
cargo test -p mads --test ui --test facade
cargo clippy -p mads-core-macros -p mads-core -p mads --all-targets --all-features -- -D warnings
```

Expected: PASS.

- [ ] **Step 10: Commit managed macros**

```bash
git --git-dir=.git-data --work-tree=. add crates/mads-core-macros crates/mads-core crates/mads
git --git-dir=.git-data --work-tree=. commit -m "feat(macros): add module and managed provider attributes"
```

---

### Task 8: Generate provider functions and Tokio-backed main

**Files:**
- Modify: `crates/mads-core-macros/src/lib.rs`
- Create: `crates/mads-core-macros/src/provider.rs`
- Create: `crates/mads-core-macros/src/main.rs`
- Create: `crates/mads-core/src/runtime.rs`
- Modify: `crates/mads-core/src/lib.rs`
- Modify: `crates/mads/src/lib.rs`
- Create: `crates/mads/tests/ui/pass/provider.rs`
- Create: `crates/mads/tests/ui/pass/main.rs`
- Create: `crates/mads/tests/ui/fail/provider_method.rs`
- Create: `crates/mads/tests/ui/fail/provider_inferred_return.rs`
- Create: `crates/mads/tests/ui/fail/main_sync.rs`
- Create after diagnostic capture: corresponding `.stderr` files

**Interfaces:**
- Consumes: Task 7 macro paths and descriptor submission; Task 6 builder construction.
- Produces: `#[provider]`, `#[main]`, and `runtime::block_on` behind `runtime-tokio`.

- [ ] **Step 1: Add provider and main compile fixtures**

The passing provider fixture covers:

```rust
#[mads::provider]
fn sync_value() -> String { "value".to_owned() }

#[mads::provider]
async fn async_value(config: Config) -> mads::core::Result<usize> {
    Ok(config.len())
}
```

The passing main fixture uses `#[mads::main] async fn main() {}`. Failure fixtures place `#[provider]` on an inherent method, omit a concrete return type, and place `#[main]` on a synchronous function.

- [ ] **Step 2: Run the compile tests and observe missing attributes**

Run: `cargo test -p mads --test ui`

Expected: FAIL because `provider` and `main` are not exported.

- [ ] **Step 3: Implement provider-function expansion**

Accept free functions without receivers or type/const generics. Treat function parameters as dependencies in declaration order. Require a concrete return type. Recognize `mads_core::Result<T>`, `mads::core::Result<T>`, and unqualified `Result<T>` syntactically as fallible output; use `T` as the provider output type.

Preserve the original function and generate a unique doc-hidden constructor that:

1. resolves and clones each dependency;
2. calls the function and awaits it when async;
3. applies `?` only for recognized MADS result output;
4. wraps the concrete output in `Arc` as `ErasedProvider`;
5. submits a `ProviderDescriptor` with `ProviderKind::Provider`.

- [ ] **Step 4: Implement runtime bootstrap and main expansion**

In `mads-core/src/runtime.rs` expose, only under `runtime-tokio`:

```rust
pub fn block_on<F>(future: F) -> F::Output
where F: std::future::Future;
```

Construct a Tokio multi-thread runtime with `enable_all()` and call `block_on`. Convert runtime construction failure into a deterministic panic message because Rust `fn main` cannot return the MADS error without changing the user's signature.

`#[main]` accepts an argument-free, non-generic async function named `main`. Replace it with synchronous `fn main()` that invokes `core::runtime::block_on(async move { original_body })`. Preserve documentation and lint attributes. Reject all unsupported signatures with span-local errors.

- [ ] **Step 5: Re-export provider and main attributes**

Add `main` and `provider` to the `mads-core`, `mads`, and `mads::prelude` barrel exports. Re-export Tokio only through a doc-hidden core path if generated code needs it; application crates must not need a direct Tokio dependency.

- [ ] **Step 6: Capture diagnostics and run compile tests**

Run:

```bash
TRYBUILD=overwrite cargo test -p mads --test ui
cargo test -p mads --test ui
```

Expected: all passing fixtures compile and every failure fixture matches its reviewed stderr.

- [ ] **Step 7: Add provider construction integration tests**

Use `Config` plus two provider functions. Explicitly call `builder.construct::<Output>()` in dependency order. Assert direct output and `Result` output are stored. Add a test proving the builder does not recursively construct a missing dependency and returns `MADS003`.

- [ ] **Step 8: Verify features and commit**

Run:

```bash
cargo test -p mads --all-features
cargo check -p mads-core --no-default-features
cargo check -p mads --no-default-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: PASS.

Commit:

```bash
git --git-dir=.git-data --work-tree=. add crates/mads-core-macros crates/mads-core crates/mads
git --git-dir=.git-data --work-tree=. commit -m "feat(macros): add provider and main attributes"
```

---

### Task 9: Finish facade ergonomics and the CLI foundation

**Files:**
- Modify: `crates/mads/src/lib.rs`
- Modify: `crates/mads-common/src/lib.rs`
- Modify: `crates/mads-common-macros/src/lib.rs`
- Modify: `crates/mads-extra/src/lib.rs`
- Modify: `crates/mads-cli/src/main.rs`
- Create: `crates/mads-cli/tests/cli.rs`
- Modify: `crates/mads-cli/Cargo.toml`
- Modify: `README.md`

**Interfaces:**
- Consumes: Complete core public API and canonical attribute exports.
- Produces: Stable facade/prelude surface and `mads foundation` command.

- [ ] **Step 1: Write failing facade surface tests**

In `crates/mads/tests/facade.rs`, add a function that imports `mads::prelude::*`, uses each bare core attribute in a nested module, and refers to `Mads`, `Config`, `Diagnostic`, `Catalog`, and `LifecycleState` without direct `mads-core` imports.

Run: `cargo test -p mads --test facade`

Expected: FAIL for any missing prelude export.

- [ ] **Step 2: Complete the facade and prelude**

At the crate root, re-export `mads_core` as `core` and the five canonical attributes. In `prelude`, re-export the five attributes plus common application-facing core types: `ApplicationContext`, `Catalog`, `Config`, `ConfigBuilder`, `Diagnostic`, `Error`, `LifecycleHook`, `LifecycleState`, `Mads`, `MadsBuilder`, `Result`, and `SourceLocation`.

Keep `common` behind the `common` feature and `extra` behind the `extra` feature. Do not expose future route/database symbols.

- [ ] **Step 3: Write failing CLI process tests**

Add dev dependencies `assert_cmd = "2"` and `predicates = "3"`. Test:

```rust
//! Process-level tests for the MADS CLI foundation.

use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn version_reports_the_workspace_version() {
    Command::cargo_bin("mads")
        .expect("binary should build")
        .arg("--version")
        .assert()
        .success()
        .stdout(contains("mads 0.1.0"));
}

#[test]
fn foundation_check_reports_available_boundaries() {
    Command::cargo_bin("mads")
        .expect("binary should build")
        .arg("foundation")
        .assert()
        .success()
        .stdout(contains("core: available"))
        .stdout(contains("common: reserved"))
        .stdout(contains("extra: reserved"));
}
```

- [ ] **Step 4: Implement the minimal CLI without claiming future commands**

Use `std::env::args` to recognize exactly `--help`, `-h`, `--version`, `-V`, and `foundation`. Unknown arguments print a concise error plus help to stderr and exit with code 2. No `clap` dependency is needed for three commands.

- [ ] **Step 5: Expand the README**

Document the crate diagram, default features, MSRV, canonical macro syntax, explicit v0.1 construction order, the `mads foundation` command, and the deferred feature list. Include a compilable example that creates a `Config`, inserts a dependency, explicitly constructs one managed provider, builds `Mads`, starts it, and shuts it down.

- [ ] **Step 6: Verify facade, CLI, and docs**

Run:

```bash
cargo test -p mads --test facade
cargo test -p mads-cli
cargo test --workspace --doc --all-features
cargo run -p mads-cli -- foundation
```

Expected: PASS; the command reports only implemented/reserved foundation boundaries.

- [ ] **Step 7: Commit facade and CLI**

```bash
git --git-dir=.git-data --work-tree=. add crates/mads crates/mads-common crates/mads-common-macros crates/mads-extra crates/mads-cli README.md
git --git-dir=.git-data --work-tree=. commit -m "feat: complete facade and CLI foundation"
```

---

### Task 10: Add CI, architecture checks, and the 85 percent coverage gate

**Files:**
- Create: `.github/workflows/ci.yml`
- Create: `crates/mads-core/tests/architecture.rs`
- Modify: `crates/mads-core/Cargo.toml`
- Modify: `README.md`

**Interfaces:**
- Consumes: Complete v0.1 workspace.
- Produces: Automated stable/MSRV/lint/rustdoc/architecture/coverage enforcement.

- [ ] **Step 1: Write a failing architecture-boundary test**

Add `cargo_metadata = "0.19"` as a `mads-core` dev dependency. The test runs `cargo_metadata::MetadataCommand`, finds package `mads-core`, follows only normal non-dev dependency edges, and asserts that package names do not contain `mads-common`, `mads-extra`, `axum`, or `diesel`.

First make the test's forbidden list include the known dependency `inventory` and run:

`cargo test -p mads-core --test architecture`

Expected: FAIL naming `inventory`. Replace the forbidden list with the real four boundaries and rerun; expected PASS. This proves the test is capable of detecting an invalid edge.

- [ ] **Step 2: Add stable and MSRV CI jobs**

Create `.github/workflows/ci.yml` triggered by pushes and pull requests. Stable jobs run:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
```

The MSRV job installs toolchain `1.85.0` and runs `cargo test --workspace --all-features`.

- [ ] **Step 3: Add LLVM coverage CI**

Install `llvm-tools-preview` and `cargo-llvm-cov`, then run:

```bash
cargo llvm-cov --workspace --all-features --ignore-filename-regex '(^|/)tests/ui/' --fail-under-lines 85
```

The filename rule excludes trybuild fixtures and their generated expansion locations. Do not exclude ordinary core, proc-macro implementation, facade, or CLI source files.

- [ ] **Step 4: Run the complete local verification suite**

Run fresh commands:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
cargo tree -p mads-core --edges normal
cargo llvm-cov --workspace --all-features --ignore-filename-regex '(^|/)tests/ui/' --fail-under-lines 85
```

Expected: every command exits zero; line coverage is at least 85 percent; the core dependency tree contains no forbidden outward dependency.

- [ ] **Step 5: Verify MSRV locally when toolchain installation is available**

Run:

```bash
rustup run 1.85.0 cargo test --workspace --all-features
```

Expected: PASS. If the managed environment does not have Rust 1.85 installed and cannot download it, preserve the CI job and report the local environmental limitation explicitly; do not weaken `rust-version` or the CI gate.

- [ ] **Step 6: Update verification documentation**

Add exact contributor commands to `README.md`, including the coverage threshold and the distinction between stable and MSRV checks.

- [ ] **Step 7: Commit CI and architecture enforcement**

```bash
git --git-dir=.git-data --work-tree=. add .github crates/mads-core README.md Cargo.lock
git --git-dir=.git-data --work-tree=. commit -m "ci: enforce architecture and coverage gates"
```

---

### Task 11: Perform final scope and release verification

**Files:**
- Modify if verification finds documentation defects: `README.md`
- Modify if verification finds test gaps: the focused test file for that behavior
- Modify: `docs/superpowers/plans/2026-08-16-foundation.md` only to check completed steps during execution

**Interfaces:**
- Consumes: All previous task deliverables.
- Produces: Evidence that Phase 0 and v0.1 meet the approved specification without later-milestone leakage.

- [x] **Step 1: Audit public documentation coverage**

Run:

```bash
rg --files crates -g '*.rs'
rg --files-without-match '^//!' crates -g '*.rs'
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
```

Expected: `rg --files-without-match` prints no Rust source file (and exits 1), and rustdoc exits zero under `missing_docs = deny`.

- [x] **Step 2: Audit milestone scope**

Run:

```bash
rg -n 'axum|diesel|redis|cacheable|rate_limit|request.scope|transient.scope' Cargo.toml crates README.md
```

Expected: matches appear only in documentation that marks those capabilities deferred or in the architecture test's forbidden dependency list; no implementation dependency or public symbol provides them.

- [x] **Step 3: Run all release gates from clean command invocations**

Run:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
cargo llvm-cov --workspace --all-features --ignore-filename-regex '(^|/)tests/ui/' --fail-under-lines 85
```

Expected: all five commands exit zero.

- [x] **Step 4: Inspect the final repository delta**

Run:

```bash
git --git-dir=.git-data --work-tree=. status --short
git --git-dir=.git-data --work-tree=. log --oneline --decorate -12
```

Expected: the worktree is clean because Task 1 committed the pre-existing project documents together with the workspace baseline.

- [x] **Step 5: Commit final documentation or test corrections if any were required**

If Step 1 through Step 3 required a correction, stage only those corrected files and commit:

```bash
git --git-dir=.git-data --work-tree=. add README.md crates docs/superpowers/plans/2026-08-16-foundation.md
git --git-dir=.git-data --work-tree=. commit -m "docs: finalize v0.1 foundation verification"
```

If no correction was required, do not create an empty commit.
