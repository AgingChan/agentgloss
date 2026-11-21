#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
LOG_DIR="$SCRIPT_DIR/logs"

cd "$PROJECT_ROOT"
source scripts/env.sh

echo "╔══════════════════════════════════════════╗"
echo "║         mdreview — Project Report        ║"
echo "╠══════════════════════════════════════════╣"
echo ""

echo "┌─ Tasks ─────────────────────────────────┐"
python3 - <<'PY'
import json
with open('state/tasks.json') as f:
    data = json.load(f)
tasks = data.get('tasks', [])
done = [t for t in tasks if t['status'] == 'done']
pending = [t for t in tasks if t['status'] == 'pending']
blocked = [t for t in tasks if t['status'] == 'blocked']
in_progress = [t for t in tasks if t['status'] == 'in_progress']
total = len(tasks)
pct = int(len(done) / total * 100) if total else 0
bar_len = 30
filled = int(bar_len * pct / 100)
bar = '█' * filled + '░' * (bar_len - filled)
print(f'│  [{bar}] {pct}%')
print(f'│  Done: {len(done)} | Pending: {len(pending)} | Blocked: {len(blocked)} | In Progress: {len(in_progress)}')
available = [t for t in pending if all(any(d['id'] == dep and d['status'] == 'done' for d in tasks) for dep in t.get('depends_on', []))]
if available:
    nxt = available[0]
    print(f'│  Next: [{nxt["id"]}] {nxt["title"]}')
    print(f'│  Spec: {nxt["spec"]}')
PY
echo "└──────────────────────────────────────────┘"
echo ""

echo "┌─ Recent Commits ─────────────────────────┐"
git log --oneline -5 2>/dev/null | while read -r line; do echo "│  $line"; done
echo "└──────────────────────────────────────────┘"
echo ""

echo "┌─ Recent Sessions ────────────────────────┐"
if [ -d "$LOG_DIR" ] && ls "$LOG_DIR"/session-*.log >/dev/null 2>&1; then
    ls -t "$LOG_DIR"/session-*.log 2>/dev/null | head -5 | while read -r logfile; do
        echo "│  $(basename "$logfile")  ($(wc -c < "$logfile" | tr -d ' ')B)"
    done
else
    echo "│  (no session logs yet)"
fi
echo "└──────────────────────────────────────────┘"
echo ""

echo "┌─ Code Health ────────────────────────────┐"
if find src -name '*.rs' 2>/dev/null | grep -q .; then
    SRC_FILES=$(find src -name '*.rs' | wc -l)
    SRC_LINES=$(find src -name '*.rs' -exec cat {} + 2>/dev/null | wc -l)
    TEST_FILES=$(find tests -name '*.rs' 2>/dev/null | wc -l)
    echo "│  Source files: $SRC_FILES ($SRC_LINES lines)"
    echo "│  Integration test files: $TEST_FILES"
    echo -n "│  cargo fmt: "
    if cargo fmt --check >/dev/null 2>&1; then echo "✓ clean"; else echo "⚠ needed"; fi
    echo -n "│  cargo test: "
    if cargo test --all-features >/tmp/mdreview-report-test.log 2>&1; then
        echo "✓ pass"
    else
        echo "✗ fail"
    fi
else
    echo "│  (no src/ content yet)"
fi
echo "└──────────────────────────────────────────┘"
echo ""

echo "┌─ Fix Plan ───────────────────────────────┐"
if [ -f state/fix_plan.md ]; then
    ITEMS=$(grep -cE '^\s*[0-9]+\.|^\s*- \[' state/fix_plan.md || true)
    DONE_ITEMS=$(grep -cE '^\s*- \[x\]' state/fix_plan.md || true)
    echo "│  Items: ${ITEMS:-0} (done: ${DONE_ITEMS:-0})"
    echo "│  File: state/fix_plan.md"
else
    echo "│  (no fix plan)"
fi
echo "└──────────────────────────────────────────┘"
echo ""
echo "Report generated: $(date)"
