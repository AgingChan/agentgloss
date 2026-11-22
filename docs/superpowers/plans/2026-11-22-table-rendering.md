# Table Rendering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render GFM tables as aligned columns with a styled header, wrapped cells, and a safe narrow-width fallback, while preserving cell styles and source ranges.

**Architecture:** The parser retains table structure (`Table` on `Block`) instead of flattening at parse time. Layout computes per-column widths for the current pane, wraps each cell, and emits visual lines with padding and `│` separators; the renderer styles header lines and separators. If minimum column widths cannot fit, layout emits the previous flattened pipe-separated form.

**Tech Stack:** Rust 2024, pulldown-cmark 0.13.6, ratatui 0.30.2, crossterm 0.29, unicode-width 0.2.3. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-11-22-table-rendering-design.md`

## Global Constraints

- Verification commands, run from the repo root:
  - `source scripts/env.sh && cargo fmt --check`
  - `source scripts/env.sh && cargo clippy --all-targets --all-features -- -D warnings`
  - `source scripts/env.sh && cargo test --all-features`
- Do not add dependencies.
- The `F3` raw-source view, exported prompt text, and sidecar format must not change.
- Table text must never disappear at narrow widths; fall back to flattened text instead.
- End every commit message with the trailer `Co-Authored-By: Claude Code <noreply@anthropic.com>`.

## Review Focus

1. **Very narrow panes:** a table must fall back to flattened text before any column drops below 3 cells, and no cell text may be lost. (Tasks 1–2: `narrow_table_falls_back_to_flattened_text`, `table_wraps_cells_to_fit_narrow_width`.)
2. **Wide characters:** CJK/emoji content must not shift column separators or exceed the pane width. (Task 2: `table_columns_align_with_cjk_text`.)
3. **Source mapping:** each rendered table line must map back to real source bytes so cursor, selection, search, and comment anchors keep working. (Task 2: `table_lines_map_to_source_ranges`.)
4. **Irregular shapes:** header-only tables, short rows, and missing cells must render empty padding without panicking. (Task 2: `table_with_ragged_rows_does_not_panic`.)
5. **Blockquoted tables:** the quote prefix must still render and its width must be subtracted from the table's available width. (Task 2: `table_in_blockquote_keeps_quote_prefix`.)

---

### Task 1: Retain structured tables in the parser

**Files:**
- Modify: `src/document/mod.rs` (types near `Inline`/`Block`, parser state, event handling, tests)
- Modify: `src/document/layout.rs` (temporary flattened Table arm; becomes the fallback in Task 2)
- Test: inline `#[cfg(test)]` modules in both files

**Interfaces:**
- Consumes: existing `Inline`, `Block`, `Builder`, `append_text`, `TagEnd` handling.
- Produces:
  ```rust
  pub struct TableCell { pub inlines: Vec<Inline>, pub source: Range<usize> }
  pub enum ColumnAlign { None, Left, Center, Right }   // Default = None
  pub struct Table {
      pub header: Vec<TableCell>,
      pub rows: Vec<Vec<TableCell>>,
      pub alignments: Vec<ColumnAlign>,
  }
  // Block gains: pub table: Option<Table>
  ```
- Produces in `layout.rs`: `fn flatten_table_cells(table: &Table, fallback: usize) -> Vec<Cell>`.

- [ ] **Step 1: Write the failing parser tests**

In `src/document/mod.rs`, replace the table assertions in `parses_lists_tables_quotes_and_code` (currently the `let cells: Vec<_> = table.inlines...` block) with:

```rust
    let data = table.table.as_ref().expect("structured table");
    assert_eq!(data.header.len(), 2);
    assert_eq!(data.header[0].inlines[0].text, "a");
    assert_eq!(data.rows[0][0].inlines[0].text, "1");
    assert_eq!(data.rows[0][1].inlines[0].text, "2");
    assert!(
        data.header
            .iter()
            .chain(data.rows.iter().flatten())
            .all(|cell| cell.source.start < cell.source.end)
    );
```

Add this test to the same module:

```rust
    #[test]
    fn table_retains_styles_alignments_and_ranges() {
        let d = doc("| Name | Value |\n|:---|---:|\n| **bold** | `code` |\n");
        let table = d.blocks[0].table.as_ref().expect("structured table");
        assert_eq!(table.alignments, vec![ColumnAlign::Left, ColumnAlign::Right]);
        assert_eq!(table.header[0].inlines[0].text, "Name");
        let row = &table.rows[0];
        assert_eq!(row[0].inlines[0].style, InlineStyle::Strong);
        assert_eq!(row[0].inlines[0].text, "bold");
        assert_eq!(row[1].inlines[0].style, InlineStyle::Code);
        assert_eq!(row[1].inlines[0].text, "code");
        assert!(row[1].source.start < row[1].source.end);
        assert!(d.source[row[1].source.clone()].contains("code"));
    }
```

In `src/document/layout.rs` tests, add:

```rust
    #[test]
    fn narrow_table_falls_back_to_flattened_text() {
        let source = "| A | B | C | D |\n| --- | --- | --- | --- |\n| 1 | 2 | 3 | 4 |\n";
        let doc = Document::from_source("x.md", source.to_string());
        let lines = layout(&doc, 10);
        let text = lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n");
        assert!(text.contains("---"), "{text:?}");
        assert!(text.contains("1 | 2"), "{text:?}");
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `source scripts/env.sh && cargo test --all-features table_retains_styles_alignments_and_ranges`
Expected: compile error, `no field table on type Block`.

Run: `source scripts/env.sh && cargo test --all-features parses_lists_tables_quotes_and_code`
Expected: compile error for the same reason.

- [ ] **Step 3: Implement the structured model and parser changes**

In `src/document/mod.rs`:

1. Add `TableCell`, `ColumnAlign` (with `#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]`), and `Table` (with `#[derive(Debug, Clone, PartialEq, Eq, Default)]`) after `Inline`.
2. Add `pub table: Option<Table>` to `Block` and to `Builder`, initializing it to `None` in `Builder::new`; move it into `Block` in `Builder::finish`.
3. Replace `Block::text()` with: for a table, join header and body cells with `" | "` between cells and `"\n"` between rows; otherwise keep the existing inline concatenation. Do not include a `---` row.
4. Import `pulldown_cmark::Alignment` and add `impl From<Alignment> for ColumnAlign`.
5. Replace parser fields `table_rows`, `current_cell: String` with:
   ```rust
   table: Option<Table>,
   current_row: Vec<TableCell>,
   current_cell_inlines: Vec<Inline>,
   current_cell_range: Option<Range<usize>>,
   ```
   Keep `in_table`.
6. `append_text`: in the `in_table` branch, extend `current_cell_range` as today, then push `Inline { text: text.to_string(), style: style_override.unwrap_or_else(|| self.style()), source: range }` into `current_cell_inlines`.
7. `Event::SoftBreak | Event::HardBreak`: inside a table append `" "`, otherwise keep `"\n"`.
8. `Tag::Table(alignments)`: finish the current block, set `in_table`, allocate `Table { alignments: alignments.into_iter().map(ColumnAlign::from).collect(), ..Default::default() }`, clear `current_row`/`current_cell_*`, and `start_block(BlockKind::Table, range.start)`.
9. `Tag::TableCell`: clear `current_cell_inlines` and `current_cell_range`.
10. `TagEnd::TableCell`: push `TableCell { inlines: std::mem::take(&mut self.current_cell_inlines), source: self.current_cell_range.take().unwrap_or_else(|| range.clone()) }` into `current_row`.
11. `TagEnd::TableHead`: if `current_row` is non-empty, set `table.header = std::mem::take(&mut self.current_row)`.
12. `TagEnd::TableRow`: if `current_row` is non-empty, push it into `table.rows`.
13. `TagEnd::Table`: set `in_table = false`, assign `self.table.take()` to the current builder's `table`, set the builder end to `range.end`, and `finish_current()`. Remove the old flattening code.

In `src/document/layout.rs`:

14. Add a `BlockKind::Table` arm that, when `block.table` is `Some`, builds a flattened cell stream: prefix with `push_prefix`, append `flatten_table_cells(table, block.source.start)`, and run `wrap_cells`. If the result is empty, push a blank line. `flatten_table_cells` emits rows joined by `"\n"`, cells joined by `" | "`, and after the first (header) row a `"---"`-joined separator row, mirroring the old output.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `source scripts/env.sh && cargo test --all-features table_retains_styles_alignments_and_ranges`
Expected: PASS.

Run: `source scripts/env.sh && cargo test --all-features parses_lists_tables_quotes_and_code`
Expected: PASS.

Run: `source scripts/env.sh && cargo test --all-features narrow_table_falls_back_to_flattened_text`
Expected: PASS.

Run: `source scripts/env.sh && cargo test --all-features`
Expected: all tests pass.

- [ ] **Step 5: Run fmt, clippy, and the full suite**

Run: `source scripts/env.sh && cargo fmt`
Then: `source scripts/env.sh && cargo fmt --check`
Expected: no output.

Run: `source scripts/env.sh && cargo clippy --all-targets --all-features -- -D warnings`
Expected: no warnings.

Run: `source scripts/env.sh && cargo test --all-features`
Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/document/mod.rs src/document/layout.rs
git commit -m "Parse and retain structured GFM tables" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 2: Aligned table layout with cell wrapping

**Files:**
- Modify: `src/document/layout.rs` (`layout_table`, helpers, `VisualKind`)
- Test: inline `#[cfg(test)]` module in `src/document/layout.rs`

**Interfaces:**
- Consumes: `Table`, `TableCell`, `ColumnAlign`, `Block::table` from Task 1.
- Produces:
  ```rust
  pub enum VisualKind { Text, TableHeader, Media(MediaKind) }   // TableHeader is new
  fn layout_table(block: &Block, table: &Table, width: usize, document: &Document, out: &mut Vec<VisualLine>)
  fn cell_cells(cell: &TableCell) -> Vec<Cell>
  fn wrap_cell(cells: &[Cell], width: usize) -> Vec<Vec<Cell>>
  fn push_padded(line: &mut Vec<Cell>, content: &[Cell], col_width: usize, align: ColumnAlign, source: usize)
  fn make_line_with_kind(cells: &[Cell], block: usize, document: &Document, kind: VisualKind) -> Option<VisualLine>
  ```

- [ ] **Step 1: Write the failing layout tests**

Add these tests to `src/document/layout.rs`'s test module:

```rust
    #[test]
    fn table_renders_aligned_columns_without_separator_row() {
        let source =
            "| Action | Vim |\n| --- | --- |\n| Move line | j |\n| Half page | Ctrl-d |\n";
        let doc = Document::from_source("x.md", source.to_string());
        let lines = layout(&doc, 40);
        let table_lines: Vec<_> = lines.iter().filter(|l| l.block == 0).collect();
        assert!(
            !table_lines.iter().any(|l| l.text.contains("---")),
            "{table_lines:?}"
        );
        let bars: Vec<usize> = table_lines
            .iter()
            .map(|l| {
                l.text
                    .chars()
                    .position(|c| c == '│')
                    .expect("column separator")
            })
            .collect();
        assert!(bars.windows(2).all(|w| w[0] == w[1]), "{bars:?}");
    }

    #[test]
    fn table_honors_column_alignment() {
        let source = "| L | C | R |\n|:---|:---:|---:|\n| a | b | c |\n";
        let doc = Document::from_source("x.md", source.to_string());
        let lines = layout(&doc, 30);
        let table_lines: Vec<_> = lines.iter().filter(|l| l.block == 0).collect();
        assert_eq!(table_lines.len(), 2);
        assert_eq!(table_lines[0].text, "L   │  C  │   R");
        assert_eq!(table_lines[1].text, "a   │  b  │   c");
        assert!(matches!(table_lines[0].kind, VisualKind::TableHeader));
        assert!(matches!(table_lines[1].kind, VisualKind::Text));
    }

    #[test]
    fn table_wraps_cells_to_fit_narrow_width() {
        let source = "| A | B |\n| --- | --- |\n| alpha beta gamma | delta epsilon |\n";
        let doc = Document::from_source("x.md", source.to_string());
        let width = 24;
        let lines = layout(&doc, width);
        let table_lines: Vec<_> = lines.iter().filter(|l| l.block == 0).collect();
        assert!(table_lines.len() >= 3, "{table_lines:?}");
        assert!(
            table_lines.iter().all(|l| l.text.chars().count() <= width),
            "{table_lines:?}"
        );
        let joined: String = table_lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(joined.contains("gamma"), "{joined:?}");
        assert!(joined.contains("epsilon"), "{joined:?}");
    }

    #[test]
    fn table_columns_align_with_cjk_text() {
        let source = "| 名前 | B |\n| --- | --- |\n| x | y |\n";
        let doc = Document::from_source("x.md", source.to_string());
        let lines = layout(&doc, 30);
        let table_lines: Vec<_> = lines.iter().filter(|l| l.block == 0).collect();
        let bars: Vec<usize> = table_lines
            .iter()
            .map(|l| {
                let index = l.text.find('│').expect("separator");
                unicode_width::UnicodeWidthStr::width(&l.text[..index])
            })
            .collect();
        assert!(bars.windows(2).all(|w| w[0] == w[1]), "{bars:?}");
    }

    #[test]
    fn table_lines_map_to_source_ranges() {
        let source = "| Action | Vim |\n| --- | --- |\n| Move line | j |\n";
        let doc = Document::from_source("x.md", source.to_string());
        let lines = layout(&doc, 40);
        let row = lines
            .iter()
            .find(|l| l.text.contains("Move line"))
            .expect("row");
        assert!(row.source.start < row.source.end);
        assert!(doc.text_in_range(row.source.clone()).contains("Move line"));
        assert!(!row.source_lines.is_empty());
    }

    #[test]
    fn table_with_ragged_rows_does_not_panic() {
        let source = "| A | B | C |\n| --- | --- | --- |\n| 1 |\n| 2 | 3 |\n";
        let doc = Document::from_source("x.md", source.to_string());
        let lines = layout(&doc, 30);
        assert!(lines.iter().any(|l| l.text.contains("1")), "{lines:?}");
        assert!(lines.iter().any(|l| l.text.contains("3")), "{lines:?}");

        let header_only =
            Document::from_source("x.md", "| A | B |\n| --- | --- |\n".to_string());
        let lines = layout(&header_only, 30);
        assert!(lines.iter().any(|l| l.text.contains("A")), "{lines:?}");
    }

    #[test]
    fn table_in_blockquote_keeps_quote_prefix() {
        let source = "> | A | B |\n> | --- | --- |\n> | 1 | 2 |\n";
        let doc = Document::from_source("x.md", source.to_string());
        let lines = layout(&doc, 40);
        let table_lines: Vec<_> = lines.iter().filter(|l| l.block == 0).collect();
        assert!(!table_lines.is_empty());
        assert!(
            table_lines[0].text.starts_with("│ "),
            "{:?}",
            table_lines[0].text
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `source scripts/env.sh && cargo test --all-features table_renders_aligned_columns_without_separator_row`
Expected: FAIL — the table still renders in the flattened form (contains `---`).

Run: `source scripts/env.sh && cargo test --all-features table_honors_column_alignment`
Expected: FAIL — no `│` separator or no `VisualKind::TableHeader` yet.

- [ ] **Step 3: Implement aligned table layout**

In `src/document/layout.rs`:

1. Add `TableHeader` to `VisualKind`.
2. Import `ColumnAlign`, `Table`, `TableCell` from `super`.
3. In `layout`, replace the temporary Table arm with `layout_table(block, table, width, document, &mut out)` when `block.table` is `Some`.
4. Define `const MIN_COLUMN: usize = 3;` and `const SEPARATOR: &str = " │ ";`.
5. Implement `layout_table`:
   - Build `prefix` with `push_prefix`; `prefix_width` is the sum of its cell widths; `available = width.saturating_sub(prefix_width)`.
   - `columns = max(header.len(), rows max len, alignments.len())`; return if zero.
   - `separator_width = SEPARATOR width * (columns - 1)`; if `available < columns * MIN_COLUMN + separator_width`, append `prefix` plus `flatten_table_cells(table, block.source.start)` and return `wrap_cells` output (the Task 1 fallback).
   - Natural widths: start at `MIN_COLUMN`, raise each column to the widest cell (sum of grapheme widths) using `cell_cells`.
   - While the sum of widths exceeds `available - separator_width`, decrement the currently widest column that is above `MIN_COLUMN` by one (`max_by_key`; break if none).
   - Render rows (header first when non-empty, then body). For each row, wrap every cell with `wrap_cell` to its column width; row height is the maximum wrapped line count (at least 1). For each line index, start from a clone of `prefix`, then for each column append `SEPARATOR` between columns (`push_str`, `InlineStyle::Plain`), and `push_padded` the cell's line (or an empty slice) to the column width using the column's alignment.
   - Emit each composed line with `make_line_with_kind`; header rows use `VisualKind::TableHeader`, body rows `VisualKind::Text`.
6. Implement `cell_cells` with the existing `push_inlines`.
7. Implement `wrap_cell` as a grapheme-level wrap at `width`, preferring the last space in the current line and hard-breaking otherwise; trim trailing spaces on each output line. Return `Vec::new()` for empty input.
8. Implement `push_padded`: compute padding, then push left padding (spaces, `InlineStyle::Plain`, zero-width source at `source`), the content cells, and right padding; `Right` puts all padding left, `Center` splits it with the extra cell on the right, `Left`/`None` put it right.
9. Refactor `make_line` to delegate to `make_line_with_kind(cells, block, document, VisualKind::Text)`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `source scripts/env.sh && cargo test --all-features table_renders_aligned_columns_without_separator_row`
Expected: PASS.

Run: `source scripts/env.sh && cargo test --all-features table_`
Expected: PASS for every table parser/layout test.

Run: `source scripts/env.sh && cargo test --all-features`
Expected: all tests pass, including `narrow_table_falls_back_to_flattened_text`.

- [ ] **Step 5: Run fmt, clippy, and the full suite**

Run: `source scripts/env.sh && cargo fmt`
Then: `source scripts/env.sh && cargo fmt --check`
Expected: no output.

Run: `source scripts/env.sh && cargo clippy --all-targets --all-features -- -D warnings`
Expected: no warnings.

Run: `source scripts/env.sh && cargo test --all-features`
Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/document/layout.rs
git commit -m "Lay out tables with aligned, wrapped columns" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 3: Style table headers and separators

**Files:**
- Modify: `src/document/mod.rs` (`InlineStyle`)
- Modify: `src/document/layout.rs` (separator style)
- Modify: `src/tui/render.rs` (`inline_style` arm, header line styling)
- Test: `tests/tui.rs`

**Interfaces:**
- Consumes: `VisualKind::TableHeader` from Task 2.
- Produces: `InlineStyle::TableSeparator`; header lines render bold cyan; separator cells render dark gray.

- [ ] **Step 1: Write the failing TUI test**

Add to `tests/tui.rs` (and add `use ratatui::style::{Color, Modifier};` at the top):

```rust
#[test]
fn table_header_is_bold_cyan_and_separators_are_dim() {
    let (mut app, _dir) =
        fixture("# Spec\n\n| Action | Vim |\n| --- | --- |\n| Move | j |\n");
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| render::draw(frame, &mut app))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let mut header = None;
    for y in 0..buffer.area.height {
        let row: String = (0..buffer.area.width)
            .map(|x| buffer[(x, y)].symbol())
            .collect();
        if row.contains("Action") {
            header = Some((row, y));
            break;
        }
    }
    let (row, y) = header.expect("table header row");
    assert!(!row.contains("---"), "{row:?}");
    let x = row.find("Action").unwrap() as u16;
    let cell = &buffer[(x, y)];
    assert_eq!(cell.fg, Color::Cyan);
    assert!(cell.modifier.contains(Modifier::BOLD));
    let bar_x = row.find('│').expect("separator") as u16;
    assert_eq!(buffer[(bar_x, y)].fg, Color::DarkGray);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `source scripts/env.sh && cargo test --all-features table_header_is_bold_cyan_and_separators_are_dim`
Expected: FAIL — header is not cyan/bold and the separator is not dark gray.

- [ ] **Step 3: Implement the styling**

1. In `src/document/mod.rs`, add `TableSeparator` to `InlineStyle`.
2. In `src/document/layout.rs`, change the `SEPARATOR` pushes in `layout_table` to use `InlineStyle::TableSeparator` (padding stays `InlineStyle::Plain`).
3. In `src/tui/render.rs` `inline_style`, add `InlineStyle::TableSeparator => Style::default().fg(Color::DarkGray)`.
4. In `render_line`, after the heading style block, add:
   ```rust
   if matches!(line.kind, VisualKind::TableHeader) {
       style = style.fg(Color::Cyan).add_modifier(Modifier::BOLD);
   }
   ```

- [ ] **Step 4: Run the test to verify it passes**

Run: `source scripts/env.sh && cargo test --all-features table_header_is_bold_cyan_and_separators_are_dim`
Expected: PASS.

- [ ] **Step 5: Run fmt, clippy, and the full suite**

Run: `source scripts/env.sh && cargo fmt`
Then: `source scripts/env.sh && cargo fmt --check`
Expected: no output.

Run: `source scripts/env.sh && cargo clippy --all-targets --all-features -- -D warnings`
Expected: no warnings.

Run: `source scripts/env.sh && cargo test --all-features`
Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/document/mod.rs src/document/layout.rs src/tui/render.rs tests/tui.rs
git commit -m "Style table headers and column separators" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 4: Update specs and resolve review item c1

**Files:**
- Modify: `specs/003-document-model.md`
- Modify: `specs/006-tui.md`
- Modify: `specs/009-qa.md`
- Include: `docs/superpowers/specs/2026-11-22-table-rendering-design.md`, `docs/superpowers/plans/2026-11-22-table-rendering.md`

**Interfaces:**
- Consumes: the implemented behavior from Tasks 1–3.
- Produces: specs that require aligned table rendering; `c1` resolved.

- [ ] **Step 1: Update spec 003**

In the `Block` code block add `pub table: Option<Table>,   // structured GFM table data` after `pub inlines: Vec<Inline>,`. After the `Inline` block, add:

```rust
pub struct Table {
    pub header: Vec<TableCell>,
    pub rows: Vec<Vec<TableCell>>,
    pub alignments: Vec<ColumnAlign>,
}

pub struct TableCell {
    pub inlines: Vec<Inline>,
    pub source: Range<usize>,
}

pub enum ColumnAlign { None, Left, Center, Right }
```

Replace the line "Tables are one block; cells are inlines with source ranges. Tables may render as aligned text." with:

```markdown
- Tables are one block. The block retains header and body rows, each cell as styled inlines with a source range, plus per-column alignment.
- Rendered tables use aligned columns and a styled header row; cell text wraps within its column, and a table that cannot fit minimum column widths falls back to flattened pipe-separated text.
```

In the Source map section, add:

```markdown
- Table lines carry the byte range of the cells they display; header lines are marked so the renderer can style them.
```

- [ ] **Step 2: Update spec 006**

In the Layout section, after the rendered-Markdown bullet, add:

```markdown
- GFM tables render as aligned columns with a `│` separator and a bold cyan header row; the Markdown separator row is not shown. Cell text wraps within its column when the table exceeds the document width; if minimum column widths do not fit, the table falls back to flattened pipe-separated text.
```

In the Tests section, add:

```markdown
- Table alignment, header styling, cell wrapping, CJK widths, and narrow-width fallback.
```

- [ ] **Step 3: Update spec 009**

In the acceptance matrix, after the "Markdown parser and source map" row, add:

```markdown
| Tables | Parser rows/cells/styles/alignment; aligned layout, wrapping, CJK, narrow fallback |
```

- [ ] **Step 4: Run the full verification suite**

Run: `source scripts/env.sh && cargo fmt`
Then: `source scripts/env.sh && cargo fmt --check`
Expected: no output.

Run: `source scripts/env.sh && cargo clippy --all-targets --all-features -- -D warnings`
Expected: no warnings.

Run: `source scripts/env.sh && cargo test --all-features`
Expected: all tests pass.

- [ ] **Step 5: Resolve the review item**

Run: `mdreview export specs/006-tui.md`
Expected: `c1` is listed as open or stale (the document changed, so its anchor may be stale); it is still the only item.

Run: `mdreview resolve specs/006-tui.md c1`
Expected: `Resolved: c1`.

Run: `mdreview status specs/006-tui.md`
Expected: `Open: 0 | Stale: 0 | Resolved: 1`.

- [ ] **Step 6: Commit**

```bash
git add specs/003-document-model.md specs/006-tui.md specs/009-qa.md docs/superpowers/specs/2026-11-22-table-rendering-design.md docs/superpowers/plans/2026-11-22-table-rendering.md
git commit -m "Specify aligned table rendering and resolve c1" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```
