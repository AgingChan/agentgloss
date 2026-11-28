# agentgloss Progress

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

## 2026-11-22 — Session 6: shell environment fix

- Fixed `scripts/env.sh` leaking `set -euo pipefail` into the caller's interactive shell. Terax's prompt hook reads `$TERAX_BLOCKS` without a default, so `nounset` made every prompt print `TERAX_BLOCKS: parameter not set`.
- The script now validates conda and the `rustbuild` environment explicitly, returns on error, and leaves the caller's shell options unchanged.
- Verified in zsh: sourcing leaves the shell without `nounset`, `TERAX_BLOCKS=unset` is harmless, and `cargo`/`rustc` resolve correctly. Build and full gate still pass (131 tests).

## 2026-11-28 — Session 7: QA cleanup, editor bridge, and VS Code extension

- Established the macOS arm64 baseline: Rust 1.100, host target `aarch64-apple-darwin`, 153 tests green. Local commands use `--target`; `scripts/env.sh` remains Linux/conda-only.
- Added the hidden editor bridge (`bridge state` / `bridge apply`) with a versioned JSON contract: lock-free pure state reads, per-operation mutations with `expect_hash`, line-range selections, reconciliation, item CRUD, and shared signoff policy. 11 bridge integration tests.
- Added the VS Code extension under `vscode/`: rendered Markdown + bundled Mermaid v11 webview, block/paragraph selection for comments/suggestions, inline threads, stale reattach, signoff, bridge state sync, source-editor exact-selection commands, trust/CSP protections, and a 1 MB VSIX. 17 Vitest/jsdom tests.
- Completed the P1 cleanup: resolve validates the document; version probing avoids backing up unsupported sidecars; empty table cells carry ranges; multi-paragraph lists/footnotes, list spacing, nesting indentation, and hanging wraps; control-character sanitization; media gutter offset; stale draft discard; standard `[`/`]` headings; scrollable help; reload resets; duplicate/orphan agent markers; SIGHUP/SIGTERM and panic-hook terminal restore.
- Completed the P3 cleanup: removed `image_src`; deduped helpers; unified timestamps; moved media hashing to workers; cache keys include cell pixel size; generation counters discard stale results; Auto `mmdc` fallback covers render/raster/encode; empty-document message reachable; mouse cleanup on init failure.
- Added extension wiring: Cargo/package exclusions, gitignore entries, a Node CI job with VSIX artifact, and updated package/docs/workflow checks.
- Final counts: 128 library + 16 CLI + 35 TUI + 11 bridge + 1 performance = 191 Rust tests, plus 17 extension tests (208 total). fmt and clippy are clean.
- Verified the PTY close path: a `script` PTY ran the TUI, SIGHUP produced exit 1, and the sidecar was saved. Verified the extension activates in an isolated VS Code 1.145 profile with no extension-host errors.
- Remaining manual check before a release: drive the webview in a real VS Code window and exercise the rendered selection → signoff flow (including Mermaid fidelity).

## 2026-11-28 — Session 8: v0.2.0 release and editor entry points

- Released v0.2.0: merge/tag on `main`, release workflow success, GitHub assets (`agentgloss-0.2.0.vsix`, both tarballs, `SHA256SUMS`), and crates.io publication.
- Added the VS Code **Open Review** editor-title button (preview icon), the Explorer file context menu entry, and the editor-tab context menu entry. The command now accepts Explorer URIs as well as the active editor.
- Added `test/manifest.test.ts`; extension tests are now 19 (210 automated tests total). `npm run check`, `npm test`, and VSIX packaging pass.
- The entry points ship as v0.2.1; the real-window webview selection → signoff check remains manual.

## 2026-11-28 — Session 9: v0.2.1 release

- Merged and tagged v0.2.1 (`cd866dc`); release workflow run 46051858621 succeeded: https://github.com/AgingChan/agentgloss/actions/runs/46051858621.
- GitHub release assets verified: `agentgloss-0.2.1.vsix`, both platform tarballs, and `SHA256SUMS`: https://github.com/AgingChan/agentgloss/releases/tag/v0.2.1.
- The crates.io publish job succeeded for 0.2.1. The VSIX manifest contains version 0.2.1, the `$(open-preview)` icon, and the `editor/title`, `explorer/context`, `editor/title/context`, and `editor/context` menus.
- Remaining manual check: drive the webview in a real VS Code window and exercise the rendered selection → signoff flow (including Mermaid fidelity).

## 2026-11-28 — Session 10: brand mark and extension icon

- Created the AgentGloss mark (a folded page whose corner becomes a speech-bubble tail, with an amber gloss line), light/dark wordmark lockups, and a palette/brand guide under `branding/`.
- Added the 128 px extension icon and 16 px light/dark editor-title annotation icons; replaced the Markdown preview codicon. The manifest test now verifies the icon paths and menu contributions.
- Ships as v0.3.0.

## 2026-11-28 — Session 11: v0.3.0 brand release

- Merged and tagged v0.3.0 (`1aa489a`); release workflow run 46091008117 succeeded: https://github.com/AgingChan/agentgloss/actions/runs/46091008117.
- GitHub release assets verified: `agentgloss-0.3.0.vsix`, both platform tarballs, and `SHA256SUMS`: https://github.com/AgingChan/agentgloss/releases/tag/v0.3.0.
- crates.io publish succeeded for 0.3.0. The VSIX manifest carries version 0.3.0, `media/icon.png`, the dark gallery banner, and the branded light/dark editor-title icons.
- The README logo now resolves from `branding/` on `main`; branding assets remain excluded from the crate package.

## 2026-11-28 — Session 12: publisher ID change

- Changed the VS Code publisher ID from `agingchan` to `InnopiaPteLtd`; the extension ID is now `InnopiaPteLtd.agentgloss`.
- Added a manifest assertion and refreshed the extension/spec/changelog docs. Ships as v1.0.0, the first stable release.

## 2026-11-28 — Session 13: v1.0.0 stable release

- Merged and tagged v1.0.0 (`1840c5b`); release workflow run 46092260108 succeeded: https://github.com/AgingChan/agentgloss/actions/runs/46092260108.
- GitHub release assets verified: `agentgloss-1.0.0.vsix`, both platform tarballs, and `SHA256SUMS`: https://github.com/AgingChan/agentgloss/releases/tag/v1.0.0.
- Downloaded the published VSIX and verified its identity: `Id="agentgloss" Version="1.0.0" Publisher="InnopiaPteLtd"`.
- crates.io publish succeeded for 1.0.0. The VS Code extension ID is now `InnopiaPteLtd.agentgloss`; the old `agingchan.agentgloss` install is a separate, deprecated identity.
