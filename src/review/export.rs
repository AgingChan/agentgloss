//! Human/agent-facing export formats for a review.

use super::{ItemKind, ItemStatus, ReviewStore};
use serde_json::{Value, json};
use std::path::Path;

pub fn status_label(store: &ReviewStore) -> &'static str {
    if store.is_approved() {
        "approved"
    } else if store.has_feedback() {
        "feedback"
    } else {
        "pending"
    }
}

pub fn export_prompt(
    store: &ReviewStore,
    include_resolved: bool,
    sidecar: Option<&Path>,
) -> String {
    let items = store.feedback_items(include_resolved);
    let open = store.open_count();
    let stale = store.stale_count();
    let comments = items
        .iter()
        .filter(|item| item.kind == ItemKind::Comment && item.status == ItemStatus::Open)
        .count();
    let suggestions = items
        .iter()
        .filter(|item| item.kind == ItemKind::Suggestion && item.status == ItemStatus::Open)
        .count();

    let mut out = String::new();
    out.push_str("# agentgloss review\n\n");
    out.push_str(&format!("- File: {}\n", store.file));
    out.push_str(&format!("- Round: {}\n", store.round));
    out.push_str(&format!("- Document hash: {}\n", store.doc_hash));
    out.push_str(&format!("- Status: {}\n", status_label(store)));
    if open == 0 {
        out.push_str("- Open items: 0\n");
        if stale > 0 {
            out.push_str(&format!("- Stale items: {stale}\n"));
        }
        out.push('\n');
    } else {
        out.push_str(&format!(
            "- Open items: {} ({} comment{}, {} suggestion{})\n",
            open,
            comments,
            if comments == 1 { "" } else { "s" },
            suggestions,
            if suggestions == 1 { "" } else { "s" }
        ));
        if stale > 0 {
            out.push_str(&format!("- Stale items: {stale}\n"));
        }
        out.push('\n');
    }

    if store.is_approved() {
        out.push_str("The reviewer approved this revision. No changes are required.\n");
        if items.is_empty() {
            return out;
        }
        out.push_str("\nResolved items (archived):\n\n");
        for (number, item) in items.iter().enumerate() {
            push_item(&mut out, number + 1, item);
        }
        return out;
    }

    if items.is_empty() {
        out.push_str("No open review items.\n");
        return out;
    }

    out.push_str(
        "Address the items below. Answer every comment and accept or reject every suggestion.\n",
    );
    out.push_str("After editing, mark fixed items resolved:\n");
    let sidecar_flag = sidecar
        .map(|path| format!(" --sidecar {}", quote_command_path(&path.to_string_lossy())))
        .unwrap_or_default();
    out.push_str(&format!(
        "  agentgloss resolve {} {}{}\n\n",
        quote_command_path(&store.file),
        items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        sidecar_flag
    ));

    for (number, item) in items.iter().enumerate() {
        push_item(&mut out, number + 1, item);
    }
    crate::sanitize::sanitize_terminal_text(&out).into_owned()
}

fn push_item(out: &mut String, number: usize, item: &super::ReviewItem) {
    let line_end = item
        .anchor
        .line_end
        .saturating_sub(1)
        .max(item.anchor.line_start);
    out.push_str(&format!(
        "## {}. [{}] {} — {} — lines {}-{}\n",
        number,
        item.id,
        item.kind.label(),
        item.status.label(),
        item.anchor.line_start,
        line_end
    ));
    if !item.anchor.heading_path.is_empty() {
        out.push_str(&format!(
            "Heading: {}\n",
            item.anchor.heading_path.join(" > ")
        ));
    }
    match item.kind {
        ItemKind::Comment => {
            out.push_str("Quote:\n");
            push_quote(out, &item.anchor.quote);
            out.push_str("Comment:\n");
            out.push_str(&item.body);
            out.push('\n');
        }
        ItemKind::Suggestion => {
            if let Some(suggestion) = &item.suggestion {
                out.push_str("Original:\n");
                out.push_str(&suggestion.original);
                if !suggestion.original.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str("Replacement:\n");
                out.push_str(&suggestion.replacement);
                if !suggestion.replacement.ends_with('\n') {
                    out.push('\n');
                }
            }
            if !item.body.is_empty() {
                out.push_str("Comment:\n");
                out.push_str(&item.body);
                out.push('\n');
            }
        }
    }
    out.push_str(&format!(
        "Anchor: bytes {}-{}\n\n",
        item.anchor.source_start, item.anchor.source_end
    ));
}

fn push_quote(out: &mut String, text: &str) {
    for line in text.lines() {
        if line.is_empty() {
            out.push_str(">\n");
        } else {
            out.push_str("> ");
            out.push_str(line);
            out.push('\n');
        }
    }
}

fn quote_command_path(text: &str) -> String {
    if text
        .chars()
        .any(|ch| ch.is_whitespace() || "'\"\\$`&;|<>()*?[]{}!#".contains(ch))
    {
        format!("'{}'", text.replace('\'', "'\\''"))
    } else {
        text.to_string()
    }
}

pub fn export_json(store: &ReviewStore, include_resolved: bool) -> Value {
    let items: Vec<&super::ReviewItem> = store.feedback_items(include_resolved);
    json!({
        "file": store.file,
        "round": store.round,
        "doc_hash": store.doc_hash,
        "status": status_label(store),
        "open": store.open_count(),
        "stale": store.stale_count(),
        "items": items,
    })
}

pub fn export_exit_code(store: &ReviewStore, include_resolved: bool) -> i32 {
    let items = store.feedback_items(include_resolved);
    if items.iter().any(|item| item.status != ItemStatus::Resolved) {
        10
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;
    use crate::review::{Anchor, ReviewStore};

    fn fixture() -> (ReviewStore, Document, String) {
        let source = "# Plan\n\nHello world.\n\nSecond paragraph.\n";
        let document = Document::from_source("spec.md", source.to_string());
        let start = source.find("Hello world.").unwrap();
        let anchor = Anchor::from_range(&document, start..start + "Hello world.".len()).unwrap();
        let mut store = ReviewStore::new("spec.md", document.hash.clone());
        let id = store.add_comment(anchor, "Please clarify this.");
        (store, document, id)
    }

    #[test]
    fn prompt_contains_numbered_items() {
        let (store, _document, id) = fixture();
        let prompt = export_prompt(&store, false, None);
        assert!(prompt.contains("# agentgloss review"));
        assert!(prompt.contains("- Status: feedback"));
        assert!(prompt.contains(&format!("[{}] comment", id)));
        assert!(prompt.contains("> Hello world."));
        assert!(prompt.contains("agentgloss resolve spec.md c1"));
    }

    #[test]
    fn prompt_sanitizes_control_characters() {
        let (mut store, document, _id) = fixture();
        let start = document.source.find("Second paragraph.").unwrap();
        let anchor =
            Anchor::from_range(&document, start..start + "Second paragraph.".len()).unwrap();
        store.add_comment(anchor, "Evil\x1b]0;pwned\x07 notice");
        let prompt = export_prompt(&store, false, None);
        assert!(!prompt.contains('\x1b'));
        assert!(!prompt.contains('\x07'));
        assert!(prompt.contains('\u{241b}'));
    }

    #[test]
    fn comment_prompt_is_a_stable_golden() {
        let (store, _document, id) = fixture();
        let anchor = &store.items[0].anchor;
        let expected = format!(
            "# agentgloss review\n\n\
- File: spec.md\n\
- Round: 1\n\
- Document hash: {}\n\
- Status: feedback\n\
- Open items: 1 (1 comment, 0 suggestions)\n\n\
Address the items below. Answer every comment and accept or reject every suggestion.\n\
After editing, mark fixed items resolved:\n\
\x20\x20agentgloss resolve spec.md {id}\n\n\
## 1. [{id}] comment — open — lines {}-{}\n\
Heading: Plan\n\
Quote:\n\
> Hello world.\n\
Comment:\n\
Please clarify this.\n\
Anchor: bytes {}-{}\n\n",
            store.doc_hash,
            anchor.line_start,
            anchor.line_end.saturating_sub(1).max(anchor.line_start),
            anchor.source_start,
            anchor.source_end
        );
        assert_eq!(export_prompt(&store, false, None), expected);
    }

    #[test]
    fn approved_prompt() {
        let document = Document::from_source("spec.md", "# Plan\n\nOk.\n".to_string());
        let mut store = ReviewStore::new("spec.md", document.hash.clone());
        store.approve();
        let prompt = export_prompt(&store, false, None);
        let expected = format!(
            "# agentgloss review\n\n- File: spec.md\n- Round: 1\n- Document hash: {}\n- Status: approved\n- Open items: 0\n\nThe reviewer approved this revision. No changes are required.\n",
            document.hash
        );
        assert_eq!(prompt, expected);
        assert_eq!(export_exit_code(&store, false), 0);
    }

    #[test]
    fn approved_all_lists_archived_items_without_action_instructions() {
        let (mut store, _document, id) = fixture();
        store.resolve(&id).unwrap();
        store.approve();
        let prompt = export_prompt(&store, true, None);
        assert!(prompt.contains("The reviewer approved this revision."));
        assert!(prompt.contains("Resolved items (archived):"));
        assert!(prompt.contains(&format!("[{id}] comment — resolved")));
        assert!(!prompt.contains("Address the items below"));
        assert!(!prompt.contains("agentgloss resolve"));
    }

    #[test]
    fn json_filtering_and_exit_code() {
        let (store, _document, id) = fixture();
        let value = export_json(&store, false);
        assert_eq!(value["open"], 1);
        assert_eq!(value["items"].as_array().unwrap().len(), 1);
        assert_eq!(export_exit_code(&store, false), 10);

        let mut store = store;
        store.resolve(&id).unwrap();
        assert_eq!(
            export_json(&store, false)["items"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        assert_eq!(
            export_json(&store, true)["items"].as_array().unwrap().len(),
            1
        );
        assert_eq!(export_exit_code(&store, false), 0);
    }

    #[test]
    fn resolve_command_quotes_special_paths() {
        let (mut store, _document, _id) = fixture();
        store.file = "spec file #1.md".to_string();
        let prompt = export_prompt(&store, false, None);
        assert!(prompt.contains("agentgloss resolve 'spec file #1.md' c1"));
    }

    #[test]
    fn quotes_preserve_blank_lines_and_bodies_stay_verbatim() {
        let source = "# A\n\nFirst line\n\nSecond line\n";
        let document = Document::from_source("spec.md", source.to_string());
        let start = source.find("First line").unwrap();
        let end = source.find("Second line").unwrap() + "Second line".len();
        let anchor = Anchor::from_range(&document, start..end).unwrap();
        let mut store = ReviewStore::new("spec.md", document.hash.clone());
        store.add_comment(anchor, "Keep this:\n```rust\nfn main() {}\n```\n");
        let prompt = export_prompt(&store, false, None);
        assert!(prompt.contains("> First line\n>\n> Second line\n"));
        assert!(prompt.contains("Keep this:\n```rust\nfn main() {}\n```\n"));
    }

    #[test]
    fn numbering_follows_anchor_order_not_insertion_order() {
        let source = "# A\n\nFirst paragraph.\n\nSecond paragraph.\n";
        let document = Document::from_source("spec.md", source.to_string());
        let mut store = ReviewStore::new("spec.md", document.hash.clone());
        let second = source.find("Second paragraph.").unwrap();
        let first = source.find("First paragraph.").unwrap();
        let second_id = store.add_comment(
            Anchor::from_range(&document, second..second + "Second paragraph.".len()).unwrap(),
            "second",
        );
        let first_id = store.add_comment(
            Anchor::from_range(&document, first..first + "First paragraph.".len()).unwrap(),
            "first",
        );
        let prompt = export_prompt(&store, false, None);
        assert!(prompt.contains(&format!("## 1. [{first_id}]")));
        assert!(prompt.contains(&format!("## 2. [{second_id}]")));
    }

    #[test]
    fn empty_and_stale_only_exports_are_clear() {
        let document = Document::from_source("spec.md", "# A\n\nHello.\n".to_string());
        let empty = ReviewStore::new("spec.md", document.hash.clone());
        let prompt = export_prompt(&empty, false, None);
        assert!(prompt.contains("Open items: 0"));
        assert!(prompt.contains("No open review items."));
        assert_eq!(export_exit_code(&empty, false), 0);

        let start = document.source.find("Hello.").unwrap();
        let anchor = Anchor::from_range(&document, start..start + "Hello.".len()).unwrap();
        let mut stale = ReviewStore::new("spec.md", document.hash.clone());
        stale.add_comment(anchor, "note");
        let changed = Document::from_source("spec.md", "# A\n\nDifferent.\n".to_string());
        stale.reanchor(&changed);
        let prompt = export_prompt(&stale, false, None);
        assert!(prompt.contains("- Open items: 0"));
        assert!(prompt.contains("- Stale items: 1"));
        assert!(prompt.contains("comment — stale"));
        assert_eq!(export_exit_code(&stale, false), 10);
    }

    #[test]
    fn resolve_command_includes_a_custom_sidecar() {
        let (store, _document, id) = fixture();
        let prompt = export_prompt(&store, false, Some(Path::new("state/review #1.json")));
        assert!(
            prompt.contains(&format!(
                "agentgloss resolve spec.md {id} --sidecar 'state/review #1.json'"
            )),
            "{prompt}"
        );
    }

    #[test]
    fn suggestion_prompt_includes_replacement() {
        let source = "# A\n\nold wording here\n";
        let document = Document::from_source("spec.md", source.to_string());
        let start = source.find("old wording here").unwrap();
        let anchor =
            Anchor::from_range(&document, start..start + "old wording here".len()).unwrap();
        let mut store = ReviewStore::new("spec.md", document.hash.clone());
        store.add_suggestion(anchor, "prefer this", "old wording", "new wording");
        let prompt = export_prompt(&store, false, None);
        assert!(prompt.contains("Original:\nold wording\nReplacement:\nnew wording\n"));
        assert!(!prompt.contains("> old wording"));
    }
}
