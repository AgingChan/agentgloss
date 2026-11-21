//! TUI application state, input handling, review editing, and file watching.

use crate::config::{Keys, MermaidEngine, Protocol, Theme};
use crate::document::{
    BlockKind, Document, InlineStyle, StyledSpan, VisualKind, VisualLine, layout,
};
use crate::media::MediaManager;
use crate::review::{Anchor, Draft, ItemKind, ItemStatus, ReviewItem, ReviewStore, now};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use edtui::{EditorEventHandler, EditorMode, EditorState as EdtuiState, Lines};
use ratatui::layout::Rect;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct TuiOptions {
    pub keys: Keys,
    pub mouse: bool,
    pub protocol: Protocol,
    pub theme: Theme,
    pub mermaid_engine: MermaidEngine,
    pub cache_dir: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuiExit {
    Aborted,
    Feedback,
    Approved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Vim,
    Standard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Rendered,
    Source,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Document,
    Outline,
    Review,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FooterAction {
    Comment,
    Suggest,
    Resolve,
    Send,
}

#[derive(Debug, Clone)]
pub struct OutlineItem {
    pub block: usize,
    pub level: u8,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub start: usize,
    pub end: usize,
    pub block: bool,
}

#[derive(Debug, Clone)]
pub struct SearchState {
    pub query: String,
    pub matches: Vec<usize>,
    pub index: usize,
}

#[derive(Debug, Clone)]
pub struct LayoutInfo {
    pub doc: Rect,
    pub outline: Option<Rect>,
    pub review: Option<Rect>,
    pub footer: Rect,
    pub footer_buttons: Vec<(Rect, FooterAction)>,
    pub doc_height: usize,
    pub outline_scroll: usize,
    pub review_scroll: usize,
}

struct EditorTarget {
    id: Option<String>,
    kind: ItemKind,
    anchor: Anchor,
    original: Option<String>,
}

pub struct Editor {
    target: EditorTarget,
    title: String,
    pub state: EdtuiState,
    handler: EditorEventHandler,
}

#[derive(Debug, Clone)]
enum ConfirmAction {
    Signoff,
    Delete(String),
}

#[derive(Debug, Clone)]
pub(crate) struct Confirm {
    pub(crate) message: String,
    action: ConfirmAction,
}

pub struct App {
    pub document: Document,
    pub store: ReviewStore,
    pub sidecar: PathBuf,
    pub options: TuiOptions,
    pub media: MediaManager,
    pub media_keys: HashMap<usize, String>,
    pub mermaid_source: HashSet<usize>,
    pub view: ViewMode,
    pub input: InputMode,
    pub focus: Focus,
    pub lines: Vec<VisualLine>,
    pub cursor: usize,
    pub scroll: usize,
    pub viewport_height: usize,
    pub selection: Option<Selection>,
    pub outline: Vec<OutlineItem>,
    pub outline_cursor: usize,
    pub outline_scroll: usize,
    pub review_cursor: usize,
    pub review_scroll: usize,
    pub markers: Vec<bool>,
    pub search: Option<SearchState>,
    pub last_search: Option<SearchState>,
    pub(crate) editor: Option<Editor>,
    pub help: bool,
    pub status: String,
    pub should_quit: bool,
    pub exit: Option<TuiExit>,
    pub layout: Option<LayoutInfo>,
    pub mouse_enabled: bool,
    pending_key: Option<char>,
    drag_start: Option<usize>,
    pub(crate) confirm: Option<Confirm>,
    layout_width: usize,
    last_meta: Option<(SystemTime, u64)>,
    pending_reload: bool,
}

impl App {
    pub fn new(
        document: Document,
        store: ReviewStore,
        sidecar: PathBuf,
        options: TuiOptions,
    ) -> Self {
        let input = match options.keys {
            Keys::Vim => InputMode::Vim,
            Keys::Standard => InputMode::Standard,
        };
        let mouse_enabled = options.mouse;
        let last_meta = file_meta(&document.path);
        let media = MediaManager::new(&options);
        let lines = layout(&document, 80);
        let outline = build_outline(&document);
        let mut app = Self {
            document,
            store,
            sidecar,
            options,
            media,
            media_keys: HashMap::new(),
            mermaid_source: HashSet::new(),
            view: ViewMode::Rendered,
            input,
            focus: Focus::Document,
            lines,
            cursor: 0,
            scroll: 0,
            viewport_height: 10,
            selection: None,
            outline,
            outline_cursor: 0,
            outline_scroll: 0,
            review_cursor: 0,
            review_scroll: 0,
            markers: Vec::new(),
            search: None,
            last_search: None,
            editor: None,
            help: false,
            status: "Ready. ? for help".to_string(),
            should_quit: false,
            exit: None,
            layout: None,
            mouse_enabled,
            pending_key: None,
            drag_start: None,
            confirm: None,
            layout_width: 80,
            last_meta,
            pending_reload: false,
        };
        app.refresh_markers();
        if let Some(draft) = app.store.draft.clone() {
            let title = match draft.kind {
                ItemKind::Comment if draft.editing_id.is_some() => "Resume comment edit",
                ItemKind::Comment => "Resume comment",
                ItemKind::Suggestion if draft.editing_id.is_some() => "Resume suggestion edit",
                ItemKind::Suggestion => "Resume suggestion",
            };
            let target = EditorTarget {
                id: draft.editing_id,
                kind: draft.kind,
                anchor: draft.anchor,
                original: draft.original,
            };
            app.editor = Some(Editor::new(
                target,
                title.to_string(),
                draft.body,
                app.input,
            ));
            app.status = "Resumed an unfinished draft".to_string();
        }
        app
    }

    pub fn is_editing(&self) -> bool {
        self.editor.is_some()
    }

    pub fn editor_text(&self) -> Option<String> {
        self.editor
            .as_ref()
            .map(|editor| editor.state.lines.to_string())
    }

    pub fn set_editor_text(&mut self, text: &str) {
        if let Some(editor) = self.editor.as_mut() {
            editor.state.lines = Lines::from(text);
        }
        self.persist_draft();
    }

    fn persist_draft(&mut self) {
        let Some(editor) = self.editor.as_ref() else {
            return;
        };
        let draft = Draft {
            kind: editor.target.kind,
            body: editor
                .state
                .lines
                .to_string()
                .trim_end_matches('\n')
                .to_string(),
            original: editor.target.original.clone(),
            anchor: editor.target.anchor.clone(),
            editing_id: editor.target.id.clone(),
        };
        self.store.set_draft(draft);
        if let Err(error) = self.store.save(&self.sidecar) {
            self.status = format!("Draft save failed: {error}");
        }
    }

    pub fn layout_width(&self) -> usize {
        self.layout_width
    }

    pub fn rebuild(&mut self, width: usize) {
        self.layout_width = width.max(1);
        self.lines = match self.view {
            ViewMode::Rendered => layout(&self.document, self.layout_width),
            ViewMode::Source => source_lines(&self.document),
        };
        if self.lines.is_empty() {
            self.lines.push(blank_visual(&self.document));
        }
        self.cursor = self.cursor.min(self.lines.len() - 1);
        self.scroll = self.scroll.min(self.lines.len().saturating_sub(1));
        self.media.clear_encodings();
        self.media_keys.clear();
        self.refresh_markers();
        self.ensure_cursor_visible(self.viewport_height.max(1));
    }

    pub fn toggle_view(&mut self) {
        let old_source = self
            .lines
            .get(self.cursor)
            .map(|line| line.source.start)
            .unwrap_or(0);
        self.view = match self.view {
            ViewMode::Rendered => ViewMode::Source,
            ViewMode::Source => ViewMode::Rendered,
        };
        self.rebuild(self.layout_width);
        let target = match self.view {
            ViewMode::Source => self.document.line_of(old_source).saturating_sub(1),
            ViewMode::Rendered => self
                .lines
                .iter()
                .position(|line| line.source.start <= old_source && old_source < line.source.end)
                .unwrap_or(0),
        };
        self.cursor = target.min(self.lines.len() - 1);
        self.selection = None;
        self.status = match self.view {
            ViewMode::Rendered => "Rendered view".to_string(),
            ViewMode::Source => "Source view".to_string(),
        };
        self.ensure_cursor_visible(self.viewport_height.max(1));
    }

    pub fn toggle_input_mode(&mut self) {
        self.input = match self.input {
            InputMode::Vim => InputMode::Standard,
            InputMode::Standard => InputMode::Vim,
        };
        self.status = match self.input {
            InputMode::Vim => "Vim mode".to_string(),
            InputMode::Standard => "Standard mode".to_string(),
        };
    }

    pub fn cycle_focus(&mut self, reverse: bool) {
        self.focus = if reverse {
            match self.focus {
                Focus::Document => Focus::Review,
                Focus::Outline => Focus::Document,
                Focus::Review => Focus::Outline,
            }
        } else {
            match self.focus {
                Focus::Document => Focus::Outline,
                Focus::Outline => Focus::Review,
                Focus::Review => Focus::Document,
            }
        };
        self.status = format!("Focus: {:?}", self.focus);
    }

    pub fn set_focus(&mut self, focus: Focus) {
        self.focus = focus;
    }

    pub fn set_viewport(&mut self, height: usize) {
        self.viewport_height = height.max(1);
        self.ensure_cursor_visible(self.viewport_height);
    }

    /// Ensure media for visible blocks is decoded and encoded before drawing.
    pub fn prepare_media(&mut self, area: Rect) {
        self.media.poll();
        if area.height == 0 {
            return;
        }
        let start = self.scroll;
        let end = (start + area.height as usize).min(self.lines.len());
        let mut index = start;
        while index < end {
            if !self.lines[index].is_media() {
                index += 1;
                continue;
            }
            let block = self.lines[index].block;
            let group_start = index;
            let mut group_end = index;
            while group_end < end
                && self.lines[group_end].is_media()
                && self.lines[group_end].block == block
            {
                group_end += 1;
            }
            let target = (area.width, (group_end - group_start).max(1) as u16);
            let key = match self.document.blocks.get(block).map(|b| b.kind.clone()) {
                Some(BlockKind::Image { src, .. }) => {
                    Some(self.media.ensure_image(&self.document.path, &src, target))
                }
                Some(BlockKind::Mermaid { source }) => {
                    if self.mermaid_source.contains(&block) {
                        None
                    } else {
                        Some(self.media.ensure_mermaid(&source, target))
                    }
                }
                _ => None,
            };
            if let Some(key) = key {
                if let Some(error) = self.media.error(&key)
                    && (self.status.starts_with("Ready") || self.status.starts_with("Media:"))
                {
                    self.status = format!("Media: {error}");
                }
                self.media_keys.insert(block, key);
            }
            index = group_end;
        }
    }

    pub fn media_key(&self, block: usize) -> Option<&str> {
        self.media_keys.get(&block).map(String::as_str)
    }

    pub fn has_visible_media(&self, area: Rect) -> bool {
        let start = self.scroll;
        let end = (start + area.height as usize).min(self.lines.len());
        self.lines[start..end].iter().any(|line| {
            line.is_media()
                && self
                    .media_key(line.block)
                    .is_some_and(|key| self.media.get(key).is_some())
        })
    }

    pub fn current_line(&self) -> Option<&VisualLine> {
        self.lines.get(self.cursor)
    }

    pub fn current_block(&self) -> Option<&crate::document::Block> {
        self.current_line()
            .and_then(|line| self.document.blocks.get(line.block))
    }

    pub fn selected_range(&self) -> Option<std::ops::Range<usize>> {
        let (start, end, block_select) = if let Some(selection) = self.selection {
            (
                selection.start.min(selection.end),
                selection.start.max(selection.end),
                selection.block,
            )
        } else {
            (self.cursor, self.cursor, false)
        };
        let first = self.lines.get(start)?;
        let last = self.lines.get(end)?;
        if block_select {
            return self
                .document
                .blocks
                .get(first.block)
                .map(|block| block.source.clone());
        }
        let start_source = first.source.start;
        let end_source = last.source.end;
        if start_source == end_source
            || (first.source.start == first.source.end && last.source.start == last.source.end)
        {
            return self
                .document
                .blocks
                .get(first.block)
                .map(|block| block.source.clone());
        }
        let start_line = self.document.line_of(start_source);
        let end_line = self
            .document
            .line_of(end_source.saturating_sub(1).max(start_source));
        let line_start = self.document.line_starts[start_line - 1];
        let line_end = self
            .document
            .line_starts
            .get(end_line)
            .copied()
            .unwrap_or(self.document.source.len());
        Some(line_start..line_end.max(line_start))
    }

    pub fn move_cursor(&mut self, delta: isize) {
        if self.lines.is_empty() {
            return;
        }
        let new = (self.cursor as isize + delta).clamp(0, (self.lines.len() - 1) as isize) as usize;
        self.cursor = new;
        if let Some(selection) = self.selection.as_mut() {
            selection.end = new;
        }
        self.ensure_cursor_visible(self.viewport_height.max(1));
    }

    pub fn set_cursor(&mut self, index: usize) {
        if self.lines.is_empty() {
            return;
        }
        self.cursor = index.min(self.lines.len() - 1);
        if let Some(selection) = self.selection.as_mut() {
            selection.end = self.cursor;
        }
        self.ensure_cursor_visible(self.viewport_height.max(1));
    }

    pub fn scroll(&mut self, delta: isize) {
        let max = self.lines.len().saturating_sub(1) as isize;
        self.scroll = (self.scroll as isize + delta).clamp(0, max) as usize;
    }

    pub fn start_selection(&mut self) {
        self.selection = Some(Selection {
            start: self.cursor,
            end: self.cursor,
            block: false,
        });
        self.status = "Visual selection — c comment, s suggest, Esc cancel".to_string();
    }

    pub fn select_line(&mut self) {
        self.selection = Some(Selection {
            start: self.cursor,
            end: self.cursor,
            block: false,
        });
        self.status = "Line selected".to_string();
    }

    pub fn select_block(&mut self) {
        let Some(line) = self.lines.get(self.cursor) else {
            return;
        };
        let block = line.block;
        let start = self.lines.iter().position(|line| line.block == block);
        let end = self.lines.iter().rposition(|line| line.block == block);
        if let (Some(start), Some(end)) = (start, end) {
            self.selection = Some(Selection {
                start,
                end,
                block: true,
            });
            self.cursor = start;
            self.status = "Block selected".to_string();
            self.ensure_cursor_visible(self.viewport_height.max(1));
        }
    }

    pub fn next_heading(&mut self, forward: bool) {
        if self.outline.is_empty() {
            return;
        }
        let current_block = self.current_line().map(|line| line.block).unwrap_or(0);
        let current = self
            .outline
            .iter()
            .rposition(|item| item.block <= current_block)
            .unwrap_or(0);
        let target = if forward {
            (current + 1).min(self.outline.len() - 1)
        } else {
            current.saturating_sub(1)
        };
        self.jump_to_block(self.outline[target].block);
        self.outline_cursor = target;
    }

    pub fn move_block(&mut self, delta: isize) {
        let Some(line) = self.lines.get(self.cursor) else {
            return;
        };
        let mut target = line.block as isize + delta;
        while target >= 0 && target < self.document.blocks.len() as isize {
            if let Some(index) = self
                .lines
                .iter()
                .position(|line| line.block == target as usize)
            {
                self.set_cursor(index);
                return;
            }
            target += delta;
        }
    }

    pub fn jump_to_block(&mut self, block: usize) {
        if let Some(line) = self.lines.iter().position(|line| line.block == block) {
            self.set_cursor(line);
            self.ensure_cursor_visible(self.viewport_height.max(1));
        }
    }

    pub fn jump_to_source(&mut self, source_start: usize) {
        let target = match self.view {
            ViewMode::Source => self.document.line_of(source_start).saturating_sub(1),
            ViewMode::Rendered => self
                .lines
                .iter()
                .position(|line| {
                    line.source.start <= source_start && source_start < line.source.end
                })
                .or_else(|| {
                    self.lines
                        .iter()
                        .position(|line| line.source.start >= source_start)
                })
                .unwrap_or(0),
        };
        self.set_cursor(target);
        self.ensure_cursor_visible(self.viewport_height.max(1));
    }

    fn ensure_cursor_visible(&mut self, height: usize) {
        if self.cursor < self.scroll {
            self.scroll = self.cursor;
        } else if self.cursor >= self.scroll + height {
            self.scroll = self.cursor + 1 - height;
        }
    }

    pub fn refresh_markers(&mut self) {
        let items: Vec<&ReviewItem> = self
            .store
            .items
            .iter()
            .filter(|item| item.status != ItemStatus::Resolved)
            .collect();
        self.markers = self
            .lines
            .iter()
            .map(|line| {
                items.iter().any(|item| {
                    ranges_overlap(&item.anchor.range(), &line.source)
                        || lines_overlap(&item.anchor.lines(), &line.source_lines)
                })
            })
            .collect();
    }

    pub fn review_items(&self) -> Vec<&ReviewItem> {
        let mut items: Vec<&ReviewItem> = self.store.items.iter().collect();
        items.sort_by_key(|item| {
            (
                item.anchor.source_start,
                crate::review::item_number(&item.id),
            )
        });
        items
    }

    pub fn selected_review_item(&self) -> Option<&ReviewItem> {
        self.review_items().get(self.review_cursor).copied()
    }

    pub fn move_review_cursor(&mut self, delta: isize) {
        let len = self.store.items.len();
        if len == 0 {
            return;
        }
        self.review_cursor =
            (self.review_cursor as isize + delta).clamp(0, (len - 1) as isize) as usize;
    }

    pub fn move_outline_cursor(&mut self, delta: isize) {
        if self.outline.is_empty() {
            return;
        }
        self.outline_cursor = (self.outline_cursor as isize + delta)
            .clamp(0, (self.outline.len() - 1) as isize) as usize;
    }

    pub fn open_comment_editor(&mut self) {
        let Some(range) = self.selected_range() else {
            self.status = "Nothing selected".to_string();
            return;
        };
        if self
            .current_block()
            .is_some_and(|block| matches!(block.kind, BlockKind::Image { .. }))
        {
            self.status = "Images accept comments; select text for suggestions".to_string();
        }
        match Anchor::from_range(&self.document, range) {
            Ok(anchor) => {
                let editor = Editor::new(
                    EditorTarget {
                        id: None,
                        kind: ItemKind::Comment,
                        anchor,
                        original: None,
                    },
                    "New comment".to_string(),
                    String::new(),
                    self.input,
                );
                self.editor = Some(editor);
                self.persist_draft();
                self.status = "Comment editor — Ctrl+Enter saves, Esc cancels".to_string();
            }
            Err(error) => self.status = format!("Cannot anchor comment: {error}"),
        }
    }

    pub fn open_suggestion_editor(&mut self) {
        let Some(range) = self.selected_range() else {
            self.status = "Nothing selected".to_string();
            return;
        };
        if self
            .current_block()
            .is_some_and(|block| matches!(block.kind, BlockKind::Image { .. }))
        {
            self.status = "Images accept comments, not suggestions".to_string();
            return;
        }
        let original = self.document.text_in_range(range.clone()).to_string();
        if original.trim().is_empty() {
            self.status = "Selected range is empty".to_string();
            return;
        }
        match Anchor::from_range(&self.document, range) {
            Ok(anchor) => {
                let editor = Editor::new(
                    EditorTarget {
                        id: None,
                        kind: ItemKind::Suggestion,
                        anchor,
                        original: Some(original.clone()),
                    },
                    "New suggestion".to_string(),
                    original,
                    self.input,
                );
                self.editor = Some(editor);
                self.persist_draft();
                self.status = "Suggestion editor — Ctrl+Enter saves, Esc cancels".to_string();
            }
            Err(error) => self.status = format!("Cannot anchor suggestion: {error}"),
        }
    }

    pub fn edit_selected_item(&mut self) {
        let Some(item) = self.selected_review_item() else {
            return;
        };
        let (kind, body, original) = match item.kind {
            ItemKind::Comment => (ItemKind::Comment, item.body.clone(), None),
            ItemKind::Suggestion => (
                ItemKind::Suggestion,
                item.suggestion
                    .as_ref()
                    .map(|s| s.replacement.clone())
                    .unwrap_or_default(),
                item.suggestion.as_ref().map(|s| s.original.clone()),
            ),
        };
        let editor = Editor::new(
            EditorTarget {
                id: Some(item.id.clone()),
                kind,
                anchor: item.anchor.clone(),
                original: original.clone(),
            },
            format!(
                "Edit {}",
                if kind == ItemKind::Comment {
                    "comment"
                } else {
                    "suggestion"
                }
            ),
            body,
            self.input,
        );
        self.editor = Some(editor);
        self.persist_draft();
        self.status = "Editing — Ctrl+Enter saves, Esc cancels".to_string();
    }

    pub fn resolve_selected(&mut self) {
        let Some(id) = self.selected_review_item().map(|item| item.id.clone()) else {
            return;
        };
        if let Err(error) = self.store.resolve(&id) {
            self.status = format!("Resolve failed: {error}");
            return;
        }
        self.save_store();
        self.refresh_markers();
        self.status = format!("Resolved {id}");
    }

    pub fn unresolve_selected(&mut self) {
        let Some(id) = self.selected_review_item().map(|item| item.id.clone()) else {
            return;
        };
        if let Err(error) = self.store.unresolve(&id) {
            self.status = format!("Unresolve failed: {error}");
            return;
        }
        self.save_store();
        self.refresh_markers();
        self.status = format!("Reopened {id}");
    }

    pub fn confirm_delete_selected(&mut self) {
        let Some(id) = self.selected_review_item().map(|item| item.id.clone()) else {
            return;
        };
        self.confirm = Some(Confirm {
            message: format!("Delete {id}? Enter confirms, Esc cancels."),
            action: ConfirmAction::Delete(id),
        });
    }

    pub fn reattach_selected(&mut self) {
        let Some(item) = self.selected_review_item() else {
            return;
        };
        if item.status != ItemStatus::Stale {
            self.status = "Only stale items can be reattached".to_string();
            return;
        }
        let id = item.id.clone();
        let Some(range) = self.selected_range() else {
            self.status = "Select the new location in the document first".to_string();
            return;
        };
        match Anchor::from_range(&self.document, range) {
            Ok(anchor) => match self.store.reattach(&id, anchor) {
                Ok(()) => {
                    self.save_store();
                    self.refresh_markers();
                    self.status = format!("Reattached {id}");
                }
                Err(error) => self.status = format!("Reattach failed: {error}"),
            },
            Err(error) => self.status = format!("Cannot anchor reattach: {error}"),
        }
    }

    fn save_store(&mut self) {
        match self.store.save(&self.sidecar) {
            Ok(()) => self.status = "Saved".to_string(),
            Err(error) => self.status = format!("Save failed: {error}"),
        }
    }

    pub fn open_signoff(&mut self) {
        let open = self.store.open_count();
        let stale = self.store.stale_count();
        let message = if open + stale == 0 {
            "No open items. Approve this revision?".to_string()
        } else {
            format!("Send {open} open and {stale} stale item(s) to the agent?")
        };
        self.confirm = Some(Confirm {
            message,
            action: ConfirmAction::Signoff,
        });
    }

    fn apply_confirm(&mut self) {
        let Some(confirm) = self.confirm.take() else {
            return;
        };
        match confirm.action {
            ConfirmAction::Signoff => {
                if self.store.has_feedback() {
                    self.store.signoff_feedback();
                    self.exit = Some(TuiExit::Feedback);
                } else {
                    self.store.approve();
                    self.exit = Some(TuiExit::Approved);
                }
                self.save_store();
                self.should_quit = true;
            }
            ConfirmAction::Delete(id) => {
                if let Err(error) = self.store.remove(&id) {
                    self.status = format!("Delete failed: {error}");
                } else {
                    self.save_store();
                    self.refresh_markers();
                    self.status = format!("Deleted {id}");
                }
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.kind == crossterm::event::KeyEventKind::Release {
            return;
        }
        if self.editor.is_some() {
            self.handle_editor_key(key);
            return;
        }
        if self.confirm.is_some() {
            match key.code {
                KeyCode::Enter => self.apply_confirm(),
                KeyCode::Esc => {
                    self.confirm = None;
                    self.status = "Cancelled".to_string();
                }
                _ => {}
            }
            return;
        }
        if self.help {
            if matches!(
                key.code,
                KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q')
            ) {
                self.help = false;
            }
            return;
        }
        if self.search.is_some() {
            self.handle_search_key(key);
            return;
        }

        if key.code == KeyCode::F(1) || (key.code == KeyCode::Char('?') && self.no_control(&key)) {
            self.help = true;
            return;
        }
        if key.code == KeyCode::F(2) {
            self.toggle_input_mode();
            return;
        }
        if key.code == KeyCode::F(3) {
            self.toggle_view();
            return;
        }
        if key.code == KeyCode::Tab {
            self.cycle_focus(key.modifiers.contains(KeyModifiers::SHIFT));
            return;
        }
        if key.code == KeyCode::BackTab {
            self.cycle_focus(true);
            return;
        }
        if key.code == KeyCode::Enter && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.open_signoff();
            return;
        }
        if key.code == KeyCode::Esc {
            if self.selection.take().is_some() {
                self.status = "Selection cleared".to_string();
            } else {
                self.focus = Focus::Document;
            }
            return;
        }
        if key.code == KeyCode::Char('q') && self.no_control(&key) {
            self.should_quit = true;
            self.exit = Some(TuiExit::Aborted);
            return;
        }

        match self.focus {
            Focus::Document => self.handle_document_key(key),
            Focus::Outline => self.handle_outline_key(key),
            Focus::Review => self.handle_review_key(key),
        }
    }

    fn no_control(&self, key: &KeyEvent) -> bool {
        !key.modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
    }

    fn handle_document_key(&mut self, key: KeyEvent) {
        self.handle_pending_key(&key);
        if self.pending_key.is_some() {
            return;
        }
        match self.input {
            InputMode::Vim => match key.code {
                KeyCode::Char('j') => self.move_cursor(1),
                KeyCode::Char('k') => self.move_cursor(-1),
                KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    let page = (self.viewport_height.max(1) / 2).max(1) as isize;
                    self.scroll(page);
                    self.cursor = (self.cursor as isize + page)
                        .clamp(0, (self.lines.len().saturating_sub(1)) as isize)
                        as usize;
                    self.ensure_cursor_visible(self.viewport_height.max(1));
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    let page = (self.viewport_height.max(1) / 2).max(1) as isize;
                    self.scroll(-page);
                    self.cursor = (self.cursor as isize - page)
                        .clamp(0, (self.lines.len().saturating_sub(1)) as isize)
                        as usize;
                    self.ensure_cursor_visible(self.viewport_height.max(1));
                }
                KeyCode::Char('G') => {
                    self.cursor = self.lines.len().saturating_sub(1);
                    self.ensure_cursor_visible(self.viewport_height.max(1));
                }
                KeyCode::Char('v') => self.start_selection(),
                KeyCode::Char('V') => self.select_line(),
                KeyCode::Char('P') => self.select_block(),
                KeyCode::Char('}') => self.move_block(1),
                KeyCode::Char('{') => self.move_block(-1),
                KeyCode::Char('c') => self.open_comment_editor(),
                KeyCode::Char('s') => self.open_suggestion_editor(),
                KeyCode::Char('/') => self.open_search(),
                KeyCode::Char('n') => self.next_match(true),
                KeyCode::Char('N') => self.next_match(false),
                KeyCode::Enter => self.enter_action(),
                _ => {}
            },
            InputMode::Standard => match key.code {
                KeyCode::Down if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.move_block(1);
                }
                KeyCode::Up if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.move_block(-1);
                }
                KeyCode::Home if key.modifiers.contains(KeyModifiers::SHIFT) => {
                    self.select_line();
                }
                KeyCode::End if key.modifiers.contains(KeyModifiers::SHIFT) => {
                    self.select_line();
                }
                KeyCode::Down => {
                    if key.modifiers.contains(KeyModifiers::SHIFT) {
                        if self.selection.is_none() {
                            self.start_selection();
                        }
                        self.move_cursor(1);
                    } else {
                        self.move_cursor(1);
                    }
                }
                KeyCode::Up => {
                    if key.modifiers.contains(KeyModifiers::SHIFT) {
                        if self.selection.is_none() {
                            self.start_selection();
                        }
                        self.move_cursor(-1);
                    } else {
                        self.move_cursor(-1);
                    }
                }
                KeyCode::PageDown => {
                    let page = self.viewport_height.max(1) as isize;
                    self.scroll(page);
                    self.cursor = (self.cursor as isize + page)
                        .clamp(0, (self.lines.len().saturating_sub(1)) as isize)
                        as usize;
                    self.ensure_cursor_visible(self.viewport_height.max(1));
                }
                KeyCode::PageUp => {
                    let page = self.viewport_height.max(1) as isize;
                    self.scroll(-page);
                    self.cursor = (self.cursor as isize - page)
                        .clamp(0, (self.lines.len().saturating_sub(1)) as isize)
                        as usize;
                    self.ensure_cursor_visible(self.viewport_height.max(1));
                }
                KeyCode::Home => self.set_cursor(0),
                KeyCode::End => self.set_cursor(self.lines.len().saturating_sub(1)),
                KeyCode::Char(']') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.next_heading(true);
                }
                KeyCode::Char('[') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.next_heading(false);
                }
                KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.select_block();
                }
                KeyCode::Char('c') => self.open_comment_editor(),
                KeyCode::Char('s') => self.open_suggestion_editor(),
                KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.open_search();
                }
                KeyCode::Char('n') => self.next_match(true),
                KeyCode::Char('N') => self.next_match(false),
                KeyCode::Enter => self.enter_action(),
                _ => {}
            },
        }
    }

    fn handle_pending_key(&mut self, key: &KeyEvent) {
        if let Some(pending) = self.pending_key.take() {
            match (pending, key.code) {
                ('g', KeyCode::Char('g')) => {
                    self.set_cursor(0);
                    return;
                }
                ('[', KeyCode::Char('[')) => {
                    self.next_heading(false);
                    return;
                }
                (']', KeyCode::Char(']')) => {
                    self.next_heading(true);
                    return;
                }
                _ => {}
            }
        }
        if self.input == InputMode::Vim {
            match key.code {
                KeyCode::Char('g') | KeyCode::Char('[') | KeyCode::Char(']') => {
                    self.pending_key = match key.code {
                        KeyCode::Char(ch) => Some(ch),
                        _ => None,
                    };
                }
                _ => {}
            }
        }
    }

    fn enter_action(&mut self) {
        let block = self.current_line().map(|line| line.block);
        if let Some(block) = block
            && matches!(
                self.document.blocks.get(block).map(|b| &b.kind),
                Some(BlockKind::Mermaid { .. })
            )
        {
            self.toggle_media_source();
            return;
        }
        if self.markers.get(self.cursor).copied().unwrap_or(false) {
            self.open_item_at_line(self.cursor);
        }
    }

    fn toggle_media_source(&mut self) {
        let block = self.current_line().map(|line| line.block);
        if let Some(block) = block
            && matches!(
                self.document.blocks.get(block).map(|b| &b.kind),
                Some(BlockKind::Mermaid { .. })
            )
        {
            if self.mermaid_source.contains(&block) {
                self.mermaid_source.remove(&block);
                self.status = "Mermaid rendered view".to_string();
            } else {
                self.mermaid_source.insert(block);
                self.status = "Mermaid source view".to_string();
            }
        }
    }

    fn handle_outline_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.move_outline_cursor(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_outline_cursor(-1),
            KeyCode::Enter => {
                if let Some(item) = self.outline.get(self.outline_cursor) {
                    self.jump_to_block(item.block);
                }
            }
            KeyCode::Esc => self.focus = Focus::Document,
            _ => {}
        }
    }

    fn handle_review_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.move_review_cursor(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_review_cursor(-1),
            KeyCode::Enter => {
                if let Some(item) = self.selected_review_item() {
                    let start = item.anchor.source_start;
                    self.jump_to_source(start);
                }
            }
            KeyCode::Char('x') => self.resolve_selected(),
            KeyCode::Char('X') => self.unresolve_selected(),
            KeyCode::Char('e') => self.edit_selected_item(),
            KeyCode::Char('r') => self.reattach_selected(),
            KeyCode::Char('d') | KeyCode::Delete => self.confirm_delete_selected(),
            KeyCode::Esc => self.focus = Focus::Document,
            _ => {}
        }
    }

    fn open_search(&mut self) {
        self.search = Some(SearchState {
            query: String::new(),
            matches: Vec::new(),
            index: 0,
        });
        self.status = "Search: type a query, Enter jumps, Esc cancels".to_string();
    }

    fn handle_search_key(&mut self, key: KeyEvent) {
        let Some(search) = self.search.as_mut() else {
            return;
        };
        match key.code {
            KeyCode::Esc => {
                self.search = None;
                self.status = "Search cancelled".to_string();
            }
            KeyCode::Enter => {
                let target = search.matches.get(search.index).copied();
                let state = self.search.take();
                self.last_search = state;
                if let Some(target) = target {
                    self.set_cursor(target);
                } else {
                    self.status = "No matches".to_string();
                }
            }
            KeyCode::Backspace => {
                search.query.pop();
                search.recompute(&self.lines);
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                search.query.push(ch);
                search.recompute(&self.lines);
            }
            KeyCode::Down if !search.matches.is_empty() => {
                search.index = (search.index + 1) % search.matches.len();
            }
            KeyCode::Up if !search.matches.is_empty() => {
                search.index = (search.index + search.matches.len() - 1) % search.matches.len();
            }
            _ => {}
        }
    }

    fn next_match(&mut self, forward: bool) {
        let Some(mut search) = self.last_search.clone() else {
            self.status = "No previous search".to_string();
            return;
        };
        search.recompute(&self.lines);
        if search.matches.is_empty() {
            self.status = "No matches".to_string();
            return;
        }
        search.index = if forward {
            (search.index + 1) % search.matches.len()
        } else {
            (search.index + search.matches.len() - 1) % search.matches.len()
        };
        let target = search.matches[search.index];
        self.last_search = Some(search);
        self.set_cursor(target);
    }

    fn handle_editor_key(&mut self, key: KeyEvent) {
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        if key.code == KeyCode::Enter && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.save_editor();
            return;
        }
        if key.code == KeyCode::Esc {
            let can_cancel = match self.input {
                InputMode::Standard => true,
                InputMode::Vim => editor.state.mode == EditorMode::Normal,
            };
            if can_cancel {
                self.store.clear_draft();
                self.save_store();
                self.editor = None;
                self.status = "Edit cancelled".to_string();
                return;
            }
        }
        editor.handler.on_key_event(key, &mut editor.state);
        self.persist_draft();
    }

    fn save_editor(&mut self) {
        let Some(editor) = self.editor.take() else {
            return;
        };
        let body = editor
            .state
            .lines
            .to_string()
            .trim_end_matches('\n')
            .to_string();
        if body.trim().is_empty() && editor.target.kind == ItemKind::Comment {
            self.status = "Comment is empty; not saved".to_string();
            self.editor = Some(editor);
            return;
        }
        if editor.target.kind == ItemKind::Suggestion
            && editor.target.original.as_deref() == Some(body.as_str())
        {
            self.status = "Suggestion is unchanged; not saved".to_string();
            self.editor = Some(editor);
            return;
        }
        match &editor.target.id {
            None => match editor.target.kind {
                ItemKind::Comment => {
                    self.store.add_comment(editor.target.anchor, body);
                }
                ItemKind::Suggestion => {
                    let original = editor.target.original.clone().unwrap_or_default();
                    self.store
                        .add_suggestion(editor.target.anchor, "", original, body);
                }
            },
            Some(id) => {
                if let Some(item) = self.store.item_mut(id) {
                    item.updated_at = now();
                    match item.kind {
                        ItemKind::Comment => item.body = body,
                        ItemKind::Suggestion => {
                            if let Some(suggestion) = item.suggestion.as_mut() {
                                suggestion.replacement = body;
                            }
                        }
                    }
                }
            }
        }
        self.store.clear_draft();
        self.save_store();
        self.refresh_markers();
        self.status = "Review item saved".to_string();
    }

    pub fn handle_mouse(&mut self, mouse: MouseEvent) {
        if !self.mouse_enabled {
            return;
        }
        let Some(layout) = self.layout.clone() else {
            return;
        };
        let x = mouse.column;
        let y = mouse.row;
        match mouse.kind {
            MouseEventKind::ScrollDown => {
                self.scroll(3);
            }
            MouseEventKind::ScrollUp => {
                self.scroll(-3);
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some((_, action)) = layout
                    .footer_buttons
                    .iter()
                    .find(|(rect, _)| contains(*rect, x, y))
                {
                    self.run_footer_action(*action);
                    return;
                }
                if let Some(area) = layout.outline
                    && contains(area, x, y)
                {
                    let index = (y.saturating_sub(area.y) as usize) + layout.outline_scroll;
                    if index < self.outline.len() {
                        self.outline_cursor = index;
                        self.jump_to_block(self.outline[index].block);
                    }
                    return;
                }
                if let Some(area) = layout.review
                    && contains(area, x, y)
                {
                    let index = (y.saturating_sub(area.y) as usize) + layout.review_scroll;
                    if index < self.store.items.len() {
                        self.review_cursor = index;
                        if let Some(item) = self.selected_review_item() {
                            let start = item.anchor.source_start;
                            self.jump_to_source(start);
                        }
                    }
                    return;
                }
                if contains(layout.doc, x, y) {
                    let index = (self.scroll + (y.saturating_sub(layout.doc.y) as usize))
                        .min(self.lines.len().saturating_sub(1));
                    if x < layout.doc.x.saturating_add(2)
                        && self.markers.get(index).copied().unwrap_or(false)
                    {
                        self.open_item_at_line(index);
                        return;
                    }
                    self.drag_start = Some(index);
                    self.selection = None;
                    self.set_cursor(index);
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if let Some(start) = self.drag_start
                    && contains(layout.doc, x, y)
                {
                    let row = y.saturating_sub(layout.doc.y) as usize;
                    let index = (self.scroll + row).min(self.lines.len().saturating_sub(1));
                    self.selection = Some(Selection {
                        start,
                        end: index,
                        block: false,
                    });
                    self.cursor = index;
                    self.ensure_cursor_visible(self.viewport_height.max(1));
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.drag_start = None;
            }
            _ => {}
        }
    }

    fn run_footer_action(&mut self, action: FooterAction) {
        match action {
            FooterAction::Comment => self.open_comment_editor(),
            FooterAction::Suggest => self.open_suggestion_editor(),
            FooterAction::Resolve => self.resolve_selected(),
            FooterAction::Send => self.open_signoff(),
        }
    }

    fn open_item_at_line(&mut self, index: usize) {
        let Some(line) = self.lines.get(index) else {
            return;
        };
        let line_source = line.source.clone();
        let line_number = line.source_lines.clone();
        let id = self
            .store
            .items
            .iter()
            .filter(|item| item.status != ItemStatus::Resolved)
            .find(|item| {
                ranges_overlap(&item.anchor.range(), &line_source)
                    || lines_overlap(&item.anchor.lines(), &line_number)
            })
            .map(|item| item.id.clone());
        let Some(id) = id else {
            return;
        };
        let Some(position) = self.review_items().iter().position(|item| item.id == id) else {
            return;
        };
        self.focus = Focus::Review;
        self.review_cursor = position;
        if let Some(item) = self.selected_review_item() {
            let start = item.anchor.source_start;
            self.jump_to_source(start);
        }
        self.status = format!("Opened {id}");
    }

    pub fn check_file_change(&mut self) {
        if self.pending_reload && self.editor.is_none() && self.confirm.is_none() {
            self.pending_reload = false;
            self.reload_document();
            return;
        }
        let meta = file_meta(&self.document.path);
        if meta == self.last_meta {
            return;
        }
        self.last_meta = meta;
        if self.editor.is_some() || self.confirm.is_some() {
            self.pending_reload = true;
            self.status = "File changed on disk; finish editing to reload".to_string();
            return;
        }
        self.reload_document();
    }

    fn reload_document(&mut self) {
        let old_source = self
            .lines
            .get(self.cursor)
            .map(|line| line.source.start)
            .unwrap_or(0);
        match Document::load(&self.document.path) {
            Ok(document) => {
                if document.hash == self.document.hash {
                    self.last_meta = file_meta(&self.document.path);
                    return;
                }
                self.document = document;
                self.last_meta = file_meta(&self.document.path);
                self.mermaid_source.clear();
                self.outline = build_outline(&self.document);
                self.outline_cursor = self
                    .outline_cursor
                    .min(self.outline.len().saturating_sub(1));
                self.review_cursor = self
                    .review_cursor
                    .min(self.store.items.len().saturating_sub(1));
                self.outline_scroll = 0;
                self.review_scroll = 0;
                self.store.start_round_if_changed(&self.document);
                self.save_store();
                self.rebuild(self.layout_width);
                self.jump_to_source(old_source);
                self.status = "File changed; open items re-anchored".to_string();
            }
            Err(error) => self.status = format!("Reload failed: {error}"),
        }
    }

    pub fn source_mode(&self) -> bool {
        self.view == ViewMode::Source
    }

    pub fn input_label(&self) -> &'static str {
        match self.input {
            InputMode::Vim => "VIM",
            InputMode::Standard => "STD",
        }
    }
}

impl SearchState {
    pub fn recompute(&mut self, lines: &[VisualLine]) {
        let query = self.query.to_lowercase();
        self.matches = if query.is_empty() {
            Vec::new()
        } else {
            lines
                .iter()
                .enumerate()
                .filter(|(_, line)| line.text.to_lowercase().contains(&query))
                .map(|(index, _)| index)
                .collect()
        };
        if self.matches.is_empty() {
            self.index = 0;
        } else {
            self.index = self.index.min(self.matches.len() - 1);
        }
    }
}

impl Editor {
    fn new(target: EditorTarget, title: String, initial: String, input: InputMode) -> Self {
        let mut state = EdtuiState::new(Lines::from(initial));
        state.set_single_line(false);
        let handler = match input {
            InputMode::Vim => EditorEventHandler::vim_mode(),
            InputMode::Standard => {
                state.mode = EditorMode::Insert;
                EditorEventHandler::emacs_mode()
            }
        };
        Self {
            target,
            title,
            state,
            handler,
        }
    }

    pub fn original(&self) -> Option<&str> {
        self.target.original.as_deref()
    }

    pub fn title(&self) -> &str {
        &self.title
    }
}

fn build_outline(document: &Document) -> Vec<OutlineItem> {
    document
        .blocks
        .iter()
        .filter_map(|block| match block.kind {
            BlockKind::Heading(level) => Some(OutlineItem {
                block: block.index,
                level,
                text: block.text(),
            }),
            _ => None,
        })
        .collect()
}

fn source_lines(document: &Document) -> Vec<VisualLine> {
    let mut lines = Vec::new();
    for line_number in 1..=document.line_count() {
        let text = document
            .source_line(line_number)
            .unwrap_or_default()
            .to_string();
        let start = document.line_starts[line_number - 1];
        let end = start + text.len();
        let block = document.block_index_at(start).unwrap_or(0);
        lines.push(VisualLine {
            spans: vec![StyledSpan {
                text: text.clone(),
                style: InlineStyle::Plain,
            }],
            text,
            source: start..end,
            source_lines: line_number..(line_number + 1),
            block,
            kind: VisualKind::Text,
        });
    }
    lines
}

fn blank_visual(document: &Document) -> VisualLine {
    VisualLine {
        spans: Vec::new(),
        text: String::new(),
        source: 0..0,
        source_lines: document.lines_of(0..0),
        block: 0,
        kind: VisualKind::Text,
    }
}

fn ranges_overlap(left: &std::ops::Range<usize>, right: &std::ops::Range<usize>) -> bool {
    left.start < right.end && right.start < left.end
}

fn lines_overlap(left: &std::ops::Range<usize>, right: &std::ops::Range<usize>) -> bool {
    left.start < right.end && right.start < left.end
}

fn contains(rect: Rect, x: u16, y: u16) -> bool {
    x >= rect.x
        && x < rect.x.saturating_add(rect.width)
        && y >= rect.y
        && y < rect.y.saturating_add(rect.height)
}

fn file_meta(path: &std::path::Path) -> Option<(SystemTime, u64)> {
    let metadata = std::fs::metadata(path).ok()?;
    Some((metadata.modified().ok()?, metadata.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Keys, MermaidEngine, Protocol, Theme};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn fixture(source: &str) -> (App, tempfile::TempDir) {
        fixture_with(source, Keys::Vim)
    }

    fn fixture_with(source: &str, keys: Keys) -> (App, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("spec.md");
        std::fs::write(&path, source).unwrap();
        let document = Document::load(&path).unwrap();
        let sidecar = crate::review::sidecar_path(&path);
        let store = ReviewStore::new(path.display().to_string(), document.hash.clone());
        let options = TuiOptions {
            keys,
            mouse: true,
            protocol: Protocol::Halfblocks,
            theme: Theme::Dark,
            mermaid_engine: MermaidEngine::Source,
            cache_dir: dir.path().join("cache"),
        };
        (App::new(document, store, sidecar, options), dir)
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn navigation_and_focus() {
        let (mut app, _dir) = fixture("# A\n\none\n\ntwo\n");
        app.handle_key(key(KeyCode::Char('j')));
        assert_eq!(app.cursor, 1);
        app.handle_key(key(KeyCode::Tab));
        assert_eq!(app.focus, Focus::Outline);
        app.handle_key(key(KeyCode::Tab));
        assert_eq!(app.focus, Focus::Review);
        app.handle_key(key(KeyCode::Tab));
        assert_eq!(app.focus, Focus::Document);
    }

    #[test]
    fn comment_editor_creates_item() {
        let (mut app, _dir) = fixture("# A\n\nhello world\n");
        app.set_cursor(1);
        app.handle_key(key(KeyCode::Char('c')));
        assert!(app.editor.is_some());
        let editor = app.editor.as_mut().unwrap();
        editor.state.lines = edtui::Lines::from("clarify this");
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL));
        assert!(app.editor.is_none());
        assert_eq!(app.store.items.len(), 1);
        assert_eq!(app.store.items[0].body, "clarify this");
    }

    #[test]
    fn suggestion_editor_creates_item() {
        let (mut app, _dir) = fixture("# A\n\nhello world\n");
        app.set_cursor(1);
        app.handle_key(key(KeyCode::Char('V')));
        app.handle_key(key(KeyCode::Char('s')));
        assert!(app.editor.is_some());
        let editor = app.editor.as_mut().unwrap();
        editor.state.lines = edtui::Lines::from("replacement text");
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL));
        assert_eq!(app.store.items.len(), 1);
        assert_eq!(app.store.items[0].kind, ItemKind::Suggestion);
        assert_eq!(
            app.store.items[0].suggestion.as_ref().unwrap().replacement,
            "replacement text"
        );
    }

    #[test]
    fn signoff_exit_codes() {
        let (mut app, _dir) = fixture("# A\n\nhello\n");
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL));
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.exit, Some(TuiExit::Approved));
        assert!(app.store.is_approved());

        let (mut app, _dir) = fixture("# A\n\nhello\n");
        app.set_cursor(1);
        app.handle_key(key(KeyCode::Char('c')));
        app.editor.as_mut().unwrap().state.lines = edtui::Lines::from("fix");
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL));
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.exit, Some(TuiExit::Feedback));
    }

    #[test]
    fn search_and_view_toggle() {
        let (mut app, _dir) = fixture("# A\n\nalpha\n\nbeta\n");
        app.handle_key(key(KeyCode::Char('/')));
        app.search.as_mut().unwrap().query = "beta".to_string();
        app.search.as_mut().unwrap().recompute(&app.lines);
        app.handle_key(key(KeyCode::Enter));
        assert!(app.current_line().unwrap().text.contains("beta"));
        app.handle_key(key(KeyCode::F(3)));
        assert!(app.source_mode());
        assert!(app.current_line().unwrap().text.contains("beta"));
    }

    #[test]
    fn resolve_and_delete_items() {
        let (mut app, _dir) = fixture("# A\n\nhello\n");
        app.set_cursor(1);
        app.handle_key(key(KeyCode::Char('c')));
        app.editor.as_mut().unwrap().state.lines = edtui::Lines::from("note");
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL));
        app.focus = Focus::Review;
        app.handle_key(key(KeyCode::Char('x')));
        assert_eq!(app.store.resolved_count(), 1);
        app.handle_key(key(KeyCode::Char('d')));
        app.handle_key(key(KeyCode::Enter));
        assert!(app.store.items.is_empty());
    }

    #[test]
    fn edit_existing_comment() {
        let (mut app, _dir) = fixture("# A\n\nhello\n");
        app.set_cursor(1);
        app.handle_key(key(KeyCode::Char('c')));
        app.set_editor_text("first");
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL));
        app.focus = Focus::Review;
        app.handle_key(key(KeyCode::Char('e')));
        assert!(app.editor.is_some());
        app.set_editor_text("second");
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL));
        assert_eq!(app.store.items[0].body, "second");
    }

    #[test]
    fn block_and_heading_motions() {
        let (mut app, _dir) = fixture("# A\n\none\n\n## B\n\ntwo\n");
        assert_eq!(app.current_block().unwrap().index, 0);
        app.handle_key(key(KeyCode::Char('}')));
        assert_eq!(app.current_block().unwrap().index, 1);
        app.handle_key(key(KeyCode::Char('}')));
        assert_eq!(app.current_block().unwrap().index, 2);
        app.handle_key(key(KeyCode::Char('g')));
        app.handle_key(key(KeyCode::Char('g')));
        assert_eq!(app.cursor, 0);
        app.handle_key(key(KeyCode::Char(']')));
        app.handle_key(key(KeyCode::Char(']')));
        assert_eq!(app.current_block().unwrap().index, 2);
    }

    #[test]
    fn standard_mode_navigation_and_comment() {
        let (mut app, _dir) = fixture_with("# A\n\nhello\n", Keys::Standard);
        assert_eq!(app.input, InputMode::Standard);
        app.handle_key(key(KeyCode::Down));
        assert_eq!(app.cursor, 1);
        app.handle_key(key(KeyCode::Char('c')));
        assert!(app.editor.is_some());
        app.handle_key(key(KeyCode::Esc));
        assert!(app.editor.is_none());
        app.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL));
        assert!(app.search.is_some());
    }

    #[test]
    fn draft_resumes_after_reopen() {
        let (mut app, dir) = fixture("# A\n\nhello\n");
        app.set_cursor(1);
        app.handle_key(key(KeyCode::Char('c')));
        app.set_editor_text("unfinished thought");
        let sidecar = app.sidecar.clone();
        let document_path = app.document.path.clone();
        drop(app);

        let document = Document::load(&document_path).unwrap();
        let store = ReviewStore::load(&sidecar).unwrap();
        let options = TuiOptions {
            keys: Keys::Vim,
            mouse: true,
            protocol: Protocol::Halfblocks,
            theme: Theme::Dark,
            mermaid_engine: MermaidEngine::Source,
            cache_dir: dir.path().join("cache"),
        };
        let reopened = App::new(document, store, sidecar, options);
        assert!(reopened.editor.is_some());
        assert_eq!(
            reopened.editor_text().as_deref(),
            Some("unfinished thought")
        );
    }
}
