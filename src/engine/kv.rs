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
        "expire" => cmd_expire(db, parts, fmt, had_error),
        "incrby" => cmd_incrby(db, parts, fmt, had_error),
        "decrby" => cmd_decrby(db, parts, fmt, had_error),
        "setnx" => cmd_setnx(db, parts, fmt, had_error),
        "mset" => cmd_mset(db, parts, fmt, had_error),
        "mget" => cmd_mget(db, parts, fmt, had_error),
        "rename" => cmd_rename(db, parts, fmt, had_error),
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
        report(had_error, fmt, ":kv set <key> <value> [--ttl <secs>]");
        return;
    }
    // 解析可选 --ttl N
    let ttl_secs: Option<u64> = parts
        .iter()
        .position(|&p| p == "--ttl")
        .and_then(|i| parts.get(i + 1))
        .and_then(|s| s.parse().ok());
    match db.kv() {
        Ok(kv) => match kv.set(parts[2].as_bytes(), parts[3].as_bytes(), ttl_secs) {
            Ok(()) => {
                if fmt == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"key":parts[2],"action":"set","ttl_secs":ttl_secs})
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

fn cmd_expire(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 4 {
        report(had_error, fmt, ":kv expire <key> <secs>");
        return;
    }
    let secs: u64 = match parts[3].parse() {
        Ok(n) => n,
        Err(_) => {
            report(had_error, fmt, "错误: <secs> 必须是非负整数");
            return;
        }
    };
    match db.kv() {
        Ok(kv) => match kv.expire(parts[2].as_bytes(), secs) {
            Ok(()) => {
                if fmt == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"key":parts[2],"action":"expire","ttl_secs":secs})
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

fn cmd_incrby(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 4 {
        report(had_error, fmt, ":kv incrby <key> <delta>");
        return;
    }
    let delta: i64 = match parts[3].parse() {
        Ok(n) => n,
        Err(_) => {
            report(had_error, fmt, "错误: <delta> 必须是整数");
            return;
        }
    };
    match db.kv() {
        Ok(kv) => match kv.incrby(parts[2].as_bytes(), delta) {
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

fn cmd_decrby(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 4 {
        report(had_error, fmt, ":kv decrby <key> <delta>");
        return;
    }
    let delta: i64 = match parts[3].parse() {
        Ok(n) => n,
        Err(_) => {
            report(had_error, fmt, "错误: <delta> 必须是整数");
            return;
        }
    };
    match db.kv() {
        Ok(kv) => match kv.decrby(parts[2].as_bytes(), delta) {
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

fn cmd_setnx(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 4 {
        report(had_error, fmt, ":kv setnx <key> <value>");
        return;
    }
    match db.kv() {
        Ok(kv) => match kv.setnx(parts[2].as_bytes(), parts[3].as_bytes(), None) {
            Ok(set) => {
                if fmt == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"key":parts[2],"set":set})
                    );
                } else {
                    println!("{}", if set { "1" } else { "0" });
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

fn cmd_mset(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    // parts[2..] 必须是成对的 k v k v ...
    let kv_args = &parts[2..];
    if kv_args.is_empty() || kv_args.len() % 2 != 0 {
        report(had_error, fmt, ":kv mset <k1> <v1> <k2> <v2> ...（参数必须成对）");
        return;
    }
    let keys: Vec<&[u8]> = kv_args.iter().step_by(2).map(|s| s.as_bytes()).collect();
    let values: Vec<&[u8]> = kv_args.iter().skip(1).step_by(2).map(|s| s.as_bytes()).collect();
    match db.kv() {
        Ok(kv) => match kv.mset(&keys, &values) {
            Ok(()) => {
                if fmt == OutputFormat::Json {
                    let key_strs: Vec<&str> = kv_args.iter().step_by(2).copied().collect();
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"action":"mset","count":keys.len(),"keys":key_strs})
                    );
                } else {
                    println!("OK ({} 对)", keys.len());
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

fn cmd_mget(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 3 {
        report(had_error, fmt, ":kv mget <k1> <k2> ...");
        return;
    }
    let key_args = &parts[2..];
    let keys: Vec<&[u8]> = key_args.iter().map(|s| s.as_bytes()).collect();
    match db.kv_read() {
        Ok(kv) => match kv.mget(&keys) {
            Ok(vals) => {
                if fmt == OutputFormat::Json {
                    let entries: Vec<serde_json::Value> = key_args
                        .iter()
                        .zip(vals.iter())
                        .map(|(k, v)| {
                            let val = v.as_deref().map(|b| String::from_utf8_lossy(b).into_owned());
                            serde_json::json!({"key": k, "value": val})
                        })
                        .collect();
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"entries":entries})
                    );
                } else {
                    for (k, v) in key_args.iter().zip(vals.iter()) {
                        match v {
                            Some(b) => println!("{} => {}", k, String::from_utf8_lossy(b)),
                            None => println!("{} => (nil)", k),
                        }
                    }
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

fn cmd_rename(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 4 {
        report(had_error, fmt, ":kv rename <src> <dst>");
        return;
    }
    match db.kv() {
        Ok(kv) => match kv.rename(parts[2].as_bytes(), parts[3].as_bytes()) {
            Ok(()) => {
                if fmt == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"src":parts[2],"dst":parts[3],"action":"rename"})
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

/// 统一错误输出。
fn report(had_error: &AtomicBool, fmt: OutputFormat, msg: &str) {
    had_error.store(true, Ordering::Relaxed);
    if fmt == OutputFormat::Json {
        println!("{}", serde_json::json!({"ok":false,"error":msg}));
    } else {
        eprintln!("{}", msg);
    }
}
