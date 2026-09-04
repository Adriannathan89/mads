# v0.7.0 Stable Promotion

Stable promotion is a release verification step for the unchanged
`0.7.0-beta.1` feature set. It is not a feature-development milestone.

Promotion is blocked until:

- all beta-blocking defects are fixed;
- documentation corrections are merged;
- the unchanged command matrix passes;
- MSRV, feature, platform, PostgreSQL, coverage, and package gates pass;
- release notes contain no feature absent from beta.1.

During promotion, do not add new commands, output modes, validators, HTTP
errors, configuration APIs, or schema operations. New capability moves to
v0.8 or a later milestone. Allowed changes are bug fixes, focused regressions,
documentation corrections, and release verification.

The promotion evidence must include complete Linux verification plus the
macOS/Windows `cli-platform` smoke matrix, the PostgreSQL 16 service suites,
Rust 1.85 checks, the workspace line coverage threshold of 85%, and clean
`cargo package --locked --workspace` archives. Publishing, tagging, and
creating a GitHub release remain explicit user-controlled actions after these
checks.
