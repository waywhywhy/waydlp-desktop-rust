#!/bin/sh
# No network: curl shim supplies an executable that must never run on bad hashes.
set -eu
root=$(pwd)
fixture=$(mktemp -d)
trap 'rm -rf -- "$fixture"' EXIT
mkdir "$fixture/bin"
export BOOTSTRAP_FIXTURE=$fixture
printf '#!/bin/sh\nprintf "%%s\\n" "$@" > "$BOOTSTRAP_FIXTURE/args"\ntouch "$BOOTSTRAP_FIXTURE/executed"\n' > "$fixture/payload"
cat > "$fixture/bin/curl" <<'EOF'
#!/bin/sh
set -eu
out=
url=
while [ "$#" -gt 0 ]; do
    case "$1" in
        --output) out=$2; shift 2 ;;
        https://*) url=$1; shift ;;
        *) shift ;;
    esac
done
case "$url" in
    */SHA256SUMS.txt) cp "$BOOTSTRAP_FIXTURE/manifest" "$out" ;;
    *) cp "$BOOTSTRAP_FIXTURE/payload" "$out" ;;
esac
EOF
chmod +x "$fixture/bin/curl"
printf '#!/bin/sh\ncase "$1" in -s) echo Linux ;; -m) echo x86_64 ;; esac\n' > "$fixture/bin/uname"
chmod +x "$fixture/bin/uname"
for checksum in corrupt missing duplicate; do
    asset=waydlp-installer-0.1.0-linux-x64
    hash=0000000000000000000000000000000000000000000000000000000000000000
    case "$checksum" in
        corrupt) printf '%s  %s\n' "$hash" "$asset" > "$fixture/manifest" ;;
        missing) printf '%s  wrong-file\n' "$hash" > "$fixture/manifest" ;;
        duplicate) printf '%s  %s\n%s  %s\n' "$hash" "$asset" "$hash" "$asset" > "$fixture/manifest" ;;
    esac
    if PATH="$fixture/bin:$PATH" sh "$root/install.sh" --version 0.1.0 > "$fixture/log" 2>&1; then
        echo "Installer accepted $checksum checksum" >&2; exit 1
    fi
    cat "$fixture/log"
    grep -i checksum "$fixture/log" >/dev/null
    test ! -e "$fixture/executed"
done
hash=$(sha256sum "$fixture/payload" | awk '{print $1}')
printf '%s  %s\r\n' "$hash" "$asset" > "$fixture/manifest"
PATH="$fixture/bin:$PATH" sh "$root/install.sh" --version 0.1.0 --dir "$fixture/path with spaces" --no-menu
test -f "$fixture/executed"
grep -Fx -- "$fixture/path with spaces" "$fixture/args" >/dev/null
grep -Fx -- --no-menu "$fixture/args" >/dev/null
echo 'Checksum rejection tests passed.'
