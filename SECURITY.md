# Security policy

## Supported versions

Security fixes are made for the latest released version of waydlp desktop.

## Reporting a vulnerability

**Please do not report security problems in public issues.**

Use GitHub's private vulnerability reporting: open the repository's
**Security** tab and choose **Report a vulnerability**
(<https://github.com/waywhywhy/waydlp-desktop-rust/security/advisories/new>).

Include what you found, how to reproduce it, the affected version and
platform, and the impact you expect. We aim to acknowledge reports within
7 days and to agree on a disclosure timeline with you.

## Scope

Examples of issues we want to hear about:

* command or argument injection into yt-dlp/FFmpeg,
* writing files outside the chosen download folder (path traversal),
* installer/updater integrity problems (unverified downloads, archive extraction escapes),
* leaking cookies, credentials or other secrets (e.g. into logs),
* unintended network requests or data collection.

Bugs in yt-dlp or FFmpeg themselves should be reported to those projects.
