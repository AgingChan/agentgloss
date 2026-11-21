#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
LOG_DIR="$SCRIPT_DIR/logs"

cd "$PROJECT_ROOT"
source scripts/env.sh
mkdir -p "$LOG_DIR"

MAX_SESSIONS="${1:-0}"
STOP_ON_FAIL="${STOP_ON_FAIL:-false}"
COOLDOWN="${COOLDOWN:-5}"
SESSION_COUNT=0
INTERRUPTED=false

cleanup() {
    if [ "$INTERRUPTED" = true ]; then
        echo ""
        echo "=== Loop interrupted. $SESSION_COUNT session(s) completed. ==="
        echo "Logs: $LOG_DIR/"
    fi
    exit 0
}
trap cleanup SIGINT SIGTERM

log_file() {
    echo "$LOG_DIR/session-$(date +%Y%m%d-%H%M%S).log"
}

preflight() {
    local errors=0

    if [ ! -f state/tasks.json ]; then
        echo "PREFLIGHT FAIL: state/tasks.json missing"
        errors=$((errors + 1))
    elif ! python3 -c "import json; json.load(open('state/tasks.json'))" 2>/dev/null; then
        echo "PREFLIGHT FAIL: state/tasks.json is not valid JSON"
        errors=$((errors + 1))
    else
        local pending
        pending=$(python3 -c "
import json
with open('state/tasks.json') as f:
    data = json.load(f)
print(sum(1 for t in data.get('tasks', []) if t['status'] in ('pending', 'in_progress')))
" 2>/dev/null || echo 0)
        if [ "$pending" -eq 0 ]; then
            echo "PREFLIGHT FAIL: no pending or in-progress tasks"
            errors=$((errors + 1))
        fi
    fi

    if ! command -v cargo >/dev/null 2>&1; then
        echo "PREFLIGHT FAIL: cargo not in PATH"
        errors=$((errors + 1))
    fi

    if ! command -v claude >/dev/null 2>&1; then
        echo "PREFLIGHT FAIL: claude CLI not in PATH (required for autonomous sessions)"
        errors=$((errors + 1))
    fi

    if command -v git >/dev/null 2>&1 && git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
        local dirty
        dirty=$(git status --porcelain 2>/dev/null | wc -l)
        if [ "$dirty" -gt 0 ]; then
            echo "PREFLIGHT WARN: git working tree has $dirty uncommitted change(s)"
        fi
    fi

    return "$errors"
}

echo "=== mdreview Harness Loop ==="
echo "Project: $PROJECT_ROOT"
echo "Max sessions: $([ "$MAX_SESSIONS" -gt 0 ] && echo "$MAX_SESSIONS" || echo unlimited)"
echo "Stop on fail: $STOP_ON_FAIL"
echo "Cooldown: ${COOLDOWN}s"
echo "Logs: $LOG_DIR"
echo ""

echo "--- Pre-flight checks ---"
if ! preflight; then
    echo ""
    echo "Pre-flight failed. Fix the issues above and retry."
    exit 1
fi
echo "Pre-flight OK."
echo ""

while true; do
    if [ "$INTERRUPTED" = true ]; then
        break
    fi

    SESSION_COUNT=$((SESSION_COUNT + 1))
    SESSION_LOG="$(log_file)"

    echo "--- Session $SESSION_COUNT starting at $(date) ---"
    echo "    Log: $SESSION_LOG"

    if cat "$SCRIPT_DIR/PROMPT.md" | claude --print 2>&1 | tee "$SESSION_LOG"; then
        echo "--- Session $SESSION_COUNT completed at $(date) ---"
        if [ -x "$SCRIPT_DIR/gate.sh" ]; then
            echo "--- Running inter-session gate ---"
            if ! "$SCRIPT_DIR/gate.sh" >> "$SESSION_LOG" 2>&1; then
                echo "GATE FAILED: see $SESSION_LOG for details"
                if [ "$STOP_ON_FAIL" = "true" ]; then
                    echo "STOP_ON_FAIL is set, halting loop."
                    exit 1
                fi
            fi
        fi
    else
        echo "--- Session $SESSION_COUNT exited with error at $(date) ---"
        if [ "$STOP_ON_FAIL" = "true" ]; then
            echo "STOP_ON_FAIL is set, halting loop."
            exit 1
        fi
    fi

    if [ "$MAX_SESSIONS" -gt 0 ] && [ "$SESSION_COUNT" -ge "$MAX_SESSIONS" ]; then
        echo "=== Reached max sessions ($MAX_SESSIONS), stopping ==="
        break
    fi

    echo "Cooling down ${COOLDOWN}s before next session..."
    sleep "$COOLDOWN"
done

echo ""
echo "=== Loop finished: $SESSION_COUNT session(s) completed ==="
echo "Logs in: $LOG_DIR"
