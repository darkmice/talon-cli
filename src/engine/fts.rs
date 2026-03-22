//! FTS 全文搜索引擎命令处理。

use crate::OutputFormat;
use std::sync::atomic::{AtomicBool, Ordering};
use talon::Talon;

/// 处理 `:fts` 子命令。
pub fn handle(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 2 {
        report(had_error, fmt, ":fts 需要子命令。输入 :help 查看。");
        return;
    }
    match parts[1] {
        "search" => {
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
        _ => report(
            had_error,
            fmt,
            &format!("未知 FTS 子命令: {}", parts[1]),
        ),
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
