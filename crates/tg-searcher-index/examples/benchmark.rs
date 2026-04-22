//! Benchmark comparing jieba (tg-searcher-index) vs ICU (tantivy-analyzer-icu)
//! indexing backends using real Telegram Desktop export data.
//!
//! Usage:
//!   cargo run -p tg-searcher-index --example benchmark --release -- \
//!     --data /path/to/result.json [--limit 1000] [--repeat 10] [--queries "q1,q2"]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use clap::Parser;
use serde::Deserialize;
use tantivy::collector::Count;
use tantivy::schema::*;
use tantivy::store::Compressor;
use tantivy::{Index, IndexSettings, IndexWriter, ReloadPolicy, doc};
use tantivy_analyzer_icu::search::ICUSearchConfig;

// ── CLI ─────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(about = "Benchmark jieba vs ICU indexing backends")]
struct Args {
    /// Path to Telegram Desktop export result.json
    #[arg(long)]
    data: PathBuf,

    /// Maximum number of messages to load from JSON (default: all)
    #[arg(long)]
    limit: Option<usize>,

    /// Repeat the loaded messages N times to create a larger dataset
    #[arg(long, default_value_t = 1)]
    repeat: usize,

    /// Comma-separated query list (default: built-in set)
    #[arg(long)]
    queries: Option<String>,

    /// Directory to persist index files for inspection (default: use tempdir)
    #[arg(long)]
    keep_dir: Option<PathBuf>,

    /// Doc store compressor: "lz4", "zstd" (default level), "zstd7", "zstd15", "zstd22", "none"
    #[arg(long, default_value = "lz4")]
    compressor: String,
}

// ── JSON parsing ────────────────────────────────────────────────────

#[derive(Deserialize)]
struct TelegramExport {
    messages: Vec<RawMessage>,
}

#[derive(Deserialize)]
struct RawMessage {
    id: i64,
    #[serde(rename = "type")]
    msg_type: String,
    #[serde(default)]
    date_unixtime: Option<String>,
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    text: Option<TextField>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum TextField {
    Plain(String),
    Rich(Vec<RichTextPart>),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RichTextPart {
    Plain(String),
    Entity { text: String },
}

struct BenchMessage {
    id: i64,
    text: String,
    post_time: DateTime<Utc>,
    sender: String,
}

fn extract_text(field: &TextField) -> String {
    match field {
        TextField::Plain(s) => s.clone(),
        TextField::Rich(parts) => parts
            .iter()
            .map(|p| match p {
                RichTextPart::Plain(s) => s.as_str(),
                RichTextPart::Entity { text } => text.as_str(),
            })
            .collect(),
    }
}

fn load_messages(path: &Path, limit: Option<usize>, repeat: usize) -> Vec<BenchMessage> {
    let data = std::fs::read_to_string(path).expect("failed to read JSON file");
    let export: TelegramExport = serde_json::from_str(&data).expect("failed to parse JSON");

    let mut base: Vec<BenchMessage> = export
        .messages
        .iter()
        .filter(|m| m.msg_type == "message")
        .filter_map(|m| {
            let text_field = m.text.as_ref()?;
            let text = extract_text(text_field);
            if text.is_empty() {
                return None;
            }
            let ts: i64 = m.date_unixtime.as_deref()?.parse().ok()?;
            let post_time = DateTime::from_timestamp(ts, 0)?;
            Some(BenchMessage {
                id: m.id,
                text,
                post_time,
                sender: m.from.clone().unwrap_or_default(),
            })
        })
        .collect();

    if let Some(n) = limit {
        base.truncate(n);
    }

    if repeat <= 1 {
        return base;
    }

    // Multiply: each copy gets a unique round prefix in the URL to avoid collisions
    let mut msgs = Vec::with_capacity(base.len() * repeat);
    for round in 0..repeat {
        for (i, m) in base.iter().enumerate() {
            msgs.push(BenchMessage {
                id: (round * base.len() + i) as i64,
                text: m.text.clone(),
                post_time: m.post_time,
                sender: m.sender.clone(),
            });
        }
    }
    msgs
}

// ── Helpers ─────────────────────────────────────────────────────────

fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let meta = entry.metadata().unwrap();
            if meta.is_file() {
                total += meta.len();
            } else if meta.is_dir() {
                total += dir_size(&entry.path());
            }
        }
    }
    total
}

fn parse_compressor(s: &str) -> Compressor {
    match s {
        "none" => Compressor::None,
        "lz4" => Compressor::Lz4,
        "zstd" => Compressor::Zstd(tantivy::store::ZstdCompressor::default()),
        s if s.starts_with("zstd") => {
            let level: i32 = s[4..].parse().expect("invalid zstd level, use e.g. zstd7");
            Compressor::Zstd(tantivy::store::ZstdCompressor {
                compression_level: Some(level),
            })
        }
        _ => panic!("unknown compressor: {s}, use lz4/zstd/zstdN/none"),
    }
}

/// Force-merge all segments into one, then commit.
fn force_merge_single(writer: &mut IndexWriter, index: &Index) {
    let seg_ids: Vec<_> = index
        .searchable_segment_ids()
        .expect("failed to get segment ids");
    if seg_ids.len() > 1 {
        let _ = writer.merge(&seg_ids).wait();
        writer.commit().unwrap();
    }
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1_000_000 {
        format!("{:.1} MB", bytes as f64 / 1_000_000.0)
    } else if bytes >= 1_000 {
        format!("{:.1} KB", bytes as f64 / 1_000.0)
    } else {
        format!("{} B", bytes)
    }
}

fn median(sorted: &[Duration]) -> Duration {
    let n = sorted.len();
    if n == 0 {
        return Duration::ZERO;
    }
    if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2
    }
}

const DEFAULT_QUERIES: &[&str] = &[
    // Single Han
    "的",
    "人",
    // 2-char CJK
    "问题",
    // 3-char CJK
    "不知道",
    "服务器",
    "羽毛球",
    // 4-char CJK
    "文件系统",
    "登录服务器",
    // 5+ char CJK
    "信息学竞赛",
    "无法访问文件系统",
    "账号分配",
    // English short / long
    "hello",
    "Telegram",
    "message",
    "update",
    "compress sensing",
    "verification code",
    // Mixed CJK + Latin
    "VSCode 连接",
    "ssh 服务器",
    // No match
    "xyzzyspoon",
];

// ── Jieba backend ───────────────────────────────────────────────────

struct BenchResult {
    insert_time: Duration,
    index_size: u64,
    num_docs: usize,
    query_results: Vec<QueryResult>,
}

struct QueryResult {
    query: String,
    median_latency: Duration,
    hit_count: usize,
}

const QUERY_ITERATIONS: usize = 100;

fn bench_jieba(
    msgs: &[BenchMessage],
    queries: &[&str],
    keep_dir: Option<&Path>,
    settings: IndexSettings,
) -> BenchResult {
    let tmp = tempfile::tempdir().unwrap();
    let index_dir = keep_dir
        .map(|d| d.join("jieba"))
        .unwrap_or_else(|| tmp.path().join("jieba"));
    std::fs::create_dir_all(&index_dir).unwrap();

    let mut schema_builder = Schema::builder();
    let text_options = TextOptions::default()
        .set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("jieba")
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        )
        .set_stored();
    let content_field = schema_builder.add_text_field("content", text_options);
    let url_field = schema_builder.add_text_field("url", STRING | STORED);
    let chat_id_field = schema_builder.add_i64_field("chat_id", INDEXED | STORED);
    let post_time_field = schema_builder.add_date_field("post_time", STORED | FAST);
    let sender_field = schema_builder.add_text_field("sender", STORED);
    let schema = schema_builder.build();

    let start = Instant::now();
    let index = Index::builder()
        .schema(schema)
        .settings(settings)
        .create_in_dir(&index_dir)
        .unwrap();
    index
        .tokenizers()
        .register("jieba", tg_searcher_index::ChineseTokenizer::new());

    let mut writer = index.writer(200_000_000).unwrap();

    let num_docs = msgs.len();
    for m in msgs {
        let url = format!("https://t.me/c/bench/{}", m.id);
        let post_time = tantivy::DateTime::from_timestamp_secs(m.post_time.timestamp());
        writer
            .add_document(doc!(
                content_field => m.text.as_str(),
                url_field => url,
                chat_id_field => 1i64,
                post_time_field => post_time,
                sender_field => m.sender.as_str(),
            ))
            .unwrap();
    }
    writer.commit().unwrap();
    let insert_time = start.elapsed();
    force_merge_single(&mut writer, &index);

    let index_size = dir_size(&index_dir);

    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::Manual)
        .try_into()
        .unwrap();
    let searcher = reader.searcher();

    let mut query_results = Vec::new();
    for &q in queries {
        let mut latencies = Vec::new();
        let mut hit_count = 0;
        for _ in 0..QUERY_ITERATIONS {
            let start = Instant::now();
            let query_parser = tantivy::query::QueryParser::for_index(&index, vec![content_field]);
            let query = query_parser.parse_query(q).unwrap();
            let count = searcher.search(&*query, &Count).unwrap();
            latencies.push(start.elapsed());
            hit_count = count;
        }
        latencies.sort();
        query_results.push(QueryResult {
            query: q.to_string(),
            median_latency: median(&latencies),
            hit_count,
        });
    }

    BenchResult {
        insert_time,
        index_size,
        num_docs,
        query_results,
    }
}

// ── ICU backend ─────────────────────────────────────────────────────

fn bench_icu(
    msgs: &[BenchMessage],
    queries: &[&str],
    keep_dir: Option<&Path>,
    settings: IndexSettings,
) -> BenchResult {
    let tmp = tempfile::tempdir().unwrap();
    let index_dir = keep_dir
        .map(|d| d.join("icu"))
        .unwrap_or_else(|| tmp.path().join("icu"));
    std::fs::create_dir_all(&index_dir).unwrap();

    let icu = ICUSearchConfig::default();

    let mut schema_builder = Schema::builder();
    let content = icu.add_field_group(&mut schema_builder, "content");
    let url_field = schema_builder.add_text_field("url", STRING | STORED);
    let chat_id_field = schema_builder.add_i64_field("chat_id", INDEXED | STORED);
    let post_time_field = schema_builder.add_date_field("post_time", STORED | FAST);
    let sender_field = schema_builder.add_text_field("sender", STORED);
    let schema = schema_builder.build();

    let start = Instant::now();
    let index = Index::builder()
        .schema(schema)
        .settings(settings)
        .create_in_dir(&index_dir)
        .unwrap();
    icu.register_analyzers(&index);

    let mut writer = index.writer(200_000_000).unwrap();

    let num_docs = msgs.len();
    for m in msgs {
        let url = format!("https://t.me/c/bench/{}", m.id);
        let post_time = tantivy::DateTime::from_timestamp_secs(m.post_time.timestamp());
        writer
            .add_document(doc!(
                content.stored => m.text.as_str(),
                content.folded_bigram => m.text.as_str(),
                content.unigram => m.text.as_str(),
                content.diacritic => m.text.as_str(),
                url_field => url,
                chat_id_field => 1i64,
                post_time_field => post_time,
                sender_field => m.sender.as_str(),
            ))
            .unwrap();
    }
    writer.commit().unwrap();
    let insert_time = start.elapsed();
    force_merge_single(&mut writer, &index);

    let index_size = dir_size(&index_dir);

    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::Manual)
        .try_into()
        .unwrap();
    let searcher = reader.searcher();

    let mut query_results = Vec::new();
    for &q in queries {
        let mut latencies = Vec::new();
        let mut hit_count = 0;
        for _ in 0..QUERY_ITERATIONS {
            let start = Instant::now();
            let query = icu.route_query(&index, &content, q).unwrap();
            let count = searcher.search(&*query, &Count).unwrap();
            latencies.push(start.elapsed());
            hit_count = count;
        }
        latencies.sort();
        query_results.push(QueryResult {
            query: q.to_string(),
            median_latency: median(&latencies),
            hit_count,
        });
    }

    BenchResult {
        insert_time,
        index_size,
        num_docs,
        query_results,
    }
}

// ── Main ────────────────────────────────────────────────────────────

fn main() {
    let args = Args::parse();

    let queries: Vec<&str> = if let Some(ref qs) = args.queries {
        qs.split(',').map(|s| s.trim()).collect()
    } else {
        DEFAULT_QUERIES.to_vec()
    };

    let msgs = load_messages(&args.data, args.limit, args.repeat);
    let limit_str = args.limit.map_or("all".to_string(), |n| format!("{}", n));
    println!("=== Data ===");
    println!(
        "Messages: {} (limit: {}, repeat: {}x)",
        msgs.len(),
        limit_str,
        args.repeat
    );
    println!();

    let compressor = parse_compressor(&args.compressor);
    let settings = IndexSettings {
        docstore_compression: compressor,
        ..Default::default()
    };
    println!("Compressor: {}\n", args.compressor);

    let keep = args.keep_dir.as_deref();
    if let Some(d) = keep {
        let _ = std::fs::remove_dir_all(d.join("jieba"));
        let _ = std::fs::remove_dir_all(d.join("icu"));
        std::fs::create_dir_all(d).unwrap();
    }

    println!("Running jieba benchmark...");
    let jieba = bench_jieba(&msgs, &queries, keep, settings.clone());

    println!("Running ICU benchmark...");
    let icu = bench_icu(&msgs, &queries, keep, settings);

    // Print results
    println!();
    println!("=== Insertion ===");
    println!("{:<14} {:<14} {:<14}", "", "jieba", "icu");
    println!(
        "{:<14} {:<14} {:<14}",
        "Time",
        format!("{:.3}s", jieba.insert_time.as_secs_f64()),
        format!("{:.3}s", icu.insert_time.as_secs_f64()),
    );
    println!(
        "{:<14} {:<14} {:<14}",
        "Throughput",
        format!(
            "{:.0} msg/s",
            jieba.num_docs as f64 / jieba.insert_time.as_secs_f64()
        ),
        format!(
            "{:.0} msg/s",
            icu.num_docs as f64 / icu.insert_time.as_secs_f64()
        ),
    );
    println!(
        "{:<14} {:<14} {:<14}",
        "Size",
        format_size(jieba.index_size),
        format_size(icu.index_size),
    );

    println!();
    println!("=== Queries (median of {} runs) ===", QUERY_ITERATIONS);
    println!(
        "{:<20} {:>12} {:>12} {:>12} {:>12}",
        "Query", "jieba (ms)", "icu (ms)", "jieba hits", "icu hits"
    );
    println!("{}", "-".repeat(68));
    for (jq, iq) in jieba.query_results.iter().zip(icu.query_results.iter()) {
        println!(
            "{:<20} {:>12.3} {:>12.3} {:>12} {:>12}",
            format!("\"{}\"", jq.query),
            jq.median_latency.as_secs_f64() * 1000.0,
            iq.median_latency.as_secs_f64() * 1000.0,
            jq.hit_count,
            iq.hit_count,
        );
    }
}
