#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
readme="$root/README.md"
changelog="$root/CHANGELOG.md"
design="$root/docs/superpowers/specs/2026-11-20-agentgloss-design.md"

grep -Fq 'Formerly mdreview' "$readme"
grep -Fq 'former name `mdreview`' "$readme"
if grep -Fq 'collides with the existing Python `agentgloss`' "$readme"; then
    echo "README still claims the new name collides" >&2
    exit 1
fi
grep -Fq 'delete the old' "$readme"
grep -Fq 'delete the old' "$changelog"
grep -Fq 'Library/Application Support/agentgloss' "$readme"
grep -Fq 'Library/Application Support/agentgloss' "$root/specs/002-core-cli.md"
grep -Fq 'CARGO_REGISTRY_TOKEN' "$readme"
grep -Fq 'not an ancestor of' "$readme"
grep -Fq '128 library tests' "$design"
grep -Fq '191 Rust tests' "$design"
grep -Fq 'AGENTGLOSS_CELL_SIZE' "$readme"
grep -Fq 'agentgloss.path' "$readme"
grep -Fq 'Open Review' "$readme"
test -f "$root/vscode/package.json"
test -f "$root/vscode/README.md"
test -f "$root/branding/agentgloss-icon.svg"
test -f "$root/branding/agentgloss-icon-square.svg"
test -f "$root/branding/agentgloss-logo-light.svg"
test -f "$root/branding/agentgloss-logo-dark.svg"
test -f "$root/vscode/media/icon.png"
grep -Fq 'branding/agentgloss-logo' "$readme"
grep -Fq 'agentgloss.openReview' "$root/specs/011-vscode-extension.md"
grep -Fq 'agentgloss bridge' "$root/specs/010-vscode-bridge.md"
grep -Fq 'agentgloss.openReview' "$root/specs/011-vscode-extension.md"
if grep -Fq 'working name collides' "$design"; then
    echo "design doc still uses the old working-name wording" >&2
    exit 1
fi
grep -Fq 'former name' "$design"
echo "docs checks passed"
