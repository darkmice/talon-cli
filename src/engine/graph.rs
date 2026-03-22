//! Graph 图引擎命令处理。

use crate::OutputFormat;
use std::sync::atomic::{AtomicBool, Ordering};
use talon::Talon;

/// 处理 `:graph` 子命令。
pub fn handle(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 2 {
        report(had_error, fmt, ":graph 需要子命令。输入 :help 查看。");
        return;
    }
    match parts[1] {
        "count" => cmd_count(db, parts, fmt, had_error),
        "vertex" => cmd_vertex(db, parts, fmt, had_error),
        "neighbors" => cmd_neighbors(db, parts, fmt, had_error),
        "bfs" => cmd_bfs(db, parts, fmt, had_error),
        _ => report(
            had_error,
            fmt,
            &format!("未知 graph 子命令: {}", parts[1]),
        ),
    }
}

fn cmd_count(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 3 {
        report(had_error, fmt, ":graph count <name>");
        return;
    }
    match db.graph_read() {
        Ok(g) => {
            let vc = g.vertex_count(parts[2]).unwrap_or(0);
            let ec = g.edge_count(parts[2]).unwrap_or(0);
            if fmt == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({"ok":true,"graph":parts[2],"vertices":vc,"edges":ec})
                );
            } else {
                println!("顶点: {}, 边: {}", vc, ec);
            }
        }
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

fn cmd_vertex(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 4 {
        report(had_error, fmt, ":graph vertex <name> <id>");
        return;
    }
    let id: u64 = match parts[3].parse() {
        Ok(n) => n,
        Err(_) => {
            report(had_error, fmt, "id 必须为整数");
            return;
        }
    };
    match db.graph_read() {
        Ok(g) => match g.get_vertex(parts[2], id) {
            Ok(Some(v)) => {
                if fmt == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"id":v.id,"label":v.label,"properties":v.properties})
                    );
                } else {
                    let props = serde_json::to_string(&v.properties).unwrap_or_default();
                    println!("  id={}, label={}, props={}", v.id, v.label, props);
                }
            }
            Ok(None) => {
                if fmt == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"id":id,"found":false})
                    );
                } else {
                    println!("(未找到 vertex {})", id);
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

fn cmd_neighbors(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 4 {
        report(had_error, fmt, ":graph neighbors <name> <id> [out|in|both]");
        return;
    }
    let sub: Vec<&str> = parts[3].splitn(2, ' ').collect();
    let id: u64 = match sub[0].parse() {
        Ok(n) => n,
        Err(_) => {
            report(had_error, fmt, "id 必须为整数");
            return;
        }
    };
    let dir = match sub.get(1).copied().unwrap_or("out") {
        "in" => talon::Direction::In,
        "both" => talon::Direction::Both,
        _ => talon::Direction::Out,
    };
    match db.graph_read() {
        Ok(g) => match g.neighbors(parts[2], id, dir) {
            Ok(neighbor_ids) => {
                if fmt == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"neighbors":neighbor_ids,"count":neighbor_ids.len()})
                    );
                } else {
                    for nid in &neighbor_ids {
                        println!("  vertex {}", nid);
                    }
                    println!("({} 个邻居)", neighbor_ids.len());
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

fn cmd_bfs(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 4 {
        report(had_error, fmt, ":graph bfs <name> <start> [depth]");
        return;
    }
    let sub: Vec<&str> = parts[3].splitn(2, ' ').collect();
    let start: u64 = match sub[0].parse() {
        Ok(n) => n,
        Err(_) => {
            report(had_error, fmt, "start 必须为整数");
            return;
        }
    };
    let depth: usize = sub.get(1).and_then(|s| s.parse().ok()).unwrap_or(3);
    match db.graph_read() {
        Ok(g) => match g.bfs(parts[2], start, depth, talon::Direction::Out) {
            Ok(nodes) => {
                if fmt == OutputFormat::Json {
                    let entries: Vec<serde_json::Value> = nodes
                        .iter()
                        .map(|(vid, d)| serde_json::json!({"id":vid,"depth":d}))
                        .collect();
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"nodes":entries,"count":entries.len()})
                    );
                } else {
                    for (vid, d) in &nodes {
                        println!("  vertex {} (depth={})", vid, d);
                    }
                    println!("({} 个顶点)", nodes.len());
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
