//! Agent 运行时命令处理 — 执行状态查询。

use crate::OutputFormat;
use std::sync::atomic::{AtomicBool, Ordering};
use talon::Talon;
use talon_agent::{
    ExecutionSessionStatus, ExecutionStatusResponse, ExecutionStatusStreamCursor,
    ExecutionStatusStreamEvent, TalonAgentExt, ToolDiscoveryQuery, ToolSource,
};
use talon_agent::tool::ToolCategory;

/// 处理 `:agent` 子命令。
pub fn handle(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 2 {
        report(had_error, fmt, ":agent 需要子命令。输入 :help 查看。");
        return;
    }

    match parts[1] {
        "discover" => cmd_discover(db, parts, fmt, had_error),
        "status" => cmd_status(db, parts, fmt, had_error),
        "checkpoint" => cmd_checkpoint(db, parts, fmt, had_error),
        "watch" => cmd_watch(db, parts, fmt, had_error),
        _ => report(
            had_error,
            fmt,
            &format!("未知 Agent 子命令: {}。可用: discover/status/checkpoint/watch", parts[1]),
        ),
    }
}

fn cmd_discover(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    let engine = match db.agent() {
        Ok(engine) => engine,
        Err(error) => {
            report(had_error, fmt, &format!("Agent 引擎初始化失败: {}", error));
            return;
        }
    };

    let query = match parse_discovery_query(parts.get(2..).unwrap_or(&[])) {
        Ok(query) => query,
        Err(error) => {
            report(had_error, fmt, &error);
            return;
        }
    };

    let response = engine.discover_builtin_tools(&query);
    if fmt == OutputFormat::Json {
        println!(
            "{}",
            serde_json::to_string(&response).unwrap_or_else(|_| {
                serde_json::json!({"ok": false, "error": "tool discovery serialization failed"})
                    .to_string()
            })
        );
        return;
    }

    if response.tools.is_empty() {
        println!("(没有匹配的工具)");
        return;
    }

    for tool in &response.tools {
        println!(
            "tool: {} category={:?} source={:?} read_only={} concurrency_safe={} invocable={}\n  {}",
            tool.name,
            tool.category,
            tool.source,
            tool.read_only,
            tool.concurrency_safe,
            tool.invocable,
            tool.description
        );
    }
    if response.has_more {
        if let Some(cursor) = &response.next_cursor {
            println!("\n(更多结果可用，使用 --cursor {} 继续)", cursor);
        }
    }
}

fn cmd_checkpoint(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 4 {
        report(
            had_error,
            fmt,
            ":agent checkpoint <trace|plan> <id> --consumer <id> [--group <id>]",
        );
        return;
    }

    let engine = match db.agent() {
        Ok(engine) => engine,
        Err(error) => {
            report(had_error, fmt, &format!("Agent 引擎初始化失败: {}", error));
            return;
        }
    };

    let query_kind = parts[2];
    let query_value = parts[3].split_whitespace().next().unwrap_or(parts[3]);
    let checkpoint_options = match parse_checkpoint_options(query_kind, query_value, &parts[4..]) {
        Ok(options) => options,
        Err(error) => {
            report(had_error, fmt, &error);
            return;
        }
    };

    let checkpoint = match query_kind {
        "trace" => engine.describe_team_execution_status_stream_consumer_by_trace_id(
            query_value,
            &checkpoint_options.group,
            &checkpoint_options.consumer,
        ),
        "plan" => engine.describe_team_execution_status_stream_consumer_by_plan_id(
            query_value,
            &checkpoint_options.group,
            &checkpoint_options.consumer,
        ),
        other => {
            report(
                had_error,
                fmt,
                &format!("未知 checkpoint 查询类型: {}。可用: trace/plan", other),
            );
            return;
        }
    };

    match checkpoint {
        Ok(checkpoint) => print_checkpoint(&checkpoint, fmt),
        Err(error) => report(had_error, fmt, &format!("查询执行状态 checkpoint 失败: {}", error)),
    }
}

fn cmd_status(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 4 {
        report(had_error, fmt, ":agent status <trace|plan> <id>");
        return;
    }

    let engine = match db.agent() {
        Ok(engine) => engine,
        Err(error) => {
            report(had_error, fmt, &format!("Agent 引擎初始化失败: {}", error));
            return;
        }
    };

    let status = match parts[2] {
        "trace" => engine.query_team_execution_status_by_trace_id(parts[3]),
        "plan" => engine.query_team_execution_status_by_plan_id(parts[3]),
        other => {
            report(
                had_error,
                fmt,
                &format!("未知 status 查询类型: {}。可用: trace/plan", other),
            );
            return;
        }
    };

    match status {
        Ok(Some(status)) => print_status(&status, fmt),
        Ok(None) => {
            if fmt == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "found": false,
                        "queryType": parts[2],
                        "queryValue": parts[3],
                    })
                );
            } else {
                println!("(未找到执行状态: {} {})", parts[2], parts[3]);
            }
        }
        Err(error) => report(had_error, fmt, &format!("查询执行状态失败: {}", error)),
    }
}

fn cmd_watch(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 4 {
        report(had_error, fmt, ":agent watch <trace|plan> <id>");
        return;
    }

    let engine = match db.agent() {
        Ok(engine) => engine,
        Err(error) => {
            report(had_error, fmt, &format!("Agent 引擎初始化失败: {}", error));
            return;
        }
    };

    let query_kind = parts[2];
    let query_value = parts[3].split_whitespace().next().unwrap_or(parts[3]);
    let group = format!("talon-cli-watch:{}:{}", query_kind, query_value);
    let watch_options = match parse_watch_options(&parts[4..]) {
        Ok(options) => options,
        Err(error) => {
            report(had_error, fmt, &error);
            return;
        }
    };
    let consumer = watch_options
        .consumer
        .clone()
        .unwrap_or_else(default_consumer_id);

    let current = match query_kind {
        "trace" => engine.query_team_execution_status_by_trace_id(query_value),
        "plan" => engine.query_team_execution_status_by_plan_id(query_value),
        other => {
            report(
                had_error,
                fmt,
                &format!("未知 watch 查询类型: {}。可用: trace/plan", other),
            );
            return;
        }
    };

    match current {
        Ok(Some(status)) => {
            print_status(&status, fmt);
            if watch_options.from_seq.is_none()
                && watch_options.replay_last.is_none()
                && is_terminal_status(status.status)
            {
                return;
            }
        }
        Ok(None) => {
            if fmt == OutputFormat::Human {
                println!("watching {} {} ...", query_kind, query_value);
            }
        }
        Err(error) => {
            report(had_error, fmt, &format!("查询执行状态失败: {}", error));
            return;
        }
    }

    let subscribe = match query_kind {
        "trace" => engine.subscribe_team_execution_status_by_trace_id(query_value, &group),
        "plan" => engine.subscribe_team_execution_status_by_plan_id(query_value, &group),
        _ => unreachable!(),
    };
    if let Err(error) = subscribe {
        report(had_error, fmt, &format!("订阅执行状态失败: {}", error));
        return;
    }

    if let Some(from_seq) = watch_options.from_seq {
        let reset = match query_kind {
            "trace" => engine.reset_team_execution_status_stream_consumer_by_trace_id(
                query_value,
                &group,
                &consumer,
                from_seq,
            ),
            "plan" => engine.reset_team_execution_status_stream_consumer_by_plan_id(
                query_value,
                &group,
                &consumer,
                from_seq,
            ),
            _ => unreachable!(),
        };
        if let Err(error) = reset {
            report(had_error, fmt, &format!("重置执行状态消费位点失败: {}", error));
            return;
        }
    } else if let Some(window_size) = watch_options.replay_last {
        let replay = match query_kind {
            "trace" => engine.replay_team_execution_status_stream_consumer_by_trace_id(
                query_value,
                &group,
                &consumer,
                window_size,
            ),
            "plan" => engine.replay_team_execution_status_stream_consumer_by_plan_id(
                query_value,
                &group,
                &consumer,
                window_size,
            ),
            _ => unreachable!(),
        };
        if let Err(error) = replay {
            report(had_error, fmt, &format!("回放执行状态窗口失败: {}", error));
            return;
        }
    }

    loop {
        let batch = match query_kind {
            "trace" => engine.poll_team_execution_status_stream_events_by_trace_id(
                query_value,
                &group,
                &consumer,
                16,
                1_000,
            ),
            "plan" => engine.poll_team_execution_status_stream_events_by_plan_id(
                query_value,
                &group,
                &consumer,
                16,
                1_000,
            ),
            _ => unreachable!(),
        };

        match batch {
            Ok(events) => {
                for event in events {
                    print_stream_event(&event, fmt);
                    if event.terminal {
                        return;
                    }
                }
            }
            Err(error) => {
                report(had_error, fmt, &format!("消费执行状态流失败: {}", error));
                return;
            }
        }
    }
}

fn print_stream_event(event: &ExecutionStatusStreamEvent, fmt: OutputFormat) {
    if fmt == OutputFormat::Json {
        println!("{}", serde_json::to_string(event).unwrap_or_else(|_| {
            serde_json::json!({"ok": false, "error": "execution status stream serialization failed"})
                .to_string()
        }));
        return;
    }

    println!(
        "stream: id={} version={} scope={:?} scope_id={} seq={} event={} terminal={} published_at={}{}",
        event.event_id,
        event.stream_version,
        event.scope,
        event.scope_id,
        event.seq,
        stream_event_type_label(event),
        event.terminal,
        event.published_at,
        if event.changed_task_ids.is_empty() {
            String::new()
        } else {
            format!(" changed_tasks={}", event.changed_task_ids.join(","))
        }
    );
    print_status(&event.status, fmt);
}

fn print_checkpoint(checkpoint: &ExecutionStatusStreamCursor, fmt: OutputFormat) {
    if fmt == OutputFormat::Json {
        println!("{}", serde_json::to_string(checkpoint).unwrap_or_else(|_| {
            serde_json::json!({"ok": false, "error": "execution status checkpoint serialization failed"})
                .to_string()
        }));
        return;
    }

    println!(
        "checkpoint: scope={:?} scope_id={} group={} consumer={} seen={} dedupe={:?}",
        checkpoint.scope,
        checkpoint.scope_id,
        checkpoint.group,
        checkpoint.consumer,
        checkpoint.consumer_seen,
        checkpoint.dedupe_strategy
    );
    println!(
        "cursor: last_acked_seq={} next_seq={} pending_count={}",
        checkpoint
            .last_acked_seq
            .map(|value| value.to_string())
            .unwrap_or_else(|| "n/a".to_string()),
        checkpoint.next_seq,
        checkpoint.pending_count
    );
}

fn print_status(status: &ExecutionStatusResponse, fmt: OutputFormat) {
    if fmt == OutputFormat::Json {
        println!("{}", serde_json::to_string(status).unwrap_or_else(|_| {
            serde_json::json!({"ok": false, "error": "execution status serialization failed"})
                .to_string()
        }));
        return;
    }

    println!("session: {}", status.session_id);
    println!("plan: {}", status.task_id);
    println!("teamTrace: {}", status.team_id);
    println!("status: {:?}", status.status);
    println!("phase: {}", status.phase);
    println!("progress: {}%", status.progress);
    println!(
        "metrics: total={} completed={} failed={} blocked={} running={} avg={}ms total={}ms max_parallel={}",
        status.metrics.total_tasks,
        status.metrics.completed_tasks,
        status.metrics.failed_tasks,
        status.metrics.blocked_tasks,
        status.metrics.in_flight_tasks,
        status.metrics.average_task_time,
        status.metrics.total_time,
        status
            .metrics
            .max_parallel_tasks
            .map(|value| value.to_string())
            .unwrap_or_else(|| "n/a".to_string())
    );

    if !status.metrics.role_parallel_limits.is_empty() {
        println!("role_limits:");
        for (role, limit) in &status.metrics.role_parallel_limits {
            println!("  {}={}", role, limit);
        }
    }

    if !status.metrics.capacity_scope_limits.is_empty() {
        println!("capacity_scope_limits:");
        for (scope, limit) in &status.metrics.capacity_scope_limits {
            println!("  {}={}", scope, limit);
        }
    }

    println!("tasks:");
    for task in &status.tasks {
        println!(
            "  - {} [{}] role={} progress={}{}",
            task.id,
            format!("{:?}", task.status).to_lowercase(),
            task.assigned_to,
            task.progress,
            task.capacity_scope
                .as_ref()
                .map(|scope| format!(" scope={}", scope))
                .unwrap_or_default()
        );
        if let Some(trigger) = &task.trigger {
            println!("    trigger={} ref={:?}", trigger.trigger_type, trigger.trigger_ref);
        }
        if let Some(error) = &task.error {
            println!("    error={}", error);
        }
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

fn is_terminal_status(status: ExecutionSessionStatus) -> bool {
    matches!(
        status,
        ExecutionSessionStatus::Completed
            | ExecutionSessionStatus::Failed
            | ExecutionSessionStatus::Stopped
    )
}

fn default_consumer_id() -> String {
    let pid = std::process::id();
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("cli-{}-{}", pid, now_ms)
}

#[derive(Debug, Default)]
struct WatchOptions {
    consumer: Option<String>,
    from_seq: Option<u64>,
    replay_last: Option<usize>,
}

#[derive(Debug)]
struct CheckpointOptions {
    group: String,
    consumer: String,
}

fn parse_discovery_query(parts: &[&str]) -> Result<ToolDiscoveryQuery, String> {
    let tail = parts.join(" ");
    let mut categories = Vec::new();
    let mut read_only_only = false;
    let mut concurrency_safe_only = false;
    let mut limit = None;
    let mut source = None;
    let mut cursor = None;
    let mut query_tokens = Vec::new();

    let mut tokens = tail.split_whitespace();
    while let Some(token) = tokens.next() {
        match token {
            "--readonly" => read_only_only = true,
            "--concurrency-safe" => concurrency_safe_only = true,
            "--limit" => {
                let value = tokens
                    .next()
                    .ok_or_else(|| "discover 需要 --limit <n>".to_string())?;
                let parsed = value
                    .parse::<usize>()
                    .map_err(|_| format!("无效的 limit: {}", value))?;
                limit = Some(parsed);
            }
            "--source" => {
                let value = tokens
                    .next()
                    .ok_or_else(|| "discover 需要 --source <builtin|skill|mcp|dynamic>".to_string())?;
                let parsed = match value {
                    "builtin" => ToolSource::Builtin,
                    "skill" => ToolSource::Skill,
                    "mcp" => ToolSource::Mcp,
                    "dynamic" => ToolSource::Dynamic,
                    other => {
                        return Err(format!(
                            "未知 tool source: {}。可用: builtin/skill/mcp/dynamic",
                            other
                        ))
                    }
                };
                source = Some(parsed);
            }
            "--cursor" => {
                let value = tokens
                    .next()
                    .ok_or_else(|| "discover 需要 --cursor <value>".to_string())?;
                cursor = Some(value.to_string());
            }
            "--category" => {
                let value = tokens
                    .next()
                    .ok_or_else(|| "discover 需要 --category <name>".to_string())?;
                let category = match value {
                    "query" => ToolCategory::Query,
                    "mutation" => ToolCategory::Mutation,
                    "system" => ToolCategory::System,
                    "communication" => ToolCategory::Communication,
                    "code_execution" => ToolCategory::CodeExecution,
                    other => {
                        return Err(format!(
                            "未知 tool category: {}。可用: query/mutation/system/communication/code_execution",
                            other
                        ))
                    }
                };
                categories.push(category);
            }
            other => query_tokens.push(other.to_string()),
        }
    }

    Ok(ToolDiscoveryQuery {
        query_text: if query_tokens.is_empty() {
            None
        } else {
            Some(query_tokens.join(" "))
        },
        categories,
        read_only_only,
        concurrency_safe_only,
        limit,
        source,
        cursor,
    })
}

fn parse_watch_options(parts: &[&str]) -> Result<WatchOptions, String> {
    let mut options = WatchOptions::default();
    let mut index = 0;
    while index < parts.len() {
        match parts[index] {
            "--consumer" => {
                index += 1;
                let Some(value) = parts.get(index) else {
                    return Err("--consumer 需要一个值".to_string());
                };
                options.consumer = Some((*value).to_string());
            }
            "--from-seq" => {
                index += 1;
                let Some(value) = parts.get(index) else {
                    return Err("--from-seq 需要一个正整数".to_string());
                };
                let parsed = value
                    .parse::<u64>()
                    .map_err(|_| "--from-seq 需要一个正整数".to_string())?;
                options.from_seq = Some(parsed.max(1));
            }
            "--replay-last" => {
                index += 1;
                let Some(value) = parts.get(index) else {
                    return Err("--replay-last 需要一个正整数".to_string());
                };
                let parsed = value
                    .parse::<usize>()
                    .map_err(|_| "--replay-last 需要一个正整数".to_string())?;
                options.replay_last = Some(parsed.max(1));
            }
            other => {
                return Err(format!(
                    "未知 watch 选项: {}。可用: --consumer <id> --from-seq <n> --replay-last <n>",
                    other
                ));
            }
        }
        index += 1;
    }

    if options.from_seq.is_some() && options.replay_last.is_some() {
        return Err("--from-seq 和 --replay-last 不能同时使用".to_string());
    }

    Ok(options)
}

fn parse_checkpoint_options(
    query_kind: &str,
    query_value: &str,
    parts: &[&str],
) -> Result<CheckpointOptions, String> {
    let mut group = format!("talon-cli-watch:{}:{}", query_kind, query_value);
    let mut consumer: Option<String> = None;
    let mut index = 0;

    while index < parts.len() {
        match parts[index] {
            "--group" => {
                index += 1;
                let Some(value) = parts.get(index) else {
                    return Err("--group 需要一个值".to_string());
                };
                group = (*value).to_string();
            }
            "--consumer" => {
                index += 1;
                let Some(value) = parts.get(index) else {
                    return Err("--consumer 需要一个值".to_string());
                };
                consumer = Some((*value).to_string());
            }
            other => {
                return Err(format!(
                    "未知 checkpoint 选项: {}。可用: --consumer <id> [--group <id>]",
                    other
                ));
            }
        }
        index += 1;
    }

    let consumer = consumer.ok_or_else(|| "checkpoint 查询必须提供 --consumer <id>".to_string())?;

    Ok(CheckpointOptions { group, consumer })
}

fn stream_event_type_label(event: &ExecutionStatusStreamEvent) -> &'static str {
    match event.event_type {
        talon_agent::ExecutionStatusStreamEventType::InitialSnapshot => "initial_snapshot",
        talon_agent::ExecutionStatusStreamEventType::RunningSnapshot => "running_snapshot",
        talon_agent::ExecutionStatusStreamEventType::ProgressSnapshot => "progress_snapshot",
        talon_agent::ExecutionStatusStreamEventType::FinalSnapshot => "final_snapshot",
    }
}