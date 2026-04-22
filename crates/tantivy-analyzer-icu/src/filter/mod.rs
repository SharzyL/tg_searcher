//! Token filters and script classification utilities.
//!
//! All filters implement [`tantivy_tokenizer_api::TokenFilter`] and are designed
//! to be composed in a [`tantivy::tokenizer::TextAnalyzer`] pipeline after
//! [`NormalizingICUTokenizer`](crate::NormalizingICUTokenizer).
//!
//! ## Filters
//!
//! Applied in this recommended order:
//!
//! 1. [`SemiticNormalizationFilter`] — Arabic character normalization (alif variants,
//!    ta marbuta, tatweel removal, digit mapping) + harakat/niqqud stripping.
//! 2. [`DiacriticFoldingFilter`] — NFD decompose, strip foldable diacritics
//!    (U+0300–036F, U+1AB0–1AFF, U+1DC0–1DFF), NFC recompose. Preserves
//!    Japanese dakuten, Devanagari virama, and other structural marks.
//! 3. One of:
//!    - [`CJKBigramFilter`] — overlapping bigrams for CJK, with offset-based
//!      adjacency detection. Used for the folded_bigram index field.
//!    - [`HanOnlyFilter`] — keeps only single Han characters. Used for the
//!      unigram index field.
//!    - [`DiacriticOnlyFilter`] — keeps only tokens with foldable diacritics in
//!      their original (pre-fold) form. Used for the diacritic index field.
//!
//! ## Script Classification
//!
//! [`ScriptGroup`] classifies characters into HanKana (Han + Hiragana + Katakana),
//! Hangul, or NonCjk. [`CJKBigramFilter`] only produces bigrams within the same
//! script group.
//!
//! ## Query Analysis
//!
//! [`find_isolated_han_tokens`] identifies Han characters that would be dropped by
//! [`CJKBigramFilter`] (isolated, not adjacent to other CJK characters). Used by
//! [`ICUSearchConfig::route_query`](crate::search::ICUSearchConfig::route_query)
//! to decide which characters need unigram fallback.

mod bigram;
mod diacritic_folding;
mod diacritic_only;
mod han_only;
mod semitic_normalization;

pub use bigram::CJKBigramFilter;
pub use diacritic_folding::{DiacriticFoldingFilter, fold_diacritics};
pub use diacritic_only::DiacriticOnlyFilter;
pub use han_only::HanOnlyFilter;
pub use semitic_normalization::SemiticNormalizationFilter;

use tantivy_tokenizer_api::Token;
use unicode_normalization::UnicodeNormalization;

/// Returns true if `c` is a Han (Chinese) character.
///
/// This includes CJK Unified Ideographs and all extensions, CJK Compatibility
/// Ideographs, and the ideographic iteration mark 々.
pub fn is_han_char(c: char) -> bool {
    let cp = c as u32;
    matches!(
        cp,
        // CJK Unified Ideographs
        0x4E00..=0x9FFF
        // CJK Extension A
        | 0x3400..=0x4DBF
        // CJK Extension B
        | 0x20000..=0x2A6DF
        // CJK Extension C
        | 0x2A700..=0x2B73F
        // CJK Extension D
        | 0x2B740..=0x2B81F
        // CJK Extension E
        | 0x2B820..=0x2CEAF
        // CJK Extension F
        | 0x2CEB0..=0x2EBEF
        // CJK Extension G
        | 0x30000..=0x3134F
        // CJK Extension H
        | 0x31350..=0x323AF
        // CJK Compatibility Ideographs
        | 0xF900..=0xFAFF
        // CJK Compatibility Ideographs Supplement
        | 0x2F800..=0x2FA1F
        // Ideographic iteration mark 々
        | 0x3005
    )
}

/// Returns true if `c` is a Hiragana or Katakana character (including long vowel mark ー).
fn is_kana_char(c: char) -> bool {
    let cp = c as u32;
    matches!(
        cp,
        // Hiragana
        0x3040..=0x309F
        // Katakana (includes long vowel mark U+30FC)
        | 0x30A0..=0x30FF
        // Katakana Phonetic Extensions
        | 0x31F0..=0x31FF
        // Halfwidth Katakana
        | 0xFF65..=0xFF9F
        // 〆 and 〇
        | 0x3006 | 0x3007
    )
}

/// Returns true if `c` is a Hangul character.
fn is_hangul_char(c: char) -> bool {
    let cp = c as u32;
    matches!(
        cp,
        // Hangul Syllables
        0xAC00..=0xD7AF
        // Hangul Jamo
        | 0x1100..=0x11FF
        // Hangul Compatibility Jamo
        | 0x3130..=0x318F
        // Hangul Jamo Extended-A
        | 0xA960..=0xA97F
        // Hangul Jamo Extended-B
        | 0xD7B0..=0xD7FF
    )
}

/// Script group for CJK bigram formation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptGroup {
    /// Han + Hiragana + Katakana
    HanKana,
    /// Hangul
    Hangul,
    /// Not CJK
    NonCjk,
}

/// Returns the script group for a character.
fn script_group(c: char) -> ScriptGroup {
    if is_han_char(c) || is_kana_char(c) {
        ScriptGroup::HanKana
    } else if is_hangul_char(c) {
        ScriptGroup::Hangul
    } else {
        ScriptGroup::NonCjk
    }
}

/// Returns the script group of a token (based on its first character).
pub fn token_script_group(text: &str) -> ScriptGroup {
    text.chars()
        .next()
        .map_or(ScriptGroup::NonCjk, script_group)
}

/// Returns true if `ch` is a combining mark in the "foldable diacritic" ranges.
///
/// These ranges cover Latin, Greek, Cyrillic, Vietnamese, and IPA accents.
/// They deliberately exclude structurally significant marks:
/// Japanese dakuten (U+3099/309A), Devanagari virama (U+094D),
/// Arabic harakat (U+064B–0652), Hebrew niqqud (U+0591–05C7).
pub fn is_foldable_diacritic(ch: char) -> bool {
    matches!(
        ch as u32,
        // Combining Diacritical Marks
        0x0300..=0x036F
        // Combining Diacritical Marks Extended
        | 0x1AB0..=0x1AFF
        // Combining Diacritical Marks Supplement
        | 0x1DC0..=0x1DFF
    )
}

/// Returns true if the NFD decomposition of `text` contains any foldable diacritic.
pub fn has_foldable_diacritic(text: &str) -> bool {
    text.nfd().any(is_foldable_diacritic)
}

/// Finds isolated Han characters in a token sequence produced by the base
/// analyzer (NormalizingICUTokenizer + DiacriticFolding + ArabicNorm, without
/// CJK bigram/unigram filters).
///
/// Uses the same offset-adjacency + script-group logic as [`CJKBigramFilter`]:
/// tokens form a "run" when they share the same script group AND their offsets
/// are contiguous or overlapping. A Han token that ends up in a run of length 1
/// is "isolated" — it would be dropped by the bigram filter and needs unigram
/// coverage.
///
/// The `<=` check covers two cases (offset mapping is monotonic, so no
/// others exist): `==` for adjacent source regions (京(0,3) 东(3,6)), and
/// `<` for same-source NFKC expansions (㍿ → 株(8,11) 式(8,11)). A gap (`>`)
/// means intervening content in the original text.
pub fn find_isolated_han_tokens(tokens: &[Token]) -> Vec<String> {
    let mut isolated = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let group = token_script_group(&tokens[i].text);

        if group == ScriptGroup::NonCjk {
            i += 1;
            continue;
        }

        // Collect a run of same-group, offset-adjacent/overlapping CJK tokens.
        let run_start = i;
        i += 1;
        while i < tokens.len()
            && token_script_group(&tokens[i].text) == group
            && tokens[i].offset_from <= tokens[i - 1].offset_to
        {
            i += 1;
        }

        // Isolated single-token run: check if it's a Han character.
        if i - run_start == 1 {
            let text = &tokens[run_start].text;
            if text.chars().next().is_some_and(is_han_char) {
                isolated.push(text.clone());
            }
        }
    }
    isolated
}
