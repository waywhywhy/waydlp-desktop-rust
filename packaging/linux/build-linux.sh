#!/usr/bin/env bash
# Build Linux release artifacts for waydlp into dist/:
#   waydlp-<ver>-linux-<arch>.tar.gz   portable archive
#   waydlp-<ver>-linux-<arch>.deb      (cargo-deb)
#   waydlp-<ver>-linux-<arch>.rpm      (cargo-generate-rpm)
#   waydlp-<ver>-linux-<arch>.AppImage (appimagetool)
# Requirements: cargo, cargo-deb, cargo-generate-rpm, appimagetool (for AppImage).
# Usage: packaging/linux/build-linux.sh <version> [x64|arm64]
set -euo pipefail
VERSION="${1:?usage: build-linux.sh <version> [x64|arm64]}"
ARCH="${2:-x64}"
case "$ARCH" in
  x64) TRIPLE=x86_64-unknown-linux-gnu; DEB_ARCH=amd64; RPM_ARCH=x86_64; AI_ARCH=x86_64 ;;
  arm64) TRIPLE=aarch64-unknown-linux-gnu; DEB_ARCH=arm64; RPM_ARCH=aarch64; AI_ARCH=aarch64 ;;
  *) echo "unknown arch $ARCH" >&2; exit 2 ;;
esac
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
DIST="$ROOT/dist"; mkdir -p "$DIST"
BIN="$ROOT/target/$TRIPLE/release"
NAME="waydlp-$VERSION-linux-$ARCH"

echo "==> cargo build ($TRIPLE)"
cargo build --release --locked --target "$TRIPLE" -p waydlp-gui -p waydlp-cli -p waydlp-installer-cli -p waydlp-installer-gui

echo "==> tar.gz"
STAGE="$(mktemp -d)"; trap 'rm -rf "$STAGE"' EXIT
mkdir -p "$STAGE/$NAME/share/applications" "$STAGE/$NAME/share/metainfo" "$STAGE/$NAME/share/icons"
install -m755 "$BIN/waydlp" "$BIN/waydlp-cli" "$BIN/waydlp-installer" "$BIN/waydlp-setup" "$STAGE/$NAME/"
cp -r assets/icons/hicolor "$STAGE/$NAME/share/icons/"
install -m644 packaging/linux/io.github.waywhywhy.waydlp.desktop "$STAGE/$NAME/share/applications/"
install -m644 packaging/linux/io.github.waywhywhy.waydlp.metainfo.xml "$STAGE/$NAME/share/metainfo/"
install -m644 LICENSE "$STAGE/$NAME/LICENSE"
install -m644 packaging/linux/README-linux.txt "$STAGE/$NAME/README.txt"
touch "$STAGE/$NAME/waydlp.portable"
tar -C "$STAGE" -czf "$DIST/$NAME.tar.gz" "$NAME"

echo "==> deb"
cargo deb --no-build --no-strip -p waydlp-gui --target "$TRIPLE" --deb-version "$VERSION" -o "$DIST/$NAME.deb"

echo "==> rpm"
# RPM separates version from release and forbids '-' in Version. A tilde
# preserves prerelease ordering (0.1.0~rc.1 sorts before 0.1.0).
RPM_VERSION="$VERSION"
if [[ "$VERSION" == *-* ]]; then
  PRERELEASE="${VERSION#*-}"
  RPM_VERSION="${VERSION%%-*}~${PRERELEASE//-/_}"
fi
cargo generate-rpm -p crates/waydlp-gui --target "$TRIPLE" --set-metadata "version = \"$RPM_VERSION\"" -o "$DIST/$NAME.rpm"

if command -v appimagetool >/dev/null; then
  echo "==> AppImage"
  "$ROOT/packaging/appimage/build-appimage.sh" "$BIN/waydlp" "$DIST/$NAME.AppImage" "$AI_ARCH"
else
  echo "appimagetool not found; refusing to omit a required release artifact" >&2
  exit 1
fi

( cd "$DIST" && sha256sum "$NAME".* >> SHA256SUMS.txt )
echo "==> done: $(ls "$DIST")"
