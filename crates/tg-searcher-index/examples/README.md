# Benchmark: jieba vs ICU indexing backends

Compares `tg-searcher-index` (jieba tokenizer, single content field) against
`tantivy-analyzer-icu` (ICU tokenizer, three-field schema: folded\_bigram +
unigram + diacritic).

## Usage

```bash
cargo run -p tg-searcher-index --example benchmark --release -- \
  --data /path/to/result.json \
  [--limit 10000] \
  [--repeat 5] \
  [--queries "hello,问题,服务器"] \
  [--compressor zstd7] \
  [--keep-dir /tmp/bench_output]
```

Options:
- `--data`: Telegram Desktop export `result.json`
- `--limit`: max messages to load from JSON
- `--repeat`: multiply dataset (for stress testing)
- `--queries`: comma-separated queries (default: built-in set)
- `--compressor`: `lz4` (default), `zstd`, `zstd7`, `zstd22`, `none`
- `--keep-dir`: persist index files for inspection

## Results

Test environment: Linux 6.18, Rust 1.87 (release profile), tantivy 0.26.
Data: 276,291 real Telegram messages (avg 14 chars), writer heap 200 MB,
force-merged to single segment after commit.

### Insertion throughput

| Compressor | jieba (msg/s) | ICU (msg/s) | ratio |
|---|---|---|---|
| none | 263,497 | 209,361 | 0.79x |
| lz4 | 253,703 | 131,477 | 0.52x |
| zstd7 | 259,841 | 184,986 | 0.71x |
| zstd22 | 168,630 | 156,499 | 0.93x |

ICU insertion is 1.1–1.9x slower due to the heavier tokenization pipeline
(NFKC Casefold + ICU word break + SemiticNorm + DiacriticFolding + CJK filters)
applied to three fields per document.

zstd7 outperforms lz4 for ICU because smaller compressed blocks reduce disk I/O.

### Index size (single segment, 276K docs)

Compression only affects `.store` (document store). The index fields
(`.idx`, `.pos`, `.term`) are identical across compressors.

| Compressor | jieba .store | ICU .store | jieba total | ICU total |
|---|---|---|---|---|
| none | 33.5 MB | 33.5 MB | 44.0 MB | 55.8 MB |
| lz4 | 15.3 MB | 15.3 MB | 25.8 MB | 37.6 MB |
| zstd7 | 12.0 MB | 12.0 MB | 22.5 MB | 34.3 MB |
| zstd22 | 11.0 MB | 11.4 MB | 21.5 MB | 33.7 MB |

Both backends store the same fields (content, url, chat\_id, post\_time,
sender), so `.store` size is identical. The ~12 MB difference is entirely from
ICU's three index fields vs jieba's one:

| File type | Purpose | jieba | ICU | delta |
|---|---|---|---|---|
| `.store` | document text | 15.3 MB | 15.3 MB | 0 |
| `.idx` | postings | 4.8 MB | 10.7 MB | +5.9 MB |
| `.pos` | positions | 2.4 MB | 5.4 MB | +3.0 MB |
| `.term` | term dictionary | 1.8 MB | 4.2 MB | +2.4 MB |
| `.fieldnorm` | field lengths | 1.1 MB | 1.7 MB | +0.6 MB |
| `.fast` | columnar/sort | 0.4 MB | 0.3 MB | -0.1 MB |
| **total** | | **25.8 MB** | **37.6 MB** | **+11.8 MB (+46%)** |

### Query latency (276K docs, lz4, median of 100 runs)

| Category | Query | jieba (ms) | ICU (ms) | Notes |
|---|---|---|---|---|
| Single Han | `的` | 0.062 | 0.058 | ICU slightly faster |
| Single Han | `人` | 0.016 | 0.022 | |
| 2-char CJK | `问题` | 0.013 | 0.015 | ~parity |
| 3-char CJK | `不知道` | 0.263 | 0.217 | ICU faster |
| 3-char CJK | `服务器` | 0.011 | 0.048 | ICU 4x slower (PhraseQuery) |
| 3-char CJK | `羽毛球` | 0.010 | 0.033 | ICU 3x slower |
| 4-char CJK | `文件系统` | 0.008 | 0.032 | ICU 4x slower |
| 5-char CJK | `信息学竞赛` | 0.010 | 0.026 | ICU 3x slower |
| 7-char CJK | `无法访问文件系统` | 0.014 | 0.032 | ICU 2x slower |
| English | `Telegram` | 0.010 | 0.013 | ~parity |
| English | `message` | 0.009 | 0.010 | ~parity |
| English 2-word | `compress sensing` | 0.019 | 0.017 | ~parity |
| Mixed | `VSCode 连接` | 0.026 | 0.035 | |
| Mixed | `ssh 服务器` | 0.031 | 0.065 | ICU 2x slower |
| No match | `xyzzyspoon` | 0.006 | 0.005 | ~parity |

ICU's `route_query` builds queries directly from tokens without
`QueryParser` (single tokenization pass). For ≥3-char CJK queries, ICU
uses `PhraseQuery` on bigram sequences which is slower than jieba's simple
`TermQuery` OR. All latencies are sub-millisecond and imperceptible in
practice.

### Optimizations applied

Two optimizations were applied to `tantivy-analyzer-icu` during this
benchmarking work:

1. **Cached ICU break iterator** (`word_break/mod.rs`): The RBBI rule
   compilation in `UBreakIterator::try_new_rules` costs ~7ms per call.
   Changed to `thread_local!` prototype + `safe_clone()` + `set_text()`
   which costs <1µs. This alone reduced query latency from ~13.5ms to
   ~0.005ms.

2. **Eliminated double tokenization in `route_query`** (`search.rs`):
   Previously `route_query` ran the ICU pipeline twice (once in
   `base_tokenize`, once in `QueryParser::parse_query`). Now it tokenizes
   once with `semitic_tokenize()`, then derives all three term sets
   (folded\_bigram, unigram, diacritic) with pure Rust string operations,
   building `TermQuery`/`PhraseQuery` directly.
