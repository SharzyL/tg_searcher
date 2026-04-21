use std::ops::Range;

use tantivy::collector::TopDocs;
use tantivy::query::Query;
use tantivy::schema::Value;
use tantivy::snippet::SnippetGenerator;
use tantivy::{Result, Searcher};

use crate::router::QueryRouter;
use crate::schema::SchemaFields;

#[derive(Debug)]
pub struct SearchHit {
    pub id: String,
    pub body: String,
    pub score: f32,
    pub snippet_html: String,
    pub snippet_fragment: String,
    pub highlighted_ranges: Vec<Range<usize>>,
}

pub fn search_with_snippets(
    searcher: &Searcher,
    query: &dyn Query,
    fields: &SchemaFields,
    query_text: &str,
    limit: usize,
) -> Result<Vec<SearchHit>> {
    let top = searcher.search(query, &TopDocs::with_limit(limit).order_by_score())?;

    let snippet_field = if QueryRouter::is_single_han(query_text) {
        fields.body_unigram
    } else {
        fields.body_bigram
    };

    let snippet_gen = SnippetGenerator::create(searcher, query, snippet_field)?;

    let mut hits = Vec::new();
    for (score, doc_address) in top {
        let doc: tantivy::TantivyDocument = searcher.doc(doc_address)?;
        let id = doc
            .get_first(fields.id)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let body = doc
            .get_first(fields.body)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        // Use the stored body text for snippet generation, since the indexed
        // fields (body_bigram, body_unigram) are not stored.
        let snippet = snippet_gen.snippet(&body);

        hits.push(SearchHit {
            id,
            body,
            score,
            snippet_html: snippet.to_html(),
            snippet_fragment: snippet.fragment().to_string(),
            highlighted_ranges: snippet.highlighted().to_vec(),
        });
    }
    Ok(hits)
}
