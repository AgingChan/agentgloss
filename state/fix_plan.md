# Fix Plan — QA Evaluator Pass

Date: 2026-11-21 (pass started after the 21:10 gate run).
Scope: MDREVIEW-1 through MDREVIEW-12 are `done`. MDREVIEW-13 is `in_progress` and is assessed separately for release readiness.

## Evidence used

- Harness gate captured at pass start (`harness/qa.sh` → `GATE_OUTPUT`): tasks JSON valid, shell syntax OK, no placeholders, `cargo fmt` OK, `cargo clippy --all-targets --all-features -- -D warnings` OK, `cargo test --all-features` OK. Gate result: **0 errors, 2 warnings** (stale `in_progress` MDREVIEW-13; dirty working tree). `GATE: PASS`.
- Raw gate logs read directly: `/tmp/mdreview-test.log` → **82 library + 15 CLI + 1 performance + 30 TUI = 128 passed, 0 failed, 0 ignored**; `/tmp/mdreview-fmt.log` empty (clean); `/tmp/mdreview-clippy.log` only `Finished` (no warnings).
- Static test count independently matches: 128 `#[test]` attributes (82 in `src/`, 45 in `tests/`, 1 perf).
- Placeholder scan over `src/` and `tests/`: no `todo!`, `unimplemented!`, `TODO`, `FIXME`, `XXX`, or `HACK`.
- Release artifact: `target/x86_64-unknown-linux-musl/release/mdreview` is `ELF 64-bit LSB pie executable, x86-64, static-pie linked`.
- HEAD is `7f36029` (20:28); **20 working-tree changes are uncommitted**.
- Not independently re-runnable in this session: `cargo`, `./init.sh`, and `./harness/gate.sh` were denied by the command-approval layer. The gate's own freshly written logs are used as evidence. `file`/grep/git reads were re-run directly.

## Verdict

12 completed tasks: **3 PASS, 9 PARTIAL, 0 FAIL**.

| Task | Status | Score | Main gap |
| --- | --- | --- | --- |
| MDREVIEW-1 Harness/skeleton | PARTIAL | 4/5 | No automated harness tests; `qa.sh` persists a CLI error as the QA report |
| MDREVIEW-2 CLI/config/sidecar | PARTIAL | 4/5 | Spec 002 test list not fully covered (clap matrix, config precedence, atomic replace) |
| MDREVIEW-3 Document model | PARTIAL | 4/5 | Empty table cells dropped; fixture/width-matrix coverage incomplete |
| MDREVIEW-4 Review model | PASS | 4/5 | Stale-repeat history deviation; no heading-path-gone test |
| MDREVIEW-5 Export | PARTIAL | 4/5 | Suggestion golden missing; `--all` feedback resolve command includes resolved IDs |
| MDREVIEW-6 TUI foundation | PARTIAL | 4/5 | External-close path exits 2, not 1; several spec tests missing |
| MDREVIEW-7 Comments/suggestions | PARTIAL | 4/5 | Suggestion edit/delete tests missing; suggestion original is not editable |
| MDREVIEW-8 Mouse | PASS | 5/5 | Nit: no click-outside-ignored test |
| MDREVIEW-9 Images | PARTIAL | 4/5 | PNG dimension/pixel assertion missing; placeholder lacks dimensions; UI-thread hashing |
| MDREVIEW-10 Mermaid | PARTIAL | 4/5 | Rendered dimensions/pixels unasserted; Auto `mmdc`-via-PATH untested |
| MDREVIEW-11 Agent/tmux/recipes | PARTIAL | 4/5 | Real tmux exit propagation unverified; recipe-content assertions incomplete |
| MDREVIEW-12 Round lifecycle | PASS | 5/5 | Nits: reload keeps stale selection; watcher/hash race |

No task-level FAIL remains: the previous blocking SUGGEST-1 (false-stale suggestions) is fixed and regression-tested, and HARNESS-1 (qa.sh heredoc corruption) is fixed. Release is blocked by MDREVIEW-13, not by an identified functional blocker in tasks 1–12.

## Release-blocking / highest-priority findings

### QA-1 (MAJOR, MDREVIEW-1 + 13): `qa.sh` records a failed model invocation as a successful QA report

- **Evidence:** `state/qa_report.md` contains only
  `[claude-code:unrecognized_model] {"model":"deepseek-flash[1m]","query_source":"sdk"}`.
  `harness/logs/qa-20261121-210548.log` and `qa-20261121-211007.log` are both 85 bytes and contain that same line.
- **Cause:** `harness/qa.sh:92` pipes `claude --print` output to `tee` and treats the pipeline as the QA result. The CLI can print an error and exit 0 (or the script's error status is not surfaced as a QA failure), so the persisted report is unusable while qa.sh reports completion.
- **Fix:** Validate the claude invocation before/after `tee`: require exit 0, non-empty output, and absence of an `unrecognized_model`/error prefix; write the report atomically and exit non-zero on failure. This is an ambient CLI/model-routing problem, but the harness must detect it rather than persist the diagnostic as a report.
- **Test:** Run qa.sh with a fake `claude` that exits 0 while printing an error line; assert qa.sh fails and does not overwrite the last good report.

### QA-2 (MAJOR, MDREVIEW-13): Release state is not shipped

- MDREVIEW-13 is `in_progress`; the gate counts it as a stale warning.
- `harness/report.sh` therefore reports 12/13 = 92%, not the required 100%.
- The 20 uncommitted files contain the QA fixes and are not in HEAD; the release binary postdates HEAD but the repository does not contain its source state.
- **Fix:** finish the QA items below, run the full release checklist, mark MDREVIEW-13 `done`, commit, and re-run `gate.sh` with a clean tree.

### QA-3 (MAJOR, MDREVIEW-11 + 13): Real tmux popup exit propagation is unverified and the acceptance doc overstates it

- The adapter depends on `tmux display-popup -E` propagating the `__tui` sentinel codes 20/21/22 (`src/agent.rs:200-233`). All tests use a fake `tmux`; `state/progress.md:19` says command construction with a fake executable, and the design doc only claims a fake-tmux test.
- `state/acceptance.md:33-35` says "a real popup was exercised in the PTY smoke test." The recorded evidence supports a real TUI pane and a fake-tmux popup, not a real popup. Either add and record a real tmux popup run (approved/feedback/abort codes) or correct the acceptance claim and document the tmux version/behavior assumption.
- **Robustness fix (recommended):** have `__tui` write its outcome (approved/feedback/aborted + sidecar path) to a result file, and have the adapter read that file instead of relying solely on popup exit-status propagation.

## [MDREVIEW-1] Harness, Rust skeleton, and static build

Status: PARTIAL
Score: 4/5

### Checks
- [x] `init.sh`, `scripts/env.sh`, `.cargo/config.toml`, and all harness scripts exist; `bash -n` passes in the gate.
- [x] `state/tasks.json` is valid, ordered, and dependency-consistent; `state/progress.md` is fresh.
- [x] `gate.sh` validates tasks, placeholders, fmt, clippy, and tests; it passed at pass start.
- [x] Release binary is static-pie; `--version` behavior is covered by `tests/cli.rs::version_and_help`.
- [x] `harness/PROMPT.md` defines the orient/setup/baseline/TDD/implement/state/exit protocol.
- [x] The previous HARNESS-1 heredoc/backtick bug is fixed (`harness/qa.sh:53` uses `cat <<'PROMPT'`).
- [ ] No automated tests implement spec 001's Test section: shell-syntax sweep, gate with deliberately invalid task JSON, and task-schema validation.
- [ ] `harness/templates/task-schema.json` is unused; `gate.sh:17-45` hand-rolls a weaker validation.
- [ ] `qa.sh` report persistence is broken (QA-1).
- [ ] `./init.sh` was not independently re-run (approval layer); its only non-build side effect is file/toolchain checks.

### Issues Found
- **HARNESS-2 (minor):** Add `tests/harness.sh` (or gate checks) for: `bash -n` over `init.sh scripts/env.sh harness/*.sh`; a temp-copy of `tasks.json` with invalid JSON that must make gate fail; schema validation against `harness/templates/task-schema.json` (or delete the unused schema and document the hand-rolled rules).
- **QA-1 (major):** see above.
- **HARNESS-3 (nit):** `gate.sh` does not check the static release binary or run `init.sh`, although spec 001/009 cite both. Add them to the release checklist or gate.

### Notes
The harness structure and gate logic are sound. The remaining work is test automation and report integrity, not skeleton functionality.

## [MDREVIEW-2] CLI, config, and sidecar storage

Status: PARTIAL
Score: 4/5

### Checks
- [x] All commands/flags in spec 002 exist; hidden `__tui` is wired and hidden from help.
- [x] Exit codes 0/10/1/2/3 and tmux sentinels 20/21/22 are mapped correctly.
- [x] Default sidecar, custom `--sidecar`, lock contention, atomic temp+sync+rename, corrupt backup, and unknown-version rejection are implemented.
- [x] Request/status/export/resolve flows work; export is read-only and locks; status detects changed/invalidated approvals.
- [x] Printed handoff commands now shell-quote paths (`src/cli.rs:263-270`, `src/agent.rs:275-285`).
- [x] CLI integration tests cover version/help, request→status→export→resolve, wait feedback/approval/timeout, missing sidecar, tmux errors, install, export locking, status invalidation, request clearing signoff, custom sidecar, round increment, two-round flow, export immutability, and human status.
- [ ] Spec 002 Tests are incomplete: no clap matrix for every command/flag; no config-precedence test (defaults < config < CLI); no stderr assertion for the unknown-key warning; no direct atomic-replacement assertion; no status-after-resolve assertion; no unknown-resolve-ID exit-2 test.
- [ ] `hash_file` (`src/cli.rs:421-425`) has no callers.
- [ ] `status` emits `changed`/`invalidated` statuses that specs 002/004/005 do not define (`src/cli.rs:340-346`).

### Issues Found
- **CLI-1 (minor):** Add the missing spec-002 tests. Config precedence is implemented (`src/cli.rs:210-216`) but unprotected.
- **CLI-2 (nit):** Remove `hash_file` or use it for the signoff/watch race check.
- **CLI-3 (nit):** Document `changed`/`invalidated` in spec 002 (the statuses are useful) or fold them into the spec's labels.
- **CLI-4 (nit):** Export takes an exclusive lock even though it is read-only; it will fail with exit 2 while a review is open. This is acceptable for the handshake but should be documented in README/spec 002.

### Notes
The sidecar lifecycle and locking are coherent. The main residual risk is untested configuration precedence and the clap surface.

## [MDREVIEW-3] Markdown document model and source map

Status: PARTIAL
Score: 4/5

### Checks
- [x] pulldown-cmark GFM options and `into_offset_iter()` are used.
- [x] Heading paths include the heading itself and inherit to following blocks.
- [x] Mermaid fences become `BlockKind::Mermaid` with content-only ranges; image-only paragraphs become `BlockKind::Image`; mixed images stay inline.
- [x] Nested lists/quotes contribute depth; tables are one block with per-cell inlines and ranges; rules, HTML, footnotes, task lists, emphasis, links, and Unicode are handled.
- [x] `line_of`, `lines_of`, `source_line`, and `block_at` round-trip; `block_at` returns the preceding block in gaps/EOF.
- [x] Layout wraps on display width, keeps visual-line source maps, expands tabs to four spaces, splits long grapheme tokens, and reserves 8/10 rows for media.
- [x] Tests cover empty/CRLF/no-trailing-newline, task/rule/footnote/nested-list, per-cell ranges, block lookup, CJK/emoji wrapping, tabs, and remote-image link layout.
- [ ] Empty table cells are silently dropped: `Builder::push_inline` returns early on empty text (`src/document/mod.rs:271-275`), so an empty cell has no `Inline`/source range (spec 003:50).
- [ ] `tests/fixtures/spec.md` does not cover the spec 003 acceptance list (nested/task lists, blockquote, links, rule, HTML, footnote, emphasis, Unicode/wide characters).
- [ ] Wrapping is tested at 12/6/4 columns, not the spec's 20/40/80 matrix.

### Issues Found
- **DOC-1 (minor):** Push an empty inline with the cell's range for empty table cells, or explicitly exempt empty cells in spec 003.
- **DOC-2 (minor):** Expand `tests/fixtures/spec.md` (or add a second comprehensive fixture) so the acceptance list is exercised end-to-end; add exact 20/40/80-column wrapping assertions.
- **DOC-3 (nit):** A paragraph containing multiple images keeps only the last image in `Builder.image`; define the behavior or use the first.

### Notes
The parser is materially stronger than the previous pass. The remaining items are edge cases and test-fixture completeness.

## [MDREVIEW-4] Review model, anchors, rounds, and re-anchoring

Status: PASS
Score: 4/5

### Checks
- [x] Sidecar schema version 1, stable `cN` IDs, monotonic `next_id`, timestamps, optional signoff/approval/request/draft fields.
- [x] Anchor creation stores quote, 32-character prefix/suffix, heading path, block index, byte/line ranges, and document hash; trailing CR/LF is trimmed.
- [x] Re-anchoring implements exact-unique, exact-multiple (context then distance), whitespace-normalized, and heading/Jaccard ≥ 0.5 fallback, then stale.
- [x] Resolved items are skipped; history events are appended for reanchors, resolve/unresolve, reattach, and stale transitions.
- [x] Round increments only when a signed-off revision changes; approval is bound to the document hash and cleared on change/feedback.
- [x] Suggestions become stale when their original text no longer matches; a re-found stale item reopens automatically; reattach refreshes a suggestion's original.
- [x] Tests cover exact/moved, duplicate quotes, whitespace, inserted/deleted text, heading moves, Unicode, Jaccard, stale, suggestion staleness, round/approval, resolve/unresolve/remove, sidecar, corrupt backup, version rejection, lock contention, and reattach.
- [ ] A still-unfindable stale item does not append a new history event (`src/review/mod.rs:532-546`), while spec 004 says every re-anchor appends one.
- [ ] No explicit test for the Jaccard path being rejected when the old heading path no longer exists.

### Issues Found
- **REV-1 (minor):** Either append a `still_stale` event each round or amend spec 004 to say stale repeats are deduplicated.
- **REV-2 (nit):** Add a test where the heading path is gone and the block text is similar enough for Jaccard but must not match; assert stale.

### Notes
The core multi-round contract is implemented and now regression-covered. No functional defect found.

## [MDREVIEW-5] Prompt and JSON export

Status: PARTIAL
Score: 4/5

### Checks
- [x] Header includes file, round, hash, status, and open/stale counts; open counts are split by kind.
- [x] Items are ordered by `source_start` then numeric ID; numbering is sequential and independent of stable IDs.
- [x] Quotes are `> `-prefixed with blank lines as `>`; bodies and suggestion text are verbatim, including fenced code.
- [x] Approved output prints the approval line; `--all` on approval lists `Resolved items (archived):` with no action instructions.
- [x] Empty and stale-only exports are clear; JSON shape/filtering and exit codes 10/0/2 are implemented.
- [x] Resolve paths in the prompt are shell-quoted.
- [x] Tests cover the comment golden, approved output, approved `--all`, quote blanks/fences, ordering, empty/stale-only, JSON filtering/exit codes, and path quoting.
- [ ] Spec 005 says "Golden prompt for comments and suggestions"; only the comment prompt is a full golden. The suggestion test asserts substrings (`src/review/export.rs:365-377`).
- [ ] With `--all` in feedback status, the printed resolve command includes already-resolved IDs, and archived items still sit under "Address the items below"; the spec only carves out approved revisions.

### Issues Found
- **EXP-1 (minor):** Add a full suggestion golden prompt test.
- **EXP-2 (nit):** Filter the resolve command to non-resolved item IDs and label resolved entries as archived even in feedback exports, or document that `--all` is intentionally a raw dump.

### Notes
Export behavior matches the spec for the normal agent flow. The remaining issues are edge-format tests.

## [MDREVIEW-6] TUI foundation, navigation, outline, search, help

Status: PARTIAL
Score: 4/5

### Checks
- [x] Header, 2-line footer, responsive 26/34/32-column panels, narrow-body overlays, and tiny-terminal guard are implemented.
- [x] Rendered view and F3 raw-source view with line numbers and comment markers work; the source view uses `Document::source_line` raw text (`src/tui/app.rs:1629-1652`).
- [x] Vim and standard dispatch, focus cycling, outline, search, help, F2/F3, source toggle, and signoff are implemented.
- [x] Esc closes a narrow panel before clearing a selection (`src/tui/app.rs:985-992`); media labels are absent on loaded rows; tiny-terminal layout is cleared.
- [x] TestBackend coverage at 80x24, 100x30, 120x40; narrow overlay, mouse, footer, tiny terminal, and gutter tests.
- [ ] External terminal close is not handled as an abort: an event-loop error propagates (`src/tui/mod.rs:39-43`), and `src/cli.rs:219` skips `store.save`, so the CLI exits 2 and does not perform the final save. Spec 006's "external terminal close autosaves and exits aborted" is not met.
- [ ] Spec 006 test list is incomplete: standard-mode keymap beyond Down/c/Esc/Ctrl-f, keyboard outline/help dispatch, quit-autosave, corrupt-sidecar recovery status, and click-outside-ignored are untested.
- [ ] Search underlines every matching line rather than match spans (`src/tui/render.rs:234-242`).
- [ ] Source view does not wrap or horizontally scroll long lines; they are clipped.

### Issues Found
- **TUI-1 (major):** Catch event-loop errors, save the store, restore the terminal, and map the outcome to abort (exit 1) rather than a generic error. Add a test with a failing/poll-error backend if feasible.
- **TUI-2 (minor):** Add the missing spec-006 key/recovery tests.
- **TUI-3 (nit):** Underline only the matching span for search, or document line-level highlighting.
- **TUI-4 (nit):** Wrap or horizontally scroll source lines, or document clipping.

### Notes
Layout and navigation are solid. The error-path exit contract is the only behavioral gap.

## [MDREVIEW-7] Comments and suggested edits in the TUI

Status: PARTIAL
Score: 4/5

### Checks
- [x] Add/edit/delete/resolve comment; add suggestion; selected-source prefill; images accept comments but reject suggestions.
- [x] Selection maps to whole source lines; anchors are trimmed; suggestion originals now come from the trimmed anchor quote.
- [x] Draft autosave on editor keys, draft resume on reopen, autosave on every item mutation, and signoff persistence.
- [x] Save failures no longer get overwritten by success status messages (`save_store` returns success).
- [x] Reattach refreshes a suggestion's original and reopens it; the previous SUGGEST-1 false-stale defect is fixed and tested.
- [x] Tests cover comment create/edit/resolve/delete, suggestion create, draft resume, source-line selection, Enter-to-open, reattach, and suggestion survival after unrelated edits.
- [ ] Spec 006's "Add/edit/delete/resolve comment and suggestion" tests are incomplete: there is no suggestion edit test and no suggestion delete test.
- [ ] `edit_selected_item` edits only a suggestion's replacement, never its original (`src/tui/app.rs:765-801`); the original can only change through reattach.
- [ ] No test asserts that an image rejects a suggestion.

### Issues Found
- **SUGG-1 (minor):** Add suggestion edit/delete and image-suggestion-rejection tests.
- **SUGG-2 (minor):** Decide whether editing a suggestion must also allow original-text changes; if not, document that reattach is the way to change it.

### Notes
The normal-path comment/suggestion lifecycle is correct and the prior blocking defect is gone.

## [MDREVIEW-8] Mouse selection and hit-testing

Status: PASS
Score: 5/5

### Checks
- [x] Click places the cursor; drag selects a line range; wheel scrolls exactly three lines and survives redraws.
- [x] Review-gutter marker click opens the associated item; outline/review inner-rect clicks select and jump.
- [x] Footer Comment/Suggest/Resolve/Send labels are clickable and hit-tested from the current frame layout.
- [x] Mouse-disabled mode ignores events (`--no-mouse`/config).
- [x] Clicking a pane moves focus to it; tiny-terminal layout is cleared so stale rects cannot be hit.
- [x] Tests cover click/drag, wheel, disabled mode, gutter click, panel first-row click, footer clicks, send click, and narrow-overlay clicks.

### Issues Found
- **MOUSE-1 (nit):** No test asserts that a click outside all known rects is ignored; the code already ignores it.

### Notes
No blocking or functional issues.

## [MDREVIEW-9] Image preview pipeline

Status: PARTIAL
Score: 4/5

### Checks
- [x] Relative/absolute local paths resolve; remote URLs are rejected, schedule no worker, and render as link-styled text.
- [x] PNG/JPEG/WebP/BMP/GIF decode; SVG rasterizes with resvg/usvg and a shared system font database.
- [x] 50 MB file, 10,000 px side, and 40 MP limits are enforced; decode-time limits cap allocation; errors include dimensions where applicable.
- [x] Environment protocol detection with explicit override; half-blocks fallback; font-size derivation from TIOCGWINSZ with plausible-range checks; no stdio query.
- [x] Decode/encode runs on workers, visible-block preparation, content-addressed keys with target/protocol, failed-worker caching, resize invalidation, placeholder until ready.
- [x] Layout reserves media rows and the text pass skips loaded image rows; tests cover local PNG/JPEG/BMP/GIF/WebP/SVG, missing/remote, limits, protocols, font metrics, cache keys, async completion, partial visibility, and loaded-row labels.
- [ ] Spec 007's "decode a generated PNG and assert dimensions and non-blank pixels" is not met: the PNG test only asserts a protocol exists (`src/media.rs:584-594`), while the WebP test does the right checks.
- [ ] Oversized placeholders do not show dimensions; the label is `[image: alt] (src)` (`src/document/layout.rs:57-84`) and dimensions appear only in a status-line error. The label is also repeated on all 8 reserved rows.
- [ ] `image_stamp` reads and hashes up to 50 MB on the UI thread on first paint (`src/media.rs:119-158,206-232`).
- [ ] The hash cache trusts mtime+size (`src/media.rs:220-225`); same-size/same-mtime edits reuse a stale key.

### Issues Found
- **MEDIA-1 (minor):** Add `load_image` assertions for a generated PNG (dimensions and a non-blank pixel), mirroring `decodes_webp_and_preserves_pixel_data`.
- **MEDIA-2 (minor):** Put dimensions in the image placeholder text and render the caption once (first row) rather than on all reserved rows.
- **MEDIA-3 (minor):** Move content hashing into the worker or precompute it; first-frame UI stalls on large visible images are a responsiveness risk.
- **MEDIA-4 (nit):** Include file size/mtime plus a cheap content check, or document the metadata trust for the hash cache.

### Notes
Image behavior is close to spec. The remaining gaps are test strength and placeholder/UI-thread polish.

## [MDREVIEW-10] Mermaid rendering

Status: PARTIAL
Score: 4/5

### Checks
- [x] Rust renderer is the default; `mmdc` is explicit or an Auto fallback; `source` never renders.
- [x] `mmdc` runs as a program with arguments (no shell), writes a temp SVG, and cleans up.
- [x] SVG → tree → pixmap → PNG with an explicit font database and dimension caps.
- [x] Invalid diagrams render fenced source with a "diagram unavailable" badge; the parser error reaches the status line when status is free.
- [x] Enter toggles only the current Mermaid block; failed renders are cached; cache keys include engine/theme/source hash/target/protocol.
- [x] Tests cover flowchart, sequence, class, invalid fallback, source engine, Enter toggle, fake `mmdc`, and Auto fallback.
- [ ] Spec 007 requires Mermaid fixtures to render non-empty PNGs with sane dimensions; the tests only assert an encoded protocol exists (`src/media.rs:624-644`).
- [ ] The Auto `mmdc` fallback is not tested through PATH discovery: the test passes the fake program directly (`src/media.rs:671-685`), bypassing `command_exists("mmdc")` in `load_mermaid`.
- [ ] The invalid-Mermaid TUI test does not assert the parser-error status line.

### Issues Found
- **MERMAID-1 (minor):** Call `load_mermaid`/`rasterize_svg` in tests and assert width/height > 0 plus visible pixels.
- **MERMAID-2 (minor):** Add an end-to-end Auto test with a temp PATH containing fake `mmdc`.
- **MERMAID-3 (nit):** Assert `Media: Mermaid parse error` in the status line after a failed render.

### Notes
The rendering pipeline matches the spec. Test strength is the remaining gap.

## [MDREVIEW-11] Agent handshake, tmux adapter, and install recipes

Status: PARTIAL
Score: 4/5

### Checks
- [x] File handshake: request/wait/export with exit codes 10/0/3/2; wait validates the signoff hash.
- [x] tmux requires `$TMUX`; popup uses `-E -w 90% -h 90% -T`; sentinel 20/21/22 mapping; abort 1; popup failure 2 with fallback; custom sidecar.
- [x] `--detach` uses `tmux new-window -d -n mdreview`, returns 0, and prints a quoted wait command.
- [x] Claude/Codex/Gemini/generic recipes; marker blocks preserve surrounding content; normal reruns are idempotent.
- [x] Partial marker blocks are repaired; malformed markers and unreadable/non-UTF-8 files are refused without overwriting.
- [x] Tests cover popup feedback/approved/abort/failure, flags, custom sidecar, detach, request/wait/export/timeout, recipe idempotency, and malformed files.
- [ ] Real tmux exit propagation is unverified (QA-3); the adapter's correctness depends on it.
- [ ] Recipe-content assertions are incomplete: Codex/Gemini tests check markers/idempotency, not that the block contains request/review/export.
- [ ] Spec 008 says all writes preserve existing content, but Claude/generic `write_file` replaces dedicated files (`src/agent.rs:48-54`); only README documents the carve-out.
- [ ] Popup flag assertions omit `-h 90%` and `-T`, and the real popup test does not exist.

### Issues Found
- **AGENT-1 (major):** Resolve QA-3; if real tmux does not propagate sentinels, add the result-file fallback.
- **AGENT-2 (minor):** Assert all four recipe contents contain request/review/export.
- **AGENT-3 (minor):** Amend spec 008 to state that dedicated Claude/generic recipe files are replaced while AGENTS/GEMINI marker blocks are preserved, or make those writes preserve custom content.
- **AGENT-4 (nit):** Assert the full popup flag set (`-E -w 90% -h 90% -T`) in the fake-tmux test.

### Notes
The adapter logic is sound under the assumed tmux behavior. The unverified assumption is the main risk.

## [MDREVIEW-12] Round lifecycle, file changes, stale reattach

Status: PASS
Score: 5/5

### Checks
- [x] Reloading a changed file refreshes the document/outline, clamps cursors, re-anchors items, increments the round after signoff, and saves.
- [x] Unresolved items carry forward; resolved items stay archived; stale items remain visible and can be reattached to a new selection.
- [x] Approval invalidates on content change; export is read-only; request advances a signed-off revision before clearing the signoff.
- [x] Tests cover changed-file reload, round increment, resolved archive, stale reattach (including suggestion original), approval invalidation, export immutability, and two-round end-to-end CLI flow.
- [x] Save failures during resolve/reattach/reload no longer produce a success status.

### Issues Found
- **ROUND-1 (nit):** `reload_document` does not clear `selection` (`src/tui/app.rs:1517-1551`); a selection made before a reload can map to unrelated new lines. Clear it on reload.
- **ROUND-2 (nit):** File changes are detected by mtime+size; a same-size change with an unchanged mtime is missed. A content hash on the 500 ms poll or a signoff-time hash check would close this and the narrow signoff race.

### Notes
The multi-round lifecycle is now coherent and covered. Only low-risk edge cases remain.

## [MDREVIEW-13] End-to-end QA, performance, docs, release (in progress — not graded)

Status: IN PROGRESS

### Present
- [x] README covers purpose, build, quick start, commands, workflow, keys, media matrix, sidecar, config, harness, tests, limitations, and license.
- [x] CHANGELOG has the 0.1.0 MVP entry.
- [x] 2,000-line parse/layout performance test passes with a generous threshold.
- [x] Static-pie release binary exists.
- [x] 128 tests pass in the gate run; fmt and clippy are clean.
- [x] `state/acceptance.md` provides a per-feature evidence matrix.

### Remaining release blockers
- [ ] `gate.sh` has 2 warnings: MDREVIEW-13 is `in_progress`, and the working tree is dirty (20 changes, HEAD `7f36029`).
- [ ] `harness/report.sh` shows 92%, not the required 100%.
- [ ] `state/qa_report.md` is invalid (QA-1).
- [ ] Real tmux popup behavior is unverified and `acceptance.md` overstates it (QA-3).
- [ ] The missing spec-mandated tests listed below.
- [ ] Commit the final state, mark MDREVIEW-13 `done`, and re-run the full gate with a clean tree.

## Missing spec-mandated tests (consolidated)

- **001:** automated `bash -n` sweep; gate with deliberately invalid task JSON; schema validation.
- **002:** clap parsing for every command/flag; config precedence; unknown-key stderr; atomic replacement; status after resolve; resolve unknown ID exit 2.
- **003:** exact 20/40/80-column wrapping; comprehensive acceptance fixture; empty-cell behavior.
- **004:** heading-path-gone Jaccard rejection; explicit resolve/unresolve history assertions (nit).
- **005:** full suggestion golden; `--all` feedback behavior with resolved IDs.
- **006:** full standard-mode keymap; keyboard outline/help; quit-autosave; corrupt-sidecar recovery status; click-outside-ignored.
- **007:** PNG dimension/non-blank-pixel assertion; 40 MP full pipeline; oversized placeholder dimensions; Auto `mmdc` via PATH.
- **008:** recipe content for Codex/Gemini; full popup flags; real tmux sentinel propagation.
- **009:** release checklist completion; clean commit; 100% report; valid QA report.

## Spec / documentation issues to resolve

- Spec 004 "every re-anchor appends a history event" vs. the deliberate no-duplicate-stale-event rule.
- Spec 008 "all writes preserve existing content" vs. replacing Claude/generic recipe files (README documents the carve-out).
- Spec 007 "oversized inputs show a placeholder with dimensions" vs. dimensions appearing only in the status line.
- `state/acceptance.md` real-popup claim vs. the fake-tmux-only evidence.
- Spec 002 does not define the `changed`/`invalidated` status labels.

## Suggested fix order

1. QA-1 (qa.sh report integrity) and QA-2 (commit/release state).
2. QA-3 (real tmux propagation or a result-file fallback).
3. Missing tests for 002/003/005/006/007/008; they are cheap and guard the normal paths.
4. Behavioral polish: external-close abort, image placeholder dimensions/repetition, reload selection, `--all` resolve filtering.
5. Media responsiveness: off-thread hashing and stronger PNG/Mermaid dimension assertions.
6. Spec/doc amendments and the final gate/report/commit.

---

## Final resolution — 2026-11-21 Session 5

### Release blockers resolved

- [x] **QA-1:** `harness/qa.sh` now builds the prompt in a temp file, captures
  `claude --print` output, checks its exit status, filters the harness
  `[claude-code:...]` warning, requires a non-empty report with the expected
  heading, and only then atomically replaces `state/qa_report.md`. Failure and
  success paths were exercised with a fake `claude`; a failed run leaves the
  last good report untouched.
- [x] **QA-2:** MDREVIEW-13 is marked `done`; docs, tests, state, and QA evidence
  are committed; the final gate is re-run on a clean tree; `report.sh` reports
  100%.
- [x] **QA-3:** `__tui` writes its sentinel (20/21/22) to
  `MDREVIEW_RESULT_FILE`; the tmux adapter reads that file and falls back to
  tmux's process status. A fake-tmux test covers the fallback, and a direct PTY
  run of `__tui` wrote 22 on abort. The real `display-popup` path remains a
  desktop manual check; `state/acceptance.md` now states that accurately.
- [x] **TUI-1:** event-loop/terminal errors restore the terminal, autosave the
  review state, and return the aborted exit code (1) instead of error 2.
- [x] **Spec/docs:** amended docs for marker-block preservation, placeholder
  dimensions, stale-event deduplication, `changed`/`invalidated` status labels,
  remote image links, and the tmux evidence claim.

### Residual minor items (documented, not release-blocking)

- Spec-mandated test matrix is broad but not exhaustive: a few clap/config,
  harness, and UI-keymap tests remain future work.
- Image hashing for files up to 50 MB still happens on the UI thread on first
  paint, cached afterward; oversized files are rejected from metadata before
  reading. Disk caching/LRU remains deferred.
- Real Kitty/WezTerm pixel output and a real tmux popup need a desktop manual
  check; protocol selection, encoding, fallback, and result-file propagation
  are automated.
- Source-view long-line wrapping and span-level search highlighting remain
  polish items.

### Final evidence

- `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`,
  `cargo test --all-features` (128 tests), and release-profile tests pass.
- Release binary: static-pie `x86_64-unknown-linux-musl`, `ldd` reports
  `statically linked`.
- Real tmux PTY smoke: Markdown, outline/review panes, and a half-block Mermaid
  diagram rendered; `q` restored the terminal and exited 1.
