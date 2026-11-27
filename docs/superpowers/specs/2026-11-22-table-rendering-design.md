# Table rendering in the rendered view

- **Date:** 2026-11-22
- **Status:** Implemented (aligned tables, styled header, wrapping, CJK, and a
  narrow-width flattened fallback; empty cells carry source ranges)

## Problem

The rendered view treats a GFM table as one paragraph: cells are joined with `" | "` and a literal `--- | ---` separator row is inserted (`src/document/mod.rs:723`), then the result is word-wrapped with no column alignment (`src/document/layout.rs:111`). At a 60-column document pane the keymap table in `specs/006-tui.md` wraps mid-row and shows its Markdown separator, so it reads as unrendered source. Inline styles inside cells (code spans, emphasis, links) are also lost, because table text is accumulated as a plain `String` (`src/document/mod.rs:453`).

## Intent

Make tables look like tables in the default rendered view, while preserving existing line-based cursor, selection, search, comment anchoring, and the `F3` raw-source view.

## Goals

- Aligned columns with a styled header row; no literal `---` separator row.
- Preserve per-cell inline styles and source ranges, matching spec 003 ("cells are inlines with source ranges").
- Honor GFM column alignment (`:---`, `:---:`, `---:`).
- Reflow on resize and panel toggles (layout already reruns on width changes).
- Degrade gracefully when the pane is too narrow: wrap cell content, and fall back to the current flattened form if minimum column widths cannot fit.

## Non-goals

- Horizontal scrolling or manual column resizing.
- Remote content, HTML tables, or tables nested in blockquotes/lists beyond what the parser already supports.
- Editing tables in place.

## Anti-goals

- Losing table text at narrow widths.
- Breaking comment anchors or search within cell text.
- Special-casing only the spec's keymap table.

## Constraints

- `layout` is pure and receives only the document plus a width; it is rerun whenever width changes.
- `VisualLine` source ranges drive anchoring; each rendered table line must map back to real source bytes.
- The raw-source view and exported prompt text must not change.

## Approaches considered

1. **Structured table retained on the block, rendered in layout (chosen).** The parser keeps header/body rows, alignments, and per-cell inlines plus ranges; layout computes column widths for the current pane and wraps within cells. Reflows correctly, keeps styles and ranges, and supports alignment.
2. **Pad cells at parse time.** Fixed widths baked into the block. Rejected: widths cannot respond to resize or panel toggles, and wrapping still breaks alignment.
3. **Split the flattened block text on `|` in layout.** Rejected: ambiguous with pipes inside code spans, and header/alignment information is already gone.

## Design

### Parser (`src/document/mod.rs`)

- Extend `Block` with `table: Option<Table>`.
- `Table { header: Vec<TableCell>, rows: Vec<Vec<TableCell>>, alignments: Vec<ColumnAlign> }`.
- `TableCell { inlines: Vec<Inline>, source: Range<usize> }`; `ColumnAlign` mirrors `None/Left/Center/Right`.
- While `in_table`, append text/code/emphasis/etc. into the current cell's inlines with the same style logic used for ordinary inlines; `SoftBreak`/`HardBreak` become a space inside a cell.
- On `Tag::Table(alignments)` store alignments. `TagEnd::TableHead` records the header row; `TagEnd::Table` finishes the block without flattening into `block.inlines`.
- `Block::text()` returns the concatenation of cell text for tables so outline/anchor fallback behavior is unchanged.

### Layout (`src/document/layout.rs`)

- `BlockKind::Table` dispatches to `layout_table`.
- Natural column width = max visual width of the column's cell text (including header), min 3, using `unicode-width`.
- Available width = pane width minus the review gutter (2). Separator is `" │ "` between columns.
- If natural widths fit, use them. Otherwise shrink the widest columns (proportionally or widest-first) until the total fits, never below the minimum; wrap each cell's styled inlines to its column width, and emit one visual line per wrapped row segment.
- Pad cells to the column width honoring alignment; a header line is marked so the renderer can style it.
- If even minimum widths plus separators exceed the available width, fall back to the existing flattened text path for that table.

### Renderer (`src/tui/render.rs`)

- Header visual lines are styled bold + cyan (consistent with headings); padding and `│` separators are dim.
- Existing marker/gutter, selection, cursor, search underline, and source mode behavior stays unchanged.

### Specs

- `specs/003-document-model.md`: replace "Tables may render as aligned text" with the structured-cell and aligned-rendering requirements.
- `specs/006-tui.md`: add table rendering to the Layout section (header style, `│` separators, cell wrapping, narrow-width fallback).
- `specs/009-qa.md`: add table rendering to the test matrix.

### Tests

- Parser: header/body rows, alignments, inline styles, and cell source ranges retained; `Block::text()` still usable.
- Layout: alignment output at a fixed width, header marking, wrapping when narrow, CJK cell widths, and flattened fallback when too narrow.
- Regression: existing parser/layout/TUI tests pass.

## Left to the builder

- Exact width-shrinking heuristic (proportional vs widest-first) and the minimum column width.
- Whether to cache natural widths or recompute during layout (recompute is acceptable at current document sizes).
- Whether separators or padding alone read better once implemented.
