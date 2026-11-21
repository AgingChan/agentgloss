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
