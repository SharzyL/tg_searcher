//! Utility functions for TG Searcher

use html_escape::encode_text;

/// Escape HTML content and replace newlines with spaces
pub fn escape_content(content: &str) -> String {
    encode_text(content).replace('\n', " ")
}

/// Get a brief version of content for logging
pub fn brief_content(content: &str, trim_len: usize) -> String {
    if content.len() < trim_len {
        content.to_string()
    } else {
        format!(
            "{}…{}",
            &content[..trim_len - 4],
            &content[content.len() - 2..]
        )
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
        // Note: '…' is 3 bytes in UTF-8, so the byte length will be slightly more than 20
        assert!(brief.len() >= 20 && brief.len() <= 23);
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
}
