# MADS.rs 0.3 Architecture

MADS.rs separates framework-neutral application semantics from HTTP delivery.
The v0.3 runtime makes controller routes executable without making the core an
HTTP framework.

```text
Application route traits and controllers
                 |
                 v
        mads macros: metadata + typed registrars
                 |
                 v
 mads-core: provider graph, Mads, lifecycle, diagnostics
                 |
                 v
 mads-common: validation, Axum adapter, router, server
                 |
                 v
         Axum + Tower + Hyper + Tokio
```

## Crate boundary

`mads-core` owns providers, construction order, lifecycle, diagnostics, and
the application context. It remains HTTP-free: it has no Axum, Tower, Hyper,
HTTP, or `mads-common` dependency and contains no HTTP behavior.

`mads-common` is the Axum adapter. It validates route metadata, translates
validated MADS paths, resolves controllers, invokes typed registrars, and
offers HTTP extractors, response wrappers, `build_router`, and `serve`.
`mads-common-macros` generates metadata and typed adapters but does not link
the HTTP runtime. `mads` is the facade that re-exports the standard v0.3 API.

## Bootstrap sequence

```text
Mads::builder().build().await
        |
        v
provider graph validation and application-scoped construction
        |
        v
build_router(&application) or serve(application, address)
        |
        v
validate the complete RouteCatalog (MADS030 on failure)
        |
        v
translate validated /:parameter segments to Axum /{parameter}
        |
        v
resolve each controller once and invoke its typed registrar
        |
        +-- build_router: return Router<()> for composition/testing
        |
        +-- serve: start lifecycle, bind, serve, then shut down lifecycle
```

`serve` never starts lifecycle hooks or opens a listener if validation or
router construction fails. If startup succeeded, a later bind or serving error
still triggers shutdown; operational and shutdown failures retain both error
contexts.

## Typed registration and ownership

Route traits describe actual Rust method signatures. A generated registrar
uses fully qualified trait calls, so two traits may use the same handler name
without ambiguous dispatch. Handler names remain immutable inspection metadata;
they never select executable code. The registrar resolves the managed
controller while building the router and generated closures capture its
application-scoped handle. There is no manual `State<AppState>` requirement,
no string dispatch, and no per-request provider lookup.

## Validation and routing policy

The catalog validates all controllers before any `Router::route` call. It
rejects malformed or inconsistent metadata, duplicate controller identities,
empty contracts, conflicts, invalid source locations, missing registrars, and
invalid route grammar with `MADS030`. This is a fail-closed boundary for macro
output and manually constructed descriptors alike.

MADS preserves `/:id` route syntax in metadata, translating only validated
full paths to Axum 0.8 syntax. Axum path checks remain active. HTTP behavior is
explicit:

- GET handles HEAD, with Axum suppressing the response body.
- OPTIONS is not synthesized; unsupported methods return Axum's 405 and
  `Allow` header, including HEAD for GET routes.
- Static routes take precedence over parameter routes.
- `/users` and `/users/` remain distinct. Non-root trailing-slash declarations
  are invalid, and v0.3 does not redirect or normalize.
- Missing paths use Axum's 404 response.

## Application-facing HTTP API

The prelude exports `Path`, `Query`, `Json`, typed `Header`, `Request`,
`HttpError`, `HttpResult`, `Created`, `NoContent`, `build_router`, and `serve`.
Extractor semantics and rejections remain Axum/axum-extra semantics. Native
extractors, `Router`, `IntoResponse`, middleware, and Tower composition remain
available through the explicit `mads::common::axum` escape hatch.

`HttpResult<T>` is a handler-delivery result whose error is `HttpError`.
`mads::core::Result` remains the explicit result type for framework/bootstrap
operations. `Created<T>` produces 201, `NoContent` produces empty 204, and
`HttpError` produces stable JSON errors for 400, 404, 409, and 500.

## Testing

Use `build_router` with `tower::ServiceExt::oneshot` for in-process requests.
This exercises generated adapters and routing policy without binding a socket.
The runtime test suite covers CRUD verbs, all exported extractors, native Axum
extractors, response wrappers, typed same-name trait calls, static precedence,
HEAD/OPTIONS/405/Allow, strict trailing slashes, 404, conditional routes, and
server validation before lifecycle or bind.

## Deliberately deferred

v0.3 does not add persistence, Diesel, database configuration, automatic input
validation, application/domain error normalization, custom error registries,
MADS middleware abstractions, generated OPTIONS handlers, trailing-slash
redirects, request scopes, or auto-binding configuration. These are not part
of the current runtime contract.
