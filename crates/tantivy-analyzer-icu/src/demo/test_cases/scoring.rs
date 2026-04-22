use super::{QueryTestCase, QueryTestGroup};

pub const GROUP: QueryTestGroup = QueryTestGroup {
    name: "Scoring",
    docs: &[
        // --- CJK docs: 天 is a common unigram (appears in many docs → low IDF) ---
        ("sc-1", "今天去公园散步"),
        ("sc-2", "明天有大雨"),
        ("sc-3", "天空很蓝很美"),
        ("sc-4", "昨天很冷"),
        ("sc-5", "天气预报说晴天"),
        ("sc-6", "每天学习很重要"),
        // --- 北京 is a rarer bigram (appears in few docs → high IDF) ---
        ("sc-7", "北京欢迎你"),
        ("sc-8", "我爱北京天安门"),
        ("sc-9", "北京今天天气好"),
        // --- Diacritic docs ---
        ("sc-10", "a cup of café"),
        ("sc-11", "a cup of cafe"),
        ("sc-12", "le résumé du projet"),
        ("sc-13", "the resume is ready"),
        // --- Mixed CJK + diacritic ---
        ("sc-14", "北京的café真好"),
        ("sc-15", "北京的cafe也行"),
        // --- English multi-term ---
        ("sc-16", "Tokyo tower and Kyoto temple"),
        ("sc-17", "Tokyo is modern"),
        ("sc-18", "Kyoto is ancient"),
        // --- Multiple diacritics ---
        ("sc-19", "naïve résumé approach"),
        ("sc-20", "naive resume approach"),
    ],
    cases: &[
        // === Bigram vs unigram, multiple vs single match ===
        // Query "北京 天" produces: bigram "北京" (high IDF) + unigram "天" (low IDF)
        // - Docs with both terms (sc-8, sc-9) rank highest (sum of IDFs)
        // - Bigram-only matches (sc-7, sc-14, sc-15) beat unigram-only (sc-1..6)
        QueryTestCase {
            name: "bigram_and_multi_term",
            query: "北京 天",
            matches: &[
                "sc-1",
                "sc-2",
                "sc-3",
                "sc-4",
                "sc-5",
                "sc-6", // unigram 天 only
                "sc-7",
                "sc-8",
                "sc-9", // bigram 北京 (some also have 天)
                "sc-14",
                "sc-15",       // bigram 北京
                "sc-9 > sc-7", // both terms > bigram only
                "sc-8 > sc-7", // both terms > bigram only
                "sc-7 > sc-1", // bigram-only > unigram-only
                "sc-9 > sc-1", // both terms > unigram-only
            ],
            description: "Rare bigram outscores common unigram; two-term match outscores one-term",
        },
        // === Diacritic match > folded match ===
        // Query "café" → folded_bigram:"cafe" + diacritic:"café" (boosted 1.5x)
        // sc-10 "café" matches both fields → higher score
        // sc-11 "cafe" matches only folded → lower score
        QueryTestCase {
            name: "diacritic_over_folded",
            query: "café",
            matches: &[
                "sc-10",
                "sc-11",
                "sc-14",
                "sc-15",
                "sc-10 > sc-11", // diacritic match > folded only
                "sc-14 > sc-15", // same pattern with CJK context
            ],
            description: "Accented 'café' outscores plain 'cafe' via diacritic boost",
        },
        QueryTestCase {
            name: "diacritic_over_folded_resume",
            query: "résumé",
            matches: &[
                "sc-12",
                "sc-13",
                "sc-19",
                "sc-20",
                "sc-12 > sc-13", // diacritic > folded
                "sc-19 > sc-20", // diacritic > folded
            ],
            description: "Accented 'résumé' outscores plain 'resume'",
        },
        // === Multiple diacritics accumulate ===
        // Query "naïve résumé" → folded:naive,resume + diacritic:naïve,résumé
        // sc-19 has both diacritics → matches all 4 clauses
        // sc-20 has neither diacritic → matches only 2 folded clauses
        QueryTestCase {
            name: "multi_diacritic_accumulation",
            query: "naïve résumé",
            matches: &[
                "sc-12",
                "sc-13",
                "sc-19",
                "sc-20",
                "sc-19 > sc-20", // 4 matching clauses > 2 matching clauses
            ],
            description: "Two diacritic matches accumulate larger score advantage",
        },
        // === English multi-term accumulation ===
        QueryTestCase {
            name: "english_multi_term",
            query: "tokyo kyoto",
            matches: &[
                "sc-16",
                "sc-17",
                "sc-18",
                "sc-16 > sc-17", // both terms > single
                "sc-16 > sc-18", // both terms > single
            ],
            description: "Doc with both 'tokyo' and 'kyoto' ranks above single-term matches",
        },
        // === Mixed CJK + diacritic accumulation ===
        // Query "北京 café" → bigram:北京 + folded:cafe + diacritic:café
        // sc-14 "北京的café真好" matches all 3 → highest
        // sc-15 "北京的cafe也行" matches bigram + folded → medium
        // sc-7 "北京欢迎你" matches bigram only → lower
        QueryTestCase {
            name: "mixed_cjk_diacritic",
            query: "北京 café",
            matches: &[
                "sc-7",
                "sc-8",
                "sc-9",
                "sc-10",
                "sc-11",
                "sc-14",
                "sc-15",
                "sc-14 > sc-15", // bigram + diacritic café > bigram + folded cafe
                "sc-14 > sc-7",  // bigram + café > bigram only
                "sc-15 > sc-7",  // bigram + cafe > bigram only
            ],
            description: "CJK bigram + diacritic café accumulate across fields",
        },
    ],
};
