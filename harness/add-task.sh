#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_ROOT"

usage() {
    cat <<EOF
Usage: $0 --title "Task title" --spec "specs/NNN-name.md" [options]

Required:
  --title     Task title
  --spec      Spec file reference

Options:
  --depends   Comma-separated dependency IDs
  --tests     Comma-separated test descriptions
  --checks    Comma-separated manual check descriptions
  --help      Show this help
EOF
    exit 1
}

TITLE=""
SPEC=""
DEPENDS=""
TESTS=""
CHECKS=""

while [ $# -gt 0 ]; do
    case "$1" in
        --title) TITLE="$2"; shift 2 ;;
        --spec) SPEC="$2"; shift 2 ;;
        --depends) DEPENDS="$2"; shift 2 ;;
        --tests) TESTS="$2"; shift 2 ;;
        --checks) CHECKS="$2"; shift 2 ;;
        --help) usage ;;
        *) echo "Unknown option: $1"; usage ;;
    esac
done

if [ -z "$TITLE" ] || [ -z "$SPEC" ]; then
    echo "ERROR: --title and --spec are required"
    usage
fi

if [ ! -f "$SPEC" ]; then
    echo "WARNING: spec file '$SPEC' does not exist yet"
fi

TITLE="$TITLE" SPEC="$SPEC" DEPENDS="$DEPENDS" TESTS="$TESTS" CHECKS="$CHECKS" python3 - <<'PY'
import json, os, sys, datetime

with open('state/tasks.json') as f:
    data = json.load(f)
tasks = data.get('tasks', [])

if tasks:
    max_num = max(int(t['id'].split('-')[1]) for t in tasks)
    prefix = tasks[0]['id'].split('-')[0]
else:
    max_num = 0
    prefix = 'MDREVIEW'
next_id = f'{prefix}-{max_num + 1}'
next_priority = max((t['priority'] for t in tasks), default=0) + 1

depends = [d.strip() for d in os.environ.get('DEPENDS', '').split(',') if d.strip()]
ids = {t['id'] for t in tasks}
for dep in depends:
    if dep not in ids:
        print(f'ERROR: dependency {dep} not found', file=sys.stderr)
        sys.exit(1)

def split_list(name):
    return [x.strip() for x in os.environ.get(name, '').split(',') if x.strip()]

task = {
    'id': next_id,
    'title': os.environ['TITLE'],
    'spec': os.environ['SPEC'],
    'status': 'pending',
    'priority': next_priority,
    'depends_on': depends,
    'verification': {
        'tests': split_list('TESTS'),
        'manual_checks': split_list('CHECKS'),
    },
    'completed_at': None,
}
tasks.append(task)
data['tasks'] = tasks
with open('state/tasks.json', 'w') as f:
    json.dump(data, f, indent=2)
    f.write('\n')

print(f"Added: [{next_id}] {task['title']}")
print(f"  Priority: {next_priority}")
print(f"  Spec: {task['spec']}")
if depends:
    print(f"  Depends on: {', '.join(depends)}")
PY
