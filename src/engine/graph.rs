//! Graph 图引擎命令处理。

use crate::cmdline::parse_props_json;
use crate::OutputFormat;
use std::collections::BTreeMap;
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
        "add-vertex" => cmd_add_vertex(db, parts, fmt, had_error),
        "add-edge" => cmd_add_edge(db, parts, fmt, had_error),
        "update-vertex" => cmd_update_vertex(db, parts, fmt, had_error),
        "del-vertex" => cmd_del_vertex(db, parts, fmt, had_error),
        "del-edge" => cmd_del_edge(db, parts, fmt, had_error),
        "edges" => cmd_edges(db, parts, fmt, had_error),
        "path" => cmd_path(db, parts, fmt, had_error),
        "pagerank" => cmd_pagerank(db, parts, fmt, had_error),
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
    let id: u64 = match parts[3].parse() {
        Ok(n) => n,
        Err(_) => {
            report(had_error, fmt, "id 必须为整数");
            return;
        }
    };
    let dir = match parts.get(4).copied().unwrap_or("out") {
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
    let start: u64 = match parts[3].parse() {
        Ok(n) => n,
        Err(_) => {
            report(had_error, fmt, "start 必须为整数");
            return;
        }
    };
    let depth: usize = parts.get(4).and_then(|s| s.parse().ok()).unwrap_or(3);
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

// ── 辅助：解析可选 props_json 参数 ─────────────────────────────────────────

fn resolve_props(
    had_error: &AtomicBool,
    fmt: OutputFormat,
    raw: Option<&str>,
) -> Option<BTreeMap<String, String>> {
    match raw {
        None => Some(BTreeMap::new()),
        Some(s) => match parse_props_json(s) {
            Ok(m) => Some(m),
            Err(e) => {
                report(had_error, fmt, &format!("props 解析失败: {}", e));
                None
            }
        },
    }
}

// ── 辅助：解析 u64 id ───────────────────────────────────────────────────────

fn parse_id(had_error: &AtomicBool, fmt: OutputFormat, s: &str, label: &str) -> Option<u64> {
    match s.parse::<u64>() {
        Ok(n) => Some(n),
        Err(_) => {
            report(had_error, fmt, &format!("{} 必须为整数", label));
            None
        }
    }
}

// ── 写命令 ────────────────────────────────────────────────────────────────

fn cmd_add_vertex(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    // :graph add-vertex <graph> <label> [props_json]
    if parts.len() < 4 {
        report(had_error, fmt, ":graph add-vertex <graph> <label> [props_json]");
        return;
    }
    let graph = parts[2];
    let label = parts[3];
    let props = match resolve_props(had_error, fmt, parts.get(4).copied()) {
        Some(p) => p,
        None => return,
    };
    match db.graph() {
        Ok(g) => match g.add_vertex(graph, label, &props) {
            Ok(id) => {
                if fmt == OutputFormat::Json {
                    println!("{}", serde_json::json!({"ok":true,"id":id}));
                } else {
                    println!("已添加顶点 id={}", id);
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

fn cmd_add_edge(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    // :graph add-edge <graph> <from_id> <to_id> <label> [props_json]
    if parts.len() < 6 {
        report(
            had_error,
            fmt,
            ":graph add-edge <graph> <from_id> <to_id> <label> [props_json]",
        );
        return;
    }
    let graph = parts[2];
    let from = match parse_id(had_error, fmt, parts[3], "from_id") {
        Some(n) => n,
        None => return,
    };
    let to = match parse_id(had_error, fmt, parts[4], "to_id") {
        Some(n) => n,
        None => return,
    };
    let label = parts[5];
    let props = match resolve_props(had_error, fmt, parts.get(6).copied()) {
        Some(p) => p,
        None => return,
    };
    match db.graph() {
        Ok(g) => match g.add_edge(graph, from, to, label, &props) {
            Ok(id) => {
                if fmt == OutputFormat::Json {
                    println!("{}", serde_json::json!({"ok":true,"id":id}));
                } else {
                    println!("已添加边 id={} ({} → {})", id, from, to);
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

fn cmd_update_vertex(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    // :graph update-vertex <graph> <id> <props_json>
    if parts.len() < 5 {
        report(had_error, fmt, ":graph update-vertex <graph> <id> <props_json>");
        return;
    }
    let graph = parts[2];
    let id = match parse_id(had_error, fmt, parts[3], "id") {
        Some(n) => n,
        None => return,
    };
    let props = match parse_props_json(parts[4]) {
        Ok(m) => m,
        Err(e) => {
            report(had_error, fmt, &format!("props 解析失败: {}", e));
            return;
        }
    };
    match db.graph() {
        Ok(g) => match g.update_vertex(graph, id, &props) {
            Ok(()) => {
                if fmt == OutputFormat::Json {
                    println!("{}", serde_json::json!({"ok":true}));
                } else {
                    println!("顶点 {} 已更新", id);
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

fn cmd_del_vertex(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    // :graph del-vertex <graph> <id>
    if parts.len() < 4 {
        report(had_error, fmt, ":graph del-vertex <graph> <id>");
        return;
    }
    let graph = parts[2];
    let id = match parse_id(had_error, fmt, parts[3], "id") {
        Some(n) => n,
        None => return,
    };
    match db.graph() {
        Ok(g) => match g.delete_vertex(graph, id) {
            Ok(deleted) => {
                if fmt == OutputFormat::Json {
                    println!("{}", serde_json::json!({"ok":true,"deleted":deleted}));
                } else if deleted {
                    println!("顶点 {} 已删除", id);
                } else {
                    println!("顶点 {} 不存在", id);
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

fn cmd_del_edge(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    // :graph del-edge <graph> <edge_id>
    if parts.len() < 4 {
        report(had_error, fmt, ":graph del-edge <graph> <edge_id>");
        return;
    }
    let graph = parts[2];
    let edge_id = match parse_id(had_error, fmt, parts[3], "edge_id") {
        Some(n) => n,
        None => return,
    };
    match db.graph() {
        Ok(g) => match g.delete_edge(graph, edge_id) {
            Ok(deleted) => {
                if fmt == OutputFormat::Json {
                    println!("{}", serde_json::json!({"ok":true,"deleted":deleted}));
                } else if deleted {
                    println!("边 {} 已删除", edge_id);
                } else {
                    println!("边 {} 不存在", edge_id);
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

// ── 读命令 ────────────────────────────────────────────────────────────────

fn cmd_edges(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    // :graph edges <graph> <vertex_id> [out|in]
    if parts.len() < 4 {
        report(had_error, fmt, ":graph edges <graph> <vertex_id> [out|in]");
        return;
    }
    let graph = parts[2];
    let vertex_id = match parse_id(had_error, fmt, parts[3], "vertex_id") {
        Some(n) => n,
        None => return,
    };
    let is_out = match parts.get(4).copied().unwrap_or("out") {
        "in" => false,
        _ => true,
    };
    match db.graph_read() {
        Ok(g) => {
            let result = if is_out {
                g.out_edges(graph, vertex_id)
            } else {
                g.in_edges(graph, vertex_id)
            };
            match result {
                Ok(edges) => {
                    if fmt == OutputFormat::Json {
                        let arr: Vec<serde_json::Value> = edges
                            .iter()
                            .map(|e| {
                                serde_json::json!({
                                    "id": e.id,
                                    "from": e.from,
                                    "to": e.to,
                                    "label": e.label,
                                    "properties": e.properties
                                })
                            })
                            .collect();
                        println!(
                            "{}",
                            serde_json::json!({"ok":true,"edges":arr,"count":arr.len()})
                        );
                    } else {
                        for e in &edges {
                            let props =
                                serde_json::to_string(&e.properties).unwrap_or_default();
                            println!(
                                "  edge id={} {} → {} label={} props={}",
                                e.id, e.from, e.to, e.label, props
                            );
                        }
                        println!("({} 条边)", edges.len());
                    }
                }
                Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
            }
        }
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

fn cmd_path(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    // :graph path <graph> <from> <to> [max_depth]
    if parts.len() < 5 {
        report(had_error, fmt, ":graph path <graph> <from> <to> [max_depth]");
        return;
    }
    let graph = parts[2];
    let from = match parse_id(had_error, fmt, parts[3], "from") {
        Some(n) => n,
        None => return,
    };
    let to = match parse_id(had_error, fmt, parts[4], "to") {
        Some(n) => n,
        None => return,
    };
    let max_depth: usize = parts.get(5).and_then(|s| s.parse().ok()).unwrap_or(10);
    match db.graph_read() {
        Ok(g) => match g.shortest_path(graph, from, to, max_depth) {
            Ok(Some(path)) => {
                if fmt == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"path":path,"hops":path.len()-1})
                    );
                } else {
                    let s: Vec<String> = path.iter().map(|id| id.to_string()).collect();
                    println!("路径 ({} 跳): {}", path.len() - 1, s.join(" → "));
                }
            }
            Ok(None) => {
                if fmt == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"path":null,"message":"无可达路径"})
                    );
                } else {
                    println!("无可达路径（{} → {}，max_depth={}）", from, to, max_depth);
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

fn cmd_pagerank(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    // :graph pagerank <graph> [limit]
    if parts.len() < 3 {
        report(had_error, fmt, ":graph pagerank <graph> [limit]");
        return;
    }
    let graph = parts[2];
    let limit: usize = parts.get(3).and_then(|s| s.parse().ok()).unwrap_or(10);
    match db.graph_read() {
        Ok(g) => match g.pagerank(graph, 0.85, 20, limit) {
            Ok(ranks) => {
                if fmt == OutputFormat::Json {
                    let arr: Vec<serde_json::Value> = ranks
                        .iter()
                        .map(|(id, score)| serde_json::json!({"id":id,"score":score}))
                        .collect();
                    println!("{}", serde_json::json!({"ok":true,"results":arr}));
                } else {
                    for (rank, (id, score)) in ranks.iter().enumerate() {
                        println!("  #{} vertex {} score={:.6}", rank + 1, id, score);
                    }
                    println!("({} 个节点)", ranks.len());
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
