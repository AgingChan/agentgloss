#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
cd "$root"
list=$(cargo package --list --allow-dirty)
for path in .cargo/ state/ harness/ docs/ specs/ .github/ .claude/ .superpowers/ scripts/ init.sh vscode/ branding/; do
    if printf '%s\n' "$list" | grep -q "^$path"; then
        echo "unexpected in package: $path" >&2
        exit 1
    fi
done
for path in README.md LICENSE CHANGELOG.md Cargo.toml src/lib.rs src/main.rs; do
    printf '%s\n' "$list" | grep -qx "$path" || { echo "missing from package: $path" >&2; exit 1; }
done
echo "package contents ok"
