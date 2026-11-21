//! Layout: turn blocks and inline spans into wrapped visual lines with source maps.

use super::{BlockKind, Document, Inline, InlineStyle};
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

pub fn layout(document: &Document, width: usize) -> Vec<VisualLine> {
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
                for _ in 0..IMAGE_ROWS {
                    out.push(media_line(
                        &label,
                        block.source.clone(),
                        block.index,
                        MediaKind::Image,
                        document,
                    ));
                }
            }
            BlockKind::Mermaid { .. } => {
                for _ in 0..MERMAID_ROWS {
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
    let mut cells = cells;
    while cells.last().is_some_and(|cell| cell.text == " ") {
        cells = &cells[..cells.len() - 1];
    }
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
        kind: VisualKind::Text,
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
}
