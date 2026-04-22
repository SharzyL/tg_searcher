use super::{QueryTestCase, QueryTestGroup};

pub const GROUP: QueryTestGroup = QueryTestGroup {
    name: "Brahmic Script",
    docs: &[
        ("hi-1", "क्षमा"),
        ("hi-2", "कमा"),
        ("hi-3", "हिन्दी भाषा"),
        ("hi-4", "हिंदी भाषा"),
    ],
    cases: &[
        QueryTestCase {
            name: "devanagari_virama_preserved",
            query: "क्षमा",
            matches: &["hi-1"],
            description: "Devanagari virama preserved: क्ष ≠ क",
        },
        QueryTestCase {
            name: "devanagari_virama_recall",
            query: "हिन्दी",
            matches: &["hi-3"],
            description: "Virama form हिन्दी matches itself but not anusvara form हिंदी",
        },
        QueryTestCase {
            name: "devanagari_anusvara_recall",
            query: "हिंदी",
            matches: &["hi-4"],
            description: "Anusvara form हिंदी matches itself but not virama form हिन्दी",
        },
    ],
};
