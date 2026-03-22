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
        "count" => {
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
        _ => report(
            had_error,
            fmt,
            &format!("未知 vec 子命令: {}", parts[1]),
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
