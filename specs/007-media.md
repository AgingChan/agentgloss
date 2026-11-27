# 007 — Images and Mermaid rendering

## Images

- Resolve Markdown image paths relative to the document directory. Absolute local paths are allowed; `http://` and `https://` are not fetched, schedule no media worker, and render as link-styled text.
- Supported: PNG, JPEG, WebP, BMP, and the first frame of GIF. SVG is rasterized with `resvg`/`usvg`.
- Limits: 50 MB per file, 10,000 px per side, 40 million pixels; oversized inputs show a placeholder and report the limit in the status line (dimensions when they are known).
- Scale to the available content width, preserve aspect ratio. Alt text is the caption; the source path is shown in the placeholder/status.
- Media blocks are sized to the document page: the block reserves the image's aspect-derived rows at the page width, capped at the viewport height, and the image is centered. Unknown dimensions fall back to fixed placeholder rows. Partially scrolled media is rendered with a sliced protocol so the visible portion stays correct.
- `image` decodes raster formats; `fontdb` loads system fonts once for SVG text. If no fonts are available, SVG text may be missing.

## Terminal protocols

- Detect from the environment (Kitty, Ghostty, WezTerm/iTerm2/Warp, Windows Terminal/foot/mlterm, otherwise half-blocks); an explicit `--protocol` overrides. Do not query stdio interactively: its reader thread can steal TUI keyboard input.
- Override with `--protocol auto|kitty|iterm2|sixel|halfblocks` or config.
- Half-blocks are the universal fallback and are always available. The `chafa-dyn` feature is disabled; no libchafa dependency.
- Kitty/Ghostty/WezTerm/Warp: sharp graphics (Warp via iTerm2 inline images; it does not support Sixel). Terax 0.9: half-blocks; its Ghostty core parses Kitty placements but does not render them yet. tmux: `allow-passthrough` is required beyond half-blocks; tmux 3.2a lacks Sixel, so auto-detection falls back from Sixel to half-blocks inside tmux. WSL2/Windows Terminal: Sixel only when the terminal build supports it.
- Derive the terminal cell pixel size from `TIOCGWINSZ` (`window_size`) when columns, rows, and pixel dimensions are present and plausible; otherwise fall back to 10x20. Never query stdio for font size, for the same input-stealing reason.
- `AGENTGLOSS_CELL_SIZE=WxH` overrides the cell pixel size (for high-DPI terminals or Retina displays that report points); `AGENTGLOSS_RENDER_SCALE=1..4` multiplies it before rasterization/encoding. The layout aspect and rasterization use the same cell size.
- On resize, invalidate all protocol encodings and force a full repaint.
- Never draw text over an image rectangle, including blank cells. Clear stale placements when scrolling or resizing.

## Media pipeline

- `MediaResolver` produces `MediaRef` values from `BlockKind::Image` and `BlockKind::Mermaid`.
- Hashing, decoding/rasterization, and encoding run off the UI thread. The UI draws a placeholder until a result arrives; generation counters discard results from an older layout after `clear_encodings`.
- Content-addressed in-memory cache for encoded protocol states, keyed by content/engine/theme/protocol/target cell size, terminal cell pixel size, and render scale. Disk caching and an LRU bound are deferred.
- Image key: SHA-256 of file content (computed on a hash worker; a metadata-only placeholder key is used until the hash arrives), plus source string, protocol, target cell size, and cell pixel size. Mermaid key: SHA-256 of the source, plus engine, theme, protocol, target cell size, and cell pixel size. A cache-version prefix invalidates old encodings.
- Media rectangles start after the gutter so gutter markers stay visible; source-view numbering uses the wider gutter.
- Only blocks near the viewport are decoded and encoded.

## Mermaid

- Default engine: compiled `mermaid-rs-renderer` (default features off; SVG output).
- `mmdc` is optional and selected with `mermaid_engine = "mmdc"` or automatically if any stage of the Rust pipeline (render, rasterize, or encode) fails and `mmdc` is on PATH. It is invoked with arguments, never a shell; SVG/PNG output goes to a temp file.
- `mermaid_engine = "source"` never renders; it shows the fenced source.
- Rasterize SVG in-process: `usvg::Tree` → `tiny_skia::Pixmap` → PNG. Set the font database explicitly. Cap output dimensions.
- Invalid diagrams render as highlighted source with a "diagram unavailable" badge and the parser error in the status line.
- `Enter` on a Mermaid block toggles rendered/source.
- Cache keys include engine and theme; a failed render is cached until the source or configuration changes.

## Tests

- Resolve relative, absolute, missing, remote, and oversized image paths.
- Decode a generated PNG and a simple SVG; assert dimensions and non-blank pixels.
- Protocol selection precedence and half-block fallback.
- Font-size derivation accepts plausible pixel metrics and rejects zero/bogus ones.
- Cache hit/miss, invalidation on size change, and content-addressed keys.
- Mermaid flowchart, sequence, and class fixtures render non-empty PNGs with sane dimensions.
- Invalid Mermaid falls back to source; `mmdc` fallback uses a fake executable.
- Layout reserves media rectangles and never places text inside them.
