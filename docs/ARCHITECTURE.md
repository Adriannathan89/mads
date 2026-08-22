# MADS.rs 0.4 Architecture

MADS.rs separates framework-neutral application semantics from HTTP delivery
and explicit PostgreSQL persistence. The core can load generic TOML and dotenv
configuration without acquiring a database or HTTP dependency.

```text
Application providers, route traits, and controllers
                 |
                 v
        mads macros: metadata + typed registrars
                 |
                 v
 mads-core: config, provider graph, lifecycle, diagnostics
                 |
                 v
mads-common: route validation, Diesel database, Axum adapter
                 |
                 v
      PostgreSQL + Diesel       Axum + Tower + Tokio
```

## Crate boundary

`mads-core` owns generic configuration, providers, construction order,
lifecycle, diagnostics, and the application context. Its dotenv support reads
temporary interpolation values only; it neither has database semantics nor
mutates the process environment.

`mads-common` is the Axum and PostgreSQL integration. It validates route
metadata, builds routers, owns the managed deadpool-diesel PostgreSQL pool,
and exposes embedded/file migration operations. `mads` is the facade that
re-exports the standard v0.4 API, including native `diesel` and
`diesel_migrations` escape hatches.

## Startup sequence

`DatabaseBootstrap` explicitly registers a shared `Database` provider and its
lifecycle hook. Runtime ordering is fixed:

```text
build/validate graph
build/validate router
start lifecycle
check database
run configured embedded migrations
bind listener
serve
shutdown lifecycle and close pool
```

Route validation and router construction happen before lifecycle startup.
Therefore invalid routes prevent database connection attempts as well as
listener binding. A database readiness or migration failure prevents binding;
after a successful start, bind and serving failures still attempt lifecycle
shutdown, which closes the shared pool.

## Configuration and persistence

`ConfigBuilder` can merge optional dotenv variables, TOML, and process
configuration. A typical database key is `database.url = "${DATABASE_URL}"`.
Process variables override dotenv values during interpolation; the final
`EnvSource::new("MADS_")` source maps `MADS_DATABASE__URL` directly to
`database.url` and wins over earlier sources. `DatabaseConfig` then validates
the resolved `database.url`, with `database.pool_size` defaulting to 10 and
`database.migrate` defaulting to false.

`Database::run` is the boundary for synchronous native Diesel queries. It
checks out a pool connection and uses deadpool-diesel's blocking interaction,
preserving configuration, pool, interaction, query, and migration failure
classification. MADS deliberately does not hide native Diesel imports or map
database errors automatically into HTTP responses.

## Deliberately deferred

v0.4 is PostgreSQL-only and requires explicit registration. It does not add
automatic database configuration or retry/back-off, MySQL/SQLite support,
automatic validation, request/domain error normalization, MADS middleware
abstractions, generated OPTIONS handlers, trailing-slash redirects, request
scopes, or automatic listener configuration.
