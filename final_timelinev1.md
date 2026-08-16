# MADS.rs — Final Timeline to v1

## Scope

Timeline ini mendefinisikan jalur implementasi MADS dari foundation sampai **v1.0.0**.

Target utama v1:

> Membuktikan bahwa MADS dapat menyediakan modular Rust backend dengan type-driven dependency wiring, Axum HTTP integration, Diesel persistence, auto-configuration, diagnostics, CLI, dan testing foundation secara stabil.

**Cache, Redis, `#[cacheable]`, `#[cache_evict]`, built-in rate limiting, dan `#[rate_limit]` tidak masuk scope v1.** Fitur tersebut dimulai setelah v1 agar application graph dan public API tidak berubah karena feature pressure terlalu awal.

---

## Release Strategy

```text
0.1.0  Foundation
  ↓
0.2.0  Dependency Graph
  ↓
0.3.0  HTTP/Common Runtime
  ↓
0.4.0  Diesel Persistence
  ↓
0.5.0  Auto Configuration
  ↓
0.6.0  Modules + Visibility
  ↓
0.7.0  Validation + Errors
  ↓
0.8.0  CLI + Diagnostics
  ↓
0.9.0  Testing + Hardening
  ↓
1.0.0  Stable Contract
```

Version number menunjukkan capability milestone, bukan janji calendar date. Setiap milestone harus lolos acceptance criteria sebelum release berikutnya.

---

# Phase 0 — Repository and Architecture Foundation

## Goal

Menetapkan physical crate/module boundaries sebelum API berkembang terlalu jauh.

Target layout:

```text
mads/
├── core/
├── common/
└── extra/
```

Untuk v1:

```text
core   → aktif
common → aktif
extra  → reserved / minimal shell
```

`extra` tidak menjadi dependency requirement aplikasi v1.

### Deliverables

- workspace structure;
- feature flags strategy;
- facade/prelude strategy;
- CI for stable + minimum supported Rust policy;
- formatting/lint/test baseline;
- compile-test harness untuk proc macros;
- architecture dependency rules.

### Acceptance

```text
core does not depend on common
core does not depend on extra
common depends only downward into core abstractions
future extra can plug into core/common without changing core
```

---

# v0.1.0 — Core Runtime Foundation

## Objective

Membangun MADS sebagai application runtime terlebih dahulu, sebelum HTTP/ORM menjadi fokus utama.

### `mads/core`

Implement:

```text
Mads instance
Mads builder
application context
basic provider registry
application-scoped lifecycle
configuration bootstrap foundation
Result/Error foundation
#[mads::main]
#[service]
#[repository]
#[provider]
```

### Service / Repository Macro MVP

Input:

```rust
#[repository]
struct UserRepository {
    db: Database,
}

#[service]
struct UserService {
    users: UserRepository,
}
```

Macro minimal harus mampu menghasilkan metadata dependency yang dapat dibaca graph engine berikutnya.

### Constraints

Belum perlu:

```text
request scope
transient scope
trait qualifiers
cache
rate limit
Redis
advanced HTTP
```

### Exit Criteria

- service/repository metadata deterministic;
- instance dapat dibangun oleh generated constructor path;
- startup/shutdown basic lifecycle bekerja;
- proc macro diagnostics cukup jelas untuk malformed declarations;
- no mandatory Axum dependency di core.

---

# v0.2.0 — Dependency and Application Graph

## Objective

Menjadikan type-driven dependency graph sebagai pusat framework.

### Implement

```text
provider nodes
dependency edges
topological construction order
missing dependency detection
duplicate provider detection
dependency cycle detection
ambiguous provider detection
provider visibility foundation
graph inspection API
```

Example:

```text
UserService
└── UserRepository
    └── Database
```

### Construction Plan

Graph engine harus menghasilkan deterministic plan:

```text
Database
  ↓
UserRepository
  ↓
UserService
```

### Exit Criteria

- graph validation terjadi sebelum server/runtime application start;
- cycle dapat ditampilkan sebagai readable dependency path;
- no reliance on registration order;
- `mads graph` internal data model sudah tersedia meskipun CLI belum final.

---

# v0.3.0 — `mads/common` HTTP and Routing

## Objective

Membawa Axum sebagai standard HTTP engine tanpa membuat `core` menjadi HTTP-specific.

### Implement in `common`

```text
route registry metadata
#[get]
#[post]
#[put]
#[patch]
#[delete]
Axum router generation
Path<T>
Query<T>
Json<T>
Header<T>
Request integration
basic response mapping
Created<T>
NoContent
```

### Route Dependency Injection

Target:

```rust
#[get("/users/:id")]
async fn get_user(
    id: Path<i64>,
    users: UserService,
) -> Result<Json<User>> {
    // ...
}
```

MADS harus membedakan:

```text
Path<i64>   → extractor
UserService → dependency graph node
```

### Exit Criteria

- CRUD routes dapat berjalan;
- no manual `State<AppState>` untuk standard path;
- no manual Axum router wiring list;
- native Axum extractor escape hatch tetap bekerja;
- HTTP adapter tidak mengubah dependency semantics core.

---

# v0.4.0 — Diesel-first Persistence

## Objective

Membuat persistence experience yang cukup lengkap untuk real CRUD application.

### Implement in `common::database`

```text
Database abstraction
DatabaseConfig
Diesel integration
connection pool lifecycle
PostgreSQL initial backend
Database::run
migration runner
startup migration option
Diesel error normalization foundation
```

Target configuration:

```toml
[database]
url = "${DATABASE_URL}"
pool_size = 10
migrate = true
```

Target repository:

```rust
#[repository]
struct UserRepository {
    db: Database,
}
```

### CLI foundation

```bash
mads db migrate
mads db rollback
mads db status
```

### Exit Criteria

- complete User CRUD works with Diesel/PostgreSQL;
- database pool initialized once;
- migration can run manually/startup;
- repository receives `Database` through dependency graph;
- direct Diesel DSL remains available.

---

# v0.5.0 — Auto-Configuration Engine

## Objective

Menggabungkan application requirements, capability availability, dan configuration menjadi automatic infrastructure provisioning.

### Core Auto-Config Model

```text
Requirement
  +
Capability
  +
Configuration
  ↓
AutoConfiguration
```

### Diesel Auto Configuration

Activation example:

```text
UserRepository requires Database
        +
Diesel integration available
        +
database.url configured
        ↓
DieselDatabaseAutoConfiguration
```

### Back-Off

Implement rule:

```text
custom provider exists
      ↓
default auto-config backs off
```

### Inspection Model

Auto-config engine harus menyimpan reason:

```text
ACTIVE
SKIPPED
OVERRIDDEN
FAILED
```

### Exit Criteria

- default database wiring membutuhkan zero bootstrap code di `main`;
- custom Database provider dapat override default;
- auto-config activation deterministic;
- reason dapat dipakai oleh `mads doctor` di milestone berikutnya.

---

# v0.6.0 — Modules and Architectural Boundaries

## Objective

Membuat modules sebagai explicit architecture boundary tanpa mengubahnya menjadi DI manifest.

### Implement

```text
#[module]
module imports
module path prefix
exports / visibility
cross-module dependency validation
module cycle detection
root AppModule
```

Target:

```rust
#[module(path = "/users")]
pub struct UserModule;

#[module(imports = [UserModule])]
pub struct AppModule;
```

Rule:

```text
module relationship   = explicit
provider relationship = inferred
```

### Exit Criteria

- root application graph dapat dimulai dari `AppModule`;
- dependency lintas module mengikuti visibility/export rules;
- route prefix module bekerja;
- module cycle diagnostic readable;
- services/routes tidak perlu didaftarkan ulang secara manual.

---

# v0.7.0 — Validation, Errors, and Configuration UX

## Objective

Membuat common REST application tidak perlu merakit validation/error plumbing sendiri.

### Validation

Implement target API:

```rust
#[derive(Input)]
struct CreateUser {
    #[validate(email)]
    email: String,
}
```

Flow:

```text
deserialize
  ↓
validate
  ↓
handler
```

### Error Model

Standard errors:

```text
BadRequest
Unauthorized
Forbidden
NotFound
Conflict
ValidationError
InternalError
```

Error response schema dibuat konsisten.

### Typed Config

Implement:

```text
mads.toml
environment interpolation
typed config structs
startup validation
secret-safe values foundation
```

### Exit Criteria

- invalid input tidak masuk handler;
- missing configuration error memiliki source/path yang jelas;
- common Diesel errors dapat dipetakan ke framework Result;
- developer tetap dapat return native Axum response.

---

# v0.8.0 — CLI, Dev Loop, and Framework Diagnostics

## Objective

Menjadikan explainable magic sebagai bagian produk, bukan debugging internal.

### CLI

```bash
mads dev
mads run
mads routes
mads graph
mads doctor
mads db migrate
mads db rollback
mads db status
```

### `mads dev`

Implement:

```text
source watch
incremental build integration
restart
route table
startup summary
MADS diagnostic rendering
```

### Diagnostics

Minimum codes/categories:

```text
unresolved dependency
duplicate provider
ambiguous dependency
dependency cycle
module cycle
private provider
database auto-config failure
invalid route
invalid configuration
```

### `mads doctor`

Example:

```text
ACTIVE
✓ HttpServerAutoConfiguration
✓ DieselDatabaseAutoConfiguration

OVERRIDDEN
↷ <capability> default provider
```

### Exit Criteria

- developer dapat melihat route table dan graph;
- auto-config dapat dijelaskan;
- common framework failures tidak hanya muncul sebagai opaque trait-bound errors;
- dev loop dapat digunakan untuk membangun sample CRUD secara nyaman.

---

# v0.9.0 — Testing, Compatibility, and Hardening

## Objective

Menghentikan feature expansion dan fokus pada reliability menuju 1.0.

### Testing Runtime

Target APIs:

```rust
#[mads::test]
async fn service_test(users: UserService) {}
```

```rust
#[mads::test]
async fn http_test(client: TestClient) {}
```

### Provider Overrides for Tests

Foundation untuk replacing provider pada test graph.

### Hardening

```text
compile-fail macro tests
integration CRUD suite
module visibility suite
auto-config back-off suite
migration suite
shutdown/lifecycle suite
Axum escape-hatch suite
native Diesel suite
configuration error suite
```

### Performance Baseline

Bandingkan:

```text
startup overhead
request overhead
memory overhead
compile impact
```

terhadap equivalent Axum application.

Tujuan bukan artificial zero-overhead claim, tetapi cost yang kecil dan dapat dijelaskan.

### API Freeze Candidate

Mulai larang perubahan besar pada:

```text
#[service]
#[repository]
#[module]
route macros
Mads::run
Database
Result
core graph concepts
```

### Exit Criteria

- sample applications stabil;
- no known graph correctness bugs;
- public API candidate documented;
- migration path dari 0.9 ke 1.0 minimal.

---

# v1.0.0 — Stable MADS Foundation

## Objective

Menetapkan contract yang dapat dibangun ecosystem tanpa terus berubah.

### v1 Includes

```text
mads/core
  Mads runtime
  modules
  service/repository/provider macros
  application graph
  dependency graph
  lifecycle
  auto-configuration engine
  diagnostics core

mads/common
  Axum HTTP integration
  route macros
  request/response types
  validation
  standard errors
  Diesel integration
  Database
  pool lifecycle
  migrations

CLI
  dev
  run
  routes
  graph
  doctor
  db commands

Testing foundation
Escape hatches
Documentation
```

### Explicitly NOT in v1

```text
Redis
CacheService
#[cacheable]
#[cache_evict]
RateLimitService
#[rate_limit]
request-scoped DI
transient DI
advanced auth framework
message queue
background jobs
```

### Stability Commitments

v1 should commit to the concepts, not every internal implementation detail:

```text
explicit modules
type-inferred dependencies
automatic wiring
explainable auto-configuration
Diesel-first persistence
Axum compatibility
framework diagnostics
escape hatches
```

---

# v1 Reference Application Gate

Sebelum 1.0 release, satu reference CRUD application harus membuktikan seluruh common path.

Required routes:

```text
GET    /users
GET    /users/:id
POST   /users
PUT    /users/:id
DELETE /users/:id
```

Required graph:

```text
Route
  ↓
UserService
  ↓
UserRepository
  ↓
Database
  ↓
Diesel
  ↓
PostgreSQL
```

Required developer-visible application code harus bebas dari:

```text
Arc<AppState>
State<T>
manual repository construction
manual service construction
manual Diesel pool bootstrap
manual Axum Router composition
manual Tokio bootstrap
```

---

# Documentation Gate for 1.0

Sebelum v1:

```text
Getting Started
Core Concepts
Modules
Services
Repositories
Routing
Validation
Error Handling
Configuration
Diesel / Database
Migrations
Dependency Graph
Auto Configuration
Diagnostics
Testing
Axum Escape Hatch
Diesel Escape Hatch
Clean Architecture Example
Migration Guide from 0.9
```

---

# Post-v1 — `mads/extra`

Setelah 1.0 stabil, development dapat bergerak ke optional policies.

Suggested first sequence:

```text
1.1 / 1.x foundation
  Redis capability
      ↓
  CacheService
      ↓
  #[cacheable] / #[cache_evict]
      ↓
  RateLimitService
      ↓
  #[rate_limit]
```

Semua fitur ini harus plug into existing graph/policy metadata tanpa memerlukan redesign `mads/core`.

---

# Final v1 Success Criterion

MADS v1 berhasil apabila backend developer yang memahami basic Rust dapat membuat CRUD production-oriented dengan mental model:

```text
Model
  ↓
Repository
  ↓
Service
  ↓
Route
  ↓
Run
```

sementara framework menangani:

```text
application graph
dependency construction
shared ownership
Axum runtime/router
Diesel pool
configuration
migrations
validation
error mapping
diagnostics
```

> **v1 harus membuktikan fondasi MADS terlebih dahulu. Optional infrastructure policies datang setelah fondasi tersebut stabil.**
