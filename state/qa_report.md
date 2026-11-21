# mdreview QA Evaluator Pass

**Scope:** all 13 tasks (`MDREVIEW-1` … `MDREVIEW-13`), all marked `done`.
**Evidence source:** the pass-start gate (21:57) plus direct reads of `/tmp/mdreview-test.log`, `/tmp/mdreview-clippy.log`, `/tmp/mdreview-fmt.log`, source, tests, specs, and state. **Direct `cargo`/`gate.sh` re-execution was denied by this session's command-approval layer**, so the fresh gate logs are the execution evidence; every code claim below was checked against current source.

**Deliverable status:** the `Write` to `state/fix_plan.md` was not granted by the permission layer. The actionable plan is included in full under “Consolidated fix plan” below; `harness/qa.sh` will persist this report to `state/qa_report.md`.

## Verdict

| Task | Status | Score | Primary gap |
| --- | --- | --- | --- |
| MDREVIEW-1 Harness/skeleton | PARTIAL | 4/5 | No automated negative/schema harness tests; `plan.sh` persistence |
| MDREVIEW-2 CLI/config/sidecar | PARTIAL | 4/5 | `resolve` accepts a missing file; read-only export locks; test matrix |
| MDREVIEW-3 Document model | PARTIAL | 4/5 | Empty table cells, multi-paragraph list items, fixture/width matrix, control chars |
| MDREVIEW-4 Review model | PASS | 4/5 | Version probe ordering; two weak re-anchor tests |
| MDREVIEW-5 Export | PARTIAL | 4/5 | Suggestion golden missing; `--all` feedback labeling |
| MDREVIEW-6 TUI foundation | PARTIAL | 4/5 | `Ctrl+[` cannot fire; help clipping; missing key/recovery tests |
| MDREVIEW-7 Comments/suggestions | PARTIAL | 4/5 | No suggestion comment field; draft not re-anchored; test gaps |
| MDREVIEW-8 Mouse | PASS | 5/5 | Click-outside test nit |
| MDREVIEW-9 Images | PARTIAL | 4/5 | Media covers gutter; weak PNG/pixel tests; PATH/hash/resize gaps |
| MDREVIEW-10 Mermaid | PARTIAL | 4/5 | Dimensions/pixels unasserted; Rust theme inert; Auto PATH untested |
| MDREVIEW-11 Agent/tmux/recipes | PARTIAL | 4/5 | Real popup evidence; duplicate markers; flag/content assertions |
| MDREVIEW-12 Round lifecycle | PASS | 4/5 | Reload keeps stale selection; mtime+size race |
| MDREVIEW-13 QA/release | PASS | 4/5 | Manual evidence unverifiable here; stale state/doc wording |

**Overall: 4 PASS, 9 PARTIAL, 0 FAIL. No critical or major finding.** The three release blockers from the 21:38 QA report (save portability, custom-sidecar resolve commands, orphan-END data loss) are verified fixed, as are modal mouse blocking, result-file sentinels, `qa.sh` report validation, and the complete gate summary.

**Global gate checks** (apply to every task below unless noted): `[x]` tests pass — gate log: 84 library + 15 CLI + 31 TUI + 1 performance = **131 passed, 0 failed**; `[x]` clippy `-D warnings` clean; `[x]` `cargo fmt --check` clean; `[x]` no `todo!`/`unimplemented!`/TODO/FIXME in `src/` or `tests/`; `[x]` clean tree at HEAD `91b9d57`; `[x]` static-pie release binary confirmed.

---

## [MDREVIEW-1] Harness, Rust skeleton, and static build
Status: PASS? — **PARTIAL**
Score: 4/5

### Checks
- [x] `init.sh`, `scripts/env.sh`, `.cargo/config.toml`, harness scripts, `PROMPT.md`, and state files exist; `bash -n` passes.
- [x] `tasks.json` valid, ordered, dependency-consistent; all 13 tasks `done`; progress fresh.
- [x] Gate validates placeholders, fmt, clippy, tests; prints all test result lines (`harness/gate.sh:136-144`).
- [x] `qa.sh` validates the model invocation, filters `[claude-code:...]`, requires the report heading, and atomically replaces the report (`harness/qa.sh:98-117`).
- [x] Release binary is static-pie (`file` confirmed this pass).
- [ ] Spec 001's Test section is not automated: no invalid-task-JSON negative gate test, no schema validation, no separate shell-sweep test. `harness/templates/task-schema.json` is unused; `gate.sh:17-45` hand-rolls a weaker check.
- [ ] `harness/plan.sh:22-48` does not validate or persist `state/fix_plan.md`; it relies on the model's file tools.
- [ ] `harness/loop.sh:104` treats an exit-0 error line from `claude` as a successful session.
- [ ] Gate does not run `init.sh` or verify the static release binary, both cited by specs 001/009.

### Issues Found
- **HARNESS-1 (minor):** add a negative gate test with a temp `tasks.json` that must fail, plus schema validation or documented hand-rolled rules.
- **HARNESS-2 (minor):** validate/persist `plan.sh` output and validate `loop.sh` model output, using the `qa.sh` capture/validate/mv pattern.
- **HARNESS-3 (minor):** add `init.sh` and static-binary checks to the release gate or checklist.
- **Note:** `state/progress.md` ends with “Final QA rerun and release evidence: in progress,” and the design doc status says the same; refresh both after this pass.

### Notes
The harness structure and gate are sound. The residual work is automation and report integrity, not skeleton functionality.

---

## [MDREVIEW-2] CLI, config, and sidecar storage
Status: PARTIAL
Score: 4/5

### Checks
- [x] All commands/flags from spec 002 exist; hidden `__tui` is wired and hidden.
- [x] Exit codes 0/10/1/2/3 and tmux sentinels 20/21/22 map correctly.
- [x] Default/custom sidecar, exclusive lock, atomic temp+sync+rename, corrupt backup, unknown-version rejection (compatible-body case), request/status/export/resolve flows, export immutability, and config/CLI precedence in code.
- [x] CLI tests cover version/help, flows, wait/timeout, missing sidecar, tmux errors, install, lock, invalidation, signoff clearing, custom sidecar, rounds, export immutability, and human status.
- [ ] `run_resolve` never loads or stats the Markdown file (`src/cli.rs:404-421`). With a valid sidecar, `mdreview resolve missing.md c1` succeeds and exits 0, contradicting the “missing file = 2” exit-code table.
- [ ] `export` takes an exclusive lock (`src/cli.rs:314`) although spec 004 reserves the lock for a TUI session or mutating command; it fails with exit 2 while a review is open, and README/spec do not document the intended handshake behavior.
- [ ] Missing spec-002 tests: clap matrix for every command/flag, config precedence, unknown-key stderr, direct atomic replacement, status after resolve, unknown resolve ID → 2, missing-file resolve → 2, stale-hash `wait`.
- [ ] An unknown future version with an incompatible body is deserialized before the version check; `load_or_recover` then backs it up and starts fresh instead of rejecting it (`src/review/mod.rs:226-275`).
- [ ] `status`/`export` use the sidecar's stored `file` rather than the invoked path; a copied sidecar reports another document.

### Issues Found
- **CLI-RESOLVE-1 (minor):** validate the Markdown file before resolving; add a deleted-file test.
- **CLI-EXPORT-1 (minor):** remove the export lock or document/test it as handshake policy.
- **REV-VERSION-1 (minor):** probe `version` before full deserialization; treat only JSON errors as corruption.
- **CLI-TESTS-1 (minor):** add the missing spec-002 matrix.
- **CLI-FILE-1 (minor):** validate or document the stored `file` mismatch.

### Notes
The sidecar lifecycle is coherent; the missing-file resolve path is the only behavioral defect found.

---

## [MDREVIEW-3] Markdown document model and source map
Status: PARTIAL
Score: 4/5

### Checks
- [x] pulldown-cmark GFM options and `into_offset_iter()` are used.
- [x] Heading paths include the heading and inherit to following blocks.
- [x] Mermaid fences, image-only paragraphs, nested lists/quotes, tables, HTML, rules, footnotes, tasks, emphasis, links, and Unicode are handled.
- [x] `line_of`, `lines_of`, `source_line`, and `block_at` round-trip; layout wraps on display width, keeps source maps, expands tabs, splits graphemes, and reserves media rows.
- [x] Tests cover empty/CRLF/no-trailing-newline, tasks/rules/footnotes/nested lists, per-cell ranges, block lookup, CJK/emoji, tabs, and remote-image link layout.
- [ ] Empty table cells are dropped: `push_inline` returns early on empty text (`src/document/mod.rs:271-275`), so empty cells have no inline/source range, contrary to spec 003:50.
- [ ] `tests/fixtures/spec.md` does not cover the acceptance list (nested/task lists, blockquote, links, Rust fence, rules, HTML, footnote, emphasis, Unicode/wide text).
- [ ] Wrapping is tested at 12/6/4 columns, not the spec's 20/40/80 matrix (`src/document/layout.rs:357-432`).
- [ ] Consecutive paragraphs inside a list item/footnote are merged without a separator (`src/document/mod.rs:655-665`): a loose item's “first” and “second” paragraphs become `firstsecond`. Blocks inside lists after the item marker also miss depth indentation (`src/document/layout.rs:126-139`), and every list item gets a blank separator (`:52-55`).
- [ ] Control characters are not sanitized; ESC/OSC text reaches the rendered and source views.
- [ ] An image-only paragraph inside a blockquote/list item is not promoted to `BlockKind::Image` (only `BlockKind::Paragraph` is converted, `src/document/mod.rs:299-306`).
- [ ] Multiple images in an image-only paragraph keep only the last; the `image_src` field is dead code.

### Issues Found
- **DOC-EMPTY-CELL-1 (minor):** push an empty inline with the cell range, or exempt empty cells explicitly.
- **DOC-FIXTURE-1 (minor):** expand the fixture and add exact 20/40/80-column assertions.
- **DOC-LIST-1 (minor):** insert paragraph separators, indent nested list blocks, and suppress blank lines between adjacent list items; add tight-list tests.
- **DOC-SANITIZE-1 (minor, security hardening):** replace/strip C0 controls except tab/newline in both views.
- **DOC-IMAGE-1 (minor):** define the blockquote/list image-only behavior and the multiple-image rule.
- **DOC-SOURCE-1 (minor):** verify inline Code source ranges — the pulldown-cmark `Code` event range includes the delimiters, so the layout’s per-grapheme source mapping can be offset within code spans.

### Notes
The parser is materially complete. The remaining defects are edge fidelity and test-fixture completeness.

---

## [MDREVIEW-4] Review model, anchors, rounds, and re-anchoring
Status: PASS
Score: 4/5

### Checks
- [x] Sidecar schema v1, stable `cN` IDs, monotonic `next_id`, timestamps, signoff/approval/request/draft fields.
- [x] Anchors store quote, 32-char prefix/suffix, heading path, block index, byte/line ranges, and doc hash; trailing CR/LF is trimmed.
- [x] Exact, duplicate-context, whitespace-normalized, and heading/Jaccard ≥ 0.5 re-anchoring, then stale; resolved items skipped; suggestions become stale; stale items reopen; reattach refreshes suggestion originals.
- [x] Rounds increment only for a changed signed-off revision; approval is bound to the current hash and cleared on feedback; resolve/unresolve/remove work.
- [x] Tests cover the anchor matrix, staleness, rounds/approval, sidecar round-trip, lock, corrupt backup, unknown version, and reattach.
- [ ] Version is checked after full deserialization, so an incompatible future schema is treated as corruption (`src/review/mod.rs:226-275`).
- [ ] No heading-path-gone Jaccard rejection test.
- [ ] `duplicate_quotes_use_context` asserts only that “hello” is present, not which duplicate was chosen; `whitespace_and_block_fallback` does not assert the `normalized` method (`src/review/mod.rs:944-970`).
- [ ] No explicit `resolved_round`/history assertion in `resolve_unresolve_and_remove`.

### Issues Found
- **REV-VERSION-1 (minor):** probe version first; only JSON parse errors should recover.
- **REV-TEST-1 (minor):** strengthen the duplicate/whitespace assertions and add the heading-gone test.

### Notes
The core multi-round contract is correct; no functional defect found.

---

## [MDREVIEW-5] Prompt and JSON export
Status: PARTIAL
Score: 4/5

### Checks
- [x] Header includes file, round, hash, status, and open/stale counts, with open counts split by kind.
- [x] Items ordered by `source_start` then numeric ID; numbering is sequential and independent of IDs.
- [x] Quotes are `>`-prefixed with blank lines as `>`; bodies and suggestions are verbatim; approved output and approved `--all` archived output are correct.
- [x] Empty/stale-only exports are clear; JSON shape/filtering and exit codes 10/0/2 work.
- [x] Custom-sidecar resolve commands include a quoted `--sidecar` (`src/review/export.rs:78-94`); the TUI/tmux/wait/export callers pass it.
- [x] Tests cover the comment golden, approved output, approved `--all`, quote blanks/fences, ordering, empty/stale-only, JSON, exit codes, and path quoting.
- [ ] Spec 005 requires a golden prompt for comments **and suggestions**; the suggestion test only asserts substrings (`src/review/export.rs:386-398`). The “escaping” test does not test quoted `>` content.
- [ ] With `--all` in feedback status, the resolve command includes already-resolved IDs and resolved entries sit under “Address the items below” (`src/review/export.rs:78-96`).
- [ ] The TUI suggestion editor cannot set a comment body, so the exported suggestion `Comment:` path is unreachable from the UI (model/export support it).
- [ ] The prompt uses `store.file`, not the path supplied to `export`.

### Issues Found
- **EXPORT-SUGGESTION-1 (minor):** add a full suggestion golden and quoted-`>` coverage.
- **EXPORT-ALL-1 (minor):** filter resolved IDs from the feedback resolve command and label archived entries.
- **EXPORT-FILE-1 (minor):** validate or override the stored file path.

### Notes
Normal default-sidecar export is stable and correct; the custom-sidecar blocker is fixed.

---

## [MDREVIEW-6] TUI foundation, navigation, outline, search, help
Status: PARTIAL
Score: 4/5

### Checks
- [x] Header, 2-line footer, responsive 26/34/32-column panes, narrow overlays, and tiny-terminal guard.
- [x] Rendered and F3 source views with line numbers and markers; Vim/standard dispatch; focus cycling; outline, search, help, F2/F3, signoff, quit.
- [x] Portable save/signoff: `F10` and `Ctrl+S` work in the editor; `F10` opens signoff globally (`src/tui/app.rs:981-986, 1294-1320`).
- [x] Event-loop/terminal errors autosave and map to abort exit 1 (`src/tui/mod.rs:39-47`).
- [x] TestBackend at 80x24, 100x30, 120x40; narrow/tiny, mouse, footer, gutter, signoff, modal protection.
- [ ] `Ctrl+[` previous-heading is unreachable: terminals report it as ESC, which is consumed earlier (`src/tui/app.rs:987-995, 1110-1112`). Standard mode has no working keyboard previous-heading binding; spec 006/README advertise it.
- [ ] The help overlay clips content at 80x24 and 100x30 (26 lines in a 17/22-row inner area, `src/tui/render.rs:466-503`) and has no scrolling.
- [ ] Signal-only terminal close (SIGHUP/SIGTERM) is not handled; only event-loop errors take the abort/autosave path.
- [ ] Missing tests: full standard keymap, keyboard outline/help, quit-autosave, corrupt-sidecar recovery status, click-outside-ignored.
- [ ] Search underlines whole lines rather than match spans; source-view long lines are clipped with no horizontal scroll.
- [ ] An empty document does not show “Empty document” after `rebuild` inserts a blank line (`src/tui/render.rs:171-173`, `src/tui/app.rs:286-289`).

### Issues Found
- **TUI-CTRLBRACKET-1 (minor):** choose a feasible binding and update spec/README, or document the limitation.
- **TUI-HELP-1 (minor):** size or scroll the help overlay; also advertise `Ctrl+S` in the hints.
- **TUI-CLOSE-1 (minor):** add signal handling or a PTY close test.
- **TUI-RELOAD/SEARCH-1 (minor):** clear/recompute selection and search on rebuild; consider span-level highlighting and source wrapping.
- **TUI-EMPTY-1 (minor):** make the empty-document message reachable.

### Notes
Layout, navigation, and portability are solid. No blocking defect.

---

## [MDREVIEW-7] Comments and suggested edits in the TUI
Status: PARTIAL
Score: 4/5

### Checks
- [x] Add/edit/delete/resolve comments; create/edit suggestions; line/block selection mapping; images accept comments and reject suggestions.
- [x] Draft autosave on editor keys, draft resume, autosave on mutations, signoff persistence.
- [x] Save failures are not overwritten by success statuses; reattach refreshes a suggestion’s original.
- [x] Tests cover comment create/edit/resolve/delete, suggestion create, F10/Ctrl+S save, draft resume, selection mapping, reattach, and suggestion survival after unrelated edits.
- [ ] The suggestion editor has no comment/body field; UI-created suggestions always have an empty body (`src/tui/app.rs:1344-1353`), so spec 005’s suggestion `Comment:` section is unreachable from the TUI.
- [ ] A resumed draft keeps its old anchor when the document changed (`src/tui/app.rs:213-232`); saving can anchor to unrelated text.
- [ ] No suggestion edit/delete test, no image-suggestion-rejection test, and no review-panel `X` unresolve test.
- [ ] Bodies/replacements lose all trailing newlines (`src/tui/app.rs:254-274, 1326-1331`), including deliberate code-block trailing lines.

### Issues Found
- **SUGG-COMMENT-1 (minor):** add an optional suggestion comment field or document the omission.
- **SUGG-DRAFT-1 (minor):** re-anchor or discard a draft whose `doc_hash` differs, with a status message.
- **SUGG-TRAILING-1 (minor):** trim at most one newline.
- **SUGG-TESTS-1 (minor):** add suggestion edit/delete, image rejection, and `X`-unresolve tests.

### Notes
The normal comment/suggestion lifecycle is correct; the previous false-stale defect remains fixed.

---

## [MDREVIEW-8] Mouse selection and hit-testing
Status: PASS
Score: 5/5

### Checks
- [x] Click places the cursor; drag selects a line range; wheel scrolls three lines and survives redraws.
- [x] Gutter marker clicks, outline/review inner-rect clicks, and footer Comment/Suggest/Resolve/Send clicks work.
- [x] Mouse-disabled mode ignores events; clicking a pane moves focus; tiny-terminal/stale layout rects are not hit.
- [x] Modal editor, confirmation, help, and search block mouse actions (`src/tui/app.rs:1376-1387`), regression-tested.
- [x] Clicks outside all known rectangles are ignored in code.
- [ ] No explicit click-outside-ignored test (nit).

### Issues Found
- **MOUSE-1 (minor):** add a one-line click-outside test.

### Notes
No functional issues. This task is complete.

---

## [MDREVIEW-9] Image preview pipeline
Status: PARTIAL
Score: 4/5

### Checks
- [x] Relative/absolute local paths resolve; remote URLs schedule no worker and render as link text; missing paths error.
- [x] PNG/JPEG/WebP/BMP/GIF decode; SVG rasterizes with resvg/usvg and a shared font database.
- [x] 50 MB file, 10,000 px side, and 40 MP limits are enforced; decode-time allocation limits; errors include dimensions where known.
- [x] Environment protocol detection with explicit override; half-blocks fallback; TIOCGWINSZ font metrics with plausible-range checks and no stdio query.
- [x] Decode/encode runs off-thread; visible-only preparation; content-addressed keys; failed-worker caching; resize invalidation; placeholder until ready.
- [x] Tests cover local PNG/JPEG/BMP/GIF/WebP/SVG, missing/remote, limits, protocols, font metrics, cache keys, async completion, partial visibility, and loaded-row labels.
- [ ] Loaded media is drawn at the full doc rectangle and encoded at `area.width`, covering the 2-column comment gutter and using a width 2 cells wider than the text layout (`src/tui/render.rs:41-47, 278-283`; `src/tui/app.rs:406`). Comment markers on image/Mermaid rows are hidden.
- [ ] `resolves_and_encodes_local_png` only asserts a protocol exists (`src/media.rs:584-594`); no dimensions/non-blank pixels. Mermaid tests likewise only assert protocol existence.
- [ ] Auto `mmdc` PATH discovery is untested; the test passes the fake program directly (`src/media.rs:669-685`).
- [ ] First-paint hashing reads/hashes up to 50 MB on the UI thread (`src/media.rs:206-232`); the hash cache trusts mtime+size.
- [ ] Cell pixel size is derived once and is not in the cache key (`src/media.rs:420-448`); resize does not re-derive it.
- [ ] Partially visible images are encoded for the full group and rendered into the visible slice; stale Kitty placements when scrolling out are unverified.
- [ ] Pending results that arrive after `clear_encodings` are retained in the old-key cache rather than discarded (`src/media.rs:90-102, 239-242`).
- [ ] The placeholder label is repeated on all 8/10 reserved rows (`src/document/layout.rs:57-84`).

### Issues Found
- **MEDIA-GUTTER-1 (minor):** render at `x + gutter`, width `width - gutter`, target the content width, and draw markers on loaded media rows; add a TestBackend assertion.
- **MEDIA-PIXELS-1 (minor):** assert PNG/Mermaid dimensions and visible pixels.
- **MEDIA-PATH-1 (minor):** test Auto `mmdc` through a temp `PATH`.
- **MEDIA-PERF-1 (minor):** move hashing off the UI thread; include font size/cache metadata; discard stale pending results.
- **MEDIA-CAPTION-1 (minor):** show the placeholder/caption once, not on every reserved row.

### Notes
The media behavior is close to spec; the gutter/content-width defect is the main UI issue, and test strength is the main coverage gap.

---

## [MDREVIEW-10] Mermaid rendering
Status: PARTIAL
Score: 4/5

### Checks
- [x] Rust renderer is the default; `mmdc` is explicit or an Auto fallback; `source` never renders and starts no worker.
- [x] `mmdc` runs as a program with arguments (no shell), writes a temp SVG, and cleans up.
- [x] SVG → tree → pixmap → PNG with an explicit font database and dimension caps.
- [x] Invalid diagrams render fenced source with a “diagram unavailable” badge; the parser error reaches a free status line.
- [x] Enter toggles only the current Mermaid block; failed renders are cached; keys include engine/theme/source hash/target/protocol.
- [x] Tests cover flowchart, sequence, class, invalid fallback, source engine, Enter toggle, fake `mmdc`, and direct Auto fallback.
- [ ] Spec 007 requires non-empty PNGs with sane dimensions; tests only assert an encoded protocol exists (`src/media.rs:623-644`).
- [ ] Auto `mmdc` via PATH discovery is untested; the direct fake-program test bypasses `command_exists("mmdc")`.
- [ ] The invalid-Mermaid TUI test does not assert the parser error in the status line.
- [ ] `Theme` is included in the cache key but not passed to `mermaid_rs_renderer::render` (`src/media.rs:160-172, 301-318`); `--theme` is a no-op for the Rust renderer.
- [ ] Auto fallback triggers only on a Rust **parse** error, not a raster/encode failure (`src/media.rs:283-315`).
- [ ] Source fallback is confined to 10 rows; long diagrams clip.

### Issues Found
- **MERMAID-PIXELS-1 (minor):** assert width/height and non-transparent pixels for all three fixtures.
- **MERMAID-PATH-1 (minor):** test `load_mermaid` with a fake `mmdc` on the temp `PATH`.
- **MERMAID-THEME-1 (minor):** apply the theme where supported or document that it affects only `mmdc`/cache keys.
- **MERMAID-FALLBACK-1 (minor):** consider falling back on the whole render path, not just the parser.
- **MERMAID-STATUS-1 (minor):** assert the status-line error and test long source fallback.

### Notes
The pipeline matches the spec; test strength and the inert Rust theme are the residual gaps.

---

## [MDREVIEW-11] Agent handshake, tmux adapter, and install recipes
Status: PARTIAL
Score: 4/5

### Checks
- [x] File handshake request/wait/export with exit codes 10/0/3/2; `wait` validates the signoff hash.
- [x] tmux requires `$TMUX`; popup uses `-E -w 90% -h 90% -T`; sentinel 20/21/22 mapping; abort 1; popup failure 2 with fallback; result-file fallback; custom sidecar.
- [x] `--detach` uses `tmux new-window -d -n mdreview`, returns 0, and prints a quoted wait command.
- [x] Claude/Codex/Gemini/generic recipes; marker preservation; idempotency for a single block; partial-marker repair; malformed/non-UTF-8 refusal; orphan-END preservation.
- [x] Fake-tmux tests cover feedback/approved/abort/failure/custom sidecar/detach; `__tui` writes the result file.
- [ ] The real `mdreview review --tmux` popup E2E is only a manual claim in `state/acceptance.md:19,33-35`; no transcript/artifact is stored, and it cannot be reproduced in this session.
- [ ] Multiple complete marker blocks are not collapsed (`src/agent.rs:68-103`); a file with duplicate blocks stays duplicated.
- [ ] Popup flag assertions omit `-h 90%` and `-T`; Codex/Gemini recipe content (`request`/`review`/`export`) is not asserted.
- [ ] No stale-hash `wait` timeout test.
- [ ] The orphan-START repair intentionally drops text after the START marker; documented/tested, but worth a spec note.

### Issues Found
- **AGENT-MARKERS-1 (minor):** collapse or refuse duplicate complete marker blocks; add a test.
- **AGENT-TESTS-1 (minor):** assert the full popup flags, all four recipe contents, and the stale-hash wait.
- **AGENT-EVIDENCE-1 (minor):** preserve a real popup transcript or keep the acceptance wording strictly manual.

### Notes
The adapter logic is sound and the sentinel fallback reduces the tmux-version risk. No functional blocker.

---

## [MDREVIEW-12] Round lifecycle, file changes, stale reattach
Status: PASS
Score: 4/5

### Checks
- [x] Reloading a changed file refreshes the document/outline, clamps cursors, re-anchors items, increments the round after signoff, and saves.
- [x] Unresolved items carry forward; resolved items stay archived; stale items remain visible and reattach to a new selection.
- [x] Approval invalidates on content change; export is read-only (it re-anchors a clone without saving); request advances a signed-off revision before clearing the signoff.
- [x] Tests cover changed-file reload, round increment, resolved archive, stale reattach (including suggestion original), approval invalidation, export immutability, and a two-round CLI flow.
- [x] Save failures during resolve/reattach/reload are not reported as success.
- [ ] `reload_document` does not clear `selection` (`src/tui/app.rs:1527-1561`); a selection made before an external edit can map to unrelated new lines.
- [ ] File changes are detected by mtime+size (`src/tui/app.rs:1695-1698`); same-size/same-mtime edits are missed, and a signoff within the 500 ms poll window binds approval to the old hash (the hash checks in `wait`/`export` prevent a false approval, but the TUI still exits as approved).
- [ ] Search matches are not recomputed after a reload.
- [ ] The draft-across-revision issue is tracked under MDREVIEW-7.

### Issues Found
- **ROUND-SELECTION-1 (minor):** clear the selection on reload; recompute search matches.
- **ROUND-WATCH-1 (minor):** hash on the poll or re-check the hash at signoff; document the 500 ms window.

### Notes
The multi-round lifecycle is coherent and well covered. Only low-risk edge cases remain.

---

## [MDREVIEW-13] End-to-end QA, performance, docs, release
Status: PASS
Score: 4/5

### Checks
- [x] Full gate passes: fmt, clippy `-D warnings`, 131 tests (84/15/31/1), clean tree.
- [x] 2,000-line parse/layout performance test passes (0.04 s).
- [x] `state/acceptance.md` provides a per-feature evidence matrix with names that exist in the suite.
- [x] README covers purpose, build, quick start, commands, workflow, keys, media matrix, sidecar, config, harness, tests, and limitations; CHANGELOG has 0.1.0.
- [x] Static-pie musl binary confirmed.
- [x] All tasks `done`; task progress 100%.
- [x] No placeholders in `src/`; previous three release majors verified fixed by source inspection; modal-mouse, sentinel, `qa.sh`, and gate-summary fixes verified.
- [x] This pass has no critical or major findings, so the spec-009 “no critical/major QA findings” criterion is met once this report is persisted.
- [ ] `state/qa_report.md` is written by `harness/qa.sh` after this run; verify it exists and starts with the expected heading. The previous 21:38 report (3 majors) must not remain current.
- [ ] Manual evidence (real tmux popup, PTY F10 comment/signoff, Kitty/WezTerm pixels) has no stored transcript and cannot be re-run in this headless session; it remains a documented manual check.
- [ ] Performance test threshold is 2 s (`tests/performance.rs:28-31`) while spec 009 says “under 1 second”; actual is 0.04 s.
- [ ] Design-doc status and `state/progress.md` still say the final QA/release evidence is in progress; `MDREVIEW-13.completed_at` (13:26:03Z) predates the 21:54–21:57 fixes/gate.

### Issues Found
- **REL-REPORT-1 (minor, process):** confirm `qa.sh` persists this report; do not leave the old major-finding report in place.
- **REL-EVIDENCE-1 (minor):** keep a transcript for the manual tmux/PTY/pixel checks or keep them explicitly marked manual.
- **REL-DOCS-1 (minor):** refresh the design status, progress entry, and completion timestamp; align the performance threshold.

### Notes
All release artifacts exist and the release criteria are met as of this pass. The remaining items are documentation freshness and manual-evidence traceability.

---

# Consolidated fix plan

The `Write` to `state/fix_plan.md` was denied by the session permission layer; this is the actionable plan that should be written there.

## P1 — minor behavior fixes
1. **CLI-RESOLVE-1:** validate the Markdown file in `run_resolve` (`src/cli.rs:404-421`); test deleted-file → exit 2.
2. **CLI-EXPORT-1:** remove the read-only export lock or document/test it as handshake policy (`src/cli.rs:314`).
3. **REV-VERSION-1:** probe `version` before deserializing; only JSON errors should trigger backup/recovery (`src/review/mod.rs:226-275`); add an incompatible v2 test.
4. **DOC-EMPTY-CELL-1:** emit an empty inline with the cell range or exempt empty cells (`src/document/mod.rs:271-275, 758-765`).
5. **DOC-LIST-1:** separate multi-paragraph list/footnote content, indent nested list blocks, and suppress blank lines between adjacent list items (`src/document/mod.rs:655-665`, `src/document/layout.rs:52-55, 126-139`).
6. **DOC-SANITIZE-1:** sanitize C0/ESC/OSC in rendered and source views; add a test.
7. **MEDIA-GUTTER-1:** render media at `x+gutter`/`width-gutter`, target content width, and keep gutter markers visible (`src/tui/render.rs:41-47, 278-283`; `src/tui/app.rs:406`).
8. **SUGG-DRAFT-1:** re-anchor or discard a resumed draft across revisions (`src/tui/app.rs:213-232`).
9. **SUGG-COMMENT-1:** add an optional suggestion comment field or document that UI suggestions are title-less (`src/tui/app.rs:1344-1353`).
10. **TUI-CTRLBRACKET-1:** use a feasible standard previous-heading binding and update spec/README (`src/tui/app.rs:1110-1112`).
11. **TUI-HELP-1:** fit or scroll the help overlay; add `Ctrl+S` to hints (`src/tui/render.rs:466-503`).
12. **TUI-CLOSE-1:** add SIGHUP/SIGTERM handling or a PTY close test.
13. **TUI-RELOAD-1:** clear selection and recompute search on reload (`src/tui/app.rs:1527-1561`).
14. **AGENT-MARKERS-1:** collapse/refuse duplicate complete marker blocks (`src/agent.rs:68-103`).

## P2 — spec-mandated / high-value tests
- **001:** invalid-task-JSON negative gate test; schema validation or documented hand-rolled rules; `plan.sh`/`loop.sh` output validation; init/static-binary gate checks.
- **002:** clap matrix, config precedence, unknown-key stderr, atomic replacement, status after resolve, unknown resolve ID → 2, missing-file resolve → 2, stale-hash wait.
- **003:** full acceptance fixture; exact 20/40/80 wrapping; empty table cell; multi-paragraph list item; control-character test.
- **004:** heading-path-gone Jaccard rejection; assert the duplicate candidate and `normalized` method; resolved_round/history.
- **005:** full suggestion golden; quoted-`>` escaping; `--all` feedback archival/ID filtering; stored-file path.
- **006/007:** full standard keymap; keyboard outline/help; quit-autosave; corrupt-recovery status; click-outside; suggestion edit/delete; image-suggestion rejection; draft across revision.
- **007:** PNG and Mermaid dimensions/non-blank pixels; Auto `mmdc` via temp `PATH`; invalid-Mermaid status line; gutter/content-width rendering; partial-scroll/placements.
- **008:** full popup flags; Codex/Gemini recipe content; stale-hash wait; real-popup transcript.
- **009:** persist/verify `state/qa_report.md`; align performance threshold; refresh design status/progress/completion timestamp.

## P3 — code-quality nits
- Remove unused `BlockParser::image_src` (`src/document/mod.rs:346,377,635`).
- Deduplicate `ranges_overlap`/`lines_overlap` (`src/tui/app.rs:1675-1681`) and the timestamp helpers in `src/review/mod.rs:629-648`.
- Capture `now()` once in `reattach` (`src/review/mod.rs:444-451`).
- Move media hashing off the UI thread and include cell pixel size in the cache key/refresh (`src/media.rs:206-232, 420-448`).
- Discard stale pending media results after `clear_encodings` (`src/media.rs:90-102, 239-242`).
- Consider Auto `mmdc` fallback for raster/encode failures, not only parse errors (`src/media.rs:283-315`).
- Make the empty-document message reachable (`src/tui/render.rs:171-173`, `src/tui/app.rs:286-289`).
- Disable mouse capture if `Terminal::new` fails after capture (`src/tui/mod.rs:68-75`).

## Release decision
**Release is not blocked.** The gate is clean, all tasks are done, the static binary is confirmed, and this pass has no critical or major findings. The remaining work is minor behavior polish, test strength, and documentation/state refresh. Verify `state/qa_report.md` is persisted by the harness before closing the release.
