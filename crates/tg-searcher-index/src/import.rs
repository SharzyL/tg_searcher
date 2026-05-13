//! Parse Telegram Desktop "Export chat history" JSON (`result.json`)
//! into `IndexMsg` records ready for `Indexer::add_documents_batch`.

use chrono::{DateTime, Utc};
use serde::Deserialize;
use tracing::warn;

use crate::{Error, IndexMsg, Result};

#[derive(Deserialize)]
struct TelegramExport {
    id: i64,
    #[serde(default)]
    name: String,
    messages: Vec<RawMessage>,
}

#[derive(Deserialize)]
struct RawMessage {
    id: i64,
    #[serde(rename = "type")]
    msg_type: String,
    #[serde(default)]
    date_unixtime: Option<String>,
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    text: Option<TextField>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum TextField {
    Plain(String),
    Rich(Vec<RichTextPart>),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RichTextPart {
    Plain(String),
    Entity { text: String },
}

fn extract_text(field: &TextField) -> String {
    match field {
        TextField::Plain(s) => s.clone(),
        TextField::Rich(parts) => parts
            .iter()
            .map(|p| match p {
                RichTextPart::Plain(s) => s.as_str(),
                RichTextPart::Entity { text } => text.as_str(),
            })
            .collect(),
    }
}

/// Result of parsing a Telegram Desktop export.
#[derive(Debug)]
pub struct ParsedExport {
    pub chat_id: i64,
    pub chat_name: String,
    pub messages: Vec<IndexMsg>,
}

/// Parse a Telegram Desktop export (`result.json`) into indexable messages.
///
/// - Only entries with `type == "message"` and non-empty resolved text are kept.
/// - The export's top-level `id` is used directly as `chat_id` — it matches
///   the share_id form (positive integer) the bot uses everywhere.
/// - URL is `https://t.me/c/{chat_id}/{msg_id}`, same as the live indexer.
pub fn parse_telegram_export(json: &str) -> Result<ParsedExport> {
    let export: TelegramExport = serde_json::from_str(json)
        .map_err(|e| Error::Index(format!("invalid export JSON: {e}")))?;
    let chat_id = export.id;
    let chat_name = export.name;
    let messages = export
        .messages
        .into_iter()
        .filter(|m| m.msg_type == "message")
        .filter_map(|m| {
            let text_field = m.text.as_ref()?;
            let content = extract_text(text_field);
            if content.is_empty() {
                return None;
            }
            let Some(ts_str) = m.date_unixtime.as_deref() else {
                warn!("import: skipping msg {}: missing date_unixtime", m.id);
                return None;
            };
            let Ok(ts) = ts_str.parse::<i64>() else {
                warn!(
                    "import: skipping msg {}: unparseable date_unixtime={:?}",
                    m.id, ts_str
                );
                return None;
            };
            let Some(post_time) = DateTime::<Utc>::from_timestamp(ts, 0) else {
                warn!(
                    "import: skipping msg {}: timestamp {} out of range",
                    m.id, ts
                );
                return None;
            };
            Some(IndexMsg {
                content,
                url: format!("https://t.me/c/{}/{}", chat_id, m.id),
                chat_id,
                post_time,
                sender: m.from.clone().unwrap_or_default(),
            })
        })
        .collect();
    Ok(ParsedExport {
        chat_id,
        chat_name,
        messages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_export() {
        let json = r#"{
            "id": 1639873385,
            "name": "Test Chat",
            "type": "private_supergroup",
            "messages": [
                {
                    "id": 100,
                    "type": "message",
                    "date_unixtime": "1583735876",
                    "from": "Alice",
                    "text": "hello world"
                }
            ]
        }"#;
        let parsed = parse_telegram_export(json).unwrap();
        assert_eq!(parsed.chat_id, 1639873385);
        assert_eq!(parsed.chat_name, "Test Chat");
        assert_eq!(parsed.messages.len(), 1);
        let m = &parsed.messages[0];
        assert_eq!(m.content, "hello world");
        assert_eq!(m.url, "https://t.me/c/1639873385/100");
        assert_eq!(m.chat_id, 1639873385);
        assert_eq!(m.sender, "Alice");
    }

    #[test]
    fn skips_service_and_empty_keeps_rich_text() {
        let json = r#"{
            "id": 42,
            "name": "Mixed",
            "type": "private_group",
            "messages": [
                {
                    "id": 1,
                    "type": "service",
                    "action": "create_group",
                    "date_unixtime": "1583664917",
                    "text": ""
                },
                {
                    "id": 2,
                    "type": "message",
                    "date_unixtime": "1583735876",
                    "from": "Bob",
                    "text": ""
                },
                {
                    "id": 3,
                    "type": "message",
                    "date_unixtime": "1583735900",
                    "from": "Carol",
                    "text": [
                        "see this ",
                        { "type": "link", "text": "https://example.com" },
                        " for details"
                    ]
                }
            ]
        }"#;
        let parsed = parse_telegram_export(json).unwrap();
        assert_eq!(parsed.messages.len(), 1);
        let m = &parsed.messages[0];
        assert_eq!(m.content, "see this https://example.com for details");
        assert_eq!(m.url, "https://t.me/c/42/3");
        assert_eq!(m.sender, "Carol");
    }

    #[test]
    fn rejects_malformed_json() {
        let err = parse_telegram_export("not json").unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("invalid export JSON"));
    }
}
