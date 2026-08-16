# MADS.rs — Architecture

## 1. Architectural Goal

MADS is an application framework layered on top of Axum.

Its architecture must preserve a strict boundary:

```text
┌──────────────────────────────────────────────┐
│             Application Developer            │
│                                              │
│  Module · Service · Route · Input · Result   │
└──────────────────────┬───────────────────────┘
                       │
                       ▼
┌──────────────────────────────────────────────┐
│                   MADS                       │
│                                              │
│ module metadata                              │
│ service construction                         │
│ route registration                           │
│ application state                            │
│ extraction / response adapters               │
│ error mapping                                │
│ lifecycle                                    │
│ diagnostics                                  │
└──────────────────────┬───────────────────────┘
                       │
                       ▼
┌──────────────────────────────────────────────┐
│                   Axum                       │
│ routing · extractors · response · middleware │
└──────────────────────┬───────────────────────┘
                       │
                       ▼
                    Tower
                       │
                       ▼
                    Hyper
                       │
                       ▼
                    Tokio
```

MADS owns application semantics. Axum owns HTTP request routing and handling.

---

## 2. Workspace Layout

Recommended initial workspace:

```text
mads/
├── Cargo.toml
├── README.md
├── LICENSE
│
├── crates/
│   ├── mads/
│   │   └── src/
│   │       ├── lib.rs
│   │       └── prelude.rs
│   │
│   ├── mads-core/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── app.rs
│   │       ├── module.rs
│   │       ├── service.rs
│   │       ├── registry.rs
│   │       ├── error.rs
│   │       └── lifecycle.rs
│   │
│   ├── mads-web/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── router.rs
│   │       ├── handler.rs
│   │       ├── extract.rs
│   │       ├── response.rs
│   │       ├── state.rs
│   │       └── server.rs
│   │
│   ├── mads-macros/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── main.rs
│   │       ├── module.rs
│   │       ├── service.rs
│   │       └── route.rs
│   │
│   └── mads-cli/
│       └── src/
│           ├── main.rs
│           ├── dev.rs
│           └── check.rs
│
├── examples/
│   ├── hello-world/
│   ├── basic-rest/
│   └── modular-app/
│
└── docs/
    ├── idea.md
    ├── ARCHITECTURE.md
    └── timeline.md
```

Do not create many integration crates during the MVP. The architecture above gives separation without forcing the project to maintain a large ecosystem immediately.

---

## 3. Crate Responsibilities

### `mads`

Public facade.

Responsibilities:

- re-export the stable application API;
- expose `mads::prelude::*`;
- re-export procedural macros;
- keep most underlying implementation crates out of user-facing code.

Expected usage:

```toml
[dependencies]
mads = "0.x"
```

rather than asking users to manually select the normal Axum/Tokio/Tower integration set.

### `mads-core`

Framework-neutral application semantics.

Responsibilities:

- application builder;
- module descriptors;
- service descriptors;
- dependency registry;
- service lifecycle metadata;
- application/framework errors;
- future module graph representation.

It should avoid depending directly on Axum wherever practical.

### `mads-web`

Axum adapter and HTTP-facing implementation.

Responsibilities:

- turn MADS routes into Axum routes;
- own MADS application state representation;
- bridge MADS extractors to Axum extractors;
- bridge MADS results/responses to Axum responses;
- server startup and shutdown defaults;
- Tower/Axum escape hatches.

### `mads-macros`

Procedural macros.

Initial macros:

```text
#[mads::main]
#[module]
#[service]
#[get]
#[post]
#[put]
#[patch]
#[delete]
```

Macros should generate metadata/adapter code, not create a hidden runtime reflection system.

### `mads-cli`

Developer workflow tooling.

Initial commands:

```text
mads dev
mads check
```

Potential later commands:

```text
mads new
mads routes
mads graph
mads inspect
```

---

## 4. Public Programming Model

The application-facing API should remain intentionally compact.

### Root application

```rust
#[mads::main]
async fn main() {
    Mads::new()
        .module(AppModule)
        .run()
        .await;
}
```

### Module

MVP:

```rust
#[module(
    services = [UserService],
    routes = [list_users, get_user],
)]
pub struct UserModule;
```

Later:

```rust
#[module(
    imports = [DatabaseModule],
    services = [UserRepository, UserService],
    routes = [list_users, get_user],
    exports = [UserService],
)]
pub struct UserModule;
```

### Service

```rust
#[service]
pub struct UserService {
    db: Database,
}
```

### Route

```rust
#[get("/users/:id")]
async fn get_user(
    id: Path<u64>,
    users: UserService,
) -> Result<User> {
    users.find(*id).await
}
```

---

## 5. Internal Registration Model

Avoid full runtime reflection.

Procedural macros should generate static registration descriptors that can be collected through module registration.

Conceptual model:

```rust
pub struct RouteDescriptor {
    pub method: Method,
    pub path: &'static str,
    pub register: fn(Router, &AppContext) -> Router,
}

pub struct ServiceDescriptor {
    pub type_name: &'static str,
    pub constructor: ServiceConstructor,
}

pub struct ModuleDescriptor {
    pub name: &'static str,
    pub services: &'static [ServiceDescriptor],
    pub routes: &'static [RouteDescriptor],
}
```

The exact Rust representation should be selected after experimentation; the key design requirement is that metadata remains explicit, deterministic, and debuggable.

---

## 6. Application Boot Sequence

MVP boot process:

```text
Mads::new()
    ↓
register root modules
    ↓
collect module descriptors
    ↓
collect service descriptors
    ↓
construct application-scoped services
    ↓
build MADS application state
    ↓
collect route descriptors
    ↓
build Axum Router
    ↓
apply framework defaults / configured layers
    ↓
start Axum server
```

Future boot process can add graph validation before construction:

```text
collect descriptors
    ↓
build application graph
    ↓
validate graph
    ↓
construct services
```

---

## 7. Service Storage

### MVP lifecycle

All framework-managed services are:

```text
application-scoped
Send
Sync
'static
```

They are constructed once during application startup.

Conceptually, internal storage may use:

```rust
Arc<T>
```

or a type-erased registry containing `Arc<T>` values.

A possible MVP implementation:

```rust
HashMap<TypeId, Arc<dyn Any + Send + Sync>>
```

This is acceptable as a boot/runtime implementation if it keeps the initial design tractable, provided route invocation does not become unnecessarily expensive or opaque.

A later version can replace or optimize this with generated/static wiring without changing the public API.

### Important distinction

`Arc<T>` is not itself the lifecycle.

```text
MADS lifecycle policy → service constructed once
Arc<T>                → shared ownership mechanism
```

The developer sees `UserService`; MADS may internally carry `Arc<UserService>`.

---

## 8. Service Resolution Strategy

Recommended evolution:

### Stage A — explicit constructors + startup registry

```text
metadata known at compile time
construction at startup
service lookup through MADS state
```

This provides fast implementation and clear behavior.

### Stage B — graph validation

Add knowledge of service dependencies and validate missing bindings/duplicates/cycles before server startup or through generated compile-time checks where practical.

### Stage C — generated wiring optimization

If profiling or diagnostics justify it, generate more concrete service wiring and reduce type-erased lookup.

Do not start at Stage C merely for a “zero-cost” marketing statement.

---

## 9. Handler Adapter

A MADS route macro should generate an Axum-compatible adapter.

Application code:

```rust
#[get("/users/:id")]
async fn get_user(
    id: Path<u64>,
    users: UserService,
) -> Result<User> {
    users.find(*id).await
}
```

Conceptual generated adapter:

```text
Axum request
    ↓
extract Path<u64>
    ↓
resolve UserService from MADS state
    ↓
call get_user(...)
    ↓
map MADS Result<User>
    ↓
Axum IntoResponse
```

The adapter layer is where most of the complexity should live so the user-facing handler remains simple.

---

## 10. Extractors

MVP supported inputs:

```text
Json<T>
Path<T>
Query<T>
Header<T>   (optional for first MVP cut)
```

MADS should reuse Axum extraction behavior when possible rather than reimplement request parsing.

Possible implementation strategies:

1. type aliases/re-exports where semantics match exactly;
2. thin newtypes where MADS wants standardized error mapping;
3. custom extractors only when MADS-specific behavior is required.

Prefer 1 before 2, and 2 before 3.

---

## 11. Response Model

Common return types should work automatically:

```text
String
&'static str
Json<T>
T where T satisfies the MADS JSON response contract
Result<T>
Result<Json<T>>
```

MADS should define a small application error vocabulary and map it to HTTP status codes.

Example:

```rust
pub enum MadsError {
    BadRequest(...),
    Unauthorized(...),
    Forbidden(...),
    NotFound(...),
    Conflict(...),
    Internal(...),
}
```

The design should allow custom error types to participate without forcing every handler to use a single framework enum.

---

## 12. Module Graph — Later Architecture

After the MVP, `ModuleDescriptor` can expand into an application graph.

Node model:

```text
ModuleNode
├── id
├── imports
├── services
├── exports
└── routes
```

Service model:

```text
ServiceNode
├── type
├── owner_module
├── dependencies
├── visibility
└── lifecycle
```

Validation targets:

```text
missing dependency
private dependency
missing module import
duplicate service
module cycle
service cycle
invalid lifecycle dependency
```

Possible diagnostic:

```text
MADS003: unavailable service

OrderService depends on UserRepository.
UserRepository exists in UserModule but is not exported.

OrderModule
└── imports UserModule
    ├── UserService       exported
    └── UserRepository    private
```

This application-level diagnostic experience is strategically more valuable than exposing raw implementation trait failures.

---

## 13. Lifecycles — Later Architecture

Do not include multiple lifecycles in the first MVP.

Future model:

```text
Application / Singleton
Request
Transient
```

Potential rule:

```text
longer-lived service cannot directly retain a shorter-lived service
```

Example invalid graph:

```text
AuditService [application]
     ↓
CurrentUser [request]
```

MADS should eventually detect this at registration/build time rather than allowing a confusing runtime failure.

---

## 14. Middleware

Do not invent a competing middleware ecosystem.

Axum already integrates with Tower. MADS should provide:

1. a simple common-path API;
2. direct compatibility with Tower layers.

Potential public API:

```rust
Mads::new()
    .layer(CorsLayer::permissive())
```

or module-level layering later.

MADS-specific guards/interceptors should only be introduced if they solve an application-level problem not already handled cleanly by Tower/Axum.

---

## 15. Configuration

MVP defaults should be usable without configuration:

```text
host = 127.0.0.1
port = 3000
development logging enabled in dev mode
graceful shutdown enabled
```

Future configuration can support:

```toml
[server]
host = "0.0.0.0"
port = 8080
```

Avoid creating a large configuration abstraction until recurring use cases are clear.

---

## 16. Diagnostics Architecture

Diagnostics are a core subsystem, not polish.

Framework-controlled errors should be represented internally as structured diagnostics:

```text
Diagnostic
├── code        MADS003
├── title       missing service dependency
├── module      UserModule
├── subject     UserService
├── dependency  Database
├── explanation
└── suggestions
```

This representation can be rendered by:

```text
compiler macro errors
mads check
mads dev
future IDE tooling
```

Prefer stable MADS diagnostic codes from early versions.

---

## 17. CLI Architecture

### `mads dev`

Responsibilities:

```text
watch files
invoke cargo build/run
capture relevant compiler diagnostics
restart application
show MADS startup summary
```

Do not attempt true Node-style hot module replacement initially. Process restart over Rust incremental compilation is sufficient for the first implementation.

### `mads check`

Runs framework validation without intentionally starting the server.

Long-term it can display:

```text
module graph
service graph
route conflicts
lifecycle errors
```

### `mads routes`

Potential later command:

```text
GET     /users
POST    /users
GET     /users/:id
```

---

## 18. Axum Escape Hatch

MADS must preserve interoperability.

Possible approaches:

```rust
Mads::new()
    .axum_layer(...)
```

or:

```rust
fn configure(router: axum::Router) -> axum::Router
```

or exposing a controlled access point from a module.

The exact API is secondary to the rule:

> Do not force developers to abandon the Axum/Tower ecosystem to adopt MADS.

---

## 19. Compile-Time Strategy

MADS should use procedural macros for ergonomic declarations and static metadata, but should not make “everything compile-time” an MVP requirement.

Recommended progression:

```text
v0.1
macros generate descriptors and Axum adapters
service construction validated at startup where necessary

v0.2+
module/service dependency metadata
better static validation and diagnostics

later
more compile-time graph validation/generated wiring where it produces measurable DX or runtime benefits
```

This avoids turning an application-framework project into a compiler project before its product thesis is proven.

---

## 20. Performance Model

MADS should aim for:

```text
Axum-level request handling
+ small framework overhead
```

Performance priorities:

1. do expensive graph/service setup at startup, not per request;
2. avoid unnecessary allocation in route adapters;
3. use shared handles for application services;
4. reuse Axum extractors and responses;
5. benchmark real endpoints, not only hello-world;
6. optimize only after profiling.

A small amount of service-resolution overhead is acceptable for early releases if it produces dramatically better maintainability and can later be optimized without breaking application APIs.

---

## 21. Example Application Structure

Recommended feature-oriented structure:

```text
src/
├── main.rs
├── app.rs
│
├── users/
│   ├── mod.rs
│   ├── routes.rs
│   ├── service.rs
│   ├── repository.rs
│   └── model.rs
│
├── auth/
│   ├── mod.rs
│   ├── routes.rs
│   └── service.rs
│
└── infrastructure/
    ├── mod.rs
    └── database.rs
```

`users/mod.rs`:

```rust
#[module(
    services = [UserRepository, UserService],
    routes = [list_users, get_user],
)]
pub struct UserModule;
```

The module definition becomes a compact map of the feature without making the developer manually construct Axum state.

---

## 22. Architecture Rules

The following rules should be treated as design constraints.

### Rule 1 — No custom HTTP stack

MADS delegates HTTP behavior to Axum unless an application-level abstraction is required.

### Rule 2 — No mandatory `Arc<T>` in normal application signatures

MADS owns shared service plumbing.

### Rule 3 — No mandatory `State<AppState>` in normal route signatures

MADS owns application state plumbing.

### Rule 4 — Explicit module registration first

Avoid magical source scanning and registration until a robust need and implementation strategy exist.

### Rule 5 — Application services default to singleton/application scope

Additional scopes are later features.

### Rule 6 — Keep generated code inspectable

Macro expansion should be understandable and avoid unnecessary generic complexity.

### Rule 7 — Axum interoperability is a feature

Do not wall off the lower ecosystem.

### Rule 8 — Errors at the MADS layer should mention MADS concepts

A missing service should be described as a missing service, not only as an unrelated trait-bound failure.

### Rule 9 — Do not optimize away the architecture too early

A simple service registry is acceptable while the public programming model is being proven.

### Rule 10 — Every major feature must reduce or justify cognitive load

If a new abstraction introduces more concepts than it removes, reconsider it.

---

## 23. First Technical Spike

Before implementing the full MVP, build one end-to-end spike that supports exactly this:

```rust
#[service]
struct GreetingService;

impl GreetingService {
    fn hello(&self, name: &str) -> String {
        format!("Hello {name}")
    }
}

#[get("/hello/:name")]
async fn hello(
    name: Path<String>,
    greeting: GreetingService,
) -> String {
    greeting.hello(&name)
}

#[module(
    services = [GreetingService],
    routes = [hello],
)]
struct AppModule;

#[mads::main]
async fn main() {
    Mads::run::<AppModule>().await;
}
```

This spike should prove five things:

1. route macro registration works;
2. service construction works;
3. service injection works without application-visible `Arc`/`State`;
4. handler inputs still use normal typed Rust;
5. the application runs on Axum.

If this spike requires a large amount of unstable magic, simplify the public model before expanding the project.
