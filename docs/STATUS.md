# Project status (0.2.0)

This page records what is implemented, what was actually verified and how, and what is still open. It is updated with each milestone.

## Verified

| Area | How it was verified |
|---|---|
| Core logic (URLs, metadata parsing, argument builder, progress/error parsing, sanitisation, artifact names) | 68 unit tests, including real yt-dlp JSON fixtures |
| Download queue: progress, cancel (process tree killed, partial files deleted), pause/resume, retries, concurrency | 10 integration tests against a fake yt-dlp process |
| Settings (migration, corruption backup, import safety), history (SQLite), platform, installer core | 40 unit tests |
| Real yt-dlp + FFmpeg (Windows) | CLI against a Creative Commons video. Analysis was correct and the original audio stream came out byte-exact. The MP3 conversion gave 320 kbps with cover art and the MP4 merge gave AV1 + AAC with a thumbnail, both confirmed with ffprobe. |
| Verified tool installs | The managed yt-dlp and FFmpeg downloads both had their SHA-256 checked against the release checksums |
| Desktop GUI (Windows) | Started with wgpu on a machine with only a software renderer. Ran `waydlp <url>` and checked by screenshot: the real thumbnail, metadata, available qualities and options rendered. |
| Windows per-user install | `waydlp-installer install --file …-portable.zip`: files, Start-menu shortcut, HKCU uninstall entry, install-kind marker, `verify` (including tamper detection), and a full `uninstall` that removes the running uninstaller's folder |
| Offline `setup.exe` | Bundled with `waydlp-installer bundle`. It launches and detects its embedded payload. |
| Linux packages (Debian 12 container) | `build-linux.sh` produced the `.deb`, `.rpm` and `.tar.gz`. Linux tests passed. `desktop-file-validate` passed. The `.deb` installed (`waydlp-cli` ran, install-kind `deb`) and removed cleanly. `rpm -qpil` listed the expected files. |
| Linux user install | As a normal user: `waydlp-installer install --file …tar.gz` installed into `~/.local/opt/waydlp`, created the `~/.local/bin` link and desktop entry, `verify` reported the files intact, `uninstall` removed everything, and `detect` listed the right methods |
| Lint | `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -D warnings` |
| Release CI, 2026-11-08 | 130 tests passed on Windows and Linux; strict linting, audits, generated-media downloads/conversion and package checks passed |
| Native x64/ARM64 installers | Windows and Linux CLI install → verify (including tampering) → uninstall cycles and installer GUI rendering passed |
| MSI | Windows x64 installed and uninstalled in CI; Windows ARM64 MSI built on a native ARM64 runner |
| AppImage | x64 and ARM64 built from checksum-verified release archives; both packaged GUIs started under Xvfb |
| Arch | Prebuilt package built, installed, CLI version checked, and uninstalled in an Arch container |
| Flatpak | Bundle built successfully with embedded FFmpeg and yt-dlp |
| Published release | 0.1.0 published with 25 assets. All GitHub server digests match the complete checksum manifest; downloaded Windows portable archive hash and CLI versions independently checked |

## Not yet verified

These are implemented but haven't been run in this environment:

* **Clicking through the GUI:** the actual mouse clicks (Download button → Queue → History) weren't automated. The underlying engine paths are covered by the tests above.
* **ARM64 MSI installation** on a physical Windows device.
* **Flatpak runtime downloads** inside its sandbox; bundle compilation was verified.
* **pkexec/sudo delegation** for system packages (.deb/.rpm through the installer).
* Physical-device coverage beyond the automated x64/ARM64 CI runners.

## Known limitations

* Binaries aren't code-signed (SmartScreen warnings on Windows).
* The GPG signature of yt-dlp's checksum file isn't verified; the SHA-256 is.
* The download queue isn't persisted across restarts (history is). Interrupted downloads are marked in history and can be downloaded again.
* No system-tray icon or download scheduling yet.
* Managed FFmpeg downloads are Windows-only by design; Linux uses the distribution's FFmpeg.
* The UI is English-only. Strings are centralised so translations can be added.
* GitHub releases 0.1.0 and 0.2.0 are published. AUR and Flathub submissions remain prepared but unpublished.
