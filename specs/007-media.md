# 007 — Images and Mermaid rendering

## Images

- Resolve Markdown image paths relative to the document directory. Absolute local paths are allowed; `http://` and `https://` are not fetched and render as links.
- Supported: PNG, JPEG, WebP, BMP, and the first frame of GIF. SVG is rasterized with `resvg`/`usvg`.
- Limits: 50 MB per file, 10,000 px per side, 40 million pixels; oversized inputs show a placeholder with dimensions.
- Scale to the available content width, preserve aspect ratio. Alt text is the caption; the source path is shown in the placeholder/status.
- `image` decodes raster formats; `fontdb` loads system fonts once for SVG text. If no fonts are available, SVG text may be missing; the status line warns.

## Terminal protocols

- Detect from the environment (Kitty, Ghostty, WezTerm/iTerm2, Windows Terminal/foot/mlterm, otherwise half-blocks); an explicit `--protocol` overrides. Do not query stdio interactively: its reader thread can steal TUI keyboard input.
- Override with `--protocol auto|kitty|iterm2|sixel|halfblocks` or config.
- Half-blocks are the universal fallback and are always available. The `chafa-dyn` feature is disabled; no libchafa dependency.
- Kitty/Ghostty/WezTerm: sharp graphics. Terax/xterm.js: half-blocks. tmux: `allow-passthrough` is required beyond half-blocks; tmux 3.2a lacks Sixel. WSL2/Windows Terminal: Sixel only when the terminal build supports it.
- On resize, invalidate all protocol encodings and force a full repaint.
- Never draw text over an image rectangle, including blank cells. Clear stale placements when scrolling or resizing.

## Media pipeline

- `MediaResolver` produces `MediaRef` values from `BlockKind::Image` and `BlockKind::Mermaid`.
- Decoding/rasterization/encoding runs off the UI thread. The UI draws a placeholder until a result arrives; generation counters discard stale results.
- Content-addressed in-memory cache for encoded protocol states, keyed by content/engine/theme/protocol/target size. Disk caching and an LRU bound are deferred.
- Cache key: SHA-256 of content/engine/version/theme/target pixel dimensions.
- Only blocks near the viewport are decoded and encoded.

## Mermaid

- Default engine: compiled `mermaid-rs-renderer` (default features off; SVG output).
- `mmdc` is optional and selected with `mermaid_engine = "mmdc"` or automatically if the Rust renderer fails and `mmdc` is on PATH. It is invoked with arguments, never a shell; SVG/PNG output goes to a temp file.
- `mermaid_engine = "source"` never renders; it shows the fenced source.
- Rasterize SVG in-process: `usvg::Tree` → `tiny_skia::Pixmap` → PNG. Set the font database explicitly. Cap output dimensions.
- Invalid diagrams render as highlighted source with a "diagram unavailable" badge and the parser error in the status line.
- `Enter` on a Mermaid block toggles rendered/source.
- Cache keys include engine and theme; a failed render is cached until the source or configuration changes.

## Tests

- Resolve relative, absolute, missing, remote, and oversized image paths.
- Decode a generated PNG and a simple SVG; assert dimensions and non-blank pixels.
- Protocol selection precedence and half-block fallback.
- Cache hit/miss, invalidation on size change, and content-addressed keys.
- Mermaid flowchart, sequence, and class fixtures render non-empty PNGs with sane dimensions.
- Invalid Mermaid falls back to source; `mmdc` fallback uses a fake executable.
- Layout reserves media rectangles and never places text inside them.
