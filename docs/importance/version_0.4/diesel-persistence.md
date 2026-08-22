# v0.4 — Diesel Persistence

## Objective and boundary

v0.4 adds explicit, PostgreSQL-only Diesel persistence to MADS.rs. It keeps
`mads-core` generic and database-free while `mads-common` owns the pool,
migrations, lifecycle hook, and native Diesel escape hatch. MySQL and SQLite
are not supported in this release.

## Configuration, dotenv, and precedence

The database keys are `database.url` (required), `database.pool_size` (default
`10`), and `database.migrate` (default `false`). Store only a placeholder in
tracked `mads.toml`:

```toml
[database]
url = "${DATABASE_URL}"
pool_size = 10
migrate = false
```

`ConfigBuilder::dotenv(DotenvSource::optional(".env"))` loads generic dotenv
variables into an interpolation map without mutating process state. Exact
whole-value `${NAME}` placeholders resolve after TOML/config sources merge.
Later dotenv files override earlier ones; real process variables override
dotenv variables. Sources merge in insertion order, so loading
`EnvSource::new("MADS_")` last makes `MADS_DATABASE__URL` a direct
`database.url` override. The same generic mechanism can later resolve
`redis.url = "${REDIS_URL}"` without a database-specific loader.

## Git secret policy

The root `.gitignore` contains:

```gitignore
.env
.env.*
!.env.example
```

Commit `.env.example` with dummy values only, copy it locally to ignored
`.env`, and provide real production/CI values through process variables. Do
not commit a real `.env` or literal secret URLs in `mads.toml`.

## Explicit bootstrap and lifecycle

Applications construct `DatabaseConfig` from resolved `Config`, create a
builder with that config, then explicitly register:

```rust,ignore
builder.database(
    DatabaseBootstrap::new(database_config).with_migrations(MIGRATIONS),
)?;
```

`DatabaseBootstrap` supplies exactly one shared `Database` graph value and a
lifecycle hook. Serving follows this order: build/validate graph, build/validate
router, start lifecycle, check database, run configured embedded migrations,
bind listener, serve, then shut down lifecycle and close the pool. Invalid
routes therefore prevent database connections. When `database.migrate = true`,
an embedded migration source is required.

## Pool, queries, and errors

`Database::run` acquires from the managed PostgreSQL pool and runs synchronous
Diesel work through deadpool-diesel's blocking interaction. Configuration,
pool, interaction, query, and migration failures preserve their classification
and source context; debug output redacts database URLs. Import native query
types from `mads::diesel` when the framework has no useful wrapper.

## Migrations

Embedded migrations may run at application startup. The `mads db migrate`,
`mads db rollback`, and `mads db status` commands load file migrations from the
project `migrations/` directory. They respectively apply pending work, revert
the latest migration owned by that source, and list deterministic applied and
pending versions. Command output uses `applied <version>`, `reverted <version>`,
and a `summary: <applied> applied, <pending> pending` status line.

## Non-goals

v0.4 does not auto-configure or retry/back off database setup; that is a v0.5
candidate. It also does not add MySQL/SQLite, automatic request validation, or
automatic HTTP normalization of database errors. Applications choose their own
HTTP error mapping.

## Acceptance commands

The local release gates are:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace --all-features --doc
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
cargo +1.85.0 test --locked --workspace --all-features
```

With `MADS_TEST_DATABASE_URL` pointing at PostgreSQL 16, also run:

```sh
cargo test -p mads-common --test database_postgres -- --ignored --test-threads=1
cargo test -p mads-common server::tests::database_migration_failure_prevents_listener_binding -- --ignored --test-threads=1
cargo test -p mads-cli --test database_cli -- --ignored --test-threads=1
cargo test -p mads --test postgres_crud -- --ignored --test-threads=1
cargo llvm-cov --workspace --all-features --ignore-filename-regex '(^|/)tests/ui/' --fail-under-lines 85 -- --include-ignored --test-threads=1
```

CI provisions PostgreSQL 16 and enforces these ignored integration suites and
the 85% coverage threshold.
