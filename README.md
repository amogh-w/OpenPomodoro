# OpenPomodoro

An open source, local-first Pomodoro and focus timer that records *what* you worked on, not just *when*.

- Rust backend, installable PWA frontend (Svelte 5 + Vite).
- One SQLite file, full export, no account, no telemetry.
- Works offline once installed. Alerts need the app open (see `PLAN.md`, section 2).

**Status:** early development. Nothing is usable yet; see the build order in [PLAN.md](PLAN.md).

## Run it

Requirements: Rust (stable) and Node.js.

```sh
npm --prefix web install
npm --prefix web run build
cargo run -p op-server        # http://127.0.0.1:3000
```

The server listens on `127.0.0.1` only. `OP_PORT` and `OP_WEB_DIR` override the port and the folder it serves (default `web/dist`).

For UI work, run `cargo run -p op-server` and `npm --prefix web run dev` side by side; Vite proxies `/api` to the server.

## Check

```sh
scripts/check.sh   # rustfmt, clippy, tests, svelte-check, web build
```

## Docs

- [PLAN.md](PLAN.md): features, architecture, build order.
- [CONTRIBUTING.md](CONTRIBUTING.md): how to contribute.

## License

[MIT](LICENSE)
