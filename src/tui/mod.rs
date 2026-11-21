//! Terminal UI entry point and event loop.

pub mod app;
pub mod render;

pub use app::{App, InputMode, TuiExit, TuiOptions, ViewMode};

use crate::document::Document;
use crate::review::ReviewStore;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::stdout;
use std::path::Path;
use std::time::{Duration, Instant};

pub fn run(
    document: &Document,
    store: &mut ReviewStore,
    sidecar: &Path,
    options: &TuiOptions,
    startup_status: Option<String>,
) -> anyhow::Result<TuiExit> {
    let mut app = App::new(
        document.clone(),
        store.clone(),
        sidecar.to_path_buf(),
        options.clone(),
    );
    if let Some(status) = startup_status {
        app.status = status;
    }
    let mut session = TerminalSession::new(options.mouse)?;
    app.media.configure(options);
    let result = event_loop(&mut session.terminal, &mut app);
    drop(session);
    *store = app.store;
    if let Err(error) = result {
        // A closed terminal or a failed draw is an aborted review, not a
        // command failure: the caller still saves the edited review state.
        eprintln!("mdreview: terminal session ended: {error}");
    }
    Ok(app.exit.unwrap_or(TuiExit::Aborted))
}

struct TerminalSession {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    mouse: bool,
}

impl TerminalSession {
    fn new(mouse: bool) -> anyhow::Result<Self> {
        enable_raw_mode()?;
        let mut out = stdout();
        if let Err(error) = execute!(out, EnterAlternateScreen) {
            let _ = disable_raw_mode();
            return Err(error.into());
        }
        if mouse && let Err(error) = execute!(out, EnableMouseCapture) {
            let _ = execute!(out, LeaveAlternateScreen);
            let _ = disable_raw_mode();
            return Err(error.into());
        }
        let terminal = match Terminal::new(CrosstermBackend::new(out)) {
            Ok(terminal) => terminal,
            Err(error) => {
                let _ = execute!(stdout(), LeaveAlternateScreen);
                let _ = disable_raw_mode();
                return Err(error.into());
            }
        };
        Ok(Self { terminal, mouse })
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let mut out = stdout();
        if self.mouse {
            let _ = execute!(out, DisableMouseCapture);
        }
        let _ = execute!(out, LeaveAlternateScreen);
        let _ = disable_raw_mode();
        let _ = self.terminal.show_cursor();
    }
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
) -> anyhow::Result<()> {
    let mut last_check = Instant::now();
    loop {
        terminal.draw(|frame| render::draw(frame, app))?;
        if app.should_quit {
            break;
        }
        if crossterm::event::poll(Duration::from_millis(100))? {
            match crossterm::event::read()? {
                crossterm::event::Event::Key(key) => app.handle_key(key),
                crossterm::event::Event::Mouse(mouse) => app.handle_mouse(mouse),
                crossterm::event::Event::Resize(_, _) => {
                    // The next draw recomputes the document width.
                    app.rebuild(app.layout_width());
                }
                _ => {}
            }
        }
        if last_check.elapsed() >= Duration::from_millis(500) {
            last_check = Instant::now();
            app.check_file_change();
        }
    }
    Ok(())
}
