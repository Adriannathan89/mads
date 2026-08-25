# MADS.rs 0.5 Architecture

MADS.rs separates framework-neutral application semantics from HTTP delivery
and PostgreSQL persistence. Version 0.5 adds an official, conditional database
default between complete-catalog discovery and graph validation. The core can
load generic TOML and dotenv configuration without acquiring a database or HTTP
dependency; applications still load that configuration explicitly.

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
mads-common: route validation, official Diesel default, Axum adapter
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

`mads-common` is the Axum and PostgreSQL integration. It validates route
metadata, builds routers, supplies the official Diesel default, owns its
managed deadpool-diesel PostgreSQL pool and infrastructure lifecycle, and
exposes embedded/file migration operations. `DatabaseBootstrap` remains the
explicit native override; custom `Database` providers own their complete
lifecycle. `mads` is the facade that re-exports the standard v0.5 API,
including native `diesel` and `diesel_migrations` escape hatches.

## Startup sequence

Configuration loading is explicit. The release build sequence is fixed:

```text
explicit configuration
  -> complete catalog
  -> auto-configuration evaluation
  -> virtual graph validation
  -> active default application
  -> ordinary provider construction
  -> route validation
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
configuration. A typical database key is `database.url = "${DATABASE_URL}"`.
Process variables override dotenv values during interpolation; the final
`EnvSource::new("MADS_")` source maps `MADS_DATABASE__URL` directly to
`database.url` and wins over earlier sources. `Mads::builder()` does not load
those sources. With a direct `Database` requirement and no override, the
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

## Deliberately deferred

v0.5 is PostgreSQL-only and does not add automatic configuration loading,
chained defaults, priority selection, public third-party registration,
module-scoped evaluation, migration generation, proactive schema validation,
MySQL/SQLite support, request/domain error normalization, `mads doctor`, or
HTTP listener auto-binding. The listener address remains explicit; HTTP
auto-binding is assigned to v0.5.5.
