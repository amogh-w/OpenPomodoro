# OpenPomodoro

An open source, self-hosted Pomodoro and focus timer that records *what* you worked on, not just *when*.

- Rust backend, installable PWA frontend (Svelte 5 + Vite).
- Self-hosted and multi-user: one SQLite file on your server, accounts for the people you invite, full export, no telemetry.
- Works offline once installed. Alerts need the app open (see `PLAN.md`, section 2).

**Status:** version 0.1. It works end to end: accounts, the timer with notes and moods, history, export and restore, offline use, alerts. See [CHANGELOG.md](CHANGELOG.md) and the plan in [PLAN.md](PLAN.md).

## Run it

Requirements: Rust (stable) and Node.js.

```sh
scripts/build-release.sh                              # one program, with the web app inside it
OP_PUBLIC_URL=http://localhost:3000 ./target/release/op-server
```

The first visit asks you to create the administrator account. After that the admin can allow sign-up or add people
(Settings, then Manage users). Data lives in one SQLite file (`data/openpomodoro.db` by default). For Docker, a
reverse proxy with HTTPS, every setting, backups and upgrading, see [docs/self-hosting.md](docs/self-hosting.md).

While developing, run `cargo run -p op-server` and `npm --prefix web run dev` side by side and open
http://localhost:5173; Vite proxies `/api` (and the live-update WebSocket) to the server. Without a built web app
the server reads `web/dist`, or any folder named by `OP_WEB_DIR`. After changing markup, run
`npm --prefix web run css` so new styles are generated.

## Check

```sh
scripts/check.sh   # rustfmt, clippy, tests, svelte-check, web build
```

## Docs

- [PLAN.md](PLAN.md): features, architecture, build order.
- [docs/self-hosting.md](docs/self-hosting.md): running your own server.
- [docs/links-and-shortcuts.md](docs/links-and-shortcuts.md): links that control the timer, and keyboard shortcuts.
- [CONTRIBUTING.md](CONTRIBUTING.md): how to contribute.

## License

[MIT](LICENSE)
