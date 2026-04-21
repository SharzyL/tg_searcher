use std::ops::Range;

use crate::search::{ICUFieldGroup, ICUSearchConfig};
use tantivy::collector::TopDocs;
use tantivy::query::Query;
use tantivy::schema::{Field, Value};
use tantivy::{Result, Searcher};

const SNIPPET_MAX_NUM_CHARS: usize = 60;

/// Schema fields used by the demo.
#[derive(Clone)]
pub struct DemoFields {
    pub id: Field,
    pub icu: ICUFieldGroup,
}

#[derive(Debug)]
pub struct SearchHit {
    pub id: String,
    pub body: String,
    pub score: f32,
    pub snippet_fragment: String,
    pub highlighted_ranges: Vec<Range<usize>>,
}

pub fn search_with_snippets(
    searcher: &Searcher,
    query: &dyn Query,
    fields: &DemoFields,
    limit: usize,
) -> Result<Vec<SearchHit>> {
    let top = searcher.search(query, &TopDocs::with_limit(limit).order_by_score())?;

    let config = ICUSearchConfig {
        max_snippet_chars: SNIPPET_MAX_NUM_CHARS,
    };

    let mut hits = Vec::new();
    for (score, doc_address) in top {
        let doc: tantivy::TantivyDocument = searcher.doc(doc_address)?;
        let id = doc
            .get_first(fields.id)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let body = doc
            .get_first(fields.icu.stored)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let snippet = config.snippet(searcher, query, &fields.icu, &body);

        hits.push(SearchHit {
            id,
            body,
            score,
            snippet_fragment: snippet.fragment,
            highlighted_ranges: snippet.highlights,
        });
    }
    Ok(hits)
}
