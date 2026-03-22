//! MQ 引擎命令处理。

use crate::OutputFormat;
use std::sync::atomic::{AtomicBool, Ordering};
use talon::Talon;

/// 处理 `:mq` 子命令。
pub fn handle(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 2 {
        report(had_error, fmt, ":mq 需要子命令。输入 :help 查看。");
        return;
    }
    match parts[1] {
        "topics" => {
            match db.mq_read() {
                Ok(mq) => match mq.list_topics() {
                    Ok(topics) => {
                        if fmt == OutputFormat::Json {
                            println!(
                                "{}",
                                serde_json::json!({"ok":true,"topics":topics,"count":topics.len()})
                            );
                        } else if topics.is_empty() {
                            println!("(无 topic)");
                        } else {
                            for t in &topics {
                                println!("  {}", t);
                            }
                            println!("({} topics)", topics.len());
                        }
                    }
                    Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
                },
                Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
            }
        }
        "len" => {
            if parts.len() < 3 {
                report(had_error, fmt, ":mq len <topic>");
                return;
            }
            match db.mq_read() {
                Ok(mq) => match mq.len(parts[2]) {
                    Ok(n) => {
                        if fmt == OutputFormat::Json {
                            println!(
                                "{}",
                                serde_json::json!({"ok":true,"topic":parts[2],"length":n})
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
        "pub" => {
            if parts.len() < 4 {
                report(had_error, fmt, ":mq pub <topic> <message>");
                return;
            }
            match db.mq() {
                Ok(mq) => match mq.publish(parts[2], parts[3].as_bytes()) {
                    Ok(id) => {
                        if fmt == OutputFormat::Json {
                            println!(
                                "{}",
                                serde_json::json!({"ok":true,"topic":parts[2],"msg_id":id})
                            );
                        } else {
                            println!("OK (msg_id: {})", id);
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
            &format!("未知 MQ 子命令: {}", parts[1]),
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
