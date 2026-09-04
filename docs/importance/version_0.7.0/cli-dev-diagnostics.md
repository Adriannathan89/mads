# v0.7.0 CLI, Development Loop, and Diagnostics

**Status:** Implemented in 0.7.0-beta.1  
**Date:** 2026-09-01

This record is the evidence-backed v0.7 decision boundary. The beta contains
the complete approved command surface; stable promotion may correct defects,
tests, documentation, and release verification only.

## Command and selector matrix

| Area | Supported form | Result |
| --- | --- | --- |
| Help/version | `mads --help`, `mads --version` | Human-readable exit 0. |
| Execution | `mads run`, with `-p`/`--package`, `--bin`, and `--` | Cargo builds one selected binary and preserves its ordinary exit status. |
| Development | `mads dev`, with the execution selectors and forwarded arguments | Starts the supervisor and watches the selected reachable application. |
| Route inspection | `mads routes`, with package/binary selectors | Reports method, path, route, controller, guard, and source. |
| Graph inspection | `mads graph`, with package/binary selectors | Reports modules, providers, dependencies, and construction order. |
| Diagnostics | `mads doctor`, with package/binary selectors | Reports grouped `PASS`, `SKIPPED`, `OVERRIDDEN`, and `FAILED` checks. |
| Generation | `mads db generate`, optionally `-p`/`--package` | Generates one automatically named, review-required schema diff. |
| Migration apply | `mads db migrate`, optionally `-p`/`--package` | Applies file-based pending migrations. |
| Migration rollback | `mads db rollback`, optionally `-p`/`--package` | Reverts the latest applied migration. |
| Migration status | `mads db status`, optionally `-p`/`--package` | Lists applied/pending versions and a summary. |

`mads foundation`, named generation, `--diff-schema`, and application
arguments on inspection commands are invalid. Syntax errors return 2;
operational failures return 1. An application ordinary exit code is preserved
by `mads run`.

## Process protocol and diagnostics

`routes`, `graph`, and `doctor` build the selected Cargo target, start a
short-lived private inspection child, receive compiled metadata through the
private protocol, render it, and terminate the child. The inspection path does
not construct providers, start lifecycle hooks, connect to PostgreSQL, run
migrations, bind a listener, or serve traffic. Effects before the standard
`Mads::run::<AppModule>()` entry point remain application or build-script
responsibilities.

The CLI diagnostic family is:

| Code | Boundary |
| --- | --- |
| `MADS200` | Cargo target selection. |
| `MADS201` | Cargo metadata. |
| `MADS202` | Application process. |
| `MADS210` | Diesel schema loading. |
| `MADS211`/`MADS212` | Schema planning and SQL rendering. |
| `MADS213` | Migration publication. |
| `MADS220` | File watching. |

Diagnostics are human-readable, stable in ordering, and redact configuration
values, credentials, URLs, and private protocol paths.

## Watcher and last-good semantics

The watcher includes reachable local package sources, Cargo manifests and
lockfiles, migrations, and the selected package's `.env` and `mads.toml`.
Generated `target` and `.git` paths, editor backups, and unreachable nested
package sources are ignored. Events use a 150 ms debounce. Source, Cargo, and
migration changes rebuild; selected-package configuration changes restart. A
rebuild dominates a restart in one event batch.

The supervisor stops and replaces the application process only after a
successful build. A failed rebuild keeps the last good process and continues
watching. An exited application is not immediately rebuilt without a relevant
change. Ctrl-C cancels an active build, stops the child, and cleans up. The
implementation intentionally has no hot module replacement.

## Schema-diff boundary

| Schema concern | v0.7 behavior |
| --- | --- |
| Source layout | `src/schema.rs` or recursive `src/schema/**/*.rs`, loaded in lexical order. |
| Tables/columns/types | Supported Diesel declarations and supported PostgreSQL types are diffed. |
| Primary keys | Included in the supported table shape. |
| Defaults/indexes/checks/triggers | Not inferred; review and author migration SQL. |
| Foreign keys | No complete policy synthesis; review and author required constraints. |
| Migration naming | One automatic timestamp-based name; no positional name. |
| Publication | `up.sql` and `down.sql` are written atomically under `migrations/`. |
| Safety | Generated SQL is review-required and never auto-applied. |
| External dependency | No external Diesel CLI is required. |
| No diff | Prints `schema is up to date` and creates no migration. |

Generation uses the selected package's `.env` interpolation, `mads.toml`, and
final `MADS_*` configuration chain, then reads live PostgreSQL state.

## Verification evidence

Focused beta checks:

```bash
cargo test -p mads-cli --test command_matrix -- --test-threads=1
cargo test -p mads-cli -- --test-threads=1
cargo clippy -p mads-cli --all-targets --all-features -- -D warnings
cargo test --workspace --all-features --doc
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
```

The v0.7 release workflows use Linux as the sole verification platform.
Complete workspace, CLI, PostgreSQL, MSRV, coverage, and package evidence runs
in the Linux jobs in `.github/workflows/ci.yml`,
`.github/workflows/beta-publish.yml`, and
`.github/workflows/stable-publish.yml`. macOS and Windows are intentionally
outside the v0.7 release gate. Real database evidence remains in the
PostgreSQL 16 service job:

```bash
cargo test -p mads-common --test database_postgres -- --ignored --test-threads=1
cargo test -p mads-common server::tests::database_migration_failure_prevents_listener_binding -- --ignored --test-threads=1
cargo test -p mads-cli --test database_cli -- --ignored --test-threads=1
cargo test -p mads-cli --test database_generate_postgres -- --ignored --test-threads=1
cargo test -p mads --test postgres_crud -- --ignored --test-threads=1
```

These PostgreSQL commands require an isolated PostgreSQL 16 database through
`MADS_TEST_DATABASE_URL`; CI provisions it rather than moving SQL round trips
to the platform matrix.

## Deferred to v0.8

The approved v0.8 boundary remains:

- `#[derive(Input)]`, validators, and deserialize/validate/handler integration;
- structured validation responses;
- expanded standard HTTP application errors;
- automatic Diesel-to-HTTP error mapping;
- generic typed configuration and dedicated secret-safe value APIs;
- broader opaque trait-bound and compiler diagnostic improvements;
- JSON or other machine-readable CLI output.
