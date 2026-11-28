# 005 — Prompt and JSON export

## Prompt format

`agloss export <file>` prints a numbered, paste-ready review. Open and stale items are included by default; resolved items only with `--all`.

```markdown
# agentgloss review

- File: docs/specs/checkout.md
- Round: 2
- Document hash: 9f2c...
- Status: feedback
- Open items: 2 (1 comment, 1 suggestion)
- Stale items: 1

Address the items below. Answer every comment and accept or reject every suggestion.
After editing, mark fixed items resolved:
  agloss resolve docs/specs/checkout.md c1 c2

## 1. [c1] comment — open — lines 42-44
Heading: Architecture > Data flow
Quote:
> exact source text
Comment:
This conflicts with the retry section.
Anchor: bytes 1234-1300

## 2. [c2] suggestion — open — lines 88-88
Heading: API
Original:
old text
Replacement:
new text
Comment:
Prefer this wording.
Anchor: bytes 4000-4012
```

Approved output:

```markdown
# agentgloss review

- File: docs/specs/checkout.md
- Round: 2
- Document hash: 9f2c...
- Status: approved
- Open items: 0

The reviewer approved this revision. No changes are required.
```

Status is `feedback` when items need action, `approved` when the current hash is approved, and `pending` when no signoff exists.

Rules:
- Items are ordered by `anchor.source_start`, then ID.
- Numbering is sequential and independent of the stable ID.
- Quote lines are prefixed with `> `; blank lines inside quotes become `>`.
- Bodies and suggestions are printed verbatim, with no Markdown escaping.
- Suggestions may carry an optional comment body; the editor bridge/webview sets it, while the TUI suggestion editor is title-less and leaves it empty.
- `--all` includes resolved items, clearly labelled. If the revision is approved, archived items are listed under `Resolved items (archived):` without action instructions.
- Text never exceeds one blank line between sections.

## JSON export

`--format json` prints a stable object:

```json
{
  "file": "docs/specs/checkout.md",
  "round": 2,
  "doc_hash": "...",
  "status": "feedback",
  "open": 2,
  "stale": 1,
  "items": [ /* ReviewItem objects, same schema as the sidecar */ ]
}
```

Resolved items are excluded unless `--all`. The JSON is pretty-printed with two spaces.

## Exit codes

- 10 if any open or stale item is exported.
- 0 for a clean approval or an empty export.
- 2 for unreadable files or invalid sidecars.

## Tests

- Golden prompt for comments and suggestions.
- Quote escaping, blank lines, and a body containing Markdown fences.
- Ordering with anchors out of insertion order.
- Approved/empty output.
- JSON shape and filtering.
- Exit codes.
