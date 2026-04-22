use std::collections::HashSet;
use std::fmt::Write as _;

use crate::search::ICUSearchConfig;
use tantivy::{Index, IndexWriter, Result, Searcher};

use super::search::{DemoFields, SearchHit, search_with_snippets};
use super::test_cases::{QUERY_TEST_GROUPS, QueryTestGroup};
use super::{build_demo_schema, index_group_documents};

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
    // Hangul Jamo (conjoining): terminals auto-compose these into syllable blocks
    if matches!(cp, 0x1100..=0x11FF | 0xA960..=0xA97F | 0xD7B0..=0xD7FF) {
        return true;
    }
    false
}

const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const BLUE: &str = "\x1b[34m";
const MAGENTA: &str = "\x1b[35m";
const GRAY: &str = "\x1b[90m";
const BOLD: &str = "\x1b[1m";
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

/// Render a snippet fragment with ANSI red highlights on matched ranges, gray for the rest.
fn render_snippet_ansi(fragment: &str, ranges: &[std::ops::Range<usize>]) -> String {
    if ranges.is_empty() {
        return format!("{GRAY}{}{RESET}", escape_invisible(fragment));
    }
    let merged = merge_ranges(ranges);
    let mut out = String::new();
    let mut pos = 0;
    for range in &merged {
        if range.start > pos {
            out.push_str(GRAY);
            out.push_str(&escape_invisible(&fragment[pos..range.start]));
            out.push_str(RESET);
        }
        out.push_str(RED);
        out.push_str(&escape_invisible(&fragment[range.start..range.end]));
        out.push_str(RESET);
        pos = range.end;
    }
    if pos < fragment.len() {
        out.push_str(GRAY);
        out.push_str(&escape_invisible(&fragment[pos..]));
        out.push_str(RESET);
    }
    out
}

fn print_hits(hits: &[SearchHit]) {
    if hits.is_empty() {
        println!("  (no hits)");
        return;
    }
    for (i, hit) in hits.iter().enumerate() {
        println!("  {}. [{}] score={:.3}", i + 1, hit.id, hit.score);
        println!("     body:    {GRAY}{}{RESET}", escape_invisible(&hit.body));
        if !hit.highlighted_ranges.is_empty() {
            println!(
                "     snippet: {}",
                render_snippet_ansi(&hit.snippet_fragment, &hit.highlighted_ranges)
            );
        }
    }
}

fn print_group_header(title: &str) {
    let bar = "─".repeat(60);
    println!("{MAGENTA}{BOLD}┌{bar}┐{RESET}");
    println!("{MAGENTA}{BOLD}│ {title:^58} │{RESET}");
    println!("{MAGENTA}{BOLD}└{bar}┘{RESET}");
}

fn print_group_docs(group: &QueryTestGroup) {
    println!("  Documents ({}):", group.docs.len());
    for (id, body) in group.docs {
        println!("    [{id}] {GRAY}{}{RESET}", escape_invisible(body));
    }
    println!();
}

/// Create an in-RAM index with the given group's documents.
pub fn create_group_index(
    config: &ICUSearchConfig,
    group: &QueryTestGroup,
) -> Result<(Index, DemoFields)> {
    let (schema, fields) = build_demo_schema(config);
    let index = Index::create_in_ram(schema);
    config.register_analyzers(&index);
    let mut writer: IndexWriter = index.writer(50_000_000)?;
    index_group_documents(&writer, &fields, group)?;
    writer.commit()?;
    Ok((index, fields))
}

pub fn run_group_tests(config: &ICUSearchConfig) -> Result<bool> {
    let mut total_passed = 0;
    let mut total_cases = 0;
    let mut all_failed = Vec::new();

    for group in QUERY_TEST_GROUPS {
        if group.cases.is_empty() {
            continue;
        }

        println!();
        print_group_header(group.name);
        print_group_docs(group);

        let (index, fields) = create_group_index(config, group)?;
        let reader = index.reader()?;
        let searcher = reader.searcher();

        let mut passed = 0;

        for case in group.cases {
            let query = match config.route_query(&searcher, &fields.icu, case.query) {
                Ok(q) => q,
                Err(_) if case.matches.is_empty() => {
                    passed += 1;
                    println!(
                        "{GREEN}PASS{RESET} {name}: {YELLOW}{desc}{RESET}",
                        name = case.name,
                        desc = case.description,
                    );
                    println!(
                        "Query: {RED}{}{RESET} -> {BLUE}(parse error → empty){RESET}",
                        escape_invisible(case.query),
                    );
                    println!("  (no hits)\n");
                    continue;
                }
                Err(e) => return Err(e),
            };
            let query_debug = format!("{:?}", query);
            let hits = search_with_snippets(&searcher, query.as_ref(), &fields, 100)?;
            let hit_ids: HashSet<String> = hits.iter().map(|h| h.id.clone()).collect();

            // Separate plain doc IDs from ordering constraints ("a > b")
            let mut expected: HashSet<&str> = HashSet::new();
            let mut ordering_constraints: Vec<(&str, &str)> = Vec::new();
            for entry in case.matches {
                if let Some((left, right)) = entry.split_once(" > ") {
                    ordering_constraints.push((left.trim(), right.trim()));
                } else {
                    expected.insert(entry);
                }
            }

            let mut errors = Vec::new();

            for id in &expected {
                if !hit_ids.contains(*id) {
                    errors.push(format!("missing expected hit: {id}"));
                }
            }
            for id in &hit_ids {
                if !expected.contains(id.as_str()) {
                    errors.push(format!("unexpected hit: {id}"));
                }
            }

            // Verify ordering constraints: "a > b" means a must have score >= b
            for (higher, lower) in &ordering_constraints {
                let h_score = hits.iter().find(|h| h.id == *higher).map(|h| h.score);
                let l_score = hits.iter().find(|h| h.id == *lower).map(|h| h.score);
                match (h_score, l_score) {
                    (Some(hs), Some(ls)) if hs < ls => {
                        errors.push(format!(
                            "ordering violated: {higher} ({hs:.3}) < {lower} ({ls:.3})"
                        ));
                    }
                    (None, _) => {
                        errors.push(format!("ordering: {higher} not found in hits"));
                    }
                    (_, None) => {
                        errors.push(format!("ordering: {lower} not found in hits"));
                    }
                    _ => {}
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
                        let _ = &frag[range.start..range.end];
                    }
                }

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

            let (status, status_color) = if errors.is_empty() {
                passed += 1;
                ("PASS", GREEN)
            } else {
                ("FAIL", RED)
            };

            println!(
                "{status_color}{status}{RESET} {name}: {YELLOW}{desc}{RESET}",
                name = case.name,
                desc = case.description,
            );
            println!(
                "Query: {RED}{}{RESET} -> {BLUE}{query_debug}{RESET}",
                escape_invisible(case.query),
            );
            print_hits(&hits);

            if !errors.is_empty() {
                for err in &errors {
                    println!("  !! {err}");
                }
                all_failed.push((group.name, case, errors));
            }

            println!();
        }

        total_passed += passed;
        total_cases += group.cases.len();
        println!(
            "  {GRAY}Group result: {passed}/{}{RESET}",
            group.cases.len()
        );
    }

    println!("\n=== Test Summary ===");
    println!("Passed: {total_passed}/{total_cases}");

    if !all_failed.is_empty() {
        println!("\n=== Failures ===");
        for (group_name, case, errors) in &all_failed {
            println!(
                "  {RED}FAIL{RESET} [{group_name}] {name}: {YELLOW}{desc}{RESET}",
                name = case.name,
                desc = case.description,
            );
            for err in errors {
                println!("    - {err}");
            }
        }
    }

    Ok(all_failed.is_empty())
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
