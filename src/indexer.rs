//! Full-text search indexer using Tantivy with Chinese tokenization
//!
//! This module provides a wrapper around Tantivy for indexing and searching
//! Telegram messages with support for Chinese word segmentation via jieba.

use crate::types::{Error, IndexMsg, Result, SearchHit, SearchResult};
use jieba_rs::Jieba;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, Occur, Query, QueryParser, TermQuery};
use tantivy::schema::*;
use tantivy::snippet::SnippetGenerator;
use tantivy::tokenizer::{Token, TokenStream, Tokenizer};
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, Term, doc};

/// Chinese tokenizer using jieba
#[derive(Clone)]
pub struct ChineseTokenizer {
    jieba: Arc<Jieba>,
}

impl ChineseTokenizer {
    pub fn new() -> Self {
        Self {
            jieba: Arc::new(Jieba::new()),
        }
    }
}

impl Tokenizer for ChineseTokenizer {
    type TokenStream<'a> = ChineseTokenStream<'a>;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        ChineseTokenStream::new(text, self.jieba.clone())
    }
}

/// Token stream for Chinese text
pub struct ChineseTokenStream<'a> {
    tokens: Vec<Token>,
    index: usize,
    _text: &'a str,
}

impl<'a> ChineseTokenStream<'a> {
    fn new(text: &'a str, jieba: Arc<Jieba>) -> Self {
        // Use jieba to segment the text
        let words = jieba.cut(text, false);
        let mut tokens = Vec::new();
        let mut byte_offset = 0;

        for (position, word) in words.into_iter().enumerate() {
            let word_bytes = word.len();
            let token = Token {
                offset_from: byte_offset,
                offset_to: byte_offset + word_bytes,
                position,
                text: word.to_string(),
                position_length: 1,
            };
            tokens.push(token);
            byte_offset += word_bytes;
        }

        Self {
            tokens,
            index: 0,
            _text: text,
        }
    }
}

impl TokenStream for ChineseTokenStream<'_> {
    fn advance(&mut self) -> bool {
        if self.index < self.tokens.len() {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn token(&self) -> &Token {
        &self.tokens[self.index - 1]
    }

    fn token_mut(&mut self) -> &mut Token {
        &mut self.tokens[self.index - 1]
    }
}

/// Indexer for full-text search
pub struct Indexer {
    index: Index,
    writer: Arc<RwLock<IndexWriter>>,
    reader: IndexReader,
    fields: IndexFields,
}

struct IndexFields {
    content: Field,
    url: Field,
    chat_id: Field,
    post_time: Field,
    sender: Field,
}

impl Indexer {
    /// Create or open an index
    pub async fn new(index_dir: &Path, from_scratch: bool) -> Result<Self> {
        // Create directory if it doesn't exist
        tokio::fs::create_dir_all(index_dir).await?;

        // Build schema matching Python's Whoosh schema
        let schema = Self::build_schema();

        // Clear index if requested
        if from_scratch && index_dir.join("meta.json").exists() {
            tokio::fs::remove_dir_all(index_dir).await?;
            tokio::fs::create_dir_all(index_dir).await?;
        }

        // Open or create index
        let index = if index_dir.join("meta.json").exists() {
            Index::open_in_dir(index_dir).map_err(|e| Error::Index(e.to_string()))?
        } else {
            Index::create_in_dir(index_dir, schema.clone())
                .map_err(|e| Error::Index(e.to_string()))?
        };

        // Register Chinese tokenizer
        index
            .tokenizers()
            .register("jieba", ChineseTokenizer::new());

        // Create writer with 50MB heap
        let writer = index
            .writer(50_000_000)
            .map_err(|e| Error::Index(e.to_string()))?;

        // Create reader with auto-reload
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e| Error::Index(e.to_string()))?;

        let fields = IndexFields {
            content: schema.get_field("content").unwrap(),
            url: schema.get_field("url").unwrap(),
            chat_id: schema.get_field("chat_id").unwrap(),
            post_time: schema.get_field("post_time").unwrap(),
            sender: schema.get_field("sender").unwrap(),
        };

        Ok(Self {
            index,
            writer: Arc::new(RwLock::new(writer)),
            reader,
            fields,
        })
    }

    /// Build Tantivy schema matching Python's Whoosh schema
    fn build_schema() -> Schema {
        let mut schema_builder = Schema::builder();

        // content: TEXT with Chinese analyzer, stored
        let text_options = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("jieba")
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions),
            )
            .set_stored();
        schema_builder.add_text_field("content", text_options);

        // url: ID (STRING), stored, indexed (for unique lookups)
        schema_builder.add_text_field("url", STRING | STORED);

        // chat_id: i64, stored, indexed (for filtering)
        schema_builder.add_i64_field("chat_id", INDEXED | STORED);

        // post_time: DATETIME, stored, indexed, fast (for sorting)
        schema_builder.add_date_field("post_time", INDEXED | STORED | FAST);

        // sender: TEXT, stored
        schema_builder.add_text_field("sender", STORED);

        schema_builder.build()
    }

    /// Add a document to the index
    pub async fn add_document(&self, msg: IndexMsg) -> Result<()> {
        // Deduplicate by URL (Telegram message ID is encoded in the URL).
        // Tantivy doesn't enforce uniqueness, so we explicitly delete any existing doc first.
        let url_term = Term::from_field_text(self.fields.url, &msg.url);
        let doc = doc!(
            self.fields.content => msg.content,
            self.fields.url => msg.url,
            self.fields.chat_id => msg.chat_id,
            self.fields.post_time => tantivy::DateTime::from_timestamp_secs(msg.post_time.timestamp()),
            self.fields.sender => msg.sender,
        );

        let mut writer = self.writer.write().unwrap();
        writer.delete_term(url_term);
        writer
            .add_document(doc)
            .map_err(|e| Error::Index(e.to_string()))?;
        writer.commit().map_err(|e| Error::Index(e.to_string()))?;

        // Reload reader to see changes
        self.reader
            .reload()
            .map_err(|e| Error::Index(e.to_string()))?;

        Ok(())
    }

    /// Add multiple documents in batch (much faster than individual adds)
    pub async fn add_documents_batch(&self, msgs: Vec<IndexMsg>) -> Result<()> {
        if msgs.is_empty() {
            return Ok(());
        }

        let mut writer = self.writer.write().unwrap();

        // Deduplicate by URL within the batch as well (keep the last occurrence).
        let mut by_url: HashMap<String, IndexMsg> = HashMap::new();
        for msg in msgs {
            by_url.insert(msg.url.clone(), msg);
        }

        for (_, msg) in by_url {
            writer.delete_term(Term::from_field_text(self.fields.url, &msg.url));
            let doc = doc!(
                self.fields.content => msg.content,
                self.fields.url => msg.url,
                self.fields.chat_id => msg.chat_id,
                self.fields.post_time => tantivy::DateTime::from_timestamp_secs(msg.post_time.timestamp()),
                self.fields.sender => msg.sender,
            );
            writer
                .add_document(doc)
                .map_err(|e| Error::Index(e.to_string()))?;
        }

        // Commit once for all documents
        writer.commit().map_err(|e| Error::Index(e.to_string()))?;

        // Reload reader to see changes
        self.reader
            .reload()
            .map_err(|e| Error::Index(e.to_string()))?;

        Ok(())
    }

    /// Update a document in the index
    pub async fn update_document(&self, url: &str, content: &str) -> Result<()> {
        let searcher = self.reader.searcher();

        // Find existing document by URL
        let url_term = Term::from_field_text(self.fields.url, url);
        let url_query = TermQuery::new(url_term.clone(), IndexRecordOption::Basic);

        let top_docs = searcher
            .search(&url_query, &TopDocs::with_limit(1))
            .map_err(|e| Error::Index(e.to_string()))?;

        if let Some((_, doc_address)) = top_docs.first() {
            let doc: tantivy::TantivyDocument = searcher
                .doc(*doc_address)
                .map_err(|e| Error::Index(e.to_string()))?;

            // Extract existing fields
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

            // Create updated document
            let updated_doc = doc!(
                self.fields.content => content,
                self.fields.url => url,
                self.fields.chat_id => chat_id,
                self.fields.post_time => post_time,
                self.fields.sender => sender,
            );

            // Delete old and add new
            let mut writer = self.writer.write().unwrap();
            writer.delete_term(url_term);
            writer
                .add_document(updated_doc)
                .map_err(|e| Error::Index(e.to_string()))?;
            writer.commit().map_err(|e| Error::Index(e.to_string()))?;

            // Reload reader to see changes
            self.reader
                .reload()
                .map_err(|e| Error::Index(e.to_string()))?;
        }

        Ok(())
    }

    /// Delete a document from the index
    pub async fn delete_document(&self, url: &str) -> Result<()> {
        let term = Term::from_field_text(self.fields.url, url);
        let mut writer = self.writer.write().unwrap();
        writer.delete_term(term);
        writer.commit().map_err(|e| Error::Index(e.to_string()))?;

        // Reload reader to see changes
        self.reader
            .reload()
            .map_err(|e| Error::Index(e.to_string()))?;

        Ok(())
    }

    /// Delete all documents for a specific chat
    pub async fn delete_chat_documents(&self, chat_id: i64) -> Result<()> {
        let term = Term::from_field_i64(self.fields.chat_id, chat_id);
        let mut writer = self.writer.write().unwrap();

        // Delete all documents matching this chat_id
        writer.delete_term(term);
        writer.commit().map_err(|e| Error::Index(e.to_string()))?;

        // Reload reader to see changes
        self.reader
            .reload()
            .map_err(|e| Error::Index(e.to_string()))?;

        Ok(())
    }

    /// Search the index
    pub async fn search(
        &self,
        query_str: &str,
        in_chats: Option<&[i64]>,
        page_len: usize,
        page_num: usize,
    ) -> Result<SearchResult> {
        let searcher = self.reader.searcher();

        // Parse query for content field
        let query_parser = QueryParser::for_index(&self.index, vec![self.fields.content]);
        let mut query = query_parser
            .parse_query(query_str)
            .map_err(|e| Error::Index(e.to_string()))?;

        // Add chat filter if specified
        if let Some(chats) = in_chats {
            let chat_queries: Vec<(Occur, Box<dyn Query>)> = chats
                .iter()
                .map(|&chat_id| {
                    let term = Term::from_field_i64(self.fields.chat_id, chat_id);
                    let query: Box<dyn Query> =
                        Box::new(TermQuery::new(term, IndexRecordOption::Basic));
                    (Occur::Should, query)
                })
                .collect();

            let chat_filter = BooleanQuery::new(chat_queries);

            // Combine content query with chat filter
            let combined_query = BooleanQuery::new(vec![
                (Occur::Must, Box::new(query)),
                (Occur::Must, Box::new(chat_filter)),
            ]);
            query = Box::new(combined_query);
        }

        // Calculate offset
        let offset = (page_num - 1) * page_len;

        // Search with sorting by post_time descending
        let collector = TopDocs::with_limit(page_len)
            .and_offset(offset)
            .order_by_fast_field::<tantivy::DateTime>("post_time", tantivy::Order::Desc);

        let top_docs = searcher
            .search(&query, &collector)
            .map_err(|e| Error::Index(e.to_string()))?;

        // Get total count
        let count_collector = tantivy::collector::Count;
        let total_results = searcher
            .search(&query, &count_collector)
            .map_err(|e| Error::Index(e.to_string()))?;

        // Create snippet generator for highlighting
        let mut snippet_generator =
            SnippetGenerator::create(&searcher, &*query, self.fields.content)
                .map_err(|e| Error::Index(e.to_string()))?;
        snippet_generator.set_max_num_chars(100);

        // Convert results to SearchHits
        let mut hits = Vec::new();
        for (_score, doc_address) in top_docs {
            let doc: tantivy::TantivyDocument = searcher
                .doc(doc_address)
                .map_err(|e| Error::Index(e.to_string()))?;

            // Extract fields
            let content = doc
                .get_first(self.fields.content)
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

            let msg = IndexMsg {
                content: content.clone(),
                url,
                chat_id,
                post_time,
                sender,
            };

            // Generate highlighted snippet
            let snippet = snippet_generator.snippet_from_doc(&doc);
            let highlighted = snippet.to_html();

            hits.push(SearchHit { msg, highlighted });
        }

        let is_last_page = offset + page_len >= total_results;

        Ok(SearchResult {
            hits,
            is_last_page,
            total_results,
        })
    }

    /// List all indexed chat IDs
    pub async fn list_indexed_chats(&self) -> Result<Vec<i64>> {
        let searcher = self.reader.searcher();
        let mut chat_ids = std::collections::HashSet::new();

        // Iterate through all documents and collect unique chat_ids
        for segment_reader in searcher.segment_readers() {
            let store_reader = segment_reader
                .get_store_reader(0)
                .map_err(|e| Error::Index(e.to_string()))?;

            for doc_id in 0..segment_reader.max_doc() {
                if let Ok(doc) = store_reader.get::<tantivy::TantivyDocument>(doc_id)
                    && let Some(chat_id_value) = doc.get_first(self.fields.chat_id)
                    && let Some(chat_id) = chat_id_value.as_i64()
                {
                    chat_ids.insert(chat_id);
                }
            }
        }

        Ok(chat_ids.into_iter().collect())
    }

    /// Get document counts per chat (efficient single-pass counting)
    /// Returns a HashMap of chat_id -> document_count
    pub async fn get_chat_document_counts(&self) -> Result<std::collections::HashMap<i64, usize>> {
        let searcher = self.reader.searcher();
        let mut counts: std::collections::HashMap<i64, usize> = std::collections::HashMap::new();

        // Iterate through all documents and count by chat_id
        for segment_reader in searcher.segment_readers() {
            let store_reader = segment_reader
                .get_store_reader(0)
                .map_err(|e| Error::Index(e.to_string()))?;

            for doc_id in 0..segment_reader.max_doc() {
                if let Ok(doc) = store_reader.get::<tantivy::TantivyDocument>(doc_id)
                    && let Some(chat_id_value) = doc.get_first(self.fields.chat_id)
                    && let Some(chat_id) = chat_id_value.as_i64()
                {
                    *counts.entry(chat_id).or_insert(0) += 1;
                }
            }
        }

        Ok(counts)
    }

    /// Retrieve a random document (for /random command)
    pub async fn retrieve_random_document(&self) -> Result<Option<IndexMsg>> {
        let searcher = self.reader.searcher();
        let segment_readers = searcher.segment_readers();

        if segment_readers.is_empty() {
            return Ok(None);
        }

        // Simple random selection: pick random segment and document
        use rand::Rng;
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

        // Extract fields
        let content = doc
            .get_first(self.fields.content)
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_indexer_basic_operations() {
        let temp_dir = TempDir::new().unwrap();
        let indexer = Indexer::new(temp_dir.path(), true).await.unwrap();

        // Add a document
        let msg = IndexMsg {
            content: "test message hello world".to_string(),
            url: "https://t.me/c/123/456".to_string(),
            chat_id: 123,
            post_time: Utc::now(),
            sender: "Alice".to_string(),
        };

        indexer.add_document(msg.clone()).await.unwrap();

        // Search for it
        let results = indexer.search("test", None, 10, 1).await.unwrap();
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

        // Update
        indexer
            .update_document("https://t.me/c/123/456", "updated content")
            .await
            .unwrap();

        let results = indexer.search("updated", None, 10, 1).await.unwrap();
        assert_eq!(results.total_results, 1);

        // Delete
        indexer
            .delete_document("https://t.me/c/123/456")
            .await
            .unwrap();
        let results = indexer.search("updated", None, 10, 1).await.unwrap();
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

        let results = indexer.search("*", None, 10, 1).await.unwrap();
        assert_eq!(results.total_results, 1);

        let results = indexer.search("second", None, 10, 1).await.unwrap();
        assert_eq!(results.total_results, 1);
        assert_eq!(results.hits[0].msg.url, url);
    }

    #[tokio::test]
    async fn test_chat_filter() {
        let temp_dir = TempDir::new().unwrap();
        let indexer = Indexer::new(temp_dir.path(), true).await.unwrap();

        // Add messages from different chats
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

        // Search in specific chats
        let results = indexer
            .search("message", Some(&[100, 200]), 10, 1)
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

        // Test with repeated Chinese characters
        let msg = IndexMsg {
            content: "人人都在说这个人很好".to_string(),
            url: "https://t.me/c/123/1".to_string(),
            chat_id: 123,
            post_time: Utc::now(),
            sender: "User".to_string(),
        };
        indexer.add_document(msg).await.unwrap();

        // Search for single character that appears multiple times
        let results = indexer.search("人", None, 10, 1).await.unwrap();
        assert_eq!(results.total_results, 1);
        assert!(results.hits[0].highlighted.contains("<b>人</b>"));
    }

    #[tokio::test]
    async fn test_delete_chat_documents() {
        let temp_dir = TempDir::new().unwrap();
        let indexer = Indexer::new(temp_dir.path(), true).await.unwrap();

        // Add messages from multiple chats
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

        // Verify all messages are indexed
        let results = indexer.search("message", None, 100, 1).await.unwrap();
        assert_eq!(results.total_results, 15); // 3 chats * 5 messages

        // Delete all documents from chat 200
        indexer.delete_chat_documents(200).await.unwrap();

        // Verify chat 200 messages are gone
        let results = indexer
            .search("message", Some(&[200]), 100, 1)
            .await
            .unwrap();
        assert_eq!(results.total_results, 0);

        // Verify other chats still exist
        let results = indexer
            .search("message", Some(&[100, 300]), 100, 1)
            .await
            .unwrap();
        assert_eq!(results.total_results, 10); // 2 chats * 5 messages

        // Delete all documents from chat 100
        indexer.delete_chat_documents(100).await.unwrap();

        // Verify only chat 300 remains
        let results = indexer.search("message", None, 100, 1).await.unwrap();
        assert_eq!(results.total_results, 5);
    }
}
