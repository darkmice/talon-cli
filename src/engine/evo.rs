//! EvoCore 引擎命令处理 — Soul / 个性 / 进化历史（只读查询）。
//!
//! 注意：CLI 不启动 EvoCore 认知循环（太重），
//! 而是直接从 KV 读取持久化的 Soul/进化数据做只读展示。

use crate::OutputFormat;
use evo_core::ToolCache; // trait 提供 get_cached_tool_result
use std::sync::atomic::{AtomicBool, Ordering};
use talon::Talon;

// EvoCore 类型（仅做反序列化）
use evo_core::{Soul, SoulEvolutionProposal};

// 与 evo_core::soul.rs 中定义一致的 KV key
const SOUL_KV_KEY: &str = "evocore:soul";
const PERSONALITY_KV_KEY: &str = "evocore:personality";
const LEARN_COUNT_KEY: &str = "evocore:learn_count";
const SOUL_PROPOSALS_KEY: &str = "evocore:soul:proposals";

/// 处理 `:evo` 子命令。
pub fn handle(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 2 {
        report(had_error, fmt, ":evo 需要子命令。可用: soul/personality/history/proposals/stats");
        return;
    }
    match parts[1] {
        "soul" => cmd_soul(db, fmt, had_error),
        "personality" => cmd_personality(db, fmt, had_error),
        "history" => cmd_history(db, fmt, had_error),
        "proposals" => cmd_proposals(db, fmt, had_error),
        "stats" => cmd_stats(db, fmt, had_error),
        _ => report(
            had_error,
            fmt,
            &format!("未知 EvoCore 子命令: {}。可用: soul/personality/history/proposals/stats", parts[1]),
        ),
    }
}

fn cmd_soul(db: &Talon, fmt: OutputFormat, had_error: &AtomicBool) {
    match db.get_cached_tool_result("evocore", SOUL_KV_KEY) {
        Ok(Some(entry)) => match serde_json::from_str::<Soul>(&entry.result) {
            Ok(soul) => {
                if fmt == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({
                            "ok": true,
                            "name": soul.identity.name,
                            "personality_type": format!("{:?}", soul.identity.personality_type),
                            "comm_style": format!("{:?}", soul.identity.comm_style),
                            "vibe": format!("{:?}", soul.vibe),
                            "mission": soul.identity.mission,
                            "emoji": soul.identity.emoji,
                            "core_truths": soul.core_truths.iter()
                                .map(|t| serde_json::json!({"principle": t.principle, "weight": t.weight}))
                                .collect::<Vec<_>>(),
                            "boundaries": soul.boundaries,
                            "evolution_version": soul.evolution.version,
                            "evolution_count": soul.evolution.accepted.len(),
                        })
                    );
                } else {
                    println!("🧠 Soul — {}", soul.identity.name);
                    println!("  人格类型: {:?}", soul.identity.personality_type);
                    println!("  沟通风格: {:?}", soul.identity.comm_style);
                    println!("  气质: {:?}", soul.vibe);
                    if let Some(ref emoji) = soul.identity.emoji {
                        println!("  表情: {}", emoji);
                    }
                    println!("  使命: {}", soul.identity.mission);
                    if !soul.core_truths.is_empty() {
                        println!("  核心真理:");
                        for t in &soul.core_truths {
                            if (t.weight - 1.0).abs() < f64::EPSILON {
                                println!("    - {}", t.principle);
                            } else {
                                println!("    - {} (weight: {:.1})", t.principle, t.weight);
                            }
                        }
                    }
                    if !soul.boundaries.is_empty() {
                        println!("  边界:");
                        for b in &soul.boundaries {
                            println!("    - {}", b);
                        }
                    }
                    println!("  进化版本: v{}", soul.evolution.version);
                    if !soul.evolution.accepted.is_empty() {
                        println!("  进化历史: {} 次", soul.evolution.accepted.len());
                    }
                }
            }
            Err(e) => report(had_error, fmt, &format!("Soul 反序列化失败: {}", e)),
        },
        Ok(None) => {
            if fmt == OutputFormat::Json {
                println!("{}", serde_json::json!({"ok":true,"soul":null,"message":"未配置 Soul（使用默认）"}));
            } else {
                println!("(未配置 Soul — 使用默认 EvoCore Soul)");
            }
        }
        Err(e) => report(had_error, fmt, &format!("读取 Soul 失败: {}", e)),
    }
}

fn cmd_personality(db: &Talon, fmt: OutputFormat, had_error: &AtomicBool) {
    match db.get_cached_tool_result("evocore", PERSONALITY_KV_KEY) {
        Ok(Some(entry)) => {
            match serde_json::from_str::<std::collections::BTreeMap<String, f64>>(&entry.result) {
                Ok(dims) => {
                    if fmt == OutputFormat::Json {
                        println!(
                            "{}",
                            serde_json::json!({"ok":true,"personality":dims})
                        );
                    } else {
                        println!("个性维度:");
                        for (name, val) in &dims {
                            let bar = personality_bar(*val);
                            println!("  {} {:.2} {}", name, val, bar);
                        }
                    }
                }
                Err(e) => report(had_error, fmt, &format!("个性数据解析失败: {}", e)),
            }
        }
        Ok(None) => {
            if fmt == OutputFormat::Json {
                println!("{}", serde_json::json!({"ok":true,"personality":null,"message":"未进化（使用初始值）"}));
            } else {
                println!("(未进化 — 使用初始个性值)");
            }
        }
        Err(e) => report(had_error, fmt, &format!("读取个性数据失败: {}", e)),
    }
}

fn cmd_history(db: &Talon, fmt: OutputFormat, had_error: &AtomicBool) {
    match db.get_cached_tool_result("evocore", SOUL_KV_KEY) {
        Ok(Some(entry)) => match serde_json::from_str::<Soul>(&entry.result) {
            Ok(soul) => {
                if fmt == OutputFormat::Json {
                    let records: Vec<serde_json::Value> = soul.evolution.accepted.iter()
                        .map(|r| serde_json::json!({
                            "version": r.version,
                            "reason": r.reason,
                            "changes": r.changes,
                            "timestamp": r.timestamp,
                        }))
                        .collect();
                    println!(
                        "{}",
                        serde_json::json!({
                            "ok": true,
                            "current_version": soul.evolution.version,
                            "records": records,
                            "count": records.len(),
                        })
                    );
                } else if soul.evolution.accepted.is_empty() {
                    println!("(无进化历史 — Soul v{})", soul.evolution.version);
                } else {
                    println!("Soul 进化历史 (当前 v{}):", soul.evolution.version);
                    for r in &soul.evolution.accepted {
                        println!("  v{}: {}", r.version, r.reason);
                        for c in &r.changes {
                            println!("    - {}", c);
                        }
                    }
                }
            }
            Err(e) => report(had_error, fmt, &format!("Soul 解析失败: {}", e)),
        },
        Ok(None) => {
            if fmt == OutputFormat::Json {
                println!("{}", serde_json::json!({"ok":true,"records":[],"count":0}));
            } else {
                println!("(无进化历史)");
            }
        }
        Err(e) => report(had_error, fmt, &format!("读取 Soul 失败: {}", e)),
    }
}

fn cmd_proposals(db: &Talon, fmt: OutputFormat, had_error: &AtomicBool) {
    match db.get_cached_tool_result("evocore", SOUL_PROPOSALS_KEY) {
        Ok(Some(entry)) => {
            match serde_json::from_str::<Vec<SoulEvolutionProposal>>(&entry.result) {
                Ok(proposals) => {
                    let pending: Vec<_> = proposals.iter()
                        .filter(|p| format!("{:?}", p.status) == "Pending")
                        .collect();
                    if fmt == OutputFormat::Json {
                        let list: Vec<serde_json::Value> = proposals.iter()
                            .map(|p| serde_json::json!({
                                "proposed_version": p.proposed_version,
                                "reason": p.reason,
                                "status": format!("{:?}", p.status),
                                "timestamp": p.timestamp,
                                "changes": p.proposed_changes.iter().map(|c| serde_json::json!({
                                    "dimension": c.dimension,
                                    "old_bias": c.old_bias,
                                    "current_value": c.current_value,
                                    "drift": c.drift,
                                })).collect::<Vec<_>>(),
                            }))
                            .collect();
                        println!(
                            "{}",
                            serde_json::json!({
                                "ok": true,
                                "proposals": list,
                                "total": list.len(),
                                "pending": pending.len(),
                            })
                        );
                    } else if proposals.is_empty() {
                        println!("(无进化提议)");
                    } else {
                        println!("Soul 进化提议 ({} 个, {} 待确认):", proposals.len(), pending.len());
                        for p in &proposals {
                            println!(
                                "  v{} [{:?}] {}",
                                p.proposed_version,
                                p.status,
                                p.reason
                            );
                            for c in &p.proposed_changes {
                                println!(
                                    "    {} {:.2} → {:.2} (drift {:.2})",
                                    c.dimension, c.old_bias, c.current_value, c.drift
                                );
                            }
                        }
                    }
                }
                Err(e) => report(had_error, fmt, &format!("提议数据解析失败: {}", e)),
            }
        }
        Ok(None) => {
            if fmt == OutputFormat::Json {
                println!("{}", serde_json::json!({"ok":true,"proposals":[],"total":0,"pending":0}));
            } else {
                println!("(无进化提议)");
            }
        }
        Err(e) => report(had_error, fmt, &format!("读取提议失败: {}", e)),
    }
}

fn cmd_stats(db: &Talon, fmt: OutputFormat, _had_error: &AtomicBool) {
    let learn_count = match db.get_cached_tool_result("evocore", LEARN_COUNT_KEY) {
        Ok(Some(entry)) => entry.result.parse::<u64>().unwrap_or(0),
        _ => 0,
    };

    let soul_version = match db.get_cached_tool_result("evocore", SOUL_KV_KEY) {
        Ok(Some(entry)) => serde_json::from_str::<Soul>(&entry.result)
            .map(|s| s.evolution.version)
            .unwrap_or(0),
        _ => 0,
    };

    let proposal_count = match db.get_cached_tool_result("evocore", SOUL_PROPOSALS_KEY) {
        Ok(Some(entry)) => serde_json::from_str::<Vec<serde_json::Value>>(&entry.result)
            .map(|v| v.len())
            .unwrap_or(0),
        _ => 0,
    };

    if fmt == OutputFormat::Json {
        println!(
            "{}",
            serde_json::json!({
                "ok": true,
                "learn_count": learn_count,
                "soul_version": soul_version,
                "proposal_count": proposal_count,
            })
        );
    } else {
        println!("EvoCore 统计:");
        println!("  学习次数: {}", learn_count);
        println!("  Soul 版本: v{}", soul_version);
        println!("  进化提议: {} 个", proposal_count);
    }
}

/// 生成个性值的可视化条形图。
fn personality_bar(val: f64) -> String {
    let clamped = val.clamp(-1.0, 1.0);
    let pos = ((clamped + 1.0) / 2.0 * 20.0) as usize;
    let mut bar = String::with_capacity(22);
    bar.push('[');
    for i in 0..20 {
        if i == pos {
            bar.push('●');
        } else if i == 10 {
            bar.push('|');
        } else {
            bar.push('─');
        }
    }
    bar.push(']');
    bar
}

fn report(had_error: &AtomicBool, fmt: OutputFormat, msg: &str) {
    had_error.store(true, Ordering::Relaxed);
    if fmt == OutputFormat::Json {
        println!("{}", serde_json::json!({"ok":false,"error":msg}));
    } else {
        eprintln!("{}", msg);
    }
}
