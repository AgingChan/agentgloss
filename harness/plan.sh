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

PLAN_LOG="$LOG_DIR/plan-$(date +%Y%m%d-%H%M%S).log"
echo "=== agentgloss Plan Regeneration ==="
echo "Log: $PLAN_LOG"
echo ""

cat <<'PROMPT' | claude --print 2>&1 | tee "$PLAN_LOG"
# agentgloss Plan Regeneration

Regenerate `state/fix_plan.md` by comparing the specs against the current implementation.

## Steps
1. Read every file in `specs/` and `docs/superpowers/specs/2026-11-20-agentgloss-design.md`.
2. Read the source in `src/`.
3. Read `state/tasks.json` to see what is marked done versus pending.
4. Compare: identify gaps, incomplete implementations, missing tests, deviations from spec.
5. Overwrite `state/fix_plan.md` with:
   - Critical gaps that block the next task
   - Major quality gaps
   - Minor polish items
   - Spec contradictions or ambiguities

## Rules
- Be thorough. Check every spec against every relevant source file.
- Do not just list tasks.json items; find what tasks.json missed.
- Search for `todo!`, `unimplemented!`, TODO, FIXME, dead code, unused imports, missing error handling, and untested paths.
- Output only the fix plan content; no preamble.
PROMPT

echo ""
echo "--- Plan regeneration completed at $(date) ---"
echo "Results in: state/fix_plan.md"
echo "Log: $PLAN_LOG"
