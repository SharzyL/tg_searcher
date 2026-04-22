use super::DEMO_SENTENCE;
use super::{QueryTestCase, QueryTestGroup};

pub const GROUP: QueryTestGroup = QueryTestGroup {
    name: "Japanese Query",
    docs: &[
        ("demo-1", DEMO_SENTENCE),
        ("ja-2", "東京タワーは素晴らしい"),
        ("ja-3", "コンピュータを使う"),
        ("ja-4", "食べる"),
        ("ja-5", "ありがとうございます"),
        ("ja-6", "日本語を勉強しています"),
        ("ja-11", "タベル"),
        ("ja-12", "たべる"),
        ("ja-13", "がき"),
        ("ja-14", "か\u{3099}き"),
        ("ja-15", "でんわ"),
        ("ja-16", "てんわ"),
    ],
    cases: &[
        QueryTestCase {
            name: "ja_taberu",
            query: "食べる",
            matches: &["ja-4"],
            description: "Japanese verb via Han-Kana bigram",
        },
        QueryTestCase {
            name: "ja_tokyo_tower",
            query: "東京タワー",
            matches: &["ja-2"],
            description: "Kanji + Katakana combo query",
        },
        QueryTestCase {
            name: "ja_nihongo",
            query: "日本語",
            matches: &["ja-6"],
            description: "Pure Kanji Japanese query",
        },
        QueryTestCase {
            name: "ja_arigatou",
            query: "ありがとう",
            matches: &["ja-5"],
            description: "Pure Hiragana query",
        },
        QueryTestCase {
            name: "ja_computer",
            query: "コンピュータ",
            matches: &["ja-3"],
            description: "Pure Katakana query with long vowel",
        },
        QueryTestCase {
            name: "ivs_shimokitazawa",
            query: "北沢",
            matches: &["demo-1"],
            description: "Plain query matches doc with IVS on 沢 (IVS absorbed by NFKC)",
        },
        QueryTestCase {
            name: "katakana_vs_hiragana",
            query: "食べる",
            matches: &["ja-4"],
            description: "Han+Hiragana should not match pure Katakana or pure Hiragana",
        },
        QueryTestCase {
            name: "hiragana_vs_katakana",
            query: "たべる",
            matches: &["ja-12"],
            description: "Pure Hiragana should not match Han+Hiragana or Katakana",
        },
        QueryTestCase {
            name: "katakana_only",
            query: "タベル",
            matches: &["ja-11"],
            description: "Pure Katakana query only matches Katakana doc",
        },
        QueryTestCase {
            name: "dakuten_preserved_de",
            query: "でんわ",
            matches: &["ja-15"],
            description: "Dakuten NOT foldable: で ≠ て",
        },
        QueryTestCase {
            name: "dakuten_preserved_te",
            query: "てんわ",
            matches: &["ja-16"],
            description: "てんわ does not match でんわ",
        },
        QueryTestCase {
            name: "precomposed_dakuten_query",
            query: "がき",
            matches: &["ja-13", "ja-14"],
            description: "Precomposed dakuten query matches both forms",
        },
        QueryTestCase {
            name: "decomposed_dakuten_query",
            query: "か\u{3099}き",
            matches: &["ja-13", "ja-14"],
            description: "Decomposed dakuten query matches both forms",
        },
        QueryTestCase {
            name: "single_kana_low_recall",
            query: "は",
            matches: &[],
            description: "[locked] Single kana: dropped by unigram, bigrammed in docs → no match",
        },
    ],
};
