//! Property-based tests for NormalizingICUTokenizer.

use proptest::prelude::*;
use tantivy_tokenizer_api::{TokenStream, Tokenizer};

use tantivy_analyzer_icu::NormalizingICUTokenizer;

fn collect_tokens(text: &str) -> Vec<tantivy_tokenizer_api::Token> {
    let mut tokenizer = NormalizingICUTokenizer;
    let mut stream = tokenizer.token_stream(text);
    let mut out = Vec::new();
    while stream.advance() {
        out.push(stream.token().clone());
    }
    out
}

/// Strategy that generates strings with a mix of interesting Unicode characters.
fn unicode_string() -> impl Strategy<Value = String> {
    prop::string::string_regex("([a-zA-Z0-9 ,.!?]|[\u{4E00}-\u{4E20}]|[\u{3040}-\u{3060}]|[\u{AC00}-\u{AC20}]|[\u{0410}-\u{0430}]|[\u{0600}-\u{0610}]|🎉|💩|🌍|👍|\u{200B}|\u{200D}|\u{FEFF}|\u{0301}|\u{0000}|\n|\t){0,200}")
        .unwrap()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn prop_tokenizer_never_panics(s in "\\PC{0,500}") {
        let _ = collect_tokens(&s);
    }

    #[test]
    fn prop_tokenizer_offsets_valid(s in "\\PC{0,500}") {
        let tokens = collect_tokens(&s);
        for (i, tok) in tokens.iter().enumerate() {
            prop_assert!(
                tok.offset_to <= s.len(),
                "token {}: offset_to {} > len {}", i, tok.offset_to, s.len()
            );
            prop_assert!(
                tok.offset_from <= tok.offset_to,
                "token {}: offset_from {} > offset_to {}", i, tok.offset_from, tok.offset_to
            );
            prop_assert!(
                s.is_char_boundary(tok.offset_from),
                "token {}: offset_from {} not char boundary", i, tok.offset_from
            );
            prop_assert!(
                s.is_char_boundary(tok.offset_to),
                "token {}: offset_to {} not char boundary", i, tok.offset_to
            );
            // Offsets must be sliceable
            let _ = &s[tok.offset_from..tok.offset_to];
        }
    }

    #[test]
    fn prop_tokenizer_unicode_mix(s in unicode_string()) {
        let tokens = collect_tokens(&s);
        for tok in &tokens {
            prop_assert!(tok.offset_to <= s.len());
            prop_assert!(s.is_char_boundary(tok.offset_from));
            prop_assert!(s.is_char_boundary(tok.offset_to));
        }
    }

    #[test]
    fn prop_tokenizer_positions_sequential(s in "\\PC{0,200}") {
        let tokens = collect_tokens(&s);
        for (i, tok) in tokens.iter().enumerate() {
            prop_assert_eq!(tok.position, i, "position mismatch at index {}", i);
        }
    }

    #[test]
    fn prop_tokenizer_deterministic(s in "\\PC{0,100}") {
        let r1 = collect_tokens(&s);
        let r2 = collect_tokens(&s);
        prop_assert_eq!(r1.len(), r2.len());
        for (a, b) in r1.iter().zip(r2.iter()) {
            prop_assert_eq!(&a.text, &b.text);
            prop_assert_eq!(a.offset_from, b.offset_from);
            prop_assert_eq!(a.offset_to, b.offset_to);
        }
    }
}
