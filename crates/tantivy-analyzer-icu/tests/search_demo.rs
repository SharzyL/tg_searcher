#[cfg(feature = "demo")]
#[test]
fn search_demo_all_tests_pass() {
    let all_ok = tantivy_analyzer_icu::demo::run_all_tests().expect("demo should not error");
    assert!(all_ok, "some search demo tests failed");
}
