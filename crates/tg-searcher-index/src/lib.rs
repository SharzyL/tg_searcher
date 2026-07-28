//! Full-text search indexer using Tantivy with ICU-based tokenization.
//!
//! This crate wraps [`tantivy_analyzer_icu`] for indexing and searching
//! Telegram messages with NFKC casefolding, ICU word break, diacritic
//! folding and CJK bigram tokenization.
//!
//! The schema includes hardcoded fields:
//! - `content` (ICU field group: stored + folded_bigram + unigram + diacritic)
//! - `url`, `chat_id`, `post_time`, `sender`

use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tantivy::collector::{Count, TopDocs};
use tantivy::query::{BooleanQuery, Occur, Query, TermQuery};
use tantivy::schema::*;
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, Term, doc};
use tantivy_analyzer_icu::search::{ICUFieldGroup, ICUSearchConfig};
use thiserror::Error;
use tracing::warn;

pub mod import;

const SNIPPET_MAX_CHARS: usize = 100;

// ── Error type ──────────────────────────────────────────────────────

/// Error type for index operations.
#[derive(Error, Debug)]
pub enum Error {
    #[error("Index error: {0}")]
    Index(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type alias for index operations.
pub type Result<T> = std::result::Result<T, Error>;

// ── Types ───────────────────────────────────────────────────────────

/// Message to be indexed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexMsg {
    /// Message text content
    pub content: String,

    /// URL to the message (format: `https://t.me/c/{share_id}/{msg_id}`)
    pub url: String,

    /// Chat ID (normalized share_id)
    pub chat_id: i64,

    /// Message timestamp
    pub post_time: DateTime<Utc>,

    /// Sender's name
    pub sender: String,
}

/// A text snippet with highlighted (matched) byte ranges.
#[derive(Debug, Clone)]
pub struct HighlightedSnippet {
    /// The plain text fragment
    pub fragment: String,
    /// Byte ranges within `fragment` that should be highlighted (bold)
    pub highlights: Vec<Range<usize>>,
}

/// Search result hit with highlighting.
#[derive(Debug, Clone)]
pub struct SearchHit {
    /// The indexed message
    pub msg: IndexMsg,

    /// Snippet with highlight ranges
    pub snippet: HighlightedSnippet,
}

/// Sort criterion for search results.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortMode {
    /// Order by `post_time` (newest first when `reverse=false`).
    #[default]
    Time,
    /// Order by relevance score from `route_query`
    /// (most relevant first when `reverse=false`).
    Relevance,
}

/// Search results with pagination info.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Search hits
    pub hits: Vec<SearchHit>,

    /// Whether this is the last page
    pub is_last_page: bool,

    /// Total number of results
    pub total_results: usize,
}

// ── Indexer ─────────────────────────────────────────────────────────

/// Full-text search indexer for Telegram messages.
pub struct Indexer {
    #[allow(dead_code)]
    index: Index,
    writer: Arc<RwLock<IndexWriter>>,
    reader: IndexReader,
    fields: IndexFields,
    icu: ICUSearchConfig,
}

struct IndexFields {
    content: ICUFieldGroup,
    url: Field,
    chat_id: Field,
    post_time: Field,
    sender: Field,
}

fn parse_msg_id_from_url(url: &str) -> Option<i32> {
    url.rsplit('/').next()?.parse().ok()
}

/// Decode an i64 term-dictionary key (8 big-endian bytes, sign-bit flipped) back
/// to the original chat id. Tantivy stores i64 terms via `i64_to_u64` so the
/// dictionary is sorted in the natural numeric order; we just invert that.
fn decode_chat_id_term(key: &[u8]) -> Option<i64> {
    let bytes: [u8; 8] = key.try_into().ok()?;
    Some(tantivy::u64_to_i64(u64::from_be_bytes(bytes)))
}

impl Indexer {
    /// Create or open an index at the given directory.
    pub async fn new(index_dir: &Path, from_scratch: bool) -> Result<Self> {
        tokio::fs::create_dir_all(index_dir).await?;

        let icu = ICUSearchConfig {
            max_snippet_chars: SNIPPET_MAX_CHARS,
        };

        if from_scratch && index_dir.join("meta.json").exists() {
            tokio::fs::remove_dir_all(index_dir).await?;
            tokio::fs::create_dir_all(index_dir).await?;
        }

        let (index, fields) = if index_dir.join("meta.json").exists() {
            let index = Index::open_in_dir(index_dir).map_err(|e| Error::Index(e.to_string()))?;
            let schema = index.schema();
            let fields = IndexFields::from_schema(&schema)?;
            (index, fields)
        } else {
            let (schema, fields) = Self::build_schema(&icu);
            let index =
                Index::create_in_dir(index_dir, schema).map_err(|e| Error::Index(e.to_string()))?;
            (index, fields)
        };

        icu.register_analyzers(&index);

        let writer = index
            .writer(50_000_000)
            .map_err(|e| Error::Index(e.to_string()))?;

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e| Error::Index(e.to_string()))?;

        Ok(Self {
            index,
            writer: Arc::new(RwLock::new(writer)),
            reader,
            fields,
            icu,
        })
    }

    fn build_schema(icu: &ICUSearchConfig) -> (Schema, IndexFields) {
        let mut schema_builder = Schema::builder();

        let content = icu.add_field_group(&mut schema_builder, "content");
        let url = schema_builder.add_text_field("url", STRING | STORED);
        let chat_id = schema_builder.add_i64_field("chat_id", INDEXED | STORED);
        let post_time = schema_builder.add_date_field("post_time", STORED | FAST);
        let sender = schema_builder.add_text_field("sender", STORED);

        let schema = schema_builder.build();
        let fields = IndexFields {
            content,
            url,
            chat_id,
            post_time,
            sender,
        };
        (schema, fields)
    }

    /// Build a tantivy document for a single [`IndexMsg`].
    ///
    /// Fans out the content text to all four ICU fields (stored, folded_bigram,
    /// unigram, diacritic).
    fn make_doc(&self, msg: &IndexMsg) -> tantivy::TantivyDocument {
        doc!(
            self.fields.content.stored => msg.content.as_str(),
            self.fields.content.folded_bigram => msg.content.as_str(),
            self.fields.content.unigram => msg.content.as_str(),
            self.fields.content.diacritic => msg.content.as_str(),
            self.fields.url => msg.url.as_str(),
            self.fields.chat_id => msg.chat_id,
            self.fields.post_time => tantivy::DateTime::from_timestamp_secs(msg.post_time.timestamp()),
            self.fields.sender => msg.sender.as_str(),
        )
    }

    /// Add a document to the index.
    pub async fn add_document(&self, msg: IndexMsg) -> Result<()> {
        let url_term = Term::from_field_text(self.fields.url, &msg.url);
        let doc = self.make_doc(&msg);

        let mut writer = self.writer.write().unwrap();
        writer.delete_term(url_term);
        writer
            .add_document(doc)
            .map_err(|e| Error::Index(e.to_string()))?;
        writer.commit().map_err(|e| Error::Index(e.to_string()))?;

        self.reader
            .reload()
            .map_err(|e| Error::Index(e.to_string()))?;

        Ok(())
    }

    /// Add multiple documents in batch (much faster than individual adds).
    pub async fn add_documents_batch(&self, msgs: Vec<IndexMsg>) -> Result<()> {
        if msgs.is_empty() {
            return Ok(());
        }

        let mut writer = self.writer.write().unwrap();

        let mut by_url: HashMap<String, IndexMsg> = HashMap::new();
        for msg in msgs {
            by_url.insert(msg.url.clone(), msg);
        }

        for (_, msg) in by_url {
            writer.delete_term(Term::from_field_text(self.fields.url, &msg.url));
            let doc = self.make_doc(&msg);
            writer
                .add_document(doc)
                .map_err(|e| Error::Index(e.to_string()))?;
        }

        writer.commit().map_err(|e| Error::Index(e.to_string()))?;

        self.reader
            .reload()
            .map_err(|e| Error::Index(e.to_string()))?;

        Ok(())
    }

    /// Update a document's content in the index.
    pub async fn update_document(&self, url: &str, content: &str) -> Result<()> {
        let searcher = self.reader.searcher();

        let url_term = Term::from_field_text(self.fields.url, url);
        let url_query = TermQuery::new(url_term.clone(), IndexRecordOption::Basic);

        let top_docs: Vec<(f32, tantivy::DocAddress)> = searcher
            .search(&url_query, &TopDocs::with_limit(1).order_by_score())
            .map_err(|e| Error::Index(e.to_string()))?;

        if let Some((_, doc_address)) = top_docs.first() {
            let doc: tantivy::TantivyDocument = searcher
                .doc(*doc_address)
                .map_err(|e| Error::Index(e.to_string()))?;

            let chat_id = doc
                .get_first(self.fields.chat_id)
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let post_time = doc
                .get_first(self.fields.post_time)
                .and_then(|v| v.as_datetime())
                .unwrap_or(tantivy::DateTime::from_timestamp_secs(0));
            let sender = doc
                .get_first(self.fields.sender)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let updated_doc = doc!(
                self.fields.content.stored => content,
                self.fields.content.folded_bigram => content,
                self.fields.content.unigram => content,
                self.fields.content.diacritic => content,
                self.fields.url => url,
                self.fields.chat_id => chat_id,
                self.fields.post_time => post_time,
                self.fields.sender => sender,
            );

            let mut writer = self.writer.write().unwrap();
            writer.delete_term(url_term);
            writer
                .add_document(updated_doc)
                .map_err(|e| Error::Index(e.to_string()))?;
            writer.commit().map_err(|e| Error::Index(e.to_string()))?;

            self.reader
                .reload()
                .map_err(|e| Error::Index(e.to_string()))?;
        }

        Ok(())
    }

    /// Delete a document from the index.
    pub async fn delete_document(&self, url: &str) -> Result<()> {
        let term = Term::from_field_text(self.fields.url, url);
        let mut writer = self.writer.write().unwrap();
        writer.delete_term(term);
        writer.commit().map_err(|e| Error::Index(e.to_string()))?;

        self.reader
            .reload()
            .map_err(|e| Error::Index(e.to_string()))?;

        Ok(())
    }

    /// Delete all documents for a specific chat.
    pub async fn delete_chat_documents(&self, chat_id: i64) -> Result<()> {
        let term = Term::from_field_i64(self.fields.chat_id, chat_id);
        let mut writer = self.writer.write().unwrap();

        writer.delete_term(term);
        writer.commit().map_err(|e| Error::Index(e.to_string()))?;

        self.reader
            .reload()
            .map_err(|e| Error::Index(e.to_string()))?;

        Ok(())
    }

    /// Search the index with optional chat filtering and pagination.
    pub async fn search(
        &self,
        query_str: &str,
        in_chats: Option<&[i64]>,
        page_len: usize,
        page_num: usize,
        sort_mode: SortMode,
        reverse: bool,
    ) -> Result<SearchResult> {
        let searcher = self.reader.searcher();

        let mut query: Box<dyn Query> = self
            .icu
            .route_query(&searcher, &self.fields.content, query_str)
            .map_err(|e| Error::Index(e.to_string()))?;

        if let Some(chats) = in_chats {
            let chat_queries: Vec<(Occur, Box<dyn Query>)> = chats
                .iter()
                .map(|&chat_id| {
                    let term = Term::from_field_i64(self.fields.chat_id, chat_id);
                    let q: Box<dyn Query> =
                        Box::new(TermQuery::new(term, IndexRecordOption::Basic));
                    (Occur::Should, q)
                })
                .collect();

            let chat_filter = BooleanQuery::new(chat_queries);

            let combined_query = BooleanQuery::new(vec![
                (Occur::Must, query),
                (Occur::Must, Box::new(chat_filter)),
            ]);
            query = Box::new(combined_query);
        }

        let offset = (page_num - 1) * page_len;
        let to_idx_err = |e: tantivy::TantivyError| Error::Index(e.to_string());

        let doc_addresses: Vec<tantivy::DocAddress> = match (sort_mode, reverse) {
            (SortMode::Time, rev) => {
                let order = if rev {
                    tantivy::Order::Asc
                } else {
                    tantivy::Order::Desc
                };
                let collector = TopDocs::with_limit(page_len)
                    .and_offset(offset)
                    .order_by_fast_field::<tantivy::DateTime>("post_time", order);
                searcher
                    .search(&query, &collector)
                    .map_err(to_idx_err)?
                    .into_iter()
                    .map(|(_dt, addr)| addr)
                    .collect()
            }
            (SortMode::Relevance, false) => {
                let collector = TopDocs::with_limit(page_len)
                    .and_offset(offset)
                    .order_by_score();
                searcher
                    .search(&query, &collector)
                    .map_err(to_idx_err)?
                    .into_iter()
                    .map(|(_score, addr)| addr)
                    .collect()
            }
            (SortMode::Relevance, true) => {
                let collector = TopDocs::with_limit(page_len)
                    .and_offset(offset)
                    .tweak_score(|_seg: &tantivy::SegmentReader| {
                        |_doc: tantivy::DocId, score: tantivy::Score| -> tantivy::Score { -score }
                    });
                let raw: Vec<(tantivy::Score, tantivy::DocAddress)> =
                    searcher.search(&query, &collector).map_err(to_idx_err)?;
                raw.into_iter().map(|(_neg, addr)| addr).collect()
            }
        };

        let total_results = searcher.search(&query, &Count).map_err(to_idx_err)?;

        let mut hits = Vec::new();
        for doc_address in doc_addresses {
            let doc: tantivy::TantivyDocument = searcher
                .doc(doc_address)
                .map_err(|e| Error::Index(e.to_string()))?;

            let content = html_escape::decode_html_entities(
                doc.get_first(self.fields.content.stored)
                    .and_then(|v| v.as_str())
                    .unwrap_or(""),
            )
            .into_owned();
            let url = doc
                .get_first(self.fields.url)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let chat_id = doc
                .get_first(self.fields.chat_id)
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let post_time_ts = doc
                .get_first(self.fields.post_time)
                .and_then(|v| v.as_datetime())
                .map(|dt| dt.into_timestamp_secs())
                .unwrap_or(0);
            let post_time =
                chrono::DateTime::from_timestamp(post_time_ts, 0).unwrap_or_else(chrono::Utc::now);
            let sender = doc
                .get_first(self.fields.sender)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let icu_snippet = self
                .icu
                .snippet(&searcher, &*query, &self.fields.content, &content);

            let msg = IndexMsg {
                content: content.clone(),
                url,
                chat_id,
                post_time,
                sender,
            };
            let snippet_data = if icu_snippet.highlights.is_empty() && !content.is_empty() {
                warn!(
                    url = %msg.url,
                    "Empty snippet highlights for non-empty content: {:?}",
                    content.chars().take(SNIPPET_MAX_CHARS).collect::<String>(),
                );
                let truncated: String = content.chars().take(SNIPPET_MAX_CHARS).collect();
                HighlightedSnippet {
                    fragment: truncated,
                    highlights: vec![],
                }
            } else {
                HighlightedSnippet {
                    fragment: icu_snippet.fragment,
                    highlights: icu_snippet.highlights,
                }
            };

            hits.push(SearchHit {
                msg,
                snippet: snippet_data,
            });
        }

        let is_last_page = offset + page_len >= total_results;

        Ok(SearchResult {
            hits,
            is_last_page,
            total_results,
        })
    }

    /// Total number of documents in the index (O(1)).
    pub fn num_docs(&self) -> u64 {
        self.reader.searcher().num_docs()
    }

    /// Number of documents indexed for a specific chat.
    pub async fn chat_doc_count(&self, chat_id: i64) -> Result<usize> {
        let searcher = self.reader.searcher();
        let term = Term::from_field_i64(self.fields.chat_id, chat_id);
        let query = TermQuery::new(term, IndexRecordOption::Basic);
        searcher
            .search(&query, &Count)
            .map_err(|e| Error::Index(e.to_string()))
    }

    /// Largest indexed msg_id for the given chat (by `post_time`, descending),
    /// extracted from the document's stored URL. `None` if the chat has no
    /// indexed documents.
    pub async fn latest_msg_id(&self, chat_id: i64) -> Result<Option<i32>> {
        let searcher = self.reader.searcher();
        let query = TermQuery::new(
            Term::from_field_i64(self.fields.chat_id, chat_id),
            IndexRecordOption::Basic,
        );
        let collector = TopDocs::with_limit(1)
            .order_by_fast_field::<tantivy::DateTime>("post_time", tantivy::Order::Desc);
        let docs = searcher
            .search(&query, &collector)
            .map_err(|e| Error::Index(e.to_string()))?;
        let Some((_, addr)) = docs.first() else {
            return Ok(None);
        };
        let doc: tantivy::TantivyDocument = searcher
            .doc(*addr)
            .map_err(|e| Error::Index(e.to_string()))?;
        let url = doc
            .get_first(self.fields.url)
            .and_then(|v| v.as_str())
            .unwrap_or("");
        Ok(parse_msg_id_from_url(url))
    }

    /// List all indexed chat IDs.
    ///
    /// Reads the `chat_id` field's term dictionary in each segment instead of
    /// scanning stored documents — O(unique chat ids) per segment with no
    /// store decompression.
    pub async fn list_indexed_chats(&self) -> Result<Vec<i64>> {
        let searcher = self.reader.searcher();
        let mut chat_ids = std::collections::HashSet::new();

        for segment_reader in searcher.segment_readers() {
            let inv = segment_reader
                .inverted_index(self.fields.chat_id)
                .map_err(|e| Error::Index(e.to_string()))?;
            let mut stream = inv
                .terms()
                .stream()
                .map_err(|e| Error::Index(e.to_string()))?;
            while stream.advance() {
                if let Some(id) = decode_chat_id_term(stream.key()) {
                    chat_ids.insert(id);
                }
            }
        }

        Ok(chat_ids.into_iter().collect())
    }

    /// Get document counts per chat (efficient single-pass counting).
    ///
    /// Uses term `doc_freq` from the `chat_id` field's inverted index instead
    /// of scanning stored documents.
    pub async fn get_chat_document_counts(&self) -> Result<HashMap<i64, usize>> {
        let searcher = self.reader.searcher();
        let mut counts: HashMap<i64, usize> = HashMap::new();

        for segment_reader in searcher.segment_readers() {
            let inv = segment_reader
                .inverted_index(self.fields.chat_id)
                .map_err(|e| Error::Index(e.to_string()))?;
            let mut stream = inv
                .terms()
                .stream()
                .map_err(|e| Error::Index(e.to_string()))?;
            while stream.advance() {
                if let Some(id) = decode_chat_id_term(stream.key()) {
                    *counts.entry(id).or_insert(0) += stream.value().doc_freq as usize;
                }
            }
        }

        Ok(counts)
    }

    /// Retrieve a random document (for /random command).
    pub async fn retrieve_random_document(&self) -> Result<Option<IndexMsg>> {
        let searcher = self.reader.searcher();
        let segment_readers = searcher.segment_readers();

        if segment_readers.is_empty() {
            return Ok(None);
        }

        use rand::RngExt;
        let mut rng = rand::rng();
        let segment = &segment_readers[rng.random_range(0..segment_readers.len())];
        let max_doc = segment.max_doc();

        if max_doc == 0 {
            return Ok(None);
        }

        let doc_id = rng.random_range(0..max_doc);
        let store_reader = segment
            .get_store_reader(0)
            .map_err(|e| Error::Index(e.to_string()))?;
        let doc: tantivy::TantivyDocument = store_reader
            .get(doc_id)
            .map_err(|e| Error::Index(e.to_string()))?;

        let content = doc
            .get_first(self.fields.content.stored)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let url = doc
            .get_first(self.fields.url)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let chat_id = doc
            .get_first(self.fields.chat_id)
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let post_time_ts = doc
            .get_first(self.fields.post_time)
            .and_then(|v| v.as_datetime())
            .map(|dt| dt.into_timestamp_secs())
            .unwrap_or(0);
        let post_time =
            chrono::DateTime::from_timestamp(post_time_ts, 0).unwrap_or_else(chrono::Utc::now);
        let sender = doc
            .get_first(self.fields.sender)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        Ok(Some(IndexMsg {
            content,
            url,
            chat_id,
            post_time,
            sender,
        }))
    }
}

impl IndexFields {
    fn from_schema(schema: &Schema) -> Result<Self> {
        let lookup = |name: &str| -> Result<Field> {
            schema
                .get_field(name)
                .map_err(|e| Error::Index(format!("missing field {name}: {e}")))
        };
        let content = ICUFieldGroup {
            stored: lookup("content")?,
            folded_bigram: lookup("content_folded_bigram")?,
            unigram: lookup("content_unigram")?,
            diacritic: lookup("content_diacritic")?,
        };
        Ok(IndexFields {
            content,
            url: lookup("url")?,
            chat_id: lookup("chat_id")?,
            post_time: lookup("post_time")?,
            sender: lookup("sender")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_indexer_basic_operations() {
        let temp_dir = TempDir::new().unwrap();
        let indexer = Indexer::new(temp_dir.path(), true).await.unwrap();

        let msg = IndexMsg {
            content: "test message hello world".to_string(),
            url: "https://t.me/c/123/456".to_string(),
            chat_id: 123,
            post_time: Utc::now(),
            sender: "Alice".to_string(),
        };

        indexer.add_document(msg.clone()).await.unwrap();

        let results = indexer
            .search("test", None, 10, 1, SortMode::Time, false)
            .await
            .unwrap();
        assert_eq!(results.total_results, 1);
        assert_eq!(results.hits[0].msg.content, msg.content);
    }

    #[tokio::test]
    async fn test_update_and_delete() {
        let temp_dir = TempDir::new().unwrap();
        let indexer = Indexer::new(temp_dir.path(), true).await.unwrap();

        let msg = IndexMsg {
            content: "original content".to_string(),
            url: "https://t.me/c/123/456".to_string(),
            chat_id: 123,
            post_time: Utc::now(),
            sender: "Bob".to_string(),
        };

        indexer.add_document(msg).await.unwrap();

        indexer
            .update_document("https://t.me/c/123/456", "updated content")
            .await
            .unwrap();

        let results = indexer
            .search("updated", None, 10, 1, SortMode::Time, false)
            .await
            .unwrap();
        assert_eq!(results.total_results, 1);

        indexer
            .delete_document("https://t.me/c/123/456")
            .await
            .unwrap();
        let results = indexer
            .search("updated", None, 10, 1, SortMode::Time, false)
            .await
            .unwrap();
        assert_eq!(results.total_results, 0);
    }

    #[tokio::test]
    async fn test_add_document_deduplicates_by_url() {
        let temp_dir = TempDir::new().unwrap();
        let indexer = Indexer::new(temp_dir.path(), true).await.unwrap();

        let url = "https://t.me/c/123/456".to_string();

        indexer
            .add_document(IndexMsg {
                content: "first".to_string(),
                url: url.clone(),
                chat_id: 123,
                post_time: Utc::now(),
                sender: "User".to_string(),
            })
            .await
            .unwrap();

        indexer
            .add_document(IndexMsg {
                content: "second".to_string(),
                url: url.clone(),
                chat_id: 123,
                post_time: Utc::now(),
                sender: "User".to_string(),
            })
            .await
            .unwrap();

        assert_eq!(indexer.num_docs(), 1);

        let results = indexer
            .search("second", None, 10, 1, SortMode::Time, false)
            .await
            .unwrap();
        assert_eq!(results.total_results, 1);
        assert_eq!(results.hits[0].msg.url, url);
    }

    #[tokio::test]
    async fn test_chat_filter() {
        let temp_dir = TempDir::new().unwrap();
        let indexer = Indexer::new(temp_dir.path(), true).await.unwrap();

        for chat_id in [100, 200, 300] {
            let msg = IndexMsg {
                content: format!("message from chat {}", chat_id),
                url: format!("https://t.me/c/{}/1", chat_id),
                chat_id,
                post_time: Utc::now(),
                sender: "User".to_string(),
            };
            indexer.add_document(msg).await.unwrap();
        }

        let results = indexer
            .search("message", Some(&[100, 200]), 10, 1, SortMode::Time, false)
            .await
            .unwrap();
        assert_eq!(results.total_results, 2);
    }

    #[tokio::test]
    async fn test_list_indexed_chats() {
        let temp_dir = TempDir::new().unwrap();
        let indexer = Indexer::new(temp_dir.path(), true).await.unwrap();

        for chat_id in [111, 222, 333] {
            let msg = IndexMsg {
                content: "test".to_string(),
                url: format!("https://t.me/c/{}/1", chat_id),
                chat_id,
                post_time: Utc::now(),
                sender: "User".to_string(),
            };
            indexer.add_document(msg).await.unwrap();
        }

        let mut chats = indexer.list_indexed_chats().await.unwrap();
        chats.sort();
        assert_eq!(chats, vec![111, 222, 333]);
    }

    #[tokio::test]
    async fn test_chinese_search_with_highlighting() {
        let temp_dir = TempDir::new().unwrap();
        let indexer = Indexer::new(temp_dir.path(), true).await.unwrap();

        let msg = IndexMsg {
            content: "人人都在说这个人很好".to_string(),
            url: "https://t.me/c/123/1".to_string(),
            chat_id: 123,
            post_time: Utc::now(),
            sender: "User".to_string(),
        };
        indexer.add_document(msg).await.unwrap();

        let results = indexer
            .search("人", None, 10, 1, SortMode::Time, false)
            .await
            .unwrap();
        assert_eq!(results.total_results, 1);
        assert!(results.hits[0].snippet.fragment.contains("人"));
        assert!(!results.hits[0].snippet.highlights.is_empty());
    }

    #[tokio::test]
    async fn test_chat_doc_count() {
        let temp_dir = TempDir::new().unwrap();
        let indexer = Indexer::new(temp_dir.path(), true).await.unwrap();

        for chat_id in [100, 100, 100, 200] {
            indexer
                .add_document(IndexMsg {
                    content: "x".to_string(),
                    url: format!(
                        "https://t.me/c/{}/{}",
                        chat_id,
                        rand::random::<u32>() // unique url per insert
                    ),
                    chat_id,
                    post_time: Utc::now(),
                    sender: "U".to_string(),
                })
                .await
                .unwrap();
        }

        assert_eq!(indexer.chat_doc_count(100).await.unwrap(), 3);
        assert_eq!(indexer.chat_doc_count(200).await.unwrap(), 1);
        assert_eq!(indexer.chat_doc_count(999).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_delete_chat_documents() {
        let temp_dir = TempDir::new().unwrap();
        let indexer = Indexer::new(temp_dir.path(), true).await.unwrap();

        for chat_id in [100, 200, 300] {
            for i in 1..=5 {
                let msg = IndexMsg {
                    content: format!("message {} from chat {}", i, chat_id),
                    url: format!("https://t.me/c/{}/{}", chat_id, i),
                    chat_id,
                    post_time: Utc::now(),
                    sender: "User".to_string(),
                };
                indexer.add_document(msg).await.unwrap();
            }
        }

        let results = indexer
            .search("message", None, 100, 1, SortMode::Time, false)
            .await
            .unwrap();
        assert_eq!(results.total_results, 15);

        indexer.delete_chat_documents(200).await.unwrap();

        let results = indexer
            .search("message", Some(&[200]), 100, 1, SortMode::Time, false)
            .await
            .unwrap();
        assert_eq!(results.total_results, 0);

        let results = indexer
            .search("message", Some(&[100, 300]), 100, 1, SortMode::Time, false)
            .await
            .unwrap();
        assert_eq!(results.total_results, 10);

        indexer.delete_chat_documents(100).await.unwrap();

        let results = indexer
            .search("message", None, 100, 1, SortMode::Time, false)
            .await
            .unwrap();
        assert_eq!(results.total_results, 5);
    }

    #[tokio::test]
    async fn test_latest_msg_id() {
        let temp_dir = TempDir::new().unwrap();
        let indexer = Indexer::new(temp_dir.path(), true).await.unwrap();

        assert_eq!(indexer.latest_msg_id(123).await.unwrap(), None);

        let base = Utc::now();
        for (msg_id, secs) in [(10, 0_i64), (42, 100), (7, 50)] {
            indexer
                .add_document(IndexMsg {
                    content: format!("msg {}", msg_id),
                    url: format!("https://t.me/c/123/{}", msg_id),
                    chat_id: 123,
                    post_time: base + chrono::Duration::seconds(secs),
                    sender: "U".to_string(),
                })
                .await
                .unwrap();
        }
        // Different chat shouldn't influence chat 123's result.
        indexer
            .add_document(IndexMsg {
                content: "other".to_string(),
                url: "https://t.me/c/999/9999".to_string(),
                chat_id: 999,
                post_time: base + chrono::Duration::seconds(1000),
                sender: "U".to_string(),
            })
            .await
            .unwrap();

        // msg_id=42 has the latest post_time within chat 123.
        assert_eq!(indexer.latest_msg_id(123).await.unwrap(), Some(42));
        assert_eq!(indexer.latest_msg_id(999).await.unwrap(), Some(9999));
        assert_eq!(indexer.latest_msg_id(555).await.unwrap(), None);
    }

    #[test]
    fn test_parse_msg_id_from_url() {
        assert_eq!(
            super::parse_msg_id_from_url("https://t.me/c/123/456"),
            Some(456)
        );
        assert_eq!(super::parse_msg_id_from_url(""), None);
        assert_eq!(super::parse_msg_id_from_url("not-a-url"), None);
        assert_eq!(super::parse_msg_id_from_url("https://t.me/c/123/"), None);
    }
}
