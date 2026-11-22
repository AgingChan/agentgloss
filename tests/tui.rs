use agentgloss::config::{Keys, MermaidEngine, Protocol, Theme};
use agentgloss::document::Document;
use agentgloss::review::{Anchor, ItemStatus, ReviewStore, sidecar_path};
use agentgloss::tui::TuiOptions;
use agentgloss::tui::app::{App, Focus};
use agentgloss::tui::render;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::{Color, Modifier};
use std::path::Path;

fn options(dir: &Path) -> TuiOptions {
    TuiOptions {
        keys: Keys::Vim,
        mouse: true,
        protocol: Protocol::Halfblocks,
        theme: Theme::Dark,
        mermaid_engine: MermaidEngine::Rust,
        cache_dir: dir.join("cache"),
    }
}

fn fixture(source: &str) -> (App, tempfile::TempDir) {
    fixture_with_engine(source, MermaidEngine::Rust)
}

fn fixture_with_engine(source: &str, engine: MermaidEngine) -> (App, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("spec.md");
    std::fs::write(&path, source).unwrap();
    let document = Document::load(&path).unwrap();
    let sidecar = sidecar_path(&path);
    let store = ReviewStore::new(path.display().to_string(), document.hash.clone());
    let mut opts = options(dir.path());
    opts.mermaid_engine = engine;
    (App::new(document, store, sidecar, opts), dir)
}

fn draw(app: &mut App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render::draw(frame, app)).unwrap();
    let buffer = terminal.backend().buffer();
    let mut out = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            out.push_str(buffer[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

#[test]
fn renders_at_80x24_and_wide() {
    let (mut app, _dir) = fixture("# Plan\n\nHello world.\n\nSecond paragraph.\n");
    let text = draw(&mut app, 80, 24);
    assert!(text.contains("agentgloss"));
    assert!(text.contains("Plan"));
    assert!(text.contains("Hello world"));
    assert!(text.contains("[Comment]"));

    let text = draw(&mut app, 120, 40);
    assert!(text.contains("Outline"));
    assert!(text.contains("Review"));
}

#[test]
fn comment_marker_and_review_panel_render() {
    let (mut app, _dir) = fixture("# Plan\n\nHello world.\n");
    let start = app.document.source.find("Hello world.").unwrap();
    let anchor = Anchor::from_range(&app.document, start..start + 12).unwrap();
    app.store.add_comment(anchor, "Clarify this.");
    app.refresh_markers();
    let text = draw(&mut app, 120, 40);
    assert!(text.contains("c1"));
    assert!(text.contains("open"));
}

#[test]
fn source_toggle_keeps_cursor() {
    let (mut app, _dir) = fixture("# Plan\n\nHello world.\n");
    app.set_cursor(2);
    app.toggle_view();
    let text = draw(&mut app, 80, 24);
    assert!(text.contains("Hello world"));
    assert!(app.source_mode());
}

#[test]
fn mouse_click_and_drag_select_lines() {
    let (mut app, _dir) = fixture("# Plan\n\nHello world.\n\nThird line.\n");
    draw(&mut app, 80, 24);
    let layout = app.layout.clone().unwrap();
    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: layout.doc.x + 2,
        row: layout.doc.y + 2,
        modifiers: KeyModifiers::NONE,
    };
    app.handle_mouse(click);
    assert!(app.cursor >= 2);
    let drag = MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: layout.doc.x + 2,
        row: layout.doc.y + 4,
        modifiers: KeyModifiers::NONE,
    };
    app.handle_mouse(drag);
    assert!(app.selection.is_some());
}

#[test]
fn file_change_reanchors_and_updates_round() {
    let (mut app, dir) = fixture("# Plan\n\nHello world.\n");
    let start = app.document.source.find("Hello world.").unwrap();
    let anchor = Anchor::from_range(&app.document, start..start + 12).unwrap();
    app.store.add_comment(anchor, "Keep this.");
    app.store.signoff_feedback();
    app.store.save(&app.sidecar).unwrap();

    std::thread::sleep(std::time::Duration::from_millis(20));
    let path = dir.path().join("spec.md");
    std::fs::write(&path, "# Plan\n\n## Added\n\nHello world.\n").unwrap();
    app.check_file_change();
    assert_eq!(app.store.items.len(), 1);
    assert_eq!(app.store.round, 2);
    assert_eq!(app.store.open_count(), 1);
    assert_eq!(app.outline.len(), 2);
}

#[test]
fn resolved_items_stay_archived_across_reloads() {
    let (mut app, dir) = fixture("# Plan\n\nHello world.\n");
    let start = app.document.source.find("Hello world.").unwrap();
    let anchor = Anchor::from_range(&app.document, start..start + 12).unwrap();
    let id = app.store.add_comment(anchor, "Old note.");
    app.store.resolve(&id).unwrap();
    app.store.signoff_feedback();
    app.store.save(&app.sidecar).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    let path = dir.path().join("spec.md");
    std::fs::write(&path, "# Plan\n\nHello world.\n\nChanged.\n").unwrap();
    app.check_file_change();
    assert_eq!(app.store.resolved_count(), 1);
    assert_eq!(app.store.open_count(), 0);
}

#[test]
fn image_media_is_prepared_and_cached() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("spec.md");
    let image_path = dir.path().join("red.png");
    let image = image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]));
    image.save(&image_path).unwrap();
    std::fs::write(&path, "# Media\n\n![red](red.png)\n").unwrap();
    let document = Document::load(&path).unwrap();
    let store = ReviewStore::new(path.display().to_string(), document.hash.clone());
    let mut app = App::new(document, store, sidecar_path(&path), options(dir.path()));
    draw(&mut app, 80, 24);
    assert!(!app.media_keys.is_empty());
    let key = app.media_keys.values().next().unwrap().clone();
    assert!(
        app.media.wait_for(&key, std::time::Duration::from_secs(10)),
        "{:?}",
        app.media.error(&key)
    );
    assert!(app.media.get(&key).is_some());
}

#[test]
fn loaded_image_rows_do_not_contain_the_media_label() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("spec.md");
    let image_path = dir.path().join("red.png");
    let image = image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]));
    image.save(&image_path).unwrap();
    std::fs::write(&path, "# Media\n\n![red](red.png)\n").unwrap();
    let document = Document::load(&path).unwrap();
    let store = ReviewStore::new(path.display().to_string(), document.hash.clone());
    let mut app = App::new(document, store, sidecar_path(&path), options(dir.path()));
    draw(&mut app, 120, 40);
    let key = app.media_keys.values().next().unwrap().clone();
    assert!(
        app.media.wait_for(&key, std::time::Duration::from_secs(10)),
        "{:?}",
        app.media.error(&key)
    );
    draw(&mut app, 120, 40);

    let layout = app.layout.clone().unwrap();
    let line = app
        .lines
        .iter()
        .position(|line| line.is_media())
        .expect("media line");
    let y = layout.doc.y + (line.saturating_sub(app.scroll)) as u16;
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| render::draw(frame, &mut app))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let row: String = (0..buffer.area.width)
        .map(|x| buffer[(x, y)].symbol())
        .collect();
    assert!(!row.contains("image"), "{row:?}");
    assert!(!row.contains("red.png"), "{row:?}");
}

#[test]
fn remote_image_renders_as_a_link_without_starting_a_worker() {
    let (mut app, _dir) = fixture("# Remote\n\n![diagram](https://example.com/remote.png)\n");
    let text = draw(&mut app, 100, 30);
    assert!(text.contains("https://example.com/remote.png"), "{text}");
    assert!(text.contains("diagram"), "{text}");
    assert!(
        app.media_keys.is_empty(),
        "remote images must not schedule media work: {:?}",
        app.media_keys
    );
}

#[test]
fn mermaid_media_is_prepared_and_cached() {
    let (mut app, _dir) = fixture("# Diagram\n\n```mermaid\nflowchart LR\nA-->B\n```\n");
    draw(&mut app, 100, 30);
    assert!(!app.media_keys.is_empty());
    let key = app.media_keys.values().next().unwrap().clone();
    assert!(
        app.media.wait_for(&key, std::time::Duration::from_secs(10)),
        "{:?}",
        app.media.error(&key)
    );
    assert!(app.media.get(&key).is_some());
}

#[test]
fn partially_visible_media_keeps_a_stable_cache_key() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("spec.md");
    let image_path = dir.path().join("red.png");
    image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]))
        .save(&image_path)
        .unwrap();
    let mut source = String::from("# Media\n\n");
    for index in 0..6 {
        source.push_str(&format!("Paragraph {index} before the image.\n\n"));
    }
    source.push_str("![red](red.png)\n");
    std::fs::write(&path, source).unwrap();
    let document = Document::load(&path).unwrap();
    let store = ReviewStore::new(path.display().to_string(), document.hash.clone());
    let mut app = App::new(document, store, sidecar_path(&path), options(dir.path()));
    draw(&mut app, 80, 20);
    let first = app.media_keys.values().next().unwrap().clone();
    app.scroll(2);
    draw(&mut app, 80, 20);
    let second = app.media_keys.values().next().unwrap().clone();
    assert_eq!(first, second);
}

#[test]
fn tiny_terminal_does_not_panic() {
    let (mut app, _dir) = fixture("# A\n");
    let text = draw(&mut app, 12, 4);
    assert!(text.contains("Terminal"));
}

#[test]
fn gutter_width_is_reserved_in_layout() {
    let long = "x".repeat(78);
    let source = format!("# A\n\n{long}\n");
    let (mut app, _dir) = fixture(&source);
    let text = draw(&mut app, 80, 24);
    assert_eq!(app.layout_width(), 78);
    assert!(text.matches('x').count() >= 78);
}

#[test]
fn enter_on_a_commented_line_opens_the_item() {
    let (mut app, _dir) = fixture("# A\n\nhello world\n");
    let start = app.document.source.find("hello world").unwrap();
    let anchor = Anchor::from_range(&app.document, start..start + 11).unwrap();
    app.store.add_comment(anchor, "note");
    app.refresh_markers();
    let line = app
        .lines
        .iter()
        .position(|line| line.text.contains("hello world"))
        .unwrap();
    app.set_cursor(line);
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.focus, Focus::Review);
}

#[test]
fn review_panel_and_footer_clicks_work() {
    let (mut app, _dir) = fixture("# A\n\nhello world\n");
    let start = app.document.source.find("hello world").unwrap();
    let anchor = Anchor::from_range(&app.document, start..start + 11).unwrap();
    app.store.add_comment(anchor, "note");
    app.refresh_markers();
    app.focus = Focus::Document;
    draw(&mut app, 120, 40);
    let review = app.layout.as_ref().unwrap().review.unwrap();
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: review.x,
        row: review.y,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.review_cursor, 0);
    assert_eq!(app.focus, Focus::Review);

    app.focus = Focus::Document;
    draw(&mut app, 80, 24);
    let layout = app.layout.clone().unwrap();
    let (comment_button, _) = layout
        .footer_buttons
        .iter()
        .find(|(_, action)| *action == agentgloss::tui::app::FooterAction::Comment)
        .unwrap();
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: comment_button.x,
        row: comment_button.y,
        modifiers: KeyModifiers::NONE,
    });
    assert!(app.is_editing());
}

#[test]
fn send_button_hits_the_send_action() {
    let (mut app, _dir) = fixture("# A\n\nhello\n");
    draw(&mut app, 80, 24);
    let layout = app.layout.clone().unwrap();
    let (send, _) = layout
        .footer_buttons
        .iter()
        .find(|(_, action)| *action == agentgloss::tui::app::FooterAction::Send)
        .unwrap();
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: send.x,
        row: send.y,
        modifiers: KeyModifiers::NONE,
    });
    assert!(app.is_confirming());
}

#[test]
fn renders_at_100x30() {
    let (mut app, _dir) = fixture("# Plan\n\nHello world.\n\n## Next\n\nMore.\n");
    app.focus = Focus::Review;
    let text = draw(&mut app, 100, 30);
    assert!(text.contains("Plan"));
    assert!(text.contains("Review"));
}

#[test]
fn mouse_actions_do_not_bypass_an_open_editor() {
    let (mut app, _dir) = fixture("# A\n\nhello world\n");
    app.set_cursor(2);
    app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));
    assert!(app.is_editing());
    app.set_editor_text("unfinished thought");
    draw(&mut app, 120, 40);
    let layout = app.layout.clone().unwrap();
    for (rect, _) in layout.footer_buttons {
        app.handle_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: rect.x,
            row: rect.y,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(app.editor_text().as_deref(), Some("unfinished thought"));
        assert!(app.is_editing());
        assert!(!app.is_confirming());
    }
    let cursor = app.cursor;
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: layout.doc.x + 5,
        row: layout.doc.y + 1,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.cursor, cursor);
    assert_eq!(app.editor_text().as_deref(), Some("unfinished thought"));
}

#[test]
fn signoff_from_tui_exits_with_feedback() {
    let (mut app, _dir) = fixture("# Plan\n\nHello world.\n");
    app.set_cursor(1);
    app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));
    app.set_editor_text("Fix it");
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL));
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL));
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.exit, Some(agentgloss::tui::TuiExit::Feedback));
}

#[test]
fn narrow_terminal_shows_focused_panel_overlay() {
    let (mut app, _dir) = fixture("# A\n\none\n\n## B\n\ntwo\n");
    app.focus = Focus::Outline;
    let text = draw(&mut app, 60, 24);
    assert!(text.contains("Outline"), "{text}");
    app.focus = Focus::Review;
    let text = draw(&mut app, 60, 24);
    assert!(text.contains("Review"), "{text}");
}

#[test]
fn narrow_overlay_is_clickable() {
    let (mut app, _dir) = fixture("# A\n\none\n\n## B\n\ntwo\n");
    app.focus = Focus::Outline;
    app.outline_cursor = 1;
    draw(&mut app, 60, 24);
    let area = app.layout.as_ref().unwrap().outline.unwrap();
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: area.x,
        row: area.y,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.outline_cursor, 0);
}

#[test]
fn esc_closes_a_narrow_panel_before_clearing_the_selection() {
    let (mut app, _dir) = fixture("# A\n\none\n\n## B\n\ntwo\n");
    app.handle_key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE));
    assert!(app.selection.is_some());
    app.focus = Focus::Review;
    draw(&mut app, 60, 24);
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.focus, Focus::Document);
    assert!(app.selection.is_some());
    draw(&mut app, 60, 24);
    assert!(app.layout.as_ref().unwrap().review.is_none());
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.selection.is_none());
}

#[test]
fn shift_tab_cycles_focus_backwards() {
    let (mut app, _dir) = fixture("# A\n\none\n");
    app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE));
    assert_eq!(app.focus, Focus::Review);
}

#[test]
fn panel_first_row_click_selects_first_item() {
    let (mut app, _dir) = fixture("# A\n\none\n\n## B\n\ntwo\n");
    app.focus = Focus::Outline;
    app.outline_cursor = 1;
    draw(&mut app, 80, 24);
    let area = app.layout.as_ref().unwrap().outline.unwrap();
    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: area.x,
        row: area.y,
        modifiers: KeyModifiers::NONE,
    };
    app.handle_mouse(click);
    assert_eq!(app.outline_cursor, 0);
    assert_eq!(app.focus, Focus::Outline);
}

#[test]
fn gutter_click_opens_the_item() {
    let (mut app, _dir) = fixture("# A\n\nhello world\n");
    let start = app.document.source.find("hello world").unwrap();
    let anchor = Anchor::from_range(&app.document, start..start + 11).unwrap();
    app.store.add_comment(anchor, "note");
    app.refresh_markers();
    draw(&mut app, 80, 24);
    let line = app
        .lines
        .iter()
        .position(|line| line.text.contains("hello world"))
        .unwrap();
    let layout = app.layout.clone().unwrap();
    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: layout.doc.x,
        row: layout.doc.y + line as u16,
        modifiers: KeyModifiers::NONE,
    };
    app.handle_mouse(click);
    assert_eq!(app.focus, Focus::Review);
    assert!(app.status.contains("Opened"));
}

#[test]
fn wheel_and_mouse_disabled() {
    let (mut app, _dir) = fixture("# A\n\none\n\n## B\n\ntwo\n");
    draw(&mut app, 80, 24);
    let layout = app.layout.clone().unwrap();
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: layout.doc.x,
        row: layout.doc.y,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.scroll, 3);
    draw(&mut app, 80, 24);
    assert_eq!(app.scroll, 3, "wheel scroll should survive a redraw");
    app.mouse_enabled = false;
    let before = app.cursor;
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: layout.doc.x + 2,
        row: layout.doc.y + 1,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.cursor, before);
}

#[test]
fn stale_item_reattaches_to_current_selection() {
    let (mut app, _dir) = fixture("# A\n\nold text\n\nnew target\n");
    let old_line = app
        .lines
        .iter()
        .position(|line| line.text.contains("old text"))
        .unwrap();
    app.set_cursor(old_line);
    app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));
    app.set_editor_text("move me");
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL));
    app.store.items[0].status = ItemStatus::Stale;
    app.refresh_markers();

    let target = app
        .lines
        .iter()
        .position(|line| line.text.contains("new target"))
        .unwrap();
    app.set_cursor(target);
    app.select_line();
    app.focus = Focus::Review;
    app.review_cursor = 0;
    app.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
    assert_eq!(app.store.items[0].status, ItemStatus::Open);
    assert!(app.store.items[0].anchor.quote.contains("new target"));
    assert_eq!(
        app.store.items[0].history.last().unwrap().method,
        "reattached"
    );
}

#[test]
fn selection_expands_to_whole_source_line() {
    let (mut app, _dir) = fixture("one two three four five six seven eight nine ten\n");
    app.rebuild(20);
    let continuation = app
        .lines
        .iter()
        .position(|line| line.source.start > 0)
        .unwrap();
    app.set_cursor(continuation);
    app.select_line();
    let range = app.selected_range().unwrap();
    assert_eq!(range.start, 0);
    let anchor = Anchor::from_range(&app.document, range).unwrap();
    assert!(!anchor.quote.ends_with('\n'));
    assert!(anchor.quote.contains("one two"));
}

#[test]
fn invalid_mermaid_renders_source_fallback() {
    let (mut app, _dir) = fixture("# Diagram\n\n```mermaid\nnot a diagram\n```\n");
    draw(&mut app, 100, 30);
    let key = app.media_keys.values().next().unwrap().clone();
    assert!(app.media.wait_for(&key, std::time::Duration::from_secs(10)));
    let text = draw(&mut app, 100, 30);
    assert!(text.contains("diagram unavailable"), "{text}");
    assert!(text.contains("not a diagram"), "{text}");
}

#[test]
fn mermaid_source_engine_renders_source_without_starting_a_worker() {
    let (mut app, _dir) = fixture_with_engine(
        "# Diagram\n\n```mermaid\nflowchart LR\n  A-->B\n```\n",
        MermaidEngine::Source,
    );
    let text = draw(&mut app, 100, 30);
    assert!(app.media_keys.is_empty(), "{:?}", app.media_keys);
    assert!(text.contains("Mermaid source"), "{text}");
    assert!(text.contains("flowchart LR"), "{text}");
    assert!(!text.contains("diagram unavailable"), "{text}");
}

#[test]
fn enter_toggles_only_the_mermaid_block_source() {
    let (mut app, _dir) =
        fixture("# Diagram\n\n```mermaid\nflowchart LR\n  A-->B\n```\n\nAfter.\n");
    draw(&mut app, 100, 30);
    let line = app
        .lines
        .iter()
        .position(|line| line.text.contains("mermaid diagram"))
        .unwrap();
    app.set_cursor(line);
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let text = draw(&mut app, 100, 30);
    assert!(text.contains("Mermaid source"), "{text}");
    assert!(text.contains("flowchart LR"), "{text}");
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let text = draw(&mut app, 100, 30);
    assert!(!text.contains("Mermaid source"), "{text}");
}

#[test]
fn table_header_is_bold_cyan_and_separators_are_dim() {
    let (mut app, _dir) = fixture("# Spec\n\n| Action | Vim |\n| --- | --- |\n| Move | j |\n");
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
