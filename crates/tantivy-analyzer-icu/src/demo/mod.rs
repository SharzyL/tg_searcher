//! Demo and test harness for the ICU search pipeline.
//!
//! Contains test documents, query test cases, and a runner that validates
//! the full index → query → snippet pipeline. Used by both the `search_demo`
//! example and the integration test.

pub mod runner;
pub mod search;
pub mod test_cases;

use tantivy::IndexWriter;
use tantivy::doc;
use tantivy::schema::{STORED, STRING, Schema};

use crate::search::ICUSearchConfig;
use search::DemoFields;
use test_cases::QueryTestGroup;

/// Build the demo schema and fields.
pub fn build_demo_schema(config: &ICUSearchConfig) -> (Schema, DemoFields) {
    let mut builder = Schema::builder();
    let id = builder.add_text_field("id", STRING | STORED);
    let icu = config.add_field_group(&mut builder, "body");
    (builder.build(), DemoFields { id, icu })
}

/// Index a group's documents into the given writer.
pub fn index_group_documents(
    writer: &IndexWriter,
    fields: &DemoFields,
    group: &QueryTestGroup,
) -> tantivy::Result<()> {
    for (id, body) in group.docs {
        writer.add_document(doc!(
            fields.id => *id,
            fields.icu.stored => *body,
            fields.icu.folded_bigram => *body,
            fields.icu.unigram => *body,
            fields.icu.diacritic => *body,
        ))?;
    }
    Ok(())
}

/// Run all automated tests. Returns `true` if all pass.
pub fn run_all_tests() -> tantivy::Result<bool> {
    use runner::*;

    let config = ICUSearchConfig::default();
    let mut all_ok = true;

    all_ok &= run_group_tests(&config)?;
    all_ok &= run_very_long_query_test(&config)?;
    all_ok &= run_long_doc_snippet_tests(&config)?;
    all_ok &= run_property_tests(&config)?;
    all_ok &= run_phrase_tests(&config)?;
    all_ok &= run_score_tests(&config)?;

    Ok(all_ok)
}
