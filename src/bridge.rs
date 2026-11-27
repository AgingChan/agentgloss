//! Hidden JSON bridge for editor integrations.
//!
//! The bridge is a thin, versioned façade over the review library. It prints
//! one JSON envelope per invocation, takes no long-lived server state, and
//! never changes the sidecar schema.

use crate::document::{Block, BlockKind, Document, DocumentError};
use crate::review::export::status_label;
use crate::review::{
    Anchor, ItemKind, ItemStatus, ReviewError, ReviewStore, SignoffKind, StoreLock, sidecar_path,
};
use clap::{Args, Subcommand};
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::Read;
use std::path::{Path, PathBuf};

pub const BRIDGE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Args)]
pub struct BridgeArgs {
    #[command(subcommand)]
    pub command: BridgeCommand,
}

#[derive(Debug, Subcommand)]
pub enum BridgeCommand {
    /// Print the current document and review state.
    State(BridgeStateArgs),
    /// Apply one review operation from JSON on stdin.
    Apply(BridgeApplyArgs),
}

#[derive(Debug, Args)]
pub struct BridgeStateArgs {
    pub file: PathBuf,
    #[arg(long)]
    pub sidecar: Option<PathBuf>,
    #[arg(long)]
    pub no_source: bool,
}

#[derive(Debug, Args)]
pub struct BridgeApplyArgs {
    pub file: PathBuf,
    #[arg(long)]
    pub sidecar: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Op {
    Sync,
    Comment,
    Suggestion,
    Edit,
    Resolve,
    Unresolve,
    Delete,
    Reattach,
    Signoff,
}

#[derive(Debug, Deserialize)]
struct ApplyRequest {
    #[serde(default)]
    schema_version: u32,
    op: Op,
    #[serde(default)]
    expect_hash: Option<String>,
    #[serde(default)]
    selection: Option<Selection>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    replacement: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    include_source: bool,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct Selection {
    start_line: usize,
    end_line: usize,
}

pub fn run(args: &BridgeArgs) -> i32 {
    let result = match &args.command {
        BridgeCommand::State(state) => run_state(state),
        BridgeCommand::Apply(apply) => run_apply(apply),
    };
    match result {
        Ok(value) => {
            print_json(&value);
            0
        }
        Err(error) => {
            eprintln!("agentgloss bridge: {error}");
            print_json(&error.envelope());
            2
        }
    }
}

fn print_json(value: &Value) {
    match serde_json::to_string(value) {
        Ok(text) => println!("{text}"),
        Err(error) => eprintln!("agentgloss bridge: cannot serialize response: {error}"),
    }
}

fn run_state(args: &BridgeStateArgs) -> Result<Value, BridgeError> {
    let document = load_document(&args.file)?;
    let sidecar = sidecar_for(&args.file, args.sidecar.as_ref());
    let store = if sidecar.exists() {
        Some(ReviewStore::load(&sidecar)?)
    } else {
        None
    };
    Ok(state_value(
        &document,
        store.as_ref(),
        &sidecar,
        !args.no_source,
    ))
}

fn run_apply(args: &BridgeApplyArgs) -> Result<Value, BridgeError> {
    let document = load_document(&args.file)?;
    let sidecar = sidecar_for(&args.file, args.sidecar.as_ref());
    let request = read_request()?;
    if request.schema_version != BRIDGE_SCHEMA_VERSION {
        return Err(BridgeError::InvalidOperation(format!(
            "unsupported bridge schema version {}; expected {BRIDGE_SCHEMA_VERSION}",
            request.schema_version
        )));
    }
    if request.op == Op::Sync {
        return apply_sync(&document, &sidecar);
    }
    apply_mutation(&document, &sidecar, request)
}

fn apply_sync(document: &Document, sidecar: &Path) -> Result<Value, BridgeError> {
    if !sidecar.exists() {
        let mut value = state_value(document, None, sidecar, true);
        add_apply_fields(
            &mut value,
            ApplyFields {
                changed: false,
                id: None,
                reconciled: false,
                recovered: false,
                backup_path: None,
                terminal_exit_code: None,
            },
        );
        return Ok(value);
    }
    let _lock = StoreLock::acquire(sidecar)?;
    let outcome = ReviewStore::load_or_recover(
        sidecar,
        &document.path.display().to_string(),
        &document.hash,
    )?;
    let recovered = outcome.recovered;
    let backup_path = outcome
        .backup_path
        .as_ref()
        .map(|path| path.display().to_string());
    let mut store = outcome.store;
    let before = store.clone();
    let reconciled = store.doc_hash != document.hash;
    store.start_round_if_changed(document);
    let changed = store != before || recovered;
    if changed {
        store.save(sidecar)?;
    }
    let mut value = state_value(document, Some(&store), sidecar, true);
    add_apply_fields(
        &mut value,
        ApplyFields {
            changed,
            id: None,
            reconciled,
            recovered,
            backup_path,
            terminal_exit_code: None,
        },
    );
    Ok(value)
}

#[derive(Default)]
struct ApplyFields {
    changed: bool,
    id: Option<String>,
    reconciled: bool,
    recovered: bool,
    backup_path: Option<String>,
    terminal_exit_code: Option<i32>,
}

fn add_apply_fields(value: &mut Value, fields: ApplyFields) {
    value["changed"] = json!(fields.changed);
    value["id"] = fields.id.map_or(Value::Null, Value::String);
    value["reconciled"] = json!(fields.reconciled);
    value["recovered"] = json!(fields.recovered);
    value["backup_path"] = fields.backup_path.map_or(Value::Null, Value::String);
    value["terminal_exit_code"] = fields
        .terminal_exit_code
        .map_or(Value::Null, |code| json!(code));
}

fn apply_mutation(
    document: &Document,
    sidecar: &Path,
    request: ApplyRequest,
) -> Result<Value, BridgeError> {
    let expect_hash = request
        .expect_hash
        .as_deref()
        .ok_or_else(|| BridgeError::InvalidOperation("expect_hash is required".to_string()))?;
    if expect_hash != document.hash.as_str() {
        return Err(BridgeError::DocumentChanged);
    }
    let _lock = StoreLock::acquire(sidecar)?;
    let sidecar_exists = sidecar.exists();
    let mut store = if sidecar_exists {
        ReviewStore::load(sidecar)?
    } else {
        ReviewStore::new(document.path.display().to_string(), document.hash.clone())
    };
    let before_hash = store.doc_hash.clone();
    store.start_round_if_changed(document);
    let reconciled = before_hash != document.hash;

    let mut fields = ApplyFields {
        reconciled,
        ..ApplyFields::default()
    };
    match request.op {
        Op::Sync => unreachable!("sync handled before mutation"),
        Op::Comment => {
            let selection = require_selection(&request)?;
            let body = request.body.unwrap_or_default();
            if body.trim().is_empty() {
                return Err(BridgeError::InvalidOperation(
                    "comment body is empty".to_string(),
                ));
            }
            let anchor = anchor_from_selection(document, selection)?;
            fields.id = Some(store.add_comment(anchor, body));
            fields.changed = true;
        }
        Op::Suggestion => {
            let selection = require_selection(&request)?;
            let replacement = request.replacement.unwrap_or_default();
            if replacement.trim().is_empty() {
                return Err(BridgeError::InvalidOperation(
                    "suggestion replacement is empty".to_string(),
                ));
            }
            let anchor = anchor_from_selection(document, selection)?;
            if document
                .block_at(anchor.source_start)
                .is_some_and(|block| matches!(block.kind, BlockKind::Image { .. }))
            {
                return Err(BridgeError::InvalidOperation(
                    "suggestions are not supported on image blocks".to_string(),
                ));
            }
            if replacement == anchor.quote {
                return Err(BridgeError::InvalidOperation(
                    "suggestion replacement is unchanged".to_string(),
                ));
            }
            let original = anchor.quote.clone();
            fields.id = Some(store.add_suggestion(
                anchor,
                request.body.unwrap_or_default(),
                original,
                replacement,
            ));
            fields.changed = true;
        }
        Op::Edit => {
            let id = require_id(&request)?;
            let kind = store
                .item(&id)
                .map(|item| item.kind)
                .ok_or_else(|| BridgeError::InvalidId(id.clone()))?;
            match kind {
                ItemKind::Comment => {
                    if request.replacement.is_some() {
                        return Err(BridgeError::InvalidOperation(
                            "comments do not have a replacement".to_string(),
                        ));
                    }
                    let body = request.body.ok_or_else(|| {
                        BridgeError::InvalidOperation("comment body is required".to_string())
                    })?;
                    store.update_comment(&id, body)?;
                }
                ItemKind::Suggestion => {
                    if request.body.is_none() && request.replacement.is_none() {
                        return Err(BridgeError::InvalidOperation(
                            "no edit was requested".to_string(),
                        ));
                    }
                    store.update_suggestion(&id, request.body, request.replacement)?;
                }
            }
            fields.changed = true;
        }
        Op::Resolve => {
            let id = require_id(&request)?;
            let status = store
                .item(&id)
                .map(|item| item.status)
                .ok_or_else(|| BridgeError::InvalidId(id.clone()))?;
            if status != ItemStatus::Resolved {
                store.resolve(&id)?;
                fields.changed = true;
            }
        }
        Op::Unresolve => {
            let id = require_id(&request)?;
            let status = store
                .item(&id)
                .map(|item| item.status)
                .ok_or_else(|| BridgeError::InvalidId(id.clone()))?;
            match status {
                ItemStatus::Open => {}
                ItemStatus::Stale => {
                    return Err(BridgeError::InvalidOperation(
                        "stale items must be reattached before reopening".to_string(),
                    ));
                }
                ItemStatus::Resolved => {
                    store.unresolve(&id)?;
                    fields.changed = true;
                }
            }
        }
        Op::Delete => {
            let id = require_id(&request)?;
            store.remove(&id)?;
            fields.changed = true;
        }
        Op::Reattach => {
            let id = require_id(&request)?;
            let status = store
                .item(&id)
                .map(|item| item.status)
                .ok_or_else(|| BridgeError::InvalidId(id.clone()))?;
            if status != ItemStatus::Stale {
                return Err(BridgeError::InvalidOperation(
                    "only stale items can be reattached".to_string(),
                ));
            }
            let selection = require_selection(&request)?;
            let anchor = anchor_from_selection(document, selection)?;
            store.reattach(&id, anchor)?;
            fields.changed = true;
        }
        Op::Signoff => {
            let kind = store.signoff();
            fields.terminal_exit_code = Some(match kind {
                SignoffKind::Approved => 0,
                SignoffKind::Feedback => 10,
            });
            fields.changed = true;
        }
    }

    if fields.changed || fields.reconciled || !sidecar_exists {
        store.save(sidecar)?;
    }
    let mut value = state_value(document, Some(&store), sidecar, request.include_source);
    add_apply_fields(&mut value, fields);
    Ok(value)
}

fn read_request() -> Result<ApplyRequest, BridgeError> {
    let mut raw = String::new();
    std::io::stdin()
        .read_to_string(&mut raw)
        .map_err(|error| BridgeError::Io(format!("cannot read bridge request: {error}")))?;
    serde_json::from_str(&raw)
        .map_err(|error| BridgeError::InvalidOperation(format!("invalid bridge request: {error}")))
}

fn require_selection(request: &ApplyRequest) -> Result<Selection, BridgeError> {
    request
        .selection
        .ok_or_else(|| BridgeError::InvalidSelection("selection is required".to_string()))
}

fn require_id(request: &ApplyRequest) -> Result<String, BridgeError> {
    request
        .id
        .clone()
        .ok_or_else(|| BridgeError::InvalidId("item id is required".to_string()))
}

fn anchor_from_selection(document: &Document, selection: Selection) -> Result<Anchor, BridgeError> {
    if selection.start_line == 0 || selection.end_line < selection.start_line {
        return Err(BridgeError::InvalidSelection(
            "selection must be a positive, ordered line range".to_string(),
        ));
    }
    let line_count = document.line_count();
    if selection.start_line > line_count || selection.end_line > line_count {
        return Err(BridgeError::InvalidSelection(format!(
            "selection lines {}-{} exceed the document's {line_count} lines",
            selection.start_line, selection.end_line
        )));
    }
    let start = *document
        .line_starts
        .get(selection.start_line - 1)
        .ok_or_else(|| BridgeError::InvalidSelection("start line is out of range".to_string()))?;
    let end = document
        .line_starts
        .get(selection.end_line)
        .copied()
        .unwrap_or(document.source.len());
    let end = end.max(start).min(document.source.len());
    if document
        .source
        .get(start..end)
        .is_none_or(|slice| slice.trim().is_empty())
    {
        return Err(BridgeError::InvalidSelection(
            "selection has no reviewable text".to_string(),
        ));
    }
    Anchor::from_range(document, start..end).map_err(Into::into)
}

fn sidecar_for(file: &Path, override_path: Option<&PathBuf>) -> PathBuf {
    override_path.cloned().unwrap_or_else(|| sidecar_path(file))
}

fn load_document(file: &Path) -> Result<Document, BridgeError> {
    Document::load(file).map_err(|error| match error {
        DocumentError::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound => {
            BridgeError::MissingDocument(file.display().to_string())
        }
        other => BridgeError::Io(other.to_string()),
    })
}

fn state_value(
    document: &Document,
    store: Option<&ReviewStore>,
    sidecar: &Path,
    include_source: bool,
) -> Value {
    let approval_valid = store
        .and_then(|store| store.approval.as_ref())
        .is_some_and(|approval| approval.hash == document.hash)
        && store.is_none_or(|store| !store.has_feedback());
    let doc_changed = store.is_some_and(|store| store.doc_hash != document.hash);
    let status = if store.is_some_and(|store| store.approval.is_some()) && !approval_valid {
        "invalidated"
    } else if doc_changed {
        "changed"
    } else {
        store.map_or("pending", status_label)
    };
    let open = store.map_or(0, ReviewStore::open_count);
    let stale = store.map_or(0, ReviewStore::stale_count);
    let resolved = store.map_or(0, ReviewStore::resolved_count);
    let total = store.map_or(0, |store| store.items.len());
    let blocks: Vec<Value> = document.blocks.iter().map(block_value).collect();
    let mut value = json!({
        "schema_version": BRIDGE_SCHEMA_VERSION,
        "ok": true,
        "agentgloss_version": env!("CARGO_PKG_VERSION"),
        "file": document.path.display().to_string(),
        "sidecar": sidecar.display().to_string(),
        "sidecar_exists": store.is_some(),
        "doc_hash": document.hash,
        "line_count": document.line_count(),
        "blocks": blocks,
        "review": store,
        "status": status,
        "doc_changed": doc_changed,
        "approval_valid": approval_valid,
        "counts": {
            "open": open,
            "stale": stale,
            "resolved": resolved,
            "total": total,
        },
    });
    if include_source {
        value["source"] = json!(document.source);
    }
    value
}

fn block_value(block: &Block) -> Value {
    let end_line = block.lines.end.saturating_sub(1).max(block.lines.start);
    let marker = match &block.kind {
        BlockKind::ListItem { marker, .. } => json!(marker),
        _ => Value::Null,
    };
    json!({
        "index": block.index,
        "kind": block.kind.kind_name(),
        "level": block.kind.heading_level(),
        "start_line": block.lines.start,
        "end_line": end_line,
        "heading_path": block.heading_path,
        "marker": marker,
    })
}

#[derive(Debug, thiserror::Error)]
enum BridgeError {
    #[error("cannot read document {0}")]
    MissingDocument(String),
    #[error("corrupt sidecar: {0}")]
    CorruptSidecar(String),
    #[error("unsupported sidecar version {found}; supported version is {supported}")]
    UnsupportedVersion { found: u32, supported: u32 },
    #[error("review state is locked")]
    Locked,
    #[error("the document changed since the requested state")]
    DocumentChanged,
    #[error("invalid selection: {0}")]
    InvalidSelection(String),
    #[error("invalid operation: {0}")]
    InvalidOperation(String),
    #[error("unknown review item: {0}")]
    InvalidId(String),
    #[error("io error: {0}")]
    Io(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl BridgeError {
    fn code(&self) -> &'static str {
        match self {
            BridgeError::MissingDocument(_) => "missing_document",
            BridgeError::CorruptSidecar(_) => "corrupt_sidecar",
            BridgeError::UnsupportedVersion { .. } => "unsupported_sidecar_version",
            BridgeError::Locked => "locked",
            BridgeError::DocumentChanged => "document_changed",
            BridgeError::InvalidSelection(_) => "invalid_selection",
            BridgeError::InvalidOperation(_) => "invalid_operation",
            BridgeError::InvalidId(_) => "invalid_id",
            BridgeError::Io(_) => "io_error",
            BridgeError::Internal(_) => "internal",
        }
    }

    fn envelope(&self) -> Value {
        let mut error = json!({
            "code": self.code(),
            "message": self.to_string(),
        });
        if let BridgeError::UnsupportedVersion { found, supported } = self {
            error["found"] = json!(found);
            error["supported"] = json!(supported);
        }
        json!({
            "schema_version": BRIDGE_SCHEMA_VERSION,
            "ok": false,
            "error": error,
        })
    }
}

impl From<ReviewError> for BridgeError {
    fn from(error: ReviewError) -> Self {
        match error {
            ReviewError::Io { .. } => BridgeError::Io(error.to_string()),
            ReviewError::Json { .. } => BridgeError::CorruptSidecar(error.to_string()),
            ReviewError::UnsupportedVersion { found, supported } => {
                BridgeError::UnsupportedVersion { found, supported }
            }
            ReviewError::Locked { .. } => BridgeError::Locked,
            ReviewError::ItemNotFound(id) => BridgeError::InvalidId(id),
            ReviewError::StaleItem(id) => BridgeError::InvalidOperation(format!(
                "stale item cannot be reopened; reattach it first: {id}"
            )),
            ReviewError::InvalidOperation(message) => BridgeError::InvalidOperation(message),
            ReviewError::EmptyDocument => {
                BridgeError::InvalidSelection("the document is empty".to_string())
            }
            ReviewError::EmptyAnchor => {
                BridgeError::InvalidSelection("the selection has no reviewable text".to_string())
            }
            ReviewError::Serialize(_) => BridgeError::Internal(error.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidecar_version_constant_is_stable() {
        assert_eq!(crate::review::SIDECAR_VERSION, 1);
        assert_eq!(BRIDGE_SCHEMA_VERSION, 1);
    }

    #[test]
    fn error_envelopes_carry_codes() {
        assert_eq!(BridgeError::Locked.code(), "locked");
        assert_eq!(
            BridgeError::UnsupportedVersion {
                found: 2,
                supported: 1
            }
            .code(),
            "unsupported_sidecar_version"
        );
        let value = BridgeError::DocumentChanged.envelope();
        assert_eq!(value["ok"], false);
        assert_eq!(value["error"]["code"], "document_changed");
    }
}
