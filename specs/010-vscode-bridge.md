# 010 — Editor bridge (hidden JSON CLI)

## Purpose

The bridge lets an external editor render and mutate a review without
reimplementing AgentGloss semantics. Rust remains the single owner of parsing,
anchors, re-anchoring, rounds, approval, locking, and persistence. The sidecar
schema stays `version = 1`; the bridge is additive and does not change any
existing command or output.

The bridge is exposed under a hidden `agloss bridge` subcommand. It is a
machine contract, not part of the interactive CLI; it is documented here and
tested like any other feature.

## Commands

```text
agloss bridge state <FILE> [--sidecar <PATH>] [--no-source]
agloss bridge apply <FILE> [--sidecar <PATH>]   # request JSON on stdin
```

Both print exactly one compact JSON object plus a newline to stdout;
diagnostics go to stderr. The process exits `0` when the envelope has
`"ok": true` and `2` when it has `"ok": false`. The bridge never uses the agent
handshake codes (10) or TUI sentinels (20/21/22). A signoff reports its logical
result as `terminal_exit_code` inside the JSON.

Each call is a short-lived process. There is no server or daemon.

## `state` — pure read

`state` opens no lock, writes nothing, and never recovers or renames a sidecar.
It loads the document and performs a strict sidecar load. Concurrent TUI
sessions and atomic saves mean a reader sees a consistent old or new file.

Success envelope:

```json
{
  "schema_version": 1,
  "ok": true,
  "agentgloss_version": "0.2.0",
  "file": "/abs/doc.md",
  "sidecar": "/abs/doc.md.review.json",
  "sidecar_exists": true,
  "doc_hash": "<lowercase sha256 hex>",
  "line_count": 120,
  "source": "---\ntitle: ...",
  "blocks": [
    {
      "index": 0,
      "kind": "heading",
      "level": 1,
      "start_line": 1,
      "end_line": 1,
      "heading_path": ["Title"],
      "marker": null
    }
  ],
  "review": { "...": "ReviewStore JSON, unmodified" },
  "status": "feedback",
  "doc_changed": false,
  "approval_valid": false,
  "counts": { "open": 2, "stale": 0, "resolved": 1, "total": 3 }
}
```

- `source` is omitted when `--no-source` is passed. `line_count` and block
  ranges always reflect the current on-disk document.
- Block `start_line`/`end_line` are 1-based inclusive. Code and Mermaid blocks
  exclude their fences, matching `Block.source`. `kind` is the snake_case block
  kind (`heading`, `paragraph`, `list_item`, `code_block`, `table`,
  `blockquote`, `rule`, `image`, `mermaid`, `html`, `footnote`).
- `status` follows `agloss status`: `invalidated` when an approval exists
  but is invalid, else `changed` when the document hash differs, else
  `approved` / `feedback` / `pending`.
- `state` does **not** re-anchor. `doc_changed: true` tells the client to run
  `sync` before trusting item ranges.

Missing sidecar returns success with `sidecar_exists: false`, `review: null`,
zero counts, `status: "pending"`, `doc_changed: false`, and
`approval_valid: false`. No sidecar is created.

Corrupt or unsupported sidecars return `ok: false`; the file is left untouched.

## `apply` — one operation per request

Request:

```json
{
  "schema_version": 1,
  "op": "comment",
  "expect_hash": "<sha256 hex>",
  "selection": { "start_line": 5, "end_line": 7 },
  "body": "Please clarify this.",
  "replacement": null,
  "id": null,
  "include_source": false
}
```

Ops and requirements:

| op | required fields | behavior |
| --- | --- | --- |
| `sync` | none (no `expect_hash`) | lock, load document, load-or-recover sidecar, `start_round_if_changed`, save only if state changed |
| `comment` | `expect_hash`, `selection`, non-empty `body` | anchor the selection and `add_comment` |
| `suggestion` | `expect_hash`, `selection`, non-empty `replacement` different from the anchor quote; optional `body` note | `add_suggestion(anchor, body, original, replacement)`; reject image blocks |
| `edit` | `expect_hash`, `id`, new `body` and/or `replacement` | update the matching item through store methods; `updated_at` refreshed |
| `resolve` / `unresolve` | `expect_hash`, `id` | idempotent (`changed: false` when already in that state); unknown id errors; unresolve of stale errors |
| `delete` | `expect_hash`, `id` | remove the item; unknown id errors |
| `reattach` | `expect_hash`, `id`, `selection` | build an anchor and reattach a stale item; refresh a suggestion's original |
| `signoff` | `expect_hash` | reconcile, then feedback if open+stale > 0, else approve |

All mutating ops require `expect_hash` equal to the current document hash; a
mismatch returns `document_changed` without touching the sidecar. They then
acquire the per-operation lock, load the store, call
`start_round_if_changed`, apply the operation, and save. Mutations create a
fresh store when the sidecar is absent and fail closed when it is corrupt.

Successful responses repeat the `state` envelope (`sync` always includes the
source so the client can render a changed document; other ops include it only
when `include_source`) plus:

```json
{
  "changed": true,
  "id": "c4",
  "reconciled": true,
  "recovered": false,
  "backup_path": null,
  "terminal_exit_code": null
}
```

`terminal_exit_code` is `0` for an approval signoff, `10` for feedback, and
`null` otherwise. `sync` with no sidecar returns empty state and writes
nothing; `sync` may recover a corrupt sidecar, save a fresh store, and report
`recovered: true` with `backup_path`. Unsupported versions are never backed up.

## Selection contract

Selections are 1-based inclusive line ranges:

- Require `1 <= start_line <= end_line <= line_count`; reject out-of-range
  values, never clamp.
- Map to bytes with `start = line_starts[start_line - 1]` and
  `end = line_starts.get(end_line).unwrap_or(source.len())`.
- Reject a selection whose source slice is whitespace-only after trimming;
  `Anchor::from_range` would otherwise expand an empty range to the whole
  block.
- CRLF is handled by the anchor constructor, which trims one trailing `\n`
  and then one `\r`. A final synthetic line caused by a trailing newline is
  rejected as an empty selection.
- The client may select across blocks; Rust records the first block's heading
  path and block index, as the TUI does.

## Errors

Error envelope:

```json
{
  "schema_version": 1,
  "ok": false,
  "error": {
    "code": "document_changed",
    "message": "the document changed since the requested state"
  }
}
```

Codes: `missing_document`, `corrupt_sidecar`,
`unsupported_sidecar_version`, `locked`, `document_changed`,
`invalid_selection`, `invalid_operation`, `invalid_id`, `io_error`,
`internal`. Structured fields (`backup_path`, `found`, `supported`) are added
where useful.

## Locking and lifecycle

- `state` is lock-free, so a webview can read while the TUI holds its
  session-wide lock.
- `sync` and mutations take the exclusive sidecar lock per call. While the TUI
  is open they fail with `locked`; the client keeps state usable and surfaces
  the conflict.
- `export` keeps its exclusive lock as handshake policy. Bridge mutations and
  export serialize; the client must not auto-retry a mutation after a process
  failure without first refreshing `state`.
- Re-anchoring, round increments, approval invalidation, and stale semantics
  are the existing review-model rules (spec 004). A signoff never resolves
  items.

## Tests

`tests/bridge.rs` covers every operation; `state` purity (no mtime/content
change); absent, corrupt, and unsupported sidecars; sync idempotence; stale
`expect_hash`; lock contention; line-range edge cases (CRLF, missing trailing
newline, synthetic final line, reversed/zero/out-of-range, whitespace-only);
signoff exit codes; source included/omitted; and the v2-shaped unsupported
payload leaving no backup.
