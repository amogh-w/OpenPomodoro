#!/usr/bin/env bash
# Quality gate: Rust fmt, clippy, tests, then web type check and build.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm --prefix web run check
npm --prefix web test
npm --prefix web run build
echo "all checks passed"
