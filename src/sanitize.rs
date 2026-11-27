//! Display-only sanitization for terminal output.
//!
//! Source text, hashes, quotes, and anchors must stay byte-exact; this module
//! is only applied when text is about to be drawn or printed. Control
//! characters become visible control pictures so a hostile document cannot
//! inject terminal escape sequences.

use std::borrow::Cow;

pub fn sanitize_terminal_text(input: &str) -> Cow<'_, str> {
    if !input.chars().any(needs_escape) {
        return Cow::Borrowed(input);
    }
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        out.push(replacement(ch).unwrap_or(ch));
    }
    Cow::Owned(out)
}

fn needs_escape(ch: char) -> bool {
    matches!(
        ch,
        '\u{0}'..='\u{8}'
            | '\u{b}'
            | '\u{c}'
            | '\u{e}'..='\u{1f}'
            | '\u{7f}'
            | '\u{80}'..='\u{9f}'
    )
}

fn replacement(ch: char) -> Option<char> {
    match ch {
        '\t' | '\n' => None,
        '\u{0}'..='\u{1f}' => char::from_u32(0x2400 + ch as u32),
        '\u{7f}' => Some('\u{2421}'),
        '\u{80}'..='\u{9f}' => Some('\u{fffd}'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_text_is_borrowed() {
        let text = "plain 你好 👋\tindented\nnext";
        assert!(matches!(sanitize_terminal_text(text), Cow::Borrowed(_)));
    }

    #[test]
    fn escapes_control_characters() {
        let hostile = "title\x1b]0;pwned\x07\r\x7f";
        let clean = sanitize_terminal_text(hostile);
        assert!(!clean.contains('\x1b'));
        assert!(!clean.contains('\x07'));
        assert!(!clean.contains('\r'));
        assert!(clean.contains('\u{241b}'));
        assert!(clean.contains('\u{2421}'));
        assert!(clean.ends_with("\u{2407}\u{240d}\u{2421}"));
    }

    #[test]
    fn replaces_c1_controls_and_keeps_layout_whitespace() {
        let clean = sanitize_terminal_text("a\u{9b}b\tc\nd");
        assert_eq!(clean, "a\u{fffd}b\tc\nd");
    }
}
