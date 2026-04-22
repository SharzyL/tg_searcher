use tantivy_analyzer_icu::demo;
use tantivy_analyzer_icu::demo::runner::interactive_mode;
use tantivy_analyzer_icu::demo::test_cases::QUERY_TEST_GROUPS;
use tantivy_analyzer_icu::search::ICUSearchConfig;

fn main() -> tantivy::Result<()> {
    let all_ok = demo::run_all_tests()?;

    if !all_ok {
        println!("\n!!! SOME TESTS FAILED !!!");
        std::process::exit(1);
    }

    println!("\n=== ALL TESTS PASSED ===");

    if std::env::args().any(|a| a == "--interactive") {
        let config = ICUSearchConfig::default();
        let (schema, fields) = demo::build_demo_schema(&config);
        let index = tantivy::Index::create_in_ram(schema);
        config.register_analyzers(&index);

        let mut writer = index.writer(50_000_000)?;
        for group in QUERY_TEST_GROUPS {
            demo::index_group_documents(&writer, &fields, group)?;
        }
        writer.commit()?;

        let reader = index.reader()?;
        let searcher = reader.searcher();
        interactive_mode(&searcher, &config, &fields)?;
    }

    Ok(())
}
