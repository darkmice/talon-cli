//! 集群运维命令处理。

use crate::OutputFormat;
use std::sync::atomic::{AtomicBool, Ordering};
use talon::Talon;

/// 处理 `:cluster` 子命令。
pub fn handle(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 2 {
        report(had_error, fmt, ":cluster 需要子命令。输入 :help 查看。");
        return;
    }
    match parts[1] {
        "status"    => cmd_status(db, fmt, had_error),
        "role"      => cmd_role(db, fmt, had_error),
        "epoch"     => cmd_epoch(db, fmt, had_error),
        "promote"   => cmd_promote(db, fmt, had_error),
        "step-down" => cmd_step_down(db, parts, fmt, had_error),
        _ => report(
            had_error,
            fmt,
            &format!("未知 cluster 子命令: {}", parts[1]),
        ),
    }
}

// ── status ────────────────────────────────────────────────────────────────────

fn cmd_status(db: &Talon, fmt: OutputFormat, had_error: &AtomicBool) {
    let status = db.cluster_status();
    if fmt == OutputFormat::Json {
        match serde_json::to_value(&status) {
            Ok(v) => println!("{}", serde_json::json!({"ok":true,"status":v})),
            Err(e) => report(had_error, fmt, &format!("序列化错误: {}", e)),
        }
    } else {
        let role_str = format!("{:?}", status.role);
        println!("角色:          {}", role_str);
        println!("Epoch:         {}", status.epoch);
        println!("当前 LSN:      {}", status.current_lsn);
        println!("最小 LSN:      {}", status.min_lsn);
        println!("OpLog 条目:    {}", status.oplog_entries);
        if status.replicas.is_empty() {
            println!("副本节点:      (无)");
        } else {
            println!("副本节点 ({}):", status.replicas.len());
            for r in &status.replicas {
                println!(
                    "  addr={}  confirmed_lsn={}  lag={}  lag_ms={}ms  last_heartbeat={}ms",
                    r.addr, r.confirmed_lsn, r.lag, r.lag_ms, r.last_heartbeat_ms
                );
            }
        }
    }
}

// ── role ──────────────────────────────────────────────────────────────────────

fn cmd_role(db: &Talon, fmt: OutputFormat, had_error: &AtomicBool) {
    let role = db.cluster_role();
    if fmt == OutputFormat::Json {
        match serde_json::to_value(&role) {
            Ok(v) => println!("{}", serde_json::json!({"ok":true,"role":v})),
            Err(e) => report(had_error, fmt, &format!("序列化错误: {}", e)),
        }
    } else {
        println!("角色: {:?}", role);
    }
}

// ── epoch ─────────────────────────────────────────────────────────────────────

fn cmd_epoch(db: &Talon, fmt: OutputFormat, _had_error: &AtomicBool) {
    let epoch = db.cluster_epoch();
    if fmt == OutputFormat::Json {
        println!("{}", serde_json::json!({"ok":true,"epoch":epoch}));
    } else {
        println!("Epoch: {}", epoch);
    }
}

// ── promote ───────────────────────────────────────────────────────────────────

fn cmd_promote(db: &Talon, fmt: OutputFormat, had_error: &AtomicBool) {
    match db.promote() {
        Ok(()) => {
            if fmt == OutputFormat::Json {
                println!("{}", serde_json::json!({"ok":true,"promoted":true,"role":"Primary"}));
            } else {
                println!("OK — 已提升为 Primary");
            }
        }
        Err(e) => report(had_error, fmt, &format!("promote 失败: {}", e)),
    }
}

// ── step-down ─────────────────────────────────────────────────────────────────

fn cmd_step_down(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    // :cluster step-down <new_primary_addr> <observed_epoch>
    if parts.len() < 4 {
        report(
            had_error,
            fmt,
            ":cluster step-down <new_primary_addr> <observed_epoch>",
        );
        return;
    }
    let addr = parts[2].to_string();
    let observed_epoch: u64 = match parts[3].parse() {
        Ok(n) => n,
        Err(_) => {
            report(had_error, fmt, "observed_epoch 必须为无符号整数");
            return;
        }
    };
    match db.step_down(addr.clone(), observed_epoch) {
        Ok(()) => {
            if fmt == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "stepped_down": true,
                        "new_primary": addr,
                        "observed_epoch": observed_epoch
                    })
                );
            } else {
                println!(
                    "OK — 已降为 Replica，新主节点: {}（epoch={}）",
                    addr, observed_epoch
                );
            }
        }
        Err(e) => report(had_error, fmt, &format!("step-down 失败: {}", e)),
    }
}

// ── 工具函数 ──────────────────────────────────────────────────────────────────

fn report(had_error: &AtomicBool, fmt: OutputFormat, msg: &str) {
    had_error.store(true, Ordering::Relaxed);
    if fmt == OutputFormat::Json {
        println!("{}", serde_json::json!({"ok":false,"error":msg}));
    } else {
        eprintln!("{}", msg);
    }
}
