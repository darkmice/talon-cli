//! 备份恢复（`:backup`）和操作日志（`:oplog`）命令处理。
//!
//! 仅嵌入模式可用（直接持有 `&talon::Talon`）。
//! 网络模式：`:backup export/import` 可用；`:backup snapshot/restore` 和 `:oplog` 均受限（标注）。

use crate::OutputFormat;
use std::sync::atomic::{AtomicBool, Ordering};
use talon::{RecoveryTarget, Talon};

// ── :backup ──────────────────────────────────────────────────────────────────

/// 处理 `:backup` 子命令（嵌入模式）。
pub fn handle(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 2 {
        report(had_error, fmt, ":backup 需要子命令。输入 :help 查看。");
        return;
    }
    match parts[1] {
        "export"   => cmd_export(db, parts, fmt, had_error),
        "import"   => cmd_import(db, parts, fmt, had_error),
        "snapshot" => cmd_snapshot(db, parts, fmt, had_error),
        "restore"  => cmd_restore(db, parts, fmt, had_error),
        _ => report(had_error, fmt, &format!("未知 backup 子命令: {}", parts[1])),
    }
}

/// `:backup export <dir> [ks1 ks2 ...]`
///
/// 导出快照到目录；可选 keyspace 列表，空=全部。
fn cmd_export(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 3 {
        report(had_error, fmt, "用法: :backup export <dir> [ks1 ks2 ...]");
        return;
    }
    let dir = parts[2];
    // parts[3..] 为可选 keyspace 名列表；空 slice 表示导出全部
    let ks_names: Vec<&str> = parts[3..].to_vec();
    match db.export(dir, &ks_names) {
        Ok(count) => {
            if fmt == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({"ok":true,"dir":dir,"exported":count,"keyspaces":ks_names})
                );
            } else {
                println!(
                    "导出完成: {} 条条目 → {}{}",
                    count,
                    dir,
                    if ks_names.is_empty() {
                        String::new()
                    } else {
                        format!("（keyspace: {}）", ks_names.join(", "))
                    }
                );
            }
        }
        Err(e) => report(had_error, fmt, &format!("export 错误: {}", e)),
    }
}

/// `:backup import <dir>`
///
/// 从目录导入快照。
fn cmd_import(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 3 {
        report(had_error, fmt, "用法: :backup import <dir>");
        return;
    }
    let dir = parts[2];
    match db.import(dir) {
        Ok(count) => {
            if fmt == OutputFormat::Json {
                println!("{}", serde_json::json!({"ok":true,"dir":dir,"imported":count}));
            } else {
                println!("导入完成: {} 条条目 ← {}", count, dir);
            }
        }
        Err(e) => report(had_error, fmt, &format!("import 错误: {}", e)),
    }
}

/// `:backup snapshot <dir> [timestamp_ms]`
///
/// 一致性快照（PITR）；timestamp_ms 可选，默认 0（使用系统时间由后端填充）。
fn cmd_snapshot(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 3 {
        report(had_error, fmt, "用法: :backup snapshot <dir> [timestamp_ms]");
        return;
    }
    let dir = parts[2];
    let ts_ms: u64 = parts
        .get(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    match db.backup_consistent(dir, ts_ms) {
        Ok(outcome) => {
            if fmt == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "dir": outcome.dir,
                        "entries": outcome.entries,
                        "lsn_anchor": outcome.lsn_anchor,
                        "timestamp_ms": outcome.timestamp_ms,
                    })
                );
            } else {
                println!(
                    "快照完成: {} 条条目，lsn_anchor={}，ts_ms={} → {}",
                    outcome.entries,
                    outcome.lsn_anchor,
                    outcome.timestamp_ms,
                    outcome.dir.display(),
                );
            }
        }
        Err(e) => report(had_error, fmt, &format!("snapshot 错误: {}", e)),
    }
}

/// `:backup restore <dir> <latest|snapshot|lsn <n>|ts <ms>>`
///
/// 恢复到指定 PITR 点：
/// - `latest`      → 重放全部归档 OpLog
/// - `snapshot`    → 仅恢复快照（不重放任何 OpLog）
/// - `lsn <n>`     → 恢复到 LSN n（含）
/// - `ts <ms>`     → 恢复到时间戳 ms（含）
fn cmd_restore(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 4 {
        report(
            had_error,
            fmt,
            "用法: :backup restore <dir> <latest|snapshot|lsn <n>|ts <ms>>",
        );
        return;
    }
    let dir = parts[2];
    let target = match parts[3] {
        "latest"   => RecoveryTarget::Latest,
        "snapshot" => RecoveryTarget::SnapshotOnly,
        "lsn" => {
            let n = match parts.get(4).and_then(|s| s.parse::<u64>().ok()) {
                Some(n) => n,
                None => {
                    report(
                        had_error,
                        fmt,
                        "lsn 后需提供有效的整数，例如: :backup restore <dir> lsn 42",
                    );
                    return;
                }
            };
            RecoveryTarget::Lsn(n)
        }
        "ts" => {
            let ms = match parts.get(4).and_then(|s| s.parse::<u64>().ok()) {
                Some(ms) => ms,
                None => {
                    report(
                        had_error,
                        fmt,
                        "ts 后需提供有效的毫秒时间戳，例如: :backup restore <dir> ts 1748000000000",
                    );
                    return;
                }
            };
            RecoveryTarget::TimestampMs(ms)
        }
        other => {
            report(
                had_error,
                fmt,
                &format!(
                    "未知恢复目标 `{}`，支持: latest | snapshot | lsn <n> | ts <ms>",
                    other
                ),
            );
            return;
        }
    };
    match db.restore_to_point(dir, target) {
        Ok(outcome) => {
            if fmt == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "dir": dir,
                        "imported_entries": outcome.imported_entries,
                        "replayed_ops": outcome.replayed_ops,
                        "final_lsn": outcome.final_lsn,
                    })
                );
            } else {
                println!(
                    "恢复完成: 快照 {} 条，重放 {} 条 OpLog，最终 LSN={}",
                    outcome.imported_entries,
                    outcome.replayed_ops,
                    outcome.final_lsn,
                );
            }
        }
        Err(e) => report(had_error, fmt, &format!("restore 错误: {}", e)),
    }
}

// ── :oplog ───────────────────────────────────────────────────────────────────

/// 处理 `:oplog` 子命令（仅嵌入模式；网络模式无独立 module，不可用）。
pub fn handle_oplog(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 2 {
        report(had_error, fmt, ":oplog 需要子命令。输入 :help 查看。");
        return;
    }
    match parts[1] {
        "lsn"   => cmd_oplog_lsn(db, fmt, had_error),
        "get"   => cmd_oplog_get(db, parts, fmt, had_error),
        "range" => cmd_oplog_range(db, parts, fmt, had_error),
        _ => report(had_error, fmt, &format!("未知 oplog 子命令: {}", parts[1])),
    }
}

/// `:oplog lsn`  — 当前最大 LSN。
fn cmd_oplog_lsn(db: &Talon, fmt: OutputFormat, _had_error: &AtomicBool) {
    let lsn = db.oplog_current_lsn();
    if fmt == OutputFormat::Json {
        println!("{}", serde_json::json!({"ok":true,"lsn":lsn}));
    } else {
        println!("当前 LSN: {}", lsn);
    }
}

/// `:oplog get <lsn>`  — 取单条 OpLog 条目。
fn cmd_oplog_get(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 3 {
        report(had_error, fmt, "用法: :oplog get <lsn>");
        return;
    }
    let lsn: u64 = match parts[2].parse() {
        Ok(n) => n,
        Err(_) => {
            report(had_error, fmt, "lsn 必须为整数");
            return;
        }
    };
    match db.oplog_get(lsn) {
        Ok(Some(entry)) => {
            if fmt == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "lsn": entry.lsn,
                        "timestamp_ms": entry.timestamp_ms,
                        "op": format!("{:?}", entry.op),
                    })
                );
            } else {
                println!(
                    "LSN={} ts_ms={} op={:?}",
                    entry.lsn, entry.timestamp_ms, entry.op
                );
            }
        }
        Ok(None) => {
            if fmt == OutputFormat::Json {
                println!("{}", serde_json::json!({"ok":true,"lsn":lsn,"found":false}));
            } else {
                println!("(未找到 LSN {})", lsn);
            }
        }
        Err(e) => report(had_error, fmt, &format!("oplog get 错误: {}", e)),
    }
}

/// `:oplog range <from> <to> [limit]`  — 取范围内条目（limit 默认 100）。
fn cmd_oplog_range(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 4 {
        report(had_error, fmt, "用法: :oplog range <from_lsn> <to_lsn> [limit]");
        return;
    }
    let from: u64 = match parts[2].parse() {
        Ok(n) => n,
        Err(_) => {
            report(had_error, fmt, "from_lsn 必须为整数");
            return;
        }
    };
    let to: u64 = match parts[3].parse() {
        Ok(n) => n,
        Err(_) => {
            report(had_error, fmt, "to_lsn 必须为整数");
            return;
        }
    };
    let limit: usize = parts
        .get(4)
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);
    match db.oplog_range(from, to, limit) {
        Ok(entries) => {
            if fmt == OutputFormat::Json {
                let arr: Vec<serde_json::Value> = entries
                    .iter()
                    .map(|e| {
                        serde_json::json!({
                            "lsn": e.lsn,
                            "timestamp_ms": e.timestamp_ms,
                            "op": format!("{:?}", e.op),
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::json!({"ok":true,"entries":arr,"count":arr.len()})
                );
            } else {
                for e in &entries {
                    println!("  LSN={} ts_ms={} op={:?}", e.lsn, e.timestamp_ms, e.op);
                }
                println!("({} 条)", entries.len());
            }
        }
        Err(e) => report(had_error, fmt, &format!("oplog range 错误: {}", e)),
    }
}

// ── 辅助 ─────────────────────────────────────────────────────────────────────

fn report(had_error: &AtomicBool, fmt: OutputFormat, msg: &str) {
    had_error.store(true, Ordering::Relaxed);
    if fmt == OutputFormat::Json {
        println!("{}", serde_json::json!({"ok":false,"error":msg}));
    } else {
        eprintln!("{}", msg);
    }
}
