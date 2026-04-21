//! Combined normalizing + word-breaking + CJK-splitting tokenizer.
//!
//! [`NormalizingICUTokenizer`] performs NFKC Casefold normalization, ICU word
//! segmentation, and CJK unigram expansion in a single pass, with byte offsets
//! mapped back to the original (pre-normalization) text.

use crate::cjk::try_expand_cjk;
use crate::normalizer::NormalizedText;
use crate::word_break::icu_word_break;
use tantivy_tokenizer_api::{Token, TokenStream, Tokenizer};

/// A tokenizer that normalizes text (NFKC Casefold), segments words with ICU,
/// and splits CJK tokens into individual characters.
///
/// Token offsets always refer to the **original** (pre-normalization) text,
/// so tantivy's snippet/highlight features work correctly.
#[derive(Clone, Copy, Debug, Default)]
pub struct NormalizingICUTokenizer;

impl Tokenizer for NormalizingICUTokenizer {
    type TokenStream<'a> = NormalizingICUTokenStream;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        NormalizingICUTokenStream::new(text)
    }
}

/// Token stream produced by [`NormalizingICUTokenizer`].
pub struct NormalizingICUTokenStream {
    tokens: Vec<Token>,
    index: usize,
}

impl NormalizingICUTokenStream {
    fn new(text: &str) -> Self {
        if text.is_empty() {
            return Self {
                tokens: Vec::new(),
                index: 0,
            };
        }

        // Step 1: Normalize and build offset mapping.
        let nt = NormalizedText::new(text);
        let norm = nt.normalized();

        // Step 2: Word-break the normalized text.
        let spans = icu_word_break(norm);

        // Step 3: Build tokens with CJK expansion and offset mapping.
        let mut tokens = Vec::new();
        let mut position: usize = 0;

        for (norm_start, norm_end) in spans {
            let word = &norm[norm_start..norm_end];

            // Try CJK expansion: if the word is all CJK and multi-char, split it.
            if let Some(chars) = try_expand_cjk(word) {
                for (ch, char_offset_in_word, _char_len) in chars {
                    let char_norm_start = norm_start + char_offset_in_word;
                    let char_norm_end = char_norm_start + ch.len_utf8();
                    let (orig_start, orig_end) =
                        nt.to_original_range(char_norm_start, char_norm_end);

                    tokens.push(Token {
                        offset_from: orig_start,
                        offset_to: orig_end,
                        position,
                        text: ch.to_string(),
                        position_length: 1,
                    });
                    position += 1;
                }
            } else {
                let (orig_start, orig_end) = nt.to_original_range(norm_start, norm_end);

                tokens.push(Token {
                    offset_from: orig_start,
                    offset_to: orig_end,
                    position,
                    text: word.to_string(),
                    position_length: 1,
                });
                position += 1;
            }
        }

        Self { tokens, index: 0 }
    }
}

impl TokenStream for NormalizingICUTokenStream {
    fn advance(&mut self) -> bool {
        if self.index < self.tokens.len() {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn token(&self) -> &Token {
        &self.tokens[self.index - 1]
    }

    fn token_mut(&mut self) -> &mut Token {
        &mut self.tokens[self.index - 1]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect_tokens(text: &str) -> Vec<Token> {
        let mut t = NormalizingICUTokenizer;
        let mut stream = t.token_stream(text);
        let mut result = Vec::new();
        while stream.advance() {
            result.push(stream.token().clone());
        }
        result
    }

    fn texts(tokens: &[Token]) -> Vec<&str> {
        tokens.iter().map(|t| t.text.as_str()).collect()
    }

    // --- CJK unigram tests ---

    #[test]
    fn cjk_unigram_expansion() {
        let tokens = collect_tokens("我爱北京");
        assert_eq!(texts(&tokens), &["我", "爱", "北", "京"]);
    }

    #[test]
    fn katakana_unigram() {
        let tokens = collect_tokens("コンピュータ");
        assert_eq!(texts(&tokens), &["コ", "ン", "ピ", "ュ", "ー", "タ"]);
    }

    #[test]
    fn hiragana_unigram() {
        let tokens = collect_tokens("ありがとう");
        assert_eq!(texts(&tokens), &["あ", "り", "が", "と", "う"]);
    }

    #[test]
    fn hangul_unigram() {
        let tokens = collect_tokens("안녕하세요");
        assert_eq!(texts(&tokens), &["안", "녕", "하", "세", "요"]);
    }

    #[test]
    fn mixed_script_not_unigram_non_cjk() {
        let tokens = collect_tokens("Hello 你好");
        assert_eq!(texts(&tokens), &["hello", "你", "好"]);
    }

    // --- Signature test ---

    #[test]
    fn signature_test_special_cjk_compatibility() {
        let input = "㋿Ξ㍾㍿";
        let tokens = collect_tokens(input);
        assert_eq!(
            texts(&tokens),
            &["令", "和", "ξ", "明", "治", "株", "式", "会", "社"]
        );

        let token_offsets: Vec<(usize, usize)> = tokens
            .iter()
            .map(|t| (t.offset_from, t.offset_to))
            .collect();
        assert_eq!(
            token_offsets,
            &[
                (0, 3),  // 令 → ㋿
                (0, 3),  // 和 → ㋿
                (3, 5),  // ξ → Ξ
                (5, 8),  // 明 → ㍾
                (5, 8),  // 治 → ㍾
                (8, 11), // 株 → ㍿
                (8, 11), // 式 → ㍿
                (8, 11), // 会 → ㍿
                (8, 11), // 社 → ㍿
            ]
        );

        for (i, token) in tokens.iter().enumerate() {
            assert_eq!(token.position, i);
            assert_eq!(token.position_length, 1);
        }

        for token in &tokens {
            assert!(input.is_char_boundary(token.offset_from));
            assert!(input.is_char_boundary(token.offset_to));
            assert!(token.offset_from <= token.offset_to);
            assert!(token.offset_to <= input.len());
        }
    }

    // --- Roundtrip and property tests ---

    #[test]
    fn roundtrip_all_tokens() {
        let inputs = &[
            "",
            " ",
            "a",
            "我",
            "Hello World",
            "㋿Ξ㍾㍿",
            "葛\u{E0100}飾 区",
            "𠮷野家2024",
            "東京タワー",
            "안녕하세요",
            "Привет мир",
            "こんにちは世界Hello🎉",
            "ｶﾞｷのころ",
            "①②③ＡＢＣ",
        ];

        for input in inputs {
            let tokens = collect_tokens(input);
            for token in &tokens {
                assert!(token.offset_from <= token.offset_to);
                assert!(token.offset_to <= input.len());
                assert!(
                    input.is_char_boundary(token.offset_from),
                    "offset_from {} invalid in {input:?}",
                    token.offset_from
                );
                assert!(
                    input.is_char_boundary(token.offset_to),
                    "offset_to {} invalid in {input:?}",
                    token.offset_to
                );
                let _ = &input[token.offset_from..token.offset_to];
            }
        }
    }

    #[test]
    fn positions_are_dense_and_sequential() {
        let tokens = collect_tokens("hello 你好 world");
        for (i, t) in tokens.iter().enumerate() {
            assert_eq!(t.position, i);
            assert_eq!(t.position_length, 1);
        }
    }

    #[test]
    fn empty_input() {
        assert!(collect_tokens("").is_empty());
    }

    #[test]
    fn whitespace_only() {
        assert!(collect_tokens("   \t\n  ").is_empty());
    }

    #[test]
    fn punctuation_only() {
        assert!(collect_tokens("!!!???，。、").is_empty());
    }

    #[test]
    fn emoji_single() {
        let input = "🎉";
        let tokens = collect_tokens(input);
        for t in &tokens {
            assert!(input.is_char_boundary(t.offset_from));
            assert!(input.is_char_boundary(t.offset_to));
        }
    }

    #[test]
    fn emoji_zwj_sequence() {
        let input = "👨\u{200D}👩\u{200D}👧\u{200D}👦";
        let tokens = collect_tokens(input);
        for t in &tokens {
            assert!(input.is_char_boundary(t.offset_from));
            assert!(input.is_char_boundary(t.offset_to));
        }
    }

    #[test]
    fn deterministic_output() {
        let input = "㋿Ξ㍾㍿ Hello 世界";
        let r1 = collect_tokens(input);
        let r2 = collect_tokens(input);
        assert_eq!(r1.len(), r2.len());
        for (a, b) in r1.iter().zip(r2.iter()) {
            assert_eq!(a.text, b.text);
            assert_eq!(a.offset_from, b.offset_from);
            assert_eq!(a.offset_to, b.offset_to);
            assert_eq!(a.position, b.position);
        }
    }

    #[test]
    fn normalization_case_folding() {
        let tokens = collect_tokens("Hello WORLD");
        assert_eq!(texts(&tokens), &["hello", "world"]);
    }

    #[test]
    fn normalization_fullwidth() {
        let tokens = collect_tokens("Ａｐｐｌｅ");
        assert_eq!(texts(&tokens), &["apple"]);
    }

    #[test]
    fn advance_past_end() {
        let mut tokenizer = NormalizingICUTokenizer;
        let mut stream = tokenizer.token_stream("hello");
        assert!(stream.advance());
        assert!(!stream.advance());
        assert!(!stream.advance());
    }
}
