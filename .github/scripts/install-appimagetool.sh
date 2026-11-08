#!/usr/bin/env bash
# Install appimagetool (continuous build from the AppImage project).
set -euo pipefail
ARCH="$(uname -m)"
URL="https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-${ARCH}.AppImage"
curl -sSfL -o /tmp/appimagetool "$URL"
chmod +x /tmp/appimagetool
# Run without FUSE inside CI containers.
sudo mkdir -p /opt/appimagetool && cd /opt/appimagetool && sudo /tmp/appimagetool --appimage-extract >/dev/null
sudo ln -sf /opt/appimagetool/squashfs-root/AppRun /usr/local/bin/appimagetool
appimagetool --version || true
