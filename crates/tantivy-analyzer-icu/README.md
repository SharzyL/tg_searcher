# tantivy-analyzer-icu

ICU-based text analysis for [tantivy](https://github.com/quickwit-oss/tantivy),
with multilingual support for CJK bigram search, diacritic folding, and Arabic
normalization. All token offsets map back to the original (pre-normalization)
text for correct snippet highlighting.

## Feature Flags

| Feature | Description |
|---------|-------------|
| *(none)* | Core tokenizer and filters only. No tantivy index dependency. |
| `tantivy-search` | High-level tantivy integration: `ICUSearchConfig`, dual-field schema, query routing, snippet generation. |
| `demo` | Test harness with 128 query test cases. Implies `tantivy-search`. |

## Quick Start

### As a tokenizer/filter library

Without any feature flags, the crate provides composable tokenizers and filters
that implement `tantivy-tokenizer-api` traits:

```rust
use tantivy::tokenizer::TextAnalyzer;
use tantivy_analyzer_icu::{
    NormalizingICUTokenizer, DiacriticFoldingFilter,
    ArabicNormalizationFilter, CJKBigramFilter,
};

let analyzer = TextAnalyzer::builder(NormalizingICUTokenizer)
    .filter(DiacriticFoldingFilter)
    .filter(ArabicNormalizationFilter)
    .filter(CJKBigramFilter)
    .build();
```

### With tantivy integration (`tantivy-search` feature)

The `search` module provides `ICUSearchConfig` which handles the dual-field
schema, analyzer registration, query routing, and snippet generation:

```rust
use tantivy::schema::Schema;
use tantivy_analyzer_icu::search::ICUSearchConfig;

let config = ICUSearchConfig::default();

// 1. Schema: add a field group (creates stored + bigram + unigram fields)
let mut builder = Schema::builder();
let content = config.add_field_group(&mut builder, "content");
// Add your own fields freely:
// let chat_id = builder.add_i64_field("chat_id", INDEXED | STORED);
let schema = builder.build();

// 2. Register analyzers on the index
let index = tantivy::Index::create_in_ram(schema);
config.register_analyzers(&index);

// 3. Index documents (all three fields get the same text)
// writer.add_document(doc!(
//     content.stored => text,
//     content.bigram => text,
//     content.unigram => text,
// ))?;

// 4. Query routing (bigram for adjacent CJK, unigram for isolated Han)
// let query = config.route_query(&index, &content, "北京 我")?;

// 5. Snippet generation with dual-field highlight merging
// let snippet = config.snippet(&searcher, &query, &content, &body);
```

## Running the Demo

The crate includes a demo with 128 query test cases covering CJK bigrams, diacritic
folding, Arabic normalization, NFKC compatibility, and various edge cases.

```bash
# Run all automated tests (requires ICU libraries)
cargo run -p tantivy-analyzer-icu --features demo --example search_demo

# Interactive query mode
cargo run -p tantivy-analyzer-icu --features demo --example search_demo -- --interactive

# Run as a cargo test
cargo test -p tantivy-analyzer-icu --features demo --test search_demo
```

## How It Works

### Analyzer Pipeline

Text passes through these stages in order:

```text
Original text
  │
  ▼
NormalizingICUTokenizer
  ├─ NFKC Casefold (fullwidth→halfwidth, uppercase→lowercase,
  │    compatibility decomposition, casefold)
  ├─ ICU word segmentation (UAX #29)
  └─ CJK unigram expansion (split Han/Kana/Hangul runs into
       individual characters)
  │
  ▼
DiacriticFoldingFilter
  NFD → strip combining marks (CCC≠0) → NFC
  café→cafe, naïve→naive, über→uber, phở→pho
  Arabic harakat: كِتَابٌ→كتاب
  │
  ▼
ArabicNormalizationFilter
  Alif variants→Alif, Ta marbuta→Ha, Alif maqsura→Ya,
  Farsi variants→Arabic, tatweel/hamza removal,
  Arabic-Indic/Persian digits→ASCII
  │
  ▼
CJKBigramFilter (bigram field)  OR  HanOnlyFilter (unigram field)
```

### Worked Example

Input: `㋿Ξ㍾㍿の葛󠄀飾区でCafé 東京タワー فى مدرسة 北京 在 东京`

```text
Tokenizer:   [令] [和] [ξ] [明] [治] [株] [式] [会] [社] [の] [葛] [飾] [区] [で] [café] [東] [京] [タ] [ワ] [ー] [فى] [مدرسة] [北] [京] [在] [东] [京]
+ Diacritic: [令] [和] [ξ] [明] [治] [株] [式] [会] [社] [の] [葛] [飾] [区] [て] [cafe] [東] [京] [タ] [ワ] [ー] [فى] [مدرسة] [北] [京] [在] [东] [京]
+ Arabic:    ...same except [فى]→[في] [مدرسة]→[مدرسه]
→ Bigram:    [令和] [ξ] [明治] [治株] [株式] [式会] [会社] [社の] [の葛] [葛飾] [飾区] [区て] [cafe] [東京] [京タ] [タワ] [ワー] [في] [مدرسه] [北京] [东京]
→ Unigram:   [令] [和] [明] [治] [株] [式] [会] [社] [葛] [飾] [区] [東] [京] [北] [京] [在] [东] [京]
```

Things to note:
- **㋿Ξ㍾㍿**: NFKC expands compatibility chars (㋿→令和, ㍾→明治, ㍿→株式会社).
  Cross-expansion bigrams form naturally (治株) because original-text offsets are contiguous.
- **葛󠄀飾区**: IVS (U+E0100) is removed by NFKC. The offset is absorbed into 葛's range,
  so 葛飾 still forms a bigram.
- **Café→cafe**: Diacritic folding strips the accent. NFKC casefold handles uppercasing (C→c).
- **فى→في**: Arabic normalization maps alif maqsura (ى) to standard ya (ي).
  **مدرسة→مدرسه**: Ta marbuta (ة) normalized to ha (ه). Both are single-token non-CJK
  words, so they pass through the bigram filter unchanged.
- **北京 在 东京**: Space breaks offset adjacency. Bigram field gets \[北京\] and \[东京\] separately
  — no bigram "京 在" or "在 东". Unigram field keeps all five individual characters.
- **で→て**: Known issue — diacritic folding strips dakuten (combining mark U+3099, CCC=8)
  from kana. で (te + dakuten) becomes て. See Limitations.

### Dual-Field Indexing

Each text source is indexed into two fields to handle CJK search correctly:

- **Bigram field**: CJK characters produce overlapping bigrams (北京→"北京",
  京天→"京天"). Isolated Han characters are dropped (covered by unigram).
  Non-CJK tokens pass through unchanged.

- **Unigram field**: Only single Han characters are kept. Covers single-character
  queries that the bigram field misses.

### Query Routing

`ICUSearchConfig::route_query` analyzes the query to decide routing:

| Query | Route | Rationale |
|-------|-------|-----------|
| `北京` | Bigram only | Adjacent CJK → bigram covers it |
| `京 东` | Unigram only | Space-separated → each char isolated |
| `北京 我` | Bigram `北京` + Unigram `我` | Mixed: "北京" is adjacent, "我" is isolated |
| `hello` | Bigram only | Non-CJK passthrough |
| `京` | Unigram only | Single Han char |

Adjacency is determined by **original-text byte offsets**: if two CJK tokens
have contiguous or overlapping offsets (`curr.offset_from <= prev.offset_to`),
they are adjacent. This correctly handles:
- Spaces and punctuation as separators
- Zero-width characters (ZWSP, ZWNJ, etc.) removed by NFKC — do NOT break adjacency
- Variation selectors absorbed into preceding character's offset
- NFKC multi-char expansions sharing the same offset (e.g. ㍿→株式会社)

### Snippet Generation

`ICUSearchConfig::snippet` generates snippets with:
- **Dual-field fallback**: tries bigram highlights first, falls back to unigram.
- **Highlight merging**: when bigram is primary, also scans with unigram to
  highlight isolated Han chars (e.g. query "北京 我" highlights both "北京"
  and "我" in the same snippet).
- **Truncation workaround**: tantivy's `SnippetGenerator` sets the fragment
  boundary at the last token's `offset_to`. If the analyzer drops trailing
  tokens (e.g. HanOnlyFilter), the fragment is truncated. For short bodies
  (within `max_snippet_chars`), the full body is used instead.

## Limitations

### Japanese dakuten/handakuten stripped by diacritic folding

`DiacriticFoldingFilter` strips all combining marks with CCC ≠ 0. Japanese
dakuten (U+3099, CCC=8) and handakuten (U+309A, CCC=8) are combining marks,
so they are removed: で→て, ガ→カ. This means a search for "で" will match
documents containing "て" and vice versa, losing the voicing distinction.

Note: this only affects text where dakuten appears as a **separate combining
character**. Precomposed kana (e.g. U+3067 で as a single codepoint) is not
affected — NFKC Casefold normalizes to the precomposed form, which has CCC=0.
The issue arises when the input is in NFD form or when NFKC Casefold
decomposes certain compatibility forms.

### South Asian scripts (Devanagari, Bengali, Tamil, etc.)

`DiacriticFoldingFilter` strips all combining marks with canonical combining
class ≠ 0. This includes **virama** (U+094D in Devanagari, CCC=9), which is
structural — it joins consonants into clusters (क + virama + ष = क्ष "ksha").
Stripping it corrupts the text. If South Asian script support is needed,
the diacritic folding logic should be made script-aware.

### Single kana/hangul recall

A query for a single kana character (e.g. "は") returns no results: the bigram
field bigramizes it with neighbors (no standalone "は" token), and the unigram
field only keeps Han characters. This is a known trade-off of the dual-field
design.

### tantivy snippet fragment truncation

tantivy's `SnippetGenerator` determines fragment boundaries from token offsets.
When the analyzer drops tokens at the end of the text (e.g. HanOnlyFilter
drops non-Han trailing content), the fragment is truncated before the end of
the document. The `ICUSearchConfig::snippet` method works around this for
short documents by extending the fragment to the full body text.

### Highlight ranges may overlap

CJK bigrams produce overlapping highlight ranges (e.g. query "北京是" on text
"北京是..." produces ranges `[0..6, 3..9]` for bigrams "北京" and "京是").
Consumers must merge overlapping ranges before rendering. The `search` module's
`ICUSnippet` returns raw ranges; the `collapse_byte_ranges` function in the
main application handles merging.
