//! Markdown parsing into a flat block model with byte/line source ranges.

pub mod layout;

pub use layout::{
    MediaKind, StyledSpan, VisualKind, VisualLine, layout, layout_with_media, media_cell_size,
};

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use sha2::{Digest, Sha256};
use std::ops::Range;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineStyle {
    Plain,
    Emphasis,
    Strong,
    Strikethrough,
    Code,
    Link,
    ImageAlt,
    TaskMarker,
    TableSeparator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inline {
    pub text: String,
    pub style: InlineStyle,
    pub source: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableCell {
    pub inlines: Vec<Inline>,
    pub source: Range<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColumnAlign {
    #[default]
    None,
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Table {
    pub header: Vec<TableCell>,
    pub rows: Vec<Vec<TableCell>>,
    pub alignments: Vec<ColumnAlign>,
}

impl From<Alignment> for ColumnAlign {
    fn from(value: Alignment) -> Self {
        match value {
            Alignment::None => Self::None,
            Alignment::Left => Self::Left,
            Alignment::Center => Self::Center,
            Alignment::Right => Self::Right,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockKind {
    Heading(u8),
    Paragraph,
    ListItem {
        ordered: bool,
        marker: String,
    },
    CodeBlock {
        lang: Option<String>,
    },
    Table,
    Blockquote,
    Rule,
    Image {
        src: String,
        alt: String,
        title: Option<String>,
    },
    Mermaid {
        source: String,
    },
    Html,
    Footnote,
}

impl BlockKind {
    pub fn is_media(&self) -> bool {
        matches!(self, BlockKind::Image { .. } | BlockKind::Mermaid { .. })
    }

    pub fn label(&self) -> &'static str {
        match self {
            BlockKind::Heading(_) => "heading",
            BlockKind::Paragraph => "paragraph",
            BlockKind::ListItem { .. } => "list item",
            BlockKind::CodeBlock { .. } => "code",
            BlockKind::Table => "table",
            BlockKind::Blockquote => "quote",
            BlockKind::Rule => "rule",
            BlockKind::Image { .. } => "image",
            BlockKind::Mermaid { .. } => "mermaid",
            BlockKind::Html => "html",
            BlockKind::Footnote => "footnote",
        }
    }

    /// Stable snake_case identifier used by the editor bridge.
    pub fn kind_name(&self) -> &'static str {
        match self {
            BlockKind::Heading(_) => "heading",
            BlockKind::Paragraph => "paragraph",
            BlockKind::ListItem { .. } => "list_item",
            BlockKind::CodeBlock { .. } => "code_block",
            BlockKind::Table => "table",
            BlockKind::Blockquote => "blockquote",
            BlockKind::Rule => "rule",
            BlockKind::Image { .. } => "image",
            BlockKind::Mermaid { .. } => "mermaid",
            BlockKind::Html => "html",
            BlockKind::Footnote => "footnote",
        }
    }

    pub fn heading_level(&self) -> Option<u8> {
        match self {
            BlockKind::Heading(level) => Some(*level),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub index: usize,
    pub kind: BlockKind,
    pub source: Range<usize>,
    pub lines: Range<usize>,
    pub heading_path: Vec<String>,
    pub depth: usize,
    pub quote_depth: usize,
    pub inlines: Vec<Inline>,
    pub table: Option<Table>,
}

impl Block {
    pub fn text(&self) -> String {
        if let Some(table) = &self.table {
            let mut rows: Vec<&[TableCell]> = Vec::new();
            if !table.header.is_empty() {
                rows.push(&table.header);
            }
            rows.extend(table.rows.iter().map(Vec::as_slice));
            let mut text = String::new();
            for (row_index, row) in rows.iter().enumerate() {
                if row_index > 0 {
                    text.push('\n');
                }
                for (cell_index, cell) in row.iter().enumerate() {
                    if cell_index > 0 {
                        text.push_str(" | ");
                    }
                    for inline in &cell.inlines {
                        text.push_str(&inline.text);
                    }
                }
            }
            return text;
        }
        self.inlines.iter().map(|i| i.text.as_str()).collect()
    }
}

#[derive(Debug, Clone)]
pub struct Document {
    pub path: PathBuf,
    pub source: String,
    pub hash: String,
    pub blocks: Vec<Block>,
    pub line_starts: Vec<usize>,
}

impl Document {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, DocumentError> {
        let path = path.as_ref().to_path_buf();
        let source = std::fs::read_to_string(&path).map_err(|source| DocumentError::Io {
            path: path.clone(),
            source,
        })?;
        Ok(Self::from_source(path, source))
    }

    pub fn from_source(path: impl Into<PathBuf>, source: String) -> Self {
        let path = path.into();
        let hash = content_hash(&source);
        let line_starts = line_starts(&source);
        let mut parser = BlockParser::new(&source, &line_starts);
        let blocks = parser.parse();
        Self {
            path,
            source,
            hash,
            blocks,
            line_starts,
        }
    }

    pub fn line_of(&self, byte: usize) -> usize {
        if self.line_starts.is_empty() {
            return 1;
        }
        let byte = byte.min(self.source.len());
        match self.line_starts.binary_search(&byte) {
            Ok(index) => index + 1,
            Err(index) => index.max(1),
        }
    }

    pub fn line_count(&self) -> usize {
        self.line_starts.len().max(1)
    }

    pub fn lines_of(&self, range: Range<usize>) -> Range<usize> {
        let start = self.line_of(range.start);
        let end = if range.end > range.start {
            self.line_of(range.end.saturating_sub(1))
        } else {
            start
        };
        start..(end + 1)
    }

    pub fn source_line(&self, line: usize) -> Option<&str> {
        if line == 0 || line > self.line_starts.len() {
            return None;
        }
        let start = self.line_starts[line - 1];
        let end = self
            .line_starts
            .get(line)
            .copied()
            .unwrap_or(self.source.len());
        Some(self.source[start..end].trim_end_matches(['\r', '\n']))
    }

    pub fn block_at(&self, byte: usize) -> Option<&Block> {
        self.blocks
            .iter()
            .rev()
            .find(|block| block.source.start <= byte)
    }

    pub fn block_index_at(&self, byte: usize) -> Option<usize> {
        self.block_at(byte).map(|block| block.index)
    }

    pub fn headings(&self) -> Vec<&Block> {
        self.blocks
            .iter()
            .filter(|block| matches!(block.kind, BlockKind::Heading(_)))
            .collect()
    }

    pub fn text_in_range(&self, range: Range<usize>) -> &str {
        let start = clamp_boundary(&self.source, range.start);
        let end = clamp_boundary(&self.source, range.end.max(start));
        &self.source[start..end]
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DocumentError {
    #[error("cannot read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

pub fn content_hash(source: &str) -> String {
    hex::encode(Sha256::digest(source.as_bytes()))
}

pub fn line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0usize];
    for (index, byte) in source.bytes().enumerate() {
        if byte == b'\n' {
            starts.push(index + 1);
        }
    }
    starts
}

fn clamp_boundary(source: &str, mut index: usize) -> usize {
    index = index.min(source.len());
    while index > 0 && !source.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn heading_number(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

#[derive(Clone)]
struct Builder {
    kind: BlockKind,
    start: usize,
    end: usize,
    inlines: Vec<Inline>,
    table: Option<Table>,
    depth: usize,
    quote_depth: usize,
    image: Option<(String, String, Option<String>)>,
    has_non_image_text: bool,
    content_start: Option<usize>,
    content_end: Option<usize>,
    in_code: bool,
    code_lang: Option<String>,
}

impl Builder {
    fn new(kind: BlockKind, start: usize, depth: usize, quote_depth: usize) -> Self {
        Self {
            kind,
            start,
            end: start,
            inlines: Vec::new(),
            table: None,
            depth,
            quote_depth,
            image: None,
            has_non_image_text: false,
            content_start: None,
            content_end: None,
            in_code: false,
            code_lang: None,
        }
    }

    fn push_inline(&mut self, text: &str, style: InlineStyle, range: Range<usize>) {
        if text.is_empty() {
            return;
        }
        if !matches!(style, InlineStyle::ImageAlt) {
            self.has_non_image_text = true;
        }
        self.inlines.push(Inline {
            text: text.to_string(),
            style,
            source: range,
        });
    }

    fn has_content(&self) -> bool {
        !self.inlines.is_empty() || self.image.is_some() || self.table.is_some()
    }

    fn finish(mut self, blocks: &mut Vec<Block>, document: &Document, heading_path: Vec<String>) {
        let source = if matches!(
            self.kind,
            BlockKind::CodeBlock { .. } | BlockKind::Mermaid { .. }
        ) && self.content_start.is_some()
            && self.content_end.is_some()
        {
            self.content_start.unwrap_or(self.start)..self.content_end.unwrap_or(self.end)
        } else {
            self.start..self.end.max(self.start)
        };
        let source = fresh_range(&document.source, source);
        let lines = document.lines_of(source.clone());

        if matches!(self.kind, BlockKind::Paragraph)
            && self.image.is_some()
            && !self.has_non_image_text
        {
            let (src, alt, title) = self.image.take().expect("image checked");
            self.kind = BlockKind::Image { src, alt, title };
            self.inlines.clear();
        }

        let block = Block {
            index: blocks.len(),
            kind: self.kind,
            source,
            lines,
            heading_path,
            depth: self.depth,
            quote_depth: self.quote_depth,
            inlines: self.inlines,
            table: self.table,
        };
        blocks.push(block);
    }
}

fn fresh_range(source: &str, range: Range<usize>) -> Range<usize> {
    let start = clamp_boundary(source, range.start);
    let end = clamp_boundary(source, range.end.max(start));
    start..end
}

struct BlockParser<'a> {
    source: &'a str,
    document: Document,
    blocks: Vec<Block>,
    current: Option<Builder>,
    inline_styles: Vec<InlineStyle>,
    list_stack: Vec<Option<u64>>,
    list_counters: Vec<u64>,
    heading_stack: Vec<(u8, String)>,
    quote_depth: usize,
    table: Option<Table>,
    current_row: Vec<TableCell>,
    current_cell_inlines: Vec<Inline>,
    current_cell_range: Option<Range<usize>>,
    in_table: bool,
    in_image: bool,
    image_alt: String,
    image_title: Option<String>,
}

impl<'a> BlockParser<'a> {
    fn new(source: &'a str, line_starts: &[usize]) -> Self {
        let document = Document {
            path: PathBuf::new(),
            source: source.to_string(),
            hash: content_hash(source),
            blocks: Vec::new(),
            line_starts: line_starts.to_vec(),
        };
        Self {
            source,
            document,
            blocks: Vec::new(),
            current: None,
            inline_styles: Vec::new(),
            list_stack: Vec::new(),
            list_counters: Vec::new(),
            heading_stack: Vec::new(),
            quote_depth: 0,
            table: None,
            current_row: Vec::new(),
            current_cell_inlines: Vec::new(),
            current_cell_range: None,
            in_table: false,
            in_image: false,
            image_alt: String::new(),
            image_title: None,
        }
    }

    fn parse(&mut self) -> Vec<Block> {
        let options = Options::ENABLE_TABLES
            | Options::ENABLE_TASKLISTS
            | Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_FOOTNOTES;
        let parser = Parser::new_ext(self.source, options).into_offset_iter();

        for (event, range) in parser {
            self.handle(event, range);
        }
        self.finish_current();
        std::mem::take(&mut self.blocks)
    }

    fn style(&self) -> InlineStyle {
        self.inline_styles
            .last()
            .cloned()
            .unwrap_or(InlineStyle::Plain)
    }

    fn heading_path(&self) -> Vec<String> {
        self.heading_stack
            .iter()
            .map(|(_, text)| text.clone())
            .collect()
    }

    fn start_block(&mut self, kind: BlockKind, start: usize) {
        if self.current.is_some() {
            self.finish_current();
        }
        let depth = self.list_stack.len() + self.quote_depth;
        self.current = Some(Builder::new(kind, start, depth, self.quote_depth));
    }

    fn ensure_current(&mut self, start: usize) {
        if self.current.is_none() {
            let kind = if self.quote_depth > 0 {
                BlockKind::Blockquote
            } else {
                BlockKind::Paragraph
            };
            self.start_block(kind, start);
        }
    }

    fn finish_current(&mut self) {
        if let Some(mut builder) = self.current.take() {
            if matches!(builder.kind, BlockKind::CodeBlock { .. })
                && let Some(lang) = builder.code_lang.clone()
                && lang.eq_ignore_ascii_case("mermaid")
            {
                let text = builder
                    .content_start
                    .zip(builder.content_end)
                    .map(|(start, end)| self.source[start..end].to_string())
                    .unwrap_or_else(|| builder.inlines.iter().map(|i| i.text.as_str()).collect());
                builder.kind = BlockKind::Mermaid { source: text };
            }
            let heading_path = self.heading_path();
            builder.finish(&mut self.blocks, &self.document, heading_path);
        }
    }

    fn append_text(
        &mut self,
        text: &str,
        range: Range<usize>,
        style_override: Option<InlineStyle>,
    ) {
        if self.in_table {
            if let Some(existing) = self.current_cell_range.as_mut() {
                existing.end = existing.end.max(range.end);
            } else {
                self.current_cell_range = Some(range.clone());
            }
            let style = style_override.unwrap_or_else(|| self.style());
            self.current_cell_inlines.push(Inline {
                text: text.to_string(),
                style,
                source: range,
            });
            return;
        }
        if self.in_image {
            self.image_alt.push_str(text);
        }
        self.ensure_current(range.start);
        let in_code = self.current.as_ref().is_some_and(|builder| builder.in_code);
        let base_style = self.style();
        if let Some(builder) = self.current.as_mut() {
            let style = if let Some(style) = style_override.clone() {
                style
            } else if self.in_image {
                InlineStyle::ImageAlt
            } else if in_code {
                InlineStyle::Code
            } else {
                base_style
            };
            if in_code {
                if builder.content_start.is_none() {
                    builder.content_start = Some(range.start);
                }
                builder.content_end = Some(range.end);
            }
            builder.push_inline(text, style, range);
        }
    }

    fn handle(&mut self, event: Event<'_>, range: Range<usize>) {
        match event {
            Event::Start(tag) => self.start_tag(tag, range),
            Event::End(tag) => self.end_tag(tag, range),
            Event::Text(text) => self.append_text(&text, range, None),
            Event::Code(text) => self.append_text(&text, range, Some(InlineStyle::Code)),
            Event::InlineMath(text) | Event::DisplayMath(text) => {
                self.append_text(&format!("${text}$"), range, Some(InlineStyle::Code));
            }
            Event::Html(text) => {
                if self.current.is_none() {
                    self.start_block(BlockKind::Html, range.start);
                }
                let style = self.style();
                if let Some(builder) = self.current.as_mut() {
                    builder.push_inline(&text, style, range.clone());
                    builder.end = range.end;
                }
            }
            Event::InlineHtml(text) => {
                self.append_text(&text, range, Some(InlineStyle::Plain));
            }
            Event::SoftBreak | Event::HardBreak => {
                let text = if self.in_table { " " } else { "\n" };
                self.append_text(text, range, None);
            }
            Event::Rule => {
                self.finish_current();
                let heading_path = self.heading_path();
                let mut builder = Builder::new(BlockKind::Rule, range.start, 0, self.quote_depth);
                builder.end = range.end;
                builder.finish(&mut self.blocks, &self.document, heading_path);
            }
            Event::TaskListMarker(checked) => {
                let marker = if checked { "[x] " } else { "[ ] " };
                self.ensure_current(range.start);
                if let Some(builder) = self.current.as_mut() {
                    builder.push_inline(marker, InlineStyle::TaskMarker, range);
                }
            }
            Event::FootnoteReference(name) => {
                self.append_text(&format!("[^{name}]"), range, None);
            }
        }
    }

    fn start_tag(&mut self, tag: Tag<'_>, range: Range<usize>) {
        match tag {
            Tag::Paragraph => {
                if self.current.as_ref().is_some_and(Builder::has_content) {
                    self.finish_current();
                }
                if self.current.is_none() {
                    let kind = if self.quote_depth > 0 {
                        BlockKind::Blockquote
                    } else {
                        BlockKind::Paragraph
                    };
                    self.start_block(kind, range.start);
                }
            }
            Tag::Heading { level, .. } => {
                let level = heading_number(level);
                self.start_block(BlockKind::Heading(level), range.start);
            }
            Tag::BlockQuote(_) => {
                self.quote_depth += 1;
            }
            Tag::CodeBlock(kind) => {
                let lang = match kind {
                    CodeBlockKind::Fenced(lang) => {
                        let lang = lang.trim();
                        if lang.is_empty() {
                            None
                        } else {
                            Some(lang.to_string())
                        }
                    }
                    CodeBlockKind::Indented => None,
                };
                if self
                    .current
                    .as_ref()
                    .is_some_and(|builder| matches!(builder.kind, BlockKind::ListItem { .. }))
                {
                    self.finish_current();
                }
                if self.current.is_some() {
                    if let Some(builder) = self.current.as_mut() {
                        builder.in_code = true;
                        builder.code_lang = lang;
                    }
                } else {
                    self.start_block(BlockKind::CodeBlock { lang: lang.clone() }, range.start);
                    if let Some(builder) = self.current.as_mut() {
                        builder.in_code = true;
                        builder.code_lang = lang;
                    }
                }
            }
            Tag::List(start) => {
                self.list_stack.push(start);
                self.list_counters.push(start.unwrap_or(1));
            }
            Tag::Item => {
                let ordered = self.list_stack.last().is_some_and(Option::is_some);
                let counter = self.list_counters.last_mut();
                let marker = if let Some(counter) = counter {
                    if ordered {
                        let value = *counter;
                        *counter += 1;
                        format!("{value}. ")
                    } else {
                        "- ".to_string()
                    }
                } else {
                    "- ".to_string()
                };
                self.start_block(BlockKind::ListItem { ordered, marker }, range.start);
            }
            Tag::Table(alignments) => {
                self.finish_current();
                self.in_table = true;
                self.table = Some(Table {
                    alignments: alignments.into_iter().map(ColumnAlign::from).collect(),
                    ..Default::default()
                });
                self.current_row.clear();
                self.current_cell_inlines.clear();
                self.current_cell_range = None;
                self.start_block(BlockKind::Table, range.start);
            }
            Tag::TableHead => {}
            Tag::TableRow => {
                self.current_row.clear();
            }
            Tag::TableCell => {
                self.current_cell_inlines.clear();
                self.current_cell_range = None;
            }
            Tag::FootnoteDefinition(_) => {
                self.start_block(BlockKind::Footnote, range.start);
            }
            Tag::Emphasis => self.inline_styles.push(InlineStyle::Emphasis),
            Tag::Strong => self.inline_styles.push(InlineStyle::Strong),
            Tag::Strikethrough => self.inline_styles.push(InlineStyle::Strikethrough),
            Tag::Link { .. } => self.inline_styles.push(InlineStyle::Link),
            Tag::Image {
                dest_url, title, ..
            } => {
                self.in_image = true;
                self.image_alt.clear();
                self.image_title = if title.is_empty() {
                    None
                } else {
                    Some(title.to_string())
                };
                self.inline_styles.push(InlineStyle::ImageAlt);
                self.ensure_current(range.start);
                if let Some(builder) = self.current.as_mut() {
                    builder.image = Some((
                        dest_url.to_string(),
                        String::new(),
                        self.image_title.clone(),
                    ));
                }
            }
            _ => {}
        }
    }

    fn end_tag(&mut self, tag: TagEnd, range: Range<usize>) {
        match tag {
            TagEnd::Paragraph => {
                if let Some(builder) = self.current.as_mut() {
                    builder.end = range.end;
                }
                self.finish_current();
            }
            TagEnd::Heading(_) => {
                if let Some(mut builder) = self.current.take() {
                    builder.end = range.end;
                    let text = builder
                        .inlines
                        .iter()
                        .map(|i| i.text.as_str())
                        .collect::<String>();
                    let level = match builder.kind {
                        BlockKind::Heading(level) => level,
                        _ => 1,
                    };
                    let parent_path = self.heading_path();
                    builder.finish(&mut self.blocks, &self.document, parent_path);
                    while self
                        .heading_stack
                        .last()
                        .is_some_and(|(last, _)| *last >= level)
                    {
                        self.heading_stack.pop();
                    }
                    self.heading_stack.push((level, text));
                    let path = self.heading_path();
                    if let Some(block) = self.blocks.last_mut() {
                        block.heading_path = path;
                    }
                }
            }
            TagEnd::Item => {
                if let Some(builder) = self.current.as_mut() {
                    builder.end = range.end;
                }
                self.finish_current();
            }
            TagEnd::BlockQuote(_) => {
                self.quote_depth = self.quote_depth.saturating_sub(1);
            }
            TagEnd::CodeBlock => {
                if let Some(builder) = self.current.as_mut()
                    && builder.in_code
                {
                    builder.end = range.end;
                    builder.in_code = false;
                }
                if self
                    .current
                    .as_ref()
                    .is_some_and(|b| matches!(b.kind, BlockKind::CodeBlock { .. }))
                {
                    self.finish_current();
                }
            }
            TagEnd::List(_) => {
                self.list_stack.pop();
                self.list_counters.pop();
            }
            TagEnd::Table => {
                self.in_table = false;
                let table = self.table.take();
                if let Some(builder) = self.current.as_mut() {
                    builder.end = range.end;
                    builder.table = table;
                }
                self.current_row.clear();
                self.current_cell_inlines.clear();
                self.current_cell_range = None;
                self.finish_current();
            }
            TagEnd::TableHead => {
                if !self.current_row.is_empty() {
                    let row = std::mem::take(&mut self.current_row);
                    if let Some(table) = self.table.as_mut() {
                        table.header = row;
                    }
                }
            }
            TagEnd::TableRow => {
                if !self.current_row.is_empty() {
                    let row = std::mem::take(&mut self.current_row);
                    if let Some(table) = self.table.as_mut() {
                        table.rows.push(row);
                    }
                }
            }
            TagEnd::TableCell => {
                let source = self
                    .current_cell_range
                    .take()
                    .unwrap_or_else(|| range.clone());
                let mut inlines = std::mem::take(&mut self.current_cell_inlines);
                if inlines.is_empty() {
                    inlines.push(Inline {
                        text: String::new(),
                        style: InlineStyle::Plain,
                        source: source.clone(),
                    });
                }
                let cell = TableCell { inlines, source };
                self.current_row.push(cell);
            }
            TagEnd::FootnoteDefinition => {
                if let Some(builder) = self.current.as_mut() {
                    builder.end = range.end;
                }
                self.finish_current();
            }
            TagEnd::Link | TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                self.inline_styles.pop();
            }
            TagEnd::Image => {
                self.in_image = false;
                if let Some(builder) = self.current.as_mut()
                    && let Some((_, alt, _)) = builder.image.as_mut()
                {
                    *alt = self.image_alt.clone();
                }
                self.inline_styles.pop();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(source: &str) -> Document {
        Document::from_source("test.md", source.to_string())
    }

    #[test]
    fn parses_headings_paragraphs_and_paths() {
        let d = doc("# Plan\n\nIntro text.\n\n## Architecture\n\nData flow.\n");
        assert!(matches!(d.blocks[0].kind, BlockKind::Heading(1)));
        assert_eq!(d.blocks[0].heading_path, vec!["Plan"]);
        assert!(matches!(d.blocks[1].kind, BlockKind::Paragraph));
        assert_eq!(d.blocks[1].heading_path, vec!["Plan"]);
        assert_eq!(d.blocks[2].heading_path, vec!["Plan", "Architecture"]);
        assert_eq!(d.blocks[3].heading_path, vec!["Plan", "Architecture"]);
    }

    #[test]
    fn detects_mermaid_and_image_blocks() {
        let d = doc("```mermaid\nflowchart LR\n  A-->B\n```\n\n![diagram](a.png)\n");
        assert!(matches!(d.blocks[0].kind, BlockKind::Mermaid { .. }));
        match &d.blocks[0].kind {
            BlockKind::Mermaid { source } => assert!(source.contains("flowchart LR")),
            _ => unreachable!(),
        }
        match &d.blocks[1].kind {
            BlockKind::Image { src, alt, .. } => {
                assert_eq!(src, "a.png");
                assert_eq!(alt, "diagram");
            }
            other => panic!("expected image, got {other:?}"),
        }
    }

    #[test]
    fn parses_lists_tables_quotes_and_code() {
        let d = doc(
            "> quoted\n\n- one\n- two\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n```rust\nfn main() {}\n```\n",
        );
        assert!(
            d.blocks
                .iter()
                .any(|b| matches!(b.kind, BlockKind::Blockquote))
        );
        assert_eq!(
            d.blocks
                .iter()
                .filter(|b| matches!(b.kind, BlockKind::ListItem { .. }))
                .count(),
            2
        );
        let table = d
            .blocks
            .iter()
            .find(|b| matches!(b.kind, BlockKind::Table))
            .expect("table");
        assert!(table.text().contains("a | b"));
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
        let code = d
            .blocks
            .iter()
            .find(|b| matches!(b.kind, BlockKind::CodeBlock { .. }))
            .expect("code");
        assert!(
            matches!(code.kind, BlockKind::CodeBlock { ref lang } if lang.as_deref() == Some("rust"))
        );
        assert!(code.text().contains("fn main"));
    }

    #[test]
    fn table_retains_styles_alignments_and_ranges() {
        let d = doc("| Name | Value |\n|:---|---:|\n| **bold** | `code` |\n");
        let table = d.blocks[0].table.as_ref().expect("structured table");
        assert_eq!(
            table.alignments,
            vec![ColumnAlign::Left, ColumnAlign::Right]
        );
        assert_eq!(table.header[0].inlines[0].text, "Name");
        let row = &table.rows[0];
        assert_eq!(row[0].inlines[0].style, InlineStyle::Strong);
        assert_eq!(row[0].inlines[0].text, "bold");
        assert_eq!(row[1].inlines[0].style, InlineStyle::Code);
        assert_eq!(row[1].inlines[0].text, "code");
        assert!(row[1].source.start < row[1].source.end);
        assert!(d.source[row[1].source.clone()].contains("code"));
    }

    #[test]
    fn empty_table_cells_have_inline_source_ranges() {
        let d = doc("| a |  | c |\n|---|---|---|\n|  | b |  |\n");
        let table = d.blocks[0].table.as_ref().expect("structured table");
        assert_eq!(table.header.len(), 3);
        assert_eq!(table.rows[0].len(), 3);
        assert!(table.header[1].inlines[0].text.is_empty());
        assert_eq!(table.header[1].inlines[0].source, table.header[1].source);
        assert!(table.rows[0][0].inlines[0].text.is_empty());
        assert_eq!(table.rows[0][0].inlines[0].source, table.rows[0][0].source);
        assert!(table.rows[0][2].inlines[0].text.is_empty());
        assert_eq!(table.rows[0][2].inlines[0].source, table.rows[0][2].source);
    }

    #[test]
    fn multi_paragraph_lists_and_footnotes_are_separate_blocks() {
        let d = doc("- first\n\n  second\n\n- next\n\n[^1]: foot one\n\n    foot two\n");
        let items: Vec<_> = d
            .blocks
            .iter()
            .filter(|block| matches!(block.kind, BlockKind::ListItem { .. }))
            .collect();
        assert_eq!(items.len(), 2, "{:?}", d.blocks);
        assert!(items.iter().any(|block| block.text().contains("first")));
        assert!(
            !items.iter().any(|block| block.text().contains("second")),
            "{:?}",
            d.blocks
        );
        let second = d
            .blocks
            .iter()
            .find(|block| block.text().contains("second"))
            .expect("second paragraph");
        assert!(matches!(second.kind, BlockKind::Paragraph));
        assert!(second.depth >= 1, "{second:?}");

        let footnote = d
            .blocks
            .iter()
            .find(|block| matches!(block.kind, BlockKind::Footnote))
            .expect("footnote");
        assert!(footnote.text().contains("foot one"));
        assert!(!footnote.text().contains("foot two"));
        let continuation = d
            .blocks
            .iter()
            .find(|block| block.text().contains("foot two"))
            .expect("footnote continuation");
        assert!(matches!(continuation.kind, BlockKind::Paragraph));
    }

    #[test]
    fn source_ranges_and_lines_round_trip() {
        let source = "# Title\n\nFirst paragraph line\ncontinues here.\n";
        let d = doc(source);
        for block in &d.blocks {
            if block.source.start < block.source.end {
                let slice = &source[block.source.clone()];
                assert!(!slice.is_empty(), "block {} empty", block.index);
            }
            assert!(block.lines.start >= 1);
            assert!(block.lines.end > block.lines.start);
        }
        assert_eq!(d.source_line(1), Some("# Title"));
        assert_eq!(d.source_line(3), Some("First paragraph line"));
        assert_eq!(d.line_of(0), 1);
        assert_eq!(d.lines_of(0..7), 1..2);
    }

    #[test]
    fn handles_inline_styles_and_unicode() {
        let d = doc("A **bold** and *italic* with `code` and [link](x) — 你好 👋\n");
        let block = &d.blocks[0];
        let styles: Vec<_> = block.inlines.iter().map(|i| i.style.clone()).collect();
        assert!(styles.contains(&InlineStyle::Strong));
        assert!(styles.contains(&InlineStyle::Emphasis));
        assert!(styles.contains(&InlineStyle::Code));
        assert!(styles.contains(&InlineStyle::Link));
        assert!(block.text().contains("你好"));
    }

    #[test]
    fn html_block_is_constructed() {
        let d = doc("<div>hello</div>\n\n# Next\n");
        assert!(matches!(d.blocks[0].kind, BlockKind::Html));
        assert!(d.blocks[0].text().contains("hello"));
    }

    #[test]
    fn nested_mermaid_becomes_a_mermaid_block() {
        let d = doc("- item\n\n  ```mermaid\n  flowchart LR\n  A-->B\n  ```\n");
        assert!(
            d.blocks
                .iter()
                .any(|block| matches!(block.kind, BlockKind::Mermaid { .. })),
            "{:?}",
            d.blocks
        );
    }

    #[test]
    fn empty_crlf_and_no_trailing_newline_inputs() {
        let empty = doc("");
        assert!(empty.blocks.is_empty());

        let crlf = doc("# Plan\r\n\r\nHello world.\r\n");
        assert!(matches!(crlf.blocks[0].kind, BlockKind::Heading(1)));
        assert!(crlf.blocks[1].text().contains("Hello world."));
        assert!(!crlf.blocks[1].text().contains('\r'));

        let no_newline = doc("# Plan\n\nLast line");
        assert_eq!(no_newline.blocks.last().unwrap().text(), "Last line");
        assert_eq!(no_newline.source_line(3), Some("Last line"));
    }

    #[test]
    fn parses_task_rules_footnotes_and_nested_lists() {
        let source = "# Fixture\n\n- [x] done\n  - [ ] nested\n\n---\n\nA note.[^1]\n\n[^1]: Footnote text.\n";
        let d = doc(source);
        assert!(d.blocks.iter().any(|block| {
            matches!(block.kind, BlockKind::ListItem { .. })
                && block.text().contains("[x]")
                && block
                    .inlines
                    .iter()
                    .any(|inline| inline.style == InlineStyle::TaskMarker)
        }));
        assert!(
            d.blocks
                .iter()
                .any(|block| matches!(block.kind, BlockKind::Rule))
        );
        assert!(
            d.blocks
                .iter()
                .any(|block| matches!(block.kind, BlockKind::Footnote))
        );
        let nested = d
            .blocks
            .iter()
            .find(|block| block.text().contains("nested"))
            .expect("nested item");
        assert!(nested.depth >= 1, "{nested:?}");
        for block in &d.blocks {
            assert!(
                block.source.end <= d.source.len(),
                "block range past EOF: {block:?}"
            );
        }
    }

    #[test]
    fn block_at_uses_the_preceding_block_in_gaps_and_at_eof() {
        let source = "# A\n\none\n\n## B\n\n two\n";
        let d = doc(source);
        let gap = source.find("\n\n## B").unwrap() + 1;
        assert_eq!(d.block_at(gap).unwrap().index, 1);
        assert_eq!(d.block_at(source.len()).unwrap().index, d.blocks.len() - 1);
    }
}
