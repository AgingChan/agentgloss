#!/usr/bin/env bash
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_ROOT"
source "$PROJECT_ROOT/scripts/env.sh" || { echo "GATE: cannot load build environment"; exit 1; }

ERRORS=0
WARNINGS=0

echo "=== agentgloss Inter-Session Gate ==="
echo ""

# 1. tasks.json integrity
echo -n "  tasks.json valid: "
if python3 - <<'PY' 2>&1
import json, os, sys
with open('state/tasks.json') as f:
    data = json.load(f)
tasks = data.get('tasks', [])
ids = [t['id'] for t in tasks]
if len(ids) != len(set(ids)):
    print('DUPLICATE IDS', file=sys.stderr); sys.exit(1)
priorities = sorted(t['priority'] for t in tasks)
if priorities != list(range(1, len(priorities) + 1)):
    print('GAP IN PRIORITIES', file=sys.stderr); sys.exit(1)
valid = {'pending', 'in_progress', 'done', 'blocked'}
for t in tasks:
    if t['status'] not in valid:
        print(f"BAD STATUS: {t['id']}", file=sys.stderr); sys.exit(1)
    if t['status'] == 'done' and not t.get('completed_at'):
        print(f"done WITH NO completed_at: {t['id']}", file=sys.stderr); sys.exit(1)
    if not os.path.exists(t['spec']):
        print(f"MISSING SPEC: {t['id']} -> {t['spec']}", file=sys.stderr); sys.exit(1)
    for dep in t.get('depends_on', []):
        if dep not in ids:
            print(f"INVALID DEP: {t['id']} depends on {dep}", file=sys.stderr); sys.exit(1)
print('OK')
PY
then
    true
else
    ERRORS=$((ERRORS + 1))
fi

# 2. stale in-progress tasks
echo -n "  stale in_progress: "
STALE=$(python3 - <<'PY' 2>/dev/null || echo error
import json
with open('state/tasks.json') as f:
    data = json.load(f)
stale = [t['id'] for t in data.get('tasks', []) if t['status'] == 'in_progress']
print(' '.join(stale) if stale else 'none')
PY
)
if [ "$STALE" = "none" ]; then
    echo "OK"
else
    echo "FOUND: $STALE"
    WARNINGS=$((WARNINGS + 1))
fi

# 3. git state
echo -n "  git state: "
if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    DIRTY=$(git status --porcelain 2>/dev/null | wc -l)
    if [ "$DIRTY" -eq 0 ]; then
        echo "clean"
    else
        echo "DIRTY ($DIRTY uncommitted changes)"
        WARNINGS=$((WARNINGS + 1))
    fi
else
    echo "not a git repo (warning)"
    WARNINGS=$((WARNINGS + 1))
fi

# 4. progress freshness (30 min)
echo -n "  progress.md fresh: "
if [ -f state/progress.md ]; then
    LAST_MOD=$(stat -c %Y state/progress.md 2>/dev/null || echo 0)
    NOW=$(date +%s)
    AGE=$((NOW - LAST_MOD))
    if [ "$AGE" -lt 1800 ]; then
        echo "OK (${AGE}s ago)"
    else
        echo "STALE (${AGE}s)"
        WARNINGS=$((WARNINGS + 1))
    fi
else
    echo "MISSING"
    WARNINGS=$((WARNINGS + 1))
fi

# 5. Shell syntax for the harness and build entry points
echo -n "  shell syntax: "
if bash -n init.sh scripts/env.sh harness/*.sh 2>/tmp/agentgloss-shell.log; then
    echo "OK"
else
    echo "FAIL"
    tail -10 /tmp/agentgloss-shell.log
    ERRORS=$((ERRORS + 1))
fi

# 6. Rust checks (only once source exists)
if find src -name '*.rs' 2>/dev/null | grep -q .; then
    echo -n "  placeholders: "
    PLACEHOLDERS=$(grep -RInE 'TODO|FIXME|unimplemented!|todo!' src --include='*.rs' 2>/dev/null || true)
    if [ -z "$PLACEHOLDERS" ]; then
        echo "none"
    else
        echo "FOUND"
        echo "$PLACEHOLDERS" | head -10
        ERRORS=$((ERRORS + 1))
    fi

    echo -n "  cargo fmt: "
    if cargo fmt --check >/tmp/agentgloss-fmt.log 2>&1; then
        echo "OK"
    else
        echo "FAIL"
        tail -10 /tmp/agentgloss-fmt.log
        ERRORS=$((ERRORS + 1))
    fi

    echo -n "  cargo clippy: "
    if cargo clippy --all-targets --all-features -- -D warnings >/tmp/agentgloss-clippy.log 2>&1; then
        echo "OK"
    else
        echo "FAIL"
        tail -20 /tmp/agentgloss-clippy.log
        ERRORS=$((ERRORS + 1))
    fi

    echo -n "  cargo test: "
    if cargo test --all-features >/tmp/agentgloss-test.log 2>&1; then
        echo "OK"
        grep -E '^test result:' /tmp/agentgloss-test.log | sed 's/^/    /'
    else
        echo "FAIL"
        tail -30 /tmp/agentgloss-test.log
        ERRORS=$((ERRORS + 1))
    fi
fi

echo ""
echo "  Result: $ERRORS error(s), $WARNINGS warning(s)"
if [ "$ERRORS" -gt 0 ]; then
    echo "  GATE: FAIL"
    exit 1
fi
echo "  GATE: PASS"
exit 0
