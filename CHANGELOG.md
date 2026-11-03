# Changelog

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
