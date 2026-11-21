//! Agent handoff, tmux launch, and install recipes.

use crate::cli::{EXIT_ABORTED, EXIT_FEEDBACK, EXIT_OK, InstallAgent, ReviewArgs};
use crate::document::Document;
use crate::review::export::export_prompt;
use crate::review::{ReviewStore, sidecar_path};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

const START: &str = "<!-- mdreview:start -->";
const END: &str = "<!-- mdreview:end -->";

pub fn install(agent: InstallAgent, dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dir)?;
    let mut written = Vec::new();
    match agent {
        InstallAgent::Claude => {
            let path = dir
                .join(".claude")
                .join("commands")
                .join("mdreview-review.md");
            write_file(&path, &claude_recipe())?;
            written.push(path);
        }
        InstallAgent::Codex => {
            let path = dir.join("AGENTS.md");
            upsert_block(&path, &codex_block())?;
            written.push(path);
        }
        InstallAgent::Gemini => {
            let path = dir.join("GEMINI.md");
            upsert_block(&path, &gemini_block())?;
            written.push(path);
        }
        InstallAgent::Generic => {
            let path = dir.join("mdreview-agent.md");
            write_file(&path, &generic_recipe())?;
            written.push(path);
        }
    }
    Ok(written)
}

fn write_file(path: &Path, content: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    Ok(())
}

fn upsert_block(path: &Path, block: &str) -> anyhow::Result<()> {
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let block = block.trim_end();
    let (before, after) = match (existing.find(START), existing.find(END)) {
        (Some(start), Some(end)) if end > start => (
            existing[..start].trim_end(),
            existing[end + END.len()..].trim_start_matches('\n'),
        ),
        (Some(start), None) => (existing[..start].trim_end(), ""),
        (None, Some(end)) => ("", existing[end + END.len()..].trim_start_matches('\n')),
        _ => (existing.trim_end(), ""),
    };
    let mut content = String::new();
    if !before.is_empty() {
        content.push_str(before);
        content.push('\n');
    }
    content.push_str(block);
    content.push('\n');
    if !after.trim().is_empty() {
        content.push('\n');
        content.push_str(after.trim_end());
        content.push('\n');
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    Ok(())
}

fn claude_recipe() -> String {
    String::from(
        "---\n\
description: Review a Markdown spec with mdreview\n\
---\n\
\n\
When the user asks to review a spec or plan:\n\
\n\
1. Run `mdreview request <file>`.\n\
2. Ask the user to run `mdreview review <file>` (or `mdreview review <file> --tmux` inside tmux).\n\
3. When they finish, run `mdreview export <file>` and address every open item.\n\
4. Run `mdreview resolve <file> <ids>` for fixed items.\n\
\n\
Do not edit the spec while the user is reviewing it.\n\
",
    )
}

fn generic_recipe() -> String {
    String::from(
        "# mdreview agent recipe\n\n\
When the user asks to review a Markdown spec or plan:\n\n\
1. Run `mdreview request <file>`.\n\
2. Ask the user to run `mdreview review <file>` in a terminal (or `mdreview review <file> --tmux` inside tmux).\n\
3. Wait or poll with `mdreview wait <file>`; alternatively run `mdreview export <file>` after the user signs off.\n\
4. Address every open item. Suggestions show an original and a replacement.\n\
5. Mark fixed items with `mdreview resolve <file> <ids>`.\n\
6. Do not edit the spec while the user is reviewing it.\n",
    )
}

fn codex_block() -> String {
    format!(
        "{START}\n\
## mdreview spec review\n\n\
When the user asks to review a spec or plan:\n\n\
1. Run `mdreview request <file>`.\n\
2. Ask the user to run `mdreview review <file>` (or `mdreview review <file> --tmux` inside tmux).\n\
3. Run `mdreview export <file>` after signoff and address every open item.\n\
4. Run `mdreview resolve <file> <ids>` for fixed items.\n\
5. Do not edit the spec while the user is reviewing it.\n\
{END}\n"
    )
}

fn gemini_block() -> String {
    format!(
        "{START}\n\
## mdreview spec review\n\n\
When the user asks to review a spec or plan:\n\n\
1. Run `mdreview request <file>`.\n\
2. Ask the user to run `mdreview review <file>` (or `mdreview review <file> --tmux` inside tmux).\n\
3. Run `mdreview export <file>` after signoff and address every open item.\n\
4. Run `mdreview resolve <file> <ids>` for fixed items.\n\
5. Do not edit the spec while the user is reviewing it.\n\
{END}\n"
    )
}

pub fn run_tmux(args: &ReviewArgs) -> anyhow::Result<i32> {
    if std::env::var_os("TMUX").is_none() {
        anyhow::bail!(
            "not inside tmux; run `mdreview review {}` in your terminal instead",
            args.file.display()
        );
    }
    let command = tui_command(args)?;
    if args.detach {
        let status = Command::new("tmux")
            .args(["new-window", "-d", "-n", "mdreview", &command])
            .status()
            .map_err(|error| {
                anyhow::anyhow!(
                    "cannot run tmux: {error}; run `mdreview review {}` in your terminal instead",
                    args.file.display()
                )
            })?;
        if !status.success() {
            anyhow::bail!(
                "tmux new-window failed; run `mdreview review {}` in your terminal instead",
                args.file.display()
            );
        }
        println!("Opened mdreview in a detached tmux window.");
        if let Some(sidecar) = &args.sidecar {
            println!(
                "Agent: mdreview wait {} --sidecar {}",
                args.file.display(),
                sidecar.display()
            );
        } else {
            println!("Agent: mdreview wait {}", args.file.display());
        }
        return Ok(EXIT_OK);
    }

    let title = format!("mdreview: {}", args.file.display());
    let sidecar = review_sidecar(args);
    let status = Command::new("tmux")
        .args([
            "display-popup",
            "-E",
            "-w",
            "90%",
            "-h",
            "90%",
            "-T",
            &title,
            &command,
        ])
        .status()
        .map_err(|error| {
            anyhow::anyhow!(
                "cannot run tmux display-popup: {error}; run `mdreview review {}` in your terminal instead",
                args.file.display()
            )
        })?;
    let code = status.code().unwrap_or(-1);
    if !matches!(code, 0 | 1 | 10) {
        anyhow::bail!(
            "tmux display-popup failed (status {code}); run `mdreview review {}` in your terminal instead",
            args.file.display()
        );
    }
    if code == EXIT_ABORTED {
        return Ok(EXIT_ABORTED);
    }
    print_review_result(&args.file, &sidecar)
}

fn review_sidecar(args: &ReviewArgs) -> PathBuf {
    args.sidecar
        .clone()
        .unwrap_or_else(|| sidecar_path(&args.file))
}

fn tui_command(args: &ReviewArgs) -> anyhow::Result<String> {
    let exe = std::env::current_exe()?;
    let mut parts: Vec<String> = vec![
        shell_quote(exe.as_os_str()),
        "__tui".to_string(),
        shell_quote(args.file.as_os_str()),
    ];
    if let Some(keys) = args.keys {
        parts.push("--keys".to_string());
        parts.push(format!("{keys:?}").to_lowercase());
    }
    if args.no_mouse {
        parts.push("--no-mouse".to_string());
    }
    if let Some(protocol) = args.protocol {
        parts.push("--protocol".to_string());
        parts.push(format!("{protocol:?}").to_lowercase());
    }
    if let Some(theme) = args.theme {
        parts.push("--theme".to_string());
        parts.push(format!("{theme:?}").to_lowercase());
    }
    if let Some(sidecar) = &args.sidecar {
        parts.push("--sidecar".to_string());
        parts.push(shell_quote(sidecar.as_os_str()));
    }
    Ok(parts.join(" "))
}

fn shell_quote(value: &OsStr) -> String {
    let text = value.to_string_lossy();
    format!("'{}'", text.replace('\'', "'\\''"))
}

fn print_review_result(file: &Path, sidecar: &Path) -> anyhow::Result<i32> {
    let document = Document::load(file)?;
    if !sidecar.exists() {
        anyhow::bail!("review TUI exited without saving state");
    }
    let mut store = ReviewStore::load(sidecar)?;
    store.start_round_if_changed(&document);
    if store.last_signoff.is_none() {
        return Ok(EXIT_ABORTED);
    }
    print!("{}", export_prompt(&store, false));
    if store.is_approved() {
        Ok(EXIT_OK)
    } else if store.has_feedback() {
        Ok(EXIT_FEEDBACK)
    } else {
        Ok(EXIT_ABORTED)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installs_claude_recipe() {
        let dir = tempfile::tempdir().unwrap();
        let written = install(InstallAgent::Claude, dir.path()).unwrap();
        let text = std::fs::read_to_string(&written[0]).unwrap();
        assert!(text.contains("mdreview request"));
        assert!(text.contains("mdreview export"));
        assert!(text.contains("mdreview review"));
    }

    #[test]
    fn installs_blocks_idempotently() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("AGENTS.md");
        std::fs::write(&path, "# Existing\n\nKeep me.\n").unwrap();
        install(InstallAgent::Codex, dir.path()).unwrap();
        let once = std::fs::read_to_string(&path).unwrap();
        install(InstallAgent::Codex, dir.path()).unwrap();
        let twice = std::fs::read_to_string(&path).unwrap();
        assert_eq!(once, twice);
        assert_eq!(twice.matches(START).count(), 1);
        assert!(twice.contains("Keep me."));
    }

    #[test]
    fn generic_recipe_mentions_all_handoff_steps() {
        let dir = tempfile::tempdir().unwrap();
        let written = install(InstallAgent::Generic, dir.path()).unwrap();
        let text = std::fs::read_to_string(&written[0]).unwrap();
        assert!(text.contains("mdreview request"));
        assert!(text.contains("mdreview wait"));
        assert!(text.contains("mdreview resolve"));
    }

    #[test]
    fn shell_quotes_spaces_and_quotes() {
        assert_eq!(shell_quote(OsStr::new("a b")), "'a b'");
        assert_eq!(shell_quote(OsStr::new("it's")), "'it'\\''s'");
    }
}
