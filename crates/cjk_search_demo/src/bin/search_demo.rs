use tantivy::doc;
use tantivy::tokenizer::TextAnalyzer;
use tantivy::{Index, IndexWriter};

use cjk_search_demo::analyzer::{CJKBigramFilter, HanOnlyFilter};
use cjk_search_demo::router::QueryRouter;
use cjk_search_demo::runner::{
    interactive_mode, print_test_documents, run_automated_tests, run_long_doc_snippet_tests,
    run_phrase_tests, run_property_tests, run_score_tests, run_very_long_query_test,
};
use cjk_search_demo::schema::{SchemaFields, build_schema};
use cjk_search_demo::test_cases::TEST_DOCUMENTS;
use tantivy_analyzer_icu::NormalizingICUTokenizer;

fn register_analyzers(index: &Index) {
    let bigram = TextAnalyzer::builder(NormalizingICUTokenizer)
        .filter(CJKBigramFilter)
        .build();
    index.tokenizers().register("cjk_bigram", bigram);

    let unigram = TextAnalyzer::builder(NormalizingICUTokenizer)
        .filter(HanOnlyFilter)
        .build();
    index.tokenizers().register("cjk_unigram", unigram);
}

fn index_documents(writer: &IndexWriter, fields: &SchemaFields) -> tantivy::Result<()> {
    for (id, body) in TEST_DOCUMENTS {
        writer.add_document(doc!(
            fields.id => *id,
            fields.body => *body,
            fields.body_bigram => *body,
            fields.body_unigram => *body,
        ))?;
    }
    Ok(())
}

fn main() -> tantivy::Result<()> {
    let (schema, fields) = build_schema();
    let index = Index::create_in_ram(schema);
    register_analyzers(&index);

    let mut writer: IndexWriter = index.writer(50_000_000)?;
    index_documents(&writer, &fields)?;
    writer.commit()?;

    let reader = index.reader()?;
    let searcher = reader.searcher();
    let router = QueryRouter::new(&index, fields.clone());

    print_test_documents();

    let mut all_ok = true;

    all_ok &= run_automated_tests(&searcher, &router, &fields)?;
    all_ok &= run_very_long_query_test(&searcher, &router, &fields)?;
    all_ok &= run_long_doc_snippet_tests(&searcher, &router, &fields)?;
    all_ok &= run_property_tests(&searcher, &router, &fields)?;
    all_ok &= run_phrase_tests(&searcher, &fields)?;
    all_ok &= run_score_tests(&searcher, &router, &fields)?;

    if !all_ok {
        println!("\n!!! SOME TESTS FAILED !!!");
        std::process::exit(1);
    }

    println!("\n=== ALL TESTS PASSED ===");

    if std::env::args().any(|a| a == "--interactive") {
        interactive_mode(&searcher, &router, &fields)?;
    }

    Ok(())
}
