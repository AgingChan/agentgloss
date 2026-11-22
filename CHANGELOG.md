# Changelog

## Unreleased

- Media blocks now fit the document page: Mermaid diagrams and images are scaled to the page width, capped at the viewport height, sized by aspect ratio instead of fixed rows, and centered. Partially scrolled images use sliced protocols.

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
