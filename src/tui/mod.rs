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
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
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
    let interrupted = Arc::new(AtomicBool::new(false));
    let _signals = SignalGuard::new(interrupted.clone());
    let previous_hook: Arc<dyn Fn(&std::panic::PanicHookInfo<'_>) + Send + Sync> =
        Arc::from(std::panic::take_hook());
    let hook = previous_hook.clone();
    let mouse = options.mouse;
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal(mouse);
        hook(info);
    }));
    let mut session = TerminalSession::new(options.mouse)?;
    let result = event_loop(&mut session.terminal, &mut app, &interrupted);
    drop(session);
    std::panic::set_hook(Box::new(move |info| previous_hook(info)));
    *store = app.store;
    if let Err(error) = result {
        // A closed terminal, a signal, or a failed draw is an aborted review,
        // not a command failure: the caller still saves the review state.
        eprintln!("agentgloss: terminal session ended: {error}");
    }
    Ok(app.exit.unwrap_or(TuiExit::Aborted))
}

#[cfg(unix)]
struct SignalGuard {
    ids: Vec<signal_hook::SigId>,
}

#[cfg(unix)]
impl SignalGuard {
    fn new(flag: Arc<AtomicBool>) -> Self {
        use signal_hook::consts::{SIGHUP, SIGTERM};
        let mut ids = Vec::new();
        for signal in [SIGHUP, SIGTERM] {
            if let Ok(id) = signal_hook::flag::register(signal, flag.clone()) {
                ids.push(id);
            }
        }
        Self { ids }
    }
}

#[cfg(unix)]
impl Drop for SignalGuard {
    fn drop(&mut self) {
        for id in &self.ids {
            signal_hook::low_level::unregister(*id);
        }
    }
}

#[cfg(not(unix))]
struct SignalGuard;

#[cfg(not(unix))]
impl SignalGuard {
    fn new(_flag: Arc<AtomicBool>) -> Self {
        Self
    }
}

fn restore_terminal(mouse: bool) {
    let mut out = stdout();
    if mouse {
        let _ = execute!(out, DisableMouseCapture);
    }
    let _ = execute!(out, LeaveAlternateScreen);
    let _ = disable_raw_mode();
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
                restore_terminal(mouse);
                return Err(error.into());
            }
        };
        Ok(Self { terminal, mouse })
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        restore_terminal(self.mouse);
        let _ = self.terminal.show_cursor();
    }
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
    interrupted: &AtomicBool,
) -> anyhow::Result<()> {
    let mut last_check = Instant::now();
    loop {
        if interrupted.load(Ordering::Relaxed) {
            app.exit = Some(TuiExit::Aborted);
            app.should_quit = true;
            return Ok(());
        }
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
