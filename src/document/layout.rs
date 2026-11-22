//! Layout: turn blocks and inline spans into wrapped visual lines with source maps.

use super::{BlockKind, ColumnAlign, Document, Inline, InlineStyle, Table, TableCell};
use std::collections::HashMap;
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyledSpan {
    pub text: String,
    pub style: InlineStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Image,
    Mermaid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VisualKind {
    Text,
    TableHeader,
    Media(MediaKind),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisualLine {
    pub spans: Vec<StyledSpan>,
    pub text: String,
    pub source: Range<usize>,
    pub source_lines: Range<usize>,
    pub block: usize,
    pub kind: VisualKind,
}

impl VisualLine {
    pub fn is_blank(&self) -> bool {
        self.text.trim().is_empty()
    }

    pub fn is_media(&self) -> bool {
        matches!(self.kind, VisualKind::Media(_))
    }
}

const IMAGE_ROWS: usize = 8;
const MERMAID_ROWS: usize = 10;
const MIN_COLUMN: usize = 3;
const SEPARATOR: &str = " │ ";

/// Fit an aspect ratio (cell columns / cell rows) into a page, preserving the
/// aspect. Width is preferred; the result is capped at `max_height`. A missing
/// or bogus aspect falls back to `fallback_rows`.
pub fn media_cell_size(
    aspect: f64,
    max_width: usize,
    max_height: usize,
    fallback_rows: usize,
) -> (usize, usize) {
    let max_width = max_width.max(1);
    if !aspect.is_finite() || aspect <= 0.0 || max_height == 0 {
        return (max_width, fallback_rows.max(1));
    }
    let rows_at_width = (max_width as f64 / aspect).ceil().max(1.0) as usize;
    if rows_at_width <= max_height {
        (max_width, rows_at_width)
    } else {
        let cols = (max_height as f64 * aspect).floor().max(1.0) as usize;
        (cols.min(max_width), max_height)
    }
}

fn media_rows(
    block: usize,
    aspects: &HashMap<usize, f64>,
    width: usize,
    max_height: usize,
    fallback: usize,
) -> usize {
    match aspects.get(&block) {
        Some(aspect) => media_cell_size(*aspect, width, max_height, fallback).1,
        None => fallback.max(1),
    }
}

pub fn layout(document: &Document, width: usize) -> Vec<VisualLine> {
    layout_with_media(document, width, &HashMap::new(), 0)
}

/// Lay out a document with media blocks sized to the page. `aspects` maps a
/// block index to its image aspect ratio in terminal cells (width / height);
/// `max_height` is the document viewport height. A missing aspect or a zero
/// height falls back to the fixed media-row defaults.
pub fn layout_with_media(
    document: &Document,
    width: usize,
    aspects: &HashMap<usize, f64>,
    max_height: usize,
) -> Vec<VisualLine> {
    let width = width.max(1);
    let mut out = Vec::new();
    for (index, block) in document.blocks.iter().enumerate() {
        if index > 0 {
            out.push(blank_line(block.source.start, block.index, document));
        }
        match &block.kind {
            BlockKind::Image { src, alt, .. } => {
                let label = if alt.is_empty() {
                    format!("[image: {src}]")
                } else {
                    format!("[image: {alt}] ({src})")
                };
                if is_remote_image(src) {
                    let mut cells = Vec::new();
                    push_str(&mut cells, &label, InlineStyle::Link, block.source.start);
                    for cell in &mut cells {
                        cell.source = block.source.clone();
                    }
                    let mut lines = wrap_cells(&cells, width, block.index, document);
                    if lines.is_empty() {
                        lines.push(blank_line(block.source.start, block.index, document));
                    }
                    out.extend(lines);
                } else {
                    let rows = media_rows(block.index, aspects, width, max_height, IMAGE_ROWS);
                    for _ in 0..rows {
                        out.push(media_line(
                            &label,
                            block.source.clone(),
                            block.index,
                            MediaKind::Image,
                            document,
                        ));
                    }
                }
            }
            BlockKind::Mermaid { .. } => {
                let rows = media_rows(block.index, aspects, width, max_height, MERMAID_ROWS);
                for _ in 0..rows {
                    out.push(media_line(
                        "[mermaid diagram — Enter toggles source]",
                        block.source.clone(),
                        block.index,
                        MediaKind::Mermaid,
                        document,
                    ));
                }
            }
            BlockKind::Rule => {
                let text = "─".repeat(width.min(40));
                out.push(VisualLine {
                    spans: vec![StyledSpan {
                        text: text.clone(),
                        style: InlineStyle::Plain,
                    }],
                    text,
                    source: block.source.clone(),
                    source_lines: block.lines.clone(),
                    block: block.index,
                    kind: VisualKind::Text,
                });
            }
            BlockKind::Table => {
                if let Some(table) = block.table.as_ref() {
                    layout_table(block, table, width, document, &mut out);
                }
            }
            _ => {
                let mut cells = Vec::new();
                push_prefix(&mut cells, block);
                push_inlines(&mut cells, &block.inlines, block.source.start);
                let mut lines = wrap_cells(&cells, width, block.index, document);
                if lines.is_empty() {
                    lines.push(blank_line(block.source.start, block.index, document));
                }
                out.extend(lines);
            }
        }
    }
    out
}

fn push_prefix(cells: &mut Vec<Cell>, block: &super::Block) {
    let source = block.source.start;
    if block.quote_depth > 0 {
        for _ in 0..block.quote_depth {
            push_str(cells, "│ ", InlineStyle::Plain, source);
        }
    }
    if let BlockKind::ListItem { marker, .. } = &block.kind {
        for _ in 0..block.depth.saturating_sub(block.quote_depth) {
            push_str(cells, "  ", InlineStyle::Plain, source);
        }
        push_str(cells, marker, InlineStyle::Plain, source);
    }
}

fn flatten_table_cells(table: &Table, fallback: usize) -> Vec<Cell> {
    let mut cells = Vec::new();
    let mut rows: Vec<&[TableCell]> = Vec::new();
    if !table.header.is_empty() {
        rows.push(&table.header);
    }
    rows.extend(table.rows.iter().map(Vec::as_slice));
    for (row_index, row) in rows.iter().enumerate() {
        if row_index > 0 {
            push_str(&mut cells, "\n", InlineStyle::Plain, fallback);
        }
        for (cell_index, cell) in row.iter().enumerate() {
            if cell_index > 0 {
                push_str(&mut cells, " | ", InlineStyle::Plain, fallback);
            }
            push_inlines(&mut cells, &cell.inlines, cell.source.start);
        }
        if row_index == 0 && !table.header.is_empty() {
            push_str(&mut cells, "\n", InlineStyle::Plain, fallback);
            let separator = row.iter().map(|_| "---").collect::<Vec<_>>().join(" | ");
            push_str(&mut cells, &separator, InlineStyle::Plain, fallback);
        }
    }
    cells
}

fn layout_table(
    block: &super::Block,
    table: &Table,
    width: usize,
    document: &Document,
    out: &mut Vec<VisualLine>,
) {
    let mut prefix = Vec::new();
    push_prefix(&mut prefix, block);
    let prefix_width: usize = prefix.iter().map(|cell| cell.width).sum();
    let available = width.saturating_sub(prefix_width);

    let columns = table
        .header
        .len()
        .max(table.rows.iter().map(Vec::len).max().unwrap_or(0))
        .max(table.alignments.len());
    if columns == 0 {
        return;
    }

    let separator_width = SEPARATOR.width() * columns.saturating_sub(1);
    if available < columns * MIN_COLUMN + separator_width {
        push_flattened_table(block, table, width, document, out);
        return;
    }

    let mut widths = vec![MIN_COLUMN; columns];
    for row in std::iter::once(&table.header).chain(table.rows.iter()) {
        for (col, cell) in row.iter().enumerate().take(columns) {
            let cell_width: usize = cell_cells(cell).iter().map(|cell| cell.width).sum();
            widths[col] = widths[col].max(cell_width);
        }
    }

    let content_available = available - separator_width;
    while widths.iter().sum::<usize>() > content_available {
        let Some((index, _)) = widths
            .iter()
            .enumerate()
            .filter(|(_, width)| **width > MIN_COLUMN)
            .max_by_key(|(_, width)| **width)
        else {
            push_flattened_table(block, table, width, document, out);
            return;
        };
        widths[index] -= 1;
    }

    let mut rows: Vec<(&[TableCell], bool)> = Vec::new();
    if !table.header.is_empty() {
        rows.push((&table.header, true));
    }
    for row in &table.rows {
        rows.push((row.as_slice(), false));
    }

    for (row, header) in rows {
        let wrapped: Vec<Vec<Vec<Cell>>> = (0..columns)
            .map(|col| {
                let cells = row.get(col).map(cell_cells).unwrap_or_default();
                wrap_cell(&cells, widths[col])
            })
            .collect();
        let row_source = union_ranges(row.iter().map(|cell| cell.source.clone()))
            .unwrap_or_else(|| block.source.clone());
        let height = wrapped.iter().map(Vec::len).max().unwrap_or(1).max(1);
        for line_index in 0..height {
            let mut line_cells = prefix.clone();
            let mut content_ranges: Vec<Range<usize>> = Vec::new();
            for col in 0..columns {
                if col > 0 {
                    push_str(
                        &mut line_cells,
                        SEPARATOR,
                        InlineStyle::TableSeparator,
                        block.source.start,
                    );
                }
                let empty = Vec::new();
                let content = wrapped[col].get(line_index).unwrap_or(&empty);
                content_ranges.extend(content.iter().map(|cell| cell.source.clone()));
                let align = table.alignments.get(col).copied().unwrap_or_default();
                let source = row
                    .get(col)
                    .map(|cell| cell.source.start)
                    .unwrap_or(block.source.start);
                push_padded(&mut line_cells, content, widths[col], align, source);
            }
            let kind = if header {
                VisualKind::TableHeader
            } else {
                VisualKind::Text
            };
            let source = union_ranges(content_ranges).unwrap_or_else(|| row_source.clone());
            if let Some(line) =
                make_line_with_source(&line_cells, block.index, document, kind, source)
            {
                out.push(line);
            }
        }
    }
}

fn push_flattened_table(
    block: &super::Block,
    table: &Table,
    width: usize,
    document: &Document,
    out: &mut Vec<VisualLine>,
) {
    let mut cells = Vec::new();
    push_prefix(&mut cells, block);
    cells.extend(flatten_table_cells(table, block.source.start));
    let mut lines = wrap_cells(&cells, width, block.index, document);
    for line in &mut lines {
        if line.source.start >= line.source.end {
            line.source = block.source.clone();
            line.source_lines = document.lines_of(line.source.clone());
        }
    }
    if lines.is_empty() {
        lines.push(blank_line(block.source.start, block.index, document));
    }
    out.extend(lines);
}

fn cell_cells(cell: &TableCell) -> Vec<Cell> {
    let mut cells = Vec::new();
    push_inlines(&mut cells, &cell.inlines, cell.source.start);
    cells
}

fn wrap_cell(cells: &[Cell], width: usize) -> Vec<Vec<Cell>> {
    if cells.is_empty() {
        return Vec::new();
    }
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut start = 0usize;
    let mut line_width = 0usize;
    let mut last_space: Option<usize> = None;

    for (index, cell) in cells.iter().enumerate() {
        let w = cell.width;
        if line_width + w > width && index > start {
            let break_at = last_space.filter(|space| *space > start).unwrap_or(index);
            lines.push(trim_cells(&cells[start..break_at]).to_vec());
            start = if break_at < cells.len() && cells[break_at].text == " " {
                break_at + 1
            } else {
                break_at
            };
            line_width = 0;
            last_space = None;
            if start > index {
                continue;
            }
            for carried in &cells[start..index] {
                line_width += carried.width;
            }
        }
        if cell.text == " " {
            last_space = Some(index);
        }
        line_width += w;
    }
    if start < cells.len() {
        lines.push(trim_cells(&cells[start..]).to_vec());
    }
    lines
}

fn trim_cells(mut cells: &[Cell]) -> &[Cell] {
    while cells.last().is_some_and(|cell| cell.text == " ") {
        cells = &cells[..cells.len() - 1];
    }
    cells
}

fn union_ranges(ranges: impl IntoIterator<Item = Range<usize>>) -> Option<Range<usize>> {
    let mut union: Option<Range<usize>> = None;
    for range in ranges {
        if range.start >= range.end {
            continue;
        }
        union = Some(match union {
            Some(existing) => existing.start.min(range.start)..existing.end.max(range.end),
            None => range,
        });
    }
    union
}

fn push_padded(
    line: &mut Vec<Cell>,
    content: &[Cell],
    col_width: usize,
    align: ColumnAlign,
    source: usize,
) {
    let used: usize = content.iter().map(|cell| cell.width).sum();
    let padding = col_width.saturating_sub(used);
    let (left, right) = match align {
        ColumnAlign::Right => (padding, 0),
        ColumnAlign::Center => (padding / 2, padding - padding / 2),
        ColumnAlign::Left | ColumnAlign::None => (0, padding),
    };
    for _ in 0..left {
        line.push(Cell {
            text: " ".to_string(),
            width: 1,
            style: InlineStyle::Plain,
            source: source..source,
        });
    }
    line.extend(content.iter().cloned());
    for _ in 0..right {
        line.push(Cell {
            text: " ".to_string(),
            width: 1,
            style: InlineStyle::Plain,
            source: source..source,
        });
    }
}

fn push_inlines(cells: &mut Vec<Cell>, inlines: &[Inline], fallback: usize) {
    for inline in inlines {
        let mut offset = inline.source.start;
        for grapheme in inline.text.graphemes(true) {
            let bytes = grapheme.len();
            let source = if inline.source.start == inline.source.end {
                fallback..fallback
            } else {
                offset..(offset + bytes).min(inline.source.end.max(offset + bytes))
            };
            if grapheme == "\t" {
                for _ in 0..4 {
                    cells.push(Cell {
                        text: " ".to_string(),
                        width: 1,
                        style: inline.style.clone(),
                        source: source.clone(),
                    });
                }
            } else {
                cells.push(Cell {
                    text: grapheme.to_string(),
                    width: UnicodeWidthStr::width(grapheme),
                    style: inline.style.clone(),
                    source,
                });
            }
            offset += bytes;
        }
    }
}

fn push_str(cells: &mut Vec<Cell>, text: &str, style: InlineStyle, source: usize) {
    for grapheme in text.graphemes(true) {
        if grapheme == "\t" {
            for _ in 0..4 {
                cells.push(Cell {
                    text: " ".to_string(),
                    width: 1,
                    style: style.clone(),
                    source: source..source,
                });
            }
        } else {
            cells.push(Cell {
                text: grapheme.to_string(),
                width: UnicodeWidthStr::width(grapheme),
                style: style.clone(),
                source: source..source,
            });
        }
    }
}

#[derive(Debug, Clone)]
struct Cell {
    text: String,
    width: usize,
    style: InlineStyle,
    source: Range<usize>,
}

fn wrap_cells(cells: &[Cell], width: usize, block: usize, document: &Document) -> Vec<VisualLine> {
    let mut lines = Vec::new();
    let mut start = 0usize;
    let mut line_width = 0usize;
    let mut last_space: Option<usize> = None;

    let flush = |end: usize,
                 lines: &mut Vec<VisualLine>,
                 start: &mut usize,
                 line_width: &mut usize,
                 last_space: &mut Option<usize>| {
        if end <= *start {
            return;
        }
        let slice = &cells[*start..end];
        if let Some(line) = make_line(slice, block, document) {
            lines.push(line);
        }
        *start = end;
        *line_width = 0;
        *last_space = None;
    };

    for (index, cell) in cells.iter().enumerate() {
        if cell.text == "\n" {
            flush(
                index,
                &mut lines,
                &mut start,
                &mut line_width,
                &mut last_space,
            );
            start = index + 1;
            line_width = 0;
            last_space = None;
            continue;
        }
        let w = cell.width;
        if line_width + w > width && index > start {
            let break_at = last_space.filter(|space| *space > start).unwrap_or(index);
            flush(
                break_at,
                &mut lines,
                &mut start,
                &mut line_width,
                &mut last_space,
            );
            if break_at < cells.len() && cells[break_at].text == " " {
                start = break_at + 1;
            }
            if start > index {
                continue;
            }
            // Recompute the width of the carried-over part.
            for carried in &cells[start..index] {
                line_width += carried.width;
            }
        }
        if cell.text == " " {
            last_space = Some(index);
        }
        line_width += w;
    }
    flush(
        cells.len(),
        &mut lines,
        &mut start,
        &mut line_width,
        &mut last_space,
    );
    lines
}

fn make_line(cells: &[Cell], block: usize, document: &Document) -> Option<VisualLine> {
    make_line_with_kind(cells, block, document, VisualKind::Text)
}

fn make_line_with_kind(
    cells: &[Cell],
    block: usize,
    document: &Document,
    kind: VisualKind,
) -> Option<VisualLine> {
    let cells = trim_cells(cells);
    if cells.is_empty() {
        return None;
    }
    let source_start = cells.first().map(|c| c.source.start).unwrap_or(0);
    let source_end = cells
        .last()
        .map(|c| {
            if c.source.end > c.source.start {
                c.source.end
            } else {
                c.source.start
            }
        })
        .unwrap_or(source_start);
    let source = source_start..source_end.max(source_start);
    make_line_with_source(cells, block, document, kind, source)
}

fn make_line_with_source(
    cells: &[Cell],
    block: usize,
    document: &Document,
    kind: VisualKind,
    source: Range<usize>,
) -> Option<VisualLine> {
    let cells = trim_cells(cells);
    if cells.is_empty() {
        return None;
    }
    let source_lines = document.lines_of(source.clone());

    let mut spans: Vec<StyledSpan> = Vec::new();
    for cell in cells {
        if let Some(last) = spans.last_mut()
            && last.style == cell.style
        {
            last.text.push_str(&cell.text);
            continue;
        }
        spans.push(StyledSpan {
            text: cell.text.clone(),
            style: cell.style.clone(),
        });
    }
    let text: String = cells.iter().map(|c| c.text.as_str()).collect();

    Some(VisualLine {
        spans,
        text,
        source,
        source_lines,
        block,
        kind,
    })
}

fn blank_line(source: usize, block: usize, document: &Document) -> VisualLine {
    let range = source..source;
    VisualLine {
        spans: Vec::new(),
        text: String::new(),
        source: range.clone(),
        source_lines: document.lines_of(range),
        block,
        kind: VisualKind::Text,
    }
}

fn is_remote_image(src: &str) -> bool {
    let lower = src.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

fn media_line(
    label: &str,
    source: Range<usize>,
    block: usize,
    kind: MediaKind,
    document: &Document,
) -> VisualLine {
    VisualLine {
        spans: vec![StyledSpan {
            text: label.to_string(),
            style: InlineStyle::Plain,
        }],
        text: label.to_string(),
        source: source.clone(),
        source_lines: document.lines_of(source),
        block,
        kind: VisualKind::Media(kind),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;

    #[test]
    fn wraps_and_maps_source() {
        let source = "alpha beta gamma delta epsilon\n";
        let doc = Document::from_source("x.md", source.to_string());
        let lines = layout(&doc, 12);
        assert!(lines.len() >= 3);
        assert!(lines[0].text.contains("alpha"));
        for line in &lines {
            let slice = &source[line.source.clone()];
            assert!(!slice.is_empty() || line.is_blank());
        }
    }

    #[test]
    fn unicode_width_wrapping() {
        let source = "你好世界你好世界\n";
        let doc = Document::from_source("x.md", source.to_string());
        let lines = layout(&doc, 6);
        assert!(lines.len() >= 3, "{lines:?}");
        assert!(lines.iter().all(|l| l.text.chars().count() <= 3));
    }

    #[test]
    fn remote_images_render_as_link_text_without_media_rows() {
        let source = "# Remote\n\n![diagram](https://example.com/a.png)\n";
        let doc = Document::from_source("x.md", source.to_string());
        let lines = layout(&doc, 80);
        assert!(lines.iter().all(|line| !line.is_media()), "{lines:?}");
        let link = lines
            .iter()
            .find(|line| line.text.contains("https://example.com/a.png"))
            .expect("remote URL line");
        assert!(link.text.contains("diagram"));
        assert!(
            link.spans
                .iter()
                .any(|span| span.style == InlineStyle::Link)
        );
    }

    #[test]
    fn media_blocks_reserve_rows() {
        let source = "![alt](x.png)\n\n```mermaid\nflowchart LR\nA-->B\n```\n";
        let doc = Document::from_source("x.md", source.to_string());
        let lines = layout(&doc, 40);
        assert!(lines.iter().filter(|l| l.is_media()).count() >= 18);
        assert!(lines.iter().any(|l| l.text.contains("mermaid")));
    }

    #[test]
    fn tabs_expand_to_four_spaces() {
        let source = "a\tb\n";
        let doc = Document::from_source("x.md", source.to_string());
        let lines = layout(&doc, 80);
        assert!(lines[0].text.contains("a    b"), "{:?}", lines[0].text);
    }

    #[test]
    fn emoji_graphemes_are_not_split() {
        let source = "👨‍👩‍👧‍👦 family\n";
        let doc = Document::from_source("x.md", source.to_string());
        let lines = layout(&doc, 4);
        assert!(
            lines.iter().any(|line| line.text.contains("👨‍👩‍👧‍👦")),
            "{lines:?}"
        );
        assert!(lines.iter().all(|line| {
            !line.text.contains('\u{200d}') || line.text.contains("👨‍👩‍👧‍👦")
        }));
    }

    #[test]
    fn narrow_table_falls_back_to_flattened_text() {
        let source = "| A | B | C | D |\n| --- | --- | --- | --- |\n| 1 | 2 | 3 | 4 |\n";
        let doc = Document::from_source("x.md", source.to_string());
        let lines = layout(&doc, 10);
        let text = lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("---"), "{text:?}");
        assert!(text.contains("1 | 2"), "{text:?}");
    }

    #[test]
    fn table_renders_aligned_columns_without_separator_row() {
        let source = "| Action | Vim |\n| --- | --- |\n| Move line | j |\n| Half page | Ctrl-d |\n";
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

        let header_only = Document::from_source("x.md", "| A | B |\n| --- | --- |\n".to_string());
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

    #[test]
    fn table_wrapping_never_panics() {
        let source = "| Action | Vim | Notes |\n| --- | --- | --- |\n| Move line down | j | moves the cursor down one line |\n| Half page | Ctrl-d | scrolls half a page |\n";
        let doc = Document::from_source("x.md", source.to_string());
        for width in 5..=120 {
            let _ = layout(&doc, width);
        }
    }

    #[test]
    fn paragraph_wrapping_never_panics_at_the_boundary() {
        let doc = Document::from_source("x.md", "abcdefghij klm\n".to_string());
        for width in 5..=20 {
            let _ = layout(&doc, width);
        }
    }

    #[test]
    fn every_non_blank_table_line_has_a_source_range() {
        let source = "| A | B | C |\n| --- | --- | --- |\n| alpha beta | gamma delta | |\n| 1 |\n";
        let doc = Document::from_source("x.md", source.to_string());
        for width in 5..=40 {
            let lines = layout(&doc, width);
            for line in lines.iter().filter(|l| l.block == 0 && !l.is_blank()) {
                assert!(
                    line.source.start < line.source.end,
                    "width {width}: {:?} src={:?}",
                    line.text,
                    line.source
                );
                assert!(line.source.end <= doc.source.len());
            }
        }
    }

    #[test]
    fn blockquoted_table_rows_map_to_the_row_source() {
        let source = "> | A | B |\n> | --- | --- |\n> | 1 | 2 |\n> | 3 | 4 |\n";
        let doc = Document::from_source("x.md", source.to_string());
        let lines = layout(&doc, 40);
        let row = lines.iter().find(|l| l.text.contains("3")).expect("row");
        let slice = doc.text_in_range(row.source.clone());
        assert!(slice.contains("3"), "{slice:?} src={:?}", row.source);
        assert!(
            !slice.contains('A'),
            "row range starts too early: {slice:?}"
        );
    }

    #[test]
    fn media_cell_size_fits_the_page() {
        assert_eq!(media_cell_size(10.0, 80, 20, 10), (80, 8));
        assert_eq!(media_cell_size(0.2, 80, 20, 10), (4, 20));
        assert_eq!(media_cell_size(1.0, 80, 20, 10), (20, 20));
        assert_eq!(media_cell_size(100.0, 80, 20, 10), (80, 1));
        assert_eq!(media_cell_size(f64::NAN, 80, 20, 10), (80, 10));
        assert_eq!(media_cell_size(10.0, 80, 0, 10), (80, 10));
    }

    #[test]
    fn mermaid_block_rows_follow_the_page_aspect() {
        let source = "# Title\n\n```mermaid\nflowchart LR\nA-->B\n```\n\nAfter.\n";
        let doc = Document::from_source("x.md", source.to_string());
        let block = doc
            .blocks
            .iter()
            .find(|block| matches!(block.kind, BlockKind::Mermaid { .. }))
            .expect("mermaid block")
            .index;
        let mut aspects = HashMap::new();
        aspects.insert(block, 10.0);
        let lines = layout_with_media(&doc, 80, &aspects, 20);
        let media = lines
            .iter()
            .filter(|line| line.block == block && line.is_media())
            .count();
        assert_eq!(media, 8);
        assert!(lines.iter().any(|line| line.text.contains("After")));
    }

    #[test]
    fn missing_media_aspect_uses_the_fallback_rows() {
        let source = "# Title\n\n```mermaid\nflowchart LR\nA-->B\n```\n\nAfter.\n";
        let doc = Document::from_source("x.md", source.to_string());
        let block = doc
            .blocks
            .iter()
            .find(|block| matches!(block.kind, BlockKind::Mermaid { .. }))
            .expect("mermaid block")
            .index;
        let lines = layout_with_media(&doc, 80, &HashMap::new(), 20);
        let media = lines
            .iter()
            .filter(|line| line.block == block && line.is_media())
            .count();
        assert_eq!(media, MERMAID_ROWS);
    }
}
