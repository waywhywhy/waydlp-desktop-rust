# waydlp user guide

You don't need to know anything about yt-dlp or FFmpeg to use waydlp. This guide explains each screen.

## First start

waydlp needs two free helper programs:

* **yt-dlp** reads web pages and finds the video/audio. *Required.*
* **FFmpeg** joins separate video and audio streams, converts audio (e.g. to MP3) and adds thumbnails/subtitles. *Recommended.*

If either is missing, the sidebar shows it in red/yellow. Open **Settings → yt-dlp & FFmpeg** and press **Install**. waydlp downloads the official builds from GitHub and checks their fingerprints (SHA-256) before using them. On Linux, install FFmpeg from your package manager instead (the settings page shows the command).

## Downloading something

1. Copy a link (for example from your browser's address bar).
2. In waydlp, press **📋 Paste** (or just press **Ctrl+V** anywhere outside a text box).
3. Press **Analyze**. waydlp asks yt-dlp what is available — this takes a few seconds.
4. Choose what you want:
   * **Video** — picture and sound. Pick a **Quality** (only real ones are listed) and a **Container** (MP4 plays almost everywhere).
   * **Audio** — sound only. Pick a **Format** such as MP3. The bitrate is an *encoding target*: converting can't add quality that wasn't in the original.
   * **Video only** — no sound.
   * **Original file** — exactly what the site provides, with no conversion (works without FFmpeg).
5. Press **Download**. Progress is shown in the **Queue**.

**Quick download** on the Home page skips analysis and uses your *default preset* (Settings → Downloads). Links that look like playlists are always previewed first.

### Playlists and channels

When a link is a playlist, the Downloader lists its items. Tick the ones you want (Select all / none / invert), choose options, and press **Download N items**. Files go into a folder named after the playlist and are numbered (both can be turned off). waydlp asks for confirmation before downloading more than 50 items (adjustable).

If a link points to one video *inside* a playlist, waydlp analyses just that video; use the playlist link to get the whole list.

### Several links at once

On Home, click **Paste several links…**, paste one link per line and press **Add to queue**.

## The queue

Each download shows its real progress: downloaded size, total size (marked `~` when it's only an estimate), speed and time left. When a site splits video and sound, you'll see “Downloading video stream” then “Downloading audio stream”, followed by steps like “Merging video and audio”.

* **Pause** stops the download but keeps what was downloaded so far; **Resume** continues.
* **Cancel** stops it and deletes partial files.
* **Retry** starts a failed download again. Temporary problems (network, rate limits) are retried automatically.
* ⏶ / ⏷ change the order; the slider sets how many run at the same time.

If something fails, waydlp explains why in plain words (for example “The media is private” or “yt-dlp may be out of date”) and what you can do. **Technical details** shows yt-dlp's exact output, which you can copy into a bug report.

## History

Every download is listed with its status, site, format, date and size. You can search, filter, open the file or its folder, copy the link, download again, remove the entry (the file stays) or delete the files (asks first).

## Presets

A preset is a saved set of options. Built-in presets: *Best Video Quality*, *Best Audio Quality*, *MP4 1080p*, *MP4 720p*, *MP3* and *Original Format*. Create your own in the Downloader (**Save as preset**) and manage them in **Settings → Presets & backup**.

## File names

The **File name** field uses yt-dlp's template syntax. Examples:

| Template | Result |
|---|---|
| `%(title)s.%(ext)s` | `My video.mp4` |
| `%(uploader)s/%(title)s.%(ext)s` | `Channel/My video.mp4` |
| `%(upload_date>%Y-%m-%d)s %(title)s.%(ext)s` | `2026-05-01 My video.mp4` |

Templates can create sub-folders but cannot leave the download folder.

## Settings overview

* **General** — download folder, start page, notifications, open folder when done.
* **Downloads** — default preset, file-name template, simultaneous downloads, retries, speed limit, timeout, skip already-downloaded items, playlist options.
* **Appearance** — accent colour (default purple), interface scale, compact/comfortable layout, animations, reduced motion.
* **yt-dlp & FFmpeg** — status, install/update, custom locations, proxy, extra yt-dlp arguments, cookies file, debug logging.
* **Privacy** — history on/off and retention, clipboard watching (off by default), thumbnails, update checks (off by default), clearing caches.
* **Presets & backup** — manage presets, export/import settings, reset to defaults.

## Sites that need you to sign in

Some content requires an account. You can give waydlp a `cookies.txt` file (Netscape format, exported with a browser extension you trust) in **Settings → yt-dlp & FFmpeg → Cookies file**. waydlp never reads your browser's cookies by itself and never uploads them. Anyone with that file can use your account — keep it private and remove it when you're done. waydlp does not bypass DRM, paywalls or other access controls.

## Troubleshooting

| Problem | What to do |
|---|---|
| “yt-dlp may be out of date” / “Unable to extract” | Update yt-dlp in Settings, then retry. Sites change often. |
| “FFmpeg is required” | Install FFmpeg, or choose *Original file*. |
| “Sign-in required” / “Age-restricted” | Provide a cookies file (see above) — only for accounts you own. |
| “The site is rate-limiting requests” | Wait, then retry; reduce simultaneous downloads. |
| “Not available in your region” | The uploader restricted it. |
| “Permission denied” / “Invalid output location” | Choose a different download folder; check the file-name template. |
| Nothing happens on Linux when opening a file | Install `xdg-utils`. |
| Windows SmartScreen warning | The builds aren't code-signed yet; choose “More info → Run anyway” if you trust the download (verify its SHA-256 first). |

Logs are in the folder shown under **Settings → yt-dlp & FFmpeg → Open logs folder**.

## Portable mode

Put an empty file named `waydlp.portable` next to `waydlp.exe` (the portable zip already contains it). Everything is then stored in a `data` folder beside the program.
