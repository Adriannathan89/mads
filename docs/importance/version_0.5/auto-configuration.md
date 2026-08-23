# v0.5 — Auto-Configuration Engine Requirements

**Status:** Design approved; implementation has not started.

The authoritative design is
[`docs/superpowers/specs/2026-08-23-v0.5-auto-configuration-design.md`](../../superpowers/specs/2026-08-23-v0.5-auto-configuration-design.md).
The task-by-task implementation plan is
[`docs/superpowers/plans/2026-08-23-v0.5-auto-configuration.md`](../../superpowers/plans/2026-08-23-v0.5-auto-configuration.md).

## Objective and compatibility boundary

v0.5 adds deterministic official auto-configuration without replacing the
provider dependency injection already used by `#[service]`, `#[repository]`,
and `#[provider]`.

The engine combines:

```text
complete-catalog requirement
        +
linked official capability
        +
explicit application configuration
        ↓
conditional default provider
```

`mads-core` owns the generic engine. `mads-common` supplies the first official
integration: a PostgreSQL/Diesel `Database` default. Core must remain free of
Diesel, pools, migrations, PostgreSQL, Axum, and `mads-common` dependencies.

Rust 1.85, edition 2024, PostgreSQL 16, Diesel `=2.2.12`,
`diesel_migrations =2.2.0`, and `deadpool-diesel =0.6.1` remain fixed release
constraints.

## Evaluation and application

Analysis and application are separate phases:

```text
collect descriptors and provider catalog
        ↓
evaluate conditions once in stable identifier order
        ↓
add active outputs as virtual graph satisfactions
        ↓
validate the combined graph
        ↓
apply selected defaults
        ↓
construct ordinary providers
```

`MadsBuilder::analyze()` may read immutable provider metadata, registered
integration inputs, and configuration. It must not invoke an apply callback,
provider constructor, pool constructor, connection, migration, or lifecycle
hook. Repeated analysis is deterministic and side-effect-free.

Requirements come from every provider in the complete process catalog. The
engine evaluates official descriptors in one pass; one active default cannot
activate another. Module-scoped reachability replaces this complete-catalog
rule in v0.6.

Descriptor identifiers are globally unique and sorted lexically. Duplicate
identifiers or multiple active official defaults for the same output are
errors; v0.5 has no priority mechanism. Registration APIs remain hidden and
reserved for official MADS crates until module scope exists.

## Decisions and back-off

Every descriptor records one of these statuses:

| Status | Meaning |
| --- | --- |
| `ACTIVE` | All conditions match and the default is selected. |
| `SKIPPED` | No provider requires the output. |
| `OVERRIDDEN` | The application already controls the output. |
| `FAILED` | Condition evaluation or default application failed. |

The stable reason codes are `conditions_matched`, `requirement_absent`,
`user_override`, `missing_configuration`, `invalid_configuration`,
`missing_migration_source`, `provisioning_failed`, `duplicate_identifier`,
and `conflicting_default`.

Override checking occurs before requirement checking or configuration access.
If both an override and no requirement exist, the result is `OVERRIDDEN`.
These application-controlled `Database` forms cause complete back-off:

- `builder.provide(database)`;
- `builder.database(DatabaseBootstrap::new(...))`;
- `builder.construct::<Database>()`; and
- one static `#[provider]` returning `Database`.

Back-off means the default does not parse database configuration, construct a
pool, check readiness, run migrations, install lifecycle hooks, or close the
custom value. Multiple static providers still produce `OVERRIDDEN` for the
default and the existing provider ambiguity diagnostic `MADS002`.

v0.5 adds no global or per-integration enable/disable switch.

## Inspection contract

Public read-only report types expose the identifier, output type name, status,
reason code, explanation, every direct requiring provider, declaration
locations when available, and configuration key/source labels.

Reports are available through:

```rust,ignore
analysis.auto_configurations();
application.auto_configurations();
```

The provider graph separately identifies an active default with
`ProviderOrigin::AutoConfiguration` and `ProviderState::AutoConfigured`.
Direct consumers and reports use deterministic order.

Reports and diagnostics never retain or print resolved values, database URLs,
credentials, dotenv values, or process-environment values. Existing v0.4
source attribution remains unchanged: a value declared in `mads.toml` stays
attributed to `mads.toml` even when an exact placeholder is resolved from a
dotenv file or the process environment.

## Diesel database default

The descriptor identifier is `mads.common.database.diesel`. Its capability is
available when `mads-common` is linked through the existing `common` feature;
there is no runtime capability probe and no new feature flag.

The default activates only when at least one provider directly requires
`Database`, no application-controlled `Database` exists, and this established
configuration is valid:

```toml
[database]
url = "${DATABASE_URL}"
pool_size = 10
migrate = false
```

Configuration loading remains explicit with `ConfigBuilder` and
`Mads::builder_with_config(config)`. v0.5 does not make `Mads::builder()` load
files or environment variables. The v0.4 dotenv, interpolation, and ordered
source-precedence behavior is preserved.

An active default creates the existing lazy `Database` pool during build and
supplies it before ordinary provider constructors run. Application bootstrap
does not construct `DatabaseConfig`, `DatabaseBootstrap`, or call
`builder.database(...)`.

If no provider requires `Database`, database keys are not parsed and invalid or
missing unused values are `SKIPPED`. With a real requirement, missing or
invalid database configuration is `MADS101` and suppresses only the redundant
missing-`Database` `MADS003`; unrelated graph diagnostics remain visible.

## Embedded startup migrations

Automatic database startup accepts at most one application-wide embedded
migration source:

```rust,ignore
builder.database_migrations(MIGRATIONS)?;
```

When `database.migrate = true`, the source is required during analysis.
Existing pending embedded migrations run after readiness succeeds. If none are
pending, the migration step succeeds without doing work. Duplicate source
registration is `MADS101`.

When `database.migrate = false`, an optional registered source is accepted but
unused. MADS neither applies migrations nor proactively checks pending schema.
A later missing-table, missing-column, or other schema access failure remains a
structured database query error for the application to map to its domain or
HTTP policy.

v0.5 does not generate `up.sql` or `down.sql`, derive schema differences, read
a runtime migration directory during startup, or invoke Diesel CLI generation.
The existing `mads db migrate`, `mads db rollback`, and `mads db status`
commands retain their file-migration behavior, output, and exit codes.

## Lifecycle and diagnostics

Framework infrastructure hooks start in lexical descriptor order before
application hooks. Application hooks retain registration order. Shutdown and
startup rollback reverse the complete successful startup sequence, so
application hooks stop before the database pool closes.

HTTP route validation remains before lifecycle startup. Invalid routes prevent
database checkout, readiness, migrations, application hooks, and listener
binding. Database readiness or migration failure prevents application hooks
and listener binding.

Diagnostic ownership is:

| Code | Owner and use |
| --- | --- |
| `MADS007` | Core descriptor identifier/default conflicts. |
| `MADS101` | Database default condition or application failure. |
| `MADS100` | Explicit database bootstrap and database runtime failures. |
| `MADS011` | Outer lifecycle-hook failure context. |

An active default remains `ACTIVE` after it is applied. Readiness and migration
failures preserve the existing inner `MADS100` source and outer `MADS011`
lifecycle context; they do not rewrite the retained decision report.

## Deferred work

The following are outside v0.5:

- HTTP host/port auto-binding, scheduled for v0.5.5;
- module-scoped evaluation, scheduled for v0.6;
- public third-party auto-configuration registration, deferred until module
  scoping exists;
- richer configuration provenance and typed configuration UX, scheduled for
  v0.7;
- `mads doctor`, scheduled for v0.8;
- automatic migration generation and proactive schema validation; and
- MySQL or SQLite support.

## Release gates

The implementation is complete only after formatting, lint, unit/integration,
rustdoc, architecture, MSRV, PostgreSQL 16, and coverage checks pass. Required
commands are enumerated in the implementation plan. PostgreSQL acceptance must
prove zero-bootstrap CRUD, pending-migration application, the no-pending no-op,
migration-disabled query-error preservation, lifecycle order, and
failure-before-listener-bind behavior. Workspace coverage remains at least
85%.
