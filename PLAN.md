# OpenPomodoro: feature list and build plan

Status legend: `[ ]` todo, `[~]` in progress, `[x]` done. Phase tags: **P1** = v1 (self-hosted multi-user server in Rust + PWA), **P2** = v1.x, **P3** = later / v2, **Dev** = needs a per-device app or extension, parked until the PWA is finished.

## 1. Goals
- A calm, fast, free focus timer that records *what* you worked on, not just *when*.
- **Self-hosted and multi-user**: you run the server (one binary or Docker, one SQLite file); the people you invite each get an account and their own private data. No cloud service, no telemetry.
- Installable PWA that works offline; Rust backend for accounts, storage, API, hooks.
- Several devices per person stay in step through the server.
- Beautiful and themeable; accessible; translatable.

Non-goals for v1: a hosted service run by the project, email or SMS, OAuth/SSO, teams or shared data between users, native mobile apps, native app/window blocking, alerts while the app is closed, a serverless (static-host only) mode.

## 2. Architecture
- `crates/core`: pure Rust. Timer maths, state machine table, id generator, event types, projections. Injectable `Clock` trait. No I/O.
- `crates/server`: `axum` HTTP + WebSocket, SQLite (WAL), migrations, accounts and sessions, per-user event log, settings, export, hooks runner, static file serving. Listens on a configurable address; TLS comes from a reverse proxy.
- `web/`: browser PWA, **Svelte 5 + Vite + TypeScript**, built as a static bundle (no SSR, no meta-framework), styled by the **cuppa-ui kit** (Tailwind v4 build, Catppuccin themes, Lucide sprite, kit `theme.js`/`ui.js`). The framework is a recorded exception to the kit's "no UI framework" rule; all other kit rules hold (no CDN, no webfonts, native dialogs). Timer and state logic live in plain `.ts` modules and a store; components only render. Signs in to the server (sign-up, sign-in, sign-out, admin screens), then talks to it by JSON API and a per-user WebSocket for live state. The service worker caches the app shell only. The server serves `web/dist`.
- `web/src/platform/`: one adapter per capability (notifications, wake lock, badge, audio unlock, shortcuts, install prompt, tab leader) with feature detection. The browser adapter ships first; desktop and mobile adapters come later without UI changes.
- `extension/` (P2): MV3 extension for website blocking, reading state from the server API with a personal token (needs the server reachable).

### Data ownership (P1)
- The server's SQLite file is the only source of truth, and every row belongs to one user: the log, the projection and the exports are all keyed by user id. P1 needs the server reachable; the PWA is served by it.
- Because the server is shared by all of a person's devices, several devices stay in step through it (WebSocket push of the current timer state and changes). That is plain client-server, not peer sync.
- The PWA keeps the live timer state and an **outbox** of not-yet-sent **commands** (timer actions) and **data events** (settings, categories, session edits) in IndexedDB, keyed by user id. If the server is unreachable, the timer keeps running from timestamps (using the TS port) and new work queues in the outbox. On reconnect the outbox is sent in order, each item once; the server ignores a command or event id it has already processed, then the PWA reloads state from the server. A command the server now rejects (the state moved on elsewhere) is dropped and the user is told. Signing out clears the outbox only after it has been sent (or the user confirms discarding it).
- App settings live in SQLite as `SETTING` events, per user. Kit appearance keys (`ui-*`, localStorage) are per-browser and are copied into the user's JSON export and backup so nothing is lost.

### Timer rule
- Running: `elapsed = now - start - pause_buffer`. Paused: `elapsed = pause_ts - start - pause_buffer`. On resume: `pause_buffer += now - pause_ts`.
- The phase ends when `elapsed >= total`.
- Extend or shrink by `d` (T-05): before the end, `total += d`; if that makes `total <= elapsed`, set `total = elapsed` (the phase ends now, with no overflow). At or past the end (overflow), a positive `d` sets `total = elapsed + d` (a fresh `d` countdown from now) and a shrink does nothing. `elapsed` is never reset, so recorded durations stay true. Pausing and extending can happen in any order.
- If the clock moves backwards and `elapsed` would be negative, clamp it to 0 and write a diagnostic entry (the core exposes `clock_went_back` so the caller can log it). A resume after the clock moved back adds 0 to `pause_buffer`, never a negative span.

### Alerts and tabs (P1)
- Sounds and notifications fire from an open page (foreground, or a background tab while the browser keeps it alive). Browsers offer no way to schedule a notification for later from a service worker, and they suspend idle workers, so P1 makes no promise for alerts while the app is closed. The UI says so next to the alert settings.
- When the page wakes or becomes visible it recomputes state from timestamps and shows phase changes it missed.
- Alerts while the app is closed come with Web Push (needs a push service, A-07, P3) or the desktop shell (F-04, P3).
- With several tabs open, one **leader tab** (Web Locks, BroadcastChannel fallback) plays sounds and shows notifications; other tabs only render.

### Events and commands
- Entities: `SESSION` (with `kind`: `focus`, `short_break`, `long_break`), `CATEGORY`, `SETTING`, `TIMER` (live timer state); `TODO` is added with C-07 (P2). Operations `ADD_/EDIT_/DELETE_`, payload JSON, applied in one transaction, always scoped to one user.
- Add of an existing id becomes an edit. Every field remembers the id of the event that set it and only a newer event changes it, so events can arrive in any order and still converge (tested over every arrival order). Edit or delete of a missing id is rejected and written to a `rejected_events` table, then replayed when its add arrives. Delete writes a tombstone that wins over later adds and edits; the highest delete id is kept.
- **Timer actions are commands, not events.** A command (`init_focus`, `timer_ends`, `toggle_pause`, `extend`, `stop`, `abandon`, `take_rest`, `done`, `skip_rest`, ...) carries a client-made id (idempotency key), an optional `at` (when the user acted, for offline replay) and arguments. The server runs the shared state machine and timer rules on the user's stored state, then writes the result atomically: one `TIMER` event (the whole timer state, written with a server-made id so the row always moves forward) plus the `SESSION` rows the effects name. Clients can never write `TIMER` events directly. A command id seen before answers `duplicate`; a command the state machine refuses answers `409` (`rejected_unknown`, `rejected_strict`, `rejected_not_ended`) and writes nothing.
- **Data edits are events**, validated field by field on the server (known settings with ranges, known session and category fields, size limits). Event ids from clients must parse and may not be more than a day ahead of the server clock (otherwise one wrong clock would win every later edit).
- Command `at` may be at most a minute ahead of the server clock. A phase that auto-starts the next one starts it where the last one ended, not when the app noticed, so sleeping through a break does not shift the cycle.
- A focus session becomes a `SESSION` record when it leaves `reflect` (`take_rest` or `done`), or when its phase ends with auto-start break on (reflect is skipped; mood and notes can be added later from history). A break becomes a record when it is stopped, skipped or followed by the next focus. Abandon writes nothing. Rounds count kept focus sessions; a saved long break resets the count. C-06 (discard 30 s or less with no intention) is applied in step 1.9.
- Live updates: the server tells a user's open WebSockets `{"type":"changed"}` after any change to their data; clients refetch. Ten sockets per user at most; a socket closes when its session ends.

### Ids
- Format `<UTC yyyyMMddHHmmss>.<5-digit seq>@<device uuid>`; sorts as a string.
- Strictly increasing per device: on start, restore from the max id **this device** issued (ignore other devices' ids, so a device with a fast clock cannot drag others forward). If the clock moves back, keep the last timestamp and increment `seq`; if `seq` passes 99999, advance the timestamp by one second.
- Across devices the order is wall clock only; ids do not prove causality.

### State machine
States: `idle, breathe, session, session_end (focus overflow), paused, reflect, rest, rest_end (break overflow)`. `paused` remembers which of `session, session_end, rest, rest_end` it paused and returns there. "Start focus" below means `breathe` when the breathing step is on, otherwise `session`.

The machine is pure and owns no timer. The caller applies a timer change first and tells the machine the result: `extend` carries `phase_ended` (the timer's answer afterwards), and the caller sends `timer_ends` when the timer says a phase is over (including after a shrink past zero or a resume of a phase that ended while paused). Each transition may name one **effect** to persist.

| Event | From | To | Effect |
|---|---|---|---|
| `init_focus` | idle | start focus | |
| `init_focus` | rest_end | start focus | save break |
| `start_focus` | idle / rest_end | session (no breathing) | save break from rest_end |
| `skip_breathe` | breathe | session | |
| `timer_ends` | breathe | session | |
| `timer_ends` | session | session_end, or rest when auto-start break is on | save focus when it goes to rest |
| `timer_ends` | rest | rest_end, or start focus when auto-start focus is on | save break when it starts focus |
| `toggle_pause` | session, session_end, rest, rest_end | paused | blocked in strict mode |
| `toggle_pause` | paused | the phase it paused | resume is never blocked |
| `extend(+/-)` | session, rest, paused (running phase) | unchanged | |
| `extend(+/-)` | session_end, rest_end, or paused from them | back to session / rest (paused: still paused) when `phase_ended` is false, else unchanged | |
| `stop` | session, session_end, paused from them | reflect | none (saved when leaving reflect) |
| `stop` | rest, rest_end, paused from them | idle | save break; blocked in strict mode while the break is still running |
| `abandon` | breathe | idle | |
| `abandon` | session, session_end, paused from them | idle | nothing saved; blocked in strict mode |
| `take_rest` | reflect | rest | save focus |
| `take_rest` | idle | rest | |
| `done` | reflect | idle | save focus |
| `skip_rest` | rest, rest_end, paused from them | idle, or start focus when auto-start focus is on | save break; blocked in strict mode while the break is still running |

Any other `(state, event)` pair is rejected as `unknown`; a blocked row is rejected as `strict`. Both are logged by the caller. `test-vectors/machine.json` holds every state and event pair under the default rules and each rule on its own (792 cases); Rust and the TS port pass all of them.

### Security
The server is meant to be reachable by other people, so it is built as an internet-facing service even when it only runs on a home network.
- **Accounts:** username + password. Passwords are hashed with argon2id and never stored or logged in clear text. The first account created on a fresh install is the admin; after that, registration follows the policy (`closed` by default, `open` if the admin turns it on) and the admin can create accounts and set a temporary password. Sign-in attempts are rate limited with backoff per account and per address. Account recovery is by the admin (no email).
- **Sessions:** random session ids stored hashed in the database, sent in an `HttpOnly`, `SameSite=Lax` cookie (`Secure` when the public URL is https). Sign-out and password change revoke sessions. The WebSocket uses the same cookie.
- **CSRF and origin:** every state-changing request must come from the app's own origin (`Origin` checked against the configured public URL) and carry a custom header; GET never changes data.
- **Hosts:** requests whose `Host` is not the configured public host (or a loopback name in development) are refused, against DNS rebinding and host-header tricks.
- **Isolation:** every query is scoped by the signed-in user id. Tests prove user A can never read, change, export or receive pushes of user B's data, including by guessing ids.
- **Network:** binds `127.0.0.1` by default; `OP_BIND` (for example `0.0.0.0` in Docker) opens it. TLS is terminated by a reverse proxy; the docs show one. Security headers (CSP without inline scripts, `X-Content-Type-Options`, `Referrer-Policy`, HSTS when https) are set by the server.
- **Tokens for scripts** (P2): personal API tokens, shown once, stored hashed, sent in a header, revocable; never in URLs.
- **Hooks (I-04, I-05) run executables, so they are configured only by the server admin in the config file**, never from a user's UI. Outgoing webhooks (I-06) refuse private and loopback addresses unless the admin allows them.
- Links that change state (`#/start`, `#/pause`, `#/break`, `#/finish`, protocol handler) show a confirm step unless the user turns on "Allow links to control the timer". Read-only routes navigate directly.

## 3. Feature list (the superset)

### A. Timer and cycle
| Id | Feature | Phase |
|---|---|---|
| T-01 | Configurable focus, short break, long break durations | P1 |
| T-02 | Rounds before a long break; long-break rule | P1 |
| T-03 | Timestamp-based timer, correct after reload, sleep, second tab and second device (leader tab plays alerts) | P1 |
| T-04 | Pause / resume; end early (stop and keep, or abandon) | P1 |
| T-05 | Extend or shrink the running phase by 5 minutes, also during overflow | P1 |
| T-06 | Overflow: keep counting after the end, repeat reminders every N minutes (default 5), max 12 | P1 |
| T-07 | Optional breathing step before a session (skippable) | P1 |
| T-08 | Auto-start next phase (break and/or focus), each toggleable | P1 |
| T-09 | Dial scrubbing with snap (default 5 min); rotate one way for focus, the other for break; keyboard and slider alternative | P1 |
| T-10 | Text shortcuts in the intention field (`30m`, `1h`, `@category`) | P1 |
| T-11 | Ending-soon warning (configurable lead time, default 1 min) before the end of focus and of breaks | P1 |
| T-12 | Special breaks at fixed times (lunch, snack) without changing the cycle | P2 |
| T-13 | Strict mode (no pause, abandon or skip once started), opt-in; also counts as focus protection | P2 |
| T-14 | Presence nudge every N minutes (soft sound) | P2 |
| T-15 | Compact / mini view | P2 |
| T-16 | Picture-in-picture timer window | P3 |

### B. Capture and meaning
| Id | Feature | Phase |
|---|---|---|
| C-01 | Intention text per session with autocomplete from past entries | P1 |
| C-02 | Categories with colors; a neutral category for breaks | P1 |
| C-03 | Notes during and after a session; recent notes shown while writing | P1 |
| C-04 | Reflection mood: focused / neutral / distracted | P1 |
| C-05 | Retroactive session entry (forgot to start) | P1 |
| C-06 | (dropped) Ending a session always saves it and moves the round on; only Abandon throws one away | – |
| C-07 | Task list; link a task to a session; mark done on finish | P2 |
| C-08 | Profiles per intention + category (own durations, sounds, blocklists) | P3 |
| C-09 | Group categories for reports | P3 |

### C. History and insight
| Id | Feature | Phase |
|---|---|---|
| H-01 | Session log by day and month | P1 |
| H-02 | Day timeline: sessions and breaks as blocks, current-time line | P1 |
| H-03 | Daily, weekly, all-time totals and streaks | P1 |
| H-04 | Export everything: JSON, CSV, ICS | P1 |
| H-05 | 52-week heatmap | P2 |
| H-06 | KPI cards with deltas: total focus, average per day, best time of day, focus/break ratio | P2 |
| H-07 | Category table and filters; notes-only view | P2 |
| H-08 | Read-only ICS calendar overlay (JSON backup restore is D-04) | P3 |

### D. Alerts and sound
| Id | Feature | Phase |
|---|---|---|
| A-01 | Notification at every phase change while the app is open (permission flow, leader tab only); closed-app alerts need A-07 or F-04 | P1 |
| A-02 | Distinct sound per phase: focus, overflow, break, break overflow | P1 |
| A-03 | Volume, mute, quiet hours | P1 |
| A-04 | Bring-your-own sounds (stored by the server next to the SQLite file and included in backups; cached in the browser) | P2 |
| A-05 | Optional tick sound for focus and break, independent | P2 |
| A-06 | Spoken announcements (speech synthesis) | P3 |
| A-07 | Web Push: phase-end alerts while the app is closed, daily reminder with weekday toggles (self-hosted VAPID keys) | P3 |
| A-08 | Time left in the page title, favicon and app badge | P1 |

### E. Appearance and language
| Id | Feature | Phase |
|---|---|---|
| U-01 | Light, dark and system theme (kit Auto/Latte/Mocha) | P1 |
| U-02 | Whole-surface color changes per state | P1 |
| U-03 | Theme tokens as CSS variables (from the kit) | P1 |
| U-04 | High-contrast theme, bundled theme set, user JSON themes with live reload; all added to the kit as flavors/tokens under its existing hooks, never a second switcher | P2 |
| U-05 | Reduced-motion / low-power mode (no progress animation) | P1 |
| U-06 | Translations: string catalog and `t()` helper from the first screen (0.2b), English first; more languages in P2 | P1/P2 |
| U-07 | In-page keyboard shortcuts (start, pause, +/-5, focus input, cancel); customizable in P2 | P1/P2 |
| U-08 | Accessibility: keyboard use, labels, contrast, screen reader state announcements; checked in every UI step, audited in 1.12 | P1 |
| U-09 | Keep the screen awake while a phase runs (wake lock), opt-in | P1 |

### F. Automation and integration
| Id | Feature | Phase |
|---|---|---|
| I-01 | Hash routes: `#/start?intent=&duration=&category=&notes=`, `#/pause`, `#/break`, `#/finish`, `#/analytics` ...; `web+openpomodoro:` protocol handler; state-changing links confirm first (section 2, Security) | P1 |
| I-02 | Documented, versioned HTTP API for scripts, authenticated by personal tokens (X-08), built on the P1 internal API | P2 |
| I-03 | Public WebSocket stream of a user's state changes for overlays and scripts (token auth), built on the P1 push channel | P2 |
| I-04 | Lifecycle hooks: start, pause, unpause, end, break start/end, went-too-long; per-hook toggle and a Test button | P2 |
| I-05 | Hook runner for the server (admin-configured only): executable per event with JSON on stdin, timeout, queue | P2 |
| I-06 | Outgoing webhooks | P2 |
| I-07 | `x-success` / `x-error` style callback URLs on routes | P3 |
| I-08 | (superseded: the server is the shared source of truth, so a person's devices sync through it in P1) | – |

### G. Focus protection
| Id | Feature | Phase |
|---|---|---|
| F-01 | Website blocker extension (needs the server reachable, uses a personal token): deny or allow list, separate lists for focus and break, active tab only, snooze 3 min, restore all, kind block page showing the intention | Dev |
| F-02 | Escape hatch: blocking off when the intention matches a chosen keyword | Dev |
| F-03 | (merged into T-13) | – |
| F-04 | Desktop shell (tray with progress, global shortcuts, always on top, close to tray, auto-update, alerts while the window is closed) | Dev |
| F-05 | App blocking in the desktop shell (hide, never kill) | Dev |
| F-06 | Full-screen break overlay in the desktop shell | Dev |
| F-07 | Window tracker that suggests a session | Dev |

### H. Data and platform
| Id | Feature | Phase |
|---|---|---|
| D-01 | SQLite storage with migrations, WAL, one file | P1 |
| D-02 | Event log with string-sortable ids | P1 |
| D-03 | Installable PWA, offline shell, outbox while the server is unreachable, update prompt | P1 |
| D-04 | Per-user backup / restore (JSON), delete my account and data; admin backup of the whole database file | P1 |
| D-05 | Diagnostic log with a copy button | P2 |
| D-06 | Docker image and single static binary release, reverse-proxy TLS guide, configuration by environment variables | P1 |

### I. Accounts and hosting
| Id | Feature | Phase |
|---|---|---|
| X-01 | Accounts: username + password sign-up, sign-in, sign-out; argon2id hashes; cookie sessions | P1 |
| X-02 | First-run setup: the first account becomes admin; registration policy `closed` (default) or `open` | P1 |
| X-03 | Admin: create and disable users, set a temporary password, list users (never their data) | P1 |
| X-04 | Per-user data isolation in storage, API, WebSocket push and export | P1 |
| X-05 | Sign-in rate limiting with backoff; session revocation on sign-out and password change | P1 |
| X-06 | Change password; delete my account and all its data | P1 |
| X-07 | Server configuration by environment variables: bind address, public URL, database path, registration policy | P1 |
| X-08 | Personal API tokens (shown once, stored hashed, revocable) | P2 |
| X-09 | Session list with revoke per device | P2 |
| X-10 | TOTP two-factor sign-in; optional OIDC sign-in | P3 |

## 4. Build order

### Phase 0: scaffold
- [~] 0.1 Cargo workspace (`core`, `server`); `web/` created as Svelte 5 + Vite + TS and initialised with the cuppa-ui kit (`init` script, Tailwind + icon sprite scripts, `theme.js`, `ui.js`); check script running fmt, clippy, tests, `svelte-check` and build (`scripts/check.sh`). Done except the kit lint: it needs this project registered in the cuppa plugin's `harness.json`, which lives outside this repo.
- [x] 0.2 `server` serves `web/dist` and `/api/health`; binds `127.0.0.1`; `Host`/`Origin` checks in place from the start.
- [x] 0.2b PWA skeleton: manifest, icons (original artwork), service worker (shell cache only), install prompt, app shell with tab bar, hash router, theme/flavor/accent settings wired to the kit hooks, i18n catalog and `t()` helper (no literal UI strings from here on). Offline after first load.
- [x] 0.3 Docs skeleton: README, CONTRIBUTING, LICENSE. License: **MIT**.

### Phase 1 (P1)
- [x] 1.1 **Timer core** (T-01..T-05): pure types, `Clock` trait, timer rule from section 2 (extend/shrink, overflow extend, backwards-clock clamp); tests for pause/resume/extend, clock jumps, sleep. Shared JSON vectors in `test-vectors/timer.json` (17 cases) pass in Rust and in the TS port `web/src/lib/timer.ts`. Config bounds (1 min to 12 h per phase, at most 12 rounds) are guesses.
- [x] 1.2 **State machine** (section 2 table): table-driven, reject unknown pairs, tests for every row, including breathe, overflow, rest, reflect, auto-start and strict-mode blocks. Shared JSON vectors for every row; TS port in `web/src/lib/machine.ts` checked against them.
- [x] 1.3 **Ids and event log** (D-02): per-device monotonic generator (backward clock, `seq` overflow, restore from this device's max id) with a TS port (`web/src/lib/ids.ts`) checked against `test-vectors/ids.json`; pure `decide` function and in-memory `Store` for events and projections (idempotent add, add-existing becomes edit, newest event wins per row, rejected-event list with replay when the add arrives, tombstones that win over late adds and edits). The SQLite layer (1.4) reuses `decide`.
- [x] 1.4 **SQLite layer** (D-01): `crates/server/src/db.rs`. Migrations by `PRAGMA user_version` (a newer file is refused), WAL, one transaction per event, duplicate event ids answer `Duplicate` with no writes, waiting events replayed after their add, projection rebuildable from the log. The projection is one `rows` table (entity, key, JSON data, per-field versions, tombstone) with an expression index on `started_at` for history queries, read through typed readers (`sessions_between`, `setting`, `current_timer`) rather than one table per entity. The device id and API token are created on first use and kept in `meta`.
- [x] 1.4b **Multi-user storage** (X-04): every table keyed by user id (log primary key `(user_id, id)`, so one user cannot collide with or probe another's event ids); users table; `Db` methods take a user id. Isolation tests.
- [x] 1.4c **Accounts and sessions** (X-01, X-02, X-03, X-05, X-06): argon2id hashing, hashed session ids, first-run admin, registration policy, create/disable users, change password, delete account, login rate limiting. Pure logic with tests, no HTTP yet.
- [x] 1.5 **API v0** (internal, for the PWA), built as 1.5a config and request guards, 1.5b auth routes, 1.5c commands, events, history and state, 1.5d WebSocket push. Setup/sign-in/sign-out, current user, admin routes, per-user state, timer commands, validated event intake for the outbox, history by date range, per-user WebSocket push. Cookie sessions, `Origin` and `Host` checks against the configured public URL, CSRF header, security headers, rate limiting, per-user isolation tests over HTTP and WebSocket (139 server tests).
- [x] 1.6a **Accounts UI and live state** (X-01..X-03, X-06): first-run setup, sign-in and sign-up screens, sign-out, change password, delete account, admin users screen (create, disable, temporary password, registration switch); API client with CSRF header, server-synced clock, persistent client ids, reconnecting WebSocket, app state store. Checked in a browser against a real server.
- [x] 1.6b **Offline and devices** (D-03): the "action -> next timer state + finished sessions" logic lives in `op-core` (`planner`) with a TS port held to `test-vectors/planner.json` (17 scenarios; Rust writes the expectations with `UPDATE_VECTORS=1`, TS must match), so the browser runs the timer on its own while the server is unreachable and lands on the same result. Last-known state, the signed-in user and an outbox of commands and data events are kept in IndexedDB (memory fallback where it is blocked). Offline, an action is worked out locally, shown at once, and queued with its id and the time the user acted; on reconnect the leader tab (Web Locks) replays the queue in order, the server ignores repeats, and anything it refuses is dropped with a notice. A reload with no server starts from what the device remembers. Signing out with unsent changes asks first. Checked in a browser against a real outage: counting and pause offline, reload offline, a chain of finish / done / start offline replayed in order with the true times. Limits: the first sign-in needs the server; browsers without Web Locks make every tab its own leader (no BroadcastChannel election); offline history and categories are not available until sync.
- [x] 1.7 **Timer screen** (T-04, T-05, T-07, T-09, T-10, U-02, U-09, A-08): intention field with `30m` / `@category` shortcuts, scrubbable dial (one turn is 60 minutes, snaps to 5; keyboard and slider semantics), breathing step with skip, pause / resume, ±5 minutes, finish and abandon, reflect with take-a-break or done, break and overtime views, whole-screen tint per state (focus, overtime, break, break overtime, breathe, reflect), time left in the page title and app badge, wake lock (device preference), Timer section in Settings (lengths, rounds, breathing, auto-start). The phase end is sent by whichever open tab notices first; one-tab-only comes with the leader tab in 1.6b. Not done yet: rotating the dial the other way to set a break length (T-09), favicon countdown (A-08), categories can be picked but not yet created (C-02, step 1.9). Checked in a browser on a real clock: countdown, pause freezing for 3 s, natural end into overtime, +5 out of overtime, shrink past zero, finish, reflect, break, end break, history rows.
- [x] 1.8 **Overflow and alerts** (T-06, T-08, T-11, A-01..A-03): a pure planner (`alertPlan.ts`) decides what is due each tick and the leader tab plays it: a different tone pattern for ending soon, end of focus, end of a break, and overtime reminders (focus and break), made with Web Audio so there are no audio files; system notifications through the service worker (click brings the app forward) after the permission question; overtime reminders every N minutes (default 5, at most 12 per phase); the ending-soon tone (default 1 min, skipped for phases shorter than twice the lead); volume, sound on/off and quiet hours (may cross midnight), all per device. Nothing sounds late: if the app was asleep through the end, the sound is skipped and a notice says how long ago the phase ended; if time jumps over several reminder intervals, one reminder plays. Settings has an Alerts card with previews of every sound and says plainly that a closed app cannot alert. Auto-start of breaks and focus (T-08) is in the Timer settings from 1.7. Checked live with the sound and notification calls recorded: end tone and "Focus complete" at 60.0 s, reminder at exactly +1 min and +3 min, silence inside quiet hours.
- [x] 1.9 **Capture** (C-01..C-06): notes and mood travel as arguments of the commands that save a focus session (`take_rest`, `done`, `timer_ends` into an auto break), so they are part of the shared planner and its vectors (25 scenarios now). Notes are written in a pane during the session (kept as a draft across reloads) and on the reflect screen, next to a mood chooser; the pane shows recent notes, those with the same intention first. Intention field autocompletes from past intentions (`<datalist>`) and clears once a session starts. Categories: add, rename, recolor, delete in Settings, picked on the timer or with `@name`; breaks have none (neutral). Accidental starts (30 s or less, no intention, no notes) are discarded and do not count as a round; a note or an intention rescues a short session (a deliberate change from the plan, so typed text is never lost silently). Retroactive entry: History has "Add a session" (start time, length, intention, category, notes, mood; refuses a session ending in the future). Checked live: shortcuts became chips and were stripped from the saved intention, notes and mood saved, the accidental start left no row and no round, autocomplete and recent notes appeared, a forgotten session was added and listed.
- [x] 1.10 **History** (H-01..H-04, C-04 edit): History screen with Today / This week / All time / Streak cards (current and longest; an empty today does not break yesterday's run), a month navigator, expandable days with a 24-hour strip (focus in the category colour, breaks green, a "now" line today) and the day's sessions; tap a session to edit its intention, category, notes and mood or delete it (two steps). Totals and streaks come from `GET /api/summary` (per-day sums over everything, in the browser's UTC offset; the current offset is applied to all days, so a session within an hour of midnight can land on the neighbouring day across a daylight-saving change). Export (`GET /api/export?format=json|csv|ics`, own data only): JSON is the full backup (sessions, categories, effective settings) and the browser adds its look (`ui-*`) before saving; CSV neutralises leading `= + - @`, quotes as needed; ICS escapes and folds at 75 octets without splitting characters. Checked live with 18 seeded sessions: all figures matched the seed, edit and delete updated every total, the previous month listed exactly the right days, exports reflected the edits.
- [x] 1.11 **Routes and shortcuts** (I-01, U-07): `#/start?intent=&duration=&category=&notes=`, `#/pause`, `#/resume`, `#/break`, `#/finish`, `#/analytics`, and the same with `web+openpomodoro:` (manifest `protocol_handlers` to `/?link=%s`; an installed app is offered as the handler). Parsing is pure and strict (unknown links and out-of-range values are ignored). A link that changes the timer is removed from the address bar at once so a reload cannot repeat it, then held until the person is signed in and the app has its state; it then asks (Allow / Cancel, with "Always allow") unless "Let links control the timer" is on (per device). A link that cannot apply in the current state does nothing and says so. Keyboard on the timer screen: Space or P pause/resume, S start, + and - five minutes, F finish, B break, / the intention field, Esc leaves a field; off while typing, with a dialog open, or with Ctrl/Cmd/Alt; Space on a focused button is left to that button. Documented in `docs/links-and-shortcuts.md`; customising keys stays P2. Checked live: the confirmation text, cancel starting nothing, allow and always-allow, every link action and refusal, the protocol-style visit, each key including the ones that must be ignored.
- [x] 1.12 **Polish** (U-05, U-06, U-08, D-04): **Backup and restore:** `POST /api/me/import` restores the JSON export into an account (each category, session and setting becomes an ordinary validated event keyed by its original id, so restoring twice changes nothing; bad items are skipped and counted; unknown fields are dropped; the browser's look comes back through a known-keys-and-values filter); `POST /api/me/wipe` deletes all of a user's data but keeps the account (password required); `GET /api/admin/backup` gives the admin a consistent copy of the whole database file (`VACUUM INTO`). A restore may be 32 MiB while every other route stays at 1 MiB, and an oversized body now answers 413 `payload_too_large`. The Settings "Your data" card has download, restore (file) and delete-everything. **U-05:** a per-device "Reduce motion and save power" switch (starting from the system setting) stops all animation and refreshes the clock once a second. **U-06:** `scripts/check-strings.mjs` (in `scripts/check.sh`) fails on text typed into a template, on hard-coded `aria-label`/`placeholder`/`title`/`alt`, and on unused catalog keys; its first run found five dead keys and one missing feature (the "ready to work offline" message is now shown). **U-08:** axe-core audit of every screen and state (setup, sign-in, timer idle / running / reflect / break, history, settings, admin) in all four themes now reports nothing. It found and we fixed: secondary text one step too faint in Latte (now `subtext1`); light text on a mid-tone accent can never reach 4.5:1, so in Latte accent-filled surfaces are a soft tint of the accent with the theme's dark text; the red and plain text buttons; and the tab bar, now a real tab list (roving focus, arrow keys, Home/End, a labelled tab panel). Screen readers are told when the timer starts, pauses, resumes, runs out, completes and a break starts or ends (not every second). Checked live: restore into an emptied account (look applied, junk ignored, twice is the same, a non-backup file refused), wipe with a wrong then right password, reduce motion with zero animating elements, tab keys, every announcement. The kit's Latte `subtext0` and accent-with-light-text pairs fail AA on their own; worth fixing in the kit itself.
- [x] 1.13 **Self-hosting** (D-06, X-07): the web app is embedded in the server binary (`rust-embed`, served by a tested `assets::serve` with SPA fallback, 404 for missing files, immutable cache only for hashed `/assets/*`); `OP_WEB_DIR` overrides it for development; `op-server healthcheck` for container checks; `scripts/build-release.sh` builds one 5.8 MB file that runs from an empty folder. `Dockerfile` (node, rust, slim non-root runtime with a `/data` volume), `.dockerignore` and `docker-compose.yml` are written, but **the image has not been built or run**: no Docker daemon was available. `docs/self-hosting.md` covers settings, Caddy and nginx (with WebSocket upgrade), accounts, backups, upgrades and troubleshooting. Version 0.1.0 and `CHANGELOG.md`. Release **v0.1**.
- [x] 1.14 **Wide screens and desk features** (U-01, T-12, H-05..H-07 in part): from 1024px the tab bar becomes a sidebar and screens use columns (settings two columns, history overview beside the day log; the timer has a context pane from 1280px: notes with earlier notes while focusing, otherwise a week strip with the chosen day's total, mood tally, timeline and sessions). Timer screen: from-to time chip, round dots, category chip, "Edit session" while running (new `edit_session` command in the planner, both ports, vectors and server test), a sound toggle, "another session" and "take a break" buttons in overtime, a dial that sets a break when turned back past zero (`take_rest` takes `duration_ms`), a darker break-over colour, and "What's next?" on break. History: month overview (totals with change against last month, average day, best and most distracted hour, focus-to-break ratio, mood tally, daily bars with an average line, time per category), a notes-only toggle and a category filter. Settings (per device): ask-for-reflection switch, dial snap step, focus the intention after picking a category, a daily reminder (days and time, while the app is open). **Full sessions first:** ending a focus session before its time asks for confirmation (the default button is "Keep going"; abandoning asks too), the log marks sessions as completed or ended early, the month overview shows the completion rate, and only a session that ran its full planned length moves the round cycle on (an early one is still saved and counted in the totals; shared vectors cover it). **Trying the flow fast:** phases may now be as short as 5 seconds (core minimum lowered; a session that ran its whole planned time is never discarded as accidental), and the per-device "Dial snaps to" setting now has 5 s (the dial counts seconds, a turn is a minute), 1, 5, 10 and 15 min. Running screens follow one pattern: -5 and +5 beside the from-to chip, then three round buttons (sound, the main action, stop); stop opens a small menu to finish or abandon. **Layout:** the sidebar became a slim icon rail and the timer screen is two panes (timer, context) from 1024px. Not done: share image, system sleep rule (a timestamp timer needs none), and anything in phases 2 and 3.
- [x] 1.15 **Interface refresh** (U-01, U-02, H-02): two ways out of a session, End session (saves it as it is and counts as a round, however long it ran; asks first while time remains) and Abandon session (saves nothing; asks first), with no automatic discarding of short sessions and no rule that early endings do not count (the earlier full-session round rule and the 30-second discard rule were removed, shared vectors updated); the timer's idle screen is a prompt ("What will you focus on?"), a category pill that opens a picker (filter, choose, rename, recolour, delete, add), the intention field, a tick-ring dial with a filling pie and a hand (drag, keys and slider semantics as before), the length as a clock with the from-to time (the dial glides smoothly under the finger and settles on the nearest step when released; dragging back no longer switches to a break, the toggle button does that), one wide start button, and a break toggle at the top right; the day panel beside it has day arrows and a "Today" button, a week strip with hours per day, a mood line (focused, neutral, distracted and total focus) and an hourly timeline with the current-time line, the sessions as blocks and a hatched block for the session about to start, plus a round add-session button; Profile is its own screen (account, devices and scripts, your data) with Settings pinned to the bottom of the rail; a small timer bar (what is running, minus 5, play or pause, plus 5) stays at the bottom of History, Profile and Settings; the month overview gains a "time by intention" table. Every earlier feature stays. Visuals, wording and artwork are this project's own (rules in `CLAUDE.md`). Checked by build and tests only; the browser tool could not open the local app, so none of it has been looked at yet.

### Phase 2 (P2)
- [x] 2.1 Strict mode, presence nudges, special breaks, compact view (T-12..T-15). **Done:** strict mode (T-13) has a switch in Timer settings; the screen already follows the state machine, so pause, abandon and skipping a running break disappear once it is on, while finishing a session early and resuming stay allowed (core, server and both ports already enforced it; no new rules). Presence nudge (T-14): per-device Alerts setting (off, 5, 10, 15, 30 min); one soft tone per whole interval while a focus session counts down, never a burst after sleep, silent when paused or in quiet hours (`lib/nudge.ts`, tested). Special breaks (T-12): per-device list in Settings (name, time, minutes, weekdays, up to 6); when one is due (its time until 20 minutes after, once a day) and the timer is idle, the timer screen offers "Start break" or "Not now" and the leader tab plays the break-end tone and notification; starting uses `take_rest` with its own length, so the round cycle is untouched (`lib/specialBreaks.ts`, tested). Compact view (T-15): a per-device button at the top right of the timer screen switches to a narrow column with only the clock, dial and buttons (no sidebar, tab bar, round dots, category line or context pane) and back; other screens always use the full layout. Not done: a separate small browser window and picture-in-picture (T-16, P3).
- [x] 2.2 **Task list linked to sessions** (C-07): new `task` entity (title, done, done_at, category) validated like the others, sent in `/api/state`, included in the JSON export and restored by import; a session started from a task carries its id (`task` on the timer state and the saved session; planner in both ports, vectors and server tests updated). The timer screen has a Tasks card (add, tick off, delete, done list, tap one to fill the intention and category and mark "Working on"), and the reflect screen offers "Mark task as done" for the session's task. Checked: Rust and TS vectors, validation, backup round trip, and over HTTP a task added and a session started on it. Not checked in a browser. Not done: time spent per task, editing a task title, tasks in CSV/ICS exports.
- [x] 2.3 Stats (H-05..H-07): KPI cards, category table and notes-only view came with 1.14; this step adds the 52-week heatmap on the History screen (`lib/heatmap.ts`, tested): a column per week, Monday first, ending with this week, each day shaded in the theme accent by quartiles of the person's own non-empty days, scrolled to the latest weeks, every cell labelled with its date and focus time. Uses the existing `/api/summary` totals (no new endpoint). Not checked in a browser.
- [x] 2.4 **Themes, shortcuts** (U-04, U-07; more languages not done): custom shortcuts as described (per device, Settings). **Themes** went into the kit (cuppa repo, commit "Added Extra Flavors and Custom Themes to the Kit") and were copied here: new flavors `contrast-light`, `contrast-dark` (text at 21:1, every accent 6:1 or better), `nord`, `gruvbox`, `solarized` (accents nudged to 4.5:1 on the page) under the same `--ctp-*` names, so every component works unchanged; `theme.js` adds a `ui-light` class for every light flavor (the app's light-only contrast rules now use `html.ui-light` instead of `html.latte`) and a `custom` flavor: JSON in `ui-custom` (`base` flavor plus `#rrggbb` colours for any of 26 tokens; unknown keys and bad values ignored), applied at once and followed live by other open tabs through a `storage` listener. Settings has the new flavor chips and a "My theme" editor (paste JSON, Apply, example, reasons for refusal; `lib/customTheme.ts`, tested); the look in a backup includes `ui-custom` only if it parses. Not done: more languages (the string catalog is English only), the kit gallery was not updated, and the axe contrast audit was not re-run on the new flavors (only computed ratios for text and accents).
- [x] 2.5 **Custom sounds and tick sounds** (A-04, A-05): Alerts settings has a faint tick every second while focusing and, separately, during breaks (per device; Web Audio click, no files; one tick per new second, never a burst after sleep; `lib/ticking.ts`, tested). Each alert (ending soon, focus ends, break ends, both overtime reminders, presence nudge) can be replaced by an uploaded file: `GET /api/sounds`, `GET/PUT/DELETE /api/sounds/{slot}`; stored in a new `sounds` table inside the SQLite file (migration 4), so the admin's database backup includes them and "delete my data" removes them; at most 512 KiB, six slots, per user; the file type is decided from its first bytes (MP3, WAV, OGG, FLAC, M4A, WebM), never from the client's claim; users cannot read each other's sounds (tests). The browser decodes them with Web Audio, caches the bytes in IndexedDB for offline use, and re-fetches when the server's copy changes; the same volume and quiet-hour rules apply. Not done: sounds are not in the JSON export (the file is too big for it) and other devices pick up changes on their next load. Not checked in a browser.
- [x] 2.6 **Integration** (X-08, X-09, I-02..I-06): **Tokens:** personal API tokens (`opt_...`, shown once, only a SHA-256 hash stored, up to 20, revocable, dead with a disabled account) and a signed-in-browsers list with sign-out per browser (the sign-in's user agent is kept as its label); both in Settings, "Devices and scripts" (migration 5). **API v1** (`docs/api.md`): `GET /api/v1/state`, `POST /api/v1/commands` (same idempotent commands as the app), `GET /api/v1/sessions`, and a WebSocket `GET /api/v1/stream` that sends the whole state on connect and after every change; bearer token only, cookies are never accepted there, so those routes are exempt from the CSRF header (the rest of `/api` still needs it). **Hooks and webhooks:** the admin lists them in a JSON file named by `OP_HOOKS_FILE` (never from the web app); nine events (`focus_started`, `focus_ended`, `focus_stopped`, `focus_abandoned`, `paused`, `resumed`, `break_started`, `break_ended`, `break_stopped`) derived from the timer's before and after state (`hooks::events_between`, tested for every transition incl. auto-start chains); a hook is a program run directly (no shell) with the JSON on stdin and `OP_EVENT` in its environment, a timeout and at most 8 at once; a webhook is a POST with an optional HMAC-SHA256 signature (checked against the RFC 4231 vectors), one retry, no redirects, and private, loopback and link-local addresses refused at connect time unless `allow_private_webhooks` is set; events go through a bounded queue and are dropped (with a log line) if it overflows, so a slow hook never delays the timer; the Admin screen lists them and has a Test button each. **New dependency:** `ureq` 3 with rustls for webhooks (the release binary grew from about 5.8 MB to 8.5 MB). Docs: `docs/api.md`, hooks and webhooks in `docs/self-hosting.md`. Not checked in a browser; the webhook test route and the hooks panel are covered by compile and unit tests only.

Original 2.7 (website blocker extension) moved to L1 and 2.8 (diagnostics, release v0.2) to N3; 2.4's languages moved to N4.

### Next: finish the PWA (everything below works in a browser, installed or not)
Order is a suggestion; each step is small enough to commit on its own. Items marked (check) are built but were
only verified by tests, not by using them.

**N1. Try it for real and fix what breaks (before anything new)**
- [ ] N1.1 Click through every screen added in Phase 2 in a real browser, in Latte, Mocha and a high-contrast flavor: tasks, special breaks, compact view, strict mode, nudge and tick sounds, custom sound upload, shortcut rebinding, heatmap, devices and scripts, admin hooks panel, "My theme" editor (check).
- [ ] N1.2 Re-run the axe accessibility audit on every screen and state, in all flavors including the new ones (the earlier audit predates Phase 2). Fix what it finds.
- [ ] N1.3 Responsive pass: phone (375px), tablet, laptop, wide (1920px). Controls below the fold on short windows (about 700px tall); icon rail; two-pane timer; compact view.
- [ ] N1.4 Build and run the Docker image once (never done: no daemon was available), and check the release binary starts from an empty folder with the hooks file and an uploaded sound.
- [ ] N1.5 Lighthouse accessibility and performance run, plus the manual install / offline / update checklist from section 5.

**N2. Small gaps left in finished steps**
- [ ] N2.1 Tasks: edit a title, show time spent per task (sessions already carry the task id), include tasks in CSV and ICS exports.
- [ ] N2.2 Custom sounds: other devices pick up a change at once (react to the push message), and include the sound files in the user's own backup (a separate download, since they are too big for the JSON).
- [ ] N2.3 Share image for the stats (a picture of the month overview made in the browser).
- [ ] N2.4 Webhook test route and the hooks panel: add end-to-end tests.
- [ ] N2.5 More specific wake handling: a message when the app comes back after a long sleep mid-session.

**N3. Diagnostics and release v0.2 (D-05)**
- [ ] N3.1 Diagnostic log kept in the browser (state changes, rejected commands, sync and outbox events, clock jumps) with a copy button in Settings.
- [ ] N3.2 Server log lines for hooks, webhooks and rejected events available to the admin on the Admin screen.
- [ ] N3.3 Update `CHANGELOG.md`, docs and screenshots; tag **v0.2**.

**N4. Make it speak more languages (U-06)**
- [ ] N4.1 Language picker (per device, remembered; follows the browser by default) and a catalog format that allows more than English.
- [ ] N4.2 One more language end to end to prove the plumbing (dates, numbers, plurals already use `Intl`).
- [ ] N4.3 A short translators' guide in `CONTRIBUTING.md`; the string checker also verifies that every language has every key.

**N5. Alerts and presence that work as well as a PWA can**
- [ ] N5.1 Web Push (A-07): self-hosted VAPID keys, subscribe per device, send phase-end alerts while the app is closed and the daily reminder with weekday toggles. Needs HTTPS; on iPhone only for an installed app. Say plainly where it does not work.
- [ ] N5.2 Picture-in-picture timer window (T-16) where the browser supports it (Chromium); hidden elsewhere.
- [ ] N5.3 Spoken announcements (A-06) through speech synthesis, off by default.
- [ ] N5.4 Callback URLs on routes (`x-success` / `x-error` style, I-07), with the same confirm step as other links.

**N6. Accounts and data depth**
- [ ] N6.1 TOTP two-factor sign-in with recovery codes (X-10); optional OIDC sign-in configured by the admin.
- [ ] N6.2 Profiles per intention + category: own durations, sounds, strictness (C-08).
- [ ] N6.3 Category groups for reports (C-09).
- [ ] N6.4 Read-only calendar overlay from an ICS file or URL on the day timeline (H-08).

**N7. Housekeeping outside this repo**
- [ ] N7.1 Bring the cuppa kit up to date with what this app learned (responsive layout: icon rail from 1024px, two panes, container-query sizing for the clock, `.seg` thumb for dynamically rendered content, contrast fixes already made here), push the kit commit, and run the kit's lint with this project registered.

### Later: per-device apps (parked until the PWA is finished)
These need something outside the browser tab, so each brings its own build, signing and update headaches. Nothing
in the PWA depends on them.
- [ ] L1 Website blocker extension and escape hatch (F-01, F-02): MV3 extension using a personal token; deny or allow list, separate lists for focus and break, snooze, block page showing the intention.
- [ ] L2 Desktop shell (F-04): tray with progress, global shortcuts, always on top, close to tray, auto-update, alerts while the window is closed.
- [ ] L3 App blocking that hides and never kills (F-05), full-screen break overlay (F-06), window tracker that suggests a session (F-07).
- [ ] L4 Native phone apps, only if the PWA hits real limits.


## 4b. Platform roadmap
| Target | When | How | Notes |
|---|---|---|---|
| **Browser PWA** (desktop and mobile browsers, installable) | **Now (P1)** | Kit-based PWA served by the self-hosted Rust server, with accounts | Notifications and sounds while the page is alive, screen wake lock, badge API, audio unlock on first tap. Limits: no alerts while closed, no global shortcuts, no tray |
| Installed PWA on phones and tablets | P1 polish | Responsive layout, safe-area insets, touch dial | Mobile browsers suspend background tabs quickly; alerts are reliable only with the app open. iOS needs install-to-home-screen for push (P3); document it |
| Desktop shell | P3 | Thin native wrapper around the same web UI, adding tray, global shortcuts, app blocking, forced break, closed-window alerts (F-04..F-07) | Adapters only; no UI fork |
| Native phone apps | P3+ | Wrapper or rewrite decided later; reuse core crate and API | Only if the PWA hits real limits |
| Self-hosted server (home machine, NAS, VPS) | **P1** | Docker image or single binary behind a TLS reverse proxy (D-06) | The main way to run the app; several people and several devices share it |

Rules: every platform feature is behind a `platform` adapter with a browser fallback; the UI never checks the device type, only capabilities.

## 5. Quality bar per step
- Unit tests for logic; one integration test for each API path; a manual checklist for UI.
- Timer correctness tests with a fake clock: pause during overflow, extend during overflow, shrink past zero, clock moves backwards, tab hidden for an hour.
- Rust and TS both pass the shared JSON vectors.
- Multi-user: user A can never read, change, export or get pushes of user B's data; sign-in rate limit, CSRF and host checks have tests.
- Two tabs open: only the leader alerts. Server stopped mid-session: outbox replays once on restart, no duplicates.
- Before each release: Lighthouse accessibility and performance checks, plus a manual install / offline / update checklist (Lighthouse no longer audits PWAs).
- No step is done until its status box is ticked and any new setting is documented.

## 6. Open decisions
- Whether `web/dist` is committed (server builds without Node) or built in CI/release only.
- Final name check and logo (original artwork only).

### Decided
- Multi-user, self-hosted from v1 (decided with the owner after step 1.4): accounts with cookie sessions, argon2id, first user is admin, registration closed by default, recovery by the admin. This replaces the earlier local-only, no-accounts design.
- SQLite through `rusqlite` (bundled, sync), called from async handlers behind a mutex or `spawn_blocking`; chosen for simplicity, the data volume is tiny.
- License: MIT (simple and familiar; Apache-2.0's patent grant was not needed for a small app).
- Svelte 5 + Vite (static bundle) over a meta-framework; a marketing/docs site, if any, is a separate project.
- Timer logic is shared with the PWA as a TS port checked against shared JSON test vectors (WASM may replace the port later).
- Data ownership in P1: the server's SQLite is the source of truth, per user; the PWA keeps a per-user IndexedDB outbox for offline periods (section 2).
- P1 alerts work only while the page is alive; closed-app alerts are P3 (push or desktop shell).
