# Changelog

All notable changes to MADS.rs are documented in this file.

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
