# agentgloss — Terminal Spec Reviewer Design

- **Date:** 2026-11-20 (updated 2026-11-28)
- **Status:** MVP complete and v1.0.2 released 2026-11-28; 191 Rust tests plus 19 VS Code extension tests (210 total); the editor bridge and VS Code extension were added in this release line
- **Name:** agentgloss (formerly mdreview, which collides with an existing Python/Textual project)
- **Ambition:** personal tool first, open-source later only if it proves useful

## 1. Problem

Reviewing long agent-generated specs and plans in a terminal is awkward. Scrollback loses context, and there is no way to attach review comments to specific lines or paragraphs. The goal is a fast, friendly TUI that renders Markdown — including images and Mermaid diagrams — lets a human comment on exact source locations and propose edits, and returns a structured, numbered review to any coding agent.

## 2. Product decisions

| Area | Decision |
| --- | --- |
| Process | Architectural: questions, market research, sectioned design, written spec, implementation plan. No code before approval. |
| Integration | Both standalone and agent-invoked. Core is a standalone TUI plus a file/prompt handshake; a tmux adapter is included as an additive layer. |
| Audience | Personal tool first. Optimize for the author's workflow; defer packaging and positioning. |
| Stack | Rust 2024, ratatui 0.30, crossterm 0.29, `edtui` for modal editing. Static musl binary; no async runtime. |
| Document scope | The spec stays read-only. The reviewer creates comments and suggested edits; the agent makes the changes. |
| Persistence | Sidecar JSON at `<file>.review.json` plus a paste-ready prompt export. Lock file at `<file>.review.json.lock`. |
| Review rounds | Unresolved comments persist across revisions, re-anchored by quote and context. Resolved items are archived; unfindable anchors become stale but keep their quote and body. |
| Agents | First-class recipes for Claude Code, Codex CLI, Gemini CLI, and generic CLIs (OpenCode, Aider, etc.). |
| Media | Local images and Mermaid diagrams render inline. Rust Mermaid renderer is the default; mermaid-cli is an optional fidelity override. Remote image fetching is out of scope; URLs render as link-styled text. |
| Input | Vim modal bindings, standard keyboard mode, and mouse. Runtime switch between input modes. |
| Network | None by default. No remote image fetching, telemetry, or cloud service. |
| Name | agentgloss; formerly mdreview, which collides with an existing project. |

## 3. Market context (November 2026)

- **The concept already exists in several forms.** The closest are Plannotator (browser-first plan review, ~13.7k stars, with a secondary Rust/ratatui TUI), critty (Go terminal review of plans and diffs, agent skills), akapen (Rust/ratatui, rendered Markdown, line comments, Vim plus mouse, but in-memory comments), and mdreview (Python/Textual, sidecar JSON and anchor drift, no agent integration). Smaller projects include annotui, mdtation, margin, rcliao/comments, revspec, and plan-review.
- **Agent vendors are absorbing the capability.** Google Antigravity CLI has per-line comments and approve/reject. Claude Code plan mode supports approval and `Ctrl+G` editor handoff, but not line comments.
- **Demand is real but direct-tool traction is thin.** The 2026 Stack Overflow survey reports 66% using coding agents and 48% trusting output only when they can verify it. Star and download counts for most direct tools are small; Plannotator's browser product is the outlier. As a commercial product this is a niche; as a personal tool it is well suited.
- **Hard constraint:** agent shells have no TTY, so an agent cannot inline a full-screen TUI. Real patterns are a standalone TUI plus file handshake, a tmux/Herdr pane, hook interception, `$EDITOR`-style handoff, or an ACP client. This design uses the first with an optional tmux adapter.
- **Stack validation:** ratatui is viable. `tui-markdown` is experimental, so the renderer is hand-rolled with pulldown-cmark and a source map. Mouse capture disables native text selection; the terminal-specific fallbacks are documented below.
- **Differentiation for this tool:** cross-agent handoff, persistent unresolved comments across rounds, re-anchoring after agent rewrites, and a review flow that is first-class for both Vim and mouse users.
- **Terminal matrix:** Kitty graphics on Kitty, Ghostty, WezTerm, and Konsole; iTerm2 images on iTerm2 and WezTerm; Sixel on foot, WezTerm, xterm, mlterm, Windows Terminal 1.22+, and VS Code 1.101+; Alacritty has no graphics protocol. Terax is an xterm.js/Tauri terminal with no known Kitty/Sixel support and uses half-blocks. tmux needs `allow-passthrough` for graphics beyond half-blocks; tmux 3.2a cannot use the 3.4 Sixel build.
- **Media stack:** `ratatui-image` 11.1 provides protocol encoding. Its default `chafa-dyn` feature links libchafa, so defaults are disabled for a static binary, with Unicode half-blocks as the universal fallback. `mermaid-rs-renderer` 0.3.1 (MIT, 23 diagram types, pre-1.0) renders Mermaid to SVG; `resvg`/`usvg` rasterizes it in-process. mermaid-cli costs roughly 300 MB of Node and Chromium and is not bundled.

## 4. Workflow

1. The agent writes or updates a spec (for example `docs/specs/checkout.md`) and reaches a review checkpoint. It runs `agentgloss request <file>`, which records the request and tells the agent that review is pending.
2. The human reviews through one of three paths:
   - **Standalone:** `agentgloss review <file>` in any terminal pane.
   - **Handshake:** the agent asks the human to run the command, then collects feedback with `agentgloss export <file>` or `agentgloss wait <file>`.
   - **tmux:** the agent runs `agentgloss review <file> --tmux`; a popup opens, and signoff returns the prompt to the agent. `--detach` returns immediately for agents that cannot block.
3. On signoff the sidecar is saved, a numbered prompt is emitted, and the exit code tells the agent whether there is feedback or a clean approval.
4. The agent revises, optionally marks IDs resolved, and requests round two. Open comments re-anchor to the new text; resolved comments archive; comments whose anchors cannot be found become stale.
5. The human can quit mid-review. Drafts autosave, and the next launch resumes.

Exit codes: `0` approved with no open or stale items, `10` signoff with open feedback, `1` user aborted, `2` error, `3` wait timeout.

## 5. Round lifecycle

- A round increments when a signed-off review is reopened against changed content.
- Approval is bound to the document content hash. If the file changes after approval, the approval is void and a new round begins.
- Item states are `open`, `resolved`, and `stale`. Stale items keep their quote and body and can be reattached to a new selection; they are never silently dropped.
- Suggested edits are tracked like comments and become stale when their original text no longer matches.

## 6. Scope

### In scope for the MVP

- One Markdown file at a time, with a GFM subset: headings, paragraphs, lists, task lists, code blocks, tables, blockquotes, links, image alt text, and rules.
- Rendered view as the default, plus a raw-source toggle for precise line work.
- Line and paragraph selection; comments; suggested edits; resolve, stale, and reattach.
- Outline pane, search, help overlay, status and review panels.
- Sidecar JSON persistence with multi-round re-anchoring.
- Paste-ready prompt export plus JSON export.
- tmux adapter and install recipes for Claude Code, Codex CLI, Gemini CLI, and a generic CLI recipe.
- Vim, standard, and mouse input modes with an explicit runtime switch.
- Local images and Mermaid diagrams rendered inline.

### Out of scope

- Editing spec prose directly; comment threads or replies; multi-file or diff review.
- Remote image URLs, animated GIF playback, video or HTML embeds, custom Mermaid themes beyond light, dark, and transparent.
- Cloud collaboration, telemetry, ACP/MCP integration, CI gating, full Vim emulation (macros, registers, ex commands), and a plugin system.
- Bundling Chromium or mermaid-cli.

## 7. Architecture

A single Rust crate with a testable library core and a thin binary.

| Module | Responsibility |
| --- | --- |
| `document` | pulldown-cmark parser → flat blocks with byte/line source ranges and heading paths; `layout` wraps spans into visual lines that keep source maps and reserve rows for media. |
| `review` | Comment/suggestion model, anchors, re-anchoring, sidecar store with locking and atomic writes, prompt/JSON export. |
| `media` | Terminal protocol detection, asynchronous image/Mermaid workers, SVG rasterization, half-block fallback. |
| `tui::app` | App state, Vim/standard key handling, mouse hit-testing, editor, search, signoff, file watching. |
| `tui::render` | Responsive panes, document view, outline/review panels, popups, media overlay. |
| `agent` | tmux adapter and install recipes. |
| `cli` / `config` | Command dispatch, exit codes, TOML config, option precedence. |

**Concurrency.** No async runtime. The TUI runs a polled crossterm event loop. Media decode/raster/encode runs on worker threads and reports through channels; generation is keyed by content, engine, theme, protocol, and target size. Results are polled during each draw. Input never blocks on media.

**Render discipline.** Text layout reserves media rectangles. The text pass skips cells when an encoded image is ready; the media pass draws them with `ratatui-image`. The document area is cleared when media is visible, and encodings are dropped on width changes.

**Persistence.** Atomic sidecar writes (temp + rename), exclusive `fs2` lock, corrupt-file backup, and strict schema versioning.

## 8. Review data model

Sidecar schema version 1 contains `file`, `doc_hash`, `round`, optional `requested_at`, optional `last_signoff`, optional `approval`, `next_id`, and `items`.

Each item has a stable `cN` ID, kind (`comment`/`suggestion`), status (`open`/`resolved`/`stale`), body, round timestamps, an optional suggestion (`original`/`replacement`), history, and an anchor. An anchor stores the heading path, block index, exact quote, 32-character prefix/suffix context, byte range, 1-based line range, and document hash.

Re-anchoring tries exact quote matches (scored by context and distance), then whitespace-normalized matches, then a heading/block fallback with token Jaccard similarity ≥ 0.5. Anything else becomes stale. Resolved items are never re-anchored.

## 9. TUI and input

- Header, document view, optional outline and review panes, footer status and clickable actions.
- Responsive: both panes at ≥110 columns; the focused pane at 80–109; document-only below 80.
- Vim mode: `j/k`, `Ctrl-d/u`, `gg/G`, `]]/[[`, `{/}`, `/`, `n/N`, `v`, `V`, `P`, `c`, `s`, `x`, `X`, `e`, `d`, `q`, `?`, `Tab`, `F2`, `F3`, `Ctrl+Enter`.
- Standard mode: arrows, PgUp/PgDn, Home/End, Shift+arrows/Home/End, `Ctrl+↑/↓` paragraphs, `Ctrl+[/]` headings, `Ctrl+f`, and clickable footer actions.
- Mouse: click to place the cursor, drag to select, wheel to scroll, click outline/review items or footer actions.
- The editor uses `edtui` in Vim mode and its Emacs/standard handler for modeless typing. `Ctrl+Enter` saves; `Esc` cancels (Vim cancels only from Normal mode).
- Signoff confirms with `Enter`; a clean review approves, otherwise feedback is sent. Quitting autosaves and exits aborted.
- File changes while editing are deferred; the reload happens after the editor closes.

## 10. Media

- Images resolve relative to the document; absolute local paths are allowed. Remote URLs are not fetched and render as link-styled text.
- PNG/JPEG/WebP/BMP and first-frame GIF decode with `image`; SVG and Mermaid rasterize with `usvg`/`resvg` using system fonts.
- Limits: 50 MB, 10,000 px per side, 40 MP.
- Protocol selection is environment-based, not an interactive stdio query: Kitty (`KITTY_WINDOW_ID`, Ghostty), iTerm2 (WezTerm/iTerm2), Sixel (`WT_SESSION`, foot/mlterm), otherwise half-blocks. An explicit `--protocol` overrides detection.
  - The stdio query was rejected after PTY testing showed its reader thread stealing keyboard input.
- Mermaid: Rust renderer by default; `mmdc` when explicitly selected or when the Rust parser fails and `mmdc` is installed; source fallback otherwise.
- Encoded media is cached in memory by content/engine/theme/protocol/target size. Disk caching is deferred; first paint is asynchronous and shows the placeholder.
- Pixel scaling derives the terminal cell size from `TIOCGWINSZ` when the terminal reports plausible pixel metrics, falling back to 10x20. No interactive stdio query is used.
- tmux 3.2a can display half-blocks and Kitty placeholders with passthrough; it cannot use Sixel. Terax (xterm.js) uses half-blocks.

## 11. Agent integration and harness

Commands: `review`, `request`, `wait`, `export`, `status`, `resolve`, `install`, and the hidden `__tui` entry used by tmux.

The tmux adapter uses `tmux display-popup -E` for a blocking review and `tmux new-window -d` for detach. Install recipes write `.claude/commands/agentgloss-review.md`, marker blocks in `AGENTS.md`/`GEMINI.md`, or a standalone generic recipe; re-running is idempotent.

The local harness mirrors `sotto/harness`:

- `init.sh` checks the toolchain, state, and baseline build.
- `harness/PROMPT.md` defines the one-task TDD session protocol.
- `harness/gate.sh` validates tasks, placeholders, fmt, clippy, and tests.
- `harness/loop.sh`, `plan.sh`, `qa.sh`, `report.sh`, `add-task.sh`, and the task schema support autonomous sessions and QA.
- `state/` holds tasks, progress, and the fix plan; `specs/` holds feature specs.

The Linux release build environment is `scripts/env.sh`: a conda `rustbuild` GCC for host build scripts and a static `x86_64-unknown-linux-musl` final binary linked by Rust's bundled `rust-lld`. This avoids requiring system sudo or a system C toolchain. On macOS, do not source `scripts/env.sh`; build and test the host target with `--target aarch64-apple-darwin` so the musl default in `.cargo/config.toml` is overridden.

## 12. Validation

- 128 library tests, 16 CLI integration tests, 35 TUI integration tests, 11 bridge integration tests, and 1 performance test pass (191 Rust tests); `cargo fmt`, `clippy -D warnings`, and `cargo test --all-features` pass.
- The VS Code extension contributes 19 Vitest/jsdom tests (209 automated tests total) and packages as a VSIX.
- Additional coverage includes tmux feedback/abort exit codes, custom sidecars, export locking, stale approval status, reattachment, narrow-terminal overlays, mouse hit-testing, asynchronous content-addressed media caching, grapheme-safe wrapping, multi-paragraph lists and footnotes, control-character sanitization, and a 2,000-line performance test.

### Editor bridge and VS Code extension (2026-11-28)

The hidden `bridge state|apply` JSON commands expose a pure, lock-free state
read and per-operation review mutations. Line ranges are the only coordinate
on the wire; Rust builds anchors, re-anchors on the current revision, and owns
locking and signoff policy. The `vscode/` extension renders Markdown with
bundled Mermaid v11, maps block/paragraph selections to line ranges, and writes
the same sidecar, so the TUI and webview are interchangeable. Specs 010 and 011
are the contracts.
- PTY smoke test: a real tmux pane rendered the document, Mermaid, and image placeholder, accepted `q`, restored the terminal, and exited with code 1 (aborted).
- Fake-tmux integration test verifies popup and detached command construction.
- Media tests cover local PNG/SVG/Mermaid rendering, missing/remote images, size limits, protocol detection, and asynchronous completion.
- Round tests cover exact/moved/duplicate/whitespace/block/stale anchors, approval invalidation, and a changed-file reload.

## 13. Known limitations

- The former name `mdreview` collides with an existing Python/Textual project.
- Media cache is in-memory only; disk caching and an LRU bound are deferred.
- `mermaid-rs-renderer` is pre-1.0 and may differ from mermaid-cli; `mmdc` is the escape hatch.
- tmux 3.2a cannot use Sixel; Terax has no inline graphics protocol.
- Remote image fetching, ACP/MCP, CI gating, comment threads, and full Vim emulation are out of scope; remote URLs render as link-styled text.
- VS Code rendered selections are block/paragraph level; source-editor commands provide exact lines. Custom sidecar paths and remote/WSL extension hosts are not supported, and the extension is not Marketplace-published.
- The autonomous `loop.sh`/`plan.sh`/`qa.sh` scripts require a `claude` CLI on PATH; `init.sh`, `gate.sh`, and `report.sh` do not.

## 14. Sources

- [Plannotator](https://github.com/backnotprop/plannotator), [plannotator-tui](https://github.com/plannotator/plannotator-tui)
- [critty](https://github.com/crittylabs/critty), [akapen](https://github.com/worldnine/akapen), [mdreview](https://github.com/lazyoft/mdreview)
- [Claude Code plan mode](https://code.claude.com/docs/en/permission-modes), [Antigravity CLI artifacts](https://antigravity.google/docs/cli/artifacts)
- [ratatui](https://github.com/ratatui/ratatui), [ratatui-image](https://github.com/ratatui/ratatui-image)
- [mermaid-rs-renderer](https://github.com/1jehuang/mermaid-rs-renderer), [mermaid-cli](https://github.com/mermaid-js/mermaid-cli)
- [presenterm](https://github.com/mfontanini/presenterm), [mdcat](https://github.com/BIRSAx2/mdcat)
- [Kitty graphics protocol](https://sw.kovidgoyal.net/kitty/graphics-protocol/), [Windows Terminal Sixel discussion](https://github.com/microsoft/terminal/discussions/17809)
- [Terax](https://terax.app/), [terax-ai on GitCode](https://gitcode.com/xafc029/terax-ai#1)
- [Stack Overflow 2026 AI survey](https://survey.stackoverflow.co/2026/ai)
