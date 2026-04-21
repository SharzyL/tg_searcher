//! Semitic text normalization token filter.
//!
//! Normalizes Arabic and Hebrew script characters for search. Covers:
//! - Arabic: alif variants, ta marbuta/ha, ya maqsura, tatweel, hamza carriers,
//!   Farsi variants, harakat stripping, and Arabic-Indic/Persian digit mapping.
//! - Hebrew: niqqud (vowel points, cantillation marks) stripping.

use tantivy_tokenizer_api::{Token, TokenFilter, TokenStream, Tokenizer};

/// Token filter that normalizes Arabic and Hebrew script characters.
///
/// Applied **before** [`DiacriticFoldingFilter`](crate::DiacriticFoldingFilter)
/// in the pipeline so that harakat and niqqud are stripped before foldable
/// diacritic detection.
///
/// Arabic normalization:
/// - Harakat (U+064B–U+0655), superscript alef (U+0670), Quranic marks (U+06D6–U+06ED) → removed
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
///
/// Hebrew normalization:
/// - Niqqud vowel points and cantillation marks (U+0591–U+05BD, U+05BF,
///   U+05C1–U+05C2, U+05C4–U+05C5, U+05C7) → removed
#[derive(Clone, Copy, Debug, Default)]
pub struct SemiticNormalizationFilter;

impl TokenFilter for SemiticNormalizationFilter {
    type Tokenizer<T: Tokenizer> = SemiticNormalizationWrapper<T>;

    fn transform<T: Tokenizer>(self, tokenizer: T) -> Self::Tokenizer<T> {
        SemiticNormalizationWrapper { inner: tokenizer }
    }
}

#[derive(Clone)]
pub struct SemiticNormalizationWrapper<T> {
    inner: T,
}

impl<T: Tokenizer> Tokenizer for SemiticNormalizationWrapper<T> {
    type TokenStream<'a> = SemiticNormalizationTokenStream<T::TokenStream<'a>>;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        SemiticNormalizationTokenStream {
            inner: self.inner.token_stream(text),
        }
    }
}

pub struct SemiticNormalizationTokenStream<S> {
    inner: S,
}

impl<S: TokenStream> TokenStream for SemiticNormalizationTokenStream<S> {
    fn advance(&mut self) -> bool {
        if !self.inner.advance() {
            return false;
        }
        let token = self.inner.token_mut();
        normalize_semitic_in_place(&mut token.text);
        true
    }

    fn token(&self) -> &Token {
        self.inner.token()
    }

    fn token_mut(&mut self) -> &mut Token {
        self.inner.token_mut()
    }
}

fn normalize_semitic_char(c: char) -> Option<char> {
    match c {
        // === Arabic character normalization ===
        '\u{0623}' => Some('\u{0627}'), // Alif + Hamza above → Alif
        '\u{0625}' => Some('\u{0627}'), // Alif + Hamza below → Alif
        '\u{0622}' => Some('\u{0627}'), // Alif + Madda above → Alif
        'ٱ' => Some('ا'),               // Alif wasla → Alif
        'ى' => Some('ي'),               // Alif maqsura → Ya
        'ة' => Some('ه'),               // Ta marbuta → Ha
        'ی' => Some('ي'),               // Farsi ya → Arabic ya
        'ک' => Some('ك'),               // Farsi kaf → Arabic kaf
        'ؤ' => Some('و'),               // Waw+hamza → Waw
        'ئ' => Some('ي'),               // Ya+hamza → Ya
        'ـ' | 'ء' => None,              // Tatweel, standalone hamza → remove
        '٠'..='٩' => {
            // Arabic-Indic digits → ASCII
            char::from_digit(c as u32 - '٠' as u32, 10)
        }
        '۰'..='۹' => {
            // Persian digits → ASCII
            char::from_digit(c as u32 - '۰' as u32, 10)
        }

        // === Arabic harakat (vowel marks), combining hamza, and Quranic marks ===
        '\u{064B}'..='\u{0655}' => None, // Fathatan through Hamza Below
        '\u{0670}' => None,              // Superscript Alef (dagger alef)
        '\u{06D6}'..='\u{06ED}' => None, // Quranic small high marks

        // === Hebrew niqqud (vowel points, cantillation marks) ===
        '\u{0591}'..='\u{05BD}' => None, // Accents + vowel points (Etnahta..Meteg)
        '\u{05BF}' => None,              // Rafe
        '\u{05C1}'..='\u{05C2}' => None, // Shin dot, Sin dot
        '\u{05C4}'..='\u{05C5}' => None, // Upper/lower dot
        '\u{05C7}' => None,              // Qamats Qatan

        _ => Some(c),
    }
}

fn needs_semitic_normalization(c: char) -> bool {
    matches!(
        c,
        // Arabic character normalization
        '\u{0623}' | '\u{0625}' | '\u{0622}'
        | 'ٱ' | 'ى' | 'ة' | 'ی' | 'ک' | 'ؤ' | 'ئ' | 'ـ' | 'ء'
        | '٠'..='٩' | '۰'..='۹'
        // Arabic harakat + combining hamza + Quranic marks
        | '\u{064B}'..='\u{0655}' | '\u{0670}' | '\u{06D6}'..='\u{06ED}'
        // Hebrew niqqud
        | '\u{0591}'..='\u{05BD}' | '\u{05BF}'
        | '\u{05C1}'..='\u{05C2}' | '\u{05C4}'..='\u{05C5}' | '\u{05C7}'
    )
}

fn normalize_semitic_in_place(text: &mut String) {
    // Fast path: no Semitic-specific chars to normalize.
    if !text.chars().any(needs_semitic_normalization) {
        return;
    }
    let normalized: String = text.chars().filter_map(normalize_semitic_char).collect();
    *text = normalized;
}

#[cfg(test)]
mod tests {
    use super::*;

    // === Arabic character normalization (unchanged) ===

    #[test]
    fn test_alif_wasla() {
        let mut s = "ٱلله".to_string();
        normalize_semitic_in_place(&mut s);
        assert_eq!(s, "الله");
    }

    #[test]
    fn test_alif_maqsura() {
        let mut s = "فى".to_string();
        normalize_semitic_in_place(&mut s);
        assert_eq!(s, "في");
    }

    #[test]
    fn test_ta_marbuta() {
        let mut s = "مدرسة".to_string();
        normalize_semitic_in_place(&mut s);
        assert_eq!(s, "مدرسه");
    }

    #[test]
    fn test_farsi_ya_kaf() {
        let mut s = "یک".to_string();
        normalize_semitic_in_place(&mut s);
        assert_eq!(s, "يك");
    }

    #[test]
    fn test_waw_hamza() {
        let mut s = "ؤلاد".to_string();
        normalize_semitic_in_place(&mut s);
        assert_eq!(s, "ولاد");
    }

    #[test]
    fn test_ya_hamza() {
        let mut s = "ئيل".to_string();
        normalize_semitic_in_place(&mut s);
        assert_eq!(s, "ييل");
    }

    #[test]
    fn test_tatweel_removal() {
        let mut s = "الــــله".to_string();
        normalize_semitic_in_place(&mut s);
        assert_eq!(s, "الله");
    }

    #[test]
    fn test_standalone_hamza_removal() {
        let mut s = "ءادم".to_string();
        normalize_semitic_in_place(&mut s);
        assert_eq!(s, "ادم");
    }

    #[test]
    fn test_arabic_indic_digits() {
        let mut s = "٢٠٢٤".to_string();
        normalize_semitic_in_place(&mut s);
        assert_eq!(s, "2024");
    }

    #[test]
    fn test_persian_digits() {
        let mut s = "۲۰۲۴".to_string();
        normalize_semitic_in_place(&mut s);
        assert_eq!(s, "2024");
    }

    // === Arabic harakat stripping (new) ===

    #[test]
    fn test_arabic_harakat_stripping() {
        // كِتَابٌ → كتاب
        let mut s = "كِتَابٌ".to_string();
        normalize_semitic_in_place(&mut s);
        assert_eq!(s, "كتاب");
    }

    #[test]
    fn test_arabic_dagger_alef() {
        // ٱلرَّحْمَنِ → الرحمن (alif wasla + harakat)
        let mut s = "ٱلرَّحْمَنِ".to_string();
        normalize_semitic_in_place(&mut s);
        assert_eq!(s, "الرحمن");
    }

    // === Hebrew niqqud stripping (new) ===

    #[test]
    fn test_hebrew_niqqud_shalom() {
        // שָׁלוֹם → שלום
        let mut s = "שָׁלוֹם".to_string();
        normalize_semitic_in_place(&mut s);
        assert_eq!(s, "שלום");
    }

    #[test]
    fn test_hebrew_niqqud_bereshit() {
        // בְּרֵאשִׁית → בראשית
        let mut s = "בְּרֵאשִׁית".to_string();
        normalize_semitic_in_place(&mut s);
        assert_eq!(s, "בראשית");
    }

    // === Unchanged by this filter ===

    #[test]
    fn test_no_arabic_unchanged() {
        let mut s = "hello world".to_string();
        normalize_semitic_in_place(&mut s);
        assert_eq!(s, "hello world");
    }

    #[test]
    fn test_cjk_unchanged() {
        let mut s = "你好".to_string();
        normalize_semitic_in_place(&mut s);
        assert_eq!(s, "你好");
    }

    #[test]
    fn test_latin_diacritics_unchanged() {
        let mut s = "café".to_string();
        normalize_semitic_in_place(&mut s);
        assert_eq!(s, "café");
    }

    #[test]
    fn test_mixed_arabic_latin() {
        let mut s = "test٢٠٢٤test".to_string();
        normalize_semitic_in_place(&mut s);
        assert_eq!(s, "test2024test");
    }

    #[test]
    fn test_empty() {
        let mut s = String::new();
        normalize_semitic_in_place(&mut s);
        assert_eq!(s, "");
    }
}
