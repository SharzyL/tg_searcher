//! Diacritic folding token filter.
//!
//! Removes all combining marks (accents, diacritics, harakat, etc.) from token
//! text via NFD decomposition → filter CCC≠0 → NFC recomposition. Offsets are
//! preserved unchanged since this operates on the token text only.

use tantivy_tokenizer_api::{Token, TokenFilter, TokenStream, Tokenizer};
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::canonical_combining_class;

/// Token filter that strips all diacritical marks from token text.
///
/// After NFKC Casefold normalization, precomposed characters like `é` (U+00E9)
/// still contain accents. This filter decomposes them via NFD, strips all
/// combining marks (canonical combining class ≠ 0), then recomposes via NFC.
///
/// Examples: `café → cafe`, `naïve → naive`, `München → munchen` (after casefold),
/// Arabic harakat like `كِتَابٌ → كتاب`.
#[derive(Clone, Copy, Debug, Default)]
pub struct DiacriticFoldingFilter;

impl TokenFilter for DiacriticFoldingFilter {
    type Tokenizer<T: Tokenizer> = DiacriticFoldingWrapper<T>;

    fn transform<T: Tokenizer>(self, tokenizer: T) -> Self::Tokenizer<T> {
        DiacriticFoldingWrapper { inner: tokenizer }
    }
}

#[derive(Clone)]
pub struct DiacriticFoldingWrapper<T> {
    inner: T,
}

impl<T: Tokenizer> Tokenizer for DiacriticFoldingWrapper<T> {
    type TokenStream<'a> = DiacriticFoldingTokenStream<T::TokenStream<'a>>;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        DiacriticFoldingTokenStream {
            inner: self.inner.token_stream(text),
        }
    }
}

pub struct DiacriticFoldingTokenStream<S> {
    inner: S,
}

impl<S: TokenStream> TokenStream for DiacriticFoldingTokenStream<S> {
    fn advance(&mut self) -> bool {
        if !self.inner.advance() {
            return false;
        }
        let token = self.inner.token_mut();
        fold_diacritics_in_place(&mut token.text);
        true
    }

    fn token(&self) -> &Token {
        self.inner.token()
    }

    fn token_mut(&mut self) -> &mut Token {
        self.inner.token_mut()
    }
}

/// Fold diacritics in place: NFD → strip CCC≠0 → NFC.
fn fold_diacritics_in_place(text: &mut String) {
    // Fast path: ASCII-only strings never have diacritics.
    if text.is_ascii() {
        return;
    }
    // Fast path: check if NFD form has any combining marks.
    if !text.nfd().any(|c| canonical_combining_class(c) != 0) {
        return;
    }
    // Slow path: decompose, filter, recompose.
    let folded: String = text
        .nfd()
        .filter(|c| canonical_combining_class(*c) == 0)
        .nfc()
        .collect();
    *text = folded;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fold_basic_latin() {
        let mut s = "café".to_string();
        fold_diacritics_in_place(&mut s);
        assert_eq!(s, "cafe");
    }

    #[test]
    fn test_fold_nfd_input() {
        // NFD: e + combining acute
        let mut s = "cafe\u{0301}".to_string();
        fold_diacritics_in_place(&mut s);
        assert_eq!(s, "cafe");
    }

    #[test]
    fn test_fold_naive() {
        let mut s = "naïve".to_string();
        fold_diacritics_in_place(&mut s);
        assert_eq!(s, "naive");
    }

    #[test]
    fn test_fold_nino() {
        let mut s = "niño".to_string();
        fold_diacritics_in_place(&mut s);
        assert_eq!(s, "nino");
    }

    #[test]
    fn test_fold_uber() {
        // After NFKC casefold, Ü → ü
        let mut s = "über".to_string();
        fold_diacritics_in_place(&mut s);
        assert_eq!(s, "uber");
    }

    #[test]
    fn test_fold_munchen() {
        // After casefold: München → münchen
        let mut s = "münchen".to_string();
        fold_diacritics_in_place(&mut s);
        assert_eq!(s, "munchen");
    }

    #[test]
    fn test_fold_greek_accent() {
        // ξένος (xi + epsilon-with-acute + nu + omicron + final-sigma)
        // After casefold: ξένος → ξένος (already lowercase, but accent remains)
        let mut s = "ξένος".to_string();
        fold_diacritics_in_place(&mut s);
        assert_eq!(s, "ξενος");
    }

    #[test]
    fn test_fold_arabic_harakat() {
        // كِتَابٌ → كتاب (harakat are combining marks)
        let mut s = "كِتَابٌ".to_string();
        fold_diacritics_in_place(&mut s);
        assert_eq!(s, "كتاب");
    }

    #[test]
    fn test_fold_vietnamese() {
        // phở has multiple diacritics on ơ
        let mut s = "phở".to_string();
        fold_diacritics_in_place(&mut s);
        assert_eq!(s, "pho");
    }

    #[test]
    fn test_fold_ascii_unchanged() {
        let mut s = "hello world".to_string();
        fold_diacritics_in_place(&mut s);
        assert_eq!(s, "hello world");
    }

    #[test]
    fn test_fold_cjk_unchanged() {
        let mut s = "你好世界".to_string();
        fold_diacritics_in_place(&mut s);
        assert_eq!(s, "你好世界");
    }

    #[test]
    fn test_fold_hangul_unchanged() {
        let mut s = "안녕하세요".to_string();
        fold_diacritics_in_place(&mut s);
        assert_eq!(s, "안녕하세요");
    }

    #[test]
    fn test_fold_empty() {
        let mut s = String::new();
        fold_diacritics_in_place(&mut s);
        assert_eq!(s, "");
    }

    #[test]
    fn test_turkish_dotted_i() {
        // İ after NFKC casefold → i̇ (i + combining dot above U+0307)
        // DiacriticFolding removes the dot → "i"
        let mut s = "i\u{0307}".to_string(); // post-casefold form
        fold_diacritics_in_place(&mut s);
        assert_eq!(s, "i");
    }
}
