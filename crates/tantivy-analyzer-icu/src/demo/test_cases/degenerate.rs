use super::{QueryTestCase, QueryTestGroup};

pub const GROUP: QueryTestGroup = QueryTestGroup {
    name: "Degenerate Input",
    docs: &[],
    cases: &[
        QueryTestCase {
            name: "empty_query",
            query: "",
            matches: &[],
            description: "Empty query returns empty, no panic",
        },
        QueryTestCase {
            name: "whitespace_only",
            query: "   \t\n  ",
            matches: &[],
            description: "Whitespace-only query returns empty",
        },
        QueryTestCase {
            name: "cjk_punct_only",
            query: "！？，。、",
            matches: &[],
            description: "CJK punctuation only returns empty",
        },
        QueryTestCase {
            name: "ascii_punct_only",
            query: "!!!???",
            matches: &[],
            description: "ASCII punctuation only returns empty",
        },
        QueryTestCase {
            name: "zero_width_only",
            query: "\u{200B}\u{FEFF}",
            matches: &[],
            description: "Zero-width chars only returns empty",
        },
        QueryTestCase {
            name: "variation_selector_only",
            query: "\u{E0100}\u{E0101}",
            matches: &[],
            description: "Variation selectors only returns empty",
        },
        QueryTestCase {
            name: "mixed_ignorable",
            query: "\u{200B}   \u{FEFF}",
            matches: &[],
            description: "Mixed ignorable chars returns empty",
        },
    ],
};
