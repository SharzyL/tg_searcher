use std::collections::HashSet;
use std::fmt::Write as _;

use crate::search::ICUSearchConfig;
use tantivy::query::PhraseQuery;
use tantivy::schema::IndexRecordOption;
use tantivy::{Result, Searcher, Term};

use super::search::{DemoFields, SearchHit, search_with_snippets};
use super::test_cases::{QUERY_TEST_CASES, TEST_DOCUMENTS};

/// Format a string with non-printable and invisible Unicode characters escaped
/// as `\u{xxxx}`, so they are visible in terminal output.
fn escape_invisible(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_control() || c.is_whitespace() && c != ' ' || is_invisible_unicode(c) {
            write!(out, "\\u{{{:04X}}}", c as u32).unwrap();
        } else {
            out.push(c);
        }
    }
    out
}

fn is_invisible_unicode(c: char) -> bool {
    let cp = c as u32;
    // Variation selectors
    if matches!(cp, 0xFE00..=0xFE0F | 0xE0100..=0xE01EF) {
        return true;
    }
    // Zero-width characters and other format chars
    if matches!(
        cp,
        0x200B..=0x200F | 0x2028..=0x202F | 0x2060..=0x206F | 0xFEFF
    ) {
        return true;
    }
    // Combining marks (general ranges)
    if matches!(
        cp,
        0x0300..=0x036F
            | 0x1AB0..=0x1AFF
            | 0x1DC0..=0x1DFF
            | 0x20D0..=0x20FF
            | 0xFE20..=0xFE2F
            | 0x3099..=0x309A
    ) {
        return true;
    }
    false
}

pub fn print_test_documents() {
    println!("=== Test Documents ({}) ===", TEST_DOCUMENTS.len());
    for (id, body) in TEST_DOCUMENTS {
        println!("  [{id}] {}", escape_invisible(body));
    }
    println!();
}

const RED: &str = "\x1b[31m";
const RESET: &str = "\x1b[0m";

/// Merge overlapping/adjacent byte ranges into non-overlapping sorted ranges.
fn merge_ranges(ranges: &[std::ops::Range<usize>]) -> Vec<std::ops::Range<usize>> {
    let mut sorted: Vec<std::ops::Range<usize>> = ranges.to_vec();
    sorted.sort_by_key(|r| r.start);
    let mut merged: Vec<std::ops::Range<usize>> = Vec::new();
    for r in sorted {
        if let Some(last) = merged.last_mut()
            && r.start <= last.end
        {
            last.end = last.end.max(r.end);
            continue;
        }
        merged.push(r);
    }
    merged
}

/// Render a snippet fragment with ANSI red highlights.
fn render_snippet_ansi(fragment: &str, ranges: &[std::ops::Range<usize>]) -> String {
    if ranges.is_empty() {
        return escape_invisible(fragment);
    }
    let merged = merge_ranges(ranges);
    let mut out = String::new();
    let mut pos = 0;
    for range in &merged {
        if range.start > pos {
            out.push_str(&escape_invisible(&fragment[pos..range.start]));
        }
        out.push_str(RED);
        out.push_str(&escape_invisible(&fragment[range.start..range.end]));
        out.push_str(RESET);
        pos = range.end;
    }
    if pos < fragment.len() {
        out.push_str(&escape_invisible(&fragment[pos..]));
    }
    out
}

/// Wrap text in ANSI red.
fn red(s: &str) -> String {
    format!("{RED}{s}{RESET}")
}

fn print_hits(hits: &[SearchHit]) {
    if hits.is_empty() {
        println!("  (no hits)");
        return;
    }
    for (i, hit) in hits.iter().enumerate() {
        println!("  {}. [{}] score={:.3}", i + 1, hit.id, hit.score);
        println!("     body:    {}", escape_invisible(&hit.body));
        if !hit.highlighted_ranges.is_empty() {
            println!(
                "     snippet: {}",
                render_snippet_ansi(&hit.snippet_fragment, &hit.highlighted_ranges)
            );
        }
    }
}

pub fn run_automated_tests(
    searcher: &Searcher,
    config: &ICUSearchConfig,
    fields: &DemoFields,
) -> Result<bool> {
    let mut passed = 0;
    let mut failed = Vec::new();

    for case in QUERY_TEST_CASES {
        let query = match config.route_query(searcher, &fields.icu, case.query) {
            Ok(q) => q,
            Err(_) if case.expect_empty => {
                // Parse error on degenerate input that expects empty results — OK
                passed += 1;
                println!(
                    "PASS [{name}] query={query} (parse error → empty)",
                    name = case.name,
                    query = red(&escape_invisible(case.query)),
                );
                println!("  (no hits)\n");
                continue;
            }
            Err(e) => return Err(e),
        };
        let hits = search_with_snippets(searcher, query.as_ref(), fields, 100)?;
        let hit_ids: HashSet<String> = hits.iter().map(|h| h.id.clone()).collect();

        let mut errors = Vec::new();

        if case.expect_empty && !hits.is_empty() {
            errors.push(format!(
                "expected empty results, got {} hits: {:?}",
                hits.len(),
                hit_ids.iter().collect::<Vec<_>>()
            ));
        }

        for required in case.must_match {
            if !hit_ids.contains(*required) {
                errors.push(format!("missing expected hit: {}", required));
            }
        }

        for forbidden in case.must_not_match {
            if hit_ids.contains(*forbidden) {
                errors.push(format!("unexpected hit: {}", forbidden));
            }
        }

        // Verify scores are descending
        for w in hits.windows(2) {
            if w[0].score < w[1].score {
                errors.push(format!(
                    "scores not descending: {} ({:.3}) < {} ({:.3})",
                    w[0].id, w[0].score, w[1].id, w[1].score
                ));
                break;
            }
        }

        // Verify highlight ranges are at char boundaries of snippet fragment
        for hit in &hits {
            let frag = &hit.snippet_fragment;
            for range in &hit.highlighted_ranges {
                if range.start > frag.len() || range.end > frag.len() {
                    errors.push(format!(
                        "highlight range {:?} out of bounds for fragment len {}",
                        range,
                        frag.len()
                    ));
                } else {
                    if !frag.is_char_boundary(range.start) {
                        errors.push(format!(
                            "highlight start {} not at char boundary in {}",
                            range.start, hit.id
                        ));
                    }
                    if !frag.is_char_boundary(range.end) {
                        errors.push(format!(
                            "highlight end {} not at char boundary in {}",
                            range.end, hit.id
                        ));
                    }
                    // Verify slicing doesn't panic
                    let _ = &frag[range.start..range.end];
                }
            }

            // Verify merged ranges cover the fragment without duplication:
            // the concatenation of highlighted + non-highlighted spans must
            // equal the original fragment text.
            let merged = merge_ranges(&hit.highlighted_ranges);
            let mut reconstructed = String::new();
            let mut pos = 0;
            for range in &merged {
                if range.start > pos {
                    reconstructed.push_str(&frag[pos..range.start]);
                }
                reconstructed.push_str(&frag[range.start..range.end]);
                pos = range.end;
            }
            if pos < frag.len() {
                reconstructed.push_str(&frag[pos..]);
            }
            if reconstructed != *frag {
                errors.push(format!(
                    "merged highlight reconstruction mismatch in {}: {:?} vs {:?}",
                    hit.id, reconstructed, frag
                ));
            }
        }

        let status = if errors.is_empty() {
            passed += 1;
            "PASS"
        } else {
            "FAIL"
        };

        println!(
            "{status} [{name}] query={query}: {desc}",
            name = case.name,
            query = red(&escape_invisible(case.query)),
            desc = case.description,
        );
        print_hits(&hits);

        if !errors.is_empty() {
            for err in &errors {
                println!("  !! {err}");
            }
            failed.push((case, errors));
        }

        println!();
    }

    println!("=== Test Summary ===");
    println!("Passed: {}/{}", passed, QUERY_TEST_CASES.len());

    if !failed.is_empty() {
        println!("\n=== Failures ===");
        for (case, errors) in &failed {
            println!(
                "  FAIL [{}] query={}: {}",
                case.name,
                red(&escape_invisible(case.query)),
                case.description
            );
            for err in errors {
                println!("    - {err}");
            }
        }
    }

    Ok(failed.is_empty())
}

/// Run the very_long_query stress test.
pub fn run_very_long_query_test(
    searcher: &Searcher,
    config: &ICUSearchConfig,
    fields: &DemoFields,
) -> Result<bool> {
    println!("=== Very Long Query Test ===");
    let query_text = super::test_cases::very_long_query();
    let query = config.route_query(searcher, &fields.icu, &query_text)?;
    let hits = search_with_snippets(searcher, query.as_ref(), fields, 100)?;
    // Should not panic. Returns 0 hits because adjacent "北北北..." produces
    // bigram terms that don't exist in any document (no unigram fallback since
    // all chars are offset-adjacent).
    println!(
        "PASS [very_long_query] 1000x'北': {} hits returned",
        hits.len()
    );
    println!();
    Ok(true)
}

/// Run long document snippet tests.
pub fn run_long_doc_snippet_tests(
    searcher: &Searcher,
    config: &ICUSearchConfig,
    fields: &DemoFields,
) -> Result<bool> {
    println!("=== Long Document Snippet Tests ===");
    let mut ok = true;

    let query = config.route_query(searcher, &fields.icu, "北京")?;
    let hits = search_with_snippets(searcher, query.as_ref(), fields, 100)?;

    let long_hit = hits.iter().find(|h| h.id == "long-1");
    if let Some(hit) = long_hit {
        let frag = &hit.snippet_fragment;
        // Snippet should be shorter than original
        if frag.len() < hit.body.len() {
            println!(
                "PASS [long_doc_snippet_window] snippet ({} bytes) < body ({} bytes)",
                frag.len(),
                hit.body.len()
            );
        } else {
            println!(
                "FAIL [long_doc_snippet_window] snippet ({} bytes) >= body ({} bytes)",
                frag.len(),
                hit.body.len()
            );
            ok = false;
        }
        // Snippet should have reasonable upper bound
        if frag.len() < 1000 {
            println!(
                "PASS [long_doc_snippet_bounded] snippet {} bytes < 1000",
                frag.len()
            );
        } else {
            println!(
                "FAIL [long_doc_snippet_bounded] snippet {} bytes >= 1000",
                frag.len()
            );
            ok = false;
        }
        // Snippet should contain 北京
        if frag.contains("北京") {
            println!("PASS [long_doc_snippet_contains] snippet contains 北京");
        } else {
            println!("FAIL [long_doc_snippet_contains] snippet missing 北京");
            ok = false;
        }
    } else {
        println!("FAIL [long_doc_hit] long-1 not found in results");
        ok = false;
    }

    println!();
    Ok(ok)
}

/// Run property-based tests using a fixed set of adversarial inputs.
pub fn run_property_tests(
    searcher: &Searcher,
    config: &ICUSearchConfig,
    fields: &DemoFields,
) -> Result<bool> {
    println!("=== Property Tests ===");
    let mut ok = true;

    let adversarial_inputs: &[&str] = &[
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
        &"あ".repeat(200),
        &"𠮷".repeat(50),
        &"Hello 你好 ".repeat(20),
        "👨\u{200D}👩\u{200D}👧\u{200D}👦 family",
        "🏳️\u{200D}🌈",
        "\0\u{0001}\u{007F}", // control chars
        "a b c d e f g h i j k l m n o p q r s t u v w x y z",
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
    ];

    for (i, input) in adversarial_inputs.iter().enumerate() {
        // Test 1: route() should not panic
        let query = match config.route_query(searcher, &fields.icu, input) {
            Ok(q) => q,
            Err(e) => {
                // Parse errors on degenerate input are OK, not failures
                println!(
                    "  [prop-{i}] parse error on {:?}: {} (OK)",
                    escape_invisible(input),
                    e
                );
                continue;
            }
        };

        // Test 2: search should not panic
        let hits = search_with_snippets(searcher, query.as_ref(), fields, 10)?;

        // Test 3: all highlight ranges at char boundaries and sliceable
        for hit in &hits {
            let frag = &hit.snippet_fragment;
            for range in &hit.highlighted_ranges {
                if range.start > frag.len() || range.end > frag.len() {
                    println!(
                        "FAIL [prop-{i}] range {:?} out of bounds for fragment len {}",
                        range,
                        frag.len()
                    );
                    ok = false;
                    continue;
                }
                if !frag.is_char_boundary(range.start) || !frag.is_char_boundary(range.end) {
                    println!(
                        "FAIL [prop-{i}] range {:?} not at char boundary in {:?}",
                        range, hit.id
                    );
                    ok = false;
                    continue;
                }
                let _ = &frag[range.start..range.end];
            }
        }

        // Test 4: determinism — search again, same results
        let hits2 = search_with_snippets(searcher, query.as_ref(), fields, 10)?;
        if hits.len() != hits2.len() {
            println!(
                "FAIL [prop-{i}] non-deterministic: {} vs {} hits",
                hits.len(),
                hits2.len()
            );
            ok = false;
        } else {
            for (a, b) in hits.iter().zip(hits2.iter()) {
                if a.id != b.id || (a.score - b.score).abs() > f32::EPSILON {
                    println!("FAIL [prop-{i}] non-deterministic results");
                    ok = false;
                    break;
                }
            }
        }
    }

    if ok {
        println!(
            "PASS [property_tests] all {} inputs OK",
            adversarial_inputs.len()
        );
    }

    println!();
    Ok(ok)
}

/// Run PhraseQuery tests on the bigram field.
pub fn run_phrase_tests(searcher: &Searcher, fields: &DemoFields) -> Result<bool> {
    println!("=== PhraseQuery Tests ===");
    let mut ok = true;

    // Check that body_bigram field has positions
    let schema = searcher.schema();
    let bigram_entry = schema.get_field_entry(fields.icu.folded_bigram);
    let has_positions = bigram_entry
        .field_type()
        .get_index_record_option()
        .is_some_and(|opt| matches!(opt, IndexRecordOption::WithFreqsAndPositions));
    if !has_positions {
        println!("SKIP [phrase_tests] body_bigram field does not have positions");
        println!();
        return Ok(true);
    }

    struct PhraseTest {
        name: &'static str,
        terms: &'static [&'static str],
        must_match: &'static [&'static str],
        must_not_match: &'static [&'static str],
        description: &'static str,
    }

    let phrase_tests = &[
        // 北京天安 → bigrams 北京, 京天, 天安 at consecutive positions
        PhraseTest {
            name: "phrase_beijing_tianan",
            terms: &["北京", "京天", "天安"],
            must_match: &["zh-1"],
            must_not_match: &["zh-2", "zh-3"],
            description: "Consecutive bigrams in zh-1 我爱北京天安门",
        },
        // 令和 and ξ are from different token types (CJK bigram vs non-CJK passthrough).
        // No bigram 和ξ should exist, so phrase [令和, 和ξ] should not match.
        PhraseTest {
            name: "phrase_reiwa_meiji",
            terms: &["令和", "和ξ"],
            must_match: &[],
            must_not_match: &["demo-1"],
            description: "No cross CJK/non-CJK bigram boundary",
        },
    ];

    for test in phrase_tests {
        let terms: Vec<Term> = test
            .terms
            .iter()
            .map(|t| Term::from_field_text(fields.icu.folded_bigram, t))
            .collect();
        let phrase_query = PhraseQuery::new(terms);

        let hits = search_with_snippets(searcher, &phrase_query, fields, 100)?;
        let hit_ids: HashSet<String> = hits.iter().map(|h| h.id.clone()).collect();

        let mut errors = Vec::new();
        for required in test.must_match {
            if !hit_ids.contains(*required) {
                errors.push(format!("missing: {required}"));
            }
        }
        for forbidden in test.must_not_match {
            if hit_ids.contains(*forbidden) {
                errors.push(format!("unexpected: {forbidden}"));
            }
        }

        if errors.is_empty() {
            println!(
                "PASS [{}] phrase={:?} hits={:?}",
                test.name,
                test.terms,
                hit_ids.iter().collect::<Vec<_>>()
            );
        } else {
            println!("FAIL [{}] phrase={:?}", test.name, test.terms);
            println!("  description: {}", test.description);
            println!("  actual hits: {:?}", hit_ids.iter().collect::<Vec<_>>());
            for err in &errors {
                println!("  !! {err}");
            }
            ok = false;
        }
    }

    println!();
    Ok(ok)
}

/// Run score-specific tests.
pub fn run_score_tests(
    searcher: &Searcher,
    config: &ICUSearchConfig,
    fields: &DemoFields,
) -> Result<bool> {
    println!("=== Score Tests ===");
    let mut ok = true;

    // exact_doc_ranks_high: "我" query, zh-6 (just "我") should be in top 3
    {
        let query = config.route_query(searcher, &fields.icu, "我")?;
        let hits = search_with_snippets(searcher, query.as_ref(), fields, 10)?;
        let top3: Vec<&str> = hits.iter().take(3).map(|h| h.id.as_str()).collect();
        if top3.contains(&"zh-6") {
            println!("PASS [exact_doc_ranks_high] zh-6 in top 3: {:?}", top3);
        } else {
            println!("FAIL [exact_doc_ranks_high] zh-6 not in top 3: {:?}", top3);
            ok = false;
        }
    }

    println!();
    Ok(ok)
}

pub fn interactive_mode(
    searcher: &Searcher,
    config: &ICUSearchConfig,
    fields: &DemoFields,
) -> Result<()> {
    use std::io::{self, BufRead, Write};
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    println!("Interactive search mode. Type query or 'exit' to quit.");
    loop {
        print!("> ");
        stdout.flush().unwrap();

        let mut line = String::new();
        if stdin.lock().read_line(&mut line)? == 0 {
            break;
        }
        let query_text = line.trim();
        if query_text.is_empty() {
            continue;
        }
        if query_text == "exit" {
            break;
        }

        let query = config.route_query(searcher, &fields.icu, query_text)?;
        let hits = search_with_snippets(searcher, query.as_ref(), fields, 10)?;

        println!("Found {} hits:", hits.len());
        print_hits(&hits);
    }
    Ok(())
}
