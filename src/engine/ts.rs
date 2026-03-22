//! TS 时序引擎命令处理。

use crate::OutputFormat;
use std::sync::atomic::{AtomicBool, Ordering};
use talon::Talon;

/// 处理 `:ts` 子命令。
pub fn handle(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 2 {
        report(had_error, fmt, ":ts 需要子命令。输入 :help 查看。");
        return;
    }
    match parts[1] {
        "list" => {
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
        "info" => {
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
        _ => report(
            had_error,
            fmt,
            &format!("未知 ts 子命令: {}", parts[1]),
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
