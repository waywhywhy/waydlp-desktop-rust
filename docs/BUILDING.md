# Building waydlp desktop

## Toolchain

* Rust stable (edition 2024). Install from <https://rustup.rs>.
* **Windows:** Visual Studio Build Tools with the "Desktop development with C++" workload (MSVC linker).
* **Linux:** a C compiler and development headers:

  ```bash
  # Debian / Ubuntu / Mint
  sudo apt install build-essential pkg-config libxkbcommon-dev libwayland-dev libx11-dev \
    libxcursor-dev libxrandr-dev libxi-dev libgl1-mesa-dev libdbus-1-dev
  # Fedora
  sudo dnf install gcc pkgconf-pkg-config libxkbcommon-devel wayland-devel libX11-devel \
    libXcursor-devel libXrandr-devel libXi-devel mesa-libGL-devel dbus-devel
  # Arch and derivatives
  sudo pacman -S --needed base-devel libxkbcommon wayland libx11 libxcursor libxrandr libxi mesa dbus
  ```

## Run

```bash
cargo run -p waydlp-gui            # debug build of the app
cargo run -p waydlp-gui --release  # optimised
cargo run -p waydlp-cli -- analyze "https://…"
```

Set `WAYDLP_LOG=debug` for verbose logs (written to the logs folder shown by `waydlp-cli paths`).

To try portable mode, create an empty `waydlp.portable` file next to the built executable (e.g. `target/debug/waydlp.portable`).

## Runtime dependencies

* **yt-dlp** (required). Install a verified managed copy with `waydlp-cli install-tool yt-dlp` or from *Settings → yt-dlp & FFmpeg*, or use a system copy (`pipx install yt-dlp`, your package manager, `winget install yt-dlp.yt-dlp`).
* **FFmpeg + FFprobe** (recommended). Windows: `waydlp-cli install-tool ffmpeg`. Linux: install from your distribution.

## Tests

```bash
cargo test --workspace --all-features
```

* `waydlp-core`: URL handling, metadata parsing against **real yt-dlp JSON fixtures** (`crates/waydlp-core/tests/fixtures`), argument generation, progress/error parsing, sanitisation, artifact naming.
* `waydlp-engine`: process-level integration tests with `waydlp-fake-yt-dlp` (built with the `fake-yt-dlp` feature) covering progress, completion, cancellation (process tree killed, partial files deleted), pause/resume, automatic retry, concurrency limits and probe errors. Deterministic, no network.
* `waydlp-config`, `waydlp-history`, `waydlp-platform`, `waydlp-installer-core`: persistence, migrations, receipts, archive safety, payload bundling, package-manager command generation.

The CI also runs a **network smoke test** (allowed to fail) that installs yt-dlp, analyses and downloads a Creative Commons video and checks the MP3 with ffprobe.

## Lints

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
