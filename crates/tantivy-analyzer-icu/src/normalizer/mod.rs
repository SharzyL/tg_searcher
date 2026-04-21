//! Unicode NFKC Casefold normalization with offset mapping.
//!
//! Normalizes text using ICU's NFKC_Casefold normalizer and builds a mapping
//! between byte offsets in the normalized text and the original text.

use rust_icu_common as common;
use rust_icu_sys::UNormalizer2;
use rust_icu_sys::versioned_function;
use rust_icu_unorm2::UNormalizer;

/// A block mapping a region of the original text to a region of the normalized text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditBlock {
    /// Byte offset in the original text.
    pub src_offset: usize,
    /// Byte length in the original text.
    pub src_length: usize,
    /// Byte offset in the normalized text.
    pub dst_offset: usize,
    /// Byte length in the normalized text.
    pub dst_length: usize,
    /// Whether the content changed during normalization.
    pub has_change: bool,
}

/// Holds a NFKC-Casefold-normalized string and an offset mapping back to the original.
///
/// The mapping is represented as a list of [`EditBlock`]s covering the entire text.
/// Each block maps a contiguous region of the original to a contiguous region of the
/// normalized text.
#[derive(Debug)]
pub struct NormalizedText {
    normalized: String,
    blocks: Vec<EditBlock>,
    original_len: usize,
}

/// Returns the raw ICU NFKC_Casefold normalizer instance pointer (global singleton).
fn get_nfkc_casefold_instance() -> *const UNormalizer2 {
    let mut status = common::Error::OK_CODE;
    // SAFETY: unorm2_getNFKCCasefoldInstance returns a global singleton pointer.
    // The pointer is valid for the lifetime of the process.
    let ptr = unsafe { versioned_function!(unorm2_getNFKCCasefoldInstance)(&mut status) };
    assert!(
        common::Error::is_ok(status),
        "unorm2_getNFKCCasefoldInstance failed"
    );
    ptr
}

/// Returns true if `c` is a normalization boundary for NFKC Casefold.
///
/// A boundary means normalization can be computed independently on each side.
/// This is more precise than checking combining class alone — it accounts for
/// NFKC-specific interactions like Hangul jamo composition and halfwidth
/// katakana dakuten decomposition.
fn has_boundary_before(norm2: *const UNormalizer2, c: char) -> bool {
    // SAFETY: unorm2_hasBoundaryBefore takes a valid normalizer pointer and a
    // UChar32 code point. The normalizer is a global singleton.
    unsafe { versioned_function!(unorm2_hasBoundaryBefore)(norm2, c as i32) != 0 }
}

impl NormalizedText {
    /// Normalizes `original` with NFKC Casefold and builds the offset mapping.
    ///
    /// The text is split into normalization-safe segments using ICU's
    /// `unorm2_hasBoundaryBefore`, each segment is normalized independently,
    /// and the per-segment mapping is recorded.
    pub fn new(original: &str) -> Self {
        let normalizer =
            UNormalizer::new_nfkc_casefold().expect("ICU NFKC_Casefold normalizer init failed");
        let norm2_ptr = get_nfkc_casefold_instance();

        if original.is_empty() {
            return Self {
                normalized: String::new(),
                blocks: Vec::new(),
                original_len: 0,
            };
        }

        let mut normalized = String::with_capacity(original.len());
        let mut blocks = Vec::new();

        let mut chars = original.char_indices().peekable();

        while let Some(&(seg_start, _first_char)) = chars.peek() {
            // Consume the first character of the segment.
            chars.next();

            // Consume following characters that don't start a new normalization boundary.
            while let Some(&(_pos, c)) = chars.peek() {
                if !has_boundary_before(norm2_ptr, c) {
                    chars.next();
                } else {
                    break;
                }
            }

            let seg_end = chars.peek().map_or(original.len(), |&(pos, _)| pos);
            let segment = &original[seg_start..seg_end];

            let dst_offset = normalized.len();
            let norm_segment = normalizer
                .normalize(segment)
                .expect("ICU normalization failed");
            normalized.push_str(&norm_segment);
            let dst_length = normalized.len() - dst_offset;

            blocks.push(EditBlock {
                src_offset: seg_start,
                src_length: seg_end - seg_start,
                dst_offset,
                dst_length,
                has_change: segment != norm_segment,
            });
        }

        // Verify that segment-by-segment normalization matches full normalization.
        debug_assert_eq!(
            normalized,
            normalizer.normalize(original).unwrap(),
            "Segment-by-segment normalization differs from full normalization"
        );

        Self {
            normalized,
            blocks,
            original_len: original.len(),
        }
    }

    /// Returns the normalized text.
    pub fn normalized(&self) -> &str {
        &self.normalized
    }

    /// Returns the byte length of the original text.
    pub fn original_len(&self) -> usize {
        self.original_len
    }

    /// Returns the edit blocks.
    pub fn blocks(&self) -> &[EditBlock] {
        &self.blocks
    }

    /// Translates a byte range `[norm_start, norm_end)` in the normalized text
    /// back to a byte range in the original text.
    ///
    /// For unchanged blocks, the mapping is precise at byte level.
    /// For changed blocks, any position inside the block maps to the entire
    /// original range of that block.
    pub fn to_original_range(&self, norm_start: usize, norm_end: usize) -> (usize, usize) {
        if self.blocks.is_empty() {
            return (0, 0);
        }

        let orig_start = self.norm_offset_to_original(norm_start, true);
        let orig_end = self.norm_offset_to_original(norm_end, false);

        (orig_start, orig_end)
    }

    /// Maps a single normalized byte offset to its corresponding original byte offset.
    ///
    /// `is_start` controls behavior when the offset falls inside a changed block:
    /// - `true`: returns the start of the block's source range
    /// - `false`: returns the end of the block's source range
    fn norm_offset_to_original(&self, norm_offset: usize, is_start: bool) -> usize {
        if norm_offset == 0 {
            return 0;
        }
        if norm_offset >= self.normalized.len() {
            return self.original_len;
        }

        // Binary search for the block containing norm_offset.
        let idx = match self
            .blocks
            .binary_search_by_key(&norm_offset, |b| b.dst_offset)
        {
            Ok(i) => {
                if !is_start && i > 0 {
                    // For end offsets, a position at a block boundary belongs to
                    // the end of the previous block, not the start of this one.
                    let prev = &self.blocks[i - 1];
                    return prev.src_offset + prev.src_length;
                }
                i
            }
            Err(i) => i.saturating_sub(1),
        };

        let block = &self.blocks[idx];
        let offset_within_block = norm_offset - block.dst_offset;

        if !block.has_change {
            // Unchanged block: precise byte-level mapping.
            block.src_offset + offset_within_block
        } else if is_start {
            // Changed block, looking for start: return block's source start.
            block.src_offset
        } else {
            // Changed block, looking for end: return block's source end.
            block.src_offset + block.src_length
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_basic_ascii() {
        let nt = NormalizedText::new("Hello World");
        assert_eq!(nt.normalized(), "hello world");
    }

    #[test]
    fn normalize_fullwidth_to_halfwidth() {
        let nt = NormalizedText::new("Ａｐｐｌｅ");
        assert_eq!(nt.normalized(), "apple");
    }

    #[test]
    fn normalize_enclosed_cjk() {
        let nt = NormalizedText::new("㈱");
        assert_eq!(nt.normalized(), "(株)");
        // ㈱ is 3 bytes; normalized "(株)" = 1 + 3 + 1 = 5 bytes.
        // All normalized positions map back to original 0..3.
        assert_eq!(nt.to_original_range(0, 5), (0, 3));
        assert_eq!(nt.to_original_range(1, 4), (0, 3));
    }

    #[test]
    fn normalize_ligature() {
        let nt = NormalizedText::new("ﬁle");
        assert_eq!(nt.normalized(), "file");
    }

    #[test]
    fn normalize_decomposed_dakuten() {
        // か + U+3099 (combining dakuten) → が
        let input = "か\u{3099}";
        let nt = NormalizedText::new(input);
        assert_eq!(nt.normalized(), "が");
    }

    #[test]
    fn normalize_halfwidth_katakana() {
        let nt = NormalizedText::new("ｶﾞｷ");
        assert_eq!(nt.normalized(), "ガキ");
    }

    #[test]
    fn normalize_hangul_jamo_to_syllable() {
        let input = "\u{110B}\u{1161}\u{11AB}";
        let nt = NormalizedText::new(input);
        assert_eq!(nt.normalized(), "안");
    }

    #[test]
    fn normalize_variation_selector_removed() {
        let input = "葛\u{E0100}飾";
        let nt = NormalizedText::new(input);
        assert_eq!(nt.normalized(), "葛飾");
    }

    #[test]
    fn normalize_zero_width_removed() {
        let input = "hello\u{200B}world";
        let nt = NormalizedText::new(input);
        assert_eq!(nt.normalized(), "helloworld");
    }

    #[test]
    fn normalize_combining_marks_sorting() {
        // a + U+0301 (above, ccc=230) + U+0320 (below, ccc=220)
        // NFKC composes a+0301→á, then appends 0320.
        let input = "a\u{0301}\u{0320}";
        let nt = NormalizedText::new(input);
        assert_eq!(nt.normalized(), "\u{00E1}\u{0320}");
    }

    #[test]
    fn normalize_empty() {
        let nt = NormalizedText::new("");
        assert_eq!(nt.normalized(), "");
        assert_eq!(nt.original_len(), 0);
    }

    // --- Offset mapping tests ---

    #[test]
    fn offset_unchanged_text_precise() {
        let input = "hello world";
        let nt = NormalizedText::new(input);
        // Pure lowercase ASCII: normalized == original.
        assert_eq!(nt.normalized(), "hello world");
        for pos in 0..=input.len() {
            let (s, e) = nt.to_original_range(pos, pos);
            assert_eq!(s, pos, "start mismatch at pos {pos}");
            assert_eq!(e, pos, "end mismatch at pos {pos}");
        }
    }

    #[test]
    fn offset_at_char_boundaries() {
        let inputs = &[
            "Hello 世界 World",
            "㈱東京",
            "café résumé",
            "𠮷野家",
            "葛\u{E0100}飾",
            "ｶﾞｷ",
        ];

        for input in inputs {
            let nt = NormalizedText::new(input);
            let norm = nt.normalized();
            for pos in 0..=norm.len() {
                if norm.is_char_boundary(pos) {
                    let (s, e) = nt.to_original_range(pos, pos);
                    assert!(s <= e, "s > e at pos {pos} in {input:?}");
                    assert!(
                        e <= input.len(),
                        "e > input.len() at pos {pos} in {input:?}"
                    );
                    assert!(
                        input.is_char_boundary(s),
                        "origin start {s} not at char boundary in {input:?}"
                    );
                    assert!(
                        input.is_char_boundary(e),
                        "origin end {e} not at char boundary in {input:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn offset_monotonic() {
        let nt = NormalizedText::new("Ａｐｐｌｅ㈱");
        let norm = nt.normalized();
        let mut last_start = 0;
        for pos in 0..=norm.len() {
            if norm.is_char_boundary(pos) {
                let (s, _) = nt.to_original_range(pos, pos);
                assert!(s >= last_start, "offset non-monotonic at pos {pos}");
                last_start = s;
            }
        }
    }

    #[test]
    fn signature_normalization() {
        // ㋿ U+32FF → "令和", Ξ U+039E → "ξ", ㍾ U+337E → "明治", ㍿ U+337F → "株式会社"
        let input = "㋿Ξ㍾㍿";
        let nt = NormalizedText::new(input);
        assert_eq!(nt.normalized(), "令和ξ明治株式会社");

        assert_eq!(input.len(), 11);
        assert_eq!(nt.normalized().len(), 26);

        assert_eq!(nt.to_original_range(0, 6), (0, 3)); // 令和 → ㋿
        assert_eq!(nt.to_original_range(3, 6), (0, 3)); // 和 → ㋿ (changed block)
        assert_eq!(nt.to_original_range(0, 3), (0, 3)); // 令 → ㋿
        assert_eq!(nt.to_original_range(6, 8), (3, 5)); // ξ → Ξ
        assert_eq!(nt.to_original_range(8, 14), (5, 8)); // 明治 → ㍾
        assert_eq!(nt.to_original_range(14, 26), (8, 11)); // 株式会社 → ㍿
        assert_eq!(nt.to_original_range(17, 20), (8, 11)); // 式 → ㍿
    }
}
