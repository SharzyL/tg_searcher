//! Diacritic folding token filter.
//!
//! Strips foldable diacritics (Latin/Greek/Cyrillic/Vietnamese/IPA accents)
//! from token text via NFD decomposition → filter foldable marks → NFC
//! recomposition. Structurally significant marks (Japanese dakuten, Devanagari
//! virama, Arabic harakat, Hebrew niqqud) are preserved — Arabic/Hebrew marks
//! are handled by [`SemiticNormalizationFilter`](crate::SemiticNormalizationFilter).

use tantivy_tokenizer_api::{Token, TokenFilter, TokenStream, Tokenizer};
use unicode_normalization::UnicodeNormalization;

use super::is_foldable_diacritic;

/// Token filter that strips foldable diacritical marks from token text.
///
/// After NFKC Casefold normalization, precomposed characters like `é` (U+00E9)
/// still contain accents. This filter decomposes them via NFD, strips only
/// combining marks in the foldable ranges (U+0300–036F, U+1AB0–1AFF,
/// U+1DC0–1DFF), then recomposes via NFC.
///
/// Examples: `café → cafe`, `naïve → naive`, `München → munchen` (after casefold).
///
/// Preserved (not foldable):
/// - Japanese dakuten `で` stays `で` (U+3099 not in foldable range)
/// - Devanagari virama `क्ष` stays `क्ष` (U+094D not in foldable range)
/// - Arabic harakat (stripped by [`SemiticNormalizationFilter`](crate::SemiticNormalizationFilter))
/// - Hebrew niqqud (stripped by [`SemiticNormalizationFilter`](crate::SemiticNormalizationFilter))
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

/// Fold foldable diacritics: NFD → strip foldable marks → NFC.
///
/// Returns a new string with foldable diacritics removed. Returns the input
/// unchanged if there are no foldable diacritics.
pub fn fold_diacritics(text: &str) -> String {
    if text.is_ascii() || !text.nfd().any(is_foldable_diacritic) {
        return text.to_string();
    }
    text.nfd()
        .filter(|c| !is_foldable_diacritic(*c))
        .nfc()
        .collect()
}

/// Fold foldable diacritics in place: NFD → strip foldable marks → NFC.
fn fold_diacritics_in_place(text: &mut String) {
    // Fast path: ASCII-only strings never have diacritics.
    if text.is_ascii() {
        return;
    }
    // Fast path: check if NFD form has any foldable diacritics.
    if !text.nfd().any(is_foldable_diacritic) {
        return;
    }
    // Slow path: decompose, filter foldable marks, recompose.
    let folded: String = text
        .nfd()
        .filter(|c| !is_foldable_diacritic(*c))
        .nfc()
        .collect();
    *text = folded;
}

#[cfg(test)]
mod tests {
    use super::*;

    // === Latin diacritics (foldable — stripped) ===

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
        let mut s = "über".to_string();
        fold_diacritics_in_place(&mut s);
        assert_eq!(s, "uber");
    }

    #[test]
    fn test_fold_munchen() {
        let mut s = "münchen".to_string();
        fold_diacritics_in_place(&mut s);
        assert_eq!(s, "munchen");
    }

    #[test]
    fn test_fold_greek_accent() {
        let mut s = "ξένος".to_string();
        fold_diacritics_in_place(&mut s);
        assert_eq!(s, "ξενος");
    }

    #[test]
    fn test_fold_vietnamese() {
        let mut s = "phở".to_string();
        fold_diacritics_in_place(&mut s);
        assert_eq!(s, "pho");
    }

    #[test]
    fn test_turkish_dotted_i() {
        // İ after NFKC casefold → i̇ (i + combining dot above U+0307)
        // DiacriticFolding removes the dot → "i"
        let mut s = "i\u{0307}".to_string();
        fold_diacritics_in_place(&mut s);
        assert_eq!(s, "i");
    }

    // === Non-foldable marks (preserved) ===

    #[test]
    fn test_fold_preserves_dakuten() {
        // Japanese dakuten (U+3099) is NOT in foldable range
        // で (in NFD: て + dakuten) should stay で
        let mut s = "で".to_string();
        fold_diacritics_in_place(&mut s);
        assert_eq!(s, "で");
    }

    #[test]
    fn test_fold_preserves_devanagari() {
        // Devanagari virama (U+094D) is NOT in foldable range
        // क्ष should stay intact
        let mut s = "क्ष".to_string();
        fold_diacritics_in_place(&mut s);
        assert_eq!(s, "क्ष");
    }

    #[test]
    fn test_fold_preserves_arabic_harakat() {
        // Arabic harakat are NOT in foldable range (handled by SemiticNorm)
        let mut s = "كِتَابٌ".to_string();
        fold_diacritics_in_place(&mut s);
        assert_eq!(s, "كِتَابٌ");
    }

    #[test]
    fn test_fold_preserves_hebrew_niqqud() {
        // Hebrew niqqud NOT in foldable range (handled by SemiticNorm)
        let mut s = "שָׁלוֹם".to_string();
        fold_diacritics_in_place(&mut s);
        assert_eq!(s, "שָׁלוֹם");
    }

    // === Unchanged inputs ===

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
}
