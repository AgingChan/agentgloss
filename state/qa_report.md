[claude-code:unrecognized_model] {"model":"deepseek-flash[1m]","query_source":"sdk"}
Report persisted to `state/fix_plan.md`. Here is the evaluation.

# mdreview QA Evaluator Pass

## Evidence

- **Gate at pass start:** tasks JSON valid; shell syntax OK; no placeholders; `cargo fmt` OK; `cargo clippy --all-targets --all-features -- -D warnings` OK; `cargo test --all-features` OK. Gate: **0 errors, 2 warnings** (stale `in_progress` MDREVIEW-13; dirty tree). `GATE: PASS`.
- **Raw gate logs:** `/tmp/mdreview-test.log` → **82 library + 15 CLI + 1 performance + 30 TUI = 128 passed, 0 failed, 0 ignored**; fmt log empty; clippy log only `Finished` (no warnings).
- **Static count:** 128 `#[test]` attributes, matching the claimed 82/15/30/1 split.
- **Placeholders:** none in `src/` or `tests/`.
- **Release artifact:** `target/x86_64-unknown-linux-musl/release/mdreview` is `ELF 64-bit LSB pie executable, x86-64, static-pie linked`.
- **Repo state:** HEAD `7f36029` (20:28), 20 uncommitted changes.
- **Limitation:** `cargo`, `./init.sh`, and `./harness/gate.sh` were blocked by the command-approval layer, so I used the gate's own freshly written logs and direct reads (`file`, grep, git). No claim below relies on an unread command output.

## Verdict

**3 PASS, 9 PARTIAL, 0 FAIL** across the 12 completed tasks. No functional blocker like the prior SUGGEST-1 remains; the release is blocked by MDREVIEW-13, not by tasks 1–12.

| Task | Status | Score | Main gap |
| --- | --- | --- | --- |
| MDREVIEW-1 Harness/skeleton | PARTIAL | 4/5 | No automated harness tests; `qa.sh` persists a CLI error as the QA report |
| MDREVIEW-2 CLI/config/sidecar | PARTIAL | 4/5 | Spec 002 test list incomplete |
| MDREVIEW-3 Document model | PARTIAL | 4/5 | Empty table cells dropped; fixture/width coverage |
| MDREVIEW-4 Review model | PASS | 4/5 | Stale-repeat history deviation |
| MDREVIEW-5 Export | PARTIAL | 4/5 | Suggestion golden missing; `--all` resolve list |
| MDREVIEW-6 TUI foundation | PARTIAL | 4/5 | External-close path exits 2, not 1; test gaps |
| MDREVIEW-7 Comments/suggestions | PARTIAL | 4/5 | Suggestion edit/delete tests missing |
| MDREVIEW-8 Mouse | PASS | 5/5 | Nit: click-outside test |
| MDREVIEW-9 Images | PARTIAL | 4/5 | PNG pixel assertion; placeholder dimensions; UI-thread hashing |
| MDREVIEW-10 Mermaid | PARTIAL | 4/5 | Rendered dimensions/pixels unasserted; Auto `mmdc` via PATH |
| MDREVIEW-11 Agent/tmux/recipes | PARTIAL | 4/5 | Real tmux exit propagation unverified |
| MDREVIEW-12 Round lifecycle | PASS | 5/5 | Nits only |

## Release-blocking findings

### QA-1 (MAJOR, MDREVIEW-1 + 13): `qa.sh` records a failed model invocation as a successful QA report

- **Evidence:** `state/qa_report.md` contains only `[claude-code:unrecognized_model] {"model":"deepseek-flash[1m]","query_source":"sdk"}`. `harness/logs/qa-20261121-210548.log` and `qa-20261121-211007.log` are both 85 bytes and contain the same line.
- **Cause:** `harness/qa.sh:92` tees `claude --print` output into the report and does not detect the CLI error.
- **Fix:** require exit 0 and non-empty/valid report output; write atomically and exit non-zero on failure. Add a fake-`claude` test that prints an error and exits 0.

### QA-2 (MAJOR, MDREVIEW-13): Release state is not shipped

MDREVIEW-13 is `in_progress`, `harness/report.sh` would show 92%, and 20 QA-fix files are uncommitted. Finish the QA items, mark 13 done, commit, and re-run the gate with a clean tree.

### QA-3 (MAJOR, MDREVIEW-11 + 13): Real tmux popup exit propagation is unverified

The adapter depends on `tmux display-popup -E` propagating sentinels 20/21/22, but all tests use a fake `tmux`. `state/acceptance.md:33-35` claims a real popup was exercised in the PTY smoke test; `state/progress.md:19` only documents fake-tmux command construction and a real TUI pane. Add a recorded real-popup run or implement a result-file fallback.

## [MDREVIEW-1] Harness, Rust skeleton, and static build
Status: PARTIAL · Score: 4/5

### Checks
- [x] Harness scripts, `env.sh`, `.cargo/config.toml`, state files, and static-pie binary exist.
- [x] Gate validates tasks, placeholders, fmt, clippy, and tests; it passed.
- [x] HARNESS-1 heredoc/backtick bug is fixed (`harness/qa.sh:53` uses `cat <<'PROMPT'`).
- [x] `PROMPT.md` defines the required session protocol.
- [ ] No automated tests for spec 001: shell sweep, deliberately invalid task JSON, schema validation.
- [ ] `harness/templates/task-schema.json` is unused; gate hand-rolls validation.
- [ ] `qa.sh` report persistence is broken (QA-1).

### Issues
- **HARNESS-2 (minor):** add a harness test/gate check for syntax, invalid JSON failure, and schema validation.
- **HARNESS-3 (nit):** gate does not check the static release binary or run `init.sh`.

### Notes
Structure is sound; remaining work is automation and report integrity.

## [MDREVIEW-2] CLI, config, and sidecar storage
Status: PARTIAL · Score: 4/5

### Checks
- [x] All spec 002 commands/flags exist; hidden `__tui` is wired.
- [x] Exit codes 0/10/1/2/3 and tmux sentinels map correctly.
- [x] Default/custom sidecar, lock, atomic temp+sync+rename, corrupt backup, strict version rejection.
- [x] Request/status/export/resolve flows; export is read-only and locks; status invalidates stale approvals.
- [x] Printed handoff commands shell-quote paths.
- [x] Integration tests cover the main flows and error codes.
- [ ] Missing: clap matrix for every flag, config precedence, unknown-key stderr, atomic-replacement assertion, status-after-resolve, resolve-unknown-ID exit 2.
- [ ] `hash_file` (`src/cli.rs:421-425`) is unused.
- [ ] `changed`/`invalidated` statuses are not defined in spec 002.

### Issues
- **CLI-1 (minor):** add the missing spec-002 tests.
- **CLI-2/3 (nit):** remove `hash_file`; document the extra statuses.
- **CLI-4 (nit):** document that read-only export can fail with exit 2 while a TUI holds the lock.

### Notes
Sidecar lifecycle and locking are coherent; config precedence is implemented but untested.

## [MDREVIEW-3] Markdown document model and source map
Status: PARTIAL · Score: 4/5

### Checks
- [x] GFM options and offset iterator; heading paths; Mermaid/image detection; nested depths.
- [x] Tables are one block with per-cell inlines; rules/HTML/footnotes/task lists/emphasis/links/Unicode handled.
- [x] Byte/line round-trips; `block_at` handles gaps/EOF; layout wraps with source maps, tabs, graphemes, media rows.
- [x] Tests cover empty/CRLF/no-newline, task/rule/footnote/nested-list, ranges, CJK/emoji, tabs, remote-image layout.
- [ ] Empty table cells are dropped (`src/document/mod.rs:271-275`), so they have no inline/source range.
- [ ] `tests/fixtures/spec.md` does not cover the spec 003 acceptance list.
- [ ] Wrapping is tested at 12/6/4, not the spec's 20/40/80.

### Issues
- **DOC-1 (minor):** push an empty inline for empty table cells or exempt them in spec 003.
- **DOC-2 (minor):** expand the fixture and add exact 20/40/80-width wrapping assertions.
- **DOC-3 (nit):** define behavior for multiple images in one paragraph (currently only the last is kept).

### Notes
The parser is materially stronger than the prior pass; remaining items are edge cases and fixture completeness.

## [MDREVIEW-4] Review model, anchors, rounds, and re-anchoring
Status: PASS · Score: 4/5

### Checks
- [x] Sidecar schema v1, stable IDs, timestamps, optional fields.
- [x] Anchors store quote, 32-char prefix/suffix, heading path, block, byte/line ranges, hash; trailing CR/LF trimmed.
- [x] Exact-unique, exact-multiple with context/distance, whitespace-normalized, heading/Jaccard ≥ 0.5, then stale.
- [x] Resolved items skipped; history appended; round increments on signed-off change; approval hash-bound.
- [x] Suggestion staleness, automatic reopen, reattach original refresh.
- [x] Anchor matrix, sidecar, corrupt backup, version, lock, and reattach tests pass.
- [ ] A still-unfindable stale item does not append a new history event (`src/review/mod.rs:532-546`), contrary to spec 004's "every re-anchor appends an event."
- [ ] No test for Jaccard rejection when the old heading path is gone.

### Issues
- **REV-1 (minor):** append a deduplicated `still_stale` event or amend the spec.
- **REV-2 (nit):** add the heading-path-gone stale test.

### Notes
Core multi-round contract is implemented and regression-covered.

## [MDREVIEW-5] Prompt and JSON export
Status: PARTIAL · Score: 4/5

### Checks
- [x] Header, counts, status, ordering, numbering, quote formatting, verbatim bodies.
- [x] Approved output and approved `--all` archived listing with no action instructions.
- [x] Empty/stale-only clarity; JSON shape/filtering; exit codes.
- [x] Resolve paths are shell-quoted.
- [x] Tests cover comment golden, approved, `--all`, blanks/fences, ordering, empty/stale, JSON, quoting.
- [ ] No full suggestion golden; only substring assertions (`src/review/export.rs:365-377`).
- [ ] Feedback `--all` includes resolved IDs in the resolve command and under "Address the items below."

### Issues
- **EXP-1 (minor):** add a suggestion golden.
- **EXP-2 (nit):** filter resolved IDs from the resolve command or document `--all` as a raw dump.

### Notes
Normal agent-flow output matches spec.

## [MDREVIEW-6] TUI foundation, navigation, outline, search, help
Status: PARTIAL · Score: 4/5

### Checks
- [x] Header, 2-line footer, responsive panels, overlays, tiny-terminal guard.
- [x] Rendered and raw-source views with line numbers and markers.
- [x] Vim/standard dispatch, focus cycling, outline/search/help, F2/F3, signoff.
- [x] Esc prioritizes closing a narrow panel; loaded media rows omit labels; tiny layout is cleared.
- [x] TestBackend coverage at 80x24, 100x30, 120x40.
- [ ] External close propagates an event-loop error (`src/tui/mod.rs:39-43`) and `src/cli.rs:219` skips the final save: exits 2 instead of abort 1.
- [ ] Missing tests: full standard keymap, keyboard outline/help, quit-autosave, corrupt-sidecar status, click-outside.
- [ ] Search underlines whole lines, not match spans.
- [ ] Source view clips long lines.

### Issues
- **TUI-1 (major):** handle event-loop errors as a saved abort (exit 1).
- **TUI-2 (minor):** add the missing tests.
- **TUI-3/4 (nit):** span-level search highlight; wrap or document source clipping.

### Notes
Layout/navigation are solid; the error-path exit contract is the behavioral gap.

## [MDREVIEW-7] Comments and suggested edits in the TUI
Status: PARTIAL · Score: 4/5

### Checks
- [x] Add/edit/delete/resolve comment; add suggestion; images reject suggestions.
- [x] Source-line selection; trimmed anchors; suggestion originals from the anchor quote.
- [x] Draft autosave/resume; item autosave; signoff persistence.
- [x] Save failures no longer masked by success status.
- [x] SUGGEST-1 fixed and regression-tested; reattach refreshes the original.
- [ ] No suggestion edit or delete test.
- [ ] `edit_selected_item` cannot edit a suggestion's original.
- [ ] No image-suggestion-rejection test.

### Issues
- **SUGG-1 (minor):** add suggestion edit/delete/rejection tests.
- **SUGG-2 (minor):** document reattach as the way to change the original, or support editing it.

### Notes
Normal-path lifecycle is correct; the prior blocker is gone.

## [MDREVIEW-8] Mouse selection and hit-testing
Status: PASS · Score: 5/5

### Checks
- [x] Click, drag, wheel (three lines), gutter click, outline/review inner-rect click, footer buttons.
- [x] Mouse-disabled mode ignores events.
- [x] Pane clicks move focus; tiny-terminal layout is cleared.
- [x] Tests cover all required mouse behaviors.

### Issues
- **MOUSE-1 (nit):** add a click-outside-ignored regression test.

### Notes
No blocking or functional issues.

## [MDREVIEW-9] Image preview pipeline
Status: PARTIAL · Score: 4/5

### Checks
- [x] Relative/absolute local paths; remote URLs are rejected, worker-free, and link-styled.
- [x] PNG/JPEG/WebP/BMP/GIF/SVG support; shared font database; limits; protocol detection/override; TIOCGWINSZ font size.
- [x] Off-thread media, visible-block prep, content-addressed cache, invalidation, placeholder, no text over loaded images.
- [x] Tests cover formats, missing/remote, limits, protocols, fonts, cache, async, partial visibility.
- [ ] PNG test does not decode/assert dimensions/pixels (`src/media.rs:584-594`).
- [ ] Oversized placeholder lacks dimensions and repeats the label on all 8 rows.
- [ ] `image_stamp` hashes up to 50 MB on the UI thread on first paint.
- [ ] mtime+size hash cache can miss same-size/same-mtime changes.

### Issues
- **MEDIA-1 (minor):** add PNG dimensions/non-blank-pixel assertions.
- **MEDIA-2 (minor):** put dimensions in the placeholder and render the caption once.
- **MEDIA-3 (minor):** move hashing off the UI thread.
- **MEDIA-4 (nit):** document or harden metadata-based cache trust.

### Notes
Behavior is close to spec; gaps are test strength and UI polish.

## [MDREVIEW-10] Mermaid rendering
Status: PARTIAL · Score: 4/5

### Checks
- [x] Rust default; `mmdc` explicit/Auto fallback; `source` never renders.
- [x] `mmdc` runs with arguments, temp SVG, cleanup.
- [x] SVG rasterization with font database and dimension caps.
- [x] Invalid diagrams show source/badge; Enter toggles only the current block; failed renders cached.
- [x] Tests cover flowchart/sequence/class, invalid, source, Enter, fake `mmdc`, Auto fallback.
- [ ] Tests only assert an encoded protocol exists, not sane dimensions/non-empty pixels (`src/media.rs:624-644`).
- [ ] Auto fallback is not tested through PATH discovery (`src/media.rs:671-685` bypasses `command_exists`).
- [ ] Invalid-Mermaid test does not assert the parser-error status line.

### Issues
- **MERMAID-1/2 (minor):** add dimension/pixel assertions and an end-to-end PATH test.
- **MERMAID-3 (nit):** assert the status-line parser error.

### Notes
Pipeline matches spec; test strength is the gap.

## [MDREVIEW-11] Agent handshake, tmux adapter, and install recipes
Status: PARTIAL · Score: 4/5

### Checks
- [x] Request/wait/export handshake and exit codes; wait validates the signoff hash.
- [x] tmux `$TMUX` requirement, popup flags, sentinel mapping, abort/failure paths, custom sidecar.
- [x] Detach command and quoted wait command.
- [x] Four recipes; marker preservation; idempotency; partial-marker repair; malformed/non-UTF-8 refusal.
- [x] Tests cover popup feedback/approved/abort/failure, custom sidecar, detach, handshake, recipes, malformed files.
- [ ] Real tmux sentinel propagation unverified (QA-3).
- [ ] Codex/Gemini recipe content not asserted.
- [ ] Spec 008 says all writes preserve content, but Claude/generic files are replaced; only README documents this.
- [ ] Popup flags `-h 90%`/`-T` not asserted.

### Issues
- **AGENT-1 (major):** resolve QA-3 or add the result-file fallback.
- **AGENT-2 (minor):** assert all four recipe contents.
- **AGENT-3 (minor):** amend spec 008 or preserve content.
- **AGENT-4 (nit):** assert the full popup flag set.

### Notes
Adapter logic is sound under the assumed tmux behavior; that assumption is the main risk.

## [MDREVIEW-12] Round lifecycle, file changes, stale reattach
Status: PASS · Score: 5/5

### Checks
- [x] Reload refreshes document/outline, clamps cursors, reanchors, increments round after signoff, saves.
- [x] Unresolved carry forward; resolved archive; stale items visible and reattachable.
- [x] Approval invalidation; read-only export; request advances signed-off revisions before clearing.
- [x] Tests cover reload, round increment, archive, reattach (including suggestion), invalidation, export immutability, two-round flow.
- [x] Save failures no longer produce success status.

### Issues
- **ROUND-1 (nit):** clear `selection` on reload.
- **ROUND-2 (nit):** hash on poll or at signoff to close the mtime/size and narrow signoff race.

### Notes
Multi-round lifecycle is coherent and covered.

## [MDREVIEW-13] End-to-end QA, performance, docs, release
Status: IN PROGRESS · not graded

### Present
- [x] README, CHANGELOG, 2,000-line performance test, static binary, 128 passing tests, acceptance matrix.

### Release blockers
- [ ] MDREVIEW-13 `in_progress`; 20 uncommitted changes; gate shows 2 warnings.
- [ ] `harness/report.sh` would show 92%, not 100%.
- [ ] `state/qa_report.md` invalid (QA-1).
- [ ] Real tmux popup unverified; acceptance overstates it (QA-3).
- [ ] Missing spec-mandated tests (below).
- [ ] Commit, mark 13 done, re-run gate clean.

## Missing spec-mandated tests (consolidated)

- **001:** shell sweep; invalid-JSON gate failure; schema validation.
- **002:** clap matrix; config precedence; unknown-key stderr; atomic replacement; status after resolve; unknown resolve ID.
- **003:** exact 20/40/80 wrapping; comprehensive fixture; empty-cell behavior.
- **004:** heading-path-gone Jaccard rejection; resolve/unresolve history assertions.
- **005:** suggestion golden; feedback `--all` resolved-ID behavior.
- **006:** full standard keymap; keyboard outline/help; quit-autosave; corrupt-sidecar status; click-outside.
- **007:** PNG dimension/pixels; 40 MP pipeline; placeholder dimensions; Auto `mmdc` via PATH.
- **008:** Codex/Gemini recipe content; full popup flags; real tmux propagation.
- **009:** clean commit; 100% report; valid QA report; release checklist completion.

## Spec / documentation issues
- Spec 004 "every re-anchor appends a history event" vs. the deduplicated stale event.
- Spec 008 "all writes preserve existing content" vs. replacing Claude/generic files.
- Spec 007 "placeholder with dimensions" vs. dimensions only in the status line.
- `acceptance.md` real-popup claim vs. fake-tmux-only evidence.
- Spec 002 does not define `changed`/`invalidated` status labels.

## Suggested fix order
1. QA-1 (qa.sh integrity) and QA-2 (commit/release state).
2. QA-3 (real tmux propagation or result-file fallback).
3. Missing tests for 002/003/005/006/007/008.
4. Behavioral polish: external-close abort, image placeholder, reload selection, `--all` resolve filtering.
5. Media responsiveness and stronger PNG/Mermaid assertions.
6. Spec/doc amendments and the final gate/report/commit.
