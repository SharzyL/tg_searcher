use tantivy::query::{BooleanQuery, BoostQuery, Occur, Query, QueryParser};
use tantivy::{Index, Result};

use crate::analyzer::is_han_char;
use crate::schema::SchemaFields;
use tantivy_analyzer_icu::NormalizedText;

pub struct QueryRouter {
    bigram_parser: QueryParser,
    unigram_parser: QueryParser,
}

/// Returns true if the query, after normalization, is a single Han character.
fn is_single_han_query(query_text: &str) -> bool {
    let nt = NormalizedText::new(query_text);
    let normalized = nt.normalized();
    let non_ws: Vec<char> = normalized.chars().filter(|c| !c.is_whitespace()).collect();
    non_ws.len() == 1 && is_han_char(non_ws[0])
}

impl QueryRouter {
    pub fn new(index: &Index, fields: SchemaFields) -> Self {
        let bigram_parser = QueryParser::for_index(index, vec![fields.body_bigram]);
        let unigram_parser = QueryParser::for_index(index, vec![fields.body_unigram]);
        Self {
            bigram_parser,
            unigram_parser,
        }
    }

    pub fn route(&self, query_text: &str) -> Result<Box<dyn Query>> {
        if is_single_han_query(query_text) {
            Ok(self.unigram_parser.parse_query(query_text)?)
        } else {
            let bigram_q = self.bigram_parser.parse_query(query_text)?;
            let unigram_q = self.unigram_parser.parse_query(query_text)?;

            Ok(Box::new(BooleanQuery::new(vec![
                (Occur::Should, Box::new(BoostQuery::new(bigram_q, 2.0))),
                (Occur::Should, unigram_q),
            ])))
        }
    }

    pub fn is_single_han(query_text: &str) -> bool {
        is_single_han_query(query_text)
    }
}
