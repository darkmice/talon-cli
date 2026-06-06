//! TS 时序引擎命令处理。
//!
//! 子命令：list / info / create / insert / query / agg / retention

use crate::OutputFormat;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use talon::{AggFunc, DataPoint, TsAggQuery, TsQuery, TsSchema, Talon};

/// 处理 `:ts` 子命令。
pub fn handle(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 2 {
        report(had_error, fmt, ":ts 需要子命令。输入 :help 查看。");
        return;
    }
    match parts[1] {
        "list" => cmd_list(db, fmt, had_error),
        "info" => cmd_info(db, parts, fmt, had_error),
        "create" => cmd_create(db, parts, fmt, had_error),
        "insert" => cmd_insert(db, parts, fmt, had_error),
        "query" => cmd_query(db, parts, fmt, had_error),
        "agg" => cmd_agg(db, parts, fmt, had_error),
        "retention" => cmd_retention(db, parts, fmt, had_error),
        _ => report(
            had_error,
            fmt,
            &format!("未知 ts 子命令: {}。输入 :help 查看。", parts[1]),
        ),
    }
}

// ── list ──────────────────────────────────────────────────────────────────────

fn cmd_list(db: &Talon, fmt: OutputFormat, had_error: &AtomicBool) {
    match talon::list_timeseries(db.store()) {
        Ok(names) => {
            if fmt == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({"ok":true,"timeseries":names,"count":names.len()})
                );
            } else if names.is_empty() {
                println!("(无时序)");
            } else {
                for name in &names {
                    println!("  {}", name);
                }
                println!("({} 个时序)", names.len());
            }
        }
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

// ── info ──────────────────────────────────────────────────────────────────────

fn cmd_info(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 3 {
        report(had_error, fmt, ":ts info <name>");
        return;
    }
    match talon::describe_timeseries(db.store(), parts[2]) {
        Ok(info) => {
            if fmt == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "name": info.name,
                        "point_count": info.point_count,
                        "retention_ms": info.retention_ms,
                    })
                );
            } else {
                println!("  名称: {}", info.name);
                println!("  数据点: {}", info.point_count);
                match info.retention_ms {
                    Some(ms) => println!(
                        "  保留策略: {} ms ({:.1} 天)",
                        ms,
                        ms as f64 / 86400000.0
                    ),
                    None => println!("  保留策略: 永久"),
                }
            }
        }
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

// ── create ────────────────────────────────────────────────────────────────────
// :ts create <name> <tag1,tag2,...> <field1,field2,...>

fn cmd_create(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 5 {
        report(had_error, fmt, ":ts create <name> <tag1,tag2,...> <field1,field2,...>");
        return;
    }
    let name = parts[2];
    let tags: Vec<String> = parts[3]
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    let fields: Vec<String> = parts[4]
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    if fields.is_empty() {
        report(had_error, fmt, "至少需要一个 field 列");
        return;
    }
    let schema = TsSchema { tags: tags.clone(), fields: fields.clone() };
    match db.create_timeseries(name, schema) {
        Ok(_) => {
            if fmt == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({"ok":true,"name":name,"tags":tags,"fields":fields})
                );
            } else {
                println!("时序表 {} 已创建 (tags={:?}, fields={:?})", name, tags, fields);
            }
        }
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

// ── insert ────────────────────────────────────────────────────────────────────
// :ts insert <name> <ts_ms> <tagk=tagv,...> <fieldk=fieldv,...>

fn cmd_insert(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 6 {
        report(had_error, fmt, ":ts insert <name> <ts_ms> <tagk=tagv,...> <fieldk=fieldv,...>");
        return;
    }
    let name = parts[2];
    let timestamp: i64 = match parts[3].parse() {
        Ok(n) => n,
        Err(_) => {
            report(had_error, fmt, "ts_ms 必须为整数（毫秒时间戳）");
            return;
        }
    };
    let tags: BTreeMap<String, String> = parse_kv_pairs(parts[4]);
    let fields: BTreeMap<String, String> = parse_kv_pairs(parts[5]);
    if fields.is_empty() {
        report(had_error, fmt, "至少需要一个 field 键值对（fieldk=fieldv）");
        return;
    }
    let point = DataPoint { timestamp, tags, fields };
    match db.open_timeseries(name).and_then(|ts| ts.insert(&point)) {
        Ok(()) => {
            if fmt == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({"ok":true,"name":name,"timestamp":timestamp})
                );
            } else {
                println!("已写入数据点 ts={} 到 {}", timestamp, name);
            }
        }
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

// ── query ─────────────────────────────────────────────────────────────────────
// :ts query <name> [limit]

fn cmd_query(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 3 {
        report(had_error, fmt, ":ts query <name> [limit]");
        return;
    }
    let name = parts[2];
    let limit: Option<usize> = parts.get(3).and_then(|s| s.parse().ok());
    let q = TsQuery {
        tag_filters: vec![],
        time_start: None,
        time_end: None,
        desc: false,
        limit,
    };
    match db.open_timeseries(name).and_then(|ts| ts.query(&q)) {
        Ok(points) => {
            if fmt == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({"ok":true,"name":name,"points":points,"count":points.len()})
                );
            } else if points.is_empty() {
                println!("(无数据点)");
            } else {
                for p in &points {
                    let tags_str: Vec<String> = p.tags.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
                    let fields_str: Vec<String> = p.fields.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
                    println!(
                        "  ts={} tags=[{}] fields=[{}]",
                        p.timestamp,
                        tags_str.join(","),
                        fields_str.join(",")
                    );
                }
                println!("({} 个数据点)", points.len());
            }
        }
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

// ── agg ───────────────────────────────────────────────────────────────────────
// :ts agg <name> <field> <func> <interval_ms>
// func: sum/avg/min/max/count/first/last/stddev

fn cmd_agg(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 6 {
        report(
            had_error,
            fmt,
            ":ts agg <name> <field> <func> <interval_ms>  (func: sum/avg/min/max/count/first/last/stddev)",
        );
        return;
    }
    let name = parts[2];
    let field = parts[3].to_string();
    let func = match parse_agg_func(parts[4]) {
        Some(f) => f,
        None => {
            report(had_error, fmt, "未知聚合函数。可用: sum/avg/min/max/count/first/last/stddev");
            return;
        }
    };
    let interval_ms: Option<i64> = match parts[5].parse::<i64>() {
        Ok(n) if n > 0 => Some(n),
        Ok(_) => None, // 0 → 全局聚合
        Err(_) => {
            report(had_error, fmt, "interval_ms 必须为整数（毫秒，0 = 全局聚合）");
            return;
        }
    };
    let q = TsAggQuery {
        tag_filters: vec![],
        time_start: None,
        time_end: None,
        field,
        func,
        interval_ms,
        sliding_ms: None,
        session_gap_ms: None,
        fill: None,
    };
    match db.open_timeseries(name).and_then(|ts| ts.aggregate(&q)) {
        Ok(buckets) => {
            if fmt == OutputFormat::Json {
                let items: Vec<serde_json::Value> = buckets
                    .iter()
                    .map(|b| serde_json::json!({"bucket_start":b.bucket_start,"value":b.value,"count":b.count}))
                    .collect();
                println!(
                    "{}",
                    serde_json::json!({"ok":true,"name":name,"buckets":items,"count":items.len()})
                );
            } else if buckets.is_empty() {
                println!("(无聚合结果)");
            } else {
                for b in &buckets {
                    println!("  bucket_start={} value={:.6} count={}", b.bucket_start, b.value, b.count);
                }
                println!("({} 个桶)", buckets.len());
            }
        }
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

// ── retention ─────────────────────────────────────────────────────────────────
// :ts retention <name> <ms>  (0 = 清除保留策略)

fn cmd_retention(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 4 {
        report(had_error, fmt, ":ts retention <name> <ms>  (0 = 永久保留)");
        return;
    }
    let name = parts[2];
    let ms: u64 = match parts[3].parse() {
        Ok(n) => n,
        Err(_) => {
            report(had_error, fmt, "ms 必须为非负整数（毫秒，0 = 永久保留）");
            return;
        }
    };
    match db.open_timeseries(name).and_then(|ts| ts.set_retention(ms)) {
        Ok(()) => {
            if fmt == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({"ok":true,"name":name,"retention_ms":ms})
                );
            } else if ms == 0 {
                println!("{} 保留策略已清除（永久保留）", name);
            } else {
                println!(
                    "{} 保留策略已设为 {} ms ({:.1} 天)",
                    name,
                    ms,
                    ms as f64 / 86400000.0
                );
            }
        }
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// 解析 "k=v,k=v" 形式的键值对字符串。
fn parse_kv_pairs(s: &str) -> BTreeMap<String, String> {
    s.split(',')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// 将字符串映射为 AggFunc 枚举。
fn parse_agg_func(s: &str) -> Option<AggFunc> {
    match s {
        "sum" => Some(AggFunc::Sum),
        "avg" => Some(AggFunc::Avg),
        "min" => Some(AggFunc::Min),
        "max" => Some(AggFunc::Max),
        "count" => Some(AggFunc::Count),
        "first" => Some(AggFunc::First),
        "last" => Some(AggFunc::Last),
        "stddev" => Some(AggFunc::Stddev),
        _ => None,
    }
}

fn report(had_error: &AtomicBool, fmt: OutputFormat, msg: &str) {
    had_error.store(true, Ordering::Relaxed);
    if fmt == OutputFormat::Json {
        println!("{}", serde_json::json!({"ok":false,"error":msg}));
    } else {
        eprintln!("{}", msg);
    }
}
