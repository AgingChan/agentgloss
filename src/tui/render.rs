//! Ratatui rendering for the reviewer.

use super::app::{App, Focus, FooterAction, LayoutInfo};
use crate::document::{BlockKind, InlineStyle, VisualKind};
use crate::review::{ItemKind, ItemStatus};
use edtui::EditorView;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui_image::Image;

pub fn draw(frame: &mut Frame<'_>, app: &mut App) {
    let area = frame.area();
    if area.width < 20 || area.height < 10 {
        frame.render_widget(
            Paragraph::new("Terminal too small for mdreview.\nMinimum: 20x10")
                .wrap(Wrap { trim: true }),
            area,
        );
        return;
    }

    let header = Rect::new(area.x, area.y, area.width, 1);
    let footer = Rect::new(area.x, area.bottom().saturating_sub(2), area.width, 2);
    let body = Rect::new(
        area.x,
        area.y + 1,
        area.width,
        footer.y.saturating_sub(area.y + 1),
    );

    let (outline_area, review_area, doc_area) = panel_areas(body, app.focus, area.width);
    let narrow_overlay = if area.width < 80 && app.focus != Focus::Document {
        Some((app.focus, centered(body, 92, 90)))
    } else {
        None
    };
    if app.layout_width() != doc_area.width as usize {
        app.rebuild(doc_area.width as usize);
    }
    app.set_viewport(doc_area.height as usize);
    app.prepare_media(doc_area);

    if app.has_visible_media(doc_area) {
        frame.render_widget(Clear, doc_area);
    }
    render_header(frame, app, header);
    render_document(frame, app, doc_area);
    render_media(frame, app, doc_area);
    let mut interactive_outline = None;
    let mut interactive_review = None;
    if let Some(area) = outline_area {
        render_outline(frame, app, area);
        interactive_outline = Some(panel_inner(area));
    }
    if let Some(area) = review_area {
        render_review(frame, app, area);
        interactive_review = Some(panel_inner(area));
    }
    if let Some((focus, overlay)) = narrow_overlay {
        frame.render_widget(Clear, overlay);
        match focus {
            Focus::Outline => {
                render_outline(frame, app, overlay);
                interactive_outline = Some(panel_inner(overlay));
            }
            Focus::Review => {
                render_review(frame, app, overlay);
                interactive_review = Some(panel_inner(overlay));
            }
            Focus::Document => {}
        }
    }
    let buttons = render_footer(frame, app, footer);
    app.layout = Some(LayoutInfo {
        doc: doc_area,
        outline: interactive_outline,
        review: interactive_review,
        footer,
        footer_buttons: buttons,
        doc_height: doc_area.height as usize,
        outline_scroll: app.outline_scroll,
        review_scroll: app.review_scroll,
    });

    if app.help {
        render_help(frame, area);
    }
    if let Some(message) = app.confirm.as_ref().map(|confirm| confirm.message.clone()) {
        render_confirm(frame, area, &message);
    }
    if app.search.is_some() {
        render_search(frame, app, doc_area);
    }
    if app.editor.is_some() {
        render_editor(frame, app, area);
    }
}

fn panel_areas(body: Rect, focus: Focus, width: u16) -> (Option<Rect>, Option<Rect>, Rect) {
    if width >= 110 {
        let outline = Rect::new(body.x, body.y, 26, body.height);
        let review = Rect::new(body.right().saturating_sub(34), body.y, 34, body.height);
        let doc = Rect::new(
            body.x + 26,
            body.y,
            body.width.saturating_sub(26 + 34),
            body.height,
        );
        (Some(outline), Some(review), doc)
    } else if width >= 80 && focus == Focus::Outline {
        let outline = Rect::new(body.x, body.y, 26, body.height);
        let doc = Rect::new(
            body.x + 26,
            body.y,
            body.width.saturating_sub(26),
            body.height,
        );
        (Some(outline), None, doc)
    } else if width >= 80 && focus == Focus::Review {
        let review = Rect::new(body.right().saturating_sub(32), body.y, 32, body.height);
        let doc = Rect::new(body.x, body.y, body.width.saturating_sub(32), body.height);
        (None, Some(review), doc)
    } else {
        (None, None, body)
    }
}

fn render_header(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let file = app
        .document
        .path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| app.document.path.display().to_string());
    let text = format!(
        " mdreview  {}  round {}  open {}  [{}]",
        file,
        app.store.round,
        app.store.open_count(),
        app.input_label()
    );
    let text = truncate(&text, area.width as usize);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            text,
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))),
        area,
    );
}

fn render_document(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let height = area.height as usize;
    let start = app.scroll.min(app.lines.len().saturating_sub(1));
    let end = (start + height).min(app.lines.len());
    let mut lines = Vec::with_capacity(end.saturating_sub(start));
    for index in start..end {
        lines.push(render_line(app, index));
    }
    let paragraph = Paragraph::new(Text::from(lines));
    frame.render_widget(paragraph, area);
    if app.lines.is_empty() {
        frame.render_widget(Paragraph::new("Empty document"), area);
    }
}

fn render_line(app: &App, index: usize) -> Line<'static> {
    let line = &app.lines[index];
    if line.is_media()
        && app
            .media_key(line.block)
            .is_some_and(|key| app.media.get(key).is_some())
    {
        return Line::default();
    }
    let mut spans: Vec<Span<'static>> = Vec::new();
    if app.source_mode() {
        let marker = if app.markers.get(index).copied().unwrap_or(false) {
            "● "
        } else {
            "  "
        };
        spans.push(Span::styled(
            marker.to_string(),
            Style::default().fg(Color::Red),
        ));
        let number = line.source_lines.start;
        spans.push(Span::styled(
            format!("{number:>5} "),
            Style::default().fg(Color::DarkGray),
        ));
    } else {
        let marker = if app.markers.get(index).copied().unwrap_or(false) {
            "● "
        } else {
            "  "
        };
        spans.push(Span::styled(
            marker.to_string(),
            Style::default().fg(Color::Red),
        ));
    }

    let block = app.document.blocks.get(line.block);
    for span in &line.spans {
        let mut style = inline_style(&span.style);
        if let Some(block) = block
            && matches!(block.kind, BlockKind::Heading(_))
        {
            style = style.fg(Color::Cyan).add_modifier(Modifier::BOLD);
        }
        if matches!(line.kind, VisualKind::Media(_)) {
            style = style.fg(Color::DarkGray).add_modifier(Modifier::ITALIC);
        }
        spans.push(Span::styled(span.text.clone(), style));
    }

    let mut line_style = Style::default();
    if is_selected(app, index) {
        line_style = line_style.bg(Color::DarkGray);
    }
    if app.focus == Focus::Document && app.cursor == index {
        line_style = line_style.bg(Color::Blue).fg(Color::White);
    }
    if let Some(search) = &app.search
        && !search.query.is_empty()
        && line
            .text
            .to_lowercase()
            .contains(&search.query.to_lowercase())
    {
        line_style = line_style.add_modifier(Modifier::UNDERLINED);
    }
    Line::from(spans).style(line_style)
}

fn inline_style(style: &InlineStyle) -> Style {
    match style {
        InlineStyle::Plain | InlineStyle::ImageAlt => Style::default(),
        InlineStyle::Emphasis => Style::default().add_modifier(Modifier::ITALIC),
        InlineStyle::Strong => Style::default().add_modifier(Modifier::BOLD),
        InlineStyle::Strikethrough => Style::default().add_modifier(Modifier::CROSSED_OUT),
        InlineStyle::Code => Style::default().fg(Color::Yellow).bg(Color::Black),
        InlineStyle::Link => Style::default()
            .fg(Color::Blue)
            .add_modifier(Modifier::UNDERLINED),
        InlineStyle::TaskMarker => Style::default().fg(Color::Green),
    }
}

fn render_media(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let start = app.scroll;
    let end = (start + area.height as usize).min(app.lines.len());
    let mut index = start;
    while index < end {
        if !app.lines[index].is_media() {
            index += 1;
            continue;
        }
        let block = app.lines[index].block;
        let group_start = index;
        let mut group_end = index;
        while group_end < end
            && app.lines[group_end].is_media()
            && app.lines[group_end].block == block
        {
            group_end += 1;
        }
        let rect = Rect::new(
            area.x,
            area.y + (group_start.saturating_sub(start)) as u16,
            area.width,
            (group_end - group_start) as u16,
        );
        if app.mermaid_source.contains(&block) {
            if let Some(crate::document::BlockKind::Mermaid { source }) =
                app.document.blocks.get(block).map(|block| &block.kind)
            {
                render_mermaid_source(frame, source, rect, "Mermaid source");
            }
            index = group_end;
            continue;
        }
        if let Some(key) = app.media_key(block) {
            if let Some(protocol) = app.media.get(key) {
                if rect.height > 0 && rect.width > 1 {
                    frame.render_widget(Image::new(protocol), rect);
                }
            } else if app.media.error(key).is_some()
                && let Some(crate::document::BlockKind::Mermaid { source }) =
                    app.document.blocks.get(block).map(|block| &block.kind)
            {
                render_mermaid_source(frame, source, rect, "diagram unavailable — source");
            }
        }
        index = group_end;
    }
}

fn render_mermaid_source(frame: &mut Frame<'_>, source: &str, rect: Rect, title: &str) {
    frame.render_widget(Clear, rect);
    let text = format!("```mermaid\n{source}\n```");
    frame.render_widget(
        Paragraph::new(text)
            .style(Style::default().fg(Color::Yellow))
            .block(Block::default().borders(Borders::ALL).title(title))
            .wrap(Wrap { trim: false }),
        rect,
    );
}

fn render_outline(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Outline")
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let height = inner.height as usize;
    if height == 0 {
        return;
    }
    app.outline_scroll = scroll_for(app.outline_cursor, app.outline_scroll, height);
    let mut lines = Vec::new();
    for (index, item) in app
        .outline
        .iter()
        .enumerate()
        .skip(app.outline_scroll)
        .take(height)
    {
        let indent = "  ".repeat(item.level.saturating_sub(1) as usize);
        let text = format!("{indent}{}", item.text);
        let style = if app.focus == Focus::Outline && index == app.outline_cursor {
            Style::default().bg(Color::DarkGray).fg(Color::White)
        } else {
            Style::default()
        };
        lines.push(Line::from(Span::styled(text, style)));
    }
    frame.render_widget(Paragraph::new(Text::from(lines)), inner);
}

fn render_review(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(
            "Review ({}/{} open)",
            app.store.open_count(),
            app.store.items.len()
        ))
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let height = inner.height as usize;
    if height == 0 {
        return;
    }
    app.review_scroll = scroll_for(app.review_cursor, app.review_scroll, height);
    let items = app.review_items();
    let mut lines = Vec::new();
    for (index, item) in items
        .iter()
        .enumerate()
        .skip(app.review_scroll)
        .take(height)
    {
        let marker = match item.kind {
            ItemKind::Comment => "●",
            ItemKind::Suggestion => "✎",
        };
        let line_no = item.anchor.line_start;
        let excerpt: String = item
            .anchor
            .quote
            .chars()
            .take(24)
            .map(|ch| if ch == '\n' || ch == '\t' { ' ' } else { ch })
            .collect();
        let text = format!(
            "{marker} {} {} L{} {}",
            item.id,
            item.status.label(),
            line_no,
            excerpt
        );
        let style = if app.focus == Focus::Review && index == app.review_cursor {
            Style::default().bg(Color::DarkGray).fg(Color::White)
        } else if item.status == ItemStatus::Stale {
            Style::default().fg(Color::Yellow)
        } else if item.status == ItemStatus::Resolved {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default()
        };
        lines.push(Line::from(Span::styled(text, style)));
    }
    frame.render_widget(Paragraph::new(Text::from(lines)), inner);
}

fn render_footer(frame: &mut Frame<'_>, app: &App, area: Rect) -> Vec<(Rect, FooterAction)> {
    let status = truncate(&format!(" {}", app.status), area.width as usize);
    frame.render_widget(
        Paragraph::new(status).style(Style::default().fg(Color::Gray)),
        Rect::new(area.x, area.y, area.width, 1),
    );
    let y = area.y + 1;
    let mut buttons = Vec::new();
    let labels = [
        ("Comment", FooterAction::Comment),
        ("Suggest", FooterAction::Suggest),
        ("Resolve", FooterAction::Resolve),
        ("Send", FooterAction::Send),
    ];
    let mut spans = Vec::new();
    let mut x = area.x + 1;
    for (label, action) in labels {
        let text = format!("[{label}]");
        let width = text.chars().count() as u16;
        if x + width >= area.right() {
            break;
        }
        buttons.push((Rect::new(x, y, width, 1), action));
        spans.push(Span::styled(
            text,
            Style::default()
                .fg(Color::Black)
                .bg(Color::Gray)
                .add_modifier(Modifier::BOLD),
        ));
        x += width + 1;
    }
    let hints = match app.focus {
        Focus::Document => "  Tab focus  ? help  Ctrl+Enter send  q quit",
        Focus::Outline => "  j/k move  Enter jump  Esc back",
        Focus::Review => {
            "  j/k move  Enter jump  e edit  r reattach  x resolve  X reopen  d delete  Esc back"
        }
    };
    spans.push(Span::styled(
        truncate(hints, area.width.saturating_sub(x) as usize),
        Style::default().fg(Color::DarkGray),
    ));
    frame.render_widget(
        Paragraph::new(Line::from(spans)),
        Rect::new(area.x, y, area.width, 1),
    );
    buttons
}

fn render_help(frame: &mut Frame<'_>, area: Rect) {
    let popup = centered(area, 76, 80);
    frame.render_widget(Clear, popup);
    let text = vec![
        Line::from("Navigation"),
        Line::from("  j/k or arrows       move one line"),
        Line::from("  Ctrl-d / Ctrl-u     half page"),
        Line::from("  gg / G              top / bottom"),
        Line::from("  ]] / [[             next / previous heading"),
        Line::from("  } / {               next / previous paragraph"),
        Line::from("  / or Ctrl-f         search; n/N next/previous"),
        Line::from(""),
        Line::from("Review"),
        Line::from("  v / Shift+arrows    visual selection"),
        Line::from("  V                   select current line"),
        Line::from("  P / Ctrl-p          select current block"),
        Line::from("  c                   add comment"),
        Line::from("  s                   add suggestion"),
        Line::from("  Enter               toggle Mermaid source"),
        Line::from("  x / X               resolve / reopen"),
        Line::from("  e                   edit selected item"),
        Line::from("  r                   reattach a stale item to the current selection"),
        Line::from("  d / Delete          delete selected item"),
        Line::from("  Ctrl+Enter          send review / approve"),
        Line::from("  Tab                 cycle document, outline, review"),
        Line::from("  F2 / F3             toggle input mode / rendered-source"),
        Line::from("  q                   quit (autosaves)"),
        Line::from(""),
        Line::from("Mouse: click to place cursor, drag to select, wheel to scroll."),
        Line::from("Press Esc, ? or q to close this help."),
    ];
    frame.render_widget(
        Paragraph::new(Text::from(text))
            .block(Block::default().borders(Borders::ALL).title("Help"))
            .wrap(Wrap { trim: false }),
        popup,
    );
}

fn render_confirm(frame: &mut Frame<'_>, area: Rect, message: &str) {
    let popup = centered(area, 70, 30);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(Text::from(vec![
            Line::from(message),
            Line::from(""),
            Line::from("Enter confirms · Esc cancels"),
        ]))
        .block(Block::default().borders(Borders::ALL).title("Confirm"))
        .wrap(Wrap { trim: true }),
        popup,
    );
}

fn render_search(frame: &mut Frame<'_>, app: &App, doc_area: Rect) {
    let Some(search) = &app.search else {
        return;
    };
    let area = Rect::new(doc_area.x, doc_area.y, doc_area.width, 1);
    frame.render_widget(Clear, area);
    let count = search.matches.len();
    let text = format!(
        " /{}  ({} match{})  Enter jump · Esc cancel",
        search.query,
        count,
        if count == 1 { "" } else { "es" }
    );
    frame.render_widget(
        Paragraph::new(text).style(Style::default().fg(Color::Black).bg(Color::Yellow)),
        area,
    );
}

fn render_editor(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let popup = centered(area, 86, 78);
    frame.render_widget(Clear, popup);
    let Some(editor) = app.editor.as_mut() else {
        return;
    };
    let title = editor.title().to_string();
    let original = editor.original().map(ToOwned::to_owned);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!("{title} · Ctrl+Enter saves · Esc cancels"));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let mut chunks = Vec::new();
    if let Some(original) = original {
        let original_lines: Vec<Line> = original
            .lines()
            .map(|line| {
                Line::from(Span::styled(
                    line.to_string(),
                    Style::default().fg(Color::Yellow),
                ))
            })
            .collect();
        let height = (original_lines.len() as u16 + 1).min(inner.height.saturating_sub(3).max(1));
        let split = Layout::vertical([Constraint::Length(height), Constraint::Min(1)]).split(inner);
        frame.render_widget(
            Paragraph::new(Text::from(original_lines))
                .block(Block::default().borders(Borders::BOTTOM).title("Original")),
            split[0],
        );
        chunks.push(split[1]);
    } else {
        chunks.push(inner);
    }
    frame.render_widget(EditorView::new(&mut editor.state).wrap(true), chunks[0]);
}

fn is_selected(app: &App, index: usize) -> bool {
    app.selection.is_some_and(|selection| {
        index >= selection.start.min(selection.end) && index <= selection.start.max(selection.end)
    })
}

fn scroll_for(cursor: usize, current: usize, height: usize) -> usize {
    if cursor < current {
        cursor
    } else if cursor >= current + height {
        cursor + 1 - height
    } else {
        current
    }
}

fn centered(area: Rect, width_percent: u16, height_percent: u16) -> Rect {
    let width = area
        .width
        .saturating_mul(width_percent)
        .saturating_div(100)
        .max(20)
        .min(area.width);
    let height = area
        .height
        .saturating_mul(height_percent)
        .saturating_div(100)
        .max(6)
        .min(area.height);
    Rect::new(
        area.x + (area.width.saturating_sub(width)) / 2,
        area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    )
}

fn panel_inner(area: Rect) -> Rect {
    Rect::new(
        area.x.saturating_add(1),
        area.y.saturating_add(1),
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    )
}

fn truncate(text: &str, width: usize) -> String {
    text.chars().take(width).collect()
}
