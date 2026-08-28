# v0.5.5 Passport, JWT, guards, and cookies

MADS.rs 0.5.5 adds generic configuration string arrays, strict cookies, an
application-scoped JWT service, managed Passport strategies, typed principals,
and guarded MADS/native Axum routes. Authentication remains opt-in and preserves
the framework-neutral `mads-core` boundary.

## Feature and configuration boundary

Select only what the application needs:

```toml
[dependencies]
# Existing HTTP + PostgreSQL defaults, plus Passport and cookie guards.
mads = { version = "0.5.5", features = ["jwt", "cookies"] }

# HTTP/Passport without PostgreSQL.
mads-http = { package = "mads", version = "0.5.5", default-features = false,
  features = ["http", "jwt", "cookies", "runtime-tokio"] }
```

`jwt` alone has no Axum or Diesel normal dependency. `cookies` includes HTTP.
Bearer Passport requires `http + jwt`; cookie sources additionally require
`cookies`. The compatibility `common` feature still means HTTP + database and
does not silently activate JWT or cookies.

Loading remains explicit and deterministic:

```rust,ignore
let config = ConfigBuilder::new()
    .dotenv(DotenvSource::optional(".env"))
    .source(TomlSource::file("mads.toml"))
    .source(EnvSource::new("MADS_"))
    .build()?;
let application = Mads::builder_with_config(config).build().await?;
```

Dotenv provides interpolation values. Process variables override dotenv during
`${NAME}` interpolation. Ordinary configuration sources merge in declaration
order and the last source wins. A later scalar or string array replaces both
the earlier value and value shape. Arrays are accepted from TOML and explicit
programmatic documents; `EnvSource` is scalar-only.

The minimum Passport configuration is:

```toml
[passport]
secret = "${JWT_SECRET}"
```

This selects HS256 only and requires at least 32 secret bytes. HS384 and HS512
require 48 and 64 bytes and must be the sole configured algorithm in simple
secret mode. MADS also supports RS256/384/512 and ES256/384 through named key
rings:

```toml
[passport]
active_key = "2026-08"
algorithms = ["RS256"]
issuer = "https://auth.example.com"
audiences = ["mads-api"]
clock_skew_seconds = 30
max_token_bytes = 8192

[passport.keys."2026-08"]
algorithm = "RS256"
private_key_file = "keys/current-private.pem"
public_key_file = "keys/current-public.pem"

[passport.keys."2026-07"]
algorithm = "RS256"
public_key_file = "keys/previous-public.pem"
```

One active key signs; active and retained keys verify by `kid`. Every key binds
to exactly one algorithm in the root allowlist. Algorithms are never inferred
from an untrusted JWT header. Inline or file-backed key material is supported,
but supplying both forms for the same field is invalid. TOML-relative paths
resolve beside the winning TOML source; other relative paths use the process
working directory.

## Tokens and strategies

`JwtService::sign`/`verify` use explicit access or refresh options. MADS issues
different `typ` headers and `token_use` claims and rejects cross-kind use.
`decode_header` and `decode_unverified` are inspection APIs whose results are
untrusted.

A strategy is eligible in v0.5.5 only when it:

1. implements `PassportStrategy`;
2. has `#[passport_strategy(name = "...")]` metadata; and
3. has a concrete type registered as a managed provider in the complete static
   provider catalog.

The framework verifies cryptography, registered claims, and token kind before
calling application `validate`. The strategy receives verified typed claims
and a read-only, credential-sanitized `PassportContext`, then returns a current
application principal such as `UserPrincipal`. `UserClaims` are signed input;
the principal is the current identity analogous to Passport's `request.user`.

`jwt` is the built-in access strategy and yields `ClaimsPrincipal<C>`. One
eligible custom `jwt` overrides it; duplicate custom names are ambiguous, and
differently named strategies coexist. `jwt-refresh` is not built in. An
application may define it with `JwtTokenKind::Refresh` and enforce its own
session/refresh records.

## Guard policy and mappings

`#[guard]` is legal only directly beneath a MADS `#[routes]` trait or a route
verb method. It is not a controller, free-function, or native-handler attribute.
A route-trait policy is inherited. Method fields replace only the corresponding
inherited fields; unrelated strategy, principal, source, role, permission, or
predicate clauses remain. `#[guard(skip)]` is the sole complete opt-out and is
valid only on a method with an inherited guard.

Bearer is the default. `source = cookie("literal-name")` requires `cookies`.
One guard extracts one source only and never falls back. Roles, permissions,
and predicates are separate AND clauses. `any` requires one role/permission
match; `all` requires every match. Every predicate is compile-checked as a
synchronous `fn(&Principal) -> bool`; multiple predicates are ANDed. I/O-based
identity/authorization work belongs in the managed strategy or another service.

| Condition | HTTP result |
| --- | --- |
| Missing, malformed, invalid, expired, or wrong-kind token | generic `401` + `WWW-Authenticate: Bearer` |
| Strategy rejects the identity | generic `401` + `WWW-Authenticate: Bearer` |
| Role, permission, or predicate rejects | `403 Forbidden` |
| Framework/strategy operational failure | `500 Internal Server Error` |
| Ordinary malformed `CookieJar` extraction | `400 Bad Request` |
| Missing, malformed, or duplicate target guard cookie | generic `401` |

Successful guards install exact `Authenticated<P>` and `VerifiedToken<C>`
request extensions. Unguarded extraction fails safely rather than panicking.

Native Axum routes can apply `PassportGuard<P>` as a Tower layer. This escape
hatch shares the runtime policy pipeline but is absent from the static MADS
route catalog, so it cannot activate JWT auto-configuration. A managed provider
must directly depend on `JwtService`, or the builder must explicitly provide a
concrete service before application build. Otherwise native guard construction
fails with `MADS131`.

## Security ownership and release boundary

MADS redacts secrets, keys, tokens, registered/custom claim values, principals,
cookie names/values, and resolved configuration values from diagnostics,
reports, debug output, and HTTP failures. Algorithm names, key IDs,
configuration keys/source labels, strategy names, and source locations are safe
structural evidence.

Cookie transport does not provide CSRF protection. Applications must choose
appropriate `HttpOnly`, `Secure`, and `SameSite` attributes and install their
own CSRF policy when cross-site state changes are possible.

MADS.rs 0.5.5 does **not** implement login/credential validation, a refresh
endpoint, refresh-token persistence, rotation, reuse detection, revocation,
password hashing, CSRF, CORS, HTTP auto-binding, remote JWKS, JWE, or module
scoping. v0.5.6 owns CORS and HTTP auto-binding.

> **Roadmap supersession (v0.6.0):** The planned v0.5.6 CORS and HTTP
> auto-binding milestone was merged into v0.6.0 so it could share the root
> module scope with provider, route, guard, and strategy discovery.

In v0.6.0, root-module reachability and provider export eligibility will filter
the stable strategy descriptors before name resolution. That is additive
scoping over the v0.5.5 descriptor model; no reachability/export enforcement
ships here.
