use agentgloss::document::{Document, layout};
use std::fmt::Write as _;
use std::time::{Duration, Instant};

#[test]
fn two_thousand_line_document_parses_and_lays_out_quickly() {
    let mut source = String::new();
    for index in 0..600 {
        writeln!(
            source,
            "## Section {index}\n\nParagraph {index} explains the behavior with a [link](https://example.com).\n\n- first item\n- second item\n\n```rust\nfn example() {{}}\n```\n"
        )
        .unwrap();
    }
    assert!(
        source.lines().count() >= 2_000,
        "{}",
        source.lines().count()
    );

    let start = Instant::now();
    let document = Document::from_source("perf.md", source);
    let lines = layout(&document, 100);
    let elapsed = start.elapsed();

    assert!(!document.blocks.is_empty());
    assert!(!lines.is_empty());
    assert!(
        elapsed < Duration::from_secs(2),
        "parse + layout took {elapsed:?}"
    );
}
