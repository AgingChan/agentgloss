//! Command-line interface and command dispatch.

use crate::agent;
use crate::config::{Config, Keys, Protocol, Theme};
use crate::document::{Document, content_hash};
use crate::review::export::{export_exit_code, export_json, export_prompt, status_label};
use crate::review::{ReviewStore, StoreLock, sidecar_path};
use crate::tui::{self, TuiOptions};
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const EXIT_OK: i32 = 0;
pub const EXIT_ABORTED: i32 = 1;
pub const EXIT_ERROR: i32 = 2;
pub const EXIT_TIMEOUT: i32 = 3;
pub const EXIT_FEEDBACK: i32 = 10;
pub const EXIT_TUI_APPROVED: i32 = 20;
pub const EXIT_TUI_FEEDBACK: i32 = 21;
pub const EXIT_TUI_ABORTED: i32 = 22;

#[derive(Debug, Parser)]
#[command(
    name = "mdreview",
    version,
    about = "Terminal reviewer for long Markdown specs"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Review a Markdown file in the terminal.
    Review(ReviewArgs),
    /// Mark a review as requested and print the handoff commands.
    Request {
        file: PathBuf,
        #[arg(long)]
        sidecar: Option<PathBuf>,
    },
    /// Block until the human signs off, then print the review.
    Wait(WaitArgs),
    /// Print the numbered review or JSON.
    Export(ExportArgs),
    /// Show review status.
    Status(StatusArgs),
    /// Mark review items resolved.
    Resolve {
        file: PathBuf,
        #[arg(required = true)]
        ids: Vec<String>,
        #[arg(long)]
        sidecar: Option<PathBuf>,
    },
    /// Install an agent integration recipe.
    Install(InstallArgs),
    /// Internal TUI entry point used by the tmux adapter.
    #[command(name = "__tui", hide = true)]
    Tui(ReviewArgs),
}

#[derive(Debug, Clone, Args)]
pub struct ReviewArgs {
    pub file: PathBuf,
    #[arg(long, value_enum)]
    pub keys: Option<Keys>,
    #[arg(long)]
    pub no_mouse: bool,
    #[arg(long, value_enum)]
    pub protocol: Option<Protocol>,
    #[arg(long, value_enum)]
    pub theme: Option<Theme>,
    #[arg(long)]
    pub sidecar: Option<PathBuf>,
    #[arg(long)]
    pub tmux: bool,
    #[arg(long)]
    pub detach: bool,
}

#[derive(Debug, Args)]
pub struct WaitArgs {
    pub file: PathBuf,
    #[arg(long, default_value_t = 1800)]
    pub timeout: u64,
    #[arg(long, default_value_t = 500)]
    pub interval: u64,
    #[arg(long)]
    pub sidecar: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ExportFormat {
    Prompt,
    Json,
}

#[derive(Debug, Args)]
pub struct ExportArgs {
    pub file: PathBuf,
    #[arg(long, value_enum, default_value_t = ExportFormat::Prompt)]
    pub format: ExportFormat,
    #[arg(long)]
    pub all: bool,
    #[arg(long)]
    pub sidecar: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct StatusArgs {
    pub file: PathBuf,
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub sidecar: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum InstallAgent {
    Claude,
    Codex,
    Gemini,
    Generic,
}

#[derive(Debug, Args)]
pub struct InstallArgs {
    #[arg(value_enum)]
    pub agent: InstallAgent,
    #[arg(long, default_value = ".")]
    pub dir: PathBuf,
}

pub fn run() -> i32 {
    let cli = Cli::parse();
    match dispatch(cli) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("mdreview: {error}");
            EXIT_ERROR
        }
    }
}

fn dispatch(cli: Cli) -> anyhow::Result<i32> {
    match cli.command {
        Command::Review(args) => {
            if args.tmux || args.detach {
                agent::run_tmux(&args)
            } else {
                run_review(&args, false)
            }
        }
        Command::Tui(args) => {
            let code = run_review(&args, true)?;
            Ok(match code {
                EXIT_OK => EXIT_TUI_APPROVED,
                EXIT_FEEDBACK => EXIT_TUI_FEEDBACK,
                EXIT_ABORTED => EXIT_TUI_ABORTED,
                other => other,
            })
        }
        Command::Request { file, sidecar } => run_request(&file, sidecar.as_ref()),
        Command::Wait(args) => run_wait(&args),
        Command::Export(args) => run_export(&args),
        Command::Status(args) => run_status(&args),
        Command::Resolve { file, ids, sidecar } => run_resolve(&file, &ids, sidecar.as_ref()),
        Command::Install(args) => run_install(&args),
    }
}

fn load_config() -> anyhow::Result<Config> {
    Config::load().map_err(Into::into)
}

fn resolved_sidecar(args: &ReviewArgs) -> PathBuf {
    args.sidecar
        .clone()
        .unwrap_or_else(|| sidecar_path(&args.file))
}

fn run_review(args: &ReviewArgs, internal: bool) -> anyhow::Result<i32> {
    let config = load_config()?;
    let document = Document::load(&args.file)?;
    let sidecar = resolved_sidecar(args);
    let _lock = StoreLock::acquire(&sidecar)?;
    let outcome = ReviewStore::load_or_recover(
        &sidecar,
        &document.path.display().to_string(),
        &document.hash,
    )?;
    if let Some(backup) = &outcome.backup_path {
        eprintln!(
            "mdreview: corrupt review state backed up to {}",
            backup.display()
        );
    }
    let mut store = outcome.store;
    let startup_status = outcome.backup_path.as_ref().map(|backup| {
        format!(
            "Recovered corrupt review state; backup: {}",
            backup.display()
        )
    });
    store.start_round_if_changed(&document);
    store.save(&sidecar)?;

    let options = TuiOptions {
        keys: args.keys.unwrap_or(config.keys),
        mouse: !args.no_mouse && config.mouse,
        protocol: args.protocol.unwrap_or(config.protocol),
        theme: args.theme.unwrap_or(config.theme),
        mermaid_engine: config.mermaid_engine,
        cache_dir: config.cache_dir(),
    };

    let exit = tui::run(&document, &mut store, &sidecar, &options, startup_status)?;
    store.save(&sidecar)?;

    match exit {
        tui::TuiExit::Aborted => Ok(EXIT_ABORTED),
        tui::TuiExit::Feedback => {
            if !internal {
                print!("{}", export_prompt(&store, false));
            }
            Ok(EXIT_FEEDBACK)
        }
        tui::TuiExit::Approved => {
            if !internal {
                print!("{}", export_prompt(&store, false));
            }
            Ok(EXIT_OK)
        }
    }
}

fn chosen_sidecar(file: &Path, override_path: Option<&PathBuf>) -> PathBuf {
    override_path.cloned().unwrap_or_else(|| sidecar_path(file))
}

fn run_request(file: &PathBuf, sidecar_override: Option<&PathBuf>) -> anyhow::Result<i32> {
    let document = Document::load(file)?;
    let sidecar = chosen_sidecar(file, sidecar_override);
    let _lock = StoreLock::acquire(&sidecar)?;
    let outcome = ReviewStore::load_or_recover(
        &sidecar,
        &document.path.display().to_string(),
        &document.hash,
    )?;
    if let Some(backup) = &outcome.backup_path {
        eprintln!(
            "mdreview: corrupt review state backed up to {}",
            backup.display()
        );
    }
    let mut store = outcome.store;
    store.start_round_if_changed(&document);
    store.request();
    store.save(&sidecar)?;
    println!("Review requested for {}", file.display());
    let sidecar_flag = sidecar_override
        .map(|path| format!(" --sidecar {}", path.display()))
        .unwrap_or_default();
    println!("Human: mdreview review {}{sidecar_flag}", file.display());
    println!(
        "Agent: mdreview export {}{sidecar_flag}   (or mdreview wait {}{sidecar_flag})",
        file.display(),
        file.display()
    );
    Ok(EXIT_OK)
}

fn run_wait(args: &WaitArgs) -> anyhow::Result<i32> {
    let start = Instant::now();
    let timeout = Duration::from_secs(args.timeout);
    let interval = Duration::from_millis(args.interval.max(50));
    let sidecar = chosen_sidecar(&args.file, args.sidecar.as_ref());
    loop {
        let document = Document::load(&args.file)?;
        if sidecar.exists() {
            let store = ReviewStore::load(&sidecar)?;
            if store.last_signoff.is_some() && store.doc_hash == document.hash {
                print!("{}", export_prompt(&store, false));
                return Ok(if store.is_approved() {
                    EXIT_OK
                } else {
                    EXIT_FEEDBACK
                });
            }
        }
        if args.timeout > 0 && start.elapsed() >= timeout {
            eprintln!("mdreview: review wait timed out");
            return Ok(EXIT_TIMEOUT);
        }
        std::thread::sleep(interval);
    }
}

fn run_export(args: &ExportArgs) -> anyhow::Result<i32> {
    let document = Document::load(&args.file)?;
    let sidecar = chosen_sidecar(&args.file, args.sidecar.as_ref());
    if !sidecar.exists() {
        anyhow::bail!("no review state at {}", sidecar.display());
    }
    let _lock = StoreLock::acquire(&sidecar)?;
    let store = ReviewStore::load(&sidecar)?;
    let mut view = store.clone();
    if view.doc_hash != document.hash {
        view.doc_hash = document.hash.clone();
        view.approval = None;
        view.reanchor(&document);
    }
    match args.format {
        ExportFormat::Prompt => print!("{}", export_prompt(&view, args.all)),
        ExportFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&export_json(&view, args.all))?
            );
        }
    }
    Ok(export_exit_code(&view, args.all))
}

fn run_status(args: &StatusArgs) -> anyhow::Result<i32> {
    let document = Document::load(&args.file)?;
    let sidecar = chosen_sidecar(&args.file, args.sidecar.as_ref());
    let store = if sidecar.exists() {
        ReviewStore::load(&sidecar)?
    } else {
        ReviewStore::new(args.file.display().to_string(), document.hash.clone())
    };
    let doc_changed = store.doc_hash != document.hash;
    let approval_valid = store
        .approval
        .as_ref()
        .is_some_and(|approval| approval.hash == document.hash)
        && !store.has_feedback();
    let status = if store.approval.is_some() && !approval_valid {
        "invalidated"
    } else if doc_changed {
        "changed"
    } else {
        status_label(&store)
    };
    if args.json {
        let value = serde_json::json!({
            "file": store.file,
            "round": store.round,
            "status": status,
            "open": store.open_count(),
            "stale": store.stale_count(),
            "resolved": store.resolved_count(),
            "requested_at": store.requested_at,
            "approval": store.approval,
            "approval_valid": approval_valid,
            "doc_changed": doc_changed,
        });
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!("File: {}", store.file);
        println!("Round: {}", store.round);
        println!("Status: {status}");
        println!(
            "Open: {} | Stale: {} | Resolved: {}",
            store.open_count(),
            store.stale_count(),
            store.resolved_count()
        );
        if doc_changed {
            println!("Document changed since the last review state.");
        }
        if let Some(requested) = &store.requested_at {
            println!("Requested: {requested}");
        }
        if let Some(approval) = &store.approval {
            println!(
                "Approval: {} at {} ({})",
                approval.hash,
                approval.at,
                if approval_valid {
                    "valid"
                } else {
                    "invalidated"
                }
            );
        }
    }
    Ok(EXIT_OK)
}

fn run_resolve(
    file: &Path,
    ids: &[String],
    sidecar_override: Option<&PathBuf>,
) -> anyhow::Result<i32> {
    let sidecar = chosen_sidecar(file, sidecar_override);
    if !sidecar.exists() {
        anyhow::bail!("no review state at {}", sidecar.display());
    }
    let _lock = StoreLock::acquire(&sidecar)?;
    let mut store = ReviewStore::load(&sidecar)?;
    for id in ids {
        store.resolve(id)?;
    }
    store.save(&sidecar)?;
    println!("Resolved: {}", ids.join(" "));
    Ok(EXIT_OK)
}

fn run_install(args: &InstallArgs) -> anyhow::Result<i32> {
    let written = agent::install(args.agent, &args.dir)?;
    for path in written {
        println!("Wrote {}", path.display());
    }
    Ok(EXIT_OK)
}

/// Compute the current hash for a path without retaining the parsed document.
pub fn hash_file(path: &Path) -> anyhow::Result<String> {
    let source = std::fs::read_to_string(path)?;
    Ok(content_hash(&source))
}
