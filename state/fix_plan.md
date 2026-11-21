# Fix Plan

QA evaluator pass: 2026-11-21
Scope: MDREVIEW-1..MDREVIEW-12 (marked done); MDREVIEW-13 is still in progress.
Evidence: 46 library + 8 CLI + 9 TUI tests pass (harness run); no `TODO`/`FIXME`/`todo!`/`unimplemented!` in `src/` or `tests/`; release binary reports `static-pie`.
Note: `cargo fmt`, `cargo clippy`, and `cargo test` could not be re-executed from this QA session (command approval denied). Findings are from a full source/test review plus the provided test run.

## Verdict

12 completed tasks: **2 FAIL** (MDREVIEW-11, MDREVIEW-12), **10 PARTIAL**. No stub markers, but several explicit spec requirements are missing and several test requirements are absent or too weak. The most serious defects are in the tmux handshake, the export lock/approval state, and stale-item reattachment.

## Critical (breaks an agent-facing contract)

- [ ] **C1 (MDREVIEW-11): tmux treats feedback/abort popup exits as a tmux failure.** `src/agent.rs:183` uses `status.success()`. `__tui` returns 10 on feedback and 1 on abort (spec 008). If tmux propagates the popup status (the spec's stated assumption), `run_tmux` bails with exit 2 and never prints the prompt. Accept 0/1/10 as valid review results; reserve exit 2 for spawn failures/unexpected statuses. Add fake-tmux tests that exit 10 and 1.
- [ ] **C2 (MDREVIEW-11): `review --tmux --sidecar <path>` reads the default sidecar.** `src/agent.rs:186` calls `print_review_result(&args.file)`, which uses `sidecar_path(file)` at `src/agent.rs:225`; the popup wrote the custom path. Pass the resolved sidecar path through and add a test.

## Major (spec requirement missing or behavior wrong)

- [ ] **M1 (MDREVIEW-2): `export` mutates state without the store lock.** `src/cli.rs:264-268` calls `start_round_if_changed` and `save` without `StoreLock`. Spec 002/004 require exclusive locking for mutating commands; this can race a live TUI and clobber state. Lock for the whole export, or compute the export without mutating; test lock contention.
- [ ] **M2 (MDREVIEW-2): `status` reports an approval invalidated by a file change.** `src/cli.rs:282-319` never compares `document.hash` to `store.doc_hash`; `is_approved()` uses the stale hash. Approve, edit the file, run `status`: it still says approved. Compare against the live document hash (report invalidated/pending, round will increment on reopen); test it.
- [ ] **M3 (MDREVIEW-4/12): stale items cannot be reattached.** `ReviewStore::reattach` (`src/review/mod.rs:378-395`) has no caller in `src/` or `tests/`; there is no TUI key or CLI command. Spec 004 and task 12 require reattachment to a new selection. Add a UI action (e.g. `r` on a stale review item) and/or an `mdreview reattach` command; test status, anchor, and history.
- [ ] **M4 (MDREVIEW-3): `Document::block_at` fallback returns block 0, not the nearest preceding block.** `src/document/mod.rs:169-174` falls back to the *first* block whose start is <= byte. Bytes in inter-block whitespace or after the last block resolve to block 0. This makes source-view blank/trailing lines belong to block 0 (`src/tui/app.rs:1395-1418`), so a comment created there anchors to the document title (`src/tui/app.rs:387-418`). Use `rfind` over `source.start <= byte`; test gaps and EOF.
- [ ] **M5 (MDREVIEW-6): no outline/review overlay below 80 columns.** `src/tui/render.rs:79-106` returns no panels, and `draw` renders no overlay, so Tab focus is invisible/useless. Spec 006 requires a full-screen overlay closed with `Esc`. Implement the overlay and a 60x24 TestBackend case.
- [ ] **M6 (MDREVIEW-8): review-gutter markers are not clickable, and panel hit-testing is off by one.** `src/tui/app.rs:1209-1244`: document clicks never inspect the 2-column gutter; outline/review rows are indexed from the outer rect (`area.y`) while content begins at `inner().y` (border row), so clicking the first item selects the second. Hit-test the gutter to open its item and subtract the border offset (`src/tui/render.rs:267,289`).
- [ ] **M7 (MDREVIEW-5): suggestion export is not verbatim.** `src/review/export.rs:92-105` sends `Original`/`Replacement` through `push_quote`, emitting `> old text`. Spec 005's golden output prints them raw and says suggestions are verbatim. Only the `Quote:` section should get `>` prefixes. Update the test to a golden string.
- [ ] **M8 (MDREVIEW-10): `mermaid_engine = "source"` and invalid diagrams do not show the fenced source.** `src/media.rs:249-267` returns an error; `src/document/layout.rs:72-81` always reserves placeholder rows; `src/tui/app.rs:340-354` only sets a status error. Spec 007 requires highlighted source, a "diagram unavailable" badge, and source mode without rendering. Render source rows for `Source`/failed renders (or auto-toggle that block); test invalid diagrams and the source engine.
- [ ] **M9 (MDREVIEW-9): image cache is not content-addressed.** `src/media.rs:125-137` keys on `src` + cell target + protocol + `mtime:len`; spec 007 requires SHA-256 of content/engine/version/theme/target dimensions. Hash the file content (and include version/theme) for the cache key. Add hit/miss, resize invalidation, and content-change tests.
- [ ] **M10 (MDREVIEW-11): `wait` can return a stale signoff after `request`.** `request()` only stamps `requested_at` (`src/review/mod.rs:286-288`); `wait` accepts any matching-hash signoff (`src/cli.rs:232-256`). Re-requesting review of an unchanged file returns the previous approval/feedback immediately. Clear `last_signoff`/`approval` on request, or require a signoff round/timestamp after the request; test.
- [ ] **M11 (MDREVIEW-3): tables do not store cells as inlines with source ranges.** `src/document/mod.rs:586-600,701-723` collects cell text into one synthetic inline covering the whole table. Spec 003 requires per-cell inlines with source ranges. Push one `Inline` per cell with its own range and test it. Also: `BlockKind::Html` is never constructed (`src/document/mod.rs:53`), and a Mermaid fence nested in a list stays a `ListItem` because the current builder is reused (`src/document/mod.rs:553-564`).
- [ ] **M12 (MDREVIEW-7): rendered-view selection is visual-line-based, not source-line-based.** `src/tui/app.rs:387-418` builds the anchor from visual line spans. Anchors then vary with terminal width and can quote a fragment of a wrapped source line, contrary to spec 004 ("union of selected source lines"). Map the selection to whole source lines, or amend the spec and test the intended behavior.
- [ ] **M13 (MDREVIEW-2): unknown config keys do not warn.** `src/config.rs:69-85` parses silently; spec 002 says unknown keys warn. Parse with `serde_ignored`/`toml::Value` and emit a warning; test stderr.
- [ ] **M14 (MDREVIEW-6): corrupt-sidecar recovery is not reported in the TUI status line.** `src/cli.rs:171-175` only writes stderr before the alternate screen starts; `src/tui/app.rs:198` always starts with "Ready". Pass recovery info into the app status; test.
- [ ] **M15 (MDREVIEW-3): tabs are not expanded and wrapping is not grapheme-aware.** `src/document/layout.rs:164-170` keeps `\t` with width 4 instead of four spaces; `src/document/layout.rs:195-231` splits at chars, not graphemes. `unicode-segmentation` is declared (`Cargo.toml:28`) but unused. Expand tabs and wrap by grapheme; add tab and emoji tests.

## Minor (polish, edge cases, code quality)

- [ ] `resolve`/`unresolve` do not append history entries (`src/review/mod.rs:348-367`), contrary to spec 004.
- [ ] Tiny-terminal threshold is `width < 20 || height < 6` (`src/tui/render.rs:16-19`); spec 006 says `height < 10`.
- [ ] Standard-mode `Shift+Home`/`Shift+End` jump to document start/end (`src/tui/app.rs:913-923`) instead of selecting the current line.
- [ ] `Enter` on a Mermaid block toggles the whole rendered/source view (`src/tui/app.rs:1008-1018`), not just that block.
- [ ] Source view replaces the 2-column comment gutter with line numbers, so comment markers disappear (`src/tui/render.rs:160-176`).
- [ ] Export header counts comments/suggestions from stale items too (`src/review/export.rs:18-27`), so "Open items: 0 (1 comment...)" is possible; count only open items.
- [ ] Item sort tiebreak is the string ID (`src/review/mod.rs:435`, `src/tui/app.rs:578`), so `c10` sorts before `c2`.
- [ ] Image dimension limits are checked after full decode (`src/media.rs:223-239`); oversized files can allocate before rejection. Check dimensions before decode and cover the 40 MP and 50 MB paths in tests.
- [ ] SVG raster scale has `.max(0.01)` (`src/media.rs:322-329`), which can exceed the 4096-pixel cap for extreme aspect ratios; SVG intrinsic size is not checked against the input limits.
- [ ] No status warning when no fonts are available for SVG text (spec 007).
- [ ] `mmdc` temp directory is leaked on failure (`src/media.rs:307`); `command_exists` does not check execute permission; scheme rejection is case-sensitive.
- [ ] `find_all` skips overlapping quote matches (`src/review/mod.rs:606-621`); normalized `context_score` uses the raw quote length (`src/review/mod.rs:747`).
- [ ] Backup filenames use second granularity and can overwrite earlier backups (`src/review/mod.rs:567-573`); temp paths use lossy `display()`.
- [ ] `TerminalSession::new` can leave raw mode enabled if `EnterAlternateScreen` fails (`src/tui/mod.rs:48-57`).
- [ ] Review-panel excerpts can contain `\n`/`\t` from multi-line quotes (`src/tui/render.rs:321`), which can corrupt one-line rendering.
- [ ] No signal handling for "external terminal close autosaves"; in-progress editor drafts are never autosaved (`src/tui/app.rs:793-865`, `src/tui/mod.rs:72-99`).
- [ ] `upsert_block` appends a duplicate block when only `START` or only `END` exists (`src/agent.rs:53-86`).
- [ ] `review --tmux` failure message does not include the fallback standalone command required by spec 008 (`src/agent.rs:184`).
- [ ] Dead API/deps: `BlockKind::Html` never constructed; `media_blocks`, `document_hash`, `media_error` unused; `cache_dir` unused; `unicode-segmentation` dependency unused.
- [ ] No git commits: every file is untracked (`git log` empty). `harness/PROMPT.md` step 7 requires a commit, and spec 001's "clean working tree" acceptance is unmet.
- [ ] `CHANGELOG.md` missing (spec 009); no 2,000-line performance test; `state/fix_plan.md` was stale ("None") until this pass.

## Missing tests required by the specs

- [ ] **001:** automated `bash -n` sweep; gate with valid and invalid task JSON; task-schema validation (currently gate only does `json.load`).
- [ ] **002:** clap parsing for every command/flag; config precedence (default < file < CLI) and the warning path; atomic replacement assertion; status after resolve; `--sidecar` + `--tmux`.
- [ ] **003:** empty input, CRLF, no trailing newline; wrapping at 20/40/80; emoji/grapheme case; task-list/rule/HTML/footnote/nested-list assertions; per-cell table ranges; nested Mermaid.
- [ ] **004:** heading-move, inserted/deleted text, Unicode quote matching, real block/Jaccard fallback (the current `whitespace_and_block_fallback` only exercises normalization), reattach, history assertions, stale approval via `status`.
- [ ] **005:** true golden prompt; quote escaping; blank lines; body containing Markdown fences; out-of-order anchors; empty export.
- [ ] **006:** 100x30 layout; width<80 overlay; full standard-mode keymap; outline/help dispatch; suggestion edit/delete; wheel; outline/review hit-testing; mouse-disabled mode; quit autosave; corrupt-sidecar recovery.
- [ ] **007:** absolute paths; JPEG/WebP/BMP/GIF; oversized pipeline (50 MB / 10k px / 40 MP); dimension and non-blank-pixel assertions for PNG/SVG; explicit `--protocol` override; cache hit/miss/invalidation/content key; sequence/class Mermaid; invalid-Mermaid source fallback; fake `mmdc`; assertion that no text is drawn inside a media rect.
- [ ] **008:** popup exit codes 10 and 1; custom sidecar path; Gemini recipe and existing `GEMINI.md`; content assertions for every recipe; idempotency for all four installs.
- [ ] **009:** 2,000-line parse/layout benchmark; acceptance matrix execution; README/CHANGELOG/release checklist.

## Spec issues / ambiguities

- Stale items: spec 004 says approval requires "no open items", but export treats stale as feedback (10). The implementation treats stale as blocking approval. Clarify which is intended.
- "Open items" line: should the comment/suggestion counts include stale items? The example implies open-only.
- Media cache key: architecture says "target size"; the pipeline says "target pixel dimensions"; implementation keys by terminal cells.
- tmux: document whether `display-popup -E` propagates the popup exit code; the adapter currently assumes it may not, but treats nonzero as failure.
- Standard mode `Ctrl+[` is indistinguishable from `Esc` in most terminals; the previous-heading binding may be unreachable.
- "height < 10" tiny-terminal rule vs. the implemented 20x6 minimum.

## Per-task grades

| Task | Status | Score | Blocking issues |
| --- | --- | --- | --- |
| MDREVIEW-1 Harness/skeleton | PARTIAL | 3/5 | no commits/dirty tree; harness tests missing |
| MDREVIEW-2 CLI/config/sidecar | PARTIAL | 3/5 | export lock, status approval, config warning, tmux sidecar |
| MDREVIEW-3 Document model | PARTIAL | 3/5 | block_at bug, table cells, tabs/graphemes, Html, nested Mermaid |
| MDREVIEW-4 Review model | PARTIAL | 3/5 | reattach unreachable, resolve history, missing tests |
| MDREVIEW-5 Export | PARTIAL | 2/5 | suggestion format not verbatim, counts, missing golden tests |
| MDREVIEW-6 TUI foundation | PARTIAL | 2/5 | no narrow overlay, height threshold, source markers |
| MDREVIEW-7 Comments/suggestions | PARTIAL | 3/5 | selection mapping, draft autosave, test gaps |
| MDREVIEW-8 Mouse | PARTIAL | 2/5 | gutter click missing, panel off-by-one, test gaps |
| MDREVIEW-9 Images | PARTIAL | 3/5 | cache key, limit enforcement, weak media tests |
| MDREVIEW-10 Mermaid | PARTIAL | 2/5 | source/invalid fallback, weak fixture assertions, no mmdc test |
| MDREVIEW-11 Agent/tmux/recipes | FAIL | 2/5 | tmux exit codes, custom sidecar, stale wait, missing recipe tests |
| MDREVIEW-12 Rounds/file changes/reattach | FAIL | 2/5 | reattach has no user path or test |

## MDREVIEW-13 remaining work (not graded)

- [ ] Fix all Critical/Major findings above; re-run `./harness/gate.sh` and a release build.
- [ ] Add `CHANGELOG.md` per spec 009.
- [ ] Add the 2,000-line performance test with a generous threshold.
- [ ] Produce the acceptance-matrix evidence and update this report after fixes.
- [ ] Confirm `harness/report.sh` shows 100% and commit the tree (currently no commits).
