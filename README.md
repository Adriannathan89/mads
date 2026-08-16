# MADS.rs

MADS.rs is a layered Rust workspace for a clean-architecture application framework.
This initial foundation establishes crate boundaries and shared build policy; it does
not yet provide HTTP, database, dependency-graph, or runtime APIs.

## Crates

- `mads-core`: framework-neutral runtime boundary and core procedural-macro re-exports.
- `mads-core-macros`: implementation boundary for future core procedural macros.
- `mads-common`: reserved standard backend integration boundary.
- `mads-common-macros`: reserved HTTP macro boundary.
- `mads-extra`: reserved post-v1 capability boundary.
- `mads`: public facade that composes supported features.
- `mads-cli`: developer command-line entry point, installed as `mads`.

`mads` enables the `common` and Tokio runtime features by default. Consumers can
disable default features when they only need the core boundary.

## Development

Use the configured stable Rust toolchain and run:

```bash
cargo check --workspace --all-features
```
