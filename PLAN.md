# OpenPomodoro: feature list and build plan

Status legend: `[ ]` todo, `[~]` in progress, `[x]` done. Phase tags: **P1** = v1 (local-only PWA + Rust), **P2** = v1.x, **P3** = later / v2.

## 1. Goals
- A calm, fast, free focus timer that records *what* you worked on, not just *when*.
- Local-first: one SQLite file, full export, no account.
- Installable PWA that works offline; Rust backend for storage, API, hooks and (later) sync.
- Beautiful and themeable; accessible; translatable.

Non-goals for v1: accounts, cloud, native mobile apps, native app/window blocking, alerts while the app is closed, a serverless (static-host only) mode.

## 2. Architecture
- `crates/core`: pure Rust. Timer maths, state machine table, id generator, event types, projections. Injectable `Clock` trait. No I/O.
- `crates/server`: `axum` HTTP + WebSocket, SQLite (WAL), migrations, event log, settings, export, hooks runner, static file serving.
- `web/`: browser PWA, **Svelte 5 + Vite + TypeScript**, built as a static bundle (no SSR, no meta-framework), styled by the **cuppa-ui kit** (Tailwind v4 build, Catppuccin themes, Lucide sprite, kit `theme.js`/`ui.js`). The framework is a recorded exception to the kit's "no UI framework" rule; all other kit rules hold (no CDN, no webfonts, native dialogs). Timer and state logic live in plain `.ts` modules and a store; components only render. Talks to the server by JSON API and a WebSocket for live state. The service worker caches the app shell only. The server serves `web/dist`.
- `web/src/platform/`: one adapter per capability (notifications, wake lock, badge, audio unlock, shortcuts, install prompt, tab leader) with feature detection. The browser adapter ships first; desktop and mobile adapters come later without UI changes.
- `extension/` (P2): MV3 extension for website blocking, reading state from the local API (works only while the server runs).

### Data ownership (P1)
- The server's SQLite file is the only source of truth. P1 needs the server running; the PWA is served by it.
- The PWA keeps the live timer state and an **outbox** of not-yet-sent events in IndexedDB. If the server is unreachable, the timer keeps running from timestamps and new events queue in the outbox. On reconnect they are posted in id order; the server applies each event once (an event id it already has is a no-op), then the PWA reloads state from the server.
- This is a one-way outbox from one browser, not sync. Two-way, multi-device sync is I-08 (P3).
- App settings live in SQLite as `SETTING` events. Kit appearance keys (`ui-*`, localStorage) are per-browser and are copied into the JSON export and backup so nothing is lost.

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

### Events
- Entities: `SESSION` (with `kind`: `focus` or `break`), `CATEGORY`, `SETTING`, `TIMER` (live timer state); `TODO` is added with C-07 (P2). Operations `ADD_/EDIT_/DELETE_`, payload JSON, applied in one transaction.
- Add of an existing id becomes an edit. Edit or delete of a missing id is rejected and written to a `rejected_events` table (kept so it can be replayed if its add arrives later). Delete writes a tombstone.
- Every state machine transition writes a `TIMER` event and updates the `current_timer` projection, so a reload, crash or second tab resumes the running phase.
- A focus session becomes a `SESSION` record when it leaves `reflect`, or when the phase ends with auto-start break on (reflect is skipped; mood and notes can be added later from history). A break becomes a `SESSION` record when it ends or is stopped or skipped. Abandon writes no `SESSION`. C-06 drops focus sessions of 30 s or less with no intention.

### Ids
- Format `<UTC yyyyMMddHHmmss>.<5-digit seq>@<device uuid>`; sorts as a string.
- Strictly increasing per device: on start, restore from the max id **this device** issued (ignore other devices' ids, so a device with a fast clock cannot drag others forward). If the clock moves back, keep the last timestamp and increment `seq`; if `seq` passes 99999, advance the timestamp by one second.
- Across devices the order is wall clock only; ids do not prove causality.

### State machine
States: `idle, breathe, session, paused, session_end (overflow), reflect, rest, rest_end (break overflow)`. `paused` remembers whether it paused `session` or `rest` and returns there.

| Event | From | To | Notes |
|---|---|---|---|
| `init_focus` | idle, rest_end | breathe, or session if breathing is off | |
| `skip_breathe` | breathe | session | |
| `start_focus` | idle, rest_end | session | starts without the breathing step |
| `timer_ends` | breathe / session / rest | session / session_end / rest_end | session goes to rest instead when auto-start break is on |
| `toggle_pause` | session, rest, paused | paused, or back | blocked in strict mode |
| `extend(+/-)` | session, session_end, rest, rest_end, paused | same state, or *_end if shrunk past zero | |
| `stop` | session, session_end, paused (from session) | reflect | end focus early, keep it |
| `stop` | rest, rest_end, paused (from rest) | idle | end break early, keep it |
| `abandon` | breathe, session, session_end, paused (from session) | idle | nothing saved; blocked in strict mode |
| `take_rest` | reflect, idle | rest | saves the focus session when leaving reflect |
| `done` | reflect | idle | saves the focus session, no break |
| `skip_rest` | rest, rest_end | idle, or session if auto-start focus is on | blocked in strict mode |

Any other `(state, event)` pair is rejected and logged. The full table is tested row by row in step 1.2.

### Security
- The server binds `127.0.0.1` by default and checks `Host` (local names only, against DNS rebinding) and `Origin` (the app's own origin) on every API and WebSocket request.
- A per-install token is stored in the SQLite file and delivered to the same-origin PWA; every request that changes data, and the WebSocket, must send it in a header.
- Hooks (I-04, I-05) can only be configured from the same-origin UI or the config file.
- Listening on a LAN address (home server / NAS) is opt-in, requires the token on every request, and should sit behind TLS or a reverse proxy.
- Links that change state (`#/start`, `#/pause`, `#/break`, `#/finish`, protocol handler) show a confirm step unless the user turns on "Allow links to control the timer". Read-only routes navigate directly.

## 3. Feature list (the superset)

### A. Timer and cycle
| Id | Feature | Phase |
|---|---|---|
| T-01 | Configurable focus, short break, long break durations | P1 |
| T-02 | Rounds before a long break; long-break rule | P1 |
| T-03 | Timestamp-based timer, correct after reload, sleep, second tab (leader tab plays alerts) | P1 |
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
| C-06 | Discard sessions of 30 s or less with no intention | P1 |
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
| A-07 | Web Push: phase-end alerts while the app is closed, daily reminder with weekday toggles (needs a push service) | P3 |
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
| I-02 | Public local HTTP API for scripts: documented and versioned, token auth (built on the P1 internal API) | P2 |
| I-03 | Public WebSocket stream of state changes for overlays and scripts (built on the P1 push channel) | P2 |
| I-04 | Lifecycle hooks: start, pause, unpause, end, break start/end, went-too-long; per-hook toggle and a Test button | P2 |
| I-05 | Hook runner for the server: executable per event with JSON on stdin, timeout, queue | P2 |
| I-06 | Outgoing webhooks | P2 |
| I-07 | `x-success` / `x-error` style callback URLs on routes | P3 |
| I-08 | Optional self-hosted, two-way sync between devices (token in header, not URL) | P3 |

### G. Focus protection
| Id | Feature | Phase |
|---|---|---|
| F-01 | Website blocker extension (needs the server running): deny or allow list, separate lists for focus and break, active tab only, snooze 3 min, restore all, kind block page showing the intention | P2 |
| F-02 | Escape hatch: blocking off when the intention matches a chosen keyword | P2 |
| F-03 | (merged into T-13) | – |
| F-04 | Desktop shell (tray with progress, global shortcuts, always on top, close to tray, auto-update, alerts while the window is closed) | P3 |
| F-05 | App blocking in the desktop shell (hide, never kill) | P3 |
| F-06 | Full-screen break overlay in the desktop shell | P3 |
| F-07 | Window tracker that suggests a session | P3 |

### H. Data and platform
| Id | Feature | Phase |
|---|---|---|
| D-01 | SQLite storage with migrations, WAL, one file | P1 |
| D-02 | Event log with string-sortable ids | P1 |
| D-03 | Installable PWA, offline shell, outbox while the server is unreachable, update prompt | P1 |
| D-04 | Backup / restore (JSON), delete all data | P1 |
| D-05 | Diagnostic log with a copy button | P2 |
| D-06 | Docker image and single static binary release; opt-in LAN listening | P2 |

## 4. Build order

### Phase 0: scaffold
- [~] 0.1 Cargo workspace (`core`, `server`); `web/` created as Svelte 5 + Vite + TS and initialised with the cuppa-ui kit (`init` script, Tailwind + icon sprite scripts, `theme.js`, `ui.js`); check script running fmt, clippy, tests, `svelte-check` and build (`scripts/check.sh`). Done except the kit lint: it needs this project registered in the cuppa plugin's `harness.json`, which lives outside this repo.
- [x] 0.2 `server` serves `web/dist` and `/api/health`; binds `127.0.0.1`; `Host`/`Origin` checks in place from the start.
- [x] 0.2b PWA skeleton: manifest, icons (original artwork), service worker (shell cache only), install prompt, app shell with tab bar, hash router, theme/flavor/accent settings wired to the kit hooks, i18n catalog and `t()` helper (no literal UI strings from here on). Offline after first load.
- [x] 0.3 Docs skeleton: README, CONTRIBUTING, LICENSE. License: **MIT**.

### Phase 1 (P1)
- [x] 1.1 **Timer core** (T-01..T-05): pure types, `Clock` trait, timer rule from section 2 (extend/shrink, overflow extend, backwards-clock clamp); tests for pause/resume/extend, clock jumps, sleep. Shared JSON vectors in `test-vectors/timer.json` (17 cases) pass in Rust and in the TS port `web/src/lib/timer.ts`. Config bounds (1 min to 12 h per phase, at most 12 rounds) are guesses.
- [ ] 1.2 **State machine** (section 2 table): table-driven, reject unknown pairs, tests for every row, including breathe, overflow, rest, reflect, auto-start and strict-mode blocks. Shared JSON vectors for every row; TS port in `web/src/lib` checked against them.
- [ ] 1.3 **Ids and event log** (D-02): per-device monotonic generator (backward clock, `seq` overflow, restore from this device's max id), events and projections, idempotent add, rejected-event table, tombstones.
- [ ] 1.4 **SQLite layer** (D-01): migrations, repositories for sessions (focus and break), categories, settings, `current_timer`; one transaction per event.
- [ ] 1.5 **API v0** (internal, for the PWA): state, commands, history, settings, idempotent event intake for the outbox; WebSocket push of state; per-install token. One integration test per path, plus token, `Host` and `Origin` rejection tests.
- [ ] 1.6 **PWA shell** (D-03, U-01, U-03): IndexedDB live state and outbox with replay on reconnect, leader tab, update prompt, `platform` adapters for the browser.
- [ ] 1.7 **Timer screen** (T-04, T-05, T-07, T-09, T-10, U-02, U-09, A-08): intention field, category pill, dial with keyboard/slider alternative, pause / stop / abandon / ±5 controls, breathe step, state colors, wake lock.
- [ ] 1.8 **Overflow and alerts** (T-06, T-08, T-11, A-01..A-03): page-side scheduler in the leader tab, overflow reminders every N minutes (max 12), sounds per phase, missed-phase summary on wake, closed-app limit shown in settings.
- [ ] 1.9 **Capture** (C-01..C-06): notes pane with recent notes, reflection mood (also editable from history), retroactive entry, junk-session rule.
- [ ] 1.10 **History** (H-01..H-04): log, day timeline with breaks, totals, streaks, export (including kit `ui-*` settings in JSON).
- [ ] 1.11 **Routes and shortcuts** (I-01, U-07): hash routes with confirm for state-changing links, protocol handler, key bindings.
- [ ] 1.12 **Polish** (U-05, U-06, U-08, D-04): reduced motion, string catalog review, accessibility audit, backup/restore and delete all. Release **v0.1**.

### Phase 2 (P2)
- [ ] 2.1 Strict mode, presence nudges, special breaks, compact view (T-12..T-15).
- [ ] 2.2 Task list linked to sessions (C-07).
- [ ] 2.3 Stats: heatmap, KPI cards, category table, notes-only view (H-05..H-07).
- [ ] 2.4 Themes: high-contrast, bundled set and user JSON themes added to the kit, then wired; more languages; custom shortcuts (U-04, U-06, U-07).
- [ ] 2.5 Custom sounds and tick sounds (A-04, A-05).
- [ ] 2.6 Integration: public HTTP API and WebSocket stream (documented, versioned), hooks with Test button, webhooks (I-02..I-06).
- [ ] 2.7 Website blocker extension and escape hatch (F-01, F-02).
- [ ] 2.8 Diagnostics, Docker image, single-binary release, opt-in LAN listening (D-05, D-06). Release **v0.2**.

### Phase 3 (P3)
- [ ] 3.1 Optional self-hosted sync using the event log (I-08): single-flight with debounce, backoff with jitter, ping, token in header, sync only events originated locally and newer than the last synced id, replay of rejected events when their add arrives.
- [ ] 3.2 Profiles and category groups (C-08, C-09). Calendar overlay (H-08).
- [ ] 3.3 Picture-in-picture, speech, Web Push alerts and daily reminder (T-16, A-06, A-07). Callback URLs (I-07).
- [ ] 3.4 Desktop shell (F-04..F-07): tray, global shortcuts, app blocking, forced break, window tracker.

## 4b. Platform roadmap
| Target | When | How | Notes |
|---|---|---|---|
| **Browser PWA** (desktop and mobile browsers, installable) | **Now (P1)** | Kit-based PWA served by the Rust server (required in P1) | Notifications and sounds while the page is alive, screen wake lock, badge API, audio unlock on first tap. Limits: no alerts while closed, no global shortcuts, no tray |
| Installed PWA on phones and tablets | P1 polish | Responsive layout, safe-area insets, touch dial | Mobile browsers suspend background tabs quickly; alerts are reliable only with the app open. iOS needs install-to-home-screen for push (P3); document it |
| Desktop shell | P3 | Thin native wrapper around the same web UI, adding tray, global shortcuts, app blocking, forced break, closed-window alerts (F-04..F-07) | Adapters only; no UI fork |
| Native phone apps | P3+ | Wrapper or rewrite decided later; reuse core crate and API | Only if the PWA hits real limits |
| Self-hosted server on a home machine/NAS | P2/P3 | Docker image, single binary, opt-in LAN listening with token and TLS/proxy (D-06), optional sync (I-08) | Lets several devices share data |
| Serverless static-host PWA | Later, if wanted | Browser becomes the source of truth | Needs its own data-ownership design; not planned |

Rules: every platform feature is behind a `platform` adapter with a browser fallback; the UI never checks the device type, only capabilities.

## 5. Quality bar per step
- Unit tests for logic; one integration test for each API path; a manual checklist for UI.
- Timer correctness tests with a fake clock: pause during overflow, extend during overflow, shrink past zero, clock moves backwards, tab hidden for an hour.
- Rust and TS both pass the shared JSON vectors.
- Two tabs open: only the leader alerts. Server stopped mid-session: outbox replays once on restart, no duplicates.
- Before each release: Lighthouse accessibility and performance checks, plus a manual install / offline / update checklist (Lighthouse no longer audits PWAs).
- No step is done until its status box is ticked and any new setting is documented.

## 6. Open decisions
- `rusqlite` (simple, sync) vs `sqlx` (async, compile-time checks).
- Whether `web/dist` is committed (server builds without Node) or built in CI/release only.
- Final name check and logo (original artwork only).

### Decided
- License: MIT (simple and familiar; Apache-2.0's patent grant was not needed for a small app).
- Svelte 5 + Vite (static bundle) over a meta-framework; a marketing/docs site, if any, is a separate project.
- Timer logic is shared with the PWA as a TS port checked against shared JSON test vectors (WASM may replace the port later).
- Data ownership in P1: the server's SQLite is the source of truth; the PWA keeps an IndexedDB outbox for offline periods (section 2). Default chosen while fixing the plan; revisit before step 1.5 if a serverless mode is wanted.
- P1 alerts work only while the page is alive; closed-app alerts are P3 (push or desktop shell).
