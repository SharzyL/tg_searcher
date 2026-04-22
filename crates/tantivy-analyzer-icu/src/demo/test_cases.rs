pub const DEMO_SENTENCE: &str = "㋿Ξ㍾㍿の下北沢\u{E0100}店でnaïveなThé Noirとphởとكَبَابを注文、שָׁלוֹםとनमस्तेで先輩に挨拶した。8月10日、二 人 幸 终。";

pub const TEST_DOCUMENTS: &[(&str, &str)] = &[
    // === Demo sentence (README worked example) ===
    ("demo-1", DEMO_SENTENCE),
    // === Chinese ===
    ("zh-1", "我爱北京天安门"),
    ("zh-2", "北京是中国的首都"),
    ("zh-3", "今天天气真好"),
    ("zh-4", "苹果公司发布了新产品"),
    ("zh-5", "学习中文很有趣"),
    ("zh-6", "我"),
    ("zh-7", "北"),
    ("zh-8", "南京市长江大桥"),
    ("zh-9", "只有北字没有后面"),
    ("zh-10", "只有京字"),
    // === Japanese ===
    ("ja-1", "今日はいい天気です"),
    ("ja-2", "東京タワーは素晴らしい"),
    ("ja-3", "コンピュータを使う"),
    ("ja-4", "食べる"),
    ("ja-5", "ありがとうございます"),
    ("ja-6", "日本語を勉強しています"),
    ("ja-11", "タベル"),
    ("ja-12", "たべる"),
    ("ja-13", "がき"),
    ("ja-14", "か\u{3099}き"),
    // === Korean ===
    ("ko-1", "안녕하세요"),
    ("ko-2", "한국어를 공부합니다"),
    ("ko-3", "서울은 한국의 수도입니다"),
    // ko-4: Jamo decomposition of 안녕하세요
    (
        "ko-4",
        "\u{110B}\u{1161}\u{11AB}\u{1102}\u{1167}\u{11BC}\u{1112}\u{1161}\u{1109}\u{1166}\u{110B}\u{116D}",
    ),
    // === English ===
    ("en-1", "Hello World"),
    ("en-2", "The quick brown fox"),
    ("en-3", "Machine learning is fun"),
    // === Mixed ===
    ("mix-1", "iPhone 15 Pro Max 发布"),
    ("mix-2", "COVID-19 疫情"),
    ("mix-3", "Hello 你好 World"),
    ("mix-4", "Python 3.12 版本发布"),
    ("mix-5", "A我B"),
    ("mix-6", "あ我い"),
    ("mix-7", "い안"),
    // === Normalization ===
    ("norm-1", "\u{FF21}\u{FF50}\u{FF50}\u{FF4C}\u{FF45}"), // Ａｐｐｌｅ
    ("norm-2", "㈱東京"),
    ("norm-3", "\u{FF76}\u{FF9E}\u{FF77}"), // ｶﾞｷ
    ("norm-4", "①②③"),
    ("norm-7", "apple"),
    ("norm-8", "caf\u{00E9}"),  // café NFC
    ("norm-9", "cafe\u{0301}"), // café NFD
    // === Case folding ===
    ("de-1", "Stra\u{00DF}e"),                            // Straße
    ("gr-1", "\u{039E}\u{03AD}\u{03BD}\u{03BF}\u{03C2}"), // Ξένος
    ("tr-1", "\u{0130}stanbul"),                          // İstanbul
    // === Supplementary plane ===
    ("aux-1", "𠮷野家"),
    ("aux-2", "家𠮷野"),
    ("aux-3", "𠮷𠮷"),
    // === Cross-segment CJK ===
    ("multi-1", "北京 在 东京"),
    ("multi-2", "北京，东京"),
    ("multi-3", "北京。东京"),
    ("multi-4", "京 东"), // minimal: two Han chars separated by a single space
    // === CJK Extensions ===
    ("ext-1", "㐀是一个字"),
    ("ext-2", "𠀀是另一个字"),
    ("ext-3", "𠀀𠁀"),
    // === Latin accent folding ===
    ("es-1", "ni\u{00F1}o"), // niño
    ("de-2", "\u{00FC}ber"), // über
    // === Arabic ===
    ("ar-1", "احمد"),     // Ahmed, no hamza
    ("ar-2", "أحمد"),     // Ahmed, hamza above
    ("ar-3", "إبراهيم"),  // Ibrahim, hamza below
    ("ar-4", "آمين"),     // Amin, madda
    ("ar-5", "الله"),     // Allah
    ("ar-6", "الــــله"), // Allah with tatweel
    ("ar-7", "كتاب"),     // book, no harakat
    ("ar-8", "كِتَابٌ"),     // book, with harakat
    ("ar-9", "مدرسة"),    // school, ta marbuta
    ("ar-10", "مدرسه"),   // school, ha
    ("ar-11", "فى"),      // "in" with alif maqsura
    ("ar-12", "في"),      // "in" with standard ya
    ("ar-13", "٢٠٢٤"),    // Arabic-Indic 2024
    ("ar-14", "۲۰۲۴"),    // Persian 2024
    // === Hebrew ===
    ("he-2", "שלום"), // shalom without niqqud
    (
        "he-3",
        "\u{05D1}\u{05B0}\u{05BC}\u{05E8}\u{05B5}\u{05D0}\u{05E9}\u{05B4}\u{05C1}\u{05D9}\u{05EA}",
    ), // בְּרֵאשִׁית (bereshit with niqqud)
    ("he-4", "בראשית"), // bereshit without niqqud
    // === Japanese dakuten preservation ===
    ("ja-15", "でんわ"), // denwa (with dakuten)
    ("ja-16", "てんわ"), // tenwa (without dakuten)
    // === Devanagari ===
    ("hi-1", "क्षमा"),       // kshama (with virama conjunct)
    ("hi-2", "कमा"),        // kama (without conjunct)
    ("hi-3", "हिन्दी भाषा"), // hindi bhasha (virama in न्दी)
    ("hi-4", "हिंदी भाषा"),  // hindi bhasha (anusvara form, no virama)
    // === Long document ===
    ("long-1", LONG_DOCUMENT_TEXT),
];

pub const LONG_DOCUMENT_TEXT: &str = "\
今天天气非常好，我和朋友一起去了北京旅游。北京是中国的首都，有很多名胜古迹。\
我们参观了故宫和天安门广场，那里的建筑非常壮观。下午我们品尝了北京烤鸭，味道真好。\
The Great Wall of China is one of the most impressive structures in the world. \
We spent the whole afternoon exploring it. \
東京タワーから見た景色は素晴らしかった。日本の文化はとても深いです。\
コンピュータを使って日本語を勉強しています。\
한국어를 공부하는 것은 재미있습니다. 서울은 아름다운 도시입니다. \
Apple recently released the iPhone 15 Pro Max with cutting edge technology. \
Python 3.12 introduced many exciting new features for developers. \
苹果公司推出了最新的产品，搭载最新的芯片。学习编程是一件非常有趣的事情。";

pub struct QueryTestCase {
    pub name: &'static str,
    pub query: &'static str,
    pub must_match: &'static [&'static str],
    pub must_not_match: &'static [&'static str],
    pub expect_empty: bool,
    pub description: &'static str,
}

pub const QUERY_TEST_CASES: &[QueryTestCase] = &[
    // =========================================================================
    //  EXISTING TESTS (from previous implementation)
    // =========================================================================

    // === Single Han character queries: use unigram field ===
    QueryTestCase {
        name: "single_han_zhong",
        query: "中",
        must_match: &["zh-2", "zh-5"],
        must_not_match: &["en-1", "ko-1", "ja-1"],
        expect_empty: false,
        description: "Single Han char should match all docs containing it",
    },
    QueryTestCase {
        name: "single_han_wo",
        query: "我",
        must_match: &["zh-1", "zh-6", "mix-5", "mix-6"],
        must_not_match: &["zh-2", "zh-4"],
        expect_empty: false,
        description: "Single '我' should match all docs containing it",
    },
    QueryTestCase {
        name: "single_han_bei",
        query: "北",
        must_match: &["zh-1", "zh-2", "zh-7", "zh-9"],
        must_not_match: &[],
        expect_empty: false,
        description: "Single '北' should match all docs containing it",
    },
    QueryTestCase {
        name: "single_han_reiwa",
        query: "令",
        must_match: &["demo-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "Single '令' should match ㋿ doc via normalization",
    },
    QueryTestCase {
        name: "single_han_kabushiki",
        query: "株",
        must_match: &["demo-1", "norm-2"],
        must_not_match: &[],
        expect_empty: false,
        description: "Single '株' should match compatibility char docs",
    },
    // === Multi-char Han queries: use bigram field ===
    QueryTestCase {
        name: "multi_han_beijing",
        query: "北京",
        must_match: &["zh-1", "zh-2"],
        must_not_match: &["zh-4", "zh-5"],
        expect_empty: false,
        description: "'北京' should match docs containing 北京",
    },
    QueryTestCase {
        name: "multi_han_tianqi",
        query: "天气",
        must_match: &["zh-3"],
        must_not_match: &["zh-1"],
        expect_empty: false,
        description: "'天气' should match only the weather doc",
    },
    QueryTestCase {
        name: "reiwa_bigram",
        query: "令和",
        must_match: &["demo-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "'令和' should match ㋿ doc",
    },
    QueryTestCase {
        name: "meiji_bigram",
        query: "明治",
        must_match: &["demo-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "'明治' should match ㍾ doc",
    },
    QueryTestCase {
        name: "kabushiki_gaisha",
        query: "株式会社",
        must_match: &["demo-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "'株式会社' should match ㍿ doc",
    },
    QueryTestCase {
        name: "cross_expansion_bigram",
        query: "治株",
        must_match: &["demo-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "Bigram spanning NFKC expansion boundary: 治 from ㍾, 株 from ㍿",
    },
    // === Japanese: Han-Kana bigrams ===
    QueryTestCase {
        name: "ja_taberu",
        query: "食べる",
        must_match: &["ja-4"],
        must_not_match: &[],
        expect_empty: false,
        description: "Japanese verb via Han-Kana bigram",
    },
    QueryTestCase {
        name: "ja_tokyo_tower",
        query: "東京タワー",
        must_match: &["ja-2"],
        must_not_match: &[],
        expect_empty: false,
        description: "Kanji + Katakana combo query",
    },
    QueryTestCase {
        name: "ja_nihongo",
        query: "日本語",
        must_match: &["ja-6"],
        must_not_match: &[],
        expect_empty: false,
        description: "Pure Kanji Japanese query",
    },
    QueryTestCase {
        name: "ja_arigatou",
        query: "ありがとう",
        must_match: &["ja-5"],
        must_not_match: &[],
        expect_empty: false,
        description: "Pure Hiragana query",
    },
    QueryTestCase {
        name: "ja_computer",
        query: "コンピュータ",
        must_match: &["ja-3"],
        must_not_match: &[],
        expect_empty: false,
        description: "Pure Katakana query with long vowel",
    },
    QueryTestCase {
        name: "ivs_shimokitazawa",
        query: "北沢",
        must_match: &["demo-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "Plain query matches doc with IVS on 沢 (IVS absorbed by NFKC)",
    },
    // === Korean ===
    QueryTestCase {
        name: "ko_annyeong",
        query: "안녕",
        must_match: &["ko-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "Korean bigram query",
    },
    QueryTestCase {
        name: "ko_hangukeo",
        query: "한국어",
        must_match: &["ko-2"],
        must_not_match: &["ko-1", "ko-3"],
        expect_empty: false,
        description: "Korean 3-char query via bigrams",
    },
    // === English ===
    QueryTestCase {
        name: "en_hello",
        query: "Hello",
        must_match: &["en-1", "mix-3"],
        must_not_match: &["en-2", "en-3"],
        expect_empty: false,
        description: "English query, case insensitive",
    },
    QueryTestCase {
        name: "en_machine",
        query: "machine",
        must_match: &["en-3"],
        must_not_match: &["en-1"],
        expect_empty: false,
        description: "English lowercase query",
    },
    // === Normalization ===
    QueryTestCase {
        name: "fullwidth_apple",
        query: "apple",
        must_match: &["norm-1", "norm-7"],
        must_not_match: &[],
        expect_empty: false,
        description: "Halfwidth query should match fullwidth and halfwidth docs",
    },
    QueryTestCase {
        name: "halfwidth_katakana_gaki",
        query: "ガキ",
        must_match: &["norm-3"],
        must_not_match: &[],
        expect_empty: false,
        description: "Fullwidth Katakana query should match halfwidth doc",
    },
    QueryTestCase {
        name: "number_123",
        query: "123",
        must_match: &["norm-4"],
        must_not_match: &[],
        expect_empty: false,
        description: "①②③ normalized to 123, should match 123 query",
    },
    // === Mixed ===
    QueryTestCase {
        name: "iphone",
        query: "iPhone",
        must_match: &["mix-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "English product name",
    },
    QueryTestCase {
        name: "mixed_a_wo_i",
        query: "我",
        must_match: &["mix-6", "zh-1", "zh-6", "mix-5"],
        must_not_match: &["zh-2"],
        expect_empty: false,
        description: "Isolated Han (between kana) should match via unigram",
    },
    // === Supplementary plane ===
    QueryTestCase {
        name: "supplementary_yoshinoya",
        query: "野家",
        must_match: &["aux-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "Bigram after supplementary plane CJK char",
    },
    // =========================================================================
    //  P0: must_not_match precision (section 1.1)
    // =========================================================================
    QueryTestCase {
        name: "beijing_not_partial",
        query: "北京",
        must_match: &["zh-1", "zh-2"],
        must_not_match: &["zh-7", "zh-8", "zh-9", "zh-10"],
        expect_empty: false,
        description: "Bigram query should not match docs with only one of the chars",
    },
    QueryTestCase {
        name: "tianqi_not_partial",
        query: "天气",
        must_match: &["zh-3"],
        must_not_match: &["zh-1"],
        expect_empty: false,
        description: "'天安门' contains '天' but not bigram '天气'",
    },
    QueryTestCase {
        name: "reiwa_not_partial",
        query: "令和",
        must_match: &["demo-1"],
        must_not_match: &["zh-1", "zh-2"],
        expect_empty: false,
        description: "Unrelated docs should not match",
    },
    // P0: Cross-script precision (section 1.2)
    QueryTestCase {
        name: "katakana_vs_hiragana",
        query: "食べる",
        must_match: &["ja-4"],
        must_not_match: &["ja-11", "ja-12"],
        expect_empty: false,
        description: "Han+Hiragana should not match pure Katakana or pure Hiragana",
    },
    QueryTestCase {
        name: "hiragana_vs_katakana",
        query: "たべる",
        must_match: &["ja-12"],
        must_not_match: &["ja-4", "ja-11"],
        expect_empty: false,
        description: "Pure Hiragana should not match Han+Hiragana or Katakana",
    },
    QueryTestCase {
        name: "katakana_only",
        query: "タベル",
        must_match: &["ja-11"],
        must_not_match: &["ja-4", "ja-12"],
        expect_empty: false,
        description: "Pure Katakana query only matches Katakana doc",
    },
    // P0: Cross-language precision (section 1.3)
    QueryTestCase {
        name: "hello_not_cjk",
        query: "hello",
        must_match: &["en-1", "mix-3"],
        must_not_match: &["zh-1", "ja-1", "ko-1"],
        expect_empty: false,
        description: "English query should not match CJK docs",
    },
    QueryTestCase {
        name: "beijing_not_english",
        query: "北京",
        must_match: &["zh-1", "zh-2"],
        must_not_match: &["en-1", "en-2", "en-3", "mix-2"],
        expect_empty: false,
        description: "Chinese query should not match English docs",
    },
    QueryTestCase {
        name: "arabic_digits",
        query: "15",
        must_match: &["mix-1"],
        must_not_match: &["mix-4", "norm-4"],
        expect_empty: false,
        description: "'15' matches 'iPhone 15', not '3.12' or '①②③'→'123'",
    },
    // =========================================================================
    //  P0: Degenerate input (section 2)
    // =========================================================================
    QueryTestCase {
        name: "empty_query",
        query: "",
        must_match: &[],
        must_not_match: &[],
        expect_empty: true,
        description: "Empty query returns empty, no panic",
    },
    QueryTestCase {
        name: "whitespace_only",
        query: "   \t\n  ",
        must_match: &[],
        must_not_match: &[],
        expect_empty: true,
        description: "Whitespace-only query returns empty",
    },
    QueryTestCase {
        name: "cjk_punct_only",
        query: "！？，。、",
        must_match: &[],
        must_not_match: &[],
        expect_empty: true,
        description: "CJK punctuation only returns empty",
    },
    QueryTestCase {
        name: "ascii_punct_only",
        query: "!!!???",
        must_match: &[],
        must_not_match: &[],
        expect_empty: true,
        description: "ASCII punctuation only returns empty",
    },
    QueryTestCase {
        name: "zero_width_only",
        query: "\u{200B}\u{FEFF}",
        must_match: &[],
        must_not_match: &[],
        expect_empty: true,
        description: "Zero-width chars only returns empty",
    },
    QueryTestCase {
        name: "variation_selector_only",
        query: "\u{E0100}\u{E0101}",
        must_match: &[],
        must_not_match: &[],
        expect_empty: true,
        description: "Variation selectors only returns empty",
    },
    QueryTestCase {
        name: "mixed_ignorable",
        query: "\u{200B}   \u{FEFF}",
        must_match: &[],
        must_not_match: &[],
        expect_empty: true,
        description: "Mixed ignorable chars returns empty",
    },
    // =========================================================================
    //  P0: Compatibility char symmetry (section 3.1)
    // =========================================================================
    QueryTestCase {
        name: "compat_reiwa_query",
        query: "㋿",
        must_match: &["demo-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "㋿ as query should match sig-1 (normalizes to 令和)",
    },
    QueryTestCase {
        name: "compat_kabushiki_query",
        query: "㍿",
        must_match: &["demo-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "㍿ as query should match sig-1 (normalizes to 株式会社)",
    },
    QueryTestCase {
        name: "compat_meiji_query",
        query: "㍾",
        must_match: &["demo-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "㍾ as query should match sig-1 (normalizes to 明治)",
    },
    // ㈱ normalizes to (株) — parens break bigram, 株 is isolated Han dropped in bigram field.
    // But unigram field catches 株. Lock actual behavior.
    QueryTestCase {
        name: "compat_kabushiki_alt_query",
        query: "㈱",
        must_match: &["norm-2", "demo-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "㈱→(株): 株 isolated in bigram but caught by unigram fallback",
    },
    // P0: Fullwidth/halfwidth symmetry (section 3.2)
    QueryTestCase {
        name: "halfwidth_apple_query",
        query: "apple",
        must_match: &["norm-1", "norm-7"],
        must_not_match: &[],
        expect_empty: false,
        description: "Halfwidth query matches both fullwidth and halfwidth docs",
    },
    QueryTestCase {
        name: "fullwidth_apple_query",
        query: "\u{FF21}\u{FF50}\u{FF50}\u{FF4C}\u{FF45}",
        must_match: &["norm-1", "norm-7"],
        must_not_match: &[],
        expect_empty: false,
        description: "Fullwidth query matches both forms",
    },
    QueryTestCase {
        name: "mixed_case_fullwidth",
        query: "\u{FF21}\u{FF30}\u{FF30}\u{FF2C}\u{FF25}",
        must_match: &["norm-1", "norm-7"],
        must_not_match: &[],
        expect_empty: false,
        description: "Fullwidth uppercase query (double normalization: fullwidth→half + casefold)",
    },
    QueryTestCase {
        name: "fullwidth_hello_query",
        query: "\u{FF28}\u{FF45}\u{FF4C}\u{FF4C}\u{FF4F}", // Ｈｅｌｌｏ
        must_match: &["en-1", "mix-3"],
        must_not_match: &[],
        expect_empty: false,
        description: "Fullwidth 'Ｈｅｌｌｏ' query matches ASCII 'Hello' docs",
    },
    QueryTestCase {
        name: "halfwidth_katakana_query",
        query: "\u{FF7A}\u{FF9D}\u{FF8B}\u{FF9F}\u{FF6D}\u{FF70}\u{FF80}", // ｺﾝﾋﾟｭｰﾀ
        must_match: &["ja-3"],
        must_not_match: &[],
        expect_empty: false,
        description: "Halfwidth katakana query matches fullwidth katakana doc",
    },
    QueryTestCase {
        name: "circled_digits_query",
        query: "①②③",
        must_match: &["norm-4"],
        must_not_match: &[],
        expect_empty: false,
        description: "Circled digit query normalizes to plain digits, matching circled digit doc",
    },
    QueryTestCase {
        name: "enclosed_cjk_query",
        query: "㈱東京",
        must_match: &["norm-2"],
        must_not_match: &[],
        expect_empty: false,
        description: "Enclosed ㈱ in query normalizes to (株), matching doc containing ㈱東京",
    },
    // P0: Variation selector symmetry (section 3.3)
    QueryTestCase {
        name: "ivs_in_query",
        query: "北沢\u{E0100}",
        must_match: &["demo-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "Query with IVS matches doc with IVS (both stripped by NFKC)",
    },
    // P0: NFC/NFD symmetry (section 3.4)
    QueryTestCase {
        name: "nfc_query_matches_both",
        query: "caf\u{00E9}",
        must_match: &["norm-8", "norm-9"],
        must_not_match: &[],
        expect_empty: false,
        description: "NFC query matches both NFC and NFD docs",
    },
    QueryTestCase {
        name: "nfd_query_matches_both",
        query: "cafe\u{0301}",
        must_match: &["norm-8", "norm-9"],
        must_not_match: &[],
        expect_empty: false,
        description: "NFD query matches both NFC and NFD docs",
    },
    QueryTestCase {
        name: "precomposed_dakuten_query",
        query: "がき",
        must_match: &["ja-13", "ja-14"],
        must_not_match: &[],
        expect_empty: false,
        description: "Precomposed dakuten query matches both forms",
    },
    QueryTestCase {
        name: "decomposed_dakuten_query",
        query: "か\u{3099}き",
        must_match: &["ja-13", "ja-14"],
        must_not_match: &[],
        expect_empty: false,
        description: "Decomposed dakuten query matches both forms",
    },
    // P0: Jamo symmetry (section 3.5)
    QueryTestCase {
        name: "syllable_query_matches_jamo_doc",
        query: "안녕",
        must_match: &["ko-1", "ko-4"],
        must_not_match: &[],
        expect_empty: false,
        description: "Precomposed syllable query matches Jamo decomposed doc",
    },
    QueryTestCase {
        name: "jamo_query_matches_syllable_doc",
        query: "\u{110B}\u{1161}\u{11AB}\u{1102}\u{1167}\u{11BC}",
        must_match: &["ko-1", "ko-4"],
        must_not_match: &[],
        expect_empty: false,
        description: "Jamo query matches precomposed syllable doc",
    },
    // =========================================================================
    //  P0: Breadth — single char tests (section 5)
    // =========================================================================
    QueryTestCase {
        name: "single_han_jing",
        query: "京",
        must_match: &["zh-1", "zh-2", "zh-8", "ja-2", "norm-2"],
        must_not_match: &["zh-4", "zh-6", "ja-1", "en-1"],
        expect_empty: false,
        description: "Common char 京, matches multiple docs",
    },
    QueryTestCase {
        name: "single_han_ri",
        query: "日",
        must_match: &["ja-1", "ja-6"],
        must_not_match: &["zh-1", "zh-2"],
        expect_empty: false,
        description: "日 only in Japanese docs in our corpus",
    },
    QueryTestCase {
        name: "single_han_ben",
        query: "本",
        must_match: &["ja-6"],
        must_not_match: &["zh-1", "ja-1"],
        expect_empty: false,
        description: "本 in 日本語 (ja-6)",
    },
    QueryTestCase {
        name: "single_han_dong",
        query: "東",
        must_match: &["ja-2", "norm-2"],
        must_not_match: &["zh-1", "zh-2"],
        expect_empty: false,
        description: "東 in 東京タワー and ㈱東京",
    },
    QueryTestCase {
        name: "single_han_empty_result",
        query: "龘",
        must_match: &[],
        must_not_match: &[],
        expect_empty: true,
        description: "Rare char not in any doc",
    },
    // Single digit '1': not a Han char, goes bigram+unigram route.
    // ICU treats "15" as one number token, so "1" ≠ "15". Lock behavior.
    QueryTestCase {
        name: "single_non_han_digit",
        query: "1",
        must_match: &[],
        must_not_match: &[],
        expect_empty: true,
        description: "[locked] single digit: ICU number tokens are whole, '1'≠'15'",
    },
    // =========================================================================
    //  P1: Script group verification (section 6)
    // =========================================================================
    QueryTestCase {
        name: "han_kana_bigram_left",
        query: "あ我",
        must_match: &["mix-6"],
        must_not_match: &[],
        expect_empty: false,
        description: "Kana+Han bigram exists in mix-6; other 我 docs match via unigram fallback",
    },
    QueryTestCase {
        name: "han_kana_bigram_right",
        query: "我い",
        must_match: &["mix-6"],
        must_not_match: &[],
        expect_empty: false,
        description: "Han+Kana bigram exists in mix-6; other 我 docs match via unigram fallback",
    },
    // "A我" query: bigram field gets "a" (我 dropped as isolated Han), unigram gets "我".
    // Matches docs via unigram "我" fallback. Lock actual behavior.
    QueryTestCase {
        name: "isolated_han_dropped_from_bigram",
        query: "A我",
        must_match: &["zh-1", "zh-6", "mix-5", "mix-6"],
        must_not_match: &[],
        expect_empty: false,
        description: "[locked] A我: 我 dropped in bigram, but unigram fallback matches 我-containing docs",
    },
    QueryTestCase {
        name: "hankana_mixed_bigram",
        query: "食べ",
        must_match: &["ja-4"],
        must_not_match: &["ja-11", "ja-12"],
        expect_empty: false,
        description: "Han+Hiragana bigram precise match",
    },
    // い and 안 are different script groups (HanKana vs Hangul), no bigram formed.
    // But individual chars may still match as isolated unigrams in bigram field.
    QueryTestCase {
        name: "kana_bigram_not_cross_hangul",
        query: "い안",
        must_match: &["mix-7"],
        must_not_match: &[],
        expect_empty: false,
        description: "[locked] No cross-group bigram, but individual chars match as isolated terms",
    },
    // =========================================================================
    //  P1: Case folding edges (section 9)
    // =========================================================================
    QueryTestCase {
        name: "german_sharp_s",
        query: "strasse",
        must_match: &["de-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "NFKC Casefold: ß → ss, so 'strasse' matches 'Straße'",
    },
    QueryTestCase {
        name: "german_sharp_s_upper",
        query: "STRASSE",
        must_match: &["de-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "Uppercase + sharp s combination",
    },
    QueryTestCase {
        name: "greek_no_accent",
        query: "ξενος",
        must_match: &["gr-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "Greek without accent matches accented doc (diacritic folding)",
    },
    QueryTestCase {
        name: "greek_with_accent",
        query: "ξένος",
        must_match: &["gr-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "Greek with accent also matches",
    },
    QueryTestCase {
        name: "greek_final_sigma",
        query: "Ξένος",
        must_match: &["gr-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "Original form also matches itself",
    },
    QueryTestCase {
        name: "mixed_case_ascii",
        query: "HeLLo",
        must_match: &["en-1", "mix-3"],
        must_not_match: &[],
        expect_empty: false,
        description: "Random case ASCII still matches",
    },
    // en-1 also matches because "HELLO" normalizes to "hello" which exists in en-1.
    // mix-3 ranks higher because it matches both "hello" and "你好".
    QueryTestCase {
        name: "all_upper_cjk_mix",
        query: "HELLO 你好",
        must_match: &["mix-3"],
        must_not_match: &[],
        expect_empty: false,
        description: "[locked] Uppercase+CJK: en-1 matches via 'hello', mix-3 via both terms",
    },
    // =========================================================================
    //  P2: Long document (section 11)
    // =========================================================================
    QueryTestCase {
        name: "long_doc_hit",
        query: "北京",
        must_match: &["long-1", "zh-1", "zh-2"],
        must_not_match: &[],
        expect_empty: false,
        description: "Long doc containing 北京 is found",
    },
    // P2: Supplementary plane (section 12)
    QueryTestCase {
        name: "supplementary_bigram_left",
        query: "𠮷野",
        must_match: &["aux-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "Supplementary(4byte) + BMP(3byte) bigram",
    },
    QueryTestCase {
        name: "supplementary_in_query",
        query: "𠮷",
        must_match: &["aux-1", "aux-2", "aux-3"],
        must_not_match: &[],
        expect_empty: false,
        description: "Supplementary single char query via unigram (is_han_char)",
    },
    QueryTestCase {
        name: "two_supplementary_bigram",
        query: "𠮷𠮷",
        must_match: &["aux-3"],
        must_not_match: &["aux-1", "aux-2"],
        expect_empty: false,
        description: "Two 4-byte chars bigram (8-byte range)",
    },
    QueryTestCase {
        name: "supplementary_reversed",
        query: "家𠮷",
        must_match: &["aux-2"],
        must_not_match: &["aux-1"],
        expect_empty: false,
        description: "Reversed order bigram",
    },
    // P2: Cross-segment CJK (section 13)
    // Space-separated Han chars in a document are NOT bigrammed at index time.
    // "京东" as a continuous bigram query should not match them.
    // multi-4 ("京 东") is the minimal case: two adjacent Han chars separated
    // by a single space. The bigram index has no "京东" entry for this doc.
    QueryTestCase {
        name: "space_breaks_bigram_in_doc",
        query: "京东",
        must_match: &[],
        must_not_match: &["multi-4"],
        expect_empty: true,
        description: "Doc '京 东' (space-separated) has no bigram '京东' — bigram query misses",
    },
    QueryTestCase {
        name: "space_separated_doc_unigram",
        query: "京 东",
        must_match: &["multi-4"],
        must_not_match: &[],
        expect_empty: false,
        description: "Doc '京 东' matches space-separated query via unigram",
    },
    // Same principle for punctuation-separated and multi-word docs.
    QueryTestCase {
        name: "cross_segment_bigram_no_match",
        query: "京东",
        must_match: &[],
        must_not_match: &["multi-1", "multi-2", "multi-3"],
        expect_empty: true,
        description: "Continuous bigram query does not match docs with separated chars",
    },
    // Space-separated query: treated as isolated Han, goes through unigram
    // route. Matches any doc containing 京 or 东.
    QueryTestCase {
        name: "space_separated_han_query",
        query: "京 东",
        must_match: &["multi-1", "multi-2", "multi-3"],
        must_not_match: &[],
        expect_empty: false,
        description: "Space-separated Han chars match via unigram (京 or 东)",
    },
    // Full-width space (U+3000) is normalized to regular space by NFKC,
    // so it also breaks bigram adjacency.
    QueryTestCase {
        name: "fullwidth_space_separated_query",
        query: "京\u{3000}东",
        must_match: &["multi-1", "multi-2", "multi-3"],
        must_not_match: &[],
        expect_empty: false,
        description: "Full-width space separator behaves like regular space",
    },
    // Zero-width characters (ZWSP, ZWNJ, etc.) are removed by NFKC, so
    // they do NOT break adjacency — query behaves like "京东".
    QueryTestCase {
        name: "zwsp_does_not_break_bigram",
        query: "京\u{200B}东",
        must_match: &[],
        must_not_match: &["multi-1", "multi-2", "multi-3"],
        expect_empty: true,
        description: "ZWSP removed by NFKC, query acts as continuous bigram '京东'",
    },
    // Mixed bigram + isolated Han: "北京 我" should match docs with "北京"
    // (via bigram) and docs with "我" (via unigram), independently.
    QueryTestCase {
        name: "mixed_bigram_and_isolated",
        query: "北京 我",
        must_match: &["zh-1", "zh-2", "zh-6"],
        must_not_match: &[],
        expect_empty: false,
        description: "Bigram '北京' + isolated unigram '我' both contribute matches",
    },
    // Multiple isolated Han: all go through unigram route.
    QueryTestCase {
        name: "multiple_isolated_han",
        query: "京 东 北",
        must_match: &["multi-1", "multi-2", "multi-3", "zh-7"],
        must_not_match: &[],
        expect_empty: false,
        description: "All three chars isolated, matched via unigram individually",
    },
    QueryTestCase {
        name: "each_segment_separate",
        query: "北京",
        must_match: &["multi-1", "multi-2", "multi-3", "zh-1", "zh-2"],
        must_not_match: &[],
        expect_empty: false,
        description: "Each segment's bigrams exist independently",
    },
    // P2: CJK Extensions (section 14)
    QueryTestCase {
        name: "cjk_ext_a_unigram",
        query: "㐀",
        must_match: &["ext-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "CJK Extension A recognized as Han",
    },
    QueryTestCase {
        name: "cjk_ext_b_unigram",
        query: "𠀀",
        must_match: &["ext-2", "ext-3"],
        must_not_match: &[],
        expect_empty: false,
        description: "CJK Extension B (supplementary plane) recognized as Han",
    },
    QueryTestCase {
        name: "cjk_ext_b_bigram",
        query: "𠀀𠁀",
        must_match: &["ext-3"],
        must_not_match: &[],
        expect_empty: false,
        description: "Two Extension B chars form a bigram",
    },
    // P2: Prefix/substring (section 15)
    QueryTestCase {
        name: "no_prefix_match_en",
        query: "appl",
        must_match: &[],
        must_not_match: &["norm-1", "norm-7"],
        expect_empty: false,
        description: "No prefix matching: 'appl' ≠ 'apple'",
    },
    QueryTestCase {
        name: "no_prefix_match_cjk",
        query: "北京是",
        must_match: &["zh-2"],
        must_not_match: &[],
        expect_empty: false,
        description: "北京是 bigrams (北京,京是) both in zh-2",
    },
    QueryTestCase {
        name: "no_suffix_match_en",
        query: "ello",
        must_match: &[],
        must_not_match: &["en-1", "mix-3"],
        expect_empty: false,
        description: "'ello' is not 'hello' — no suffix match",
    },
    QueryTestCase {
        name: "no_substring_match_en",
        query: "lear",
        must_match: &[],
        must_not_match: &["en-3"],
        expect_empty: false,
        description: "'lear' is not 'learning' — no substring match",
    },
    // =========================================================================
    //  P0: Accent/diacritic folding (new)
    // =========================================================================
    QueryTestCase {
        name: "latin_no_accent_cafe",
        query: "cafe",
        must_match: &["norm-8", "norm-9"],
        must_not_match: &[],
        expect_empty: false,
        description: "Accentless query matches accented docs",
    },
    QueryTestCase {
        name: "latin_accent_cafe",
        query: "caf\u{00E9}",
        must_match: &["norm-8", "norm-9"],
        must_not_match: &[],
        expect_empty: false,
        description: "Accented query matches both NFC and NFD forms",
    },
    QueryTestCase {
        name: "french_naive_plain",
        query: "naive",
        must_match: &["demo-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "Plain query matches accented doc: naïve",
    },
    QueryTestCase {
        name: "french_naive_accented",
        query: "na\u{00EF}ve", // naïve
        must_match: &["demo-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "Accented query matches accented doc: both fold to naive",
    },
    QueryTestCase {
        name: "spanish_nino_plain",
        query: "nino",
        must_match: &["es-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "Plain query matches accented doc: niño",
    },
    QueryTestCase {
        name: "spanish_nino_accented",
        query: "ni\u{00F1}o", // niño
        must_match: &["es-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "Accented query matches accented doc: both fold to nino",
    },
    QueryTestCase {
        name: "german_uber_plain",
        query: "uber",
        must_match: &["de-2"],
        must_not_match: &[],
        expect_empty: false,
        description: "Plain query matches accented doc: über",
    },
    QueryTestCase {
        name: "german_uber_accented",
        query: "\u{00FC}ber", // über
        must_match: &["de-2"],
        must_not_match: &[],
        expect_empty: false,
        description: "Accented query matches accented doc: both fold to uber",
    },
    QueryTestCase {
        name: "vietnamese_pho_plain",
        query: "pho",
        must_match: &["demo-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "Plain query matches multi-diacritic doc: phở",
    },
    QueryTestCase {
        name: "vietnamese_pho_accented",
        query: "ph\u{1EDF}", // phở
        must_match: &["demo-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "Accented query matches accented doc: both fold to pho",
    },
    QueryTestCase {
        name: "turkish_istanbul_plain",
        query: "istanbul",
        must_match: &["tr-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "Plain query matches İstanbul doc (İ → i̇ → i)",
    },
    QueryTestCase {
        name: "turkish_istanbul_accented",
        query: "\u{0130}stanbul", // İstanbul
        must_match: &["tr-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "İstanbul query matches İstanbul doc: both fold to istanbul",
    },
    // =========================================================================
    //  P0: Arabic normalization (new)
    // =========================================================================
    QueryTestCase {
        name: "arabic_alif_hamza_above",
        query: "احمد",
        must_match: &["ar-1", "ar-2"],
        must_not_match: &[],
        expect_empty: false,
        description: "Alif hamza above (أ) normalized to plain alif",
    },
    QueryTestCase {
        name: "arabic_alif_hamza_below",
        query: "ابراهيم",
        must_match: &["ar-3"],
        must_not_match: &[],
        expect_empty: false,
        description: "Alif hamza below (إ) normalized to plain alif",
    },
    QueryTestCase {
        name: "arabic_alif_madda",
        query: "امين",
        must_match: &["ar-4"],
        must_not_match: &[],
        expect_empty: false,
        description: "Alif madda (آ) normalized to plain alif",
    },
    QueryTestCase {
        name: "arabic_harakat_plain_query",
        query: "كتاب",
        must_match: &["ar-7", "ar-8"],
        must_not_match: &[],
        expect_empty: false,
        description: "Plain query matches harakat doc: كِتَابٌ → كتاب",
    },
    QueryTestCase {
        name: "arabic_harakat_in_query",
        query: "كِتَابٌ",
        must_match: &["ar-7", "ar-8"],
        must_not_match: &[],
        expect_empty: false,
        description: "Harakat query matches plain doc: both fold to كتاب",
    },
    QueryTestCase {
        name: "arabic_tatweel_plain_query",
        query: "الله",
        must_match: &["ar-5", "ar-6"],
        must_not_match: &[],
        expect_empty: false,
        description: "Plain query matches tatweel doc: الــــله → الله",
    },
    QueryTestCase {
        name: "arabic_tatweel_in_query",
        query: "الــــله",
        must_match: &["ar-5", "ar-6"],
        must_not_match: &[],
        expect_empty: false,
        description: "Tatweel query matches both tatweel and plain docs",
    },
    QueryTestCase {
        name: "arabic_ta_marbuta",
        query: "مدرسه",
        must_match: &["ar-9", "ar-10"],
        must_not_match: &[],
        expect_empty: false,
        description: "Ta marbuta (ة) and ha (ه) unified",
    },
    QueryTestCase {
        name: "arabic_ta_marbuta_reverse",
        query: "مدرسة",
        must_match: &["ar-9", "ar-10"],
        must_not_match: &[],
        expect_empty: false,
        description: "Symmetric: ta marbuta query also matches ha doc",
    },
    QueryTestCase {
        name: "arabic_ya_maqsura",
        query: "في",
        must_match: &["ar-11", "ar-12"],
        must_not_match: &[],
        expect_empty: false,
        description: "Ya maqsura (ى) and ya (ي) unified",
    },
    QueryTestCase {
        name: "arabic_ya_maqsura_reverse",
        query: "فى",
        must_match: &["ar-11", "ar-12"],
        must_not_match: &[],
        expect_empty: false,
        description: "Symmetric: maqsura query matches ya doc",
    },
    QueryTestCase {
        name: "arabic_digits_ascii_query",
        query: "2024",
        must_match: &["ar-13", "ar-14"],
        must_not_match: &[],
        expect_empty: false,
        description: "ASCII query matches Arabic-Indic and Persian digit docs",
    },
    QueryTestCase {
        name: "arabic_digits_indic_query",
        query: "٢٠٢٤",
        must_match: &["ar-13", "ar-14"],
        must_not_match: &[],
        expect_empty: false,
        description: "Arabic-Indic digit query matches both digit forms",
    },
    QueryTestCase {
        name: "arabic_reverse_hamza",
        query: "أحمد",
        must_match: &["ar-1", "ar-2"],
        must_not_match: &[],
        expect_empty: false,
        description: "Hamza query matches both hamza and plain docs",
    },
    // =========================================================================
    //  P3: Known limitations (section 16)
    // =========================================================================
    // Single kana は: in bigram field, not isolated (bigrammed with neighbors).
    // Query は has isolated kana → kept, but no doc has standalone は term.
    // Unigram field: HanOnlyFilter drops kana. Lock: expect empty.
    QueryTestCase {
        name: "single_kana_low_recall",
        query: "は",
        must_match: &[],
        must_not_match: &[],
        expect_empty: true,
        description: "[locked] Single kana: dropped by unigram, bigrammed in docs → no match",
    },
    QueryTestCase {
        name: "punct_in_query_ignored",
        query: "北京。",
        must_match: &["zh-1", "zh-2"],
        must_not_match: &[],
        expect_empty: false,
        description: "Punctuation in query is stripped by tokenizer",
    },
    //  ---- Smartcase diacritic tests (new) ----
    //  P0: Japanese dakuten preservation
    QueryTestCase {
        name: "dakuten_preserved_de",
        query: "でんわ",
        must_match: &["ja-15"],
        must_not_match: &["ja-16"],
        expect_empty: false,
        description: "Dakuten NOT foldable: で ≠ て",
    },
    QueryTestCase {
        name: "dakuten_preserved_te",
        query: "てんわ",
        must_match: &["ja-16"],
        must_not_match: &["ja-15"],
        expect_empty: false,
        description: "てんわ does not match でんわ",
    },
    //  P0: Hebrew niqqud normalization
    QueryTestCase {
        name: "hebrew_niqqud_stripped",
        query: "שלום",
        must_match: &["demo-1", "he-2"],
        must_not_match: &[],
        expect_empty: false,
        description: "Plain Hebrew matches niqqud doc: niqqud stripped by SemiticNorm",
    },
    QueryTestCase {
        name: "hebrew_niqqud_in_query",
        query: "\u{05E9}\u{05B8}\u{05C1}\u{05DC}\u{05D5}\u{05B9}\u{05DD}",
        must_match: &["demo-1", "he-2"],
        must_not_match: &[],
        expect_empty: false,
        description: "Niqqud in query matches plain doc: both fold to שלום",
    },
    QueryTestCase {
        name: "hebrew_bereshit",
        query: "בראשית",
        must_match: &["he-3", "he-4"],
        must_not_match: &[],
        expect_empty: false,
        description: "Plain בראשית matches niqqud version",
    },
    //  P0: Devanagari virama preservation
    QueryTestCase {
        name: "devanagari_virama_preserved",
        query: "क्षमा",
        must_match: &["hi-1"],
        must_not_match: &["hi-2"],
        expect_empty: false,
        description: "Devanagari virama preserved: क्ष ≠ क",
    },
    QueryTestCase {
        name: "devanagari_virama_recall",
        query: "हिन्दी",
        must_match: &["hi-3"],
        must_not_match: &["hi-4"],
        expect_empty: false,
        description: "Virama form हिन्दी matches itself but not anusvara form हिंदी",
    },
    QueryTestCase {
        name: "devanagari_anusvara_recall",
        query: "हिंदी",
        must_match: &["hi-4"],
        must_not_match: &["hi-3"],
        expect_empty: false,
        description: "Anusvara form हिंदी matches itself but not virama form हिन्दी",
    },
    //  P0: Arabic harakat still stripped (now by SemiticNorm)
    QueryTestCase {
        name: "arabic_harakat_still_stripped",
        query: "كتاب",
        must_match: &["ar-7", "ar-8"],
        must_not_match: &[],
        expect_empty: false,
        description: "Arabic harakat still stripped (by SemiticNorm, not DiacriticFolding)",
    },
    //  P0: Smartcase Latin diacritics
    QueryTestCase {
        name: "smartcase_cafe_plain",
        query: "cafe",
        must_match: &["norm-8", "norm-9"],
        must_not_match: &[],
        expect_empty: false,
        description: "Plain 'cafe' matches accented docs via folded_bigram (café→cafe)",
    },
    QueryTestCase {
        name: "smartcase_uber_plain",
        query: "uber",
        must_match: &["de-2"],
        must_not_match: &[],
        expect_empty: false,
        description: "Plain 'uber' matches über via folded_bigram",
    },
    //  ---- Query routing examples from README (against demo-1) ----
    QueryTestCase {
        name: "route_adjacent_cjk",
        query: "下北沢",
        must_match: &["demo-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "Adjacent CJK → bigrams 下北, 北沢 → match",
    },
    QueryTestCase {
        name: "route_bigram_no_match",
        query: "二人",
        must_match: &[],
        must_not_match: &["demo-1"],
        expect_empty: false,
        description: "二人 bigram not in index (二 and 人 are space-separated in doc)",
    },
    QueryTestCase {
        name: "route_space_separated_unigram",
        query: "二 人",
        must_match: &["demo-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "Space in query → each char isolated → unigram lookup",
    },
    QueryTestCase {
        name: "route_mixed_bigram_unigram",
        query: "注文 幸",
        must_match: &["demo-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "注文 adjacent → bigram; 幸 isolated → unigram",
    },
    QueryTestCase {
        name: "route_noir_plain",
        query: "noir",
        must_match: &["demo-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "Non-CJK, no diacritics → folded_bigram passthrough",
    },
    QueryTestCase {
        name: "route_noir_with_diacritic",
        query: "n\u{00F6}ir",
        must_match: &["demo-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "nöir folds to noir in folded_bigram (match); diacritic has no nöir (no boost)",
    },
    QueryTestCase {
        name: "route_the_plain",
        query: "the",
        must_match: &["demo-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "No diacritics → broad match (matches thé via folding)",
    },
    QueryTestCase {
        name: "route_the_accented",
        query: "th\u{00E9}",
        must_match: &["demo-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "thé → folded_bigram (the) + diacritic (thé, boosted)",
    },
    QueryTestCase {
        name: "route_pho_plain",
        query: "pho",
        must_match: &["demo-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "phở folded to pho in folded_bigram",
    },
    QueryTestCase {
        name: "route_pho_accented",
        query: "ph\u{1EDF}",
        must_match: &["demo-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "phở → folded_bigram (pho) + diacritic (phở, boosted)",
    },
    QueryTestCase {
        name: "route_naive_shimokitazawa",
        query: "na\u{00EF}ve 下北沢",
        must_match: &["demo-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "Per-token: naïve→diacritic, 下北沢→folded_bigram",
    },
    QueryTestCase {
        name: "route_single_han",
        query: "月",
        must_match: &["demo-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "Single Han char → unigram only",
    },
    QueryTestCase {
        name: "route_number_han_mix",
        query: "8月",
        must_match: &["demo-1"],
        must_not_match: &[],
        expect_empty: false,
        description: "8 is non-CJK, 月 is Han → no bigram; 月 falls back to unigram",
    },
];

/// Very long query for stress testing (1000 copies of 北).
pub fn very_long_query() -> String {
    "北".repeat(1000)
}
