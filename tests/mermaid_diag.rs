use agentgloss::config::{Keys, MermaidEngine, Protocol, Theme};
use agentgloss::document::media_cell_size;
use agentgloss::media::MediaManager;
use agentgloss::tui::TuiOptions;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui_image::sliced::{SignedPosition, SlicedImage};
use std::path::{Path, PathBuf};
use std::time::Duration;

fn fixtures() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mermaid");
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "mmd"))
        .collect();
    paths.sort();
    paths
}

#[test]
#[ignore = "diagnostic; run with --ignored --nocapture"]
fn dump_mermaid_blocks() {
    let out = Path::new("/tmp/agentgloss-mermaid");
    std::fs::create_dir_all(out).unwrap();
    let cache = tempfile::tempdir().unwrap();
    let options = TuiOptions {
        keys: Keys::Vim,
        mouse: false,
        protocol: Protocol::Halfblocks,
        theme: Theme::Dark,
        mermaid_engine: MermaidEngine::Rust,
        cache_dir: cache.path().join("cache"),
    };
    let mut media = MediaManager::new(&options);
    media.configure(&options);
    println!("protocol={} font fallback=10x20", media.protocol_name());

    for path in fixtures() {
        let source = std::fs::read_to_string(&path).unwrap();
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        for width in [40u16, 80, 120] {
            let page_height = 20u16;
            let aspect = media.mermaid_aspect(&source).unwrap_or(1.0);
            let (cols, rows) = media_cell_size(aspect, width as usize, page_height as usize, 10);
            let key = media.ensure_mermaid(&source, (cols as u16, rows as u16));
            if !media.wait_for(&key, Duration::from_secs(30)) {
                println!("{stem} {width}: TIMEOUT");
                continue;
            }
            if let Some(error) = media.error(&key) {
                println!("{stem} {width}: ERROR {error}");
                continue;
            }
            let protocol = media.get(&key).unwrap();
            let backend = TestBackend::new(width, page_height);
            let mut terminal = Terminal::new(backend).unwrap();
            let x = ((width as usize - cols) / 2) as i16;
            terminal
                .draw(|frame| {
                    frame.render_widget(
                        SlicedImage::new(protocol, SignedPosition::from((x, 0))),
                        frame.area(),
                    );
                })
                .unwrap();
            let buffer = terminal.backend().buffer();
            let mut text = String::new();
            let mut ink = 0usize;
            for y in 0..page_height {
                for x in 0..width {
                    let symbol = buffer[(x, y)].symbol();
                    text.push_str(symbol);
                    if !symbol.is_empty() && symbol != " " {
                        ink += 1;
                    }
                }
                text.push('\n');
            }
            let file = out.join(format!("{stem}-{width}.txt"));
            std::fs::write(&file, &text).unwrap();
            println!(
                "{stem} {width}: aspect={aspect:.2} cols={cols} rows={rows} ink={ink} -> {}",
                file.display()
            );
        }
    }
}
