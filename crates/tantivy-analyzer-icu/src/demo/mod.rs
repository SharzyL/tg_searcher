//! Demo and test harness for the ICU search pipeline.
//!
//! Contains test documents, query test cases, and a runner that validates
//! the full index → query → snippet pipeline. Used by both the `search_demo`
//! example and the integration test.

pub mod runner;
pub mod search;
pub mod test_cases;

use tantivy::doc;
use tantivy::schema::{STORED, STRING, Schema};
use tantivy::{Index, IndexWriter};

use crate::search::ICUSearchConfig;
use search::DemoFields;
use test_cases::TEST_DOCUMENTS;

/// Build the demo schema and fields.
pub fn build_demo_schema(config: &ICUSearchConfig) -> (Schema, DemoFields) {
    let mut builder = Schema::builder();
    let id = builder.add_text_field("id", STRING | STORED);
    let icu = config.add_field_group(&mut builder, "body");
    (builder.build(), DemoFields { id, icu })
}

/// Index all test documents.
pub fn index_documents(writer: &IndexWriter, fields: &DemoFields) -> tantivy::Result<()> {
    for (id, body) in TEST_DOCUMENTS {
        writer.add_document(doc!(
            fields.id => *id,
            fields.icu.stored => *body,
            fields.icu.bigram => *body,
            fields.icu.unigram => *body,
        ))?;
    }
    Ok(())
}

/// Run all automated tests. Returns `true` if all pass.
pub fn run_all_tests() -> tantivy::Result<bool> {
    use runner::*;

    let config = ICUSearchConfig::default();
    let (schema, fields) = build_demo_schema(&config);
    let index = Index::create_in_ram(schema);
    config.register_analyzers(&index);

    let mut writer: IndexWriter = index.writer(50_000_000)?;
    index_documents(&writer, &fields)?;
    writer.commit()?;

    let reader = index.reader()?;
    let searcher = reader.searcher();

    print_test_documents();

    let mut all_ok = true;

    all_ok &= run_automated_tests(&searcher, &config, &index, &fields)?;
    all_ok &= run_very_long_query_test(&searcher, &config, &index, &fields)?;
    all_ok &= run_long_doc_snippet_tests(&searcher, &config, &index, &fields)?;
    all_ok &= run_property_tests(&searcher, &config, &index, &fields)?;
    all_ok &= run_phrase_tests(&searcher, &fields)?;
    all_ok &= run_score_tests(&searcher, &config, &index, &fields)?;

    Ok(all_ok)
}
