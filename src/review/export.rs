//! Human/agent-facing export formats for a review.

use super::{ItemKind, ItemStatus, ReviewStore};
use serde_json::{Value, json};

pub fn status_label(store: &ReviewStore) -> &'static str {
    if store.is_approved() {
        "approved"
    } else if store.has_feedback() {
        "feedback"
    } else {
        "pending"
    }
}

pub fn export_prompt(store: &ReviewStore, include_resolved: bool) -> String {
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
    out.push_str("# mdreview review\n\n");
    out.push_str(&format!("- File: {}\n", store.file));
    out.push_str(&format!("- Round: {}\n", store.round));
    out.push_str(&format!("- Document hash: {}\n", store.doc_hash));
    out.push_str(&format!("- Status: {}\n", status_label(store)));
    if open == 0 && stale == 0 {
        out.push_str("- Open items: 0\n\n");
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

    if items.is_empty() {
        if store.is_approved() {
            out.push_str("The reviewer approved this revision. No changes are required.\n");
        } else {
            out.push_str("No open review items.\n");
        }
        return out;
    }

    out.push_str(
        "Address the items below. Answer every comment and accept or reject every suggestion.\n",
    );
    out.push_str("After editing, mark fixed items resolved:\n");
    out.push_str(&format!(
        "  mdreview resolve {} {}\n\n",
        store.file,
        items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    ));

    for (number, item) in items.iter().enumerate() {
        out.push_str(&format!(
            "## {}. [{}] {} — {} — lines {}-{}\n",
            number + 1,
            item.id,
            item.kind.label(),
            item.status.label(),
            item.anchor.line_start,
            item.anchor.line_end
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
                push_quote(&mut out, &item.anchor.quote);
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
    out
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
        let prompt = export_prompt(&store, false);
        assert!(prompt.contains("# mdreview review"));
        assert!(prompt.contains("- Status: feedback"));
        assert!(prompt.contains(&format!("[{}] comment", id)));
        assert!(prompt.contains("> Hello world."));
        assert!(prompt.contains("mdreview resolve spec.md c1"));
    }

    #[test]
    fn approved_prompt() {
        let document = Document::from_source("spec.md", "# Plan\n\nOk.\n".to_string());
        let mut store = ReviewStore::new("spec.md", document.hash.clone());
        store.approve();
        let prompt = export_prompt(&store, false);
        let expected = format!(
            "# mdreview review\n\n- File: spec.md\n- Round: 1\n- Document hash: {}\n- Status: approved\n- Open items: 0\n\nThe reviewer approved this revision. No changes are required.\n",
            document.hash
        );
        assert_eq!(prompt, expected);
        assert_eq!(export_exit_code(&store, false), 0);
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
    fn suggestion_prompt_includes_replacement() {
        let source = "# A\n\nold wording here\n";
        let document = Document::from_source("spec.md", source.to_string());
        let start = source.find("old wording here").unwrap();
        let anchor =
            Anchor::from_range(&document, start..start + "old wording here".len()).unwrap();
        let mut store = ReviewStore::new("spec.md", document.hash.clone());
        store.add_suggestion(anchor, "prefer this", "old wording", "new wording");
        let prompt = export_prompt(&store, false);
        assert!(prompt.contains("Original:\nold wording\nReplacement:\nnew wording\n"));
        assert!(!prompt.contains("> old wording"));
    }
}
