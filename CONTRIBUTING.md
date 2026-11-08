# Contributing to waydlp desktop

Thanks for helping! The general guidelines (code of conduct, how we review,
commit style) live in the [organisation contributing guide](https://github.com/waywhywhy/.github/blob/main/CONTRIBUTING.md).
This file covers what is specific to the Rust desktop app.

## Getting started

1. Install Rust stable and the platform dependencies listed in [docs/BUILDING.md](docs/BUILDING.md).
2. `cargo run -p waydlp-gui` to start the app, `cargo test --workspace --all-features` to run the tests.
3. Read [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — especially which crate owns what.

## Rules of the road

* **No fake functionality.** Every control must do what it says; progress and metadata must come from yt-dlp/FFmpeg.
* Keep `waydlp-core` free of I/O so it stays unit-testable. Put process/network code in `waydlp-engine`.
* **Never run commands through a shell** and never concatenate untrusted input into arguments. Add validation and tests for anything user-controlled.
* New user-visible options need: a sensible default, a tooltip or help text, persistence in `waydlp-config` (with a migration if the format changes) and tests.
* Don't add network requests that the user didn't ask for. No telemetry.
* Prefer maintained, permissively licensed dependencies; `cargo deny check` must pass.

## Before opening a pull request

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Describe what you changed, how you tested it (including manual testing on Windows and/or Linux for UI or packaging changes), and add a line to `CHANGELOG.md` under *Unreleased*.

## Reporting extraction problems

If a specific site fails, first update yt-dlp (Settings → yt-dlp & FFmpeg). If it still fails with plain `yt-dlp`, the problem belongs to [yt-dlp](https://github.com/yt-dlp/yt-dlp/issues). If yt-dlp works but waydlp doesn't, open an issue here with the technical details from the error view.
