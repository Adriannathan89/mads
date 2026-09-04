# MADS CLI

MADS v0.7.0 provides a Cargo-native command line for running one MADS
application, supervising it during development, inspecting its compiled
metadata, and managing PostgreSQL migrations. Output is deterministic and
human-readable. There is no JSON or other machine-readable output contract in
v0.7.

## Project and target selection

The CLI starts from the current working directory and resolves the Cargo
workspace with Cargo metadata. With one workspace package and one binary,
`mads run` and `mads dev` need no selectors. Use `--package <package>` or
`-p <package>` to select a package and `--bin <binary>` to select a binary:

```text
mads run [--package <package>] [--bin <binary>] [-- <app-args>...]
mads dev [--package <package>] [--bin <binary>] [-- <app-args>...]
```

The inspection commands accept the same package and binary selectors but do
not accept application arguments:

```text
mads routes [--package <package>] [--bin <binary>]
mads graph  [--package <package>] [--bin <binary>]
mads doctor [--package <package>] [--bin <binary>]
```

Database commands accept `--package <package>` or `-p <package>`. If a
selector is omitted, MADS follows the Cargo project model: a single eligible
package or binary is selected, a declared `default-run` is honored, and an
ambiguous target reports the Cargo-style choice rather than inventing a new
default.

For `run` and `dev`, every argument after `--` is forwarded unchanged to the
selected application. For `routes`, `graph`, and `doctor`, `--` is rejected
because inspection is not an application invocation.

## mads run

`mads run` builds the selected binary with Cargo and starts it with the
forwarded arguments. A plain invocation is the standard one-package,
one-binary workflow:

```bash
mads run
mads run -- --seed-data
mads run -p api --bin server -- --port 4000
```

The selected application is responsible for its normal startup and shutdown.
MADS preserves an ordinary application exit status. A successful application
therefore returns 0, while an application status such as 3 is returned as 3.
Cargo resolution, build, process, and other operational failures return 1;
invalid CLI syntax returns 2.

## mads dev

`mads dev` builds and supervises the selected application, then watches the
reachable local workspace packages used by that target. Relevant source files,
`Cargo.toml`, `Cargo.lock`, migrations, and the selected package's
configuration files participate in the watch set. Generated `target` files,
`.git` files, editor backups, and unreachable nested package sources are
ignored.

Events are coalesced with a 150 ms debounce. Rust/source, Cargo, and migration
changes rebuild the application. A change to the selected package's
`mads.toml` or `.env` restarts the existing build without treating it as a
source rebuild. A batch containing both kinds of change is a rebuild.

The supervisor stops the old process before replacing it. A failed rebuild
keeps the last good process when one is running and continues watching; it
does not hot-reload Rust code or provide hot module replacement. If the
application exits, MADS waits for a relevant change. Ctrl-C cancels an active
build, stops the child process, and exits after cleanup.

Typical usage is:

```bash
mads dev
mads dev -p api --bin server -- --log=debug
```

The status lines `mads dev: watching`, `rebuilding`, `restarting`,
`build failed; continuing to watch`, and `exiting` describe the supervisor
state; they are human-readable diagnostics, not a structured output protocol.

## mads routes

`mads routes` compiles the selected standard MADS application, asks its private
inspection child for route metadata, and exits without normal application
startup. The table columns are:

```text
METHOD  PATH  ROUTE  CONTROLLER  GUARD  SOURCE
```

`ROUTE` is the route trait and handler, `GUARD` is `yes` or `no`, and `SOURCE`
is the Rust source location as `file:line:column`. Routes are sorted by method,
path, controller, route trait, and handler. An empty report prints `(none)`.

## mads graph

`mads graph` reports the selected application graph in four sections:

```text
Modules
Providers
Dependencies
Construction order
```

Modules show the rooted import tree. Providers include owner, origin,
visibility, and state. Dependencies use `provider -> dependency` edges, and a
known construction order is numbered. Empty sections remain visible as
`(none)` so a partial report is explainable.

## mads doctor

`mads doctor` runs the same private inspection protocol and presents checks for
configuration, the module graph, providers, routes, guards/strategies,
server/CORS, and auto-configuration. Each row is prefixed with one of:

```text
PASS  SKIPPED  OVERRIDDEN  FAILED
```

Checks are sorted by those stable groups. `SKIPPED` explains an inactive
optional feature; `OVERRIDDEN` records an application replacement for an
official default; `FAILED` is accompanied by a diagnostic. A failed inspection
prints the available partial report and returns exit code 1.

## mads db generate

`mads db generate` creates one complete current schema-to-database diff with an
automatic timestamp-based name. It has no positional migration name. The
supported forms are:

```bash
mads db generate
mads db generate -p api
```

Schema sources may be kept in one `src/schema.rs` file or split into
`src/schema/**/*.rs`; nested files are discovered recursively in lexical order.
For example, `src/schema/user.rs` and `src/schema/comment.rs` are loaded
together. The source must use regular Diesel `table!` declarations supported by
the v0.7 schema parser.

Generation loads the selected package's conventional configuration using the
same `.env` interpolation, `mads.toml`, and final `MADS_*` override chain used
by database operations. It reads the live PostgreSQL schema, compares it to
the desired Diesel schema, and publishes `up.sql` and `down.sql` atomically in
the package's `migrations/` directory. No external Diesel CLI is required.

The generated migration is review-required: MADS prints warnings for schema
changes whose SQL semantics are not fully synthesized, then prints the path
and `review up.sql and down.sql before applying`. Generation never applies the
files. When the desired and live schemas match, it prints `schema is up to
date` and creates no migration.

v0.7 intentionally supports a bounded schema shape. Tables, columns, supported
PostgreSQL types, primary keys, and the safe diff operations implemented by the
schema planner are synthesized. Defaults, indexes, checks, triggers, and a
complete foreign-key policy are not inferred from Diesel declarations; these
must be reviewed and authored in migration SQL as appropriate. `--diff-schema`
and positional names are invalid arguments.

## mads db migrate / rollback / status

These commands operate on the selected package's file-based `migrations/`
directory and configured PostgreSQL database:

```bash
mads db migrate -p api
mads db rollback -p api
mads db status -p api
```

`migrate` applies pending migrations and prints `applied <version>` or
`database is up to date`. `rollback` reverts the latest applied migration and
prints `reverted <version>`. `status` lists applied and pending versions and
ends with an applied/pending summary. These are explicit operations; normal
application startup does not discover, generate, or automatically apply
file-based migrations.

## Diagnostics and exit codes

Exit codes are:

| Code | Meaning |
| --- | --- |
| 0 | Command completed successfully. |
| 1 | Build, Cargo resolution, inspection, database, watcher, or other operational failure. For `run`, this also represents a non-success application failure when the application does not return an ordinary code. |
| 2 | Invalid MADS CLI syntax or unsupported command/argument. |

CLI diagnostics use stable `MADS2xx` codes. Common codes include `MADS200`
for target resolution, `MADS201` for Cargo metadata, `MADS202` for application
process failures, `MADS210` for schema loading, `MADS211`/`MADS212` for schema
planning or SQL rendering, `MADS213` for migration publication, and `MADS220`
for file-watcher failures. Diagnostics identify a subject and source location
when available and may include `help:` suggestions.

Operational diagnostics redact configuration values, credentials, URLs, local
paths, and process details that are not part of the public human-readable
contract. Private inspection tokens and child-process acknowledgement paths do
not appear in normal command output.

## Platform and standard-entry-point limits

The primary v0.7 release gate runs on Linux. It performs complete workspace,
PostgreSQL, coverage, MSRV, and packaging verification. macOS and Windows are
not release-gate platforms in v0.7; users on those systems should run the
documented CLI commands locally. This keeps the release gate focused on one
reproducible machine while preserving the runtime's portable code paths.

App-aware `routes`, `graph`, and `doctor` inspection is intentionally limited
to the standard `Mads::run::<AppModule>()` entry point. MADS builds the
selected binary, starts a short-lived private child mode, receives the
compiled report, and terminates that child. The parent does not construct
providers, start lifecycle hooks, connect to PostgreSQL, run migrations, bind a
socket, or serve traffic for inspection.

Code and build-script effects that occur before the standard MADS entry point
remain Cargo/application responsibilities and cannot be hidden by the
inspection boundary. Low-level builder-only applications and arbitrary custom
entry points are not app-aware inspection targets in v0.7.
