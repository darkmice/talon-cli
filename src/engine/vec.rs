//! Vector 向量引擎命令处理。

use crate::OutputFormat;
use std::sync::atomic::{AtomicBool, Ordering};
use talon::Talon;

/// 处理 `:vec` 子命令。
pub fn handle(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 2 {
        report(had_error, fmt, ":vec 需要子命令。输入 :help 查看。");
        return;
    }
    match parts[1] {
        "count"   => cmd_count(db, parts, fmt, had_error),
        "insert"  => cmd_insert(db, parts, fmt, had_error),
        "search"  => cmd_search(db, parts, fmt, had_error),
        "get"     => cmd_get(db, parts, fmt, had_error),
        "delete"  => cmd_delete(db, parts, fmt, had_error),
        "rebuild" => cmd_rebuild(db, parts, fmt, had_error),
        _ => report(
            had_error,
            fmt,
            &format!("未知 vec 子命令: {}", parts[1]),
        ),
    }
}

// ── 工具函数 ──────────────────────────────────────────────────────────────────

/// 解析逗号分隔的 f32 向量字符串。
fn parse_vec(s: &str) -> Vec<f32> {
    s.split(',')
        .filter_map(|t| t.trim().parse::<f32>().ok())
        .collect()
}

// ── 子命令实现 ────────────────────────────────────────────────────────────────

fn cmd_count(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 3 {
        report(had_error, fmt, ":vec count <name>");
        return;
    }
    match db.vector_read(parts[2]) {
        Ok(ve) => match ve.count() {
            Ok(n) => {
                if fmt == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"name":parts[2],"count":n})
                    );
                } else {
                    println!("{}", n);
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

fn cmd_insert(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    // :vec insert <name> <id> <v1,v2,...>
    if parts.len() < 5 {
        report(had_error, fmt, ":vec insert <name> <id> <v1,v2,v3,...>");
        return;
    }
    let id: u64 = match parts[3].parse() {
        Ok(n) => n,
        Err(_) => {
            report(had_error, fmt, "id 必须为整数（u64）");
            return;
        }
    };
    let vec = parse_vec(parts[4]);
    if vec.is_empty() {
        report(had_error, fmt, "向量不能为空，请提供逗号分隔的 f32 值");
        return;
    }
    match db.vector(parts[2]) {
        Ok(ve) => match ve.insert(id, &vec) {
            Ok(()) => {
                if fmt == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"name":parts[2],"id":id,"dim":vec.len()})
                    );
                } else {
                    println!("已插入 id={} dim={} 到 {}", id, vec.len(), parts[2]);
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

fn cmd_search(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    // :vec search <name> <k> <v1,v2,...>
    if parts.len() < 5 {
        report(had_error, fmt, ":vec search <name> <k> <v1,v2,v3,...>");
        return;
    }
    let k: usize = match parts[3].parse() {
        Ok(n) if n > 0 => n,
        _ => {
            report(had_error, fmt, "k 必须为正整数");
            return;
        }
    };
    let vec = parse_vec(parts[4]);
    if vec.is_empty() {
        report(had_error, fmt, "查询向量不能为空，请提供逗号分隔的 f32 值");
        return;
    }
    match db.vector_read(parts[2]) {
        Ok(ve) => match ve.search(&vec, k, "cosine") {
            Ok(results) => {
                if fmt == OutputFormat::Json {
                    let items: Vec<serde_json::Value> = results
                        .iter()
                        .map(|(rid, dist)| serde_json::json!({"id": rid, "distance": dist}))
                        .collect();
                    let count = items.len();
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"results":items,"count":count})
                    );
                } else {
                    for (rid, dist) in &results {
                        println!("  id={} distance={:.6}", rid, dist);
                    }
                    println!("({} 个结果)", results.len());
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

fn cmd_get(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    // :vec get <name> <id>
    if parts.len() < 4 {
        report(had_error, fmt, ":vec get <name> <id>");
        return;
    }
    let id: u64 = match parts[3].parse() {
        Ok(n) => n,
        Err(_) => {
            report(had_error, fmt, "id 必须为整数（u64）");
            return;
        }
    };
    match db.vector_read(parts[2]) {
        Ok(ve) => match ve.get_vector(id) {
            Ok(Some(v)) => {
                if fmt == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"name":parts[2],"id":id,"vector":v,"dim":v.len()})
                    );
                } else {
                    let s: Vec<String> = v.iter().map(|f| format!("{:.6}", f)).collect();
                    println!("id={} dim={} [{}]", id, v.len(), s.join(", "));
                }
            }
            Ok(None) => {
                if fmt == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"name":parts[2],"id":id,"found":false})
                    );
                } else {
                    println!("(未找到 id={})", id);
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

fn cmd_delete(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    // :vec delete <name> <id>
    if parts.len() < 4 {
        report(had_error, fmt, ":vec delete <name> <id>");
        return;
    }
    let id: u64 = match parts[3].parse() {
        Ok(n) => n,
        Err(_) => {
            report(had_error, fmt, "id 必须为整数（u64）");
            return;
        }
    };
    match db.vector(parts[2]) {
        Ok(ve) => match ve.delete(id) {
            Ok(()) => {
                if fmt == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"name":parts[2],"id":id})
                    );
                } else {
                    println!("已删除 id={} 从 {}", id, parts[2]);
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

fn cmd_rebuild(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    // :vec rebuild <name>
    if parts.len() < 3 {
        report(had_error, fmt, ":vec rebuild <name>");
        return;
    }
    match db.vector(parts[2]) {
        Ok(ve) => match ve.rebuild_index() {
            Ok(n) => {
                if fmt == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"name":parts[2],"rebuilt":n})
                    );
                } else {
                    println!("索引已重建，共 {} 条记录（{}）", n, parts[2]);
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
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
