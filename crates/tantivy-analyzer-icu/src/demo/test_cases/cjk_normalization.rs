use super::DEMO_SENTENCE;
use super::{QueryTestCase, QueryTestGroup};

pub const GROUP: QueryTestGroup = QueryTestGroup {
    name: "CJK Normalization",
    docs: &[
        ("demo-1", DEMO_SENTENCE),
        ("ja-3", "コンピュータを使う"),
        ("norm-2", "㈱東京"),
        ("norm-3", "\u{FF76}\u{FF9E}\u{FF77}"),
        ("norm-4", "①②③"),
        ("aux-1", "𠮷野家"),
        ("aux-2", "家𠮷野"),
        ("aux-3", "𠮷𠮷"),
        ("ext-1", "㐀是一个字"),
        ("ext-2", "𠀀是另一个字"),
        ("ext-3", "𠀀𠁀"),
    ],
    cases: &[
        QueryTestCase {
            name: "compat_reiwa_query",
            query: "㋿",
            matches: &["demo-1"],
            description: "㋿ as query should match sig-1 (normalizes to 令和)",
        },
        QueryTestCase {
            name: "compat_kabushiki_query",
            query: "㍿",
            matches: &["demo-1"],
            description: "㍿ as query should match sig-1 (normalizes to 株式会社)",
        },
        QueryTestCase {
            name: "compat_meiji_query",
            query: "㍾",
            matches: &["demo-1"],
            description: "㍾ as query should match sig-1 (normalizes to 明治)",
        },
        QueryTestCase {
            name: "compat_kabushiki_alt_query",
            query: "㈱",
            matches: &["demo-1", "norm-2"],
            description: "㈱→(株): 株 isolated in bigram but caught by unigram fallback",
        },
        QueryTestCase {
            name: "halfwidth_katakana_query",
            query: "\u{FF7A}\u{FF9D}\u{FF8B}\u{FF9F}\u{FF6D}\u{FF70}\u{FF80}", // ｺﾝﾋﾟｭｰﾀ
            matches: &["ja-3"],
            description: "Halfwidth katakana query matches fullwidth katakana doc",
        },
        QueryTestCase {
            name: "halfwidth_katakana_gaki",
            query: "ガキ",
            matches: &["norm-3"],
            description: "Fullwidth Katakana query should match halfwidth doc",
        },
        QueryTestCase {
            name: "number_123",
            query: "123",
            matches: &["norm-4"],
            description: "①②③ normalized to 123, should match 123 query",
        },
        QueryTestCase {
            name: "circled_digits_query",
            query: "①②③",
            matches: &["norm-4"],
            description: "Circled digit query normalizes to plain digits, matching circled digit doc",
        },
        QueryTestCase {
            name: "enclosed_cjk_query",
            query: "㈱東京",
            matches: &["norm-2"],
            description: "Enclosed ㈱ in query normalizes to (株), matching doc containing ㈱東京",
        },
        QueryTestCase {
            name: "ivs_in_query",
            query: "北沢\u{E0100}",
            matches: &["demo-1"],
            description: "Query with IVS matches doc with IVS (both stripped by NFKC)",
        },
        QueryTestCase {
            name: "supplementary_yoshinoya",
            query: "野家",
            matches: &["aux-1"],
            description: "Bigram after supplementary plane CJK char",
        },
        QueryTestCase {
            name: "supplementary_bigram_left",
            query: "𠮷野",
            matches: &["aux-1", "aux-2"],
            description: "Supplementary(4byte) + BMP(3byte) bigram",
        },
        QueryTestCase {
            name: "supplementary_in_query",
            query: "𠮷",
            matches: &["aux-1", "aux-2", "aux-3"],
            description: "Supplementary single char query via unigram (is_han_char)",
        },
        QueryTestCase {
            name: "two_supplementary_bigram",
            query: "𠮷𠮷",
            matches: &["aux-3"],
            description: "Two 4-byte chars bigram (8-byte range)",
        },
        QueryTestCase {
            name: "supplementary_reversed",
            query: "家𠮷",
            matches: &["aux-2"],
            description: "Reversed order bigram",
        },
        QueryTestCase {
            name: "cjk_ext_a_unigram",
            query: "㐀",
            matches: &["ext-1"],
            description: "CJK Extension A recognized as Han",
        },
        QueryTestCase {
            name: "cjk_ext_b_unigram",
            query: "𠀀",
            matches: &["ext-2", "ext-3"],
            description: "CJK Extension B (supplementary plane) recognized as Han",
        },
        QueryTestCase {
            name: "cjk_ext_b_bigram",
            query: "𠀀𠁀",
            matches: &["ext-3"],
            description: "Two Extension B chars form a bigram",
        },
    ],
};
