#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
ci="$root/.github/workflows/ci.yml"
release="$root/.github/workflows/release.yml"
test -f "$ci" && test -f "$release" || { echo "workflow files missing" >&2; exit 1; }

grep -q 'cargo test --all-features' "$ci"
grep -q 'tests/install.sh' "$ci"
grep -q 'tests/package.sh' "$ci"
grep -q 'tests/docs.sh' "$ci"
grep -q 'tests/workflows.sh' "$ci"
grep -q 'x86_64-unknown-linux-musl' "$release"
grep -q 'aarch64-apple-darwin' "$release"
grep -q 'macos-15' "$release"
grep -q 'SHA256SUMS' "$release"
grep -q 'sha256sum' "$release"
grep -q 'tags:' "$release"
grep -q 'workflow_dispatch' "$release"
grep -Fq "github.event_name == 'push'" "$release"
grep -q 'publish-crates' "$release"
grep -q 'CARGO_REGISTRY_TOKEN' "$release"
grep -q 'cargo publish --locked' "$release"
grep -q 'verify-tag' "$release"
grep -q 'merge-base --is-ancestor' "$release"
grep -q 'origin/main' "$release"
grep -q 'fetch-depth: 0' "$release"

grep -q 'vscode:' "$ci"
grep -q 'setup-node@v4' "$ci"
grep -q 'working-directory: vscode' "$ci"
grep -q 'npm ci' "$ci"
grep -q 'npm run check' "$ci"
grep -q 'npm test' "$ci"
grep -q 'npm run package' "$ci"
grep -q "upload-artifact" "$ci"

grep -q 'agentgloss-vscode' "$release"
grep -q 'npm run package' "$release"
grep -q 'sha256sum .*vsix' "$release"
grep -q 'vscode/package.json' "$release"

marketplace="$root/.github/workflows/publish-vscode.yml"
test -f "$marketplace" || { echo "missing $marketplace" >&2; exit 1; }
grep -q 'id-token: write' "$marketplace"
grep -q -- '--oidc' "$marketplace"
grep -q 'InnopiaPteLtd' "$marketplace"
grep -q 'publish-vscode.yml' "$marketplace"
if grep -Eqi 'VSCE_PAT|--pat|AZURE_CREDENTIAL' "$marketplace"; then
    echo "trusted-publishing workflow must not use PAT or Azure credentials" >&2
    exit 1
fi

if python3 -c 'import yaml' 2>/dev/null; then
    python3 -c 'import sys, yaml; [yaml.safe_load(open(p)) for p in sys.argv[1:]]' "$ci" "$release"
fi
echo "workflow checks passed"
