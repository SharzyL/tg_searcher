//! Utility functions for TG Searcher

use crate::types::HighlightedSnippet;
use grammers_tl_types as tl;
use html_escape::encode_text;

/// Escape HTML content and replace newlines with spaces
pub fn escape_content(content: &str) -> String {
    encode_text(content).replace('\n', " ")
}

/// Get a brief version of content for logging
pub fn brief_content(content: &str, trim_len: usize) -> String {
    if content.chars().count() < trim_len {
        content.to_string()
    } else {
        let head: String = content.chars().take(trim_len - 4).collect();
        let tail: String = content.chars().rev().take(2).collect::<Vec<_>>().into_iter().rev().collect();
        format!("{}…{}", head, tail)
    }
}

/// Remove the first word from text (used for command parsing)
pub fn remove_first_word(text: &str) -> &str {
    match text.find(' ') {
        Some(pos) => &text[pos + 1..],
        None => "",
    }
}

/// Get normalized share ID from Telegram chat ID
///
/// Telegram uses different ID formats for different chat types.
/// This function normalizes them to the share ID format used in URLs.
///
/// Reference: Telethon's resolve_id function
/// https://github.com/LonamiWebs/Telethon/blob/master/telethon/utils.py
pub fn get_share_id(chat_id: i64) -> i64 {
    // Based on Telethon's resolve_id logic:
    // - Channels/megagroups: -100XXXXXXXXXX -> XXXXXXXXXX
    // - Other chats: use as-is but ensure positive

    if chat_id < 0 {
        // Remove the -100 prefix for channels/megagroups
        let abs_id = chat_id.abs();
        if abs_id > 1_000_000_000_000 {
            // It's a channel/megagroup ID (-100XXXXXXXXXX)
            abs_id - 1_000_000_000_000
        } else {
            abs_id
        }
    } else {
        chat_id
    }
}

/// Whether a character is an invisible Unicode formatting character that Telegram strips.
fn is_invisible_format_char(c: char) -> bool {
    // Note: U+200D (ZWJ) is NOT stripped — it's used in emoji sequences (👨‍👩‍👧).
    // U+200C (ZWNJ) is also kept — it has semantic meaning in some scripts.
    matches!(
        c,
        '\u{200B}'   // zero-width space
            | '\u{200E}' // left-to-right mark
            | '\u{200F}' // right-to-left mark
            | '\u{2060}' // word joiner
            | '\u{FEFF}' // zero-width no-break space / BOM
    )
}

/// Builder for constructing Telegram messages with entities (bold, underline, links, etc.)
/// directly, bypassing HTML parsing. All offsets are tracked in UTF-16 code units.
pub struct MessageBuilder {
    text: String,
    entities: Vec<tl::enums::MessageEntity>,
    utf16_offset: i32,
}

impl MessageBuilder {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            entities: Vec::new(),
            utf16_offset: 0,
        }
    }

    /// Append plain text (no formatting).
    /// Invisible formatting characters are stripped to match Telegram's server behavior.
    pub fn push(&mut self, s: &str) {
        for ch in s.chars() {
            if !is_invisible_format_char(ch) {
                self.text.push(ch);
                self.utf16_offset += ch.len_utf16() as i32;
            }
        }
    }

    /// Append text wrapped in a Bold entity
    pub fn push_bold(&mut self, s: &str) {
        let offset = self.utf16_offset;
        self.push(s);
        let length = self.utf16_offset - offset;
        if length > 0 {
            self.entities
                .push(tl::enums::MessageEntity::Bold(tl::types::MessageEntityBold {
                    offset,
                    length,
                }));
        }
    }

    /// Append text wrapped in an Underline entity
    pub fn push_underline(&mut self, s: &str) {
        let offset = self.utf16_offset;
        self.push(s);
        let length = self.utf16_offset - offset;
        if length > 0 {
            self.entities.push(tl::enums::MessageEntity::Underline(
                tl::types::MessageEntityUnderline { offset, length },
            ));
        }
    }

    /// Append text as a clickable link (TextUrl entity)
    #[cfg(test)]
    pub fn push_text_url(&mut self, display: &str, url: &str) {
        let offset = self.utf16_offset;
        self.push(display);
        let length = self.utf16_offset - offset;
        if length > 0 {
            self.entities.push(tl::enums::MessageEntity::TextUrl(
                tl::types::MessageEntityTextUrl {
                    offset,
                    length,
                    url: url.to_string(),
                },
            ));
        }
    }

    /// Record the current UTF-16 offset (for starting a span that will be closed later)
    pub fn mark(&self) -> i32 {
        self.utf16_offset
    }

    /// Add a Bold entity spanning from `start_offset` to the current position
    pub fn push_bold_since(&mut self, start_offset: i32) {
        let length = self.utf16_offset - start_offset;
        if length > 0 {
            self.entities
                .push(tl::enums::MessageEntity::Bold(tl::types::MessageEntityBold {
                    offset: start_offset,
                    length,
                }));
        }
    }

    /// Append a highlighted snippet as a clickable link with bold highlights.
    ///
    /// The entire fragment becomes a TextUrl entity pointing to `url`.
    /// Each highlight byte range becomes a nested Bold entity.
    /// Handles overlapping ranges (tantivy may produce them).
    pub fn push_highlighted_snippet(&mut self, snippet: &HighlightedSnippet, url: &str) {
        let fragment = snippet.fragment.trim_end();
        if fragment.is_empty() {
            return;
        }

        let link_start = self.utf16_offset;
        let highlights = collapse_byte_ranges(&snippet.highlights);

        // Iterate by chars to avoid slicing at invalid byte boundaries.
        // Tantivy's highlight ranges are byte offsets that may not align with
        // UTF-8 char boundaries; we determine highlighting per-char based on
        // whether the char's start byte falls within a highlight range.
        let mut highlight_iter = highlights.iter().peekable();
        let mut chunk = String::new();
        let mut chunk_is_bold = false;

        for (byte_pos, ch) in fragment.char_indices() {
            // Advance past highlight ranges we've passed
            while highlight_iter
                .peek()
                .is_some_and(|r| byte_pos >= r.end)
            {
                highlight_iter.next();
            }

            let is_bold = highlight_iter
                .peek()
                .is_some_and(|r| byte_pos >= r.start);

            if is_bold != chunk_is_bold && !chunk.is_empty() {
                if chunk_is_bold {
                    self.push_bold(&chunk);
                } else {
                    self.push(&chunk);
                }
                chunk.clear();
            }
            chunk_is_bold = is_bold;
            chunk.push(ch);
        }

        if !chunk.is_empty() {
            if chunk_is_bold {
                self.push_bold(&chunk);
            } else {
                self.push(&chunk);
            }
        }

        // Wrap the entire fragment in a TextUrl entity
        let link_length = self.utf16_offset - link_start;
        if link_length > 0 {
            self.entities.push(tl::enums::MessageEntity::TextUrl(
                tl::types::MessageEntityTextUrl {
                    offset: link_start,
                    length: link_length,
                    url: url.to_string(),
                },
            ));
        }
    }

    /// Consume the builder and return (text, entities).
    /// Entities are sorted by offset ascending, then length descending
    /// (outer entities before inner ones at the same offset), as Telegram requires.
    pub fn build(mut self) -> (String, Vec<tl::enums::MessageEntity>) {
        self.entities.sort_by(|a, b| {
            let (a_offset, a_length) = entity_offset_length(a);
            let (b_offset, b_length) = entity_offset_length(b);
            a_offset.cmp(&b_offset).then(b_length.cmp(&a_length))
        });
        (self.text, self.entities)
    }
}

/// Extract (offset, length) from a MessageEntity variant.
fn entity_offset_length(entity: &tl::enums::MessageEntity) -> (i32, i32) {
    match entity {
        tl::enums::MessageEntity::Bold(e) => (e.offset, e.length),
        tl::enums::MessageEntity::Underline(e) => (e.offset, e.length),
        tl::enums::MessageEntity::TextUrl(e) => (e.offset, e.length),
        tl::enums::MessageEntity::Italic(e) => (e.offset, e.length),
        tl::enums::MessageEntity::Strike(e) => (e.offset, e.length),
        tl::enums::MessageEntity::Code(e) => (e.offset, e.length),
        tl::enums::MessageEntity::Pre(e) => (e.offset, e.length),
        _ => unreachable!("unexpected entity type in MessageBuilder"),
    }
}

/// Collapse overlapping byte ranges: sort, deduplicate, and merge overlapping ones.
fn collapse_byte_ranges(ranges: &[std::ops::Range<usize>]) -> Vec<std::ops::Range<usize>> {
    let mut sorted: Vec<_> = ranges.to_vec();
    sorted.sort_by_key(|r| (r.start, r.end));
    sorted.dedup();

    let mut result = Vec::<std::ops::Range<usize>>::new();
    for range in sorted {
        if let Some(last) = result.last_mut() {
            if last.end >= range.start {
                last.end = last.end.max(range.end);
            } else {
                result.push(range);
            }
        } else {
            result.push(range);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_content() {
        // html-escape encodes < > & but not quotes by default
        assert_eq!(
            escape_content("<script>alert('xss')</script>"),
            "&lt;script&gt;alert('xss')&lt;/script&gt;"
        );
        assert_eq!(escape_content("line1\nline2"), "line1 line2");
    }

    #[test]
    fn test_brief_content() {
        let long_text = "a".repeat(100);
        let brief = brief_content(&long_text, 20);
        assert!(brief.contains('…'));
        // head = 16 chars + '…' + tail = 2 chars = 19 chars
        assert_eq!(brief.chars().count(), 19);

        // Short text returned as-is
        assert_eq!(brief_content("short", 20), "short");

        // CJK: should not panic on multi-byte chars
        let cjk_text = "日本の行政対応等についての説明文書です";
        let brief = brief_content(cjk_text, 10);
        assert!(brief.contains('…'));
    }

    #[test]
    fn test_remove_first_word() {
        assert_eq!(remove_first_word("/command arg1 arg2"), "arg1 arg2");
        assert_eq!(remove_first_word("/command"), "");
        assert_eq!(remove_first_word("single"), "");
    }

    #[test]
    fn test_get_share_id() {
        // Channel/megagroup ID
        assert_eq!(get_share_id(-1001234567890), 1234567890);

        // Regular negative chat ID
        assert_eq!(get_share_id(-123456), 123456);

        // Positive ID
        assert_eq!(get_share_id(123456), 123456);
    }

    #[test]
    fn test_message_builder_plain_text() {
        let mut b = MessageBuilder::new();
        b.push("hello world");
        let (text, entities) = b.build();
        assert_eq!(text, "hello world");
        assert!(entities.is_empty());
    }

    #[test]
    fn test_message_builder_bold() {
        let mut b = MessageBuilder::new();
        b.push("hi ");
        b.push_bold("bold");
        b.push(" end");
        let (text, entities) = b.build();
        assert_eq!(text, "hi bold end");
        assert_eq!(entities.len(), 1);
        match &entities[0] {
            tl::enums::MessageEntity::Bold(e) => {
                assert_eq!(e.offset, 3);
                assert_eq!(e.length, 4);
            }
            _ => panic!("expected Bold"),
        }
    }

    #[test]
    fn test_message_builder_text_url() {
        let mut b = MessageBuilder::new();
        b.push_text_url("click", "https://example.com");
        let (text, entities) = b.build();
        assert_eq!(text, "click");
        assert_eq!(entities.len(), 1);
        match &entities[0] {
            tl::enums::MessageEntity::TextUrl(e) => {
                assert_eq!(e.offset, 0);
                assert_eq!(e.length, 5);
                assert_eq!(e.url, "https://example.com");
            }
            _ => panic!("expected TextUrl"),
        }
    }

    #[test]
    fn test_message_builder_cjk_offsets() {
        // CJK characters are 1 UTF-16 unit each but multi-byte in UTF-8
        let mut b = MessageBuilder::new();
        b.push("你好");  // 2 chars, 2 UTF-16 units
        b.push_bold("世界");  // 2 chars, 2 UTF-16 units
        let (text, entities) = b.build();
        assert_eq!(text, "你好世界");
        match &entities[0] {
            tl::enums::MessageEntity::Bold(e) => {
                assert_eq!(e.offset, 2);  // after "你好"
                assert_eq!(e.length, 2);  // "世界"
            }
            _ => panic!("expected Bold"),
        }
    }

    #[test]
    fn test_message_builder_strips_invisible_chars() {
        let mut b = MessageBuilder::new();
        b.push("hello\u{200f}");  // RTL mark should be stripped
        b.push_bold("world");
        let (text, entities) = b.build();
        assert_eq!(text, "helloworld");  // no RTL mark
        match &entities[0] {
            tl::enums::MessageEntity::Bold(e) => {
                assert_eq!(e.offset, 5);  // "hello" = 5, no RTL mark
                assert_eq!(e.length, 5);
            }
            _ => panic!("expected Bold"),
        }
    }

    #[test]
    fn test_message_builder_strips_invisible_in_underline() {
        // Reproduces the \u{200f}Sharzy case
        let mut b = MessageBuilder::new();
        b.push("(");
        b.push_underline("\u{200f}Sharzy");
        b.push(") after");
        let (text, entities) = b.build();
        assert_eq!(text, "(Sharzy) after");  // RTL mark stripped
        match &entities[0] {
            tl::enums::MessageEntity::Underline(e) => {
                assert_eq!(e.offset, 1);   // after "("
                assert_eq!(e.length, 6);   // "Sharzy" without RTL mark
            }
            _ => panic!("expected Underline"),
        }
    }

    #[test]
    fn test_message_builder_emoji_offsets() {
        // Emoji like 🎉 is 2 UTF-16 code units (surrogate pair)
        let mut b = MessageBuilder::new();
        b.push("🎉");
        b.push_bold("ok");
        let (_, entities) = b.build();
        match &entities[0] {
            tl::enums::MessageEntity::Bold(e) => {
                assert_eq!(e.offset, 2);  // 🎉 = 2 UTF-16 units
                assert_eq!(e.length, 2);  // "ok"
            }
            _ => panic!("expected Bold"),
        }
    }

    #[test]
    fn test_message_builder_mark_and_bold_since() {
        let mut b = MessageBuilder::new();
        let m = b.mark();
        b.push("hello ");
        b.push_underline("world");
        b.push_bold_since(m);
        let (text, entities) = b.build();
        assert_eq!(text, "hello world");
        // Should have Underline for "world" and Bold for "hello world"
        assert_eq!(entities.len(), 2);
    }

    #[test]
    fn test_message_builder_highlighted_snippet() {
        let snippet = HighlightedSnippet {
            fragment: "hello world test".to_string(),
            highlights: vec![6..11],  // "world"
        };
        let mut b = MessageBuilder::new();
        b.push("prefix ");
        b.push_highlighted_snippet(&snippet, "https://t.me/c/123/1");
        let (text, entities) = b.build();
        assert_eq!(text, "prefix hello world test");
        // Should have TextUrl for entire snippet (sorted first, lower offset + longer),
        // then Bold for "world"
        assert_eq!(entities.len(), 2);
        match &entities[0] {
            tl::enums::MessageEntity::TextUrl(e) => {
                assert_eq!(e.offset, 7);  // after "prefix "
                assert_eq!(e.length, 16); // "hello world test"
                assert_eq!(e.url, "https://t.me/c/123/1");
            }
            _ => panic!("expected TextUrl"),
        }
        match &entities[1] {
            tl::enums::MessageEntity::Bold(e) => {
                assert_eq!(e.offset, 13);  // "prefix " (7) + "hello " (6) = 13
                assert_eq!(e.length, 5);   // "world"
            }
            _ => panic!("expected Bold"),
        }
    }

    #[test]
    fn test_message_builder_highlighted_snippet_empty() {
        let snippet = HighlightedSnippet {
            fragment: String::new(),
            highlights: vec![],
        };
        let mut b = MessageBuilder::new();
        b.push_highlighted_snippet(&snippet, "https://t.me/c/123/1");
        let (text, entities) = b.build();
        assert_eq!(text, "");
        assert!(entities.is_empty());
    }

    #[test]
    fn test_message_builder_highlighted_snippet_cjk() {
        let snippet = HighlightedSnippet {
            fragment: "人人都在说这个人很好".to_string(),
            highlights: vec![0..3],  // "人" (3 bytes in UTF-8)
        };
        let mut b = MessageBuilder::new();
        b.push_highlighted_snippet(&snippet, "https://t.me/c/123/1");
        let (text, entities) = b.build();
        assert_eq!(text, "人人都在说这个人很好");
        assert_eq!(entities.len(), 2); // TextUrl (outer) + Bold (inner)
        match &entities[0] {
            tl::enums::MessageEntity::TextUrl(e) => {
                assert_eq!(e.offset, 0);
                assert_eq!(e.length, 10); // 10 CJK chars = 10 UTF-16 units
            }
            _ => panic!("expected TextUrl"),
        }
        match &entities[1] {
            tl::enums::MessageEntity::Bold(e) => {
                assert_eq!(e.offset, 0);
                assert_eq!(e.length, 1); // "人" = 1 UTF-16 unit
            }
            _ => panic!("expected Bold"),
        }
    }

    #[test]
    fn test_message_builder_snippet_mid_char_boundary() {
        // Reproduces the panic: byte range lands inside a multi-byte CJK char.
        // "伊斯坦布尔的" — each char is 3 bytes, so byte 16 is inside '的' (bytes 15..18).
        let fragment = "伊斯坦布尔的交通系统".to_string();
        let snippet = HighlightedSnippet {
            fragment,
            highlights: vec![13..16], // mid-char boundaries
        };
        let mut b = MessageBuilder::new();
        b.push_highlighted_snippet(&snippet, "https://t.me/c/1/1");
        let (text, _entities) = b.build();
        // Should not panic; snapped range covers full chars
        assert_eq!(text, "伊斯坦布尔的交通系统");
    }

    #[test]
    fn test_collapse_byte_ranges() {
        assert_eq!(collapse_byte_ranges(&[0..3, 3..6]), vec![0..6]);
        assert_eq!(collapse_byte_ranges(&[0..2, 5..7]), vec![0..2, 5..7]);
        assert_eq!(collapse_byte_ranges(&[0..5, 2..4]), vec![0..5]);
        assert_eq!(collapse_byte_ranges(&[3..6, 0..2]), vec![0..2, 3..6]);
        let empty: Vec<std::ops::Range<usize>> = vec![];
        assert_eq!(collapse_byte_ranges(&[]), empty);
    }
}
