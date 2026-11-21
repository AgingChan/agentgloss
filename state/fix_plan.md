# Fix Plan

QA evaluator pass: 2026-11-21 (harness session started ~20:16 UTC+8).
Scope: MDREVIEW-1 through MDREVIEW-12 are marked `done`. MDREVIEW-13 is `in_progress` and is tracked separately.
Output target: `state/fix_plan.md` (read from `harness/qa.sh`, because the QA prompt lost that path — see HARNESS-1).

## Evidence

- Fresh `cargo test --all-features` captured by the harness at pass start: 60 library + 15 CLI + 25 TUI + 1 performance = **101 passed, 0 failed**. This is current-source evidence: the test names match `src/` and the release binary postdates the last source edit.
- `file target/x86_64-unknown-linux-musl/release/mdreview` → `ELF 64-bit LSB pie executable, x86-64, static-pie linked`. Binary mtime 20:16:43 is newer than HEAD (20:16:28) and all source/test edits (latest 20:16:09).
- `python3 -m json.tool state/tasks.json` → valid JSON.
- Placeholder grep over `src/` and `tests/`: no `TODO`, `FIXME`, `todo!`, or `unimplemented!`. The only `unreachable!()` is an assertion helper in `src/document/mod.rs:814`, not a stub.
- Git was clean with three commits at pass start; this report is the only working-tree change. No untracked files remain.
- **Not independently re-runnable in this QA session:** `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, `bash -n`, and `ldd` were denied by the command approval layer. README/`state/progress.md` claim fmt and clippy are clean, but this pass cannot confirm them. The static-pie result and the passing harness test run are confirmed.

## Verdict

12 completed tasks: **1 PASS, 10 PARTIAL, 1 FAIL**.

| Task | Status | Score | Blocking issues |
| --- | --- | --- | --- |
| MDREVIEW-1 Harness/skeleton | PARTIAL | 3/5 | qa.sh prompt corruption; no harness tests |
| MDREVIEW-2 CLI/config/sidecar | PARTIAL | 4/5 | unquoted printed commands; missing config/clap tests |
| MDREVIEW-3 Document model | PARTIAL | 3/5 | missing empty/CRLF/wrap/task/rule tests; empty cells skipped |
| MDREVIEW-4 Review model | PARTIAL | 3/5 | missing heading-move/inserted/deleted/Unicode/Jaccard tests |
| MDREVIEW-5 Export | PARTIAL | 4/5 | unquoted resolve command; missing escaping/blank/fence tests |
| MDREVIEW-6 TUI foundation | PARTIAL | 4/5 | missing standard/help/suggestion-edit/quit/corrupt tests |
| MDREVIEW-7 Comments/suggestions | FAIL | 2/5 | line-selected suggestions falsely go stale after any re-anchor |
| MDREVIEW-8 Mouse | PASS | 4/5 | minor hit-test/focus notes only |
| MDREVIEW-9 Images | PARTIAL | 3/5 | limits after decode; UI-thread hashing; cache-key deviations |
| MDREVIEW-10 Mermaid | PARTIAL | 4/5 | tests don't assert PNG dimensions/pixels or auto-mmdc fallback |
| MDREVIEW-11 Agent/tmux/recipes | PARTIAL | 4/5 | unquoted detach command; malformed markers; missing recipe assertions |
| MDREVIEW-12 Rounds/file change/reattach | PARTIAL | 3/5 | suggestion false-stale; missing re-anchor cases; save errors masked |

MDREVIEW-7 is FAIL because the core multi-round suggestion contract ("a suggestion becomes stale only when its original text no longer matches") is violated in the normal TUI path. The defect is fixed easily and must be fixed before MDREVIEW-13 can be marked done.

## Blocking findings

### SUGGEST-1 (MAJOR, MDREVIEW-7; cross-refs 4 and 12): TUI suggestions falsely become stale after any revision

- **Repro:** select a line in the TUI, press `s`, change the replacement, save. Then edit any unrelated part of the document (or reopen after any revision) and let the TUI reload. The suggestion becomes `stale` even though its original text is untouched.
- **Cause:** `open_suggestion_editor` stores `original` from the untrimmed line range, which includes the trailing newline (`src/tui/app.rs:711`, `:723-726`), and `save_editor` passes that string to `add_suggestion` (`src/tui/app.rs:1302-1304`). `Anchor::from_range` trims the trailing newline from the anchor quote/range (`src/review/mod.rs:103-108`). On re-anchor, the staleness check compares `suggestion.original` (with `\n`) against `document.text_in_range(anchor.range())` (without `\n`) and always fails (`src/review/mod.rs:543-546`).
- **Impact:** every suggestion created through the documented TUI flow with a normal newline-terminated file is archived as stale on the next round, so agents may ignore live feedback.
- **Fix:** derive `original` from the created `Anchor::quote` (already trimmed), or trim the trailing `\r?\n` from `original` in `open_suggestion_editor`/`save_editor`. Also update `suggestion.original` when a stale suggestion is reattached (`ReviewStore::reattach`, `src/review/mod.rs:430-447`) or require the reviewer to edit the original.
- **Required test:** create a suggestion through `App`/`open_suggestion_editor`, re-anchor with an unrelated document edit while the quote still exists, assert the status remains `Open`; then remove the original text and assert it becomes `Stale`.

### HARNESS-1 (MAJOR, MDREVIEW-1 and MDREVIEW-13): `qa.sh` corrupts its own QA prompt and executes backticked text

- **Cause:** `harness/qa.sh:44` uses an unquoted heredoc delimiter (`cat <<PROMPT | claude ...`). Bash therefore performs command substitution on every backtick in the prompt. The paths at lines 52-59 and the output target at line 79 are executed as commands and replaced with empty strings; the `cargo test` backtick at line 56 executes and injects its whole output into the prompt. This exact failure is visible in the delivered prompt: task/spec paths and the output path are blank. The test output unexpectedly appearing in parentheses is the side effect.
- **Impact:** evaluators lose the task/spec/output paths (the target had to be recovered from the script), and any future backticked text becomes command execution inside the harness.
- **Fix:** quote the heredoc (`<<'PROMPT'`) and inject `TASK_FILTER` without re-evaluating the body, e.g. build the string with a placeholder and substitute it via a method that treats the value literally (printf/parameter expansion or a small Python helper), then pipe the assembled text to `claude --print`. Do not use an unquoted heredoc for a document containing backticks.

## Prior QA findings: verified fixed

| Prior finding | Current evidence |
| --- | --- |
| C1 tmux abort/feedback treated as tmux failure | `src/agent.rs:202-215` accepts sentinels 20/21/22; tests `tests/cli.rs:364-416` cover 21/22/1 |
| C2 `--tmux --sidecar` read the default sidecar | `review_sidecar` (`src/agent.rs:182,218-222`) is passed through; test `tests/cli.rs:418-436` |
| M1 export mutated state without lock | Export locks and works on a clone, never saves (`src/cli.rs:301-325`); tests `tests/cli.rs:165-174,288-307` |
| M2 status reported stale approval | Live hash comparison (`src/cli.rs:335-347`); test `tests/cli.rs:176-193` |
| M3 stale items could not be reattached | `r` action in `src/tui/app.rs:811-835`; test `tests/tui.rs:375-405` |
| M4 `block_at` returned block 0 in gaps/EOF | `rfind`-style reverse lookup (`src/document/mod.rs:169-174`); test `src/document/mod.rs:916-922` |
| M5 no panel overlay below 80 columns | Full-body overlay (`src/tui/render.rs:35-39,64-77`); tests `tests/tui.rs:275-300` |
| M6 gutter click / panel off-by-one | Inner-rect hit-testing (`src/tui/render.rs:607-614`, `src/tui/app.rs:1352-1387`); tests `tests/tui.rs:310-349` |
| M7 suggestion export quoted | Suggestions are verbatim (`src/review/export.rs:103-115`); test `src/review/export.rs:260-272` |
| M8 Mermaid source/invalid fallback | Source render + badge (`src/tui/render.rs:283-318`); tests `tests/tui.rs:425-448` |
| M9 image cache not content-addressed | Content SHA-256 added (`src/media.rs:201-224`); test `src/media.rs:628-653` (with residual deviations below) |
| M10 wait could return a stale signoff | `request` clears signoff/approval (`src/review/mod.rs:306-310`); test `tests/cli.rs:195-210` |
| M11 table cells / HTML / nested Mermaid | Per-cell inlines (`src/document/mod.rs:723-765`); tests `:825-864,896-913` |
| M12 visual-line selection | Map to whole source lines (`src/tui/app.rs:435-477`); test `tests/tui.rs:407-423` |
| M13 unknown config keys did not warn | Warning implemented (`src/config.rs:84-108`); no stderr test |
| M14 corrupt-sidecar recovery not shown | `startup_status` (`src/cli.rs:194-206` → `src/tui/mod.rs:34-36`); no UI test |
| M15 tabs/grapheme wrapping | Four spaces + grapheme wrapping (`src/document/layout.rs:131-155,190-258`); tests `:375-395` |

Also fixed from the prior minor list: resolve/unresolve history, height <10 tiny-terminal rule, Shift+Home/End line selection, per-block Mermaid Enter toggle, source-view comment markers, open-only header counts, numeric item sort tiebreak, mmdc temp cleanup, execute-bit check, case-insensitive scheme rejection, overlapping quote search, normalized context length, nanosecond backup names, raw-mode cleanup, and newline/tab sanitization in review excerpts.

---

## [MDREVIEW-1] Harness, Rust skeleton, and static build

Status: PARTIAL
Score: 3/5

### Checks
- [x] `init.sh`, `scripts/env.sh`, `.cargo/config.toml`, and the full harness script set exist.
- [x] `state/tasks.json` follows the task schema shape and is valid JSON; tasks are ordered and not removed.
- [x] `state/progress.md` is updated; three commits exist; working tree is clean.
- [x] Release binary reports `static-pie linked`.
- [x] `gate.sh` validates task state, placeholders, fmt, clippy, and tests.
- [ ] No automated tests exist for the spec's harness test requirements (shell syntax sweep, gate with invalid JSON, schema validation).
- [ ] `qa.sh` prompt construction is broken (HARNESS-1) and executed backticks/commands.
- [ ] `harness/templates/task-schema.json` is not used by `gate.sh` or any test; validation is hand-rolled.
- [ ] `bash -n` and `init.sh`/`gate.sh` could not be re-run in this QA session (command approval denied).

### Issues Found
- **HARNESS-1 (major):** unquoted heredoc in `harness/qa.sh:44` executes every backtick and strips paths/commands from the QA prompt. Fix as described above.
- **HARNESS-2 (minor):** spec 001's "Tests" section has no implementation: no shell-syntax test, no valid/invalid task-JSON gate test, and no schema validation test. Add a `tests/harness.sh` (or Rust integration test) wired into the gate; use `harness/templates/task-schema.json` or `python3 -m json.tool`.
- **HARNESS-3 (nit):** `gate.sh` does not verify the static release binary, although spec 001 and 009 cite it. The release criterion is checked manually only.

### Notes
The harness structure is complete and the gate logic is stricter than the spec requires. The critical gap is prompt integrity, because it silently degrades every QA pass.

## [MDREVIEW-2] CLI, config, and sidecar storage

Status: PARTIAL
Score: 4/5

### Checks
- [x] All documented commands and flags exist; hidden `__tui` is wired.
- [x] Exit codes 0/10/1/2/3 map correctly; internal TUI codes 20/21/22 are mapped.
- [x] Sidecar default path, lock, atomic temp+rename, corrupt backup, and strict version rejection are implemented.
- [x] Export is read-only and locks the store; status detects changed/invalidated approvals; request clears a prior signoff.
- [x] Tests cover request/status/export/resolve, lock contention, status invalidation, request clearing signoff, custom sidecar, and tmux sidecar.
- [ ] Spec 002 tests are still incomplete: no clap parsing test for every command/flag, no config precedence test (default < file < CLI), no stderr assertion for the unknown-key warning, and no direct atomic-replacement assertion.
- [ ] Printed handoff commands do not shell-quote file paths.

### Issues Found
- **CLI-1 (major):** `run_request` prints unquoted commands (`src/cli.rs:262-271`). A path containing spaces, quotes, or shell metacharacters produces a command the human/agent cannot paste. Apply the same quoting used by `agent::shell_quote` (`src/agent.rs:253-256`).
- **CLI-2 (minor):** `export` takes an exclusive store lock (`src/cli.rs:307`) even though it is read-only. This intentionally makes export fail with exit 2 while a review TUI is open. That is acceptable for the documented handshake, but a shared/read lock or a retry would be friendlier; document the behavior.
- **CLI-3 (minor):** `Config::parse_at` warns on unknown keys (`src/config.rs:84-108`), but the test at `src/config.rs:164-180` does not capture stderr. Add a test asserting the warning text and that parsing still succeeds.
- **CLI-4 (nit):** `hash_file` (`src/cli.rs:421-425`) has no callers; remove it or use it.
- **CLI-5 (nit):** `status` introduces a `"changed"` status not defined in specs 002/004/005. It is sensible, but document it in spec 002 or use `pending`/`invalidated` only.

### Notes
The prior export/status/request lifecycle defects are fixed and tested. Remaining risk is limited to printed-command quoting and missing configuration tests.

## [MDREVIEW-3] Markdown document model and source map

Status: PARTIAL
Score: 3/5

### Checks
- [x] pulldown-cmark GFM options and offset iterator are used.
- [x] Heading paths, block depth, list-item blocks, and source/line ranges are implemented.
- [x] Tables are one block with per-cell inlines carrying source ranges; HTML is constructed; nested Mermaid becomes its own block.
- [x] `block_at` now returns the nearest preceding block in gaps and at EOF.
- [x] Tabs expand to four spaces; wrapping is grapheme-aware; media blocks reserve rows.
- [x] 2,000-line parse/layout performance test passes.
- [ ] Spec 003 tests are incomplete: no empty input, CRLF, or no-trailing-newline tests; wrapping is not exercised at exactly 20/40/80; task lists, rules, and footnotes have no block-kind assertions; the fixture does not cover the full acceptance list.
- [ ] Empty table cells are silently dropped.

### Issues Found
- **DOC-1 (minor):** `Builder::push_inline` returns early for empty text (`src/document/mod.rs:271-273`). A table cell with no content therefore produces no `Inline`, contrary to "cells are inlines with source ranges" (spec 003:50). Push an empty inline with the cell range, or document the exception.
- **DOC-2 (minor):** The fixture `tests/fixtures/spec.md` covers headings, a paragraph, a list, Mermaid, an image, and a table only. Add nested/task lists, blockquote, link, rule, HTML, footnote, emphasis, and Unicode cases to satisfy spec 003's acceptance list.
- **DOC-3 (nit):** The unit test `whitespace_and_block_fallback` in the review model only exercises whitespace normalization; the document side has no Jaccard/heading fallback test. See MDREVIEW-4.

### Notes
The prior major parser defects (block_at, per-cell ranges, HTML, nested Mermaid) are fixed. The remaining gaps are primarily test completeness and the empty-cell edge case.

## [MDREVIEW-4] Review model, anchors, rounds, and re-anchoring

Status: PARTIAL
Score: 3/5

### Checks
- [x] Sidecar schema and stable `cN` IDs with monotonic `next_id`.
- [x] Anchor creation stores quote, 32-char prefix/suffix, heading path, block, byte/line range, and hash; trailing newlines are trimmed.
- [x] Exact, duplicate-context, whitespace-normalized, and heading/Jaccard fallback paths exist; not-found becomes stale with history.
- [x] Resolved items are skipped; re-anchoring appends history; suggestion staleness is checked.
- [x] Round increment and approval invalidation on a signed-off content change; resolve/unresolve append history; reattach is reachable through the TUI.
- [x] Tests cover exact/moved, duplicate quotes, whitespace, stale, suggestion disappearance, round/approval, resolve/unresolve/remove, sidecar, corrupt backup, version rejection, and lock.
- [ ] Spec 004 tests are still missing for heading moves, inserted text, deleted text, Unicode quote matching, and a real Jaccard/block fallback.
- [ ] Suggestion staleness has a normal-path false positive (SUGGEST-1).

### Issues Found
- **REV-1 (major, shared with MDREVIEW-7):** suggestion original/range mismatch (SUGGEST-1).
- **REV-2 (minor):** `reattach` (`src/review/mod.rs:430-447`) does not update `suggestion.original` for a stale suggestion. Reattaching a suggestion to unrelated text leaves an original that can never match the new anchor. Either update it from the new anchor quote or force an edit.
- **REV-3 (minor):** The documented heading-fallback stage is effectively untested. `whitespace_and_block_fallback` (`src/review/mod.rs:954-964`) succeeds through normalization, not Jaccard. Add a case where the quote is rewritten enough that exact/normalized matching fails but token Jaccard ≥ 0.5 under the same heading, and a case where the heading path no longer exists.
- **REV-4 (minor):** Spec 004 says "every re-anchor appends a history event"; when an already-stale item is still not found, no new event is appended (`src/review/mod.rs:526-539`). This avoids duplicate events but is a literal deviation; document it.

### Notes
The re-anchoring algorithm matches the spec's five-stage design. The missing tests make the Jaccard branch a regression risk.

## [MDREVIEW-5] Prompt and JSON export

Status: PARTIAL
Score: 4/5

### Checks
- [x] Numbered prompt with file/round/hash/status/counts and per-item headings.
- [x] Quotes are `> `-prefixed, blank lines become `>`, and suggestion original/replacement are verbatim.
- [x] Items ordered by `source_start` then numeric ID; numbering is sequential; `--all` includes resolved items labelled by status.
- [x] Open/stale items included by default; JSON shape and filtering implemented; exit code 10 for open/stale, 0 otherwise.
- [x] Comment golden test and JSON filtering/exit-code test.
- [ ] Missing spec 005 tests: quote escaping, blank lines inside quotes, a body containing Markdown fences, anchors inserted out of order, empty export, and a suggestion golden.
- [ ] The `resolve` command printed in the prompt is not shell-quoted.

### Issues Found
- **EXP-1 (minor):** The resolve command at `src/review/export.rs:69-77` interpolates the file path without quoting, so the "paste-ready" command breaks on paths with spaces. Reuse `agent::shell_quote`.
- **EXP-2 (minor):** With `--all` and a valid approval, resolved items are non-empty, so the prompt prints "Address the items below. Answer every comment..." (`src/review/export.rs:56-77`) even though the review is approved and the items are archived. Print the approved/no-action line and list archived items separately.
- **EXP-3 (minor):** The header counts open comments/suggestions correctly now, but there is no test for a stale-only export where `Open items: 0` and `Stale items: N` must both appear.

### Notes
The suggestion verbatim fix is correct. The remaining work is test coverage and the two output edge cases above.

## [MDREVIEW-6] TUI foundation, navigation, outline, search, help

Status: PARTIAL
Score: 4/5

### Checks
- [x] Header, two-line footer, responsive panels, narrow full-body overlay, and the height/width "terminal too small" guard.
- [x] Rendered view and raw-source view with line numbers and comment markers.
- [x] Vim/standard dispatch, focus cycling, outline, search, help, F2/F3, and signoff implemented.
- [x] Narrow overlay is clickable and closes with Esc when no selection is active.
- [x] TestBackend coverage at 80x24, 100x30, and 120x40; narrow overlay tests.
- [ ] Missing spec 006 tests: full standard-mode keymap, outline/help key dispatch, suggestion edit/delete, quit autosave, corrupt-sidecar recovery status.
- [ ] Esc with an active selection clears the selection instead of closing a narrow overlay.

### Issues Found
- **TUI-1 (minor):** In a narrow terminal, `Esc` first clears a lingering selection (`src/tui/app.rs:943-950`) and only a second `Esc` closes the overlay. Prioritize `focus != Document` so the overlay closes on the first Esc, or clear both.
- **TUI-2 (minor):** Source view lines are not wrapped (`src/tui/app.rs:1580-1603`, `src/tui/render.rs:160-173`), so long source lines are clipped even though the gutter reserves width. Either wrap source lines or accept/document clipping.
- **TUI-3 (nit):** Search "highlighting" underlines the whole matching line (`src/tui/render.rs:233-241`), not the matching span. Spec 006:52 says matches are highlighted; span-level highlighting would be clearer.
- **TUI-4 (nit):** The standard-mode help text and README key table omit the `r` reattach action; the help overlay includes it. Add it to the README table.

### Notes
The layout, keymap, and overlay work are solid. Test coverage is strong for the Vim path and weaker for standard mode and recovery/persistence paths.

## [MDREVIEW-7] Comments and suggested edits in the TUI

Status: FAIL
Score: 2/5

### Checks
- [x] Comment add/edit/resolve/delete and suggestion creation are implemented.
- [x] Selection maps to whole source lines; suggestions prefill the selected source; images reject suggestions.
- [x] Draft autosave on every editor key and draft resume on reopen; signoff writes state.
- [x] Tests cover comment create/edit/resolve/delete, suggestion create, draft resume, selection expansion, and Enter-to-open.
- [ ] Suggestion edit/delete paths have no tests.
- [ ] Suggestion original/staleness is wrong for the normal TUI flow (SUGGEST-1).

### Issues Found
- **SUGGEST-1 (major, blocking):** line-selected suggestions store `original` with a trailing newline while re-anchoring compares against a trimmed range, so any revision falsely marks them stale. Fix in `src/tui/app.rs:711,723-726,1302-1304`, with a test that edits an unrelated sentence and asserts the suggestion stays open.
- **SUGG-2 (minor):** `edit_selected_item` only edits a suggestion's replacement, never its original; combined with reattach (`REV-2`), stale suggestions can remain semantically detached. Decide whether reattach should set the original from the new anchor.
- **SUGG-3 (minor):** `save_editor`, `resolve_selected`, `unresolve_selected`, and `reattach_selected` set a success message after `save_store` even when the save failed, hiding the failure (`src/tui/app.rs:775-799,824-830,1321-1324`). Return a success flag from `save_store` and only overwrite the status on success.
- **SUGG-4 (nit):** Empty comments and unchanged suggestions are silently not saved with an explanatory status; that is reasonable, but add tests so the behavior is locked.

### Notes
The comment path is complete. The suggestion defect is the single most important functional issue in this pass because it breaks the multi-round review promise.

## [MDREVIEW-8] Mouse selection and hit-testing

Status: PASS
Score: 4/5

### Checks
- [x] Click moves the cursor; drag selects a line range; wheel scrolls three lines.
- [x] Review-gutter marker click opens the associated item.
- [x] Outline/review panel clicks use inner rectangles (border offset fixed) and jump to the item.
- [x] Footer Comment/Suggest/Resolve/Send labels are clickable.
- [x] `--no-mouse`/`mouse=false` disables capture and handler processing.
- [x] Tests cover click/drag, wheel, mouse-disabled, gutter click, panel first-row click, footer click, and narrow-overlay click.

### Issues Found
- **MOUSE-1 (nit):** Clicking a review item jumps to its source but leaves focus in the document; clicking an outline item leaves focus where it was. This is usable but makes follow-up `j/k` ambiguous. Consider setting focus to the clicked panel.
- **MOUSE-2 (nit):** No test asserts that a click outside all known rects is ignored. The code does ignore it; add a regression test.
- **MOUSE-3 (nit):** After the terminal shrinks below the tiny-terminal threshold, `app.layout` retains the previous frame's rects. Clicks in the small terminal can hit stale rects. Clear `layout` on the early-return path.

### Notes
This task meets its verification list. No blocking issues.

## [MDREVIEW-9] Image preview pipeline

Status: PARTIAL
Score: 3/5

### Checks
- [x] Relative/absolute local paths resolve; remote and missing paths error with clear messages.
- [x] PNG/JPEG/BMP/GIF decode and SVG rasterize; dimension checks include the offending dimensions.
- [x] Environment-based protocol detection with explicit override plumbing; half-blocks fallback; no stdio query.
- [x] Off-thread decode/encode; visible-block preparation; cache hit/miss/content/resize tests.
- [x] `mmdc` temp cleanup, execute-bit check, and case-insensitive scheme rejection fixed.
- [ ] Missing spec 007 tests: WebP, 50 MB / 10k px / 40 MP pipeline cases, PNG dimension/non-blank-pixel assertions, explicit `--protocol` override, and an assertion that no text is drawn inside a media rect.
- [ ] Size/dimension limits are enforced only after a full decode.
- [ ] Cache key does not fully match spec 007: it includes `src`, uses terminal cells rather than target pixel dimensions, and omits theme for images.

### Issues Found
- **MEDIA-1 (minor, robustness):** `load_image` decodes first and calls `check_dimensions` afterwards (`src/media.rs:260-265,460-476`). A 50 MB compressed image can allocate hundreds of MB before rejection. Use `ImageReader` dimension probing or decoding limits before full decode; add the 50 MB and 40 MP pipeline tests.
- **MEDIA-2 (minor, responsiveness):** `ensure_image` calls `image_stamp` on the UI thread (`src/media.rs:127,201-224`), which reads and SHA-256-hashes up to 50 MB on the first frame for each image. Move hashing to the worker (or cache/precompute) so scrolling/first paint stays responsive.
- **MEDIA-3 (minor, spec deviation):** The image cache key is `version:src:WxH:protocol:content-hash` (`src/media.rs:128-136`). Two paths to the same content miss; theme is absent; target is cells, not pixels. Key on content hash + engine/version/theme + target pixel dimensions as spec 007:25 says.
- **MEDIA-4 (minor, spec gap):** No warning is emitted when the font database is empty (`src/media.rs:392-401`), contrary to spec 007:9. Report "SVG text unavailable: no fonts" through the media status.
- **MEDIA-5 (minor, spec gap):** Remote image URLs show a placeholder/error, not a link (spec 007:5). At minimum render the URL as a link-styled line or document the deviation.
- **MEDIA-6 (nit):** The image hash cache trusts mtime+size (`src/media.rs:212-216`); same-mtime/same-size content changes can reuse a stale key.

### Notes
The content-addressing fix and protocol tests are real improvements. The dominant risks are the pre-decode limit gap and UI-thread hashing.

## [MDREVIEW-10] Mermaid rendering

Status: PARTIAL
Score: 4/5

### Checks
- [x] Rust renderer is the default; `mmdc` is explicit or auto fallback when the Rust renderer fails and `mmdc` is on PATH.
- [x] `source` engine never renders; invalid diagrams render fenced source with a badge and status error.
- [x] Enter toggles only the current block's source; failed renders are cached; cache key includes engine/theme/source hash/target/protocol.
- [x] Tests cover flowchart, sequence, class, invalid source fallback, source engine, and a fake `mmdc` executable.
- [x] Source/fallback rendering was verified in TestBackend at 100x30.
- [ ] Spec 007's "non-empty PNGs with sane dimensions" test requirement is not met: the Mermaid tests only assert `media.get(&key).is_some()`.
- [ ] The automatic `Auto` fallback path through a fake `mmdc` on PATH is not tested; the fake program is called directly.
- [ ] No font-availability warning (shared MEDIA-4).

### Issues Found
- **MERMAID-1 (minor, test strength):** `renders_a_simple_mermaid_diagram` and `renders_sequence_and_class_diagrams` (`src/media.rs:563-584`) only assert an encoded protocol exists. Decode the cached PNG output and assert width/height > 0 and at least one non-transparent/non-background pixel, as spec 007:44 requires.
- **MERMAID-2 (minor, test gap):** The Auto engine's mmdc fallback is untested end-to-end. Add a test that puts a fake `mmdc` first on PATH, forces the Rust parser to fail, and asserts the fallback SVG is used.
- **MERMAID-3 (nit):** With `MermaidEngine::Source`, `prepare_media` still spawns a worker that errors "rendering is disabled" (`src/tui/app.rs:390-396`, `src/media.rs:276-277`) before the UI falls back to source. Skip the worker for source mode to avoid pointless work and pending-entry churn.

### Notes
Functionality matches spec 007. The missing dimension/pixel assertions are the main reason this is PARTIAL.

## [MDREVIEW-11] Agent handshake, tmux adapter, and install recipes

Status: PARTIAL
Score: 4/5

### Checks
- [x] request/wait/export handshake and exit codes.
- [x] tmux `$TMUX` requirement, display-popup flags, sentinel exit codes 20/21/22, custom sidecar, and detach/wait path.
- [x] Popup failure (status 1) exits 2 with a fallback command.
- [x] Claude/Codex/Gemini/generic recipes; marker blocks; existing content preserved; idempotent normal runs.
- [x] Tests cover popup feedback/abort/failure, custom sidecar, detach/custom detach, request/wait/export/timeout, and recipe idempotency.
- [ ] No test for popup approved code 20.
- [ ] Codex/Gemini block content is not asserted to contain request/review/export; only the markers/idempotency are checked.
- [ ] Malformed marker blocks and non-UTF-8 recipe files are mishandled.

### Issues Found
- **AGENT-1 (minor):** The detached wait command is printed unquoted (`src/agent.rs:169-177`), so paths with spaces break. Use `shell_quote`.
- **AGENT-2 (minor):** `upsert_block` (`src/agent.rs:56-83`) appends a whole new block when only `START` or only `END` exists, producing duplicate/mis-nested markers. It also uses `read_to_string(...).unwrap_or_default()`, so a non-UTF-8 or unreadable AGENTS.md/GEMINI.md is silently overwritten. Treat partial markers as an error or repair them, and propagate read errors.
- **AGENT-3 (minor):** `write_file` overwrites an existing `.claude/commands/mdreview-review.md`/`mdreview-agent.md` (`src/agent.rs:26,41,48-54`). Spec 008:47 says all writes preserve existing content. Either merge/refuse to overwrite customized files or carve out dedicated recipe files explicitly in the spec.
- **AGENT-4 (minor, verification risk):** The adapter assumes `tmux display-popup -E` propagates the `__tui` sentinel code. The fake-tmux tests prove command construction, not real tmux behavior. The spec should state the propagation assumption, and a real tmux smoke test should be recorded (exit 20/21/22).
- **AGENT-5 (nit):** Add an approved-popup test (`TMUX_EXIT=20` → exit 0, approved prompt) and assert the popup flags (`-w 90% -h 90% -T`).

### Notes
The prior tmux criticals are fixed and well tested. Remaining issues are edge-case robustness and a real-tmux verification gap, not the core handshake.

## [MDREVIEW-12] Round lifecycle, file changes, stale reattach

Status: PARTIAL
Score: 3/5

### Checks
- [x] Reloading a changed file refreshes the outline, clamps cursors, re-anchors, increments the round after signoff, and saves.
- [x] Unresolved items carry forward; resolved items stay archived; stale items are visible and can be reattached.
- [x] Approval invalidates on change; export is read-only; `request` advances a signed-off revision before clearing.
- [x] Tests cover changed-file reload, round increment, resolved archive, reattachment, export lifecycle, and two-round end-to-end flow.
- [ ] Suggestion false-stale (SUGGEST-1) is a normal-path failure of this lifecycle.
- [ ] The spec 004 re-anchor matrix (heading move, inserted/deleted text, Unicode, Jaccard) is still untested.
- [ ] Save failures during resolve/reattach/reload are reported as success (SUGG-3).

### Issues Found
- **ROUND-1 (major, shared with MDREVIEW-7):** suggestions created in the TUI acquire a trailing newline in `original` and become stale on the next re-anchor. This directly breaks "open comments survive and stale items are visible" for suggestions.
- **ROUND-2 (minor):** `reload_document` calls `save_store()` and then unconditionally overwrites the status with "File changed; open items re-anchored" (`src/tui/app.rs:1494-1498`), hiding a save failure. Same pattern in resolve/unresolve/reattach. Fix with a success-returning save helper.
- **ROUND-3 (minor, test gap):** Reattachment is covered only for a comment. Add a stale-suggestion reattach case and assert whether/how `original` changes.
- **ROUND-4 (nit):** A file change is detected by mtime+len (`src/tui/app.rs:1631-1634`); a same-size change with an unchanged mtime is missed. Low risk on modern filesystems, but a content hash on the poll would be exact.

### Notes
The round/approval/export lifecycle is now coherent and covered. The suggestion staleness defect is the remaining functional blocker.

## [MDREVIEW-13] End-to-end QA, performance, docs, release

Status: IN PROGRESS (not graded)
Score: n/a

### Present
- [x] README covers purpose, build, quick start, commands, workflow, keys, media matrix, sidecar, config, harness, tests, limitations, and license.
- [x] CHANGELOG exists with the 0.1.0 MVP entry.
- [x] 2,000-line parse/layout performance test with a generous 2 s threshold passes.
- [x] Static-pie release binary exists and is newer than the final commit.
- [x] 101 tests pass in the harness run.

### Remaining
- [ ] Fix SUGGEST-1 and HARNESS-1; re-run the full QA pass.
- [ ] Add the missing tests listed below (or amend the specs to document waivers).
- [ ] Re-run `cargo fmt --check`, clippy with `-D warnings`, `cargo test --all-features`, `./init.sh`, and `./harness/gate.sh` in an environment where those commands are permitted; capture the output.
- [ ] Record the acceptance-matrix evidence from spec 009 (the README currently asserts the test counts but does not provide per-feature evidence).
- [ ] Mark MDREVIEW-13 `done` only after the gate passes; `harness/report.sh` should then show 100% (currently 12/13 = 92%, with task 13 in progress).
- [ ] Commit the final state.

## Missing tests required by the specs

These are spec requirements, not optional polish. Several were listed in the previous QA pass and remain.

- **001:** automated `bash -n` sweep; gate with valid and invalid task JSON; schema validation against `harness/templates/task-schema.json`.
- **002:** clap parsing for every command/flag; config precedence (default < file < CLI); stderr assertion for unknown-key warning; atomic replacement assertion; status after resolve.
- **003:** empty input, CRLF, no trailing newline; wrapping at 20/40/80; task-list, rule, footnote, and nested-list assertions; full fixture coverage; per-cell empty-cell behavior.
- **004:** heading-move, inserted-text, deleted-text, Unicode quote matching, real Jaccard/block fallback, and explicit history assertions for resolve/unresolve.
- **005:** quote escaping, blank lines inside quotes, a body containing Markdown fences, anchors inserted out of order, empty export, and a suggestion golden.
- **006:** full standard-mode keymap, outline/help dispatch, suggestion edit/delete, quit-autosave, corrupt-sidecar recovery status, and click-outside-ignored.
- **007:** WebP; 50 MB / 10k px / 40 MP pipeline; PNG dimension and non-blank-pixel assertions; explicit `--protocol` override; no text inside a media rect; Auto `mmdc` fallback through PATH.
- **008:** popup approved code 20; content assertions for all four recipes; malformed/non-UTF-8 recipe files.
- **009:** acceptance-matrix execution evidence; release checklist (fmt/clippy/test/static binary); gate/report 100%.

## Spec issues / ambiguities to resolve

- Spec 004 "every re-anchor appends a history event" vs. the implementation's deliberate no-duplicate-stale-event rule.
- Spec 004/005 phrase "stale items block approval" (implementation) vs. the earlier wording "no open items"; the current code treats stale as feedback, which is the safer interpretation. Keep the spec wording consistent.
- Spec 008 says the popup "captures the popup exit status" and returns 0/10/1; the implementation depends on tmux propagating internal sentinels 20/21/22. Document that dependency and the minimum tmux behavior.
- Spec 008 "all writes preserve existing content" vs. overwriting dedicated Claude/generic recipe files; clarify whether dedicated files are replaced.
- Spec 007 cache key says "target pixel dimensions"; the implementation keys on terminal cells. Clarify which target the key must use.

## Suggested fix order

1. SUGGEST-1 (false-stale suggestions) — functional blocker for the core review loop.
2. HARNESS-1 (`qa.sh` prompt corruption) — blocks reliable QA and can execute backticked text.
3. Add the missing tests for 003/004/005/007/008; they are cheap and guard the re-anchoring and media paths.
4. Quoting in printed commands (CLI-1, EXP-1, AGENT-1) and save-error status handling (SUGG-3/ROUND-2).
5. Media robustness: pre-decode limits, off-thread hashing, cache-key alignment, font warning.
6. Harness tests and the remaining spec/doc ambiguities; then run the release checklist and mark MDREVIEW-13 done.
