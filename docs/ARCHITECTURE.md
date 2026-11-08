# waydlp desktop — architecture

This document explains how the desktop application is put together and why.

## Overview

```
┌──────────────────────────────┐   ┌──────────────────────────────┐
│ waydlp-gui  (eframe/egui)    │   │ waydlp-cli                   │
│ pages, theme, notifications  │   │ scriptable front-end         │
└──────────────┬───────────────┘   └──────────────┬───────────────┘
               │ commands / events                │
┌──────────────▼──────────────────────────────────▼───────────────┐
│ waydlp-engine (Tokio)                                           │
│  queue actor · yt-dlp runner · tool discovery · managed installs│
└──────────────┬──────────────────────────────────┬───────────────┘
               │ argv (no shell)                  │ HTTPS (rustls)
        ┌──────▼──────┐  ┌────────┐        ┌──────▼──────────────┐
        │   yt-dlp    │─▶│ FFmpeg │        │ GitHub Releases     │
        └─────────────┘  └────────┘        │ (yt-dlp, FFmpeg,    │
                                           │  waydlp updates)    │
┌──────────────────┐ ┌─────────────────┐   └─────────────────────┘
│ waydlp-config    │ │ waydlp-history  │
│ TOML + migration │ │ SQLite          │
└──────────────────┘ └─────────────────┘
┌──────────────────────────────────────────────────────────────────┐
│ waydlp-core: pure logic — URLs, metadata parsing, format analysis,│
│ options/presets, yt-dlp argument builder, progress & error parsing│
└──────────────────────────────────────────────────────────────────┘
```

| Crate | Responsibility | I/O? |
|---|---|---|
| `waydlp-core` | URL validation, yt-dlp JSON parsing, format analysis, download options & presets, **argument generation**, progress-line parsing, error classification, filename/template safety, release-artifact naming | none |
| `waydlp-engine` | Spawning yt-dlp safely, probing, downloads with real progress, the download queue (concurrency, pause/resume/cancel/retry), tool discovery, verified installs of yt-dlp/FFmpeg, update checks | processes, network |
| `waydlp-config` | Versioned settings (TOML) with migrations, validation, import/export | files |
| `waydlp-history` | Download history in SQLite | SQLite |
| `waydlp-platform` | App directories, portable mode, install-kind detection, open/reveal files, logging | OS |
| `waydlp-theme` | The AMOLED-black/purple egui theme shared by the app and the installer | — |
| `waydlp-gui` | The desktop application (`waydlp` binary) | — |
| `waydlp-cli` | Command-line front-end (`waydlp-cli`) | — |
| `waydlp-installer-core` | Shared install/uninstall/verify logic | files, network, package managers |
| `waydlp-installer-cli` | `waydlp-installer` | — |
| `waydlp-installer-gui` | `waydlp-setup` graphical installer (also the Windows `setup.exe`) | — |

## Key decisions

### GUI framework: egui/eframe

Evaluated Slint, Iced and egui:

* **egui/eframe** — chosen. Mature, actively maintained, cross-platform (Windows, X11, Wayland), first-class **AccessKit** screen-reader support, full control over styling (needed for the AMOLED/purple identity), small binaries, fast compile/iteration, and an immediate-mode model that maps naturally onto a UI driven by live download state.
* **Slint** — attractive declarative UI and good accessibility, but its own DSL adds a second language and its royalty-free license has conditions; theming beyond the built-in styles requires more custom components.
* **Iced** — elegant Elm architecture, but accessibility support lags behind and styling custom widgets is more verbose.

**Rendering:** waydlp starts with the **wgpu** renderer (Direct3D 12 / Vulkan / OpenGL ES). wgpu also works on machines without modern OpenGL drivers, such as VMs, RDP sessions and the "Microsoft Basic Render Driver". If wgpu can't start, waydlp retries with OpenGL (glow). This was found in testing: the OpenGL-only build refused to start on a machine with only OpenGL 1.1.

egui repaints only when needed (input, `request_repaint` from background tasks, or while a download is active at ~4 Hz), so idle CPU usage stays near zero.

### yt-dlp integration

* yt-dlp runs as a **separate process** with an explicit argument vector — never a shell. User input (URLs, templates, extra arguments) is validated; the URL always follows `--`.
* Progress uses our own `--progress-template` / `--print` formats (`[waydlp:dl]…`, `[waydlp:pp]…`, `[waydlp:file]…`) instead of scraping human-readable output, so parsing is stable across yt-dlp versions. **Every number shown in the UI comes from these lines.**
* Probing uses `--dump-single-json --flat-playlist`, so even huge channels are listed quickly without downloading anything; the UI only offers resolutions, codecs and subtitle languages that appear in the real format list.
* `--ignore-config` is passed by default so a user's own yt-dlp config cannot silently change waydlp's behaviour (opt-in setting to respect it).
* Extra arguments are split with shell-word rules but **options that execute commands** (`--exec`, `--netrc-cmd`, …) are rejected, as are options waydlp must control for progress parsing.

### Remux vs transcode

The argument builder prefers streams that fit the chosen container (`--format-sort ext:mp4:m4a`, codec preferences) and uses `--merge-output-format` + `--remux-video`, which copy streams without quality loss. Re-encoding (`--recode-video`) only happens when the user enables “Re-encode if the streams don't fit”. Audio conversion only happens when a target format is chosen; “Original” keeps the source stream. Bitrate options are labelled as encoder targets.

### The queue

`waydlp-engine::queue` is a single Tokio actor that owns all jobs. The UI talks to it through a cheap `QueueHandle` and receives `QueueEvent`s via a callback (which wakes the egui loop). Each job gets its own temporary directory (`--paths temp:`):

* **Pause** stops the process tree and keeps the directory; **resume** restarts yt-dlp, which continues the `.part` files.
* **Cancel / remove** stops the process and deletes the directory.
* **Completion** deletes it after yt-dlp has moved the final files.

Stopping kills the whole process tree (`taskkill /T` on Windows; a dedicated process group + `SIGTERM`/`SIGKILL` on Unix), so FFmpeg children never linger. Transient failures (network, timeouts, HTTP 429) are retried automatically with back-off; permanent ones (private, removed, DRM, unsupported) are not.

### Tool management

Tools are discovered in this order: user-configured path → waydlp-managed copy → copy bundled next to the executable → system `PATH`. Managed copies are downloaded from the official GitHub releases (yt-dlp, and yt-dlp's FFmpeg builds on Windows) and their **SHA-256 is verified** against the checksum file of the same release before use. System and package-managed copies are never modified. On Linux, FFmpeg should come from the distribution.

*Limitation:* the GPG signature of yt-dlp's checksum file is not verified yet; trust is anchored in GitHub's TLS certificate.

### Settings and history

Settings are TOML with a `schema_version`; `Settings::from_toml` migrates old files step by step, ignores unknown fields and fills missing ones with defaults. A corrupt file is backed up (`settings.toml.bak`) and defaults are used — the app always starts. Saving is atomic (write + rename). History lives in SQLite (WAL mode); removing an entry never touches files, deleting files is a separate, confirmed action.

### Portable mode

If a file named `waydlp.portable` is next to the executable, all data (settings, history, logs, managed tools, window state) is stored in a `data` folder beside it and nothing is written elsewhere.

### Updates

waydlp never auto-updates and never touches package-managed files. It can *check* GitHub Releases (manually, or at start-up if the user opts in) and then shows advice appropriate for the detected install kind (installer, MSI, portable, AppImage, Flatpak, deb, rpm, Arch, source).

## Privacy & security summary

* No telemetry, accounts or ads. Network requests happen only for: the media the user asked for (via yt-dlp), thumbnails (can be disabled), and GitHub when the user installs/updates tools or checks for updates.
* All HTTPS uses rustls with certificate validation.
* No shell is ever used to run yt-dlp/FFmpeg; output templates cannot contain absolute paths or `..`; archive extraction rejects path traversal.
* Diagnostic output is redacted (cookie/authorization headers, credentials in proxy URLs).
* Cookies are only used from a file the user explicitly selects; browsers' cookie stores are never read.
