# OpenPomodoro

An open source, self-hosted Pomodoro and focus timer that records *what* you worked on, not just *when*.

- Rust backend, installable PWA frontend (Svelte 5 + Vite).
- Self-hosted and multi-user: one SQLite file on your server, accounts for the people you invite, full export, no telemetry.
- Works offline once installed. Alerts need the app open (see `PLAN.md`, section 2).

**Status:** early development. Nothing is usable yet; see the build order in [PLAN.md](PLAN.md).

## Run it

Requirements: Rust (stable) and Node.js.

```sh
npm --prefix web install
npm --prefix web run build
cargo run -p op-server        # http://localhost:3000
```

The first visit asks you to create the administrator account. After that the admin can allow sign-up or add people (Settings, then Manage users). Data lives in one SQLite file (`data/openpomodoro.db` by default).

The server reads these environment variables:

| Variable | Default | Meaning |
|---|---|---|
| `OP_BIND` | `127.0.0.1` | Address to listen on. Use `0.0.0.0` to accept connections from other machines (needs `OP_PUBLIC_URL`). |
| `OP_PORT` | `3000` | Port. |
| `OP_PUBLIC_URL` | none | The address people type, for example `https://pomodoro.example.com`. Required when `OP_BIND` is not loopback. Put a TLS reverse proxy in front for https. |
| `OP_DB` | `data/openpomodoro.db` | SQLite file. |
| `OP_WEB_DIR` | `web/dist` | Folder with the built web app. |
| `OP_TRUST_PROXY` | off | Set to `true` behind a reverse proxy so sign-in rate limits use the real client address. |

For UI work, run `cargo run -p op-server` and `npm --prefix web run dev` side by side and open http://localhost:5173; Vite proxies `/api` (and the live-update WebSocket) to the server. After changing markup, run `npm --prefix web run css` so new styles are generated.

## Check

```sh
scripts/check.sh   # rustfmt, clippy, tests, svelte-check, web build
```

## Docs

- [PLAN.md](PLAN.md): features, architecture, build order.
- [CONTRIBUTING.md](CONTRIBUTING.md): how to contribute.

## License

[MIT](LICENSE)
