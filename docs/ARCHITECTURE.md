# MADS.rs 0.5.5 Architecture

MADS.rs separates framework-neutral application semantics from HTTP delivery
and PostgreSQL persistence. Version 0.5.5 adds generic string-array
configuration and feature-gated cookies, JWT, managed Passport strategies, and
guards without adding HTTP or cryptography to the core. Applications still
load configuration explicitly.

```text
Application providers, route traits, and controllers
                 |
                 v
        mads macros: metadata + typed registrars
                 |
                 v
 mads-core: config, auto-configuration, provider graph, lifecycle, diagnostics
                 |
                 v
mads-common: routes, cookies, Passport/JWT, Diesel default, Axum adapter
                 |
                 v
      PostgreSQL + Diesel       Axum + Tower + Tokio
```

## Crate boundary

`mads-core` owns generic configuration, official conditional-default evaluation,
public redacted reports, providers, graph validation, construction order,
lifecycle, diagnostics, and the application context. Its dotenv support reads
temporary interpolation values only; it neither has database semantics nor
mutates the process environment. The v0.5 requirement catalog is the complete
statically discovered provider catalog. In v0.6, module scoping will replace it
with the subset reachable from a root `AppModule`; modules, imports, exports,
and visibility rules do not ship in v0.5.

`mads-common` is the Axum, Passport/JWT, cookie, and PostgreSQL integration. It
validates route and guard metadata, builds routers, supplies the official
Diesel default, owns its
managed deadpool-diesel PostgreSQL pool and infrastructure lifecycle, and
exposes embedded/file migration operations. `DatabaseBootstrap` remains the
explicit native override; custom `Database` providers own their complete
lifecycle. `mads` is the facade that re-exports the standard v0.5 API,
including native Axum, cookie, Passport guard, `diesel`, and
`diesel_migrations` escape hatches.

The feature boundary is deliberate:

```text
jwt                 JWT service/configuration; no Axum or Diesel
cookies             cookie request/response support; includes HTTP
http + jwt          Passport strategies and Bearer guards
http + jwt + cookies
                    cookie-sourced guards
common              compatibility aggregate for HTTP + database only
```

`mads-core` contains scalar/string-array merge semantics and origin tracking,
but no Axum, cookie, JSON Web Token, or cryptography dependency.

## Startup sequence

Configuration loading is explicit. The release build sequence is fixed:

```text
explicit configuration
  -> complete provider, route, guard, and strategy catalogs
  -> guard metadata and managed-strategy preflight
  -> auto-configuration evaluation
  -> virtual graph validation
  -> active default application
  -> ordinary provider construction
  -> route validation and guarded router construction
  -> infrastructure lifecycle
  -> application lifecycle
  -> bind
```

An active default is applied only after the virtual graph is valid, and before
ordinary providers construct. Route validation happens before lifecycle startup,
so invalid routes prevent database checkout, readiness, migrations, application
hooks, and listener binding. Infrastructure hooks start in lexical descriptor
order before application hooks in registration order. The database lifecycle
checks readiness, then runs pending embedded migrations only when
`database.migrate = true`, before later hooks and binding.

Shutdown and startup rollback exactly reverse successfully started hooks:
application hooks stop in reverse registration order, then infrastructure hooks
stop in reverse lexical descriptor order. Therefore application shutdown hooks
run before the database closes. A bind or serving failure after startup follows
the same teardown order.

## Configuration and persistence

`ConfigBuilder` can merge optional dotenv variables, TOML, and process
configuration:

```rust,ignore
let config = ConfigBuilder::new()
    .dotenv(DotenvSource::optional(".env"))
    .source(TomlSource::file("mads.toml"))
    .source(EnvSource::new("MADS_"))
    .build()?;
let application = Mads::builder_with_config(config).build().await?;
```

Dotenv values are interpolation inputs, not configuration entries. Process
variables override dotenv values during interpolation. Ordinary sources merge
first to last; the final `EnvSource::new("MADS_")` maps
`MADS_DATABASE__URL` to `database.url` and wins over earlier sources. A later
scalar or string array replaces the earlier shape completely. TOML and
programmatic sources support string arrays; `EnvSource` remains scalar-only.
`Mads::builder()` does not load any source. With a direct `Database`
requirement and no override, the
official default validates the resolved `database.url`; `database.pool_size`
defaults to 10 and `database.migrate` defaults to false.

Startup migrations use at most one separately registered embedded source.
When enabled, a source is required; existing pending migrations apply and no
pending migrations are a successful no-op. When disabled, MADS neither applies
nor proactively checks migrations. It never generates migrations, derives
schema differences, or auto-loads a migration directory. Retained
auto-configuration reports contain stable reasons and source labels, never
resolved configuration values, URLs, or credentials.

`Database::run` is the boundary for synchronous native Diesel queries. It
checks out a pool connection and uses deadpool-diesel's blocking interaction,
preserving configuration, pool, interaction, query, and migration failure
classification. MADS deliberately does not hide native Diesel imports or map
database errors automatically into HTTP responses.

## Passport/JWT construction and request flow

The official JWT default activates when a provider directly requires
`JwtService` or a non-skipped static route guard exists. An explicit concrete
`JwtService` backs the default off before configuration is parsed. Otherwise
`[passport] secret = "${JWT_SECRET}"` selects simple HS256 mode, and named key
rings support HS256/384/512, RS256/384/512, and ES256/384 with one algorithm per
key and one active signer. Algorithms are an application allowlist; an
untrusted JWT header never expands it. Reports retain configuration keys and
source labels, never resolved values or key bytes.

Strategy eligibility in v0.5.5 is the conjunction of:

```text
implements PassportStrategy
  + #[passport_strategy(name = "...")] descriptor
  + concrete type is a managed provider in the complete static catalog
```

One eligible custom strategy overrides the built-in `jwt`; duplicate custom
names are ambiguous. `jwt` consumes access tokens. `jwt-refresh` is an
application-defined name whose strategy declares `JwtTokenKind::Refresh`.
Framework verification always precedes application validation.

```text
effective guard
  -> extract exactly one Bearer or named-cookie token
  -> enforce size, configured key/algorithm, signature, claims, token kind
  -> invoke managed strategy with verified claims + sanitized context
  -> roles clause AND permissions clause AND all predicates
  -> install Authenticated<P> and VerifiedToken<C>
  -> invoke handler
```

`UserPrincipal` is the current application identity, distinct from signed
`UserClaims`. It implements `PassportPrincipal` manually or via the derive's
`#[roles]`/`#[permissions]` fields. Route-trait guards inherit. A method guard
replaces only fields it supplies, and `#[guard(skip)]` is the sole inherited
policy opt-out. Within a role or permission clause, `any` needs one match and
`all` needs every match; separate clauses and synchronous principal predicates
are ANDed. One guard uses one source and never falls back from a cookie to a
Bearer header.

Authentication and strategy rejection map to redacted `401` responses with
`WWW-Authenticate: Bearer`, authorization failure to `403`, and operational
failure to `500`. Ordinary malformed cookie extraction is `400`; missing,
malformed, or duplicated target guard cookies are generic `401` responses.

Native Axum `PassportGuard<P>` is backed by the same runtime but is not part of
the static route catalog. It therefore cannot activate JWT auto-configuration.
Its completed application context must already have `JwtService` through a
managed provider dependency or explicit value; otherwise native guard build
fails with `MADS131`.

v0.6.0 will filter the stable strategy descriptors by root-module reachability
and provider export eligibility before name resolution. v0.5.5 intentionally
uses the complete catalog and implements no module scoping.

## Deliberately deferred

v0.5.5 is PostgreSQL-only and does not add automatic configuration loading,
chained defaults, priority selection, public third-party registration,
module-scoped evaluation, migration generation, proactive schema validation,
MySQL/SQLite support, request/domain error normalization, `mads doctor`, or
HTTP listener auto-binding. Authentication does not include login, credential
validation, password hashing, a refresh endpoint, refresh persistence,
rotation/revocation/reuse detection, CSRF, CORS, remote JWKS, or JWE. The
listener address remains explicit. CORS and auto-binding are assigned to
v0.5.6; module reachability/export enforcement is assigned to v0.6.0.
