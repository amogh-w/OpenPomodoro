# Contributing

Thanks for helping. Read [PLAN.md](PLAN.md) first: it lists the features (with ids like `T-03`), the architecture and the build order.

## Setup

Rust (stable) and Node.js. Then `npm --prefix web install` and `scripts/check.sh`.

## Ground rules

- **Original work only.** Do not copy code, assets, names, sounds, icons, copy text or layouts from other products. Describe behavior in your own words.
- **Small steps.** One plan step or feature per pull request. Reference its id (for example `T-03`) in the PR and in tests.
- **Rust:** `cargo fmt`, `cargo clippy -- -D warnings`, no `unwrap()` outside tests. Timer and state logic needs unit tests with a fake clock, including sleep and clock-jump cases.
- **Web:** Svelte 5 with TypeScript, `svelte-check` clean, no `any`. Timer and state logic lives in plain `.ts` modules; components only render. Use the kit's classes and theme hooks (`data-flavor`, `data-accent`, `data-radius`); no hard-coded colors, no hard-coded UI strings (use the translation catalog), no CDN or webfonts.
- **Accessible:** 44px targets, labels, keyboard use, and a look that works in both Latte and Mocha.
- **Docs:** short and factual. Mark guesses as such. Update the `PLAN.md` status table when a step finishes.

## Before you open a PR

Run `scripts/check.sh`. It must pass.

## License

By contributing you agree your work is released under the [MIT License](LICENSE).
