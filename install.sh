#!/bin/sh
# Download a verified native installer, then perform a per-user installation.
set -eu

usage() {
    echo 'Usage: sh install.sh [--version VERSION] [--dir DIRECTORY] [--no-menu] [--desktop-shortcut]'
    echo 'Installs the latest stable waydlp for the current user, without sudo.'
}
version=latest
install_dir=
menu=yes
shortcut=no
while [ "$#" -gt 0 ]; do
    case "$1" in
        --help|-h) usage; exit 0 ;;
        --version|--dir)
            [ "$#" -ge 2 ] && [ -n "$2" ] || { usage >&2; exit 2; }
            case "$1" in --version) version=${2#v} ;; --dir) install_dir=$2 ;; esac
            shift 2 ;;
        --no-menu) menu=no; shift ;;
        --desktop-shortcut) shortcut=yes; shift ;;
        *) echo "Unknown option: $1" >&2; usage >&2; exit 2 ;;
    esac
done
[ "$(uname -s)" = Linux ] || { echo 'This installer requires Linux.' >&2; exit 1; }
case "$(uname -m)" in
    x86_64|amd64) arch=x64 ;;
    aarch64|arm64) arch=arm64 ;;
    *) echo 'Supported architectures: x64 and ARM64.' >&2; exit 1 ;;
esac
for tool in curl sha256sum mktemp awk; do
    command -v "$tool" >/dev/null 2>&1 || { echo "Missing dependency: $tool" >&2; exit 1; }
done
repo=https://github.com/waywhywhy/waydlp-desktop-rust
if [ "$version" = latest ]; then
    resolved=$(curl --fail --silent --show-error --location --proto '=https' --proto-redir '=https' \
        --output /dev/null --write-out '%{url_effective}' "$repo/releases/latest")
    case "$resolved" in "$repo"/releases/tag/v*) version=${resolved##*/v} ;;
        *) echo 'Could not resolve the latest stable release.' >&2; exit 1 ;;
    esac
fi
# Restrict release names before constructing URLs or paths.
printf '%s\n' "$version" | LC_ALL=C awk '
    /^[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.-]+)?$/ { ok=1 }
    END { exit !ok }' || { echo 'Invalid release version.' >&2; exit 2; }
temp_dir=$(mktemp -d)
trap 'rm -rf -- "$temp_dir"' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
asset=waydlp-installer-$version-linux-$arch
base=$repo/releases/download/v$version
fetch() {
    curl --fail --silent --show-error --location --retry 3 --connect-timeout 20 \
        --proto '=https' --proto-redir '=https' "$base/$1" --output "$temp_dir/$1"
}
echo "Downloading waydlp $version ($arch)..."
fetch SHA256SUMS.txt
fetch "$asset"
expected=$(awk -v name="$asset" '$2 == name { count++; hash=$1 } END { if (count == 1) print hash }' "$temp_dir/SHA256SUMS.txt")
[ "${#expected}" -eq 64 ] || { echo 'Missing or invalid installer checksum.' >&2; exit 1; }
printf '%s  %s\n' "$expected" "$temp_dir/$asset" | sha256sum --check --status || {
    echo 'Installer checksum mismatch; refusing to execute.' >&2; exit 1;
}
chmod 700 "$temp_dir/$asset"
set -- install --format tar --yes --version "$version"
[ -z "$install_dir" ] || set -- "$@" --dir "$install_dir"
[ "$menu" = yes ] || set -- "$@" --no-menu
[ "$shortcut" = no ] || set -- "$@" --desktop-shortcut
"$temp_dir/$asset" "$@"
install_dir=${install_dir:-"$HOME/.local/opt/waydlp"}
echo "Installed. Launch: $install_dir/waydlp"
echo "CLI: $install_dir/waydlp-cli"
echo "Verify: $install_dir/waydlp-installer verify"
echo "Uninstall: $install_dir/waydlp-installer uninstall"
