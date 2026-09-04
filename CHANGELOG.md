# Changelog

All notable changes to MADS.rs are documented in this file.

## [0.7.0-beta.1] - 2026-09-01

Complete beta of the MADS.rs CLI, development loop, and framework diagnostics.

### Included

- Cargo-native `mads run` and `mads dev` with package, binary, and argument forwarding.
- `mads routes`, `mads graph`, and `mads doctor` compiled application inspection.
- `mads db generate` with automatic naming and recursive split-schema loading.
- `mads db migrate`, `mads db rollback`, and `mads db status` database operations.
- Human-readable diagnostics, stable exit classes, redaction, and Linux CI coverage.
- Bounded PostgreSQL schema diff generation with review-required reversible SQL.

### Beta limitations

- Inspection supports the standard `Mads::run::<AppModule>()` entry point only.
- Unsupported schema details such as defaults, indexes, checks, triggers, and complete foreign-key policy require manual SQL review.
- Output is human-readable only; no machine-readable mode is part of v0.7.
- Input validation, expanded HTTP errors, generic typed configuration, and related validation work are deferred to v0.8.

## [0.6.0-beta.1] - 2026-08-29

First public beta of the MADS.rs HTTP application foundation.

### Included

- Root-module application scope and managed dependency construction.
- Typed HTTP route contracts and managed controllers on Axum.
- Conventional HTTP startup, configuration, CORS, and graceful shutdown.
- PostgreSQL/Diesel integration with managed lifecycle and migrations.
- JWT, cookie, Passport strategy, principal, and route guard support.
- Native Axum and Diesel escape hatches for application-owned composition.

### Beta limitations

- Declarative validation, OpenAPI generation, and generic trait bindings are not included.
- TLS, HTTP/2 configuration, multiple listeners, and declarative middleware are application-owned.
- Public APIs may change in later `0.6.0-beta.*` releases based on adopter feedback.

[0.6.0-beta.1]: https://github.com/Adriannathan89/mads/releases/tag/v0.6.0-beta.1
[0.7.0-beta.1]: https://github.com/Adriannathan89/mads/releases/tag/v0.7.0-beta.1
