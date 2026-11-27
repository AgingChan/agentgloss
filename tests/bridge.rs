use agentgloss::review::{ReviewStore, StoreLock, sidecar_path};
use serde_json::{Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_agentgloss")
}

fn state(dir: &Path, args: &[&str]) -> Output {
    Command::new(bin())
        .args(["bridge", "state"])
        .args(args)
        .current_dir(dir)
        .env_remove("TMUX")
        .output()
        .expect("run bridge state")
}

fn apply(dir: &Path, args: &[&str], request: Value) -> Output {
    let mut child = Command::new(bin())
        .args(["bridge", "apply"])
        .args(args)
        .current_dir(dir)
        .env_remove("TMUX")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn bridge apply");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(serde_json::to_string(&request).unwrap().as_bytes())
        .unwrap();
    child.wait_with_output().expect("bridge apply output")
}

fn response(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "bad JSON ({error}): stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn write_spec(dir: &Path) -> PathBuf {
    let path = dir.join("spec.md");
    std::fs::write(
        &path,
        "# Plan\n\nHello world.\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n```mermaid\nflowchart LR\nA-->B\n```\n\n- one\n- two\n",
    )
    .unwrap();
    path
}

fn hash(dir: &Path) -> String {
    let value = response(&state(dir, &["spec.md", "--no-source"]));
    value["doc_hash"].as_str().unwrap().to_string()
}

fn request(op: &str, hash: &str) -> Value {
    json!({
        "schema_version": 1,
        "op": op,
        "expect_hash": hash,
    })
}

#[test]
fn state_is_pure_and_exposes_blocks() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_spec(dir.path());
    let output = state(dir.path(), &["spec.md"]);
    assert!(output.status.success(), "{output:?}");
    let value = response(&output);
    assert_eq!(value["ok"], true);
    assert_eq!(value["sidecar_exists"], false);
    assert_eq!(value["status"], "pending");
    assert_eq!(value["doc_changed"], false);
    assert_eq!(value["review"], Value::Null);
    assert!(value["source"].as_str().unwrap().contains("Hello world."));
    assert!(!sidecar_path(&path).exists());

    let blocks = value["blocks"].as_array().unwrap();
    let kinds: Vec<_> = blocks.iter().map(|b| b["kind"].as_str().unwrap()).collect();
    assert!(kinds.contains(&"heading"));
    assert!(kinds.contains(&"paragraph"));
    assert!(kinds.contains(&"table"));
    assert!(kinds.contains(&"mermaid"));
    assert!(kinds.contains(&"list_item"));
    let mermaid = blocks.iter().find(|b| b["kind"] == "mermaid").unwrap();
    assert_eq!(mermaid["start_line"], 10);
    assert_eq!(mermaid["end_line"], 11);
    let table = blocks.iter().find(|b| b["kind"] == "table").unwrap();
    assert_eq!(table["start_line"], 5);
    assert_eq!(table["end_line"], 7);

    let no_source = response(&state(dir.path(), &["spec.md", "--no-source"]));
    assert!(no_source.get("source").is_none());
    assert!(!sidecar_path(&path).exists());
}

#[test]
fn comment_suggestion_edit_resolve_and_signoff_flow() {
    let dir = tempfile::tempdir().unwrap();
    write_spec(dir.path());
    let hash = hash(dir.path());

    let mut req = request("comment", &hash);
    req["selection"] = json!({"start_line": 3, "end_line": 3});
    req["body"] = json!("Please clarify.");
    let output = apply(dir.path(), &["spec.md"], req);
    assert!(output.status.success(), "{output:?}");
    let value = response(&output);
    assert_eq!(value["ok"], true);
    assert_eq!(value["changed"], true);
    assert_eq!(value["id"], "c1");
    assert_eq!(value["status"], "feedback");
    assert_eq!(value["counts"]["open"], 1);

    let mut req = request("suggestion", &hash);
    req["selection"] = json!({"start_line": 14, "end_line": 14});
    req["replacement"] = json!("- uno");
    req["body"] = json!("Use Spanish.");
    let value = response(&apply(dir.path(), &["spec.md"], req));
    assert_eq!(value["id"], "c2");
    assert_eq!(
        value["review"]["items"][1]["suggestion"]["original"],
        "- one"
    );
    assert_eq!(value["review"]["items"][1]["body"], "Use Spanish.");

    let mut req = request("edit", &hash);
    req["id"] = json!("c1");
    req["body"] = json!("Clarify the greeting.");
    let value = response(&apply(dir.path(), &["spec.md"], req));
    assert_eq!(value["review"]["items"][0]["body"], "Clarify the greeting.");

    let mut req = request("edit", &hash);
    req["id"] = json!("c2");
    req["replacement"] = json!("- uno!");
    req["body"] = json!("Use Spanish here.");
    let value = response(&apply(dir.path(), &["spec.md"], req));
    assert_eq!(
        value["review"]["items"][1]["suggestion"]["replacement"],
        "- uno!"
    );
    assert_eq!(value["review"]["items"][1]["body"], "Use Spanish here.");

    let mut req = request("resolve", &hash);
    req["id"] = json!("c1");
    let value = response(&apply(dir.path(), &["spec.md"], req.clone()));
    assert_eq!(value["changed"], true);
    assert_eq!(value["counts"]["resolved"], 1);
    let value = response(&apply(dir.path(), &["spec.md"], req));
    assert_eq!(value["changed"], false);

    let mut req = request("unresolve", &hash);
    req["id"] = json!("c1");
    let value = response(&apply(dir.path(), &["spec.md"], req));
    assert_eq!(value["changed"], true);
    assert_eq!(value["counts"]["open"], 2);

    let value = response(&apply(dir.path(), &["spec.md"], request("signoff", &hash)));
    assert_eq!(value["terminal_exit_code"], 10);
    assert_eq!(value["review"]["last_signoff"], "feedback");
    assert_eq!(value["review"]["approval"], Value::Null);

    for id in ["c1", "c2"] {
        let mut req = request("resolve", &hash);
        req["id"] = json!(id);
        let value = response(&apply(dir.path(), &["spec.md"], req));
        assert_eq!(value["ok"], true);
    }
    let value = response(&apply(dir.path(), &["spec.md"], request("signoff", &hash)));
    assert_eq!(value["terminal_exit_code"], 0);
    assert_eq!(value["status"], "approved");
    assert_eq!(value["approval_valid"], true);
    assert!(value["review"]["approval"].is_object());

    let mut req = request("delete", &hash);
    req["id"] = json!("c2");
    let value = response(&apply(dir.path(), &["spec.md"], req));
    assert_eq!(value["counts"]["total"], 1);

    let stored = ReviewStore::load(&sidecar_path(&dir.path().join("spec.md"))).unwrap();
    assert_eq!(stored.items.len(), 1);
    assert_eq!(stored.items[0].id, "c1");
    assert!(stored.is_approved());
}

#[test]
fn selection_and_expect_hash_are_validated() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_spec(dir.path());
    let hash = hash(dir.path());
    let line_count = response(&state(dir.path(), &["spec.md", "--no-source"]))["line_count"]
        .as_u64()
        .unwrap() as usize;

    let mut req = request("comment", &hash);
    req["selection"] = json!({"start_line": 3, "end_line": 3});
    req["body"] = json!("note");
    req["expect_hash"] = json!("deadbeef");
    let output = apply(dir.path(), &["spec.md"], req);
    assert_eq!(output.status.code(), Some(2));
    let value = response(&output);
    assert_eq!(value["error"]["code"], "document_changed");
    assert!(!sidecar_path(&path).exists());

    let cases = [
        json!({"start_line": 0, "end_line": 1}),
        json!({"start_line": 4, "end_line": 3}),
        json!({"start_line": 1, "end_line": line_count + 1}),
        json!({"start_line": 2, "end_line": 2}),
        json!({"start_line": line_count, "end_line": line_count}),
    ];
    for selection in cases {
        let mut req = request("comment", &hash);
        req["selection"] = selection.clone();
        req["body"] = json!("note");
        let value = response(&apply(dir.path(), &["spec.md"], req));
        assert_eq!(
            value["error"]["code"], "invalid_selection",
            "{selection}: {value}"
        );
    }

    let mut req = request("comment", &hash);
    req["selection"] = json!({"start_line": 3, "end_line": 3});
    req["body"] = json!("   ");
    let value = response(&apply(dir.path(), &["spec.md"], req));
    assert_eq!(value["error"]["code"], "invalid_operation");

    let mut req = request("suggestion", &hash);
    req["selection"] = json!({"start_line": 3, "end_line": 3});
    req["replacement"] = json!("Hello world.");
    let value = response(&apply(dir.path(), &["spec.md"], req));
    assert_eq!(value["error"]["code"], "invalid_operation");

    let mut req = request("resolve", &hash);
    req["id"] = json!("c99");
    let value = response(&apply(dir.path(), &["spec.md"], req));
    assert_eq!(value["error"]["code"], "invalid_id");
}

#[test]
fn suggestions_are_rejected_on_image_blocks() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("pic.md"), "# Pic\n\n![alt](a.png)\n").unwrap();
    let hash = response(&state(dir.path(), &["pic.md", "--no-source"]))["doc_hash"]
        .as_str()
        .unwrap()
        .to_string();
    let mut req = request("suggestion", &hash);
    req["selection"] = json!({"start_line": 3, "end_line": 3});
    req["replacement"] = json!("![alt](b.png)");
    let value = response(&apply(dir.path(), &["pic.md"], req));
    assert_eq!(value["error"]["code"], "invalid_operation");
}

#[test]
fn state_stays_available_while_mutations_are_locked() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_spec(dir.path());
    let hash = hash(dir.path());
    let mut req = request("comment", &hash);
    req["selection"] = json!({"start_line": 3, "end_line": 3});
    req["body"] = json!("note");
    assert!(apply(dir.path(), &["spec.md"], req).status.success());
    let sidecar = sidecar_path(&path);
    let _lock = StoreLock::acquire(&sidecar).unwrap();
    let value = response(&state(dir.path(), &["spec.md", "--no-source"]));
    assert_eq!(value["ok"], true);
    assert_eq!(value["counts"]["open"], 1);
    let mut req = request("comment", &hash);
    req["selection"] = json!({"start_line": 3, "end_line": 3});
    req["body"] = json!("second");
    let output = apply(dir.path(), &["spec.md"], req);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(response(&output)["error"]["code"], "locked");
}

#[test]
fn state_is_repeatable_without_side_effects() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_spec(dir.path());
    let hash = hash(dir.path());
    let mut req = request("comment", &hash);
    req["selection"] = json!({"start_line": 3, "end_line": 3});
    req["body"] = json!("note");
    assert!(apply(dir.path(), &["spec.md"], req).status.success());
    let sidecar = sidecar_path(&path);
    let before = std::fs::read(&sidecar).unwrap();
    for _ in 0..2 {
        let value = response(&state(dir.path(), &["spec.md"]));
        assert_eq!(value["counts"]["open"], 1);
    }
    assert_eq!(std::fs::read(&sidecar).unwrap(), before);
    let backups = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().contains(".bak."))
        .count();
    assert_eq!(backups, 0);
}

#[test]
fn sync_creates_nothing_and_recovers_corruption() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_spec(dir.path());
    let value = response(&apply(
        dir.path(),
        &["spec.md"],
        json!({
            "schema_version": 1,
            "op": "sync",
        }),
    ));
    assert_eq!(value["ok"], true);
    assert_eq!(value["sidecar_exists"], false);
    assert_eq!(value["changed"], false);
    assert!(!sidecar_path(&path).exists());

    let sidecar = sidecar_path(&path);
    std::fs::write(&sidecar, "{not json").unwrap();
    let output = state(dir.path(), &["spec.md", "--no-source"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(response(&output)["error"]["code"], "corrupt_sidecar");
    assert!(sidecar.exists());

    let value = response(&apply(
        dir.path(),
        &["spec.md"],
        json!({
            "schema_version": 1,
            "op": "sync",
        }),
    ));
    assert_eq!(value["ok"], true);
    assert_eq!(value["recovered"], true);
    assert!(value["backup_path"].as_str().unwrap().contains(".bak."));
    assert!(sidecar.exists());
    assert_eq!(
        response(&state(dir.path(), &["spec.md", "--no-source"]))["ok"],
        true
    );
}

#[test]
fn unsupported_sidecar_is_not_backed_up() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_spec(dir.path());
    let sidecar = sidecar_path(&path);
    std::fs::write(
        &sidecar,
        r#"{"version":2,"file":9,"items":"future","extra":true}"#,
    )
    .unwrap();
    let output = state(dir.path(), &["spec.md", "--no-source"]);
    assert_eq!(output.status.code(), Some(2));
    let value = response(&output);
    assert_eq!(value["error"]["code"], "unsupported_sidecar_version");
    assert_eq!(value["error"]["found"], 2);
    assert_eq!(value["error"]["supported"], 1);

    let output = apply(
        dir.path(),
        &["spec.md"],
        json!({
            "schema_version": 1,
            "op": "sync",
        }),
    );
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        response(&output)["error"]["code"],
        "unsupported_sidecar_version"
    );
    assert!(sidecar.exists());
    let backups = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().contains(".bak."))
        .count();
    assert_eq!(backups, 0);
}

#[test]
fn crlf_and_missing_trailing_newline_selections_work() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("crlf.md"), "# Plan\r\n\r\nHello world.\r\n").unwrap();
    let hash = response(&state(dir.path(), &["crlf.md", "--no-source"]))["doc_hash"]
        .as_str()
        .unwrap()
        .to_string();
    let mut req = request("comment", &hash);
    req["selection"] = json!({"start_line": 3, "end_line": 3});
    req["body"] = json!("crlf note");
    let value = response(&apply(dir.path(), &["crlf.md"], req));
    assert_eq!(
        value["review"]["items"][0]["anchor"]["quote"],
        "Hello world."
    );

    std::fs::write(dir.path().join("plain.md"), "# Plan\n\nLast line").unwrap();
    let hash = response(&state(dir.path(), &["plain.md", "--no-source"]))["doc_hash"]
        .as_str()
        .unwrap()
        .to_string();
    let mut req = request("comment", &hash);
    req["selection"] = json!({"start_line": 3, "end_line": 3});
    req["body"] = json!("last note");
    let value = response(&apply(dir.path(), &["plain.md"], req));
    assert_eq!(value["review"]["items"][0]["anchor"]["quote"], "Last line");
}

#[test]
fn custom_sidecar_and_reattach_work() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_spec(dir.path());
    let sidecar = dir.path().join("custom.review.json");
    let hash = response(&state(dir.path(), &["spec.md", "--no-source"]))["doc_hash"]
        .as_str()
        .unwrap()
        .to_string();
    let mut req = request("comment", &hash);
    req["selection"] = json!({"start_line": 3, "end_line": 3});
    req["body"] = json!("one");
    let output = apply(
        dir.path(),
        &["spec.md", "--sidecar", "custom.review.json"],
        req,
    );
    assert!(output.status.success(), "{output:?}");
    assert!(sidecar.exists());

    let mut store = ReviewStore::load(&sidecar).unwrap();
    store.items[0].status = agentgloss::review::ItemStatus::Stale;
    store.save(&sidecar).unwrap();

    let mut req = request("reattach", &hash);
    req["id"] = json!("c1");
    req["selection"] = json!({"start_line": 15, "end_line": 15});
    let value = response(&apply(
        dir.path(),
        &["spec.md", "--sidecar", "custom.review.json"],
        req,
    ));
    assert_eq!(value["ok"], true);
    assert_eq!(value["review"]["items"][0]["status"], "open");
    assert_eq!(value["review"]["items"][0]["anchor"]["quote"], "- two");
    assert_eq!(
        value["review"]["items"][0]["history"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["method"],
        "reattached"
    );
    assert!(!sidecar_path(&path).exists());
}

#[test]
fn missing_document_and_bad_requests_are_structured_errors() {
    let dir = tempfile::tempdir().unwrap();
    let output = state(dir.path(), &["missing.md"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(response(&output)["error"]["code"], "missing_document");

    std::fs::write(dir.path().join("spec.md"), "# Plan\n").unwrap();
    let output = apply(
        dir.path(),
        &["spec.md"],
        json!({"schema_version": 99, "op": "sync"}),
    );
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(response(&output)["error"]["code"], "invalid_operation");

    let output = apply(
        dir.path(),
        &["spec.md"],
        json!({"schema_version": 1, "op": "nope"}),
    );
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(response(&output)["error"]["code"], "invalid_operation");
}
