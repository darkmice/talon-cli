//! KV 引擎命令处理。

use crate::OutputFormat;
use std::sync::atomic::{AtomicBool, Ordering};
use talon::Talon;

/// 处理 `:kv` 子命令。
pub fn handle(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 2 {
        report(had_error, fmt, ":kv 需要子命令。输入 :help 查看。");
        return;
    }
    match parts[1] {
        "get" => cmd_get(db, parts, fmt, had_error),
        "set" => cmd_set(db, parts, fmt, had_error),
        "del" => cmd_del(db, parts, fmt, had_error),
        "keys" => cmd_keys(db, parts, fmt, had_error),
        "scan" => cmd_scan(db, parts, fmt, had_error),
        "count" => cmd_count(db, fmt, had_error),
        "exists" => cmd_exists(db, parts, fmt, had_error),
        "incr" => cmd_incr(db, parts, fmt, had_error),
        "ttl" => cmd_ttl(db, parts, fmt, had_error),
        _ => report(
            had_error,
            fmt,
            &format!("未知 KV 子命令: {}", parts[1]),
        ),
    }
}

fn cmd_get(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 3 {
        report(had_error, fmt, ":kv get <key>");
        return;
    }
    match db.kv_read() {
        Ok(kv) => match kv.get(parts[2].as_bytes()) {
            Ok(Some(val)) => {
                if fmt == OutputFormat::Json {
                    let s = String::from_utf8_lossy(&val);
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"key":parts[2],"value":s.as_ref()})
                    );
                } else {
                    match std::str::from_utf8(&val) {
                        Ok(s) => println!("{}", s),
                        Err(_) => {
                            println!("(binary {} bytes) {:?}", val.len(), &val[..val.len().min(128)])
                        }
                    }
                }
            }
            Ok(None) => {
                if fmt == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"key":parts[2],"value":null})
                    );
                } else {
                    println!("(nil)");
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

fn cmd_set(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 4 {
        report(had_error, fmt, ":kv set <key> <value>");
        return;
    }
    match db.kv() {
        Ok(kv) => match kv.set(parts[2].as_bytes(), parts[3].as_bytes(), None) {
            Ok(()) => {
                if fmt == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"key":parts[2],"action":"set"})
                    );
                } else {
                    println!("OK");
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

fn cmd_del(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 3 {
        report(had_error, fmt, ":kv del <key>");
        return;
    }
    match db.kv() {
        Ok(kv) => match kv.del(parts[2].as_bytes()) {
            Ok(()) => {
                if fmt == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"key":parts[2],"action":"del"})
                    );
                } else {
                    println!("OK");
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

fn cmd_keys(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    let prefix = if parts.len() >= 3 { parts[2] } else { "" };
    match db.kv_read() {
        Ok(kv) => match kv.keys_prefix_limit(prefix.as_bytes(), 0, 100) {
            Ok(keys) => {
                if fmt == OutputFormat::Json {
                    let key_strs: Vec<String> = keys
                        .iter()
                        .map(|k| String::from_utf8_lossy(k).to_string())
                        .collect();
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"keys":key_strs,"count":key_strs.len()})
                    );
                } else {
                    for k in &keys {
                        match std::str::from_utf8(k) {
                            Ok(s) => println!("  {}", s),
                            Err(_) => println!("  (binary key {} bytes)", k.len()),
                        }
                    }
                    println!("({} keys)", keys.len());
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

fn cmd_scan(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    let prefix = if parts.len() >= 3 { parts[2] } else { "" };
    let limit: u64 = if parts.len() >= 4 {
        parts[3].parse().unwrap_or(20)
    } else {
        20
    };
    match db.kv_read() {
        Ok(kv) => match kv.scan_prefix_limit(prefix.as_bytes(), 0, limit) {
            Ok(pairs) => {
                if fmt == OutputFormat::Json {
                    let entries: Vec<serde_json::Value> = pairs
                        .iter()
                        .map(|(k, v)| {
                            serde_json::json!({
                                "key": String::from_utf8_lossy(k).as_ref(),
                                "value": String::from_utf8_lossy(v).as_ref(),
                            })
                        })
                        .collect();
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"entries":entries,"count":entries.len()})
                    );
                } else {
                    for (k, v) in &pairs {
                        let key_s = String::from_utf8_lossy(k);
                        let val_s = match std::str::from_utf8(v) {
                            Ok(s) => {
                                if s.len() > 120 {
                                    format!("{}...({}B)", &s[..120], s.len())
                                } else {
                                    s.to_string()
                                }
                            }
                            Err(_) => format!("(binary {} bytes)", v.len()),
                        };
                        println!("  {} => {}", key_s, val_s);
                    }
                    println!("({} pairs)", pairs.len());
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

fn cmd_count(db: &Talon, fmt: OutputFormat, had_error: &AtomicBool) {
    match db.kv_read() {
        Ok(kv) => match kv.key_count() {
            Ok(n) => {
                if fmt == OutputFormat::Json {
                    println!("{}", serde_json::json!({"ok":true,"count":n}));
                } else {
                    println!("{}", n);
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

fn cmd_exists(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 3 {
        report(had_error, fmt, ":kv exists <key>");
        return;
    }
    match db.kv_read() {
        Ok(kv) => match kv.exists(parts[2].as_bytes()) {
            Ok(b) => {
                if fmt == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"key":parts[2],"exists":b})
                    );
                } else {
                    println!("{}", if b { "true" } else { "false" });
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

fn cmd_incr(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 3 {
        report(had_error, fmt, ":kv incr <key>");
        return;
    }
    match db.kv() {
        Ok(kv) => match kv.incr(parts[2].as_bytes()) {
            Ok(n) => {
                if fmt == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"key":parts[2],"value":n})
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

fn cmd_ttl(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 3 {
        report(had_error, fmt, ":kv ttl <key>");
        return;
    }
    match db.kv_read() {
        Ok(kv) => match kv.ttl(parts[2].as_bytes()) {
            Ok(Some(t)) => {
                if fmt == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"key":parts[2],"ttl_secs":t})
                    );
                } else {
                    println!("{} 秒", t);
                }
            }
            Ok(None) => {
                if fmt == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"key":parts[2],"ttl_secs":null,"permanent":true})
                    );
                } else {
                    println!("永久（无 TTL）");
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

/// 统一错误输出。
fn report(had_error: &AtomicBool, fmt: OutputFormat, msg: &str) {
    had_error.store(true, Ordering::Relaxed);
    if fmt == OutputFormat::Json {
        println!("{}", serde_json::json!({"ok":false,"error":msg}));
    } else {
        eprintln!("{}", msg);
    }
}
