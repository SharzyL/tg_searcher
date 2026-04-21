//! Arabic text normalization token filter.
//!
//! Normalizes Arabic script variants to canonical forms for search:
//! alif variants, ta marbuta/ha, ya maqsura, tatweel, hamza carriers,
//! Farsi variants, and Arabic-Indic/Persian digits.

use tantivy_tokenizer_api::{Token, TokenFilter, TokenStream, Tokenizer};

/// Token filter that normalizes Arabic script characters.
///
/// Applied after [`DiacriticFoldingFilter`](crate::DiacriticFoldingFilter) which
/// handles harakat removal and alif-with-hamza decomposition (أ إ آ → ا via
/// NFD + combining mark removal).
///
/// This filter handles the remaining Arabic-specific mappings:
/// - Alif wasla (ٱ) → Alif (ا)
/// - Alif maqsura (ى) → Ya (ي)
/// - Ta marbuta (ة) → Ha (ه)
/// - Farsi Ya (ی) → Arabic Ya (ي)
/// - Farsi Kaf (ک) → Arabic Kaf (ك)
/// - Waw+hamza (ؤ) → Waw (و)
/// - Ya+hamza (ئ) → Ya (ي)
/// - Tatweel (ـ) → removed
/// - Standalone hamza (ء) → removed
/// - Arabic-Indic digits (٠-٩) → ASCII 0-9
/// - Persian digits (۰-۹) → ASCII 0-9
#[derive(Clone, Copy, Debug, Default)]
pub struct ArabicNormalizationFilter;

impl TokenFilter for ArabicNormalizationFilter {
    type Tokenizer<T: Tokenizer> = ArabicNormalizationWrapper<T>;

    fn transform<T: Tokenizer>(self, tokenizer: T) -> Self::Tokenizer<T> {
        ArabicNormalizationWrapper { inner: tokenizer }
    }
}

#[derive(Clone)]
pub struct ArabicNormalizationWrapper<T> {
    inner: T,
}

impl<T: Tokenizer> Tokenizer for ArabicNormalizationWrapper<T> {
    type TokenStream<'a> = ArabicNormalizationTokenStream<T::TokenStream<'a>>;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        ArabicNormalizationTokenStream {
            inner: self.inner.token_stream(text),
        }
    }
}

pub struct ArabicNormalizationTokenStream<S> {
    inner: S,
}

impl<S: TokenStream> TokenStream for ArabicNormalizationTokenStream<S> {
    fn advance(&mut self) -> bool {
        if !self.inner.advance() {
            return false;
        }
        let token = self.inner.token_mut();
        normalize_arabic_in_place(&mut token.text);
        true
    }

    fn token(&self) -> &Token {
        self.inner.token()
    }

    fn token_mut(&mut self) -> &mut Token {
        self.inner.token_mut()
    }
}

fn normalize_arabic_char(c: char) -> Option<char> {
    match c {
        'ٱ' => Some('ا'),  // Alif wasla → Alif
        'ى' => Some('ي'),  // Alif maqsura → Ya
        'ة' => Some('ه'),  // Ta marbuta → Ha
        'ی' => Some('ي'),  // Farsi ya → Arabic ya
        'ک' => Some('ك'),  // Farsi kaf → Arabic kaf
        'ؤ' => Some('و'),  // Waw+hamza → Waw
        'ئ' => Some('ي'),  // Ya+hamza → Ya
        'ـ' | 'ء' => None, // Tatweel, standalone hamza → remove
        '٠'..='٩' => {
            // Arabic-Indic digits → ASCII
            char::from_digit(c as u32 - '٠' as u32, 10)
        }
        '۰'..='۹' => {
            // Persian digits → ASCII
            char::from_digit(c as u32 - '۰' as u32, 10)
        }
        _ => Some(c),
    }
}

fn needs_arabic_normalization(c: char) -> bool {
    matches!(
        c,
        'ٱ' | 'ى' | 'ة' | 'ی' | 'ک' | 'ؤ' | 'ئ' | 'ـ' | 'ء' | '٠'..='٩' | '۰'..='۹'
    )
}

fn normalize_arabic_in_place(text: &mut String) {
    // Fast path: no Arabic-specific chars to normalize.
    if !text.chars().any(needs_arabic_normalization) {
        return;
    }
    let normalized: String = text.chars().filter_map(normalize_arabic_char).collect();
    *text = normalized;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alif_wasla() {
        let mut s = "ٱلله".to_string();
        normalize_arabic_in_place(&mut s);
        assert_eq!(s, "الله");
    }

    #[test]
    fn test_alif_maqsura() {
        let mut s = "فى".to_string();
        normalize_arabic_in_place(&mut s);
        assert_eq!(s, "في");
    }

    #[test]
    fn test_ta_marbuta() {
        let mut s = "مدرسة".to_string();
        normalize_arabic_in_place(&mut s);
        assert_eq!(s, "مدرسه");
    }

    #[test]
    fn test_farsi_ya_kaf() {
        let mut s = "یک".to_string();
        normalize_arabic_in_place(&mut s);
        assert_eq!(s, "يك");
    }

    #[test]
    fn test_waw_hamza() {
        let mut s = "ؤلاد".to_string();
        normalize_arabic_in_place(&mut s);
        assert_eq!(s, "ولاد");
    }

    #[test]
    fn test_ya_hamza() {
        let mut s = "ئيل".to_string();
        normalize_arabic_in_place(&mut s);
        assert_eq!(s, "ييل");
    }

    #[test]
    fn test_tatweel_removal() {
        let mut s = "الــــله".to_string();
        normalize_arabic_in_place(&mut s);
        assert_eq!(s, "الله");
    }

    #[test]
    fn test_standalone_hamza_removal() {
        let mut s = "ءادم".to_string();
        normalize_arabic_in_place(&mut s);
        assert_eq!(s, "ادم");
    }

    #[test]
    fn test_arabic_indic_digits() {
        let mut s = "٢٠٢٤".to_string();
        normalize_arabic_in_place(&mut s);
        assert_eq!(s, "2024");
    }

    #[test]
    fn test_persian_digits() {
        let mut s = "۲۰۲۴".to_string();
        normalize_arabic_in_place(&mut s);
        assert_eq!(s, "2024");
    }

    #[test]
    fn test_no_arabic_unchanged() {
        let mut s = "hello world".to_string();
        normalize_arabic_in_place(&mut s);
        assert_eq!(s, "hello world");
    }

    #[test]
    fn test_cjk_unchanged() {
        let mut s = "你好".to_string();
        normalize_arabic_in_place(&mut s);
        assert_eq!(s, "你好");
    }

    #[test]
    fn test_mixed_arabic_latin() {
        let mut s = "test٢٠٢٤test".to_string();
        normalize_arabic_in_place(&mut s);
        assert_eq!(s, "test2024test");
    }

    #[test]
    fn test_empty() {
        let mut s = String::new();
        normalize_arabic_in_place(&mut s);
        assert_eq!(s, "");
    }
}
