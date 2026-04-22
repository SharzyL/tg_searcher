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
//! let query = icu.route_query(&searcher, &content, "café 北京 我")?;
//!
//! // Snippet generation with three-way highlight merging
//! let snippet = icu.snippet(&searcher, &query, &content, &body);
//! ```

use std::ops::Range;

use tantivy::query::{BooleanQuery, ConstScoreQuery, Occur, PhraseQuery, Query, TermQuery};
use tantivy::schema::{
    Field, IndexRecordOption, STORED, SchemaBuilder, TextFieldIndexing, TextOptions,
};
use tantivy::snippet::SnippetGenerator;
use tantivy::tokenizer::TextAnalyzer;
use tantivy::{Index, Score, Searcher, Term};
use tantivy_tokenizer_api::{Token, TokenStream};

use crate::filter::{
    ScriptGroup, fold_diacritics, is_foldable_diacritic, is_han_char, token_script_group,
};
use crate::{
    CJKBigramFilter, DiacriticFoldingFilter, DiacriticOnlyFilter, HanOnlyFilter,
    NormalizingICUTokenizer, SemiticNormalizationFilter,
};

const DEFAULT_MAX_SNIPPET_CHARS: usize = 150;

/// Boost factor for diacritic field clauses relative to IDF.
const DIACRITIC_BOOST: Score = 1.5;

/// Compute IDF using the same formula as tantivy's BM25:
/// `ln(1 + (N - n + 0.5) / (n + 0.5))`
fn compute_idf(doc_freq: u64, total_num_docs: u64) -> Score {
    let x = ((total_num_docs - doc_freq) as Score + 0.5) / (doc_freq as Score + 0.5);
    (1.0 + x).ln()
}

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
    /// Tokenizes the query text once, then builds term queries for all three
    /// fields without going through `QueryParser`:
    /// - Adjacent CJK characters → folded_bigram field
    /// - Isolated Han characters → unigram field TermQueries
    /// - Non-CJK text → folded_bigram field passthrough
    /// - If the query contains foldable diacritics → also diacritic field (boosted 1.5x)
    ///
    /// All clauses use `ConstScoreQuery` with manual IDF instead of BM25,
    /// eliminating document length normalization which is harmful for short
    /// messages (long docs matching more terms would otherwise score lower
    /// than short docs matching fewer terms).
    pub fn route_query(
        &self,
        searcher: &Searcher,
        fields: &ICUFieldGroup,
        query_text: &str,
    ) -> tantivy::Result<Box<dyn Query>> {
        // Single tokenization pass: NormalizingICUTokenizer → SemiticNorm
        let semitic_tokens = self.semitic_tokenize(query_text);

        // Derive all three term sets from the semitic-normalized tokens:
        let QueryTerms {
            folded_bigram_groups,
            unigram_terms,
            diacritic_terms,
        } = Self::derive_query_terms(&semitic_tokens);

        let total_num_docs = searcher.num_docs();
        let mut clauses: Vec<(Occur, Box<dyn Query>)> = Vec::new();

        // Build folded_bigram clauses with IDF scoring.
        // Each group is either a single term or a phrase (from a CJK bigram run).
        for group in &folded_bigram_groups {
            if group.len() == 1 {
                let term = Term::from_field_text(fields.folded_bigram, &group[0]);
                let doc_freq = searcher.doc_freq(&term)?;
                let idf = compute_idf(doc_freq, total_num_docs);
                let q = TermQuery::new(term, IndexRecordOption::WithFreqsAndPositions);
                clauses.push((
                    Occur::Should,
                    Box::new(ConstScoreQuery::new(Box::new(q), idf)),
                ));
            } else {
                // Multi-bigram run → PhraseQuery to match the exact sequence.
                // Use the max IDF among the phrase's terms as the score.
                let terms: Vec<Term> = group
                    .iter()
                    .map(|t| Term::from_field_text(fields.folded_bigram, t))
                    .collect();
                let max_idf = terms
                    .iter()
                    .map(|t| {
                        searcher
                            .doc_freq(t)
                            .map(|df| compute_idf(df, total_num_docs))
                    })
                    .collect::<tantivy::Result<Vec<Score>>>()?
                    .into_iter()
                    .fold(0.0f32, f32::max);
                let q = PhraseQuery::new(terms);
                clauses.push((
                    Occur::Should,
                    Box::new(ConstScoreQuery::new(Box::new(q), max_idf)),
                ));
            };
        }

        // Build unigram clauses for isolated Han characters
        for term_text in &unigram_terms {
            let term = Term::from_field_text(fields.unigram, term_text);
            let doc_freq = searcher.doc_freq(&term)?;
            let idf = compute_idf(doc_freq, total_num_docs);
            clauses.push((
                Occur::Should,
                Box::new(ConstScoreQuery::new(
                    Box::new(TermQuery::new(term, IndexRecordOption::WithFreqs)),
                    idf,
                )),
            ));
        }

        // Build diacritic clauses (boosted 1.5x relative to IDF)
        for term_text in &diacritic_terms {
            let term = Term::from_field_text(fields.diacritic, term_text);
            let doc_freq = searcher.doc_freq(&term)?;
            let idf = compute_idf(doc_freq, total_num_docs);
            clauses.push((
                Occur::Should,
                Box::new(ConstScoreQuery::new(
                    Box::new(TermQuery::new(term, IndexRecordOption::WithFreqs)),
                    idf * DIACRITIC_BOOST,
                )),
            ));
        }

        if clauses.is_empty() {
            Ok(Box::new(BooleanQuery::new(vec![])))
        } else if clauses.len() == 1 {
            let (_, query) = clauses.pop().unwrap();
            Ok(query)
        } else {
            Ok(Box::new(BooleanQuery::new(clauses)))
        }
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

    /// Tokenize text with NormalizingICUTokenizer + SemiticNorm only (no diacritic folding).
    ///
    /// This is the single tokenization pass used by `route_query`. The returned
    /// tokens are at the "semitic-normalized" stage — DiacriticFolding and
    /// CJK bigram/unigram/diacritic-only filters have NOT been applied yet.
    fn semitic_tokenize(&self, text: &str) -> Vec<Token> {
        let mut analyzer = TextAnalyzer::builder(NormalizingICUTokenizer)
            .filter(SemiticNormalizationFilter)
            .build();
        let mut stream = analyzer.token_stream(text);
        let mut tokens = Vec::new();
        while stream.advance() {
            tokens.push(stream.token().clone());
        }
        tokens
    }

    /// Derive all three sets of query terms from semitic-normalized tokens.
    ///
    /// This replicates the logic of the three analyzer pipelines
    /// (folded_bigram, unigram, diacritic) without re-tokenizing:
    ///
    /// - **folded_bigram**: fold diacritics → CJK bigram (non-CJK pass through,
    ///   isolated Han dropped). Each CJK bigram run produces a phrase group.
    /// - **unigram**: fold diacritics → isolated Han characters only
    /// - **diacritic**: pre-fold tokens that contain foldable diacritics
    fn derive_query_terms(semitic_tokens: &[Token]) -> QueryTerms {
        // Step 1: fold diacritics on each token
        let folded: Vec<String> = semitic_tokens
            .iter()
            .map(|t| fold_diacritics(&t.text))
            .collect();

        // Step 2: CJK bigram logic on folded tokens (replicates CJKBigramFilter).
        // Each entry in folded_bigram_groups is either:
        //   - a single-element vec (non-CJK term or isolated kana/hangul)
        //   - a multi-element vec (bigrams from a contiguous CJK run → PhraseQuery)
        let mut folded_bigram_groups: Vec<Vec<String>> = Vec::new();
        let mut unigram_terms = Vec::new();
        let mut i = 0;
        let len = folded.len();

        while i < len {
            let script = token_script_group(&folded[i]);

            if script == ScriptGroup::NonCjk {
                folded_bigram_groups.push(vec![folded[i].clone()]);
                i += 1;
                continue;
            }

            // Collect a run of same-group, offset-adjacent CJK tokens
            let run_start = i;
            i += 1;
            while i < len
                && token_script_group(&folded[i]) == script
                && semitic_tokens[i].offset_from <= semitic_tokens[i - 1].offset_to
            {
                i += 1;
            }
            let run_len = i - run_start;

            if run_len == 1 {
                let c = folded[run_start].chars().next();
                if c.is_some_and(is_han_char) {
                    unigram_terms.push(folded[run_start].clone());
                } else {
                    folded_bigram_groups.push(vec![folded[run_start].clone()]);
                }
                continue;
            }

            // Multi-token CJK run → overlapping bigrams as a phrase group
            let bigrams: Vec<String> = (run_start..run_start + run_len - 1)
                .map(|j| format!("{}{}", folded[j], folded[j + 1]))
                .collect();
            folded_bigram_groups.push(bigrams);
        }

        // Step 3: diacritic terms (pre-fold tokens with foldable diacritics)
        let diacritic_terms: Vec<String> = semitic_tokens
            .iter()
            .filter(|t| {
                use unicode_normalization::UnicodeNormalization;
                t.text.nfd().any(is_foldable_diacritic)
            })
            .map(|t| t.text.clone())
            .collect();

        QueryTerms {
            folded_bigram_groups,
            unigram_terms,
            diacritic_terms,
        }
    }
}

/// The three sets of query terms derived from a single tokenization pass.
struct QueryTerms {
    /// Term groups for the folded_bigram field. Each group is either a single term
    /// (non-CJK or isolated kana/hangul) or a phrase of bigrams (contiguous CJK run).
    folded_bigram_groups: Vec<Vec<String>>,
    /// Terms for the unigram field (isolated Han characters).
    unigram_terms: Vec<String>,
    /// Terms for the diacritic field (pre-fold tokens with foldable diacritics).
    diacritic_terms: Vec<String>,
}
