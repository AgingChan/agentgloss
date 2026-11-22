#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$PROJECT_ROOT"
source scripts/env.sh

echo "=== agentgloss init ==="
echo "OS: $(uname -s)"
echo "Project: $PROJECT_ROOT"
echo "Rust: $(rustc --version)"
echo "Cargo: $(cargo --version)"

mkdir -p state harness/logs

echo ""
echo "--- State files ---"
for f in state/tasks.json state/progress.md state/fix_plan.md; do
    if [ -f "$f" ]; then
        echo "  OK: $f"
    else
        echo "  MISSING: $f"
        exit 1
    fi
done

python3 -c "import json; json.load(open('state/tasks.json'))" 2>/dev/null \
    || { echo "ERROR: state/tasks.json is not valid JSON"; exit 1; }
echo "  OK: tasks.json is valid JSON"

echo ""
echo "--- Task summary ---"
python3 - <<'PY'
import json
with open('state/tasks.json') as f:
    data = json.load(f)
tasks = data.get('tasks', [])
done = sum(1 for t in tasks if t['status'] == 'done')
in_progress = sum(1 for t in tasks if t['status'] == 'in_progress')
pending = sum(1 for t in tasks if t['status'] == 'pending')
blocked = sum(1 for t in tasks if t['status'] == 'blocked')
print(f"  Total: {len(tasks)}")
print(f"  Done: {done} | In Progress: {in_progress} | Pending: {pending} | Blocked: {blocked}")
nxt = next((t for t in tasks if t['status'] in ('in_progress', 'pending')), None)
if nxt:
    print(f"  Next: [{nxt['id']}] {nxt['title']}")
PY

if find src -name '*.rs' 2>/dev/null | grep -q .; then
    echo ""
    echo "--- cargo check ---"
    cargo check
fi

echo ""
echo "=== Init complete ==="
