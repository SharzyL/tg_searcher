# tantivy-analyzer-icu

ICU-based text analysis for [tantivy](https://github.com/quickwit-oss/tantivy),
with multilingual support for CJK bigram search, smartcase diacritic handling,
and Semitic script normalization. All token offsets map back to the original
(pre-normalization) text for correct snippet highlighting.

## Feature Flags

| Feature | Description |
|---------|-------------|
| *(none)* | Core tokenizer and filters only. No tantivy index dependency. |
| `tantivy-search` | High-level tantivy integration: `ICUSearchConfig`, three-field schema, smartcase query routing, snippet generation. |
| `demo` | Test harness with query test cases. Implies `tantivy-search`. |

## Quick Start

### As a tokenizer/filter library

Without any feature flags, the crate provides composable tokenizers and filters
that implement `tantivy-tokenizer-api` traits:

```rust
use tantivy::tokenizer::TextAnalyzer;
use tantivy_analyzer_icu::{
    NormalizingICUTokenizer, SemiticNormalizationFilter,
    DiacriticFoldingFilter, CJKBigramFilter,
};

let analyzer = TextAnalyzer::builder(NormalizingICUTokenizer)
    .filter(SemiticNormalizationFilter)
    .filter(DiacriticFoldingFilter)
    .filter(CJKBigramFilter)
    .build();
```

### With tantivy integration (`tantivy-search` feature)

The `search` module provides `ICUSearchConfig` which handles the three-field
schema, analyzer registration, smartcase query routing, and snippet generation:

```ignore
use tantivy::schema::Schema;
use tantivy_analyzer_icu::search::ICUSearchConfig;

let config = ICUSearchConfig::default();

// 1. Schema: add a field group (creates stored + folded_bigram + unigram + diacritic fields)
let mut builder = Schema::builder();
let content = config.add_field_group(&mut builder, "content");
let schema = builder.build();

// 2. Register analyzers on the index
let index = tantivy::Index::create_in_ram(schema);
config.register_analyzers(&index);

// 3. Index documents (all four fields get the same text)
writer.add_document(doc!(
    content.stored => text,
    content.folded_bigram => text,
    content.unigram => text,
    content.diacritic => text,
))?;

// 4. Query routing (smartcase: café→diacritic, cafe→folded_bigram, 北京→bigram)
let query = config.route_query(&index, &content, "café 北京 我")?;

// 5. Snippet generation with three-field highlight merging
let snippet = config.snippet(&searcher, &query, &content, &body);
```

## Running the Demo

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
SemiticNormalizationFilter
  Arabic: harakat stripping, alif variants→alif, ta marbuta→ha,
  alif maqsura→ya, Farsi variants→Arabic, tatweel/hamza removal,
  Arabic-Indic/Persian digits→ASCII
  Hebrew: niqqud (vowel points, cantillation) stripping
  │
  ├──────────────────────────────┬────────────────────────┐
  ▼                              ▼                        ▼
DiacriticFoldingFilter       DiacriticOnlyFilter       HanOnlyFilter
  NFD → strip foldable Mn     (keep only tokens         (keep only single
  → NFC                        with foldable Mn,         Han characters)
  café→cafe, über→uber          in original form)            │
  で stays で (not foldable)        │                        ▼
  क्ष stays क्ष (not foldable)       ▼                    unigram field
        │                     diacritic field
        ▼
CJKBigramFilter
        │
        ▼
  folded_bigram field
```

"Foldable" means combining marks in U+0300–036F, U+1AB0–1AFF, U+1DC0–1DFF
(Latin/Greek/Cyrillic/Vietnamese/IPA accents). Excluded: Japanese dakuten
(U+3099/309A), Devanagari virama (U+094D), Arabic harakat, Hebrew niqqud.

### Worked Example

Input: `㋿Ξ㍾㍿の下北沢\u{E0100}店でnaïveなThé Noirとphởとكَبَابを注文、שָׁלוֹםとनमस्तेで先輩に挨拶した。8月10日、二 人 幸 终。`

(`\u{E0100}` is an ideographic variation selector, invisible in rendered text)

```text
Step 1 — NFKC Casefold:
  ㋿→令和  Ξ→ξ  ㍾→明治  ㍿→株式会社          compatibility decomposition
  沢\u{E0100} → 沢                              IVS absorbed into 沢
  T→t N→n (Thé→thé, Noir→noir)                   casefold

Step 2 — ICU word segmentation (UAX #29):
  CJK/kana sequences and Latin words are split into separate segments.
  Punctuation (、。) removed. Spaces are separators, not tokens.

Step 3 — CJK unigram expansion:
  Each Han/Kana character in a segment becomes its own token.
  Non-CJK tokens (naïve, thé, noir, phở, שָׁלוֹם, नमस्ते, كَبَاب) stay intact.
  Numbers (8, 10) are separate non-CJK tokens.

  [令] [和] [ξ] [明] [治] [株] [式] [会] [社] [の] [下] [北] [沢] [店] [で]
  [naïve] [な] [thé] [noir] [と] [phở] [と] [كَبَاب] [を] [注] [文]
  [שָׁלוֹם] [と] [नमस्ते] [で] [先] [輩] [に] [挨] [拶] [し] [た]
  [8] [月] [10] [日] | [二] | [人] | [幸] | [终]
  (| = offset gap from 、or space in original text)

Step 4 — SemiticNormalizationFilter:
  [كَبَاب]→[كباب]  harakat stripped
  [שָׁלוֹם]→[שלום]  niqqud stripped
  [नमस्ते] unchanged — virama (U+094D) is NOT stripped
  (other tokens unchanged)

Step 5 — DiacriticFoldingFilter (folded_bigram / unigram paths only):
  [naïve]→[naive]  [thé]→[the]  [phở]→[pho]
  [noir] unchanged — no foldable diacritics
  [で] stays [で]    dakuten (U+3099) is NOT foldable
  [नमस्ते] stays [नमस्ते]  virama (U+094D) is NOT foldable
  (other tokens unchanged)

Step 6 — terminal filters (three parallel paths):

→ folded_bigram (CJKBigramFilter):
  [令和] [ξ] [明治] [治株] [株式] [式会] [会社] [社の] [の下] [下北] [北沢] [沢店] [店で]
  [naive] [な] [the] [noir] [と] [pho] [と] [كباب] [を注] [注文]
  [שלום] [と] [नमस्ते] [で先] [先輩] [輩に] [に挨] [挨拶] [拶し] [した]
  [8] [10]
  (月, 日 dropped — isolated Han between numbers;
   二, 人, 幸, 终 dropped — isolated Han separated by spaces)

→ unigram (HanOnlyFilter):
  [令] [和] [明] [治] [株] [式] [会] [社] [下] [北] [沢] [店]
  [注] [文] [先] [輩] [挨] [拶] [月] [日] [二] [人] [幸] [终]

→ diacritic (DiacriticOnlyFilter, pre-fold form):
  [naïve] [thé] [phở]
```

Things to note:
- **㋿Ξ㍾㍿**: NFKC expands compatibility chars (㋿→令和, ㍾→明治, ㍿→株式会社).
  Cross-expansion bigrams form naturally (治株) because original-text offsets are contiguous.
- **下北沢\u{E0100}店**: IVS (U+E0100) is removed by NFKC. The offset is absorbed into 沢's
  range, so 北沢 still forms a bigram.
- **naïve, thé, phở**: Diacritics folded in folded\_bigram (naive, the, pho);
  preserved in diacritic field for smartcase exact matching.
  **noir** has no foldable diacritics — passes through unchanged, absent from diacritic field.
- **で stays で**: Dakuten (U+3099) is NOT in the foldable range, so it is preserved.
- **नमस्ते stays नमस्ते**: Devanagari virama (U+094D) is NOT foldable. The conjunct
  स्त (sa + virama + ta) is preserved intact.
- **שָׁלוֹם→שלום**: Hebrew niqqud stripped by SemiticNormalizationFilter.
- **كَبَاب→كباب**: Arabic harakat (fatha) stripped by SemiticNormalizationFilter.
- **8月10日**: Numbers `8` and `10` are non-CJK tokens. 月 and 日 are Han chars
  isolated between them — dropped from folded\_bigram, kept in unigram.
- **、二 人 幸 终。**: Punctuation (、。) is stripped. Spaces between 二, 人, 幸, 终
  break offset adjacency — no bigrams form. These isolated Han chars are dropped
  from folded\_bigram (covered by unigram field). Compare with 注文 where the two
  chars are adjacent and produce bigram \[注文\].

### Three-Field Indexing

Each text source is indexed into three fields:

- **folded_bigram**: Primary recall field. Diacritics folded, CJK characters produce
  overlapping bigrams (北京→"北京", 京天→"京天"). Isolated Han characters are dropped
  (covered by unigram). Non-CJK tokens pass through in folded form.

- **unigram**: Only single Han characters are kept. Covers single-character
  queries that the bigram field misses.

- **diacritic**: Sparse precision field. Only tokens whose NFD form contains a
  foldable diacritic mark are kept, in their original (pre-fold) form. CJK, Arabic,
  Hebrew, and unaccented tokens produce nothing here.

### Query Routing (Smartcase)

`ICUSearchConfig::route_query` analyzes the query to decide routing.
The "Matches?" column shows results against the worked example above as the
sole indexed document.

| Query | Route | Matches? | Rationale |
|-------|-------|:-:|-----------|
| `下北沢` | folded_bigram | yes | Adjacent CJK → bigrams 下北, 北沢 |
| `二人` | folded_bigram | **no** | Bigram 二人 not in index (二 and 人 are space-separated in doc) |
| `二 人` | unigram | yes | Space in query → each char isolated → unigram lookup |
| `注文 幸` | folded_bigram + unigram | yes | 注文 adjacent → bigram; 幸 isolated → unigram |
| `noir` | folded_bigram | yes | Non-CJK, no diacritics → folded_bigram passthrough |
| `nöir` | folded_bigram + diacritic | yes | nöir folds to noir in folded_bigram (match); diacritic field has no nöir (no boost) |
| `the` | folded_bigram | yes | No diacritics → broad match (matches both thé and the) |
| `thé` | folded_bigram + diacritic | yes | Diacritic field boosts exact accent match |
| `pho` | folded_bigram | yes | phở folded to pho in folded_bigram |
| `phở` | folded_bigram + diacritic | yes | Diacritic field boosts exact match for phở |
| `naïve 下北沢` | folded_bigram + diacritic | yes | Per-token: naïve→diacritic, 下北沢→folded_bigram |
| `月` | unigram | yes | Single Han char → unigram only |
| `8月` | folded_bigram + unigram | yes | 8 is non-CJK, 月 is Han → no bigram; 月 falls back to unigram |

The smartcase decision: if `raw_query.nfd()` contains any foldable diacritic mark,
the diacritic field is also queried (boosted 3x vs folded_bigram's 2x). Tokens
without diacritics in a diacritic-bearing query still go to folded_bigram.

### Snippet Generation

`ICUSearchConfig::snippet` generates snippets with:
- **Three-field fallback**: tries folded_bigram highlights first, falls back to
  unigram, then diacritic.
- **Highlight merging**: when folded_bigram is primary, also scans with unigram
  and diacritic to merge additional highlights.
- **Truncation workaround**: tantivy's `SnippetGenerator` sets the fragment
  boundary at the last token's `offset_to`. If the analyzer drops trailing
  tokens (e.g. HanOnlyFilter), the fragment is truncated. For short bodies
  (within `max_snippet_chars`), the full body is used instead.

## Limitations

### Single kana/hangul recall

A query for a single kana character (e.g. "は") returns no results: the bigram
field bigramizes it with neighbors (no standalone "は" token), and the unigram
field only keeps Han characters. This is a known trade-off of the three-field
design.

### Cannot express "match `cafe` but not `café`"

Queries without diacritics always route to folded_bigram, which unifies accented
and unaccented forms. There is no facility for "match the unaccented form exactly,
excluding accented variants."

### Smartcase diacritic matching is asymmetric by design

Typing without diacritics gives broad matching: `cafe` matches both `cafe` and
`café`, `uber` matches `über`, `pho` matches `phở`/`phố`/`phồ`/etc. Typing with
diacritics gives exact matching: `café` matches only `café`, `über` matches only
`über`. There is no middle ground (e.g. "prefer `café` but also include `cafe`
as fallback").

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
