# Changelog

## 0.2.1 — 2026-11-28

### Added

- VS Code: an editor-title **Open Review** button (preview icon), an Explorer file context-menu entry, and an editor-tab context-menu entry.

## 0.2.0 — 2026-11-28

### Added

- Hidden, versioned JSON bridge (`agentgloss bridge state|apply`) for editor integrations: lock-free state reads, per-operation mutations, line-range selections, item CRUD, and shared signoff policy.
- VS Code extension (`vscode/`) with a rendered review webview, bundled Mermaid v11, block/paragraph comment and suggestion selection, inline threads, stale reattachment, signoff, and source-editor exact-selection commands.

### Changed

- Media hashing now runs on worker threads; cache keys include the target cell size and terminal pixel size, and stale worker results are discarded after a layout change.
- Mermaid Auto mode falls back to `mmdc` after any Rust render, raster, or encode failure.
- Standard-mode heading navigation uses `[`/`]` (Ctrl+[ arrives as Esc); `Ctrl+S` saves outside editors; the help overlay fits and scrolls.
- SIGHUP/SIGTERM and panics restore the terminal and autosave the review.
- Export keeps its exclusive lock as handshake policy; the bridge takes the lock per mutation and `state` is lock-free.

### Fixed

- `resolve` validates the Markdown file and reconciles before resolving.
- Unsupported sidecar versions are rejected without backup, even with incompatible newer fields.
- Empty table cells carry an inline with the cell source range.
- Multi-paragraph list and footnote content renders as separate, indented blocks; adjacent items no longer get blank separators; wrapped list lines use a hanging indent.
- Control characters are sanitized in rendered/source/export output without changing source bytes or anchors.
- Media no longer covers the gutter; image suggestions are rejected; stale drafts are discarded across revisions.
- Duplicate complete agent marker blocks collapse, and orphan markers are refused without rewriting the file.
- Empty documents show the empty-document message.

## 0.1.1 — 2026-11-22

- Media blocks now fit the document page: Mermaid diagrams and images are scaled to the page width, capped at the viewport height, sized by aspect ratio instead of fixed rows, and centered. Partially scrolled images use sliced protocols.
- Added `AGENTGLOSS_CELL_SIZE=WxH` and `AGENTGLOSS_RENDER_SCALE=1..4` for high-DPI terminals whose images look soft.
- Inside tmux, terminals that would otherwise use Sixel fall back to half-blocks, since tmux 3.2a cannot display Sixel.
- Warp is detected as an iTerm2 inline-image terminal, so diagrams render as real images instead of half-blocks.

## 0.1.0 — 2026-11-21

Initial MVP:

- Markdown review TUI with rendered and source views
- Vim, standard, and mouse interaction modes
- Line, paragraph, and block selection
- Comments and suggested edits with autosave
- Persistent sidecar JSON, locking, and corrupt-state recovery
- Multi-round re-anchoring with stale detection and reattachment
- Prompt and JSON export with agent-oriented exit codes
- Standalone, file-handshake, tmux popup, and detached tmux launch paths
- Agent install recipes for Claude Code, Codex, Gemini, and generic CLIs
- Local image preview with SVG rasterization and terminal protocol fallback
- Mermaid rendering through the compiled Rust renderer, with optional `mmdc`
- Asynchronous media workers and content-addressed in-memory caching
- Static musl release binary
- Local TDD harness with gate, plan, QA, and report scripts
- Renamed from mdreview to agentgloss; the crate, binary, and commands are now `agentgloss`
- Config and cache directories moved to `agentgloss`; an existing `~/.config/mdreview` is not read
- Agent marker blocks and generated recipe file names changed; delete the old `<!-- mdreview:* -->` block, then re-run `agentgloss install <agent>`
- Prebuilt Linux x86_64 and macOS Apple Silicon releases with an install script and SHA-256 checksums
- `cargo install agentgloss` from crates.io
- The release workflow publishes the crate to crates.io on version tags using the `CARGO_REGISTRY_TOKEN` secret
