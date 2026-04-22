//! Tests for the search pipeline: query routing, snippets, phrase matching, scoring.
//!
//! These tests exercise the full index → query → snippet pipeline using
//! documents from the demo test groups.

#![cfg(feature = "demo")]

use std::collections::HashSet;

use tantivy::Term;
use tantivy::query::PhraseQuery;
use tantivy::schema::IndexRecordOption;

use tantivy_analyzer_icu::demo::runner::create_group_index;
use tantivy_analyzer_icu::demo::search::search_with_snippets;
use tantivy_analyzer_icu::demo::test_cases::{QUERY_TEST_GROUPS, very_long_query};
use tantivy_analyzer_icu::search::ICUSearchConfig;

/// Find the first group whose doc set contains the given doc ID.
fn group_with_doc(id: &str) -> &'static tantivy_analyzer_icu::demo::test_cases::QueryTestGroup {
    QUERY_TEST_GROUPS
        .iter()
        .find(|g| g.docs.iter().any(|(did, _)| *did == id))
        .unwrap_or_else(|| panic!("No group contains doc '{id}'"))
}

// ---------------------------------------------------------------------------
// Very long query stress test
// ---------------------------------------------------------------------------

#[test]
fn very_long_query_does_not_crash() {
    let config = ICUSearchConfig::default();
    let group = &QUERY_TEST_GROUPS[0];
    let (index, fields) = create_group_index(&config, group).unwrap();
    let reader = index.reader().unwrap();
    let searcher = reader.searcher();

    let query_text = very_long_query();
    let query = config
        .route_query(&searcher, &fields.icu, &query_text)
        .unwrap();
    // Just verify it doesn't panic or error; hit count is irrelevant.
    let _hits = search_with_snippets(&searcher, query.as_ref(), &fields, 100).unwrap();
}

// ---------------------------------------------------------------------------
// Long document snippet tests
// ---------------------------------------------------------------------------

#[test]
fn long_doc_snippet_is_windowed() {
    let config = ICUSearchConfig::default();
    let group = group_with_doc("long-1");
    let (index, fields) = create_group_index(&config, group).unwrap();
    let reader = index.reader().unwrap();
    let searcher = reader.searcher();

    let query = config.route_query(&searcher, &fields.icu, "北京").unwrap();
    let hits = search_with_snippets(&searcher, query.as_ref(), &fields, 100).unwrap();
    let hit = hits
        .iter()
        .find(|h| h.id == "long-1")
        .expect("long-1 should be in results");

    assert!(
        hit.snippet_fragment.len() < hit.body.len(),
        "snippet ({} bytes) should be shorter than body ({} bytes)",
        hit.snippet_fragment.len(),
        hit.body.len(),
    );
    assert!(
        hit.snippet_fragment.len() < 1000,
        "snippet ({} bytes) should be bounded",
        hit.snippet_fragment.len(),
    );
    assert!(
        hit.snippet_fragment.contains("北京"),
        "snippet should contain the query term"
    );
}

// ---------------------------------------------------------------------------
// Adversarial input property tests
// ---------------------------------------------------------------------------

#[test]
fn adversarial_inputs_do_not_crash() {
    let config = ICUSearchConfig::default();
    let group = QUERY_TEST_GROUPS
        .iter()
        .find(|g| !g.docs.is_empty())
        .expect("No non-empty group");
    let (index, fields) = create_group_index(&config, group).unwrap();
    let reader = index.reader().unwrap();
    let searcher = reader.searcher();

    let inputs: &[&str] = &[
        "",
        " ",
        "   \t\n  ",
        "a",
        "我",
        "🎉",
        "hello world",
        "㋿Ξ㍾㍿",
        "𠮷野家",
        "\u{200B}\u{FEFF}\u{200D}",
        "\u{E0100}\u{E0101}",
        "!!!???。。。",
        "a\u{0301}\u{0320}",
        "か\u{3099}",
        "\u{110B}\u{1161}\u{11AB}",
        "Hello 你好 World 🎉 안녕 ありがとう",
        "葛\u{E0100}飾",
        "café résumé naïve",
        "Straße İstanbul Ξένος",
        "㐀㐁㐂",
        "𠀀𠁀𠂀",
        "A我B你C",
        "い안あ한",
        "北京 在 东京",
        "北京，东京。大阪！",
        "👨\u{200D}👩\u{200D}👧\u{200D}👦 family",
        "🏳️\u{200D}🌈",
        "\0\u{0001}\u{007F}",
        "a b c d e f g h i j k l m n o p q r s t u v w x y z",
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
    ];

    for input in inputs {
        let query = match config.route_query(&searcher, &fields.icu, input) {
            Ok(q) => q,
            Err(_) => continue, // parse errors are acceptable
        };

        let hits = search_with_snippets(&searcher, query.as_ref(), &fields, 10).unwrap();

        // Verify highlight ranges are valid
        for hit in &hits {
            let frag = &hit.snippet_fragment;
            for range in &hit.highlighted_ranges {
                assert!(
                    range.start <= frag.len() && range.end <= frag.len(),
                    "range {range:?} out of bounds for fragment len {} (input: {input:?})",
                    frag.len(),
                );
                assert!(
                    frag.is_char_boundary(range.start) && frag.is_char_boundary(range.end),
                    "range {range:?} not at char boundary in {} (input: {input:?})",
                    hit.id,
                );
            }
        }

        // Verify determinism
        let hits2 = search_with_snippets(&searcher, query.as_ref(), &fields, 10).unwrap();
        assert_eq!(
            hits.len(),
            hits2.len(),
            "non-deterministic hit count for input: {input:?}"
        );
        for (a, b) in hits.iter().zip(hits2.iter()) {
            assert_eq!(
                a.id, b.id,
                "non-deterministic ordering for input: {input:?}"
            );
            assert!(
                (a.score - b.score).abs() <= f32::EPSILON,
                "non-deterministic scores for input: {input:?}"
            );
        }
    }
}

// Also test the long repeat inputs separately (can't be &'static str)
#[test]
fn adversarial_long_repeat_inputs() {
    let config = ICUSearchConfig::default();
    let group = QUERY_TEST_GROUPS
        .iter()
        .find(|g| !g.docs.is_empty())
        .expect("No non-empty group");
    let (index, fields) = create_group_index(&config, group).unwrap();
    let reader = index.reader().unwrap();
    let searcher = reader.searcher();

    let long_inputs = ["あ".repeat(200), "𠮷".repeat(50), "Hello 你好 ".repeat(20)];

    for input in &long_inputs {
        let query = match config.route_query(&searcher, &fields.icu, input) {
            Ok(q) => q,
            Err(_) => continue,
        };
        let _hits = search_with_snippets(&searcher, query.as_ref(), &fields, 10).unwrap();
    }
}

// ---------------------------------------------------------------------------
// PhraseQuery tests on bigram field
// ---------------------------------------------------------------------------

#[test]
fn phrase_query_beijing_tiananmen() {
    let config = ICUSearchConfig::default();
    let group = group_with_doc("zh-1");
    let (index, fields) = create_group_index(&config, group).unwrap();
    let reader = index.reader().unwrap();
    let searcher = reader.searcher();

    // Verify positions are indexed
    let schema = searcher.schema();
    let entry = schema.get_field_entry(fields.icu.folded_bigram);
    assert!(
        entry
            .field_type()
            .get_index_record_option()
            .is_some_and(|opt| matches!(opt, IndexRecordOption::WithFreqsAndPositions)),
        "folded_bigram field must have positions"
    );

    // Consecutive bigrams: 北京 京天 天安 — should match zh-1 "我爱北京天安门"
    let terms: Vec<Term> = ["北京", "京天", "天安"]
        .iter()
        .map(|t| Term::from_field_text(fields.icu.folded_bigram, t))
        .collect();
    let hits = search_with_snippets(&searcher, &PhraseQuery::new(terms), &fields, 100).unwrap();
    let ids: HashSet<&str> = hits.iter().map(|h| h.id.as_str()).collect();

    assert!(ids.contains("zh-1"), "zh-1 should match");
    assert!(!ids.contains("zh-2"), "zh-2 should not match");
    assert!(!ids.contains("zh-3"), "zh-3 should not match");
}

#[test]
fn phrase_query_no_cross_script_boundary() {
    let config = ICUSearchConfig::default();
    let group = group_with_doc("zh-1"); // demo-1 is in same group
    let (index, fields) = create_group_index(&config, group).unwrap();
    let reader = index.reader().unwrap();
    let searcher = reader.searcher();

    // 令和 和ξ — should NOT produce a cross-CJK/non-CJK match on demo-1
    let terms: Vec<Term> = ["令和", "和ξ"]
        .iter()
        .map(|t| Term::from_field_text(fields.icu.folded_bigram, t))
        .collect();
    let hits = search_with_snippets(&searcher, &PhraseQuery::new(terms), &fields, 100).unwrap();
    let ids: HashSet<&str> = hits.iter().map(|h| h.id.as_str()).collect();

    assert!(
        !ids.contains("demo-1"),
        "demo-1 should not match cross-script phrase"
    );
}

// ---------------------------------------------------------------------------
// Score ranking test
// ---------------------------------------------------------------------------

#[test]
fn exact_single_char_doc_ranks_high() {
    let config = ICUSearchConfig::default();
    let group = group_with_doc("zh-6"); // zh-6 is just "我"
    let (index, fields) = create_group_index(&config, group).unwrap();
    let reader = index.reader().unwrap();
    let searcher = reader.searcher();

    let query = config.route_query(&searcher, &fields.icu, "我").unwrap();
    let hits = search_with_snippets(&searcher, query.as_ref(), &fields, 10).unwrap();
    let top3: Vec<&str> = hits.iter().take(3).map(|h| h.id.as_str()).collect();

    assert!(
        top3.contains(&"zh-6"),
        "zh-6 (exact '我') should be in top 3, got: {top3:?}"
    );
}
