# mdreview Progress

## 2026-11-21 — Session 0: design and scaffold

- Approved the design: Rust/ratatui, standalone TUI + file handshake, tmux adapter, sidecar JSON, multi-round re-anchoring, comments + suggested edits, local images, Mermaid, Vim/standard/mouse input.
- Installed Rust 1.100 via rustup; added rustfmt, clippy, and the `x86_64-unknown-linux-musl` target.
- Created a conda `rustbuild` environment with GCC for host build scripts, because the WSL image had no `cc`.
- Configured `.cargo/config.toml` for static musl + `rust-lld`; added `scripts/env.sh`.
- Added and resolved dependencies: ratatui 0.30, crossterm, pulldown-cmark, edtui (no defaults), ratatui-image 11.1 (no defaults), image, resvg/usvg/tiny-skia, fontdb, mermaid-rs-renderer.
- Created the sotto-style harness: `init.sh`, `harness/PROMPT.md`, `gate.sh`, `loop.sh`, `plan.sh`, `qa.sh`, `report.sh`, `add-task.sh`, task schema, state files.
- Current task: MDREVIEW-1 (harness + skeleton). Next: build the core CLI/document/review modules, then the TUI.

## 2026-11-21 — Session 1: MVP implementation and validation

- Implemented the Rust library and binary: document parser/layout, review anchors and re-anchoring, sidecar store, prompt/JSON export, CLI, agent recipes, tmux adapter, TUI, asynchronous media, and the local harness.
- Added 63 automated tests: 46 library, 8 CLI integration, 9 TUI integration.
- Fixed a real PTY issue: `Picker::from_query_stdio` left a reader thread that stole keyboard input. Replaced it with environment-based graphics protocol detection.
- Validated the TUI in a real tmux pane: document, Mermaid, and image content render; `q` exits; terminal restores; exit code 1 as an aborted review.
- Validated tmux popup/detach command construction with a fake tmux executable.
- Tasks MDREVIEW-1 through MDREVIEW-12 are done. MDREVIEW-13 (final gate, release, docs) is in progress.
- Next: run `./harness/gate.sh`, build the release binary, and run the final QA/report pass.

## 2026-11-21 — Session 2: QA hardening

- Fixed the request-first round lifecycle: `request` now advances a signed-off revision before clearing the signoff, so multi-round reviews increment correctly.
- Reserved the comment gutter in layout width so full-width text is no longer clipped.
- Trimmed trailing newlines from anchors and added draft autosave/resume in the sidecar.
- Fixed stale-item reopen, page motions, malformed install markers, media status clobbering, search highlighting, Enter-to-open, normalized anchor scoring, narrow overlay size, and stale-only export headers.
- Added coverage for two-round flows, Mermaid sequence/class diagrams, failed-render caching, Gemini/generic installs, export goldens, gutters, Enter-to-open, review/footer clicks, 100x30 layout, drafts, and file reload.
- 98 tests pass; fmt and clippy are clean.

## 2026-11-21 — Session 3: final QA majors

- Export is now read-only: it renders a re-anchored view without changing round, approval, or the sidecar.
- Reloading a changed document refreshes the outline and clamps outline/review cursors.
- tmux uses sentinel `__tui` exit codes 20/21/22, so a missing `display-popup` (exit 1) is a clear error instead of a fake abort.
- Oversized media errors now include dimensions; Mermaid `source` mode and failures have distinct badges; fake-`mmdc` coverage was added.
- Decision: stale items block approval; spec 004/design wording updated.
- 101 tests pass; fmt and clippy are clean. Final QA pass is running.

## 2026-11-21 — Session 4: final pre-QA fixes

- Fixed line-selected suggestion originals to use the trimmed anchor quote, preventing false staleness after unrelated edits.
- Fixed the QA harness heredoc so backticks in the prompt are literal, not shell substitutions; validated with a fake `claude` capture.
- Save failures no longer get overwritten by success status messages.
- 102 tests pass; fmt and clippy clean; final QA is running.

## 2026-11-21 — Session 5: final QA fixes and release

- QA found one task-level failure: remote image URLs were not rendered as link-styled text. They now render as links, schedule no media worker, and keep the full image source range for comments.
- Mermaid `source` engine mode now shows source directly without spawning a failing render worker or a "diagram unavailable" badge.
- Approved exports with `--all` now print the approval line and a `Resolved items (archived):` section, with no action instructions or resolve commands.
- Agent install repairs partial marker blocks, errors on malformed/end-before-start markers, and refuses to overwrite unreadable or non-UTF-8 files.
- Added anchor-matrix tests (inserted/deleted text, heading moves, Unicode normalization, Jaccard fallback), parser edge tests (empty, CRLF, no trailing newline, tasks, rules, footnotes, nested lists), export edge tests (blank quote lines, fenced bodies, ordering, empty/stale-only), media tests (WebP, pixel limits, file cap, explicit protocol, Auto `mmdc` fallback), an approved tmux popup test, and media-row/no-label checks.
- Added TIOCGWINSZ font-size derivation with sane fallback; this closes the image scaling risk without an input-stealing stdio query.
- Added media hardening: oversized files are detected from metadata before the UI-thread hash read, raster decoding uses width/height/allocation limits, and worker panics become cached errors instead of endless pending keys.
- TUI polish: mouse clicks move focus to the clicked pane, Esc closes a narrow panel before clearing selection, and `]]`/`[[` no longer skip the first heading when the cursor starts before it.
- 131 tests pass (84 library, 15 CLI, 31 TUI, 1 performance); fmt and clippy are clean.
- Fixed MODAL-1: mouse input is ignored while an editor, confirmation, help, or search overlay is open, so footer/document clicks cannot replace or discard an editor draft.
- Addressed QA majors: `qa.sh` now validates the model invocation before atomically persisting the report; `__tui` writes its sentinel to `MDREVIEW_RESULT_FILE` so tmux handoff no longer depends solely on popup status propagation; terminal/event-loop failures autosave and exit aborted (1); MDREVIEW-13 is marked done after the release gate.
- Addressed the second QA pass majors: portable `F10`/`Ctrl+S` save and signoff bindings (verified in a real PTY), custom-sidecar `--sidecar` in exported resolve commands, and preservation of content before an orphan END marker.
- Final QA pass (21:57) returned **4 PASS, 9 PARTIAL, 0 FAIL, no critical or major findings**. The remaining PARTIALs are documented minor test-matrix and polish items, not feature blockers.
- Release evidence: gate PASS with 0 errors/0 warnings, 100% task progress, 131 tests in debug and release profiles, static-pie binary, real PTY save/signoff and tmux popup probes.
