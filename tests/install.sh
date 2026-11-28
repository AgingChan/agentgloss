#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

case "$(uname -s)-$(uname -m)" in
    Linux-x86_64) asset=agentgloss-x86_64-unknown-linux-musl.tar.gz ;;
    Darwin-arm64) asset=agentgloss-aarch64-apple-darwin.tar.gz ;;
    *) echo "skipping installer test on this platform"; exit 0 ;;
esac

sha256() {
    if command -v sha256sum >/dev/null 2>&1; then sha256sum "$@"; else shasum -a 256 "$@"; fi
}

make_release() {
    dir="$tmp/releases/$1"
    mkdir -p "$dir"
    printf '#!/bin/sh\necho agloss-test\n' > "$tmp/agloss"
    chmod +x "$tmp/agloss"
    tar -czf "$dir/$asset" -C "$tmp" agloss
    (cd "$dir" && sha256 "$asset" > SHA256SUMS)
}

make_legacy_release() {
    dir="$tmp/releases/$1"
    mkdir -p "$dir"
    printf '#!/bin/sh\necho agentgloss-legacy\n' > "$tmp/agentgloss"
    chmod +x "$tmp/agentgloss"
    tar -czf "$dir/$asset" -C "$tmp" agentgloss
    (cd "$dir" && sha256 "$asset" > SHA256SUMS)
}

make_release "latest/download"
make_release "download/v0.0.0-test"

AGENTGLOSS_RELEASE_BASE="file://$tmp/releases" sh "$root/install.sh" --version v0.0.0-test --dir "$tmp/pinned"
test -x "$tmp/pinned/agloss"
test -L "$tmp/pinned/agentgloss"
test "$("$tmp/pinned/agloss")" = "agloss-test"
test "$("$tmp/pinned/agentgloss")" = "agloss-test"

AGENTGLOSS_RELEASE_BASE="file://$tmp/releases" sh "$root/install.sh" --dir "$tmp/latest"
test -x "$tmp/latest/agloss"
test -L "$tmp/latest/agentgloss"

make_legacy_release "download/v0.0.0-legacy"
AGENTGLOSS_RELEASE_BASE="file://$tmp/releases" sh "$root/install.sh" --version v0.0.0-legacy --dir "$tmp/legacy"
test -x "$tmp/legacy/agentgloss"
test -L "$tmp/legacy/agloss"
test "$("$tmp/legacy/agentgloss")" = "agentgloss-legacy"
test "$("$tmp/legacy/agloss")" = "agentgloss-legacy"

make_release "download/v0.0.0-bad"
(cd "$tmp/releases/download/v0.0.0-bad" && printf '0000000000000000000000000000000000000000000000000000000000000000  %s\n' "$asset" > SHA256SUMS)
if AGENTGLOSS_RELEASE_BASE="file://$tmp/releases" sh "$root/install.sh" --version v0.0.0-bad --dir "$tmp/bad"; then
    echo "expected checksum failure" >&2
    exit 1
fi
test ! -e "$tmp/bad/agloss"
test ! -e "$tmp/bad/agentgloss"

echo "installer tests passed"
