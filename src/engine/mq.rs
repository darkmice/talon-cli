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
        "topics" => cmd_topics(db, parts, fmt, had_error),
        "len" => cmd_len(db, parts, fmt, had_error),
        "pub" => cmd_pub(db, parts, fmt, had_error),
        "create" => cmd_create(db, parts, fmt, had_error),
        "poll" => cmd_poll(db, parts, fmt, had_error),
        "ack" => cmd_ack(db, parts, fmt, had_error),
        "sub" => cmd_sub(db, parts, fmt, had_error),
        "unsub" => cmd_unsub(db, parts, fmt, had_error),
        _ => report(
            had_error,
            fmt,
            &format!("未知 MQ 子命令: {}", parts[1]),
        ),
    }
}

fn cmd_topics(db: &Talon, _parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
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

fn cmd_len(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
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

fn cmd_pub(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
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

/// `:mq create <topic> [max_len]`
fn cmd_create(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 3 {
        report(had_error, fmt, ":mq create <topic> [max_len]");
        return;
    }
    let topic = parts[2];
    let max_len: u64 = if parts.len() >= 4 {
        match parts[3].parse() {
            Ok(n) => n,
            Err(_) => {
                report(had_error, fmt, "max_len 必须为整数");
                return;
            }
        }
    } else {
        0
    };
    match db.mq() {
        Ok(mq) => match mq.create_topic(topic, max_len) {
            Ok(()) => {
                if fmt == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"topic":topic,"max_len":max_len})
                    );
                } else {
                    if max_len > 0 {
                        println!("OK (topic: {}, max_len: {})", topic, max_len);
                    } else {
                        println!("OK (topic: {}, max_len: 无限制)", topic);
                    }
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

/// `:mq poll <topic> <group> <consumer> [count]`
fn cmd_poll(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 5 {
        report(had_error, fmt, ":mq poll <topic> <group> <consumer> [count]");
        return;
    }
    let topic = parts[2];
    let group = parts[3];
    let consumer = parts[4];
    let count: usize = if parts.len() >= 6 {
        match parts[5].parse() {
            Ok(n) => n,
            Err(_) => {
                report(had_error, fmt, "count 必须为整数");
                return;
            }
        }
    } else {
        10
    };
    match db.mq_read() {
        Ok(mq) => match mq.poll(topic, group, consumer, count) {
            Ok(msgs) => {
                if fmt == OutputFormat::Json {
                    let arr: Vec<serde_json::Value> = msgs
                        .iter()
                        .map(|m| {
                            serde_json::json!({
                                "id": m.id,
                                "payload": String::from_utf8_lossy(&m.payload),
                                "timestamp": m.timestamp,
                            })
                        })
                        .collect();
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"topic":topic,"group":group,"consumer":consumer,"count":arr.len(),"messages":arr})
                    );
                } else if msgs.is_empty() {
                    println!("(无消息)");
                } else {
                    for m in &msgs {
                        let payload_str = String::from_utf8_lossy(&m.payload);
                        println!("  id={} ts={} payload={}", m.id, m.timestamp, payload_str);
                    }
                    println!("({} 条消息)", msgs.len());
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

/// `:mq ack <topic> <group> <consumer> <msg_id>`
fn cmd_ack(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 6 {
        report(had_error, fmt, ":mq ack <topic> <group> <consumer> <msg_id>");
        return;
    }
    let topic = parts[2];
    let group = parts[3];
    let consumer = parts[4];
    let msg_id: u64 = match parts[5].parse() {
        Ok(n) => n,
        Err(_) => {
            report(had_error, fmt, "msg_id 必须为整数");
            return;
        }
    };
    match db.mq() {
        Ok(mq) => match mq.ack(topic, group, consumer, msg_id) {
            Ok(()) => {
                if fmt == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"topic":topic,"group":group,"consumer":consumer,"msg_id":msg_id})
                    );
                } else {
                    println!("OK (ack msg_id: {})", msg_id);
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

/// `:mq sub <topic> <group>`
fn cmd_sub(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 4 {
        report(had_error, fmt, ":mq sub <topic> <group>");
        return;
    }
    let topic = parts[2];
    let group = parts[3];
    match db.mq() {
        Ok(mq) => match mq.subscribe(topic, group) {
            Ok(()) => {
                if fmt == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"topic":topic,"group":group})
                    );
                } else {
                    println!("OK (subscribed: {} -> {})", group, topic);
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

/// `:mq unsub <topic> <group>`
fn cmd_unsub(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 4 {
        report(had_error, fmt, ":mq unsub <topic> <group>");
        return;
    }
    let topic = parts[2];
    let group = parts[3];
    match db.mq() {
        Ok(mq) => match mq.unsubscribe(topic, group) {
            Ok(()) => {
                if fmt == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"topic":topic,"group":group})
                    );
                } else {
                    println!("OK (unsubscribed: {} from {})", group, topic);
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
