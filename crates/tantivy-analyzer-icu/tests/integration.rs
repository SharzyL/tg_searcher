//! Integration tests for NormalizingICUTokenizer.
//!
//! These test normalization, word breaking, CJK unigram expansion, and offset
//! correctness through the full tokenizer.

use tantivy_tokenizer_api::{Token, TokenStream, Tokenizer};

use tantivy_analyzer_icu::NormalizingICUTokenizer;

fn tokenize_full(text: &str) -> Vec<Token> {
    let mut tokenizer = NormalizingICUTokenizer;
    let mut stream = tokenizer.token_stream(text);
    let mut tokens = Vec::new();
    while stream.advance() {
        tokens.push(stream.token().clone());
    }
    tokens
}

fn tokenize(text: &str) -> Vec<String> {
    tokenize_full(text).into_iter().map(|t| t.text).collect()
}

/// Verify that every token's [offset_from, offset_to) points into valid UTF-8
/// within the original `text`.
fn assert_offsets_valid(text: &str, tokens: &[Token]) {
    for (i, tok) in tokens.iter().enumerate() {
        assert!(
            tok.offset_to <= text.len(),
            "token {i} {:?}: offset_to {} exceeds text len {}",
            tok.text,
            tok.offset_to,
            text.len()
        );
        assert!(
            tok.offset_from <= tok.offset_to,
            "token {i} {:?}: offset_from {} > offset_to {}",
            tok.text,
            tok.offset_from,
            tok.offset_to
        );
        assert!(
            text.is_char_boundary(tok.offset_from),
            "token {i} {:?}: offset_from {} is not a char boundary",
            tok.text,
            tok.offset_from
        );
        assert!(
            text.is_char_boundary(tok.offset_to),
            "token {i} {:?}: offset_to {} is not a char boundary",
            tok.text,
            tok.offset_to
        );
    }
}

/// Assert that doc and query produce at least one common token.
fn assert_matches(doc: &str, query: &str) {
    let doc_tokens = tokenize(doc);
    let query_tokens = tokenize(query);
    let has_overlap = query_tokens.iter().any(|q| doc_tokens.contains(q));
    assert!(
        has_overlap,
        "Expected query {:?} to match doc {:?}.\n  doc tokens: {:?}\n  query tokens: {:?}",
        query, doc, doc_tokens, query_tokens
    );
}

// === Case folding ===

#[test]
fn test_case_insensitive_english() {
    assert_matches("Hello World", "hello");
    assert_matches("Hello World", "HELLO");
    assert_matches("Hello World", "hElLo");
}

#[test]
fn test_case_folding_german() {
    assert_matches("Straße", "strasse");
    assert_matches("STRASSE", "straße");
}

#[test]
fn test_case_folding_greek_sigma() {
    assert_matches("ΣΊΓΜΑ", "σίγμα");
}

// === Unicode normalization ===

#[test]
fn test_nfc_nfd_equivalence() {
    assert_matches("caf\u{00E9}", "caf\u{0065}\u{0301}");
    assert_matches("caf\u{0065}\u{0301}", "café");
}

#[test]
fn test_fullwidth_to_ascii() {
    assert_matches("Ｈｅｌｌｏ Ｗｏｒｌｄ", "hello");
    assert_matches("Hello World", "ｈｅｌｌｏ");
}

#[test]
fn test_fullwidth_digits() {
    assert_matches("１２３４５", "12345");
}

#[test]
fn test_halfwidth_katakana() {
    // Half-width katakana normalized to full-width, then split into unigrams.
    // ﾃｽﾄ → テスト → ["テ", "ス", "ト"]
    assert_matches("ﾃｽﾄ", "テスト");
}

#[test]
fn test_ligatures() {
    assert_matches("ﬁnd", "find");
    assert_matches("ﬂow", "flow");
}

#[test]
fn test_roman_numerals() {
    assert_matches("Chapter Ⅲ", "iii");
}

// === CJK unigram ===

#[test]
fn test_chinese_unigram() {
    let tokens = tokenize("搜索引擎");
    assert_eq!(tokens, &["搜", "索", "引", "擎"]);
}

#[test]
fn test_japanese_hiragana_unigram() {
    let tokens = tokenize("こんにちは");
    assert_eq!(tokens, &["こ", "ん", "に", "ち", "は"]);
}

#[test]
fn test_japanese_katakana_unigram() {
    let tokens = tokenize("プログラミング");
    assert_eq!(tokens, &["プ", "ロ", "グ", "ラ", "ミ", "ン", "グ"]);
}

#[test]
fn test_korean_unigram() {
    let tokens = tokenize("안녕하세요");
    assert_eq!(tokens, &["안", "녕", "하", "세", "요"]);
}

// === Mixed language ===

#[test]
fn test_chinese_english_mixed_search() {
    let doc = "我今天学了Rust语言";
    assert_matches(doc, "rust");
    assert_matches(doc, "语");
}

#[test]
fn test_japanese_english_mixed_search() {
    let doc = "Pythonでプログラミングを学ぶ";
    assert_matches(doc, "python");
    assert_matches(doc, "プ");
    assert_matches(doc, "学");
}

#[test]
fn test_emoji_searchable() {
    assert_matches("今天很开心😊", "😊");
    assert_matches("今天很开心😊", "开");
}

#[test]
fn test_emoji_with_mixed_text() {
    let doc = "Hello 🌍 World 你好 🎉";
    assert_matches(doc, "hello");
    assert_matches(doc, "🌍");
    assert_matches(doc, "你");
    assert_matches(doc, "🎉");
}

// === Edge cases ===

#[test]
fn test_empty_and_punctuation_only() {
    assert!(tokenize("").is_empty());
    assert!(tokenize("。，！？").is_empty());
    assert!(tokenize("...").is_empty());
    assert!(tokenize("   ").is_empty());
}

#[test]
fn test_numbers_in_cjk() {
    let tokens = tokenize("第3章共100页");
    // CJK chars become unigrams, numbers are whole tokens
    assert_eq!(tokens, &["第", "3", "章", "共", "100", "页"]);
}

#[test]
fn test_url_in_text() {
    let doc = "请访问example.com获取详情";
    assert_matches(doc, "example.com");
}

#[test]
fn test_mixed_with_cyrillic() {
    let doc = "Привет World 你好";
    assert_matches(doc, "привет");
    assert_matches(doc, "world");
    assert_matches(doc, "你");
}

#[test]
fn test_supplementary_plane_characters() {
    let doc = "𠀀test🎉你好";
    assert_matches(doc, "test");
    assert_matches(doc, "🎉");
    assert_matches(doc, "你");
    assert_matches(doc, "𠀀");
}

#[test]
fn test_ideographic_space_as_separator() {
    let doc = "你好\u{3000}世界";
    assert_matches(doc, "你");
    assert_matches(doc, "世");
}

// === Offset correctness ===

#[test]
fn test_offsets_ascii() {
    let text = "Hello World";
    let tokens = tokenize_full(text);
    assert_offsets_valid(text, &tokens);
    assert_eq!(&text[tokens[0].offset_from..tokens[0].offset_to], "Hello");
    assert_eq!(&text[tokens[1].offset_from..tokens[1].offset_to], "World");
}

#[test]
fn test_offsets_chinese() {
    let text = "搜索引擎";
    let tokens = tokenize_full(text);
    assert_offsets_valid(text, &tokens);
    // Each char is 3 bytes: 搜(0..3) 索(3..6) 引(6..9) 擎(9..12)
    assert_eq!(tokens.len(), 4);
    assert_eq!((tokens[0].offset_from, tokens[0].offset_to), (0, 3));
    assert_eq!((tokens[3].offset_from, tokens[3].offset_to), (9, 12));
}

#[test]
fn test_offsets_supplementary_cjk() {
    // 𠀀 is 4 bytes in UTF-8
    let text = "𠀀你好";
    let tokens = tokenize_full(text);
    assert_offsets_valid(text, &tokens);
    // 𠀀(0..4) 你(4..7) 好(7..10)
    assert_eq!((tokens[0].offset_from, tokens[0].offset_to), (0, 4));
    assert_eq!(&text[tokens[0].offset_from..tokens[0].offset_to], "𠀀");
}

#[test]
fn test_offsets_fullwidth_normalization() {
    let text = "Ｈｅｌｌｏ Ｗｏｒｌｄ";
    let tokens = tokenize_full(text);
    assert_offsets_valid(text, &tokens);
    // 5 fullwidth chars × 3 bytes = 15 bytes each word
    assert_eq!(tokens[0].text, "hello");
    assert_eq!((tokens[0].offset_from, tokens[0].offset_to), (0, 15));
    assert_eq!(
        &text[tokens[0].offset_from..tokens[0].offset_to],
        "Ｈｅｌｌｏ"
    );
}

#[test]
fn test_offsets_mixed_scripts() {
    let text = "Hello你好World";
    let tokens = tokenize_full(text);
    assert_offsets_valid(text, &tokens);
    // Hello(0..5) 你(5..8) 好(8..11) World(11..16)
    let hello = tokens.iter().find(|t| t.text == "hello").unwrap();
    assert_eq!(&text[hello.offset_from..hello.offset_to], "Hello");
    let world = tokens.iter().find(|t| t.text == "world").unwrap();
    assert_eq!(&text[world.offset_from..world.offset_to], "World");
}

#[test]
fn test_offsets_cjk_with_punctuation() {
    let text = "你好！世界";
    let tokens = tokenize_full(text);
    assert_offsets_valid(text, &tokens);
    // 你(0..3) 好(3..6) ！(6..9, dropped) 世(9..12) 界(12..15)
    assert_eq!(tokens.len(), 4);
    assert_eq!(&text[tokens[0].offset_from..tokens[0].offset_to], "你");
    assert_eq!(&text[tokens[2].offset_from..tokens[2].offset_to], "世");
}

#[test]
fn test_offsets_valid_on_complex_mixed() {
    let cases = &[
        "Hello 🌍 World 你好 🎉 사과の花草",
        "Привет мир 你好世界 🔥🔥🔥 test123",
        "𠀀𠀁test你好🎉カタカナ안녕 café",
        "well-known: 色と形の美 10/10 🕷️",
        "今天😊很开心，学了Rust语言！",
        "",
        "。，！？...   ",
    ];
    for text in cases {
        let tokens = tokenize_full(text);
        assert_offsets_valid(text, &tokens);
    }
}
