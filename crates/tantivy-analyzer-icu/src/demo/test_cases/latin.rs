use super::DEMO_SENTENCE;
use super::{QueryTestCase, QueryTestGroup};

pub const GROUP: QueryTestGroup = QueryTestGroup {
    name: "Latin Script",
    docs: &[
        ("demo-1", DEMO_SENTENCE),
        ("en-1", "Hello World"),
        ("en-2", "The quick brown fox"),
        ("en-3", "Machine learning is fun"),
        ("mix-3", "Hello 你好 World"),
        ("norm-1", "\u{FF21}\u{FF50}\u{FF50}\u{FF4C}\u{FF45}"),
        ("norm-7", "apple"),
        ("norm-8", "caf\u{00E9}"),
        ("norm-9", "cafe\u{0301}"),
        ("de-1", "Stra\u{00DF}e"),
        ("de-2", "\u{00FC}ber"),
        ("gr-1", "\u{039E}\u{03AD}\u{03BD}\u{03BF}\u{03C2}"),
        ("tr-1", "\u{0130}stanbul"),
        ("es-1", "ni\u{00F1}o"),
    ],
    cases: &[
        QueryTestCase {
            name: "en_hello",
            query: "Hello",
            matches: &["en-1", "mix-3"],
            description: "English query, case insensitive",
        },
        QueryTestCase {
            name: "en_machine",
            query: "machine",
            matches: &["en-3"],
            description: "English lowercase query",
        },
        QueryTestCase {
            name: "fullwidth_apple",
            query: "apple",
            matches: &["norm-1", "norm-7"],
            description: "Halfwidth query should match fullwidth and halfwidth docs",
        },
        QueryTestCase {
            name: "halfwidth_apple_query",
            query: "apple",
            matches: &["norm-1", "norm-7"],
            description: "Halfwidth query matches both fullwidth and halfwidth docs",
        },
        QueryTestCase {
            name: "fullwidth_apple_query",
            query: "\u{FF21}\u{FF50}\u{FF50}\u{FF4C}\u{FF45}",
            matches: &["norm-1", "norm-7"],
            description: "Fullwidth query matches both forms",
        },
        QueryTestCase {
            name: "mixed_case_fullwidth",
            query: "\u{FF21}\u{FF30}\u{FF30}\u{FF2C}\u{FF25}",
            matches: &["norm-1", "norm-7"],
            description: "Fullwidth uppercase query (double normalization: fullwidth→half + casefold)",
        },
        QueryTestCase {
            name: "fullwidth_hello_query",
            query: "\u{FF28}\u{FF45}\u{FF4C}\u{FF4C}\u{FF4F}", // Ｈｅｌｌｏ
            matches: &["en-1", "mix-3"],
            description: "Fullwidth 'Ｈｅｌｌｏ' query matches ASCII 'Hello' docs",
        },
        QueryTestCase {
            name: "nfc_query_matches_both",
            query: "caf\u{00E9}",
            matches: &["norm-8", "norm-9"],
            description: "NFC query matches both NFC and NFD docs",
        },
        QueryTestCase {
            name: "nfd_query_matches_both",
            query: "cafe\u{0301}",
            matches: &["norm-8", "norm-9"],
            description: "NFD query matches both NFC and NFD docs",
        },
        QueryTestCase {
            name: "german_sharp_s",
            query: "strasse",
            matches: &["de-1"],
            description: "NFKC Casefold: ß → ss, so 'strasse' matches 'Straße'",
        },
        QueryTestCase {
            name: "german_sharp_s_upper",
            query: "STRASSE",
            matches: &["de-1"],
            description: "Uppercase + sharp s combination",
        },
        QueryTestCase {
            name: "greek_no_accent",
            query: "ξενος",
            matches: &["gr-1"],
            description: "Greek without accent matches accented doc (diacritic folding)",
        },
        QueryTestCase {
            name: "greek_with_accent",
            query: "ξένος",
            matches: &["gr-1"],
            description: "Greek with accent also matches",
        },
        QueryTestCase {
            name: "greek_final_sigma",
            query: "Ξένος",
            matches: &["gr-1"],
            description: "Original form also matches itself",
        },
        QueryTestCase {
            name: "mixed_case_ascii",
            query: "HeLLo",
            matches: &["en-1", "mix-3"],
            description: "Random case ASCII still matches",
        },
        QueryTestCase {
            name: "latin_no_accent_cafe",
            query: "cafe",
            matches: &["norm-8", "norm-9"],
            description: "Accentless query matches accented docs",
        },
        QueryTestCase {
            name: "latin_accent_cafe",
            query: "caf\u{00E9}",
            matches: &["norm-8", "norm-9"],
            description: "Accented query matches both NFC and NFD forms",
        },
        QueryTestCase {
            name: "french_naive_plain",
            query: "naive",
            matches: &["demo-1"],
            description: "Plain query matches accented doc: naïve",
        },
        QueryTestCase {
            name: "french_naive_accented",
            query: "na\u{00EF}ve", // naïve
            matches: &["demo-1"],
            description: "Accented query matches accented doc: both fold to naive",
        },
        QueryTestCase {
            name: "spanish_nino_plain",
            query: "nino",
            matches: &["es-1"],
            description: "Plain query matches accented doc: niño",
        },
        QueryTestCase {
            name: "spanish_nino_accented",
            query: "ni\u{00F1}o", // niño
            matches: &["es-1"],
            description: "Accented query matches accented doc: both fold to nino",
        },
        QueryTestCase {
            name: "german_uber_plain",
            query: "uber",
            matches: &["de-2"],
            description: "Plain query matches accented doc: über",
        },
        QueryTestCase {
            name: "german_uber_accented",
            query: "\u{00FC}ber", // über
            matches: &["de-2"],
            description: "Accented query matches accented doc: both fold to uber",
        },
        QueryTestCase {
            name: "vietnamese_pho_plain",
            query: "pho",
            matches: &["demo-1"],
            description: "Plain query matches multi-diacritic doc: phở",
        },
        QueryTestCase {
            name: "vietnamese_pho_accented",
            query: "ph\u{1EDF}", // phở
            matches: &["demo-1"],
            description: "Accented query matches accented doc: both fold to pho",
        },
        QueryTestCase {
            name: "turkish_istanbul_plain",
            query: "istanbul",
            matches: &["tr-1"],
            description: "Plain query matches İstanbul doc (İ → i̇ → i)",
        },
        QueryTestCase {
            name: "turkish_istanbul_accented",
            query: "\u{0130}stanbul", // İstanbul
            matches: &["tr-1"],
            description: "İstanbul query matches İstanbul doc: both fold to istanbul",
        },
        QueryTestCase {
            name: "smartcase_cafe_plain",
            query: "cafe",
            matches: &["norm-8", "norm-9"],
            description: "Plain 'cafe' matches accented docs via folded_bigram (café→cafe)",
        },
        QueryTestCase {
            name: "smartcase_uber_plain",
            query: "uber",
            matches: &["de-2"],
            description: "Plain 'uber' matches über via folded_bigram",
        },
        QueryTestCase {
            name: "no_prefix_match_en",
            query: "appl",
            matches: &[],
            description: "No prefix matching: 'appl' ≠ 'apple'",
        },
        QueryTestCase {
            name: "no_suffix_match_en",
            query: "ello",
            matches: &[],
            description: "'ello' is not 'hello' — no suffix match",
        },
        QueryTestCase {
            name: "no_substring_match_en",
            query: "lear",
            matches: &[],
            description: "'lear' is not 'learning' — no substring match",
        },
        QueryTestCase {
            name: "route_noir_plain",
            query: "noir",
            matches: &["demo-1"],
            description: "Non-CJK, no diacritics → folded_bigram passthrough",
        },
        QueryTestCase {
            name: "route_noir_with_diacritic",
            query: "n\u{00F6}ir",
            matches: &["demo-1"],
            description: "nöir folds to noir in folded_bigram (match); diacritic has no nöir (no boost)",
        },
        QueryTestCase {
            name: "route_the_plain",
            query: "the",
            matches: &["demo-1", "en-2"],
            description: "No diacritics → broad match (matches thé via folding)",
        },
        QueryTestCase {
            name: "route_the_accented",
            query: "th\u{00E9}",
            matches: &["demo-1", "en-2"],
            description: "thé → folded_bigram (the) + diacritic (thé, boosted)",
        },
        QueryTestCase {
            name: "route_pho_plain",
            query: "pho",
            matches: &["demo-1"],
            description: "phở folded to pho in folded_bigram",
        },
        QueryTestCase {
            name: "route_pho_accented",
            query: "ph\u{1EDF}",
            matches: &["demo-1"],
            description: "phở → folded_bigram (pho) + diacritic (phở, boosted)",
        },
    ],
};
