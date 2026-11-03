#!/usr/bin/env bash
# Build the release program: the web app first, then the server with the web app inside it.
# Result: target/release/op-server, one file that is the whole server (copy it anywhere and run it).
set -euo pipefail
cd "$(dirname "$0")/.."

npm --prefix web ci
npm --prefix web run build
cargo build --release -p op-server --locked

ls -lh target/release/op-server
echo "built target/release/op-server (the web app is inside it)"
