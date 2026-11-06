# Changelog

## 0.2.0

The browser app is now feature-complete for what a browser can do. Nothing here needs a desktop or phone wrapper.

- **Interface:** a redesigned timer screen (prompt, category picker, tick-ring dial that drags smoothly and snaps on
  release, one start button), a day panel with an hourly timeline and the session about to start, a Profile screen,
  a small timer bar on the other screens, an icon rail and two panes on wide screens, a compact view, and a
  floating picture-in-picture timer where the browser has one.
- **Timer:** End session (saves it, counts as a round) and Abandon session (saves nothing), each asking first;
  timers as short as 5 seconds for trying the flow; strict mode; presence nudge; special breaks at fixed times;
  optional tick sound; spoken alerts; your own alert sounds.
- **Capture and reports:** tasks linked to sessions (rename, time spent, in exports), category groups and a usual
  length per category, month overview with time by category, group and intention, a 52-week heatmap, a share
  image, a calendar-file overlay on the day view.
- **Accounts and access:** two-step sign-in with recovery codes (and an admin reset), personal API tokens, a list of
  signed-in browsers, a documented token API with a live WebSocket stream ([docs/api.md](docs/api.md)).
- **Integration:** lifecycle hooks (programs) and signed webhooks set in a file by the admin, with Test buttons;
  callback addresses on acting links; closed-app alerts through Web Push.
- **Look and language:** high-contrast, Nord, Gruvbox and Solarized themes, your own JSON theme, a language picker
  (English and Spanish) and a translators' guide, custom keyboard shortcuts.
- **Diagnostics:** a copyable log on each device and a server log for the admin.
- **Changed:** the 30-second automatic discard and the "early endings do not count" rule are gone; a database
  upgrade runs on first start (new tables for sounds, tokens, two-step and push).
- **New dependencies:** `ureq` (webhooks and push, with rustls), `p256`, `aes-gcm`, `sha1`.

Known gaps: no OpenID Connect sign-in, no QR code for two-step setup, no website blocker or desktop shell (parked, see
[PLAN.md](PLAN.md)), the Docker image has never been built, and the newest screens were checked by tests and the
build, not yet by hand in a browser.

## 0.1.0

First release: a self-hosted, multi-user Pomodoro and focus tracker.

- **Server:** one Rust program with the web app built in and one SQLite file. Accounts (the first is the admin,
  sign-up closed by default, recovery by the admin), per-user data, sign-in rate limits, host/origin and CSRF
  guards, security headers, live updates over WebSocket.
- **Timer:** focus, short and long breaks, pause, overflow, strict and auto-start modes, notes and mood on
  reflection. Time is stored as timestamps, so reloads, sleep and a second device stay correct.
- **History:** timeline, daily totals and stats, editing, categories, export (JSON, CSV, ICS), backup and restore.
- **App:** installable and offline-capable; commands made offline are replayed with their true times. Sound and
  notification alerts with quiet hours, links and keyboard shortcuts, four themes, reduced motion, screen reader
  announcements.
- **Hosting:** `scripts/build-release.sh`, a `Dockerfile` and `docker-compose.yml` (written but not yet built in CI),
  and [docs/self-hosting.md](docs/self-hosting.md).

Known gaps: no push notifications while the app is closed, no site blocking, no sync between servers, no desktop
or phone wrappers yet. See [PLAN.md](PLAN.md).
