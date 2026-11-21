//! Review state: comments, suggestions, anchors, rounds, and sidecar persistence.

pub mod export;

use crate::document::Document;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::ops::Range;
use std::path::{Path, PathBuf};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

pub const SIDECAR_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Comment,
    Suggestion,
}

impl ItemKind {
    pub fn label(self) -> &'static str {
        match self {
            ItemKind::Comment => "comment",
            ItemKind::Suggestion => "suggestion",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemStatus {
    Open,
    Resolved,
    Stale,
}

impl ItemStatus {
    pub fn label(self) -> &'static str {
        match self {
            ItemStatus::Open => "open",
            ItemStatus::Resolved => "resolved",
            ItemStatus::Stale => "stale",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignoffKind {
    Feedback,
    Approved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Approval {
    pub hash: String,
    pub at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Suggestion {
    pub original: String,
    pub replacement: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Anchor {
    pub heading_path: Vec<String>,
    pub block_index: usize,
    pub quote: String,
    pub prefix: String,
    pub suffix: String,
    pub source_start: usize,
    pub source_end: usize,
    pub line_start: usize,
    pub line_end: usize,
    pub doc_hash: String,
}

impl Anchor {
    pub fn range(&self) -> Range<usize> {
        self.source_start..self.source_end
    }

    pub fn lines(&self) -> Range<usize> {
        self.line_start..self.line_end
    }

    pub fn from_range(document: &Document, range: Range<usize>) -> Result<Self, ReviewError> {
        let start = clamp_boundary(&document.source, range.start);
        let end = clamp_boundary(&document.source, range.end.max(start));
        let block = document.block_at(start).ok_or(ReviewError::EmptyDocument)?;
        let (start, mut end) = if start == end {
            (block.source.start, block.source.end.max(block.source.start))
        } else {
            (start, end)
        };
        if end > start && document.source.as_bytes()[end - 1] == b'\n' {
            end -= 1;
            if end > start && document.source.as_bytes()[end - 1] == b'\r' {
                end -= 1;
            }
        }
        let quote = document.source[start..end].to_string();
        if quote.trim().is_empty() {
            return Err(ReviewError::EmptyAnchor);
        }
        let lines = document.lines_of(start..end);
        Ok(Self {
            heading_path: block.heading_path.clone(),
            block_index: block.index,
            quote,
            prefix: context_before(&document.source, start, 32),
            suffix: context_after(&document.source, end, 32),
            source_start: start,
            source_end: end,
            line_start: lines.start,
            line_end: lines.end,
            doc_hash: document.hash.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnchorEvent {
    pub round: u32,
    pub method: String,
    pub old_start: usize,
    pub new_start: usize,
    pub at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Draft {
    pub kind: ItemKind,
    pub body: String,
    #[serde(default)]
    pub original: Option<String>,
    pub anchor: Anchor,
    #[serde(default)]
    pub editing_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewItem {
    pub id: String,
    pub kind: ItemKind,
    pub status: ItemStatus,
    pub body: String,
    pub created_round: u32,
    pub resolved_round: Option<u32>,
    pub created_at: String,
    pub updated_at: String,
    pub anchor: Anchor,
    #[serde(default)]
    pub suggestion: Option<Suggestion>,
    #[serde(default)]
    pub history: Vec<AnchorEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewStore {
    pub version: u32,
    pub file: String,
    pub doc_hash: String,
    pub round: u32,
    #[serde(default)]
    pub requested_at: Option<String>,
    #[serde(default)]
    pub last_signoff: Option<SignoffKind>,
    #[serde(default)]
    pub approval: Option<Approval>,
    #[serde(default = "default_next_id")]
    pub next_id: u32,
    #[serde(default)]
    pub items: Vec<ReviewItem>,
    #[serde(default)]
    pub draft: Option<Draft>,
}

fn default_next_id() -> u32 {
    1
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ReanchorReport {
    pub updated: usize,
    pub stale: usize,
    pub unchanged: usize,
}

impl ReanchorReport {
    pub fn total(&self) -> usize {
        self.updated + self.stale + self.unchanged
    }
}

#[derive(Debug, Clone)]
pub struct LoadOutcome {
    pub store: ReviewStore,
    pub recovered: bool,
    pub backup_path: Option<PathBuf>,
}

impl ReviewStore {
    pub fn new(file: impl Into<String>, doc_hash: impl Into<String>) -> Self {
        Self {
            version: SIDECAR_VERSION,
            file: file.into(),
            doc_hash: doc_hash.into(),
            round: 1,
            requested_at: None,
            last_signoff: None,
            approval: None,
            next_id: 1,
            items: Vec::new(),
            draft: None,
        }
    }

    pub fn load(path: &Path) -> Result<Self, ReviewError> {
        let raw = std::fs::read_to_string(path).map_err(|source| ReviewError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let store: Self = serde_json::from_str(&raw).map_err(|source| ReviewError::Json {
            path: path.to_path_buf(),
            source,
        })?;
        if store.version != SIDECAR_VERSION {
            return Err(ReviewError::UnsupportedVersion {
                found: store.version,
                supported: SIDECAR_VERSION,
            });
        }
        Ok(store)
    }

    pub fn load_or_recover(
        path: &Path,
        file: &str,
        doc_hash: &str,
    ) -> Result<LoadOutcome, ReviewError> {
        if !path.exists() {
            return Ok(LoadOutcome {
                store: Self::new(file, doc_hash),
                recovered: false,
                backup_path: None,
            });
        }
        match Self::load(path) {
            Ok(store) => Ok(LoadOutcome {
                store,
                recovered: false,
                backup_path: None,
            }),
            Err(error @ ReviewError::UnsupportedVersion { .. }) => Err(error),
            Err(_) => {
                let backup_path = backup_path(path);
                std::fs::rename(path, &backup_path).map_err(|source| ReviewError::Io {
                    path: path.to_path_buf(),
                    source,
                })?;
                Ok(LoadOutcome {
                    store: Self::new(file, doc_hash),
                    recovered: true,
                    backup_path: Some(backup_path),
                })
            }
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), ReviewError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| ReviewError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let json = serde_json::to_string_pretty(self)?;
        let tmp = temp_path(path);
        {
            let mut file = File::create(&tmp).map_err(|source| ReviewError::Io {
                path: tmp.clone(),
                source,
            })?;
            file.write_all(json.as_bytes())
                .and_then(|_| file.write_all(b"\n"))
                .and_then(|_| file.sync_all())
                .map_err(|source| ReviewError::Io {
                    path: tmp.clone(),
                    source,
                })?;
        }
        std::fs::rename(&tmp, path).map_err(|source| ReviewError::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    pub fn request(&mut self) {
        self.requested_at = Some(now());
        self.last_signoff = None;
        self.approval = None;
    }

    pub fn set_draft(&mut self, draft: Draft) {
        self.draft = Some(draft);
    }

    pub fn clear_draft(&mut self) {
        self.draft = None;
    }

    pub fn add_comment(&mut self, anchor: Anchor, body: impl Into<String>) -> String {
        self.add_item(ItemKind::Comment, anchor, body.into(), None)
    }

    pub fn add_suggestion(
        &mut self,
        anchor: Anchor,
        body: impl Into<String>,
        original: impl Into<String>,
        replacement: impl Into<String>,
    ) -> String {
        let suggestion = Suggestion {
            original: original.into(),
            replacement: replacement.into(),
        };
        self.add_item(ItemKind::Suggestion, anchor, body.into(), Some(suggestion))
    }

    fn add_item(
        &mut self,
        kind: ItemKind,
        anchor: Anchor,
        body: String,
        suggestion: Option<Suggestion>,
    ) -> String {
        let id = format!("c{}", self.next_id);
        self.next_id += 1;
        let at = now();
        self.items.push(ReviewItem {
            id: id.clone(),
            kind,
            status: ItemStatus::Open,
            body,
            created_round: self.round,
            resolved_round: None,
            created_at: at.clone(),
            updated_at: at.clone(),
            history: vec![AnchorEvent {
                round: self.round,
                method: "created".to_string(),
                old_start: anchor.source_start,
                new_start: anchor.source_start,
                at,
            }],
            anchor,
            suggestion,
        });
        id
    }

    pub fn item(&self, id: &str) -> Option<&ReviewItem> {
        self.items.iter().find(|item| item.id == id)
    }

    pub fn item_mut(&mut self, id: &str) -> Option<&mut ReviewItem> {
        self.items.iter_mut().find(|item| item.id == id)
    }

    pub fn resolve(&mut self, id: &str) -> Result<(), ReviewError> {
        let round = self.round;
        let at = now();
        let item = self
            .item_mut(id)
            .ok_or_else(|| ReviewError::ItemNotFound(id.to_string()))?;
        let start = item.anchor.source_start;
        item.status = ItemStatus::Resolved;
        item.resolved_round = Some(round);
        item.updated_at = at.clone();
        item.history.push(AnchorEvent {
            round,
            method: "resolved".to_string(),
            old_start: start,
            new_start: start,
            at,
        });
        Ok(())
    }

    pub fn unresolve(&mut self, id: &str) -> Result<(), ReviewError> {
        let at = now();
        let round = self.round;
        let item = self
            .item_mut(id)
            .ok_or_else(|| ReviewError::ItemNotFound(id.to_string()))?;
        if item.status == ItemStatus::Stale {
            return Err(ReviewError::StaleItem(id.to_string()));
        }
        let start = item.anchor.source_start;
        item.status = ItemStatus::Open;
        item.resolved_round = None;
        item.updated_at = at.clone();
        item.history.push(AnchorEvent {
            round,
            method: "unresolved".to_string(),
            old_start: start,
            new_start: start,
            at,
        });
        Ok(())
    }

    pub fn remove(&mut self, id: &str) -> Result<(), ReviewError> {
        let before = self.items.len();
        self.items.retain(|item| item.id != id);
        if self.items.len() == before {
            return Err(ReviewError::ItemNotFound(id.to_string()));
        }
        Ok(())
    }

    pub fn reattach(&mut self, id: &str, anchor: Anchor) -> Result<(), ReviewError> {
        let round = self.round;
        let new_original = anchor.quote.clone();
        let item = self
            .item_mut(id)
            .ok_or_else(|| ReviewError::ItemNotFound(id.to_string()))?;
        let old_start = item.anchor.source_start;
        item.anchor = anchor;
        if item.kind == ItemKind::Suggestion
            && let Some(suggestion) = item.suggestion.as_mut()
        {
            suggestion.original = new_original;
        }
        item.status = ItemStatus::Open;
        item.updated_at = now();
        item.history.push(AnchorEvent {
            round,
            method: "reattached".to_string(),
            old_start,
            new_start: item.anchor.source_start,
            at: now(),
        });
        Ok(())
    }

    pub fn open_count(&self) -> usize {
        self.items
            .iter()
            .filter(|item| item.status == ItemStatus::Open)
            .count()
    }

    pub fn stale_count(&self) -> usize {
        self.items
            .iter()
            .filter(|item| item.status == ItemStatus::Stale)
            .count()
    }

    pub fn resolved_count(&self) -> usize {
        self.items
            .iter()
            .filter(|item| item.status == ItemStatus::Resolved)
            .count()
    }

    pub fn has_feedback(&self) -> bool {
        self.open_count() + self.stale_count() > 0
    }

    pub fn is_approved(&self) -> bool {
        self.approval
            .as_ref()
            .is_some_and(|approval| approval.hash == self.doc_hash)
            && !self.has_feedback()
    }

    pub fn feedback_items(&self, include_resolved: bool) -> Vec<&ReviewItem> {
        let mut items: Vec<_> = self
            .items
            .iter()
            .filter(|item| include_resolved || item.status != ItemStatus::Resolved)
            .collect();
        items.sort_by_key(|item| (item.anchor.source_start, item_number(&item.id)));
        items
    }

    pub fn signoff_feedback(&mut self) {
        self.last_signoff = Some(SignoffKind::Feedback);
        self.approval = None;
    }

    pub fn approve(&mut self) {
        self.last_signoff = Some(SignoffKind::Approved);
        self.approval = Some(Approval {
            hash: self.doc_hash.clone(),
            at: now(),
        });
    }

    pub fn start_round_if_changed(&mut self, document: &Document) -> ReanchorReport {
        if self.doc_hash == document.hash {
            return ReanchorReport::default();
        }
        let was_signed_off = self.last_signoff.is_some();
        if was_signed_off {
            self.round += 1;
            self.last_signoff = None;
        }
        self.doc_hash = document.hash.clone();
        self.approval = None;
        self.reanchor(document)
    }

    pub fn reanchor(&mut self, document: &Document) -> ReanchorReport {
        let mut report = ReanchorReport::default();
        for item in &mut self.items {
            if item.status == ItemStatus::Resolved {
                report.unchanged += 1;
                continue;
            }
            let old_start = item.anchor.source_start;
            let Some((new_anchor, method)) = reanchor_anchor(&item.anchor, document) else {
                if item.status != ItemStatus::Stale {
                    item.status = ItemStatus::Stale;
                    item.updated_at = now();
                    item.history.push(AnchorEvent {
                        round: self.round,
                        method: "stale".to_string(),
                        old_start,
                        new_start: old_start,
                        at: now(),
                    });
                }
                report.stale += 1;
                continue;
            };
            item.anchor = new_anchor;
            item.updated_at = now();
            if let Some(suggestion) = item.suggestion.as_ref() {
                let text = document.text_in_range(item.anchor.range());
                if !text.contains(&suggestion.original) {
                    item.status = ItemStatus::Stale;
                } else if item.status == ItemStatus::Stale {
                    item.status = ItemStatus::Open;
                }
            } else if item.status == ItemStatus::Stale {
                item.status = ItemStatus::Open;
            }
            item.history.push(AnchorEvent {
                round: self.round,
                method: method.to_string(),
                old_start,
                new_start: item.anchor.source_start,
                at: now(),
            });
            report.updated += 1;
        }
        report
    }
}

pub fn sidecar_path(document: &Path) -> PathBuf {
    let mut value = document.as_os_str().to_os_string();
    value.push(".review.json");
    PathBuf::from(value)
}

pub fn lock_path(sidecar: &Path) -> PathBuf {
    let mut value = sidecar.as_os_str().to_os_string();
    value.push(".lock");
    PathBuf::from(value)
}

pub struct StoreLock {
    file: File,
}

impl StoreLock {
    pub fn acquire(sidecar: &Path) -> Result<Self, ReviewError> {
        let path = lock_path(sidecar);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| ReviewError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|source| ReviewError::Io {
                path: path.clone(),
                source,
            })?;
        file.try_lock_exclusive()
            .map_err(|_| ReviewError::Locked { path: path.clone() })?;
        Ok(Self { file })
    }
}

impl Drop for StoreLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

pub fn now() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

pub fn item_number(id: &str) -> u64 {
    id.trim_start_matches('c').parse().unwrap_or(0)
}

fn backup_path(path: &Path) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    PathBuf::from(format!("{}.bak.{}", path.display(), stamp))
}

fn temp_path(path: &Path) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    PathBuf::from(format!(
        "{}.tmp.{}.{}",
        path.display(),
        std::process::id(),
        stamp
    ))
}

fn clamp_boundary(source: &str, mut index: usize) -> usize {
    index = index.min(source.len());
    while index > 0 && !source.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn context_before(source: &str, start: usize, count: usize) -> String {
    let mut chars: Vec<char> = source[..start].chars().rev().take(count).collect();
    chars.reverse();
    chars.into_iter().collect()
}

fn context_after(source: &str, end: usize, count: usize) -> String {
    source[end..].chars().take(count).collect()
}

fn find_all(haystack: &str, needle: &str) -> Vec<usize> {
    if needle.is_empty() {
        return Vec::new();
    }
    let mut matches = Vec::new();
    let mut start = 0;
    while let Some(found) = haystack[start..].find(needle) {
        let index = start + found;
        matches.push(index);
        start = index + 1;
        while start < haystack.len() && !haystack.is_char_boundary(start) {
            start += 1;
        }
        if start >= haystack.len() {
            break;
        }
    }
    matches
}

fn context_score(anchor: &Anchor, source: &str, position: usize, quote_len: usize) -> usize {
    let mut score = 0;
    if !anchor.prefix.is_empty() && source[..position].ends_with(&anchor.prefix) {
        score += 2;
    }
    let end = position + quote_len;
    if !anchor.suffix.is_empty() && end <= source.len() && source[end..].starts_with(&anchor.suffix)
    {
        score += 2;
    }
    score
}

fn pick_candidate(
    anchor: &Anchor,
    source: &str,
    candidates: &[usize],
    quote_len: usize,
) -> Option<usize> {
    candidates.iter().copied().max_by_key(|position| {
        let score = context_score(anchor, source, *position, quote_len);
        let distance = (*position as isize - anchor.source_start as isize).unsigned_abs();
        (score, std::cmp::Reverse(distance))
    })
}

fn normalized_with_map(source: &str) -> (String, Vec<usize>) {
    let mut normalized = String::new();
    let mut map = Vec::new();
    let mut in_space = false;
    for (index, ch) in source.char_indices() {
        if ch.is_whitespace() {
            if !in_space {
                normalized.push(' ');
                for _ in 0..' '.len_utf8() {
                    map.push(index);
                }
                in_space = true;
            }
        } else {
            in_space = false;
            normalized.push(ch);
            for _ in 0..ch.len_utf8() {
                map.push(index);
            }
        }
    }
    (normalized, map)
}

fn normalize_needle(needle: &str) -> String {
    let mut out = String::new();
    let mut in_space = false;
    for ch in needle.chars() {
        if ch.is_whitespace() {
            if !in_space {
                out.push(' ');
                in_space = true;
            }
        } else {
            in_space = false;
            out.push(ch);
        }
    }
    out
}

fn token_set(text: &str) -> HashSet<String> {
    text.split(|ch: char| !ch.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| token.to_lowercase())
        .collect()
}

fn jaccard(left: &HashSet<String>, right: &HashSet<String>) -> f64 {
    if left.is_empty() && right.is_empty() {
        return 1.0;
    }
    let intersection = left.intersection(right).count() as f64;
    let union = left.union(right).count() as f64;
    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}

fn new_anchor(
    document: &Document,
    range: Range<usize>,
    block_index: usize,
    heading_path: Vec<String>,
) -> Anchor {
    let start = clamp_boundary(&document.source, range.start);
    let end = clamp_boundary(&document.source, range.end.max(start));
    let lines = document.lines_of(start..end);
    Anchor {
        heading_path,
        block_index,
        quote: document.source[start..end].to_string(),
        prefix: context_before(&document.source, start, 32),
        suffix: context_after(&document.source, end, 32),
        source_start: start,
        source_end: end,
        line_start: lines.start,
        line_end: lines.end,
        doc_hash: document.hash.clone(),
    }
}

fn reanchor_anchor(anchor: &Anchor, document: &Document) -> Option<(Anchor, &'static str)> {
    if anchor.quote.is_empty() {
        return None;
    }
    let source = &document.source;

    let exact = find_all(source, &anchor.quote);
    if let Some(position) = pick_candidate(anchor, source, &exact, anchor.quote.len()) {
        let range = position..(position + anchor.quote.len());
        let block = document.block_at(position)?;
        return Some((
            new_anchor(document, range, block.index, block.heading_path.clone()),
            "exact",
        ));
    }

    let (normalized_source, map) = normalized_with_map(source);
    let needle = normalize_needle(&anchor.quote);
    let normalized_matches = find_all(&normalized_source, &needle);
    if let Some(position) = pick_candidate(
        anchor,
        &normalized_source,
        &normalized_matches,
        needle.len(),
    ) {
        let normalized_end = (position + needle.len()).min(map.len());
        let start = *map.get(position)?;
        let end = map.get(normalized_end).copied().unwrap_or(source.len());
        let block = document.block_at(start)?;
        return Some((
            new_anchor(
                document,
                start..end,
                block.index,
                block.heading_path.clone(),
            ),
            "normalized",
        ));
    }

    let quote_tokens = token_set(&anchor.quote);
    let mut best: Option<(&crate::document::Block, f64)> = None;
    for block in &document.blocks {
        if block.heading_path != anchor.heading_path {
            continue;
        }
        let score = jaccard(&quote_tokens, &token_set(&block.text()));
        let better = best.is_none_or(|(_, best_score)| score > best_score);
        if better {
            best = Some((block, score));
        }
    }
    if let Some((block, score)) = best
        && score >= 0.5
    {
        return Some((
            new_anchor(
                document,
                block.source.clone(),
                block.index,
                block.heading_path.clone(),
            ),
            "block",
        ));
    }

    None
}

#[derive(Debug, thiserror::Error)]
pub enum ReviewError {
    #[error("cannot read/write {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid review JSON at {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("unsupported sidecar version {found}; supported version is {supported}")]
    UnsupportedVersion { found: u32, supported: u32 },
    #[error("review state is locked: {path}")]
    Locked { path: PathBuf },
    #[error("review item not found: {0}")]
    ItemNotFound(String),
    #[error("stale item cannot be reopened; reattach it first: {0}")]
    StaleItem(String),
    #[error("the document is empty")]
    EmptyDocument,
    #[error("the selected range has no reviewable text")]
    EmptyAnchor,
    #[error("serialization failed: {0}")]
    Serialize(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(source: &str) -> Document {
        Document::from_source("spec.md", source.to_string())
    }

    fn anchor_for(document: &Document, needle: &str) -> Anchor {
        let start = document.source.find(needle).expect("needle");
        Anchor::from_range(document, start..(start + needle.len())).expect("anchor")
    }

    #[test]
    fn anchor_roundtrip_and_sidecar() {
        let document = doc("# Plan\n\nHello world.\n");
        let mut store = ReviewStore::new("spec.md", document.hash.clone());
        let anchor = anchor_for(&document, "Hello world.");
        let id = store.add_comment(anchor, "Please clarify.");
        assert_eq!(id, "c1");
        assert_eq!(store.open_count(), 1);
        assert!(store.has_feedback());

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("spec.md.review.json");
        store.save(&path).unwrap();
        let loaded = ReviewStore::load(&path).unwrap();
        assert_eq!(loaded.items.len(), 1);
        assert_eq!(loaded.items[0].body, "Please clarify.");
        assert_eq!(loaded.version, SIDECAR_VERSION);
    }

    #[test]
    fn exact_and_moved_reanchoring() {
        let old = doc("# Plan\n\nHello world.\n");
        let anchor = anchor_for(&old, "Hello world.");
        let mut store = ReviewStore::new("spec.md", old.hash.clone());
        store.add_comment(anchor, "note");

        let new = doc("# Plan\n\nIntro.\n\nHello world.\n");
        store.start_round_if_changed(&new);
        assert_eq!(store.round, 1);
        let item = &store.items[0];
        assert_eq!(item.status, ItemStatus::Open);
        assert!(new.source[item.anchor.range()].contains("Hello world."));
    }

    #[test]
    fn duplicate_quotes_use_context() {
        let old = doc("one two\n\nhello\n\nthree hello\n");
        let first = old.source.find("hello").unwrap();
        let anchor = Anchor::from_range(&old, first..first + 5).unwrap();
        let mut store = ReviewStore::new("spec.md", old.hash.clone());
        store.add_comment(anchor, "first");

        let new = doc("zero hello\n\none two\n\nhello\n\nthree hello\n");
        store.reanchor(&new);
        let item = &store.items[0];
        assert_eq!(item.status, ItemStatus::Open);
        assert!(new.source[item.anchor.range()].contains("hello"));
    }

    #[test]
    fn whitespace_and_block_fallback() {
        let old = doc("# A\n\nretry policy backoff\n");
        let anchor = anchor_for(&old, "retry policy backoff");
        let mut store = ReviewStore::new("spec.md", old.hash.clone());
        store.add_comment(anchor, "note");

        let new = doc("# A\n\nretry   policy\nbackoff and jitter\n");
        store.reanchor(&new);
        assert_eq!(store.items[0].status, ItemStatus::Open);
        assert!(new.source[store.items[0].anchor.range()].contains("retry"));
    }

    #[test]
    fn inserted_and_deleted_text_fall_back_to_the_block() {
        let old = doc("# Plan\n\nalpha beta gamma delta\n");
        let anchor = anchor_for(&old, "alpha beta gamma delta");
        let mut inserted_store = ReviewStore::new("spec.md", old.hash.clone());
        inserted_store.add_comment(anchor.clone(), "note");
        let inserted = doc("# Plan\n\nalpha beta INSERT gamma delta\n");
        inserted_store.reanchor(&inserted);
        let item = &inserted_store.items[0];
        assert_eq!(item.status, ItemStatus::Open);
        assert_eq!(item.history.last().unwrap().method, "block");
        assert!(inserted.source[item.anchor.range()].contains("INSERT"));

        let mut deleted_store = ReviewStore::new("spec.md", old.hash.clone());
        deleted_store.add_comment(anchor, "note");
        let deleted = doc("# Plan\n\nalpha beta delta\n");
        deleted_store.reanchor(&deleted);
        let item = &deleted_store.items[0];
        assert_eq!(item.status, ItemStatus::Open);
        assert_eq!(item.history.last().unwrap().method, "block");
        assert!(deleted.source[item.anchor.range()].contains("alpha beta delta"));
    }

    #[test]
    fn heading_moves_update_the_heading_path() {
        let old = doc("# A\n\ntext here\n");
        let anchor = anchor_for(&old, "text here");
        let mut store = ReviewStore::new("spec.md", old.hash.clone());
        store.add_comment(anchor, "note");
        let new = doc("# B\n\ntext here\n");
        store.reanchor(&new);
        let item = &store.items[0];
        assert_eq!(item.status, ItemStatus::Open);
        assert_eq!(item.anchor.heading_path, vec!["B"]);
        assert_eq!(item.history.last().unwrap().method, "exact");
    }

    #[test]
    fn unicode_quotes_and_normalized_whitespace_reanchor() {
        let old = doc("# Café ☕\n\nnaïve  你好  👋\n");
        let anchor = anchor_for(&old, "naïve  你好  👋");
        let mut store = ReviewStore::new("spec.md", old.hash.clone());
        store.add_comment(anchor, "note");
        let new = doc("# Café ☕\n\nintro\n\nnaïve 你好 👋\n");
        store.reanchor(&new);
        let item = &store.items[0];
        assert_eq!(item.status, ItemStatus::Open);
        assert_eq!(item.history.last().unwrap().method, "normalized");
        assert!(new.source[item.anchor.range()].contains("👋"));
    }

    #[test]
    fn jaccard_fallback_requires_half_the_tokens() {
        let old = doc("# Plan\n\nalpha beta gamma delta\n");
        let anchor = anchor_for(&old, "alpha beta gamma delta");
        let mut store = ReviewStore::new("spec.md", old.hash.clone());
        store.add_comment(anchor, "note");
        let new = doc("# Plan\n\nalpha beta gamma epsilon zeta\n");
        store.reanchor(&new);
        let item = &store.items[0];
        assert_eq!(item.status, ItemStatus::Open);
        assert_eq!(item.history.last().unwrap().method, "block");
    }

    #[test]
    fn missing_text_goes_stale() {
        let old = doc("# A\n\nplease remove this paragraph\n");
        let anchor = anchor_for(&old, "please remove this paragraph");
        let mut store = ReviewStore::new("spec.md", old.hash.clone());
        store.add_comment(anchor, "note");

        let new = doc("# A\n\ncompletely different words now\n");
        store.reanchor(&new);
        assert_eq!(store.items[0].status, ItemStatus::Stale);
        assert_eq!(store.stale_count(), 1);
    }

    #[test]
    fn suggestion_becomes_stale_when_original_disappears() {
        let old = doc("# A\n\nuse old wording here\n");
        let anchor = anchor_for(&old, "use old wording here");
        let mut store = ReviewStore::new("spec.md", old.hash.clone());
        store.add_suggestion(anchor, "wording", "old wording", "new wording");

        let new = doc("# A\n\nuse replaced phrasing here\n");
        store.reanchor(&new);
        assert_eq!(store.items[0].status, ItemStatus::Stale);
    }

    #[test]
    fn reattach_refreshes_suggestion_original() {
        let old = doc("# A\n\nold target\n");
        let anchor = anchor_for(&old, "old target");
        let mut store = ReviewStore::new("spec.md", old.hash.clone());
        let id = store.add_suggestion(anchor, "", "old target", "replacement");
        let new = doc("# A\n\nnew selected text\n");
        let new_anchor = anchor_for(&new, "new selected text");
        store.items[0].status = ItemStatus::Stale;
        store.reattach(&id, new_anchor).unwrap();
        let suggestion = store.items[0].suggestion.as_ref().unwrap();
        assert_eq!(suggestion.original, "new selected text");
        store.reanchor(&new);
        assert_eq!(store.items[0].status, ItemStatus::Open);
    }

    #[test]
    fn approval_invalidated_by_change() {
        let old = doc("# A\n\nHello.\n");
        let mut store = ReviewStore::new("spec.md", old.hash.clone());
        store.approve();
        assert!(store.is_approved());

        let new = doc("# A\n\nHello changed.\n");
        store.start_round_if_changed(&new);
        assert!(!store.is_approved());
        assert_eq!(store.round, 2);
    }

    #[test]
    fn resolve_unresolve_and_remove() {
        let document = doc("# A\n\nHello.\n");
        let anchor = anchor_for(&document, "Hello.");
        let mut store = ReviewStore::new("spec.md", document.hash.clone());
        let id = store.add_comment(anchor, "note");
        store.resolve(&id).unwrap();
        assert_eq!(store.resolved_count(), 1);
        store.unresolve(&id).unwrap();
        assert_eq!(store.open_count(), 1);
        store.remove(&id).unwrap();
        assert!(store.items.is_empty());
        assert!(matches!(
            store.remove("nope"),
            Err(ReviewError::ItemNotFound(_))
        ));
    }

    #[test]
    fn corrupt_sidecar_is_backed_up() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("spec.md.review.json");
        std::fs::write(&path, "{not json").unwrap();
        let outcome = ReviewStore::load_or_recover(&path, "spec.md", "hash").unwrap();
        assert!(outcome.recovered);
        assert!(outcome.backup_path.unwrap().exists());
        assert!(outcome.store.items.is_empty());
    }

    #[test]
    fn unsupported_version_is_rejected_without_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("spec.md.review.json");
        let store = ReviewStore::new("spec.md", "hash");
        store.save(&path).unwrap();
        let mut value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        value["version"] = serde_json::json!(99);
        std::fs::write(&path, serde_json::to_string_pretty(&value).unwrap()).unwrap();
        assert!(matches!(
            ReviewStore::load(&path),
            Err(ReviewError::UnsupportedVersion { found: 99, .. })
        ));
        assert!(ReviewStore::load_or_recover(&path, "spec.md", "hash").is_err());
        assert!(path.exists());
    }

    #[test]
    fn lock_prevents_a_second_writer() {
        let dir = tempfile::tempdir().unwrap();
        let sidecar = dir.path().join("spec.md.review.json");
        let _lock = StoreLock::acquire(&sidecar).unwrap();
        assert!(matches!(
            StoreLock::acquire(&sidecar),
            Err(ReviewError::Locked { .. })
        ));
    }
}
