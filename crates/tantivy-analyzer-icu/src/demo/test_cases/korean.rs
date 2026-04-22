use super::{QueryTestCase, QueryTestGroup};

pub const GROUP: QueryTestGroup = QueryTestGroup {
    name: "Korean Query",
    docs: &[
        ("ko-1", "안녕하세요"),
        ("ko-2", "한국어를 공부합니다"),
        ("ko-3", "서울은 한국의 수도입니다"),
        (
            "ko-4",
            "\u{110B}\u{1161}\u{11AB}\u{1102}\u{1167}\u{11BC}\u{1112}\u{1161}\u{1109}\u{1166}\u{110B}\u{116D}",
        ),
    ],
    cases: &[
        QueryTestCase {
            name: "ko_annyeong",
            query: "안녕",
            matches: &["ko-1", "ko-4"],
            description: "Korean bigram query",
        },
        QueryTestCase {
            name: "ko_hangukeo",
            query: "한국어",
            matches: &["ko-2"],
            description: "Korean 3-char query via bigrams",
        },
        QueryTestCase {
            name: "syllable_query_matches_jamo_doc",
            query: "안녕",
            matches: &["ko-1", "ko-4"],
            description: "Precomposed syllable query matches Jamo decomposed doc",
        },
        QueryTestCase {
            name: "jamo_query_matches_syllable_doc",
            query: "\u{110B}\u{1161}\u{11AB}\u{1102}\u{1167}\u{11BC}",
            matches: &["ko-1", "ko-4"],
            description: "Jamo query matches precomposed syllable doc",
        },
    ],
};
