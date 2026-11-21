# 003 — Markdown document model and source map

## Purpose

Parse a Markdown file into a flat block model that preserves byte and line source ranges, heading context, and enough inline structure to render it in a terminal and anchor comments precisely.

## Types

```rust
pub struct Document {
    pub path: PathBuf,
    pub source: String,
    pub hash: String,          // SHA-256 hex of source bytes
    pub blocks: Vec<Block>,
    pub line_starts: Vec<usize>, // byte offset of each 1-based source line
}

pub struct Block {
    pub index: usize,
    pub kind: BlockKind,
    pub source: Range<usize>,   // byte offsets in Document.source
    pub lines: Range<usize>,    // 1-based inclusive start, exclusive end
    pub heading_path: Vec<String>,
    pub depth: usize,           // list/quote nesting
    pub inlines: Vec<Inline>,
}

pub enum BlockKind {
    Heading(u8), Paragraph, ListItem { ordered: bool, marker: String },
    CodeBlock { lang: Option<String> }, Table, Blockquote, Rule,
    Image { src: String, alt: String, title: Option<String> },
    Mermaid { source: String }, Html, Footnote,
}

pub struct Inline {
    pub text: String,
    pub style: InlineStyle,     // plain, emphasis, strong, code, link, image-alt
    pub source: Range<usize>,
}
```

## Parsing rules

- Use `pulldown-cmark` with tables, task lists, strikethrough, and footnotes enabled.
- Use `into_offset_iter()` so every block and inline keeps a byte range.
- Heading paths: maintain a stack of `(level, text)`. A heading block's path includes itself; following blocks inherit the stack.
- A fenced code block with language `mermaid` becomes `BlockKind::Mermaid`; its source is the code content excluding the fences.
- A paragraph containing only an image becomes `BlockKind::Image`; an image inside mixed text remains an inline and is not previewed.
- Nested lists and blockquotes increment `depth`; each list item is a separate block.
- Tables are one block; cells are inlines with source ranges. Tables may render as aligned text.
- HTML is kept as raw text and rendered literally.
- Rule (`---`) is a block with no inlines.
- Empty or whitespace-only blocks are skipped.

## Source map

- `line_of(byte)` and `lines_of(range)` use `line_starts`.
- `source_line(line)` returns the 1-based line text without the newline.
- Layout (`document::layout`) wraps styled spans to a display width using `unicode-width`:
  - Each `VisualLine` carries `source: Range<usize>` and `source_lines: Range<usize>`.
  - Wrapped continuation lines keep the source range of the wrapped segment.
  - Long unbreakable tokens are split at grapheme boundaries.
  - Tabs expand to four spaces.
- Media blocks reserve a rectangle in layout; the text pass never writes into it.

## Acceptance

- Fixtures cover headings, nested lists, task lists, tables, code fences (Rust and Mermaid), blockquotes, links, images, rules, HTML, footnotes, emphasis, and Unicode/wide characters.
- Every block's `source` slices back to a non-empty source string (except rules).
- Every `source_lines` range maps to the block's byte range.
- A 2,000-line fixture parses and lays out without panic.

## Tests

- Block-kind and heading-path assertions on fixtures.
- Byte/line range round-trips.
- Mermaid and image detection.
- Wrapping at 20, 40, and 80 columns, including CJK and emoji.
- Empty input, CRLF input, and a file without a trailing newline.
