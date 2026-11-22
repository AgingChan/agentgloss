//! Agent handoff, tmux launch, and install recipes.

use crate::cli::{
    EXIT_ABORTED, EXIT_FEEDBACK, EXIT_OK, EXIT_TUI_ABORTED, EXIT_TUI_APPROVED, EXIT_TUI_FEEDBACK,
    InstallAgent, ReviewArgs,
};
use crate::document::Document;
use crate::review::export::export_prompt;
use crate::review::{ReviewStore, sidecar_path};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

const START: &str = "<!-- agentgloss:start -->";
const END: &str = "<!-- agentgloss:end -->";

pub fn install(agent: InstallAgent, dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dir)?;
    let mut written = Vec::new();
    match agent {
        InstallAgent::Claude => {
            let path = dir
                .join(".claude")
                .join("commands")
                .join("agentgloss-review.md");
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
            let path = dir.join("agentgloss-agent.md");
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
    let existing = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(anyhow::anyhow!(
                "cannot read {} before updating it: {error}",
                path.display()
            ));
        }
    };
    let block = block.trim_end();
    let (before, after) = match (existing.find(START), existing.find(END)) {
        (Some(start), Some(end)) if end > start => (
            existing[..start].trim_end(),
            existing[end + END.len()..].trim_start_matches('\n'),
        ),
        (Some(start), None) => (existing[..start].trim_end(), ""),
        (None, Some(end)) => (
            existing[..end].trim_end(),
            existing[end + END.len()..].trim_start_matches('\n'),
        ),
        (Some(_), Some(_)) => {
            anyhow::bail!(
                "malformed agentgloss markers in {} (end appears before start)",
                path.display()
            );
        }
        (None, None) => (existing.trim_end(), ""),
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
description: Review a Markdown spec with agentgloss\n\
---\n\
\n\
When the user asks to review a spec or plan:\n\
\n\
1. Run `agentgloss request <file>`.\n\
2. Ask the user to run `agentgloss review <file>` (or `agentgloss review <file> --tmux` inside tmux).\n\
3. When they finish, run `agentgloss export <file>` and address every open item.\n\
4. Run `agentgloss resolve <file> <ids>` for fixed items.\n\
\n\
Do not edit the spec while the user is reviewing it.\n\
",
    )
}

fn generic_recipe() -> String {
    String::from(
        "# agentgloss agent recipe\n\n\
When the user asks to review a Markdown spec or plan:\n\n\
1. Run `agentgloss request <file>`.\n\
2. Ask the user to run `agentgloss review <file>` in a terminal (or `agentgloss review <file> --tmux` inside tmux).\n\
3. Wait or poll with `agentgloss wait <file>`; alternatively run `agentgloss export <file>` after the user signs off.\n\
4. Address every open item. Suggestions show an original and a replacement.\n\
5. Mark fixed items with `agentgloss resolve <file> <ids>`.\n\
6. Do not edit the spec while the user is reviewing it.\n",
    )
}

fn codex_block() -> String {
    format!(
        "{START}\n\
## agentgloss spec review\n\n\
When the user asks to review a spec or plan:\n\n\
1. Run `agentgloss request <file>`.\n\
2. Ask the user to run `agentgloss review <file>` (or `agentgloss review <file> --tmux` inside tmux).\n\
3. Run `agentgloss export <file>` after signoff and address every open item.\n\
4. Run `agentgloss resolve <file> <ids>` for fixed items.\n\
5. Do not edit the spec while the user is reviewing it.\n\
{END}\n"
    )
}

fn gemini_block() -> String {
    format!(
        "{START}\n\
## agentgloss spec review\n\n\
When the user asks to review a spec or plan:\n\n\
1. Run `agentgloss request <file>`.\n\
2. Ask the user to run `agentgloss review <file>` (or `agentgloss review <file> --tmux` inside tmux).\n\
3. Run `agentgloss export <file>` after signoff and address every open item.\n\
4. Run `agentgloss resolve <file> <ids>` for fixed items.\n\
5. Do not edit the spec while the user is reviewing it.\n\
{END}\n"
    )
}

pub fn run_tmux(args: &ReviewArgs) -> anyhow::Result<i32> {
    if std::env::var_os("TMUX").is_none() {
        anyhow::bail!(
            "not inside tmux; run `agentgloss review {}` in your terminal instead",
            args.file.display()
        );
    }
    let command = tui_command(args)?;
    if args.detach {
        let status = Command::new("tmux")
            .args(["new-window", "-d", "-n", "agentgloss", &command])
            .status()
            .map_err(|error| {
                anyhow::anyhow!(
                    "cannot run tmux: {error}; run `agentgloss review {}` in your terminal instead",
                    args.file.display()
                )
            })?;
        if !status.success() {
            anyhow::bail!(
                "tmux new-window failed; run `agentgloss review {}` in your terminal instead",
                args.file.display()
            );
        }
        println!("Opened agentgloss in a detached tmux window.");
        if let Some(sidecar) = &args.sidecar {
            println!(
                "Agent: agentgloss wait {} --sidecar {}",
                display_path(&args.file),
                display_path(sidecar)
            );
        } else {
            println!("Agent: agentgloss wait {}", display_path(&args.file));
        }
        return Ok(EXIT_OK);
    }

    let title = format!("agentgloss: {}", args.file.display());
    let sidecar = review_sidecar(args);
    let result_file = tui_result_path();
    let popup_command = format!(
        "MDREVIEW_RESULT_FILE={} {}",
        shell_quote(result_file.as_os_str()),
        command
    );
    let status = match Command::new("tmux")
        .args([
            "display-popup",
            "-E",
            "-w",
            "90%",
            "-h",
            "90%",
            "-T",
            &title,
            &popup_command,
        ])
        .env("MDREVIEW_RESULT_FILE", &result_file)
        .status()
    {
        Ok(status) => status,
        Err(error) => {
            let _ = std::fs::remove_file(&result_file);
            anyhow::bail!(
                "cannot run tmux display-popup: {error}; run `agentgloss review {}` in your terminal instead",
                args.file.display()
            );
        }
    };
    let file_code = std::fs::read_to_string(&result_file)
        .ok()
        .and_then(|text| text.trim().parse::<i32>().ok());
    let _ = std::fs::remove_file(&result_file);
    let code = file_code.unwrap_or_else(|| status.code().unwrap_or(-1));
    if !matches!(
        code,
        EXIT_TUI_APPROVED | EXIT_TUI_FEEDBACK | EXIT_TUI_ABORTED
    ) {
        anyhow::bail!(
            "tmux display-popup failed (status {code}); run `agentgloss review {}` in your terminal instead",
            args.file.display()
        );
    }
    if code == EXIT_TUI_ABORTED {
        return Ok(EXIT_ABORTED);
    }
    print_review_result(&args.file, &sidecar)
}

fn review_sidecar(args: &ReviewArgs) -> PathBuf {
    args.sidecar
        .clone()
        .unwrap_or_else(|| sidecar_path(&args.file))
}

fn tui_result_path() -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "agentgloss-tui-result-{}-{stamp}.txt",
        std::process::id()
    ))
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

pub(crate) fn display_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    if text
        .chars()
        .any(|ch| ch.is_whitespace() || "'\"\\$`&;|<>()*?[]{}!#".contains(ch))
    {
        shell_quote(path.as_os_str())
    } else {
        text.into_owned()
    }
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
    let default_sidecar = sidecar_path(file);
    let custom_sidecar = (sidecar != default_sidecar).then_some(sidecar);
    print!("{}", export_prompt(&store, false, custom_sidecar));
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
        assert!(text.contains("agentgloss request"));
        assert!(text.contains("agentgloss export"));
        assert!(text.contains("agentgloss review"));
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
        assert!(text.contains("agentgloss request"));
        assert!(text.contains("agentgloss wait"));
        assert!(text.contains("agentgloss resolve"));
    }

    #[test]
    fn gemini_and_generic_installs_are_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let gemini = dir.path().join("GEMINI.md");
        std::fs::write(&gemini, "# Gemini notes\n\nKeep me.\n").unwrap();
        install(InstallAgent::Gemini, dir.path()).unwrap();
        let once = std::fs::read_to_string(&gemini).unwrap();
        install(InstallAgent::Gemini, dir.path()).unwrap();
        let twice = std::fs::read_to_string(&gemini).unwrap();
        assert_eq!(once, twice);
        assert!(twice.contains("Keep me."));
        assert_eq!(twice.matches(START).count(), 1);

        let generic_path = dir.path().join("agentgloss-agent.md");
        install(InstallAgent::Generic, dir.path()).unwrap();
        let once = std::fs::read_to_string(&generic_path).unwrap();
        install(InstallAgent::Generic, dir.path()).unwrap();
        let twice = std::fs::read_to_string(&generic_path).unwrap();
        assert_eq!(once, twice);
    }

    #[test]
    fn repairs_partial_marker_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let start_only = dir.path().join("AGENTS.md");
        std::fs::write(
            &start_only,
            "# Keep\n\n<!-- agentgloss:start -->\norphaned text\n",
        )
        .unwrap();
        install(InstallAgent::Codex, dir.path()).unwrap();
        let text = std::fs::read_to_string(&start_only).unwrap();
        assert!(text.contains("# Keep"));
        assert!(!text.contains("orphaned text"), "{text}");
        assert_eq!(text.matches(START).count(), 1);
        assert_eq!(text.matches(END).count(), 1);

        let end_only = dir.path().join("GEMINI.md");
        std::fs::write(
            &end_only,
            "Notes before\n\n<!-- agentgloss:end -->\n\n# Keep\n",
        )
        .unwrap();
        install(InstallAgent::Gemini, dir.path()).unwrap();
        let text = std::fs::read_to_string(&end_only).unwrap();
        assert!(text.contains("Notes before"), "{text}");
        assert!(text.contains("# Keep"));
        assert_eq!(text.matches(START).count(), 1);
        assert_eq!(text.matches(END).count(), 1);
    }

    #[test]
    fn malformed_markers_and_unreadable_files_are_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let malformed = dir.path().join("AGENTS.md");
        let before = "<!-- agentgloss:end -->\nkeep\n<!-- agentgloss:start -->\n";
        std::fs::write(&malformed, before).unwrap();
        assert!(install(InstallAgent::Codex, dir.path()).is_err());
        assert_eq!(std::fs::read_to_string(&malformed).unwrap(), before);

        let binary = dir.path().join("GEMINI.md");
        std::fs::write(&binary, [0xff, 0xfe, 0x00]).unwrap();
        assert!(install(InstallAgent::Gemini, dir.path()).is_err());
        assert_eq!(std::fs::read(&binary).unwrap(), vec![0xff, 0xfe, 0x00]);
    }

    #[test]
    fn legacy_mdreview_markers_are_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("AGENTS.md");
        let legacy = "<!-- mdreview:start -->\nold\n<!-- mdreview:end -->\n";
        std::fs::write(&path, legacy).unwrap();
        install(InstallAgent::Codex, dir.path()).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(legacy), "{text}");
        assert_eq!(text.matches(START).count(), 1);
        assert_eq!(text.matches(END).count(), 1);
    }

    #[test]
    fn shell_quotes_spaces_and_quotes() {
        assert_eq!(shell_quote(OsStr::new("a b")), "'a b'");
        assert_eq!(shell_quote(OsStr::new("it's")), "'it'\\''s'");
        assert_eq!(display_path(Path::new("a b.md")), "'a b.md'");
        assert_eq!(display_path(Path::new("plain.md")), "plain.md");
    }
}
