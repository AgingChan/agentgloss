#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
LOG_DIR="$SCRIPT_DIR/logs"

cd "$PROJECT_ROOT"
source scripts/env.sh
mkdir -p "$LOG_DIR"

if ! command -v claude >/dev/null 2>&1; then
    echo "ERROR: claude CLI not found in PATH"
    exit 1
fi

TASK_ID="${1:-}"
QA_LOG="$LOG_DIR/qa-$(date +%Y%m%d-%H%M%S).log"
TASK_FILTER=""

if [ -n "$TASK_ID" ]; then
    TASK_FILTER=$(python3 - "$TASK_ID" <<'PY'
import json, sys
task_id = sys.argv[1]
with open('state/tasks.json') as f:
    data = json.load(f)
task = next((t for t in data['tasks'] if t['id'] == task_id), None)
if not task:
    print(f'ERROR: task {task_id} not found', file=sys.stderr)
    sys.exit(1)
if task['status'] != 'done':
    print(f"WARNING: task {task_id} is {task['status']}, not done", file=sys.stderr)
print(f"Focus ONLY on task {task['id']}: {task['title']} (spec: {task['spec']})")
PY
)
    echo "QA scope: $TASK_ID"
else
    echo "QA scope: all completed tasks"
fi

echo "Log: $QA_LOG"
echo ""

{
    printf '# mdreview QA Evaluator Pass\n\n'
    printf '%s\n\n' "${TASK_FILTER:-Evaluate ALL recently completed tasks.}"
    cat <<'PROMPT'
You are a skeptical QA evaluator for mdreview, a Rust terminal Markdown spec reviewer.

## Your Job
1. Read `state/tasks.json` for completed tasks.
2. Read the relevant specs from `specs/` and the design doc.
3. For each completed feature, verify:
   - Source code implements what the spec describes.
   - Tests exist and pass (`source scripts/env.sh && cargo test --all-features`).
   - Static analysis passes (`cargo clippy --all-targets --all-features -- -D warnings`).
   - Formatting passes (`cargo fmt --check`).
   - No placeholder/stub implementations (`todo!`, `unimplemented!`, TODO, FIXME).
4. Grade each feature on functionality, completeness, code quality, edge cases, and error handling.

## Output Format
For each feature tested:
```
## [TASK-ID] Title
Status: PASS | FAIL | PARTIAL
Score: X/5

### Checks
- [ ] or [x] for each verification item

### Issues Found
- Description, severity (critical | major | minor), suggested fix

### Notes
Observations for future work.
```

Write actionable findings to `state/fix_plan.md`.

## Rules
- Be skeptical. Do not approve work that "looks close enough."
- Grep for TODO/FIXME/stub patterns; these are automatic failures.
- Verify tests assert meaningful behavior, not just that code does not crash.
- A spec requirement with no corresponding implementation is a FAIL.
PROMPT
} | claude --print 2>&1 | tee "$QA_LOG"

echo ""
echo "--- QA pass completed at $(date) ---"
echo "Results in: $QA_LOG"
echo "Action items in: state/fix_plan.md"
