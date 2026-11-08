waydlp — portable edition
=========================

Run waydlp.exe. Nothing is installed and no system settings are changed:
settings, history and waydlp-managed copies of yt-dlp/FFmpeg are stored in
the "data" folder next to waydlp.exe (because of the waydlp.portable file).

Delete the folder to remove waydlp completely.

Dependencies
------------
waydlp needs yt-dlp (required) and FFmpeg (recommended, for merging,
conversion and embedding). If they are not found, open Settings ->
"yt-dlp & FFmpeg" and click Install: waydlp downloads the official builds
from GitHub and verifies their SHA-256 checksums.

Command line
------------
waydlp-cli.exe --help        scriptable downloads using the same settings
waydlp-installer.exe --help  install waydlp for the current user instead

License: GPL-3.0-or-later. Source: https://github.com/waywhywhy/waydlp-desktop-rust
Only download media you have the right to access.
