//! CJK character classification and unigram expansion.

/// Returns true if `c` belongs to a CJK script that should be individually
/// tokenized (split into single characters) and bigrammed.
///
/// Covers: CJK Unified Ideographs (and extensions A-H), CJK Compatibility
/// Ideographs, Hiragana, Katakana (including phonetic extensions and the
/// long vowel mark), Hangul Syllables, Hangul Jamo (and extensions),
/// and selected CJK symbols (iteration marks, etc.).
pub fn is_cjk_char(c: char) -> bool {
    let cp = c as u32;
    matches!(cp,
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
        // Hiragana
        | 0x3040..=0x309F
        // Katakana (includes long vowel mark U+30FC)
        | 0x30A0..=0x30FF
        // Katakana Phonetic Extensions
        | 0x31F0..=0x31FF
        // Hangul Syllables
        | 0xAC00..=0xD7AF
        // Hangul Jamo
        | 0x1100..=0x11FF
        // Hangul Compatibility Jamo
        | 0x3130..=0x318F
        // Hangul Jamo Extended-A
        | 0xA960..=0xA97F
        // Hangul Jamo Extended-B
        | 0xD7B0..=0xD7FF
        // Halfwidth Katakana
        | 0xFF65..=0xFF9F
        // CJK Symbols (selective)
        | 0x3005        // 々 ideographic iteration mark
        | 0x3006        // 〆
        | 0x3007        // 〇
    )
}

/// Expands a word segment into individual CJK character tokens if it consists
/// entirely of CJK characters and has more than one character.
///
/// Returns `Some(vec_of_(char, byte_offset, byte_len))` if expansion occurred,
/// or `None` if the segment should be kept as a single token.
pub fn try_expand_cjk(text: &str) -> Option<Vec<(char, usize, usize)>> {
    let char_count = text.chars().count();
    if char_count <= 1 {
        return None;
    }
    if !text.chars().all(is_cjk_char) {
        return None;
    }

    let mut result = Vec::with_capacity(char_count);
    let mut byte_pos = 0;
    for ch in text.chars() {
        let ch_len = ch.len_utf8();
        result.push((ch, byte_pos, ch_len));
        byte_pos += ch_len;
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_cjk_basic() {
        assert!(is_cjk_char('你'));
        assert!(is_cjk_char('好'));
        assert!(is_cjk_char('あ'));
        assert!(is_cjk_char('カ'));
        assert!(is_cjk_char('한'));
        assert!(is_cjk_char('ー')); // katakana long vowel
        assert!(is_cjk_char('々')); // iteration mark
    }

    #[test]
    fn test_is_not_cjk() {
        assert!(!is_cjk_char('A'));
        assert!(!is_cjk_char('1'));
        assert!(!is_cjk_char('α'));
        assert!(!is_cjk_char('🎉'));
        assert!(!is_cjk_char('。')); // CJK period — punctuation, not CJK char
    }

    #[test]
    fn test_supplementary_cjk() {
        assert!(is_cjk_char('𠮷')); // Extension B
        assert!(is_cjk_char('\u{20000}')); // Extension B start
    }

    #[test]
    fn test_hangul_jamo() {
        assert!(is_cjk_char('\u{1100}')); // Hangul Choseong
        assert!(is_cjk_char('\u{3131}')); // Hangul Compat Jamo
    }

    #[test]
    fn test_expand_cjk() {
        let result = try_expand_cjk("你好").unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, '你');
        assert_eq!(result[0].1, 0);
        assert_eq!(result[0].2, 3);
        assert_eq!(result[1].0, '好');
        assert_eq!(result[1].1, 3);
        assert_eq!(result[1].2, 3);
    }

    #[test]
    fn test_expand_single_char() {
        assert!(try_expand_cjk("你").is_none());
    }

    #[test]
    fn test_expand_mixed_not_all_cjk() {
        assert!(try_expand_cjk("hello").is_none());
        assert!(try_expand_cjk("A你").is_none());
    }
}
