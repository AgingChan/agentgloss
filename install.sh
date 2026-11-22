#!/bin/sh
# Install agentgloss from GitHub Releases.
set -eu

version="latest"
install_dir="${AGENTGLOSS_INSTALL_DIR:-$HOME/.local/bin}"

usage() {
    cat <<'EOF'
Install agentgloss from GitHub Releases.

Usage: install.sh [--version <vX.Y.Z>] [--dir <path>]

  --version <vX.Y.Z>  Install a specific release (default: latest)
  --dir <path>        Install directory (default: $AGENTGLOSS_INSTALL_DIR or ~/.local/bin)
  -h, --help          Show this help
EOF
}

while [ $# -gt 0 ]; do
    case "$1" in
        --version)
            [ $# -ge 2 ] || { echo "install.sh: --version needs a value" >&2; exit 1; }
            version=$2
            shift 2
            ;;
        --dir)
            [ $# -ge 2 ] || { echo "install.sh: --dir needs a value" >&2; exit 1; }
            install_dir=$2
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "install.sh: unknown argument: $1" >&2
            usage >&2
            exit 1
            ;;
    esac
done

case "$(uname -s)" in
    Linux) os=Linux ;;
    Darwin) os=Darwin ;;
    *)
        echo "install.sh: unsupported operating system: $(uname -s)" >&2
        echo "Supported: Linux x86_64, macOS Apple Silicon" >&2
        exit 1
        ;;
esac

case "$(uname -m)" in
    x86_64|amd64) arch=x86_64 ;;
    arm64|aarch64) arch=arm64 ;;
    *)
        echo "install.sh: unsupported architecture: $(uname -m)" >&2
        echo "Supported: Linux x86_64, macOS Apple Silicon" >&2
        exit 1
        ;;
esac

case "$os-$arch" in
    Linux-x86_64) target=x86_64-unknown-linux-musl ;;
    Darwin-arm64) target=aarch64-apple-darwin ;;
    *)
        echo "install.sh: unsupported platform: $os $arch" >&2
        echo "Supported: Linux x86_64, macOS Apple Silicon" >&2
        exit 1
        ;;
esac

asset="agentgloss-$target.tar.gz"
base=${AGENTGLOSS_RELEASE_BASE:-https://github.com/AgingChan/agentgloss/releases}
base=${base%/}

case "$version" in
    latest) url="$base/latest/download" ;;
    v*) url="$base/download/$version" ;;
    *) url="$base/download/v$version" ;;
esac

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT HUP INT TERM

echo "Downloading $asset from $url..."
curl -fsSL "$url/$asset" -o "$tmp/$asset"
curl -fsSL "$url/SHA256SUMS" -o "$tmp/SHA256SUMS"

expected=$(awk -v asset="$asset" '$2 == asset { print $1 }' "$tmp/SHA256SUMS")
if [ -z "$expected" ]; then
    echo "install.sh: no checksum for $asset in SHA256SUMS" >&2
    exit 1
fi

if command -v sha256sum >/dev/null 2>&1; then
    actual=$(sha256sum "$tmp/$asset" | awk '{ print $1 }')
else
    actual=$(shasum -a 256 "$tmp/$asset" | awk '{ print $1 }')
fi

if [ "$expected" != "$actual" ]; then
    echo "install.sh: checksum mismatch for $asset" >&2
    echo "  expected: $expected" >&2
    echo "  actual:   $actual" >&2
    exit 1
fi

tar -xzf "$tmp/$asset" -C "$tmp"
if [ ! -f "$tmp/agentgloss" ]; then
    echo "install.sh: archive did not contain agentgloss" >&2
    exit 1
fi

mkdir -p "$install_dir"
install -m 0755 "$tmp/agentgloss" "$install_dir/agentgloss"
echo "Installed agentgloss to $install_dir/agentgloss"

case ":$PATH:" in
    *":$install_dir:"*) ;;
    *) echo "Warning: $install_dir is not on PATH; add it to your shell profile." >&2 ;;
esac
