//! ICU word boundary segmentation.
//!
//! Wraps ICU's `UBreakIterator` to segment text into words, filtering out
//! whitespace and punctuation segments. Returns byte offset spans in UTF-8.

use rust_icu_ubrk::UBreakIterator;

/// Default breaking rules, copied from Lucene's ICU integration.
const DEFAULT_RULES: &str = include_str!("../Default.rbbi");

/// A character is "searchable" if it's not just whitespace or punctuation.
fn is_searchable_char(c: char) -> bool {
    if c.is_alphanumeric() {
        return true;
    }
    if c.is_whitespace() || c.is_ascii_punctuation() || c.is_control() {
        return false;
    }
    if is_unicode_punctuation(c) {
        return false;
    }
    // Everything else above ASCII is searchable: emoji, symbols, CJK radicals, etc.
    c as u32 > 0x7F
}

fn is_unicode_punctuation(c: char) -> bool {
    matches!(c,
        // General Punctuation
        '\u{2010}'..='\u{2027}' | '\u{2030}'..='\u{205E}'
        // CJK Symbols and Punctuation
        | '\u{3000}'..='\u{3003}' | '\u{3008}'..='\u{3011}' | '\u{3014}'..='\u{301F}'
        // CJK Compatibility Forms
        | '\u{FE30}'..='\u{FE4F}'
        // Small Form Variants
        | '\u{FE50}'..='\u{FE6B}'
        // Fullwidth punctuation (mapped from ASCII)
        | '\u{FF01}'..='\u{FF0F}' | '\u{FF1A}'..='\u{FF20}'
        | '\u{FF3B}'..='\u{FF3D}' | '\u{FF5B}'..='\u{FF65}'
        // Miscellaneous individual punctuation chars
        | '\u{00A1}' | '\u{00A7}' | '\u{00AB}' | '\u{00B6}' | '\u{00B7}' | '\u{00BB}' | '\u{00BF}'
    )
}

/// Segments `text` into words using ICU's rule-based break iterator.
///
/// Returns a list of `(start_byte, end_byte)` spans for each word segment,
/// filtering out whitespace-only and punctuation-only segments.
pub fn icu_word_break(text: &str) -> Vec<(usize, usize)> {
    if text.is_empty() {
        return Vec::new();
    }

    // Build UTF-16 code unit index → UTF-8 byte offset mapping.
    let mut utf16_to_byte = Vec::new();
    let mut byte_offset = 0;
    for ch in text.chars() {
        for _ in 0..ch.len_utf16() {
            utf16_to_byte.push(byte_offset);
        }
        byte_offset += ch.len_utf8();
    }
    utf16_to_byte.push(byte_offset); // sentinel

    let utf16_to_byte_offset = |utf16_pos: i32| -> usize {
        let pos = utf16_pos as usize;
        if pos < utf16_to_byte.len() {
            utf16_to_byte[pos]
        } else {
            text.len()
        }
    };

    let mut break_iter =
        UBreakIterator::try_new_rules(DEFAULT_RULES, text).expect("ICU break iterator init failed");

    let mut spans = Vec::new();

    loop {
        let start_utf16 = break_iter.current();
        let end_utf16 = match break_iter.next() {
            Some(e) => e,
            None => break,
        };

        let start_byte = utf16_to_byte_offset(start_utf16);
        let end_byte = utf16_to_byte_offset(end_utf16);
        let word = &text[start_byte..end_byte];

        if word.chars().any(is_searchable_char) {
            spans.push((start_byte, end_byte));
        }
    }

    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    fn break_texts<'a>(text: &'a str) -> Vec<&'a str> {
        icu_word_break(text)
            .iter()
            .map(|&(s, e)| &text[s..e])
            .collect()
    }

    #[test]
    fn word_break_latin() {
        assert_eq!(break_texts("hello world"), &["hello", "world"]);
    }

    #[test]
    fn word_break_cjk_mixed() {
        let tokens = break_texts("hello你好world");
        assert!(tokens.contains(&"hello"));
        assert!(tokens.contains(&"world"));
    }

    #[test]
    fn word_break_supplementary_plane() {
        let text = "𠮷野家";
        let spans = icu_word_break(text);
        for (s, e) in spans {
            assert!(text.is_char_boundary(s));
            assert!(text.is_char_boundary(e));
        }
    }

    #[test]
    fn word_break_empty() {
        assert!(icu_word_break("").is_empty());
    }

    #[test]
    fn word_break_whitespace_only() {
        assert!(icu_word_break("   \t\n  ").is_empty());
    }

    #[test]
    fn word_break_punctuation_only() {
        assert!(icu_word_break("!!!???").is_empty());
    }

    #[test]
    fn word_break_emoji() {
        let text = "hello🎉world";
        let tokens = break_texts(text);
        assert!(tokens.contains(&"hello"));
        assert!(tokens.contains(&"world"));
        // Emoji should also be a token.
        assert!(tokens.contains(&"🎉"));
    }
}
