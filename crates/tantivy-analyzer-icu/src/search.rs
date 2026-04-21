//! High-level tantivy integration for ICU-based full-text search.
//!
//! Provides [`ICUSearchConfig`] which encapsulates the dual-field (bigram + unigram)
//! indexing scheme, query routing, and snippet generation with highlight merging.
//!
//! # Example
//!
//! ```ignore
//! let icu = ICUSearchConfig::default();
//!
//! // Schema setup
//! let mut builder = Schema::builder();
//! let content = icu.add_field_group(&mut builder, "content");
//! let schema = builder.build();
//! let index = Index::create_in_ram(schema);
//! icu.register_analyzers(&index);
//!
//! // Indexing
//! writer.add_document(doc!(
//!     content.stored => text,
//!     content.bigram => text,
//!     content.unigram => text,
//! ))?;
//!
//! // Query routing
//! let query = icu.route_query(&index, &content, "北京 我")?;
//!
//! // Snippet generation
//! let snippet = icu.snippet(&searcher, &query, &content, &body);
//! ```

use std::ops::Range;

use tantivy::query::{BooleanQuery, BoostQuery, EmptyQuery, Occur, Query, QueryParser, TermQuery};
use tantivy::schema::{
    Field, IndexRecordOption, STORED, SchemaBuilder, TextFieldIndexing, TextOptions,
};
use tantivy::snippet::SnippetGenerator;
use tantivy::tokenizer::TextAnalyzer;
use tantivy::{Index, Searcher, Term};
use tantivy_tokenizer_api::{Token, TokenStream};

use crate::filter::find_isolated_han_tokens;
use crate::{
    ArabicNormalizationFilter, CJKBigramFilter, DiacriticFoldingFilter, HanOnlyFilter,
    NormalizingICUTokenizer,
};

const DEFAULT_MAX_SNIPPET_CHARS: usize = 150;

/// A group of tantivy fields for ICU full-text search on a single text source.
///
/// Each group consists of three fields:
/// - `stored`: Contains the original text, stored but not indexed.
/// - `bigram`: Indexed with the bigram analyzer for multi-character queries.
/// - `unigram`: Indexed with the unigram analyzer for single/isolated Han char queries.
///
/// When indexing, all three fields should receive the same text content.
#[derive(Clone, Debug)]
pub struct ICUFieldGroup {
    /// Stored field containing the original text (not indexed).
    pub stored: Field,
    /// Indexed with bigram analyzer. Used for multi-char queries.
    pub bigram: Field,
    /// Indexed with unigram analyzer. Used for single/isolated Han char queries.
    pub unigram: Field,
}

/// Result of snippet generation with highlight ranges.
#[derive(Debug, Clone)]
pub struct ICUSnippet {
    /// The text fragment selected for display.
    pub fragment: String,
    /// Byte ranges within `fragment` to highlight. May overlap; consumers
    /// should merge before rendering.
    pub highlights: Vec<Range<usize>>,
}

/// Configuration and entry point for ICU-based full-text search.
///
/// Encapsulates the dual-field (bigram + unigram) indexing scheme, query
/// routing logic, and snippet generation with highlight merging.
pub struct ICUSearchConfig {
    /// Maximum number of characters in generated snippets. Default: 150.
    pub max_snippet_chars: usize,
}

impl Default for ICUSearchConfig {
    fn default() -> Self {
        Self {
            max_snippet_chars: DEFAULT_MAX_SNIPPET_CHARS,
        }
    }
}

impl ICUSearchConfig {
    /// Add a field group to the schema builder.
    ///
    /// Creates three fields:
    /// - `{name}` — stored, not indexed
    /// - `{name}_bigram` — indexed with `"icu_bigram"` tokenizer
    /// - `{name}_unigram` — indexed with `"icu_unigram"` tokenizer
    pub fn add_field_group(&self, builder: &mut SchemaBuilder, name: &str) -> ICUFieldGroup {
        let stored = builder.add_text_field(name, STORED);

        let bigram_indexing = TextFieldIndexing::default()
            .set_tokenizer("icu_bigram")
            .set_index_option(IndexRecordOption::WithFreqsAndPositions);
        let bigram = builder.add_text_field(
            &format!("{name}_bigram"),
            TextOptions::default().set_indexing_options(bigram_indexing),
        );

        let unigram_indexing = TextFieldIndexing::default()
            .set_tokenizer("icu_unigram")
            .set_index_option(IndexRecordOption::WithFreqsAndPositions);
        let unigram = builder.add_text_field(
            &format!("{name}_unigram"),
            TextOptions::default().set_indexing_options(unigram_indexing),
        );

        ICUFieldGroup {
            stored,
            bigram,
            unigram,
        }
    }

    /// Register the `"icu_bigram"` and `"icu_unigram"` analyzers on the index.
    ///
    /// Must be called after index creation, before indexing or searching.
    pub fn register_analyzers(&self, index: &Index) {
        let bigram = TextAnalyzer::builder(NormalizingICUTokenizer)
            .filter(DiacriticFoldingFilter)
            .filter(ArabicNormalizationFilter)
            .filter(CJKBigramFilter)
            .build();
        index.tokenizers().register("icu_bigram", bigram);

        let unigram = TextAnalyzer::builder(NormalizingICUTokenizer)
            .filter(DiacriticFoldingFilter)
            .filter(ArabicNormalizationFilter)
            .filter(HanOnlyFilter)
            .build();
        index.tokenizers().register("icu_unigram", unigram);
    }

    /// Route a query for the given field group.
    ///
    /// Analyzes the query text to determine which Han characters are isolated
    /// (not adjacent to other CJK characters) and builds a query accordingly:
    /// - Adjacent CJK characters → bigram field only (boosted 2x)
    /// - Isolated Han characters → manual unigram TermQueries
    /// - Non-CJK text → bigram field passthrough
    pub fn route_query(
        &self,
        index: &Index,
        fields: &ICUFieldGroup,
        query_text: &str,
    ) -> tantivy::Result<Box<dyn Query>> {
        let base_tokens = self.base_tokenize(query_text);
        let isolated_han = find_isolated_han_tokens(&base_tokens);

        let bigram_parser = QueryParser::for_index(index, vec![fields.bigram]);
        // The bigram parser may return AllButQueryForbidden when the bigram
        // analyzer drops all tokens (e.g. query "京 东" where both chars are
        // isolated Han and dropped by CJKBigramFilter). Fall back to EmptyQuery.
        let bigram_q: Box<dyn Query> = bigram_parser
            .parse_query(query_text)
            .unwrap_or_else(|_| Box::new(EmptyQuery));

        if isolated_han.is_empty() {
            Ok(Box::new(BoostQuery::new(bigram_q, 2.0)))
        } else {
            let mut clauses: Vec<(Occur, Box<dyn Query>)> =
                vec![(Occur::Should, Box::new(BoostQuery::new(bigram_q, 2.0)))];
            for han_text in &isolated_han {
                let term = Term::from_field_text(fields.unigram, han_text);
                clauses.push((
                    Occur::Should,
                    Box::new(TermQuery::new(term, IndexRecordOption::WithFreqs)),
                ));
            }
            Ok(Box::new(BooleanQuery::new(clauses)))
        }
    }

    /// Generate a snippet with dual-field fallback and highlight merging.
    ///
    /// - Tries bigram highlights first, falls back to unigram.
    /// - When bigram is primary, also scans with unigram to merge additional
    ///   highlights (e.g. isolated Han chars that the bigram analyzer drops).
    /// - Works around tantivy snippet fragment truncation for short bodies.
    pub fn snippet(
        &self,
        searcher: &Searcher,
        query: &dyn Query,
        fields: &ICUFieldGroup,
        body: &str,
    ) -> ICUSnippet {
        let mut bigram_gen = match SnippetGenerator::create(searcher, query, fields.bigram) {
            Ok(g) => g,
            Err(_) => {
                return ICUSnippet {
                    fragment: body.to_string(),
                    highlights: vec![],
                };
            }
        };
        let mut unigram_gen = match SnippetGenerator::create(searcher, query, fields.unigram) {
            Ok(g) => g,
            Err(_) => {
                return ICUSnippet {
                    fragment: body.to_string(),
                    highlights: vec![],
                };
            }
        };
        bigram_gen.set_max_num_chars(self.max_snippet_chars);
        unigram_gen.set_max_num_chars(self.max_snippet_chars);

        let bigram_snippet = bigram_gen.snippet(body);
        let (snippet_fragment, mut highlighted_ranges) = if !bigram_snippet.highlighted().is_empty()
        {
            let fragment = bigram_snippet.fragment();
            let mut ranges: Vec<Range<usize>> = bigram_snippet.highlighted().to_vec();
            // Scan the same fragment with unigram to find extra highlights.
            let unigram_extra = unigram_gen.snippet(fragment);
            ranges.extend_from_slice(unigram_extra.highlighted());
            (fragment.to_string(), ranges)
        } else {
            let unigram_snippet = unigram_gen.snippet(body);
            let fragment = unigram_snippet.fragment().to_string();
            let ranges = unigram_snippet.highlighted().to_vec();
            (fragment, ranges)
        };

        // Workaround for tantivy snippet truncation: the snippet fragment's
        // right boundary is set by the last token's offset_to. If the analyzer
        // drops trailing tokens (e.g. HanOnlyFilter discards non-Han chars),
        // the fragment is truncated. For bodies within the snippet char limit
        // whose fragment starts at byte 0, we can safely extend to the full
        // body since the highlight ranges (relative to fragment start) stay valid.
        let snippet_fragment = if snippet_fragment.len() < body.len()
            && body.chars().count() <= self.max_snippet_chars
            && body.starts_with(&snippet_fragment)
        {
            let extra = unigram_gen.snippet(body);
            highlighted_ranges.extend_from_slice(extra.highlighted());
            body.to_string()
        } else {
            snippet_fragment
        };

        ICUSnippet {
            fragment: snippet_fragment,
            highlights: highlighted_ranges,
        }
    }

    /// Tokenize text with the base analyzer (no CJK filters).
    fn base_tokenize(&self, text: &str) -> Vec<Token> {
        let mut analyzer = TextAnalyzer::builder(NormalizingICUTokenizer)
            .filter(DiacriticFoldingFilter)
            .filter(ArabicNormalizationFilter)
            .build();
        let mut stream = analyzer.token_stream(text);
        let mut tokens = Vec::new();
        while stream.advance() {
            tokens.push(stream.token().clone());
        }
        tokens
    }
}
