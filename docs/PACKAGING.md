# Packaging and releases

All artifacts follow `waydlp-<version>-<os>-<arch>[-variant].<ext>` (implemented in
`waydlp_core::artifacts`, also used by the installers and the website):

| Artifact | Built by |
|---|---|
| `…-windows-x64-setup.exe` | `packaging/windows/build-release.ps1` — the GUI installer (`waydlp-setup.exe`) with the portable zip appended (`waydlp-installer bundle`) |
| `…-windows-x64.msi` | `packaging/windows/waydlp.wxs` with WiX Toolset v5 |
| `…-windows-x64-portable.zip` | `build-release.ps1` (contains `waydlp.portable`) |
| `…-linux-x64.tar.gz` | `packaging/linux/build-linux.sh` |
| `…-linux-x64.deb` | `cargo deb` using `[package.metadata.deb]` in `crates/waydlp-gui/Cargo.toml` |
| `…-linux-x64.rpm` | `cargo generate-rpm` using `[package.metadata.generate-rpm]` |
| `…-linux-x64.AppImage` | `packaging/appimage/build-appimage.sh` (appimagetool) |
| `…-linux-x64.flatpak` | `packaging/flatpak/io.github.waywhywhy.waydlp.yml` (flatpak-builder) |
| `…-linux-x64.pkg.tar.zst` | `packaging/arch/waydlp-bin/PKGBUILD` (makepkg) |
| `SHA256SUMS.txt` | release workflow |

## Installers on GitHub Actions

`.github/workflows/installers.yml` builds and tests both installers on every push to `main`, on pull requests, by hand (**Actions » Installers » Run workflow**), and from the release workflow:

| File | What it is |
|---|---|
| `waydlp-installer-<ver>-windows-<arch>.exe` | CLI installer (standalone; downloads the release and verifies its checksum, or `--file` for a local artifact) |
| `waydlp-setup-<ver>-windows-<arch>.exe` | GUI installer wizard (standalone, online) |
| `waydlp-<ver>-windows-<arch>-setup.exe` | GUI installer with the portable build embedded (offline) |
| `waydlp-installer-<ver>-linux-<arch>` | CLI installer |
| `waydlp-setup-<ver>-linux-<arch>` | GUI installer wizard |

Each job runs a real test on its runner. The CLI installer installs from a freshly built archive, then `verify` (including a tampered-file check), `uninstall`, and finally a check that nothing is left. The GUI installer runs `--self-test`: it opens the real window, renders the wizard for a few frames and exits. On Linux this happens under Xvfb with Mesa software rendering. Windows arm64 uses the `windows-11-arm` runner and Linux arm64 uses `ubuntu-22.04-arm`; both are available to public repositories.

Builds from a branch are named `<ver>-dev.<commit>` and are downloadable from the run's **Artifacts** section. Tagged releases attach them to the release.

## Release process

1. Update `version` in the root `Cargo.toml`, `CHANGELOG.md`, the AppStream `<releases>` and the PKGBUILDs.
2. Commit, then tag: `git tag v1.2.3 && git push --tags`.
3. `.github/workflows/release.yml` runs formatting, strict linting, tests, audits, real-engine download checks and package installation checks. It then builds every artifact for x64 and arm64 (where available), merges checksums and publishes the release.
4. Any required check or build failure blocks publication. The YouTube network check is advisory because GitHub runner IPs may be rate-limited; the generated-media real-engine check is required. Versions with a `-suffix` are marked as pre-releases.

## Windows

```powershell
dotnet tool install --global wix --version 5.0.2
pwsh packaging/windows/build-release.ps1 -Version 1.2.3 -Arch x64
```

* The **setup.exe** installs per-user into `%LOCALAPPDATA%\Programs\waydlp`, creates Start menu (and optional desktop) shortcuts, registers in *Apps & features* (HKCU) and copies itself as the uninstaller. No administrator rights.
* The **MSI** installs per-machine into `Program Files\waydlp` (Start menu shortcut, major-upgrade support).
* The binaries are **not code-signed** yet, so SmartScreen may warn on first run. Signing requires a certificate and is planned.

## Linux

```bash
cargo install cargo-deb cargo-generate-rpm
packaging/linux/build-linux.sh 1.2.3 x64
```

Packages install `/usr/bin/waydlp`, `/usr/bin/waydlp-cli`, the desktop entry, AppStream metadata, icons and `/usr/lib/waydlp/install-kind` (so waydlp knows not to self-update). `ffmpeg` and `yt-dlp` are *recommended* dependencies: Debian's packaged yt-dlp is often outdated, and waydlp can manage its own verified copy.

Build releases on the **oldest supported glibc** (the workflow uses Ubuntu 22.04) for broad compatibility.

### Flatpak / Flathub

```bash
python3 flatpak-cargo-generator.py Cargo.lock -o packaging/flatpak/cargo-sources.json
flatpak-builder --user --install --force-clean build-dir packaging/flatpak/io.github.waywhywhy.waydlp.yml
```

The manifest builds FFmpeg from source and pins a yt-dlp release (checksums included). Permissions: network, Wayland/X11, DRI, notifications and `xdg-download`; other folders go through the file-chooser portal.
To submit to Flathub, open a PR at <https://github.com/flathub/flathub> following their submission guide — **not done yet**. The app id `io.github.waywhywhy.waydlp` can be verified through the GitHub organisation.

### Arch Linux / AUR

Three PKGBUILDs are provided: `waydlp` (build from the tagged source), `waydlp-bin` (repackage the release tarball) and `waydlp-git` (latest `main`).

```bash
cd packaging/arch/waydlp && updpkgsums && makepkg -si
makepkg --printsrcinfo > .SRCINFO
```

Publishing to the AUR requires an AUR account with an SSH key (`git clone ssh://aur@aur.archlinux.org/waydlp.git`, copy `PKGBUILD` + `.SRCINFO`, commit, push). **Not published yet.**
