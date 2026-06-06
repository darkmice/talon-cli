//! FTS 全文搜索引擎命令处理。

use crate::cmdline::parse_props_json;
use crate::OutputFormat;
use std::sync::atomic::{AtomicBool, Ordering};
use talon::{FtsConfig, FtsDoc, Talon};

/// 处理 `:fts` 子命令。
pub fn handle(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 2 {
        report(had_error, fmt, ":fts 需要子命令。输入 :help 查看。");
        return;
    }
    match parts[1] {
        "create"  => cmd_create(db, parts, fmt, had_error),
        "drop"    => cmd_drop(db, parts, fmt, had_error),
        "index"   => cmd_index(db, parts, fmt, had_error),
        "get"     => cmd_get(db, parts, fmt, had_error),
        "del"     => cmd_del(db, parts, fmt, had_error),
        "reindex" => cmd_reindex(db, parts, fmt, had_error),
        "search"  => cmd_search(db, parts, fmt, had_error),
        _ => report(
            had_error,
            fmt,
            &format!("未知 FTS 子命令: {}", parts[1]),
        ),
    }
}

// ─── create ──────────────────────────────────────────────────────────────────

fn cmd_create(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 3 {
        report(had_error, fmt, ":fts create <name>");
        return;
    }
    let name = parts[2];
    match db.fts() {
        Ok(fts) => match fts.create_index(name, &FtsConfig::default()) {
            Ok(()) => {
                if fmt == OutputFormat::Json {
                    println!("{}", serde_json::json!({"ok":true,"index":name,"created":true}));
                } else {
                    println!("已创建索引: {}", name);
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

// ─── drop ─────────────────────────────────────────────────────────────────────

fn cmd_drop(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 3 {
        report(had_error, fmt, ":fts drop <name>");
        return;
    }
    let name = parts[2];
    match db.fts() {
        Ok(fts) => match fts.drop_index(name) {
            Ok(()) => {
                if fmt == OutputFormat::Json {
                    println!("{}", serde_json::json!({"ok":true,"index":name,"dropped":true}));
                } else {
                    println!("已删除索引: {}", name);
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

// ─── index ────────────────────────────────────────────────────────────────────

fn cmd_index(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    // :fts index <name> <doc_id> <fields_json>
    if parts.len() < 5 {
        report(had_error, fmt, ":fts index <name> <doc_id> <fields_json>");
        return;
    }
    let name   = parts[2];
    let doc_id = parts[3];
    let fields = match parse_props_json(parts[4]) {
        Ok(m) => m,
        Err(e) => {
            report(had_error, fmt, &format!("fields_json 解析失败: {}", e));
            return;
        }
    };
    let doc = FtsDoc { doc_id: doc_id.to_string(), fields };
    match db.fts() {
        Ok(fts) => match fts.index_doc(name, &doc) {
            Ok(()) => {
                if fmt == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"index":name,"doc_id":doc_id,"indexed":true})
                    );
                } else {
                    println!("已写入文档: index={} doc_id={}", name, doc_id);
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

// ─── get ──────────────────────────────────────────────────────────────────────

fn cmd_get(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    // :fts get <name> <doc_id>
    if parts.len() < 4 {
        report(had_error, fmt, ":fts get <name> <doc_id>");
        return;
    }
    let name   = parts[2];
    let doc_id = parts[3];
    match db.fts_read() {
        Ok(fts) => match fts.get_doc(name, doc_id) {
            Ok(Some(fields)) => {
                if fmt == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"index":name,"doc_id":doc_id,"found":true,"fields":fields})
                    );
                } else {
                    println!("index={} doc_id={}", name, doc_id);
                    for (k, v) in &fields {
                        println!("  {}={}", k, v);
                    }
                }
            }
            Ok(None) => {
                if fmt == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"index":name,"doc_id":doc_id,"found":false})
                    );
                } else {
                    println!("(未找到文档 {} / {})", name, doc_id);
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

// ─── del ──────────────────────────────────────────────────────────────────────

fn cmd_del(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    // :fts del <name> <doc_id>
    if parts.len() < 4 {
        report(had_error, fmt, ":fts del <name> <doc_id>");
        return;
    }
    let name   = parts[2];
    let doc_id = parts[3];
    match db.fts() {
        Ok(fts) => match fts.delete_doc(name, doc_id) {
            Ok(deleted) => {
                if fmt == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"index":name,"doc_id":doc_id,"deleted":deleted})
                    );
                } else if deleted {
                    println!("已删除文档: index={} doc_id={}", name, doc_id);
                } else {
                    println!("(文档不存在: index={} doc_id={})", name, doc_id);
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

// ─── reindex ──────────────────────────────────────────────────────────────────

fn cmd_reindex(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    // :fts reindex <name>
    if parts.len() < 3 {
        report(had_error, fmt, ":fts reindex <name>");
        return;
    }
    let name = parts[2];
    // reindex 是管理操作，admin.rs 实现在 FtsEngine 上，两个句柄均可调用；
    // 与 ffi_exec 保持一致，使用 fts_read() 获取句柄。
    match db.fts_read() {
        Ok(fts) => match fts.reindex(name) {
            Ok(count) => {
                if fmt == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"index":name,"reindexed":count})
                    );
                } else {
                    println!("重建索引完成: {} ({} 文档)", name, count);
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

// ─── search ───────────────────────────────────────────────────────────────────

fn cmd_search(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 4 {
        report(had_error, fmt, ":fts search <index_name> <query>");
        return;
    }
    match db.fts_read() {
        Ok(fts) => match fts.search(parts[2], parts[3], 10) {
            Ok(hits) => {
                if fmt == OutputFormat::Json {
                    let results: Vec<serde_json::Value> = hits
                        .iter()
                        .map(|hit| {
                            serde_json::json!({
                                "doc_id": hit.doc_id,
                                "score": hit.score,
                                "fields": hit.fields,
                            })
                        })
                        .collect();
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"results":results,"count":results.len()})
                    );
                } else if hits.is_empty() {
                    println!("(无结果)");
                } else {
                    for hit in &hits {
                        let fields_str: String = hit
                            .fields
                            .iter()
                            .map(|(k, v)| {
                                let display_v = if v.len() > 80 {
                                    format!("{}...", &v[..80])
                                } else {
                                    v.clone()
                                };
                                format!("{}={}", k, display_v)
                            })
                            .collect::<Vec<_>>()
                            .join(", ");
                        println!(
                            "  [score={:.4}] id={} | {}",
                            hit.score, hit.doc_id, fields_str
                        );
                    }
                    println!("({} 条结果)", hits.len());
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

// ─── 公共报错 ─────────────────────────────────────────────────────────────────

fn report(had_error: &AtomicBool, fmt: OutputFormat, msg: &str) {
    had_error.store(true, Ordering::Relaxed);
    if fmt == OutputFormat::Json {
        println!("{}", serde_json::json!({"ok":false,"error":msg}));
    } else {
        eprintln!("{}", msg);
    }
}
