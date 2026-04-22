use super::DEMO_SENTENCE;
use super::LONG_DOCUMENT_TEXT;
use super::{QueryTestCase, QueryTestGroup};

pub const GROUP: QueryTestGroup = QueryTestGroup {
    name: "Chinese Combined Query",
    docs: &[
        ("demo-1", DEMO_SENTENCE),
        ("zh-1", "我爱北京天安门"),
        ("zh-2", "北京是中国的首都"),
        ("zh-6", "我"),
        ("zh-7", "北"),
        ("zh-8", "南京市长江大桥"),
        ("zh-9", "只有北字没有后面"),
        ("zh-10", "只有京字"),
        ("multi-1", "北京 在 东京"),
        ("multi-2", "北京，东京"),
        ("multi-3", "北京。东京"),
        ("multi-4", "京 东"),
        ("long-1", LONG_DOCUMENT_TEXT),
    ],
    cases: &[
        QueryTestCase {
            name: "space_separated_doc_unigram",
            query: "京 东",
            matches: &[
                "long-1", "multi-1", "multi-2", "multi-3", "multi-4", "zh-1", "zh-10", "zh-2",
                "zh-8",
            ],
            description: "Doc '京 东' matches space-separated query via unigram",
        },
        QueryTestCase {
            name: "space_separated_han_query",
            query: "京 东",
            matches: &[
                "long-1", "multi-1", "multi-2", "multi-3", "multi-4", "zh-1", "zh-10", "zh-2",
                "zh-8",
            ],
            description: "Space-separated Han chars match via unigram (京 or 东)",
        },
        QueryTestCase {
            name: "fullwidth_space_separated_query",
            query: "京\u{3000}东",
            matches: &[
                "long-1", "multi-1", "multi-2", "multi-3", "multi-4", "zh-1", "zh-10", "zh-2",
                "zh-8",
            ],
            description: "Full-width space separator behaves like regular space",
        },
        QueryTestCase {
            name: "mixed_bigram_and_isolated",
            query: "北京 我",
            matches: &[
                "long-1", "multi-1", "multi-2", "multi-3", "zh-1", "zh-2", "zh-6",
            ],
            description: "Bigram '北京' + isolated unigram '我' both contribute matches",
        },
        QueryTestCase {
            name: "multiple_isolated_han",
            query: "京 东 北",
            matches: &[
                "demo-1", "long-1", "multi-1", "multi-2", "multi-3", "multi-4", "zh-1", "zh-10",
                "zh-2", "zh-7", "zh-8", "zh-9",
            ],
            description: "All three chars isolated, matched via unigram individually",
        },
        QueryTestCase {
            name: "route_space_separated_unigram",
            query: "二 人",
            matches: &["demo-1"],
            description: "Space in query → each char isolated → unigram lookup",
        },
        QueryTestCase {
            name: "route_mixed_bigram_unigram",
            query: "注文 幸",
            matches: &["demo-1"],
            description: "注文 adjacent → bigram; 幸 isolated → unigram",
        },
        QueryTestCase {
            name: "route_adjacent_cjk",
            query: "下北沢",
            matches: &["demo-1"],
            description: "Adjacent CJK → bigrams 下北, 北沢 → match",
        },
        QueryTestCase {
            name: "kabushiki_gaisha",
            query: "株式会社",
            matches: &["demo-1"],
            description: "'株式会社' should match ㍿ doc",
        },
    ],
};
