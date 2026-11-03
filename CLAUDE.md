# OpenPomodoro

An open source, **self-hosted, multi-user** Pomodoro and focus-tracking app. **Rust backend + installable PWA frontend.** You run the server (one binary or Docker); the people you invite get accounts and private data. No hosted service, no telemetry.

The feature list and build order are in `PLAN.md`. Read it before starting work, and update its status table as steps finish.

## Rules
- Never run `git commit` (or anything that creates a commit) unless the user explicitly asks for it in that turn. Staging, diffing and status are fine.
- Original work only. Do not copy code, assets, names, sounds, icons, copy text or layouts from other products. Ideas and behaviors are fair game; expression is not.
- Do not name other products or repositories in code, docs, commit text or UI. Describe behavior generically.
- `ref/` is a local, git-excluded folder of private research notes and screenshots. Read it for background if it exists; never reference it from code or docs, never commit it, never add it to `.gitignore` (it is hidden through `.git/info/exclude`). The project must build and make sense without it.

## Stack (decided in PLAN.md section 2; change only with a note there)
- Backend: Rust, `axum` HTTP + WebSocket, SQLite through `rusqlite`, `tokio`, argon2id for passwords. Serves the built PWA as static files and a JSON API.
- Frontend: a **browser PWA first** (installable, offline), built with **Svelte 5 + Vite (static bundle, no SSR, no meta-framework)** styled by the **cuppa-ui kit** (Tailwind v4 + Catppuccin tokens, Lucide sprite, `theme.js` + `ui.js`). This is a deliberate, user-approved exception to the kit's "no UI framework" rule; every other kit rule still applies: no CDN, no webfonts, native `<dialog>`/`popover`, system font stack, kit classes first. Timer and state logic stays framework-free (plain JS/Rust) and Svelte only renders it. Load the `cuppa:cuppa-ui` skill before writing any UI, follow its rules and checklist, and run the `cuppa:harness` lint/checks when they apply. Never hand-write a second theme switcher; use the kit's `data-flavor`, `data-accent`, `data-radius` hooks. Storage keys: kit owns `ui-*`; app data uses the `op_` prefix.
- Shared logic: the timer and state machine live in a pure Rust crate (`core`) with an injectable clock. The PWA uses the same rules through a thin JS port checked against shared JSON test vectors (or a WASM build later); the Rust tests are the source of truth.
- Platforms: **browser PWA now**. Later: installed desktop (tray, global shortcuts) and phone/tablet. Keep every platform-specific feature behind a small `platform` module with feature detection, so new devices add adapters instead of forking the UI. The UI is responsive from the start (phone-first layout, capped width on desktop).

## Layout (target)
```
crates/core/      pure timer, state machine, ids, event log (no I/O)
crates/server/    axum app, SQLite, API, WebSocket, hooks
web/              PWA (Svelte 5 + Vite): ui/ (cuppa kit copy), src/ (components, stores, lib), public/ (icons.svg, sw.js, manifest), dist/ (build output served by the server)
web/src/platform/ per-device adapters (notifications, wake lock, badge, shortcuts); browser first
extension/        browser extension for site blocking (later phase)
docs/             user and developer docs
PLAN.md           feature list and phases
```

## Design principles
1. **Time is stored as timestamps**, never as a counter. Remaining time is computed from `start`, `pause_buffer` and `now`, so reloads, sleep and second devices stay correct. The UI tick is only for repainting.
2. **State changes are events** in an append-only log, projected into normal tables in one transaction. This gives history, undo, export and recovery.
3. **Ids sort as strings**: `<UTC yyyyMMddHHmmss>.<5-digit seq>@<device uuid>`. They are strictly increasing per device; across devices they order by wall clock only, so they do not prove causality (rules in PLAN.md section 2).
4. **The state machine is a table**: unknown `(state, event)` pairs are rejected and logged, never ignored silently.
5. **Self-hosted, private by default**: one SQLite file on your own server, accounts for the people you invite, every row owned by one user, full per-user export (JSON, CSV, ICS), no telemetry.
6. **Gentle by default, strict by choice**: nudges are soft; strict mode and blocking are opt-in.
7. **No hard-coded UI strings** (i18n keys from day one) and **no hard-coded colors** (theme tokens).
8. **State is visible in color**: the whole surface changes for focus, overflow, break.
9. **The server is built as an internet-facing service**: argon2id passwords, hashed cookie sessions, `Origin` and `Host` checks against the configured public URL, per-user scoping of every query, rate-limited sign-in. It binds `127.0.0.1` unless `OP_BIND` says otherwise. Details in PLAN.md section 2 (Security).

## Conventions
- Rust: `cargo fmt`, `cargo clippy -- -D warnings`, tests next to the code, no `unwrap()` outside tests. Timer and state logic must be unit-tested with a fake clock, including sleep/clock-jump cases.
- Web: Svelte 5 (runes) with TypeScript in `.svelte` and `.ts` files, `svelte-check` clean (no `any`), logic in plain `.ts` modules and stores, components kept thin, kit classes first and Tailwind utilities for layout only, accessible controls (44px targets, labels, keyboard), works offline once installed, looks right in both Latte and Mocha.
- Every feature id in `PLAN.md` (for example `T-03`) is referenced in its PR/commit message and tests when relevant.
- Small steps: finish one plan step, update the status table, report, then continue.
- Keep docs short and factual. Mark guesses as such.

## Commands (fill in as they come to exist)
```
cargo test --workspace
cargo run -p op-server         # serves web/dist on 127.0.0.1:3000; env: OP_BIND, OP_PORT, OP_PUBLIC_URL, OP_DB, OP_WEB_DIR, OP_TRUST_PROXY
scripts/check.sh               # fmt, clippy, tests, svelte-check, web tests, web build
npm --prefix web run dev       # Vite dev server (proxies /api to the Rust server)
npm --prefix web run build     # icons sprite + Tailwind css + Vite build into web/dist
npm --prefix web run check     # svelte-check + tsc
# decide in PLAN.md whether web/dist is committed or built in CI
```
