# Contributing to MADS.rs

Thank you for contributing to MADS.rs. Contributions are made through a local
clone and submitted as pull requests to `develop`. Do not work directly on
`main`, `beta`, or `develop`. The `beta` and `main` branches are managed only by
the maintainers.

## Before you start

Read the project documentation before changing code. This is important because
MADS.rs has deliberate crate boundaries, feature relationships, startup rules,
and compatibility requirements that may not be obvious from one source file.

Start with:

- [`README.md`](README.md) for the public API, supported features, and usage;
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for crate boundaries and
  framework design;
- the relevant files under [`docs/`](docs/) for examples, release decisions,
  historical context, and acceptance requirements;

When documentation disagrees with current code or configuration, mention it in
the pull request instead of silently relying on an assumption. Documents under
`docs/superpowers/` are design history or proposals; use
the current source, `README.md`, `docs/ARCHITECTURE.md`, and CI configuration to
confirm present behavior.

## Clone the repository

For a small change, you may clone the repository directly:

```sh
git clone https://github.com/Adriannathan89/mads.git
cd mads
```

If you do not have permission to push branches to the repository, fork it on
GitHub first and clone your fork instead:

```sh
git clone https://github.com/<your-username>/mads.git
cd mads
git remote add upstream https://github.com/Adriannathan89/mads.git
```

For a fork, keep your local base branch current before starting work:

```sh
git fetch upstream
git switch develop
git pull --ff-only upstream develop
```

For a direct clone, use `origin` in place of `upstream`. Contributors must use
`develop` as their base branch; only maintainers promote changes to `beta` and
`main`.

## Create a branch

Create a focused branch from the pull request's target branch:

```sh
git switch -c feature/short-description
```

Use a descriptive prefix such as `feature/`, `fix/`, `docs/`, `test/`, or
`chore/`. Keep each branch and pull request limited to one coherent change.

## Make the change

MADS.rs is a Rust 2024 workspace with a minimum supported Rust version of
1.85. Follow standard `rustfmt` output and these repository conventions:

- use four-space indentation;
- use `snake_case` for modules and functions, `UpperCamelCase` for types and
  traits, and `SCREAMING_SNAKE_CASE` for constants;
- do not introduce unsafe code;
- document public APIs;
- preserve the dependency layering described in `docs/ARCHITECTURE.md`;
- add focused tests in the crate that owns the behavior;
- use `trybuild` fixtures, including matching `.stderr` files, for procedural
  macro acceptance and diagnostic tests;
- update public documentation when an API, configuration rule, feature, or
  architecture boundary changes.

The main workspace responsibilities are:

- `crates/mads-core`: framework-neutral construction, configuration, provider
  graph, lifecycle, diagnostics, and auto-configuration decisions;
- `crates/mads-core-macros`: core procedural macros;
- `crates/mads-common`: HTTP, routes, Passport/JWT, cookies, CORS, Diesel, and
  PostgreSQL integration;
- `crates/mads-common-macros`: shared route-related procedural macros;
- `crates/mads`: stable public facade;
- `crates/mads-cli`: command-line interface;
- `crates/mads-extra`: reserved extension boundary.

## Validate the change

Run the most focused relevant test while developing. Before opening a pull
request, run the same primary gates used by CI from the repository root:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace --all-features --doc
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
```

Also check the minimum supported toolchain when your change may affect
compatibility:

```sh
rustup run 1.85.0 cargo test --locked --workspace --all-features
```

Coverage contributors can run:

```sh
cargo llvm-cov --workspace --all-features \
  --ignore-filename-regex '(^|/)tests/ui/' \
  --fail-under-lines 85
```

PostgreSQL integration tests require a running PostgreSQL instance and the
`MADS_TEST_DATABASE_URL` environment variable. These tests are ignored during
ordinary local runs and executed separately by CI. Never commit `.env` files,
credentials, private keys, tokens, or database URLs.

## Commit and push

Use a concise, imperative Conventional Commit-style subject. Examples:

```text
feat(routes): add typed header extraction
fix(passport): reject duplicate guard cookies
docs: clarify module visibility
chore: update CI configuration
```

Then push your branch:

```sh
git push -u origin feat/short-description
```

## Open a pull request

Open every contribution pull request from your branch into upstream `develop`.
Do not open contribution pull requests into `beta` or `main`; those branches
are reserved for maintainer-managed promotion and releases. The pull request
should:

- explain the problem and the behavior changed;
- describe important design decisions or tradeoffs;
- list the validation commands you ran and their results;
- link related issues;
- call out breaking changes, configuration changes, migrations, or follow-up
  work;
- include documentation updates for public API or architecture changes;
- include screenshots only for user-facing CLI or visual changes.

Address review feedback with additional commits, push them to the same branch,
and keep the discussion in the pull request. Maintainers will merge the pull
request after review and required checks pass.
