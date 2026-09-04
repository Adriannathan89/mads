# MADS.rs 0.7.0 Architecture

MADS.rs separates framework-neutral application semantics from HTTP delivery and
PostgreSQL persistence. Version 0.7.0 adds a Cargo-native execution and
inspection boundary around the v0.6 rooted runtime, plus a development
supervisor and bounded PostgreSQL schema-diff generation. It does not add input
validation or machine-readable CLI output.

~~~text
Application modules, providers, route traits, and controllers
                 |
                 v
        mads macros: metadata + typed registrars
                 |
                 v
 mads-core: module graph, config, provider graph, lifecycle, diagnostics
                 |
                 v
mads-common: scoped routes, Passport/JWT, CORS, server, Diesel, Axum adapter
                 |
                 v
      PostgreSQL + Diesel       Axum + Tower + Tokio
~~~

## Crate boundary

mads-core owns generic configuration, module descriptors and graph analysis,
Rust-namespace ownership, provider graph validation and construction,
lifecycle, diagnostics, official auto-configuration evaluation, and redacted
inspection reports. It remains independent of Axum, listener binding, CORS,
cookies, JWT, and Diesel.

mads-common consumes the selected core application scope. It selects scoped
controllers, routes, guards, and Passport strategies; supplies official Diesel,
JWT, server, and CORS auto-configurations; builds and finalizes Axum routers;
and coordinates HTTP serving. DatabaseBootstrap remains the explicit native
Diesel override, while custom Database providers own their complete lifecycle.

mads is the public facade. Its standard prelude exposes rooted startup through
Mads::run::<AppModule>().await, plus the module, route, and HTTP contracts. The
low-level Mads::builder* APIs remain available through inherent methods and
mads::core; MadsBuilder is intentionally not part of the standard prelude.

The feature boundary is deliberate:

~~~text
jwt                 JWT service/configuration; no Axum or Diesel
cookies             cookie request/response support; includes HTTP
http + jwt          Passport strategies and Bearer guards
http + jwt + cookies
                    cookie-sourced guards
common              compatibility aggregate for HTTP + database only
~~~

## Application scope and module visibility

#[module(imports = [...])] selects a root application and its direct-import
graph. A descriptor belongs to the nearest annotated Rust namespace, so no
provider, controller, route, guard, or strategy manifest is required. All
descriptors owned by reachable modules participate in one scoped application;
official auto-configuration derives requirements from that same scope.

A module may use its own private or restricted Rust items. Across modules, a
provider or strategy must belong to a directly imported module and use plain,
unrestricted pub visibility. Imports are not transitive: if A imports B and B
imports C, A must import C itself to use one of C's public providers. Plain Rust
pub is therefore the cross-module contract; there is no separate exports
manifest.

Unowned providers enter a rooted application only when a selected dependency
requires them. While resolution travels through an unowned dependency chain, it
retains the requesting module context, so that chain cannot bypass a missing
direct import. When resolution reaches an owned provider, the owner module
becomes the context for that provider's own dependencies.

Routes own their HTTP paths. #[routes(prefix = "/users")], not #[module],
defines /users; modules have no HTTP path attribute. A builder without
root::<AppModule>() remains compatible with the complete-catalog analysis,
construction, and routing behavior from v0.5.5.

## Startup sequence

The standard run sequence is fixed:

~~~text
conventional config load (standard run only)
  -> root module graph
  -> scoped provider/HTTP/Passport requirements
  -> official auto-configuration evaluation
  -> virtual graph validation
  -> provider construction
  -> selected route/guard/strategy validation
  -> generated/native router merge
  -> outer CORS configuration
  -> lifecycle startup
  -> address resolution and bind
  -> serve and reverse-order shutdown
~~~

Preflight completes before lifecycle startup, so an invalid dependency, route,
guard, strategy, server configuration, or CORS configuration never starts a
hook or binds a listener. Hooks start in registration order and shut down in
reverse order. A resolution, bind, or serving failure after startup triggers the
same rollback; when shutdown also fails, the runtime retains both failures.

Passport strategy selection is context-local. An owned guard uses its owner
module; an unowned guard inherits its route or controller context. A custom
strategy in another module must be both public and directly imported in that
context. One visible custom jwt strategy overrides the built-in fallback;
multiple visible custom strategies with the same name are ambiguous, while
same-named strategies may coexist where no guard can see both.

## CLI parent and inspection child

Normal execution uses the standard parent process directly:

~~~text
mads run/dev parent
  -> Cargo resolves and builds one selected package/binary
  -> application enters Mads::run::<AppModule>()
  -> normal providers, lifecycle, configuration, and HTTP behavior run
~~~

App-aware inspection has a separate side-effect boundary:

~~~text
mads routes/graph/doctor parent
  -> Cargo resolves and builds the selected application
  -> private inspection child enters the standard Mads::run path
  -> compiled graph/report metadata crosses the private protocol
  -> child exits; parent renders human-readable output
~~~

The inspection child reports before normal application startup. It does not
construct providers, start lifecycle hooks, connect to PostgreSQL, run
migrations, bind a socket, or serve traffic. Code and build-script effects
before the standard MADS entry point remain Cargo/application responsibilities.
Low-level builder-only applications are outside the app-aware inspection
contract.

## Development supervisor

`mads dev` owns a Cargo build task, a selected application child, and a file
watcher. The watcher includes reachable local package sources, Cargo manifests
and lockfiles, migrations, and selected-package `.env`/`mads.toml`; generated
targets, editor files, `.git`, and unreachable nested packages are excluded.
Events are debounced for 150 ms. Source/Cargo/migration changes rebuild;
selected-package configuration changes restart. A batch containing both kinds
of change is a rebuild. Failed rebuilds keep the last good process and continue
watching. This is process replacement, not hot module replacement. Ctrl-C
cancels a build, stops the child, and performs cleanup.

## Configuration, server, and router composition

Only Mads::run loads conventional configuration. It reads, from the process
current working directory and in order, optional .env interpolation values,
optional mads.toml, then final MADS_* environment overrides. Process variables
win over dotenv values during interpolation, dotenv never mutates the process
environment, and MADS_SERVER__PORT maps to server.port. Both files may be
absent; a present unreadable or malformed file is a bootstrap error. MADS does
not search parent directories or CARGO_MANIFEST_DIR.

The low-level builder never loads files or process configuration automatically.
It accepts an explicit Config, explicit values, lifecycle hooks, migrations,
and an explicit listener address. server.host defaults to 127.0.0.1 and
server.port to 3000 for standard execution. serve and serve_router take an
explicit address instead, ignore those server keys for binding, and permit port
zero.

build_router(&application) returns the raw generated Axum router. Merge native
routes into that raw router first, then call configure_router for direct
in-process use or pass the raw merged router to serve_router. Final router
configuration applies CORS exactly once as the outermost layer, so generated
and native routes receive the same CORS policy. Do not pass an already
configured router to serve_router.

[server.cors] is opt-in and strict. It validates origin, method, header,
credential, and max-age settings before middleware construction; wildcard
origins or wildcard headers cannot be combined with credentials. CORS controls
browser access to responses, not authorization or CSRF protection. A
cookie-authenticated application needs its own CSRF policy.

## Configuration and persistence

The core configuration model retains deterministic source precedence,
scalar/string-array replacement, interpolation, and source attribution. TOML
and programmatic sources support string arrays; EnvSource remains scalar-only.
Official database configuration is required only when the selected application
scope needs Database. database.pool_size defaults to 10 and database.migrate
defaults to false.

Embedded migrations remain an explicit low-level builder registration. When
database.migrate = true, one embedded source is required; pending migrations run
after database readiness, while no pending migration is a successful no-op.
Normal startup does not generate, auto-load, or auto-apply file migrations.
The explicit v0.7 `mads db generate` command can recursively load split Diesel
schema sources and produce one automatically named, review-required migration.
Reports retain stable reasons and source labels, never
resolved URLs, ports, origins, credentials, tokens, or keys.

Database::run is the boundary for synchronous native Diesel queries. It checks
out a pool connection and uses deadpool-diesel's blocking interaction,
preserving configuration, pool, interaction, query, and migration failure
classification. MADS deliberately does not hide native Diesel imports or map
database errors automatically into HTTP responses.

## Passport/JWT construction and request flow

The official JWT default activates only when the selected provider or guarded
route scope requires JwtService. An explicit concrete JwtService backs the
default off before configuration is parsed. Otherwise passport.secret selects
simple HS256 mode, and named key rings support HS256/384/512, RS256/384/512,
and ES256/384 with one algorithm per key and one active signer. Algorithms are
an application allowlist; an untrusted JWT header never expands it.

~~~text
effective guard
  -> extract exactly one Bearer or named-cookie token
  -> enforce size, configured key/algorithm, signature, claims, token kind
  -> invoke the visible managed strategy with verified claims + sanitized context
  -> roles clause AND permissions clause AND all predicates
  -> install Authenticated<P> and VerifiedToken<C>
  -> invoke handler
~~~

UserPrincipal is the application identity, distinct from signed UserClaims. It
implements PassportPrincipal manually or via the derive's roles/permissions
fields. Route-trait guards inherit; a method guard replaces only fields it
supplies, and #[guard(skip)] is the sole inherited policy opt-out.
Authentication and strategy rejection map to redacted 401 responses,
authorization failure to 403, and operational failure to 500.

Native Axum PassportGuard<P> uses the same runtime but is not static MADS guard
metadata. It therefore cannot activate JWT auto-configuration; its built
application context must already contain JwtService through a selected managed
dependency or an explicit value.

## Deliberately deferred

Version 0.7.0 remains PostgreSQL-only and does not add trait or interface bindings,
Inject<dyn Trait>, request-validation derives or schemas, login or credential
validation, refresh endpoints or persistence/rotation/revocation, password
hashing, CSRF, remote JWKS, JWE, MySQL/SQLite, generic typed configuration,
third-party auto-configuration registration, proactive schema validation,
multiple listeners, TLS, or HTTP/2-specific server configuration. Database
errors remain application delivery-policy decisions. Request input validation,
expanded standard HTTP errors, generic typed configuration,
compiler-diagnostic rewriting, and machine-readable CLI output are deferred to
v0.8; v0.7 CLI inspection and bounded generation are implemented now.

The v0.6 record's migration-generation and `mads doctor` deferrals were
superseded by the v0.7 CLI decision record on 2026-09-01; the historical v0.6
architecture remains otherwise unchanged.
