# Fix Plan — 2026-11-28 cleanup pass

This replaces the stale 2026-11-21 plan. The source backlog was
`state/qa_report.md` (P1/P2/P3), and every P1 and P3 item is now addressed.
The sidecar schema and agent workflow are unchanged.

## P1 — completed

| ID | Result | Evidence |
| --- | --- | --- |
| CLI-RESOLVE-1 | `resolve` loads the Markdown, reconciles, then resolves | `tests/cli.rs::resolve_validates_the_document` |
| CLI-EXPORT-1 | Export lock retained as handshake policy; spec updated | existing lock test + spec 004 |
| REV-VERSION-1 | Version probed before full deserialization | `unsupported_v2_shape_is_rejected_without_backup` |
| DOC-EMPTY-CELL-1 | Empty cells carry an empty inline + cell range | `empty_table_cells_have_inline_source_ranges` |
| DOC-LIST-1 | Separate paragraph/footnote blocks, list spacing, nesting indent, hanging wrap | document/layout tests |
| DOC-SANITIZE-1 | C0/C1/DEL sanitized at display/export boundaries | sanitize, layout, and export tests |
| MEDIA-GUTTER-1 | Media rectangles start after the gutter | `media_area_starts_after_the_gutter` |
| SUGG-DRAFT-1 | Drafts from an older revision discarded and saved | `drafts_from_an_older_revision_are_discarded` |
| SUGG-COMMENT-1 | TUI suggestions documented title-less; bridge/webview/export carry an optional note | specs 005/006 |
| TUI-CTRLBRACKET-1 | Standard headings use plain `[`/`]` | `standard_mode_uses_plain_brackets_for_headings` |
| TUI-HELP-1 | Responsive, scrollable help with Ctrl+S hint | `help_overlay_scrolls_and_resets`, `help_overlay_fits_and_scrolls_at_80x24` |
| TUI-CLOSE-1 | SIGHUP/SIGTERM handler, panic-hook restore, mouse cleanup | implemented; real-PTY close remains a manual release check |
| TUI-RELOAD-1 | Reload clears selection and recomputes search | `reload_clears_selection_and_recomputes_search` |
| AGENT-MARKERS-1 | Duplicate complete blocks collapse; orphan markers refuse without rewriting | `duplicate_complete_marker_blocks_are_collapsed`, `orphan_marker_blocks_are_refused_without_rewriting` |

## P3 — completed

- Removed unused `BlockParser::image_src`.
- Deduplicated overlap helpers; unified backup/temp timestamps; one `now()` in reattach.
- Media hashing moved to worker threads; cache keys include target cells and cell pixel size; generation counters discard stale results; Auto falls back to `mmdc` after any render/raster/encode failure.
- Empty-document message reachable; mouse capture cleaned up on terminal-init failure.

## Added beyond the original backlog

- Hidden JSON bridge (`bridge state` / `bridge apply`) with 11 integration tests.
- VS Code extension (`vscode/`) with 17 tests, bundled Mermaid v11, and VSIX packaging.
- CI Node job, Cargo/package exclusions, and refreshed docs/state.

## Remaining — P2 test-matrix and manual checks

The broad P2 sweep (clap matrix, config precedence, full standard keymap,
popup transcripts, performance threshold alignment, etc.) is unchanged and
still worth scheduling. The real-PTY SIGHUP close was verified on 2026-11-28.
Before the next release, run the remaining manual check in
`state/acceptance.md`: a real VS Code webview selection → signoff flow.
