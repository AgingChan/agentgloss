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
    let mut markers = Vec::new();
    markers.extend(
        find_markers(&existing, START)
            .into_iter()
            .map(|at| (at, true)),
    );
    markers.extend(
        find_markers(&existing, END)
            .into_iter()
            .map(|at| (at, false)),
    );
    markers.sort_by_key(|(at, _)| *at);

    let mut pairs = Vec::new();
    let mut open = None;
    for (at, is_start) in markers {
        if is_start {
            if open.is_some() {
                anyhow::bail!(
                    "malformed agentgloss markers in {} (nested start marker)",
                    path.display()
                );
            }
            open = Some(at);
        } else {
            let Some(start) = open.take() else {
                anyhow::bail!(
                    "malformed agentgloss markers in {} (end marker without a start)",
                    path.display()
                );
            };
            pairs.push((start, at + END.len()));
        }
    }
    if open.is_some() {
        anyhow::bail!(
            "malformed agentgloss markers in {} (start marker without an end)",
            path.display()
        );
    }

    let (before, between, after) = if pairs.is_empty() {
        (
            existing.trim_end().to_string(),
            String::new(),
            String::new(),
        )
    } else {
        let first = pairs[0].0;
        let last = pairs[pairs.len() - 1].1;
        let mut between = String::new();
        for window in pairs.windows(2) {
            let content = existing[window[0].1..window[1].0].trim();
            if !content.is_empty() {
                if !between.is_empty() {
                    between.push('\n');
                }
                between.push_str(content);
            }
        }
        (
            existing[..first].trim_end().to_string(),
            between,
            existing[last..].trim_start_matches('\n').to_string(),
        )
    };

    let mut content = String::new();
    if !before.is_empty() {
        content.push_str(&before);
        content.push('\n');
    }
    content.push_str(block);
    content.push('\n');
    if !between.is_empty() {
        content.push('\n');
        content.push_str(&between);
        content.push('\n');
    }
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

fn find_markers(text: &str, marker: &str) -> Vec<usize> {
    let mut positions = Vec::new();
    let mut start = 0;
    while let Some(found) = text[start..].find(marker) {
        let at = start + found;
        positions.push(at);
        start = at + marker.len();
    }
    positions
}

fn claude_recipe() -> String {
    String::from(
        "---\n\
description: Review a Markdown spec with AgentGloss\n\
---\n\
\n\
When the user asks to review a spec or plan:\n\
\n\
1. Run `agloss request <file>`.\n\
2. Ask the user to run `agloss review <file>` (or `agloss review <file> --tmux` inside tmux).\n\
3. When they finish, run `agloss export <file>` and address every open item.\n\
4. Run `agloss resolve <file> <ids>` for fixed items.\n\
\n\
Do not edit the spec while the user is reviewing it.\n\
",
    )
}

fn generic_recipe() -> String {
    String::from(
        "# AgentGloss agent recipe\n\n\
When the user asks to review a Markdown spec or plan:\n\n\
1. Run `agloss request <file>`.\n\
2. Ask the user to run `agloss review <file>` in a terminal (or `agloss review <file> --tmux` inside tmux).\n\
3. Wait or poll with `agloss wait <file>`; alternatively run `agloss export <file>` after the user signs off.\n\
4. Address every open item. Suggestions show an original and a replacement.\n\
5. Mark fixed items with `agloss resolve <file> <ids>`.\n\
6. Do not edit the spec while the user is reviewing it.\n",
    )
}

fn codex_block() -> String {
    format!(
        "{START}\n\
## AgentGloss spec review\n\n\
When the user asks to review a spec or plan:\n\n\
1. Run `agloss request <file>`.\n\
2. Ask the user to run `agloss review <file>` (or `agloss review <file> --tmux` inside tmux).\n\
3. Run `agloss export <file>` after signoff and address every open item.\n\
4. Run `agloss resolve <file> <ids>` for fixed items.\n\
5. Do not edit the spec while the user is reviewing it.\n\
{END}\n"
    )
}

fn gemini_block() -> String {
    format!(
        "{START}\n\
## AgentGloss spec review\n\n\
When the user asks to review a spec or plan:\n\n\
1. Run `agloss request <file>`.\n\
2. Ask the user to run `agloss review <file>` (or `agloss review <file> --tmux` inside tmux).\n\
3. Run `agloss export <file>` after signoff and address every open item.\n\
4. Run `agloss resolve <file> <ids>` for fixed items.\n\
5. Do not edit the spec while the user is reviewing it.\n\
{END}\n"
    )
}

pub fn run_tmux(args: &ReviewArgs) -> anyhow::Result<i32> {
    if std::env::var_os("TMUX").is_none() {
        anyhow::bail!(
            "not inside tmux; run `agloss review {}` in your terminal instead",
            args.file.display()
        );
    }
    let command = tui_command(args)?;
    if args.detach {
        let status = Command::new("tmux")
            .args(["new-window", "-d", "-n", "agloss", &command])
            .status()
            .map_err(|error| {
                anyhow::anyhow!(
                    "cannot run tmux: {error}; run `agloss review {}` in your terminal instead",
                    args.file.display()
                )
            })?;
        if !status.success() {
            anyhow::bail!(
                "tmux new-window failed; run `agloss review {}` in your terminal instead",
                args.file.display()
            );
        }
        println!("Opened AgentGloss in a detached tmux window.");
        if let Some(sidecar) = &args.sidecar {
            println!(
                "Agent: agloss wait {} --sidecar {}",
                display_path(&args.file),
                display_path(sidecar)
            );
        } else {
            println!("Agent: agloss wait {}", display_path(&args.file));
        }
        return Ok(EXIT_OK);
    }

    let title = format!("agloss: {}", args.file.display());
    let sidecar = review_sidecar(args);
    let result_file = tui_result_path();
    let popup_command = format!(
        "AGENTGLOSS_RESULT_FILE={} {}",
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
        .env("AGENTGLOSS_RESULT_FILE", &result_file)
        .status()
    {
        Ok(status) => status,
        Err(error) => {
            let _ = std::fs::remove_file(&result_file);
            anyhow::bail!(
                "cannot run tmux display-popup: {error}; run `agloss review {}` in your terminal instead",
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
            "tmux display-popup failed (status {code}); run `agloss review {}` in your terminal instead",
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
        assert!(text.contains("agloss request"));
        assert!(text.contains("agloss export"));
        assert!(text.contains("agloss review"));
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
        assert!(text.contains("agloss request"));
        assert!(text.contains("agloss wait"));
        assert!(text.contains("agloss resolve"));
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
    fn orphan_marker_blocks_are_refused_without_rewriting() {
        let dir = tempfile::tempdir().unwrap();
        let start_only = dir.path().join("AGENTS.md");
        let start_text = "# Keep\n\n<!-- agentgloss:start -->\norphaned text\n";
        std::fs::write(&start_only, start_text).unwrap();
        assert!(install(InstallAgent::Codex, dir.path()).is_err());
        assert_eq!(std::fs::read_to_string(&start_only).unwrap(), start_text);

        let end_only = dir.path().join("GEMINI.md");
        let end_text = "Notes before\n\n<!-- agentgloss:end -->\n\n# Keep\n";
        std::fs::write(&end_only, end_text).unwrap();
        assert!(install(InstallAgent::Gemini, dir.path()).is_err());
        assert_eq!(std::fs::read_to_string(&end_only).unwrap(), end_text);
    }

    #[test]
    fn duplicate_complete_marker_blocks_are_collapsed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("AGENTS.md");
        std::fs::write(
            &path,
            "# Keep\n\n<!-- agentgloss:start -->\none\n<!-- agentgloss:end -->\n\nMiddle\n\n<!-- agentgloss:start -->\ntwo\n<!-- agentgloss:end -->\n\nTail\n",
        )
        .unwrap();
        install(InstallAgent::Codex, dir.path()).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.matches(START).count(), 1, "{text}");
        assert_eq!(text.matches(END).count(), 1, "{text}");
        assert!(text.contains("# Keep"));
        assert!(text.contains("Middle"));
        assert!(text.contains("Tail"));
        assert!(!text.contains("\none\n"));
        assert!(!text.contains("\ntwo\n"));
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
