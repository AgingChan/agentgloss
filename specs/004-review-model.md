# 004 — Review model, anchors, rounds, and re-anchoring

## Sidecar schema (`version = 1`)

```json
{
  "version": 1,
  "file": "docs/specs/checkout.md",
  "doc_hash": "<sha256>",
  "round": 2,
  "requested_at": "2026-11-21T10:00:00Z",
  "last_signoff": "feedback",
  "approval": { "hash": "<sha256>", "at": "2026-11-21T10:30:00Z" },
  "next_id": 4,
  "items": [
    {
      "id": "c1",
      "kind": "comment",
      "status": "open",
      "body": "This conflicts with the retry section.",
      "created_round": 1,
      "resolved_round": null,
      "created_at": "2026-11-21T10:05:00Z",
      "updated_at": "2026-11-21T10:05:00Z",
      "anchor": {
        "heading_path": ["Architecture", "Data flow"],
        "block_index": 12,
        "quote": "exact source text",
        "prefix": "up to 32 chars before",
        "suffix": "up to 32 chars after",
        "source_start": 1234,
        "source_end": 1300,
        "line_start": 42,
        "line_end": 44,
        "doc_hash": "<sha256 at anchor time>"
      },
      "suggestion": null,
      "history": [
        { "round": 1, "method": "created", "old_start": 1234, "new_start": 1234, "at": "..." }
      ]
    }
  ]
}
```

- `last_signoff` is `feedback`, `approved`, or `null`.
- `requested_at` is optional; set by `mdreview request`.
- `approval` is valid only while its hash equals the current document hash and there are no open or stale items.
- IDs are `c<N>` and never reused. `next_id` is monotonic.
- Suggestions add `suggestion: { "original": "...", "replacement": "..." }`.

## Anchor creation

`Anchor::from_selection(doc, range)` stores:
- `quote` = exact source slice;
- `prefix`/`suffix` = up to 32 characters on each side;
- `heading_path` and `block_index` of the first block touched;
- byte range, 1-based line range, and current document hash.

A selection is line-based in the UI: the byte range is the union of selected source lines, trimmed only of the trailing newline.

## Re-anchoring

On load, if the current document hash differs from the item anchor hash, re-anchor each open item:

1. **Exact quote, unique:** adopt the new position.
2. **Exact quote, multiple:** score candidates by prefix/suffix equality, then by distance from the old byte offset; choose the best.
3. **Whitespace-normalized quote:** collapse runs of whitespace and search; map back to original byte offsets.
4. **Heading + block fallback:** if the old heading path still exists and the candidate block's token Jaccard similarity to the quote is at least 0.5, anchor to that block's range.
5. **Not found:** mark `stale`, keep the old anchor and body, and add a history event. If a later revision makes the quote findable again, re-anchoring reopens the item automatically; manual reattach handles anchors that no longer match.

Resolved items are never re-anchored. Every successful re-anchor appends a history event with round, method, and old/new offsets. An already-stale item does not repeat a stale event on every pass; re-anchoring or reattaching it records a new event. A suggestion becomes `stale` if its `original` text no longer matches the re-anchored text.

## Rounds and signoff

- A new review round starts when a signed-off review is reopened and the document hash changed. `round` increments once per changed revision.
- Signoff with zero open and zero stale items records approval for the current hash. Signoff with any open or stale items records `last_signoff = "feedback"` and no approval.
- Any content change after approval invalidates the approval (hash mismatch) and starts a new round on next open.
- Resolve sets `status = resolved`, `resolved_round = current round`, and appends history.
- Stale items can be reattached to a new selection; reattachment sets `status = open`, updates the anchor, and records the event.

## Storage guarantees

- Atomic writes via temp + rename.
- Exclusive lock while a TUI session or mutating command runs.
- Corrupt JSON backup and recovery as described in spec 002.
- Unknown `version` is rejected without overwriting the file.

## Tests

- Exact, moved, duplicate-quote, inserted-text, deleted-text, heading-move, and stale cases.
- Unicode quote matching and whitespace normalization.
- Suggestion staleness.
- Round increment and approval invalidation on content change.
- Resolve/unresolve and reattach.
- Sidecar round-trip, lock contention, corrupt backup, unknown version.
