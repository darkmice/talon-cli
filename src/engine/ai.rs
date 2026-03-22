//! AI 引擎命令处理 — Session / Context / Memory / RAG。
//!
//! 通过 `talon_ai::TalonAiExt` trait 访问 AI 引擎。

use crate::OutputFormat;
use std::sync::atomic::{AtomicBool, Ordering};
use talon::Talon;
use talon_ai::TalonAiExt;

/// 处理 `:ai` 子命令。
pub fn handle(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 2 {
        report(had_error, fmt, ":ai 需要子命令。输入 :help 查看。");
        return;
    }
    match parts[1] {
        "sessions" => cmd_sessions(db, fmt, had_error),
        "session" => cmd_session(db, parts, fmt, had_error),
        "history" => cmd_history(db, parts, fmt, had_error),
        "memory" => cmd_memory(db, parts, fmt, had_error),
        "docs" | "rag" => cmd_rag(db, parts, fmt, had_error),
        _ => report(
            had_error,
            fmt,
            &format!("未知 AI 子命令: {}。可用: sessions/session/history/memory/docs", parts[1]),
        ),
    }
}

fn cmd_sessions(db: &Talon, fmt: OutputFormat, had_error: &AtomicBool) {
    let ai = match db.ai_read() {
        Ok(ai) => ai,
        Err(e) => {
            report(had_error, fmt, &format!("AI 引擎初始化失败: {}", e));
            return;
        }
    };
    match ai.list_sessions() {
        Ok(sessions) => {
            if fmt == OutputFormat::Json {
                let list: Vec<serde_json::Value> = sessions
                    .iter()
                    .map(|s| {
                        serde_json::json!({
                            "id": s.id,
                            "created_at": s.created_at,
                            "archived": s.archived,
                            "expires_at": s.expires_at,
                            "metadata": s.metadata,
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::json!({"ok":true,"sessions":list,"count":list.len()})
                );
            } else if sessions.is_empty() {
                println!("(无活跃 Session)");
            } else {
                for s in &sessions {
                    let meta_str = if s.metadata.is_empty() {
                        String::new()
                    } else {
                        format!(
                            " | {}",
                            s.metadata
                                .iter()
                                .map(|(k, v)| format!("{}={}", k, v))
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    };
                    println!("  {} (created: {}){}", s.id, s.created_at, meta_str);
                }
                println!("({} 个 Session)", sessions.len());
            }
        }
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

fn cmd_session(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 3 {
        report(had_error, fmt, ":ai session <id>");
        return;
    }
    let ai = match db.ai_read() {
        Ok(ai) => ai,
        Err(e) => {
            report(had_error, fmt, &format!("AI 引擎初始化失败: {}", e));
            return;
        }
    };
    match ai.get_session(parts[2]) {
        Ok(Some(s)) => {
            if fmt == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "id": s.id,
                        "created_at": s.created_at,
                        "archived": s.archived,
                        "expires_at": s.expires_at,
                        "metadata": s.metadata,
                    })
                );
            } else {
                println!("  ID: {}", s.id);
                println!("  创建时间: {}", s.created_at);
                println!("  归档: {}", s.archived);
                if let Some(exp) = s.expires_at {
                    println!("  过期时间: {}", exp);
                }
                if !s.metadata.is_empty() {
                    println!("  元数据:");
                    for (k, v) in &s.metadata {
                        println!("    {}={}", k, v);
                    }
                }
            }
        }
        Ok(None) => {
            if fmt == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({"ok":true,"id":parts[2],"found":false})
                );
            } else {
                println!("(未找到 Session: {})", parts[2]);
            }
        }
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

fn cmd_history(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 3 {
        report(had_error, fmt, ":ai history <session_id> [limit]");
        return;
    }
    let session_id = parts[2];
    // parts[3] 可能包含 limit（在 splitn(4, ' ') 下是 "10" 等）
    let limit: Option<usize> = parts
        .get(3)
        .and_then(|s| s.split_whitespace().next())
        .and_then(|s| s.parse().ok());

    let ai = match db.ai_read() {
        Ok(ai) => ai,
        Err(e) => {
            report(had_error, fmt, &format!("AI 引擎初始化失败: {}", e));
            return;
        }
    };
    match ai.get_history(session_id, limit) {
        Ok(messages) => {
            if fmt == OutputFormat::Json {
                let list: Vec<serde_json::Value> = messages
                    .iter()
                    .map(|m| {
                        serde_json::json!({
                            "role": m.role,
                            "content": m.content,
                            "timestamp": m.timestamp,
                            "token_count": m.token_count,
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::json!({"ok":true,"messages":list,"count":list.len()})
                );
            } else if messages.is_empty() {
                println!("(无消息)");
            } else {
                for m in &messages {
                    let content = if m.content.len() > 120 {
                        format!("{}...", &m.content[..120])
                    } else {
                        m.content.clone()
                    };
                    println!(
                        "  [{}] {} (tokens: {})",
                        m.role,
                        content,
                        m.token_count.unwrap_or(0)
                    );
                }
                println!("({} 条消息)", messages.len());
            }
        }
        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
    }
}

fn cmd_memory(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    let sub = parts.get(2).copied().unwrap_or("count");
    let ai = match db.ai_read() {
        Ok(ai) => ai,
        Err(e) => {
            report(had_error, fmt, &format!("AI 引擎初始化失败: {}", e));
            return;
        }
    };
    match sub {
        "count" => match ai.memory_count() {
            Ok(n) => {
                if fmt == OutputFormat::Json {
                    println!("{}", serde_json::json!({"ok":true,"memory_count":n}));
                } else {
                    println!("记忆数量: {}", n);
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        _ => report(
            had_error,
            fmt,
            &format!("未知 AI memory 子命令: {}。可用: count", sub),
        ),
    }
}

fn cmd_rag(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    let sub = parts.get(2).copied().unwrap_or("count");
    let ai = match db.ai_read() {
        Ok(ai) => ai,
        Err(e) => {
            report(had_error, fmt, &format!("AI 引擎初始化失败: {}", e));
            return;
        }
    };
    match sub {
        "count" | "doc" => match ai.document_count() {
            Ok(n) => {
                if fmt == OutputFormat::Json {
                    println!("{}", serde_json::json!({"ok":true,"rag_doc_count":n}));
                } else {
                    println!("RAG 文档数: {}", n);
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        "list" | "docs" => match ai.list_documents() {
            Ok(docs) => {
                if fmt == OutputFormat::Json {
                    let list: Vec<serde_json::Value> = docs
                        .iter()
                        .map(|d| {
                            serde_json::json!({
                                "id": d.id,
                                "source": d.source,
                                "chunk_count": d.chunk_count,
                                "created_at": d.created_at,
                                "metadata": format!("{:?}", d.metadata),
                            })
                        })
                        .collect::<Vec<serde_json::Value>>();
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"documents":list,"count":list.len()})
                    );
                } else if docs.is_empty() {
                    println!("(无 RAG 文档)");
                } else {
                    for d in &docs {
                        println!(
                            "  [{}] source={} (chunks={})",
                            d.id, d.source, d.chunk_count
                        );
                    }
                    println!("({} 个文档)", docs.len());
                }
            }
            Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
        },
        _ => report(
            had_error,
            fmt,
            &format!("未知 AI docs 子命令: {}。可用: count/list", sub),
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
