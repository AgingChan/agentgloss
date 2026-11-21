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
