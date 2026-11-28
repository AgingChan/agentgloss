use agentgloss::document::Document;
use agentgloss::review::{Anchor, ReviewStore, StoreLock, sidecar_path};
use std::path::Path;
use std::process::{Command, Output};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_agloss")
}

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(bin())
        .args(args)
        .current_dir(dir)
        .env_remove("TMUX")
        .output()
        .expect("run agloss")
}

fn write_spec(dir: &Path) -> std::path::PathBuf {
    let path = dir.join("spec.md");
    std::fs::write(
        &path,
        "# Plan\n\nHello world.\n\nSecond paragraph with more text.\n",
    )
    .unwrap();
    path
}

fn seed_comment(path: &Path, body: &str) -> String {
    let document = Document::load(path).unwrap();
    let start = document.source.find("Hello world.").unwrap();
    let anchor = Anchor::from_range(&document, start..start + "Hello world.".len()).unwrap();
    let sidecar = sidecar_path(path);
    let mut store =
        ReviewStore::load_or_recover(&sidecar, &path.display().to_string(), &document.hash)
            .unwrap()
            .store;
    let id = store.add_comment(anchor, body);
    store.save(&sidecar).unwrap();
    id
}

#[test]
fn version_and_help() {
    let dir = tempfile::tempdir().unwrap();
    let output = run(dir.path(), &["--version"]);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("agloss"));
    let help = run(dir.path(), &["--help"]);
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("request"));
}

#[test]
fn agentgloss_alias_still_works() {
    let output = Command::new(env!("CARGO_BIN_EXE_agentgloss"))
        .arg("--version")
        .env_remove("TMUX")
        .output()
        .expect("run agentgloss alias");
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("agloss"));
}

#[test]
fn request_status_export_resolve_flow() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_spec(dir.path());

    let request = run(dir.path(), &["request", "spec.md"]);
    assert!(request.status.success(), "{request:?}");
    assert!(String::from_utf8_lossy(&request.stdout).contains("agloss review spec.md"));
    assert!(sidecar_path(&path).exists());

    let status = run(dir.path(), &["status", "spec.md", "--json"]);
    assert!(status.status.success());
    let value: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(value["round"], 1);
    assert_eq!(value["open"], 0);

    let id = seed_comment(&path, "Please clarify this.");
    let export = run(dir.path(), &["export", "spec.md"]);
    assert_eq!(export.status.code(), Some(10), "{export:?}");
    let prompt = String::from_utf8_lossy(&export.stdout);
    assert!(prompt.contains("agloss resolve"));
    assert!(prompt.contains(&id));
    assert!(prompt.contains("Please clarify this."));

    let json_export = run(dir.path(), &["export", "spec.md", "--format", "json"]);
    assert_eq!(json_export.status.code(), Some(10));
    let value: serde_json::Value = serde_json::from_slice(&json_export.stdout).unwrap();
    assert_eq!(value["items"][0]["id"], id.as_str());

    let resolve = run(dir.path(), &["resolve", "spec.md", &id]);
    assert!(resolve.status.success(), "{resolve:?}");
    let export = run(dir.path(), &["export", "spec.md"]);
    assert_eq!(export.status.code(), Some(0), "{export:?}");
}

#[test]
fn wait_returns_feedback_and_times_out() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_spec(dir.path());
    let _ = seed_comment(&path, "Fix this.");
    let document = Document::load(&path).unwrap();
    let sidecar = sidecar_path(&path);
    let mut store = ReviewStore::load(&sidecar).unwrap();
    store.signoff_feedback();
    store.doc_hash = document.hash.clone();
    store.save(&sidecar).unwrap();

    let wait = run(
        dir.path(),
        &["wait", "spec.md", "--timeout", "2", "--interval", "50"],
    );
    assert_eq!(wait.status.code(), Some(10), "{wait:?}");
    assert!(String::from_utf8_lossy(&wait.stdout).contains("Fix this."));

    let path2 = dir.path().join("other.md");
    std::fs::write(&path2, "# Other\n").unwrap();
    let wait = run(
        dir.path(),
        &["wait", "other.md", "--timeout", "1", "--interval", "50"],
    );
    assert_eq!(wait.status.code(), Some(3), "{wait:?}");
}

#[test]
fn wait_returns_approval() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_spec(dir.path());
    let document = Document::load(&path).unwrap();
    let sidecar = sidecar_path(&path);
    let mut store = ReviewStore::new(path.display().to_string(), document.hash.clone());
    store.approve();
    store.save(&sidecar).unwrap();
    let wait = run(
        dir.path(),
        &["wait", "spec.md", "--timeout", "2", "--interval", "50"],
    );
    assert_eq!(wait.status.code(), Some(0), "{wait:?}");
    assert!(String::from_utf8_lossy(&wait.stdout).contains("Status: approved"));
}

#[test]
fn missing_sidecar_and_tmux_errors() {
    let dir = tempfile::tempdir().unwrap();
    write_spec(dir.path());
    let export = run(dir.path(), &["export", "spec.md"]);
    assert_eq!(export.status.code(), Some(2), "{export:?}");
    let tmux = run(dir.path(), &["review", "spec.md", "--tmux"]);
    assert_eq!(tmux.status.code(), Some(2), "{tmux:?}");
    assert!(String::from_utf8_lossy(&tmux.stderr).contains("tmux"));
    let bad = run(dir.path(), &["status", "missing.md"]);
    assert_eq!(bad.status.code(), Some(2), "{bad:?}");
}

#[test]
fn install_writes_recipes() {
    let dir = tempfile::tempdir().unwrap();
    let output = run(dir.path(), &["install", "claude", "--dir", "."]);
    assert!(output.status.success(), "{output:?}");
    let recipe = dir.path().join(".claude/commands/agentgloss-review.md");
    assert!(recipe.exists());
    let text = std::fs::read_to_string(recipe).unwrap();
    assert!(text.contains("agloss request"));
    assert!(text.contains("agloss export"));

    let output = run(dir.path(), &["install", "codex", "--dir", "."]);
    assert!(output.status.success());
    let text = std::fs::read_to_string(dir.path().join("AGENTS.md")).unwrap();
    assert_eq!(text.matches("<!-- agentgloss:start -->").count(), 1);
}

#[test]
fn resolve_validates_the_document() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_spec(dir.path());
    let id = seed_comment(&path, "Needs a document.");
    let sidecar = sidecar_path(&path);
    let before = std::fs::read(&sidecar).unwrap();
    std::fs::remove_file(&path).unwrap();
    let output = run(dir.path(), &["resolve", "spec.md", &id]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert_eq!(std::fs::read(&sidecar).unwrap(), before);
}

#[test]
fn export_acquires_the_store_lock() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_spec(dir.path());
    let _ = seed_comment(&path, "Locked.");
    let sidecar = sidecar_path(&path);
    let _lock = StoreLock::acquire(&sidecar).unwrap();
    let output = run(dir.path(), &["export", "spec.md"]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("locked"));
}

#[test]
fn status_invalidates_approval_after_a_change() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_spec(dir.path());
    let document = Document::load(&path).unwrap();
    let sidecar = sidecar_path(&path);
    let mut store = ReviewStore::new(path.display().to_string(), document.hash.clone());
    store.approve();
    store.save(&sidecar).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(&path, "# Plan\n\nChanged.\n").unwrap();
    let output = run(dir.path(), &["status", "spec.md", "--json"]);
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["status"], "invalidated");
    assert_eq!(value["approval_valid"], false);
    assert_eq!(value["doc_changed"], true);
}

#[test]
fn request_clears_a_previous_signoff() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_spec(dir.path());
    let document = Document::load(&path).unwrap();
    let sidecar = sidecar_path(&path);
    let mut store = ReviewStore::new(path.display().to_string(), document.hash.clone());
    store.approve();
    store.save(&sidecar).unwrap();
    assert!(run(dir.path(), &["request", "spec.md"]).status.success());
    let wait = run(
        dir.path(),
        &["wait", "spec.md", "--timeout", "1", "--interval", "50"],
    );
    assert_eq!(wait.status.code(), Some(3), "{wait:?}");
}

#[test]
fn custom_sidecar_commands_work() {
    let dir = tempfile::tempdir().unwrap();
    write_spec(dir.path());
    let request = run(
        dir.path(),
        &["request", "spec.md", "--sidecar", "custom.review.json"],
    );
    assert!(request.status.success(), "{request:?}");
    assert!(dir.path().join("custom.review.json").exists());
    let status = run(
        dir.path(),
        &[
            "status",
            "spec.md",
            "--sidecar",
            "custom.review.json",
            "--json",
        ],
    );
    assert!(status.status.success(), "{status:?}");
    let path = dir.path().join("spec.md");
    let document = Document::load(&path).unwrap();
    let start = document.source.find("Hello world.").unwrap();
    let anchor = Anchor::from_range(&document, start..start + "Hello world.".len()).unwrap();
    let custom = dir.path().join("custom.review.json");
    let mut store = ReviewStore::load(&custom).unwrap();
    let id = store.add_comment(anchor, "Custom sidecar item.");
    store.save(&custom).unwrap();
    let export = run(
        dir.path(),
        &["export", "spec.md", "--sidecar", "custom.review.json"],
    );
    assert_eq!(export.status.code(), Some(10), "{export:?}");
    let prompt = String::from_utf8_lossy(&export.stdout);
    assert!(
        prompt.contains(&format!(
            "agloss resolve spec.md {id} --sidecar custom.review.json"
        )),
        "{prompt}"
    );
}

#[test]
fn request_after_edit_increments_round() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_spec(dir.path());
    let _ = seed_comment(&path, "Round one.");
    let sidecar = sidecar_path(&path);
    let mut store = ReviewStore::load(&sidecar).unwrap();
    store.signoff_feedback();
    store.save(&sidecar).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(&path, "# Plan\n\nHello world.\n\nChanged revision.\n").unwrap();
    assert!(run(dir.path(), &["request", "spec.md"]).status.success());
    let status = run(dir.path(), &["status", "spec.md", "--json"]);
    let value: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(value["round"], 2, "{value}");
    assert_eq!(value["open"], 1);
}

#[test]
fn two_round_end_to_end_flow() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_spec(dir.path());
    let id = seed_comment(&path, "First round.");
    let sidecar = sidecar_path(&path);
    let mut store = ReviewStore::load(&sidecar).unwrap();
    store.signoff_feedback();
    store.save(&sidecar).unwrap();
    let first = run(dir.path(), &["export", "spec.md"]);
    assert_eq!(first.status.code(), Some(10));
    assert!(String::from_utf8_lossy(&first.stdout).contains("First round."));

    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(
        &path,
        "# Plan\n\nHello world.\n\nSecond paragraph with more text and a revision.\n",
    )
    .unwrap();
    assert!(run(dir.path(), &["request", "spec.md"]).status.success());
    let status = run(dir.path(), &["status", "spec.md", "--json"]);
    let value: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(value["round"], 2);
    assert_eq!(value["open"], 1);
    let resolve = run(dir.path(), &["resolve", "spec.md", &id]);
    assert!(resolve.status.success());
    let export = run(dir.path(), &["export", "spec.md"]);
    assert_eq!(export.status.code(), Some(0), "{export:?}");
}

#[test]
fn export_does_not_mutate_the_review_lifecycle() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_spec(dir.path());
    let document = Document::load(&path).unwrap();
    let sidecar = sidecar_path(&path);
    let mut store = ReviewStore::new(path.display().to_string(), document.hash.clone());
    store.approve();
    store.save(&sidecar).unwrap();
    let before = std::fs::read_to_string(&sidecar).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(&path, "# Plan\n\nChanged.\n").unwrap();
    let export = run(dir.path(), &["export", "spec.md"]);
    assert_eq!(export.status.code(), Some(0), "{export:?}");
    let after = std::fs::read_to_string(&sidecar).unwrap();
    assert_eq!(before, after);
    let value: serde_json::Value = serde_json::from_str(&after).unwrap();
    assert_eq!(value["round"], 1);
    assert!(value["approval"].is_object());
}

#[test]
fn status_human_output() {
    let dir = tempfile::tempdir().unwrap();
    write_spec(dir.path());
    let output = run(dir.path(), &["status", "spec.md"]);
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("Round: 1"));
    assert!(text.contains("Status: pending"));
}

#[cfg(unix)]
#[test]
fn tmux_adapter_builds_popup_and_detach_commands() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let path = write_spec(dir.path());
    let _id = seed_comment(&path, "Review this.");
    let sidecar = sidecar_path(&path);
    let mut store = ReviewStore::load(&sidecar).unwrap();
    store.signoff_feedback();
    store.save(&sidecar).unwrap();

    let bin_dir = dir.path().join("bin");
    std::fs::create_dir_all(&bin_dir).unwrap();
    let tmux = bin_dir.join("tmux");
    std::fs::write(
        &tmux,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" >> \"$TMUX_LOG\"\nif [ -n \"${TMUX_FAKE_RESULT:-}\" ]; then printf '%s\\n' \"$TMUX_FAKE_RESULT\" > \"$AGENTGLOSS_RESULT_FILE\"; fi\nexit ${TMUX_EXIT:-21}\n",
    )
    .unwrap();
    std::fs::set_permissions(&tmux, std::fs::Permissions::from_mode(0o755)).unwrap();

    let log = dir.path().join("tmux.log");
    let path_env = format!(
        "{}:{}",
        bin_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let output = Command::new(bin())
        .args(["review", "spec.md", "--tmux"])
        .current_dir(dir.path())
        .env("TMUX", "fake")
        .env("TMUX_LOG", &log)
        .env("PATH", &path_env)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(10), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("Review this."));
    let recorded = std::fs::read_to_string(&log).unwrap();
    assert!(recorded.contains("display-popup"));
    assert!(recorded.contains("__tui"));
    assert!(recorded.contains("spec.md"));
    assert!(recorded.contains("-E"));
    assert!(recorded.contains("-w"));
    assert!(recorded.contains("90%"));

    let feedback_exit = Command::new(bin())
        .args(["review", "spec.md", "--tmux"])
        .current_dir(dir.path())
        .env("TMUX", "fake")
        .env("TMUX_LOG", &log)
        .env("TMUX_EXIT", "21")
        .env("PATH", &path_env)
        .output()
        .unwrap();
    assert_eq!(feedback_exit.status.code(), Some(10), "{feedback_exit:?}");

    let result_fallback = Command::new(bin())
        .args(["review", "spec.md", "--tmux"])
        .current_dir(dir.path())
        .env("TMUX", "fake")
        .env("TMUX_LOG", &log)
        .env("TMUX_EXIT", "1")
        .env("TMUX_FAKE_RESULT", "21")
        .env("PATH", &path_env)
        .output()
        .unwrap();
    assert_eq!(
        result_fallback.status.code(),
        Some(10),
        "{result_fallback:?}"
    );
    assert!(String::from_utf8_lossy(&result_fallback.stdout).contains("Review this."));

    let abort_after_signoff = Command::new(bin())
        .args(["review", "spec.md", "--tmux"])
        .current_dir(dir.path())
        .env("TMUX", "fake")
        .env("TMUX_LOG", &log)
        .env("TMUX_EXIT", "22")
        .env("PATH", &path_env)
        .output()
        .unwrap();
    assert_eq!(
        abort_after_signoff.status.code(),
        Some(1),
        "{abort_after_signoff:?}"
    );
    assert!(String::from_utf8_lossy(&abort_after_signoff.stdout).is_empty());

    let approved_doc = dir.path().join("approved.md");
    std::fs::copy(&path, &approved_doc).unwrap();
    let approved_sidecar = sidecar_path(&approved_doc);
    let approved_id = seed_comment(&approved_doc, "Review this.");
    let mut approved_store = ReviewStore::load(&approved_sidecar).unwrap();
    approved_store.resolve(&approved_id).unwrap();
    approved_store.approve();
    approved_store.save(&approved_sidecar).unwrap();
    let approved = Command::new(bin())
        .args(["review", "approved.md", "--tmux"])
        .current_dir(dir.path())
        .env("TMUX", "fake")
        .env("TMUX_LOG", &log)
        .env("TMUX_EXIT", "20")
        .env("PATH", &path_env)
        .output()
        .unwrap();
    assert_eq!(approved.status.code(), Some(0), "{approved:?}");
    assert!(
        String::from_utf8_lossy(&approved.stdout).contains("The reviewer approved this revision.")
    );

    let popup_failure = Command::new(bin())
        .args(["review", "spec.md", "--tmux"])
        .current_dir(dir.path())
        .env("TMUX", "fake")
        .env("TMUX_LOG", &log)
        .env("TMUX_EXIT", "1")
        .env("PATH", &path_env)
        .output()
        .unwrap();
    assert_eq!(popup_failure.status.code(), Some(2), "{popup_failure:?}");
    assert!(String::from_utf8_lossy(&popup_failure.stderr).contains("agloss review"));

    let abort_doc = dir.path().join("abort.md");
    std::fs::write(&abort_doc, "# Abort\n").unwrap();
    assert!(run(dir.path(), &["request", "abort.md"]).status.success());
    let abort = Command::new(bin())
        .args(["review", "abort.md", "--tmux"])
        .current_dir(dir.path())
        .env("TMUX", "fake")
        .env("TMUX_LOG", &log)
        .env("TMUX_EXIT", "22")
        .env("PATH", &path_env)
        .output()
        .unwrap();
    assert_eq!(abort.status.code(), Some(1), "{abort:?}");
    assert!(String::from_utf8_lossy(&abort.stdout).is_empty());

    let custom = dir.path().join("custom.review.json");
    std::fs::copy(&sidecar, &custom).unwrap();
    std::fs::remove_file(&sidecar).unwrap();
    let custom_output = Command::new(bin())
        .args([
            "review",
            "spec.md",
            "--tmux",
            "--sidecar",
            "custom.review.json",
        ])
        .current_dir(dir.path())
        .env("TMUX", "fake")
        .env("TMUX_LOG", &log)
        .env("PATH", &path_env)
        .output()
        .unwrap();
    assert_eq!(custom_output.status.code(), Some(10), "{custom_output:?}");
    assert!(String::from_utf8_lossy(&custom_output.stdout).contains("Review this."));

    let detach = Command::new(bin())
        .args(["review", "spec.md", "--detach"])
        .current_dir(dir.path())
        .env("TMUX", "fake")
        .env("TMUX_LOG", &log)
        .env("TMUX_EXIT", "0")
        .env("PATH", &path_env)
        .output()
        .unwrap();
    assert!(detach.status.success(), "{detach:?}");
    assert!(String::from_utf8_lossy(&detach.stdout).contains("agloss wait"));
    let recorded = std::fs::read_to_string(&log).unwrap();
    assert!(recorded.contains("new-window"));

    let detach_custom = Command::new(bin())
        .args([
            "review",
            "spec.md",
            "--detach",
            "--sidecar",
            "custom.review.json",
        ])
        .current_dir(dir.path())
        .env("TMUX", "fake")
        .env("TMUX_LOG", &log)
        .env("TMUX_EXIT", "0")
        .env("PATH", &path_env)
        .output()
        .unwrap();
    assert!(detach_custom.status.success(), "{detach_custom:?}");
    assert!(String::from_utf8_lossy(&detach_custom.stdout).contains("--sidecar"));
}
