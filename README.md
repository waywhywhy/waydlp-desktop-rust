<p align="center">
  <img src="assets/icons/waydlp-512.png" width="112" alt="waydlp logo">
</p>

<h1 align="center">waydlp — desktop</h1>

<p align="center"><b>Your media, your format, your way.</b><br>
A fast, private, native media downloader for Windows and Linux, powered by
<a href="https://github.com/yt-dlp/yt-dlp">yt-dlp</a> and <a href="https://ffmpeg.org">FFmpeg</a>.</p>

---

waydlp lets you paste a link from YouTube, SoundCloud, Vimeo, Bandcamp and
[thousands of other sites supported by yt-dlp](https://github.com/yt-dlp/yt-dlp/blob/master/supportedsites.md),
see what's really available, pick a format and quality, and download.

* **Free and open source** (GPL-3.0-or-later). No ads, no accounts, no telemetry.
* **Native Rust app** with an AMOLED-black and purple interface (egui), accessible via screen readers (AccessKit).
* **Video** as MP4, MKV, WebM, MOV or the original file — only qualities that actually exist are offered.
* **Audio** as MP3, M4A, Opus, FLAC, WAV, OGG or the original stream, with optional loudness normalisation.
* **Playlists** with a preview and item selection, **batch** links, and a **download queue** with real progress, pause/resume, cancel and automatic retries.
* Subtitles, thumbnails/cover art, chapters and metadata embedding.
* Download history with search, filters, re-download and safe file deletion.
* Presets (built-in and your own), file-name templates, speed limit, proxy, cookies file, extra yt-dlp arguments.
* Verified, one-click installation of yt-dlp and FFmpeg (SHA-256 checked) — or use your system's copies.
* Portable mode, settings import/export, and a `waydlp-cli` for scripts.

> Only download media you have the right to access. waydlp does not circumvent DRM or other access controls.

## Install

Release artifacts are published on the [Releases page](https://github.com/waywhywhy/waydlp-desktop-rust/releases).
_No release has been published yet; see [Building from source](#building-from-source)._

| Platform | Artifact | Notes |
|---|---|---|
| Windows | `waydlp-<ver>-windows-x64-setup.exe` | Graphical installer, per-user, no admin rights |
| Windows | `waydlp-<ver>-windows-x64.msi` | Per-machine (Program Files), for admins / deployment |
| Windows | `waydlp-<ver>-windows-x64-portable.zip` | Run anywhere, data stays in the folder |
| Linux | `waydlp-<ver>-linux-x64.AppImage` | Portable single file |
| Debian/Ubuntu/Mint | `waydlp-<ver>-linux-x64.deb` | `sudo apt install ./waydlp-*.deb` |
| Fedora/openSUSE | `waydlp-<ver>-linux-x64.rpm` | `sudo dnf install ./waydlp-*.rpm` |
| Arch/CachyOS/EndeavourOS/Manjaro | `waydlp-<ver>-linux-x64.pkg.tar.zst` or the PKGBUILDs in [`packaging/arch`](packaging/arch) | AUR submission prepared, not yet published |
| Flatpak | `waydlp-<ver>-linux-x64.flatpak` | `flatpak install --user waydlp-*.flatpak`; Flathub submission prepared, not yet published |
| Linux | `waydlp-<ver>-linux-x64.tar.gz` | Portable archive; `./waydlp-installer install` for a user install |

ARM64 builds are produced for Windows and Linux where the CI can build them; they are less tested.

Every release includes `SHA256SUMS.txt`. Verify a download with `sha256sum -c SHA256SUMS.txt --ignore-missing` (Linux) or `Get-FileHash` (Windows).

### Command-line installer

```bash
waydlp-installer detect                 # platform and available methods
waydlp-installer install                # best method for this system, latest release
waydlp-installer install --format deb   # or: portable, msi, tar, appimage, rpm, arch, flatpak
waydlp-installer install --file ./waydlp-1.0.0-linux-x64.tar.gz --yes
waydlp-installer verify                 # check installed files against the receipt
waydlp-installer uninstall              # settings and history are kept
```

Package-manager formats are handed to `apt`, `dnf`/`zypper`, `pacman` or `flatpak`; the installer never writes into package-managed locations itself.

## Using waydlp

1. Paste a link on **Home** and press **Analyze** (or **Quick download** to use your default preset).
2. The **Downloader** shows the real title, thumbnail, duration and the qualities that exist. Choose *Video*, *Audio*, *Video only* or *Original file*, a container/format and a quality.
3. Press **Download**. Watch progress in the **Queue**; finished files appear in **History**.

FFmpeg is needed to merge separate video/audio streams, convert audio and embed metadata. If it's missing, use *Original file* mode or install it from **Settings → yt-dlp & FFmpeg**.

The full user guide is in [docs/USER_GUIDE.md](docs/USER_GUIDE.md).

## Building from source

Requirements: a recent stable Rust toolchain (edition 2024), and on Linux the usual X11/Wayland/OpenGL development headers:

```bash
sudo apt install pkg-config libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev libgl1-mesa-dev libdbus-1-dev
```

```bash
cargo run -p waydlp-gui --release        # the app
cargo run -p waydlp-cli -- --help        # the CLI
cargo test --workspace --all-features    # all tests (includes queue tests with a fake yt-dlp)
cargo clippy --workspace --all-targets --all-features
```

See [docs/BUILDING.md](docs/BUILDING.md) and [docs/PACKAGING.md](docs/PACKAGING.md) for installers and packages, and [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for how everything fits together.

## Project status

This is an early development version (0.1.0). See [CHANGELOG.md](CHANGELOG.md) and [docs/STATUS.md](docs/STATUS.md) for what is implemented, what was verified, and known limitations.

## Related repositories

* [waydlp-mobile-dart](https://github.com/waywhywhy/waydlp-mobile-dart) — Android app (Flutter)
* [waywhy-website](https://github.com/waywhywhy/waywhy-website) — website (Next.js)
* [.github](https://github.com/waywhywhy/.github) — organisation profile and community files

## License

GPL-3.0-or-later — see [LICENSE](LICENSE). yt-dlp is released under the Unlicense; FFmpeg under LGPL/GPL.
