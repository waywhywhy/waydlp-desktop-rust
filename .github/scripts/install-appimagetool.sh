#!/usr/bin/env bash
# Install appimagetool (continuous build from the AppImage project).
set -euo pipefail
ARCH="$(uname -m)"
URL="https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-${ARCH}.AppImage"
curl -sSfL -o /tmp/appimagetool "$URL"
chmod +x /tmp/appimagetool
# Let the runtime resolve AppRun and its bundled libraries without FUSE.
sudo mkdir -p /opt/appimagetool
sudo install -m755 /tmp/appimagetool /opt/appimagetool/appimagetool.AppImage
cat > /tmp/run-appimagetool <<'RUN'
#!/usr/bin/env bash
exec /opt/appimagetool/appimagetool.AppImage --appimage-extract-and-run "$@"
RUN
sudo install -m755 /tmp/run-appimagetool /opt/appimagetool/run
sudo ln -sf /opt/appimagetool/run /usr/local/bin/appimagetool
appimagetool --version
