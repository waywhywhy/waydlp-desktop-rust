#!/usr/bin/env bash
# Build an AppImage from a compiled waydlp binary.
# Usage: build-appimage.sh <path/to/waydlp> <output.AppImage> <x86_64|aarch64>
# Requires appimagetool (https://github.com/AppImage/appimagetool) on PATH.
set -euo pipefail
BIN="${1:?binary}"; OUT="${2:?output}"; ARCH="${3:-x86_64}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
APPDIR="$(mktemp -d)/waydlp.AppDir"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" "$APPDIR/usr/share/metainfo" "$APPDIR/usr/share/icons"
install -m755 "$BIN" "$APPDIR/usr/bin/waydlp"
cp -r "$ROOT/assets/icons/hicolor" "$APPDIR/usr/share/icons/"
install -m644 "$ROOT/packaging/linux/io.github.waywhywhy.waydlp.desktop" "$APPDIR/usr/share/applications/"
install -m644 "$ROOT/packaging/linux/io.github.waywhywhy.waydlp.metainfo.xml" "$APPDIR/usr/share/metainfo/io.github.waywhywhy.waydlp.appdata.xml"
# Top-level desktop file and icon required by the AppImage spec.
cp "$APPDIR/usr/share/applications/io.github.waywhywhy.waydlp.desktop" "$APPDIR/"
cp "$ROOT/assets/icons/hicolor/256x256/apps/io.github.waywhywhy.waydlp.png" "$APPDIR/io.github.waywhywhy.waydlp.png"
cp "$APPDIR/io.github.waywhywhy.waydlp.png" "$APPDIR/.DirIcon"
cat > "$APPDIR/AppRun" <<'RUN'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/waydlp" "$@"
RUN
chmod 755 "$APPDIR/AppRun"
# The binary links only against glibc and system graphics libraries (OpenGL,
# X11/Wayland via dlopen), which must come from the host, so nothing else is bundled.
ARCH="$ARCH" appimagetool --no-appstream "$APPDIR" "$OUT"
chmod 755 "$OUT"
