#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
ci="$root/.github/workflows/ci.yml"
release="$root/.github/workflows/release.yml"
test -f "$ci" && test -f "$release" || { echo "workflow files missing" >&2; exit 1; }

grep -q 'cargo test --all-features' "$ci"
grep -q 'tests/install.sh' "$ci"
grep -q 'tests/package.sh' "$ci"
grep -q 'tests/workflows.sh' "$ci"
grep -q 'x86_64-unknown-linux-musl' "$release"
grep -q 'aarch64-apple-darwin' "$release"
grep -q 'macos-15' "$release"
grep -q 'SHA256SUMS' "$release"
grep -q 'sha256sum' "$release"
grep -q 'tags:' "$release"
grep -q 'workflow_dispatch' "$release"

if python3 -c 'import yaml' 2>/dev/null; then
    python3 -c 'import sys, yaml; [yaml.safe_load(open(p)) for p in sys.argv[1:]]' "$ci" "$release"
fi
echo "workflow checks passed"
