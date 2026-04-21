//! High-level tantivy integration for ICU-based full-text search.
//!
//! Provides [`ICUSearchConfig`] which encapsulates the three-field
//! (folded_bigram + unigram + diacritic) indexing scheme, smartcase query
//! routing, and snippet generation with highlight merging.
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
//! // Indexing — all four fields get the same text
//! writer.add_document(doc!(
//!     content.stored => text,
//!     content.folded_bigram => text,
//!     content.unigram => text,
//!     content.diacritic => text,
//! ))?;
//!
//! // Query routing (smartcase: café→diacritic, cafe→folded_bigram)
//! let query = icu.route_query(&index, &content, "café 北京 我")?;
//!
//! // Snippet generation with three-way highlight merging
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

use crate::filter::{find_isolated_han_tokens, has_foldable_diacritic};
use crate::{
    CJKBigramFilter, DiacriticFoldingFilter, DiacriticOnlyFilter, HanOnlyFilter,
    NormalizingICUTokenizer, SemiticNormalizationFilter,
};

const DEFAULT_MAX_SNIPPET_CHARS: usize = 150;

/// A group of tantivy fields for ICU full-text search on a single text source.
///
/// Each group consists of four fields:
/// - `stored`: Contains the original text, stored but not indexed.
/// - `folded_bigram`: Indexed with diacritic-folded bigram analyzer. Primary recall field.
/// - `unigram`: Indexed with unigram analyzer for single/isolated Han char queries.
/// - `diacritic`: Sparse field indexed with diacritic-only analyzer for smartcase
///   exact-accent matching.
///
/// When indexing, all four fields should receive the same text content.
#[derive(Clone, Debug)]
pub struct ICUFieldGroup {
    /// Stored field containing the original text (not indexed).
    pub stored: Field,
    /// Indexed with folded bigram analyzer. Primary recall field for multi-char queries.
    pub folded_bigram: Field,
    /// Indexed with unigram analyzer. Used for single/isolated Han char queries.
    pub unigram: Field,
    /// Sparse field indexed with diacritic-only analyzer. Only contains tokens
    /// with foldable diacritics in their original (pre-fold) form.
    pub diacritic: Field,
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
/// Encapsulates the three-field (folded_bigram + unigram + diacritic) indexing
/// scheme, smartcase query routing logic, and snippet generation with highlight
/// merging.
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
    /// Creates four fields:
    /// - `{name}` — stored, not indexed
    /// - `{name}_folded_bigram` — indexed with `"icu_folded_bigram"` tokenizer
    /// - `{name}_unigram` — indexed with `"icu_unigram"` tokenizer
    /// - `{name}_diacritic` — indexed with `"icu_diacritic"` tokenizer
    pub fn add_field_group(&self, builder: &mut SchemaBuilder, name: &str) -> ICUFieldGroup {
        let stored = builder.add_text_field(name, STORED);

        let folded_bigram_indexing = TextFieldIndexing::default()
            .set_tokenizer("icu_folded_bigram")
            .set_index_option(IndexRecordOption::WithFreqsAndPositions);
        let folded_bigram = builder.add_text_field(
            &format!("{name}_folded_bigram"),
            TextOptions::default().set_indexing_options(folded_bigram_indexing),
        );

        let unigram_indexing = TextFieldIndexing::default()
            .set_tokenizer("icu_unigram")
            .set_index_option(IndexRecordOption::WithFreqsAndPositions);
        let unigram = builder.add_text_field(
            &format!("{name}_unigram"),
            TextOptions::default().set_indexing_options(unigram_indexing),
        );

        let diacritic_indexing = TextFieldIndexing::default()
            .set_tokenizer("icu_diacritic")
            .set_index_option(IndexRecordOption::WithFreqsAndPositions);
        let diacritic = builder.add_text_field(
            &format!("{name}_diacritic"),
            TextOptions::default().set_indexing_options(diacritic_indexing),
        );

        ICUFieldGroup {
            stored,
            folded_bigram,
            unigram,
            diacritic,
        }
    }

    /// Register the ICU analyzers on the index.
    ///
    /// Three analyzers are registered:
    /// - `"icu_folded_bigram"`: SemiticNorm → DiacriticFolding → CJKBigram
    /// - `"icu_unigram"`: SemiticNorm → DiacriticFolding → HanOnly
    /// - `"icu_diacritic"`: SemiticNorm → DiacriticOnly (sparse, pre-fold form)
    ///
    /// Must be called after index creation, before indexing or searching.
    pub fn register_analyzers(&self, index: &Index) {
        let folded_bigram = TextAnalyzer::builder(NormalizingICUTokenizer)
            .filter(SemiticNormalizationFilter)
            .filter(DiacriticFoldingFilter)
            .filter(CJKBigramFilter)
            .build();
        index
            .tokenizers()
            .register("icu_folded_bigram", folded_bigram);

        let unigram = TextAnalyzer::builder(NormalizingICUTokenizer)
            .filter(SemiticNormalizationFilter)
            .filter(DiacriticFoldingFilter)
            .filter(HanOnlyFilter)
            .build();
        index.tokenizers().register("icu_unigram", unigram);

        let diacritic = TextAnalyzer::builder(NormalizingICUTokenizer)
            .filter(SemiticNormalizationFilter)
            .filter(DiacriticOnlyFilter)
            .build();
        index.tokenizers().register("icu_diacritic", diacritic);
    }

    /// Route a query for the given field group (smartcase).
    ///
    /// Analyzes the query text and dispatches to appropriate fields:
    /// - Adjacent CJK characters → folded_bigram field (boosted 2x)
    /// - Isolated Han characters → manual unigram TermQueries
    /// - Non-CJK text → folded_bigram field passthrough
    /// - If the query contains foldable diacritics → also diacritic field (boosted 3x)
    pub fn route_query(
        &self,
        index: &Index,
        fields: &ICUFieldGroup,
        query_text: &str,
    ) -> tantivy::Result<Box<dyn Query>> {
        let base_tokens = self.base_tokenize(query_text);
        let isolated_han = find_isolated_han_tokens(&base_tokens);
        let query_has_diacritic = has_foldable_diacritic(query_text);

        // folded_bigram clause (always present)
        let folded_bigram_parser = QueryParser::for_index(index, vec![fields.folded_bigram]);
        // The parser may return AllButQueryForbidden when the bigram analyzer
        // drops all tokens (e.g. "京 东" where both are isolated Han).
        let folded_bigram_q: Box<dyn Query> = folded_bigram_parser
            .parse_query(query_text)
            .unwrap_or_else(|_| Box::new(EmptyQuery));

        let needs_multi_field = !isolated_han.is_empty() || query_has_diacritic;

        if !needs_multi_field {
            return Ok(Box::new(BoostQuery::new(folded_bigram_q, 2.0)));
        }

        let mut clauses: Vec<(Occur, Box<dyn Query>)> = vec![(
            Occur::Should,
            Box::new(BoostQuery::new(folded_bigram_q, 2.0)),
        )];

        // Unigram clauses for isolated Han characters
        for han_text in &isolated_han {
            let term = Term::from_field_text(fields.unigram, han_text);
            clauses.push((
                Occur::Should,
                Box::new(TermQuery::new(term, IndexRecordOption::WithFreqs)),
            ));
        }

        // Diacritic clause: only when query contains foldable diacritics
        if query_has_diacritic {
            let diacritic_parser = QueryParser::for_index(index, vec![fields.diacritic]);
            if let Ok(dq) = diacritic_parser.parse_query(query_text) {
                clauses.push((Occur::Should, Box::new(BoostQuery::new(dq, 3.0))));
            }
        }

        Ok(Box::new(BooleanQuery::new(clauses)))
    }

    /// Generate a snippet with three-field fallback and highlight merging.
    ///
    /// - Tries folded_bigram highlights first, falls back to unigram, then diacritic.
    /// - When folded_bigram is primary, also scans with unigram and diacritic to
    ///   merge additional highlights.
    /// - Works around tantivy snippet fragment truncation for short bodies.
    pub fn snippet(
        &self,
        searcher: &Searcher,
        query: &dyn Query,
        fields: &ICUFieldGroup,
        body: &str,
    ) -> ICUSnippet {
        let make_snippet_gen = |field: Field| -> Option<SnippetGenerator> {
            let mut sg = SnippetGenerator::create(searcher, query, field).ok()?;
            sg.set_max_num_chars(self.max_snippet_chars);
            Some(sg)
        };

        let folded_bigram_gen = make_snippet_gen(fields.folded_bigram);
        let unigram_gen = make_snippet_gen(fields.unigram);
        let diacritic_gen = make_snippet_gen(fields.diacritic);

        // Try folded_bigram first as primary source
        let bigram_snippet = folded_bigram_gen.as_ref().map(|g| g.snippet(body));
        let bigram_has_highlights = bigram_snippet
            .as_ref()
            .is_some_and(|s| !s.highlighted().is_empty());

        let (snippet_fragment, mut highlighted_ranges) = if bigram_has_highlights {
            let snippet = bigram_snippet.unwrap();
            let fragment = snippet.fragment().to_string();
            let mut ranges: Vec<Range<usize>> = snippet.highlighted().to_vec();
            // Merge unigram highlights on the same fragment
            if let Some(ref ug) = unigram_gen {
                let extra = ug.snippet(&fragment);
                ranges.extend_from_slice(extra.highlighted());
            }
            // Merge diacritic highlights on the same fragment
            if let Some(ref dg) = diacritic_gen {
                let extra = dg.snippet(&fragment);
                ranges.extend_from_slice(extra.highlighted());
            }
            (fragment, ranges)
        } else {
            // Fall back to unigram
            let unigram_snippet = unigram_gen.as_ref().map(|g| g.snippet(body));
            let unigram_has_highlights = unigram_snippet
                .as_ref()
                .is_some_and(|s| !s.highlighted().is_empty());

            if unigram_has_highlights {
                let snippet = unigram_snippet.unwrap();
                let fragment = snippet.fragment().to_string();
                let mut ranges: Vec<Range<usize>> = snippet.highlighted().to_vec();
                // Merge diacritic highlights
                if let Some(ref dg) = diacritic_gen {
                    let extra = dg.snippet(&fragment);
                    ranges.extend_from_slice(extra.highlighted());
                }
                (fragment, ranges)
            } else {
                // Fall back to diacritic
                let diacritic_snippet = diacritic_gen.as_ref().map(|g| g.snippet(body));
                if let Some(snippet) = diacritic_snippet
                    && !snippet.highlighted().is_empty()
                {
                    let fragment = snippet.fragment().to_string();
                    let ranges = snippet.highlighted().to_vec();
                    (fragment, ranges)
                } else {
                    // No highlights from any field
                    (body.to_string(), vec![])
                }
            }
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
            if let Some(ref ug) = unigram_gen {
                let extra = ug.snippet(body);
                highlighted_ranges.extend_from_slice(extra.highlighted());
            }
            if let Some(ref dg) = diacritic_gen {
                let extra = dg.snippet(body);
                highlighted_ranges.extend_from_slice(extra.highlighted());
            }
            body.to_string()
        } else {
            snippet_fragment
        };

        ICUSnippet {
            fragment: snippet_fragment,
            highlights: highlighted_ranges,
        }
    }

    /// Tokenize text with the base analyzer (SemiticNorm + DiacriticFolding, no CJK filters).
    fn base_tokenize(&self, text: &str) -> Vec<Token> {
        let mut analyzer = TextAnalyzer::builder(NormalizingICUTokenizer)
            .filter(SemiticNormalizationFilter)
            .filter(DiacriticFoldingFilter)
            .build();
        let mut stream = analyzer.token_stream(text);
        let mut tokens = Vec::new();
        while stream.advance() {
            tokens.push(stream.token().clone());
        }
        tokens
    }
}
