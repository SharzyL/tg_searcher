mod bigram;
mod han_only;

pub use bigram::CJKBigramFilter;
pub use han_only::HanOnlyFilter;

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
