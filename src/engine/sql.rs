//! SQL 引擎命令处理。

use crate::format::format_value;
use crate::OutputFormat;
use std::sync::atomic::{AtomicBool, Ordering};
use talon::Talon;

/// 执行 SQL 语句并打印结果。
pub fn handle(db: &Talon, input: &str, fmt: OutputFormat, had_error: &AtomicBool) {
    let sql = input.trim_end_matches(';');
    match db.run_sql(sql) {
        Ok(rows) => print_rows(&rows, fmt),
        Err(e) => {
            had_error.store(true, Ordering::Relaxed);
            print_error(&format!("SQL 错误: {}", e), fmt);
        }
    }
}

/// 处理 `:sql` 子命令（事务控制 + 参数化查询 + 裸 SQL 统一入口）。
///
/// 子命令：
/// - `:sql begin`                                  → BEGIN
/// - `:sql commit`                                 → COMMIT
/// - `:sql rollback`                               → ROLLBACK
/// - `:sql exec <sql...>`                          → 裸 SQL（可省略引号）
/// - `:sql param "<sql with ?>" <v1> <v2> ...`     → 参数化查询（仅嵌入模式）
///
/// 注意：`:sql param` 使用 `db.run_sql_param`，网络模式后端不支持 bind 字段，
/// 请勿在 net.rs/input_to_json 中转发此子命令；begin/commit/rollback 可翻译为
/// 裸 SQL 帧 `{"module":"sql","action":"query","params":{"sql":"BEGIN"}}` 走网络。
pub fn handle_cmd(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    // parts[0] == ":sql"，parts[1] 是子命令
    let sub = match parts.get(1) {
        Some(s) => *s,
        None => {
            had_error.store(true, Ordering::Relaxed);
            print_error(":sql 需要子命令: begin | commit | rollback | exec | param", fmt);
            return;
        }
    };

    match sub {
        "begin" => run_control(db, "BEGIN", fmt, had_error),
        "commit" => run_control(db, "COMMIT", fmt, had_error),
        "rollback" => run_control(db, "ROLLBACK", fmt, had_error),

        "exec" => {
            if parts.len() < 3 {
                had_error.store(true, Ordering::Relaxed);
                print_error(":sql exec <sql...>", fmt);
                return;
            }
            // parts[2..] 拼回 SQL（分词器已处理引号，直接 join）
            let sql = parts[2..].join(" ");
            let sql = sql.trim_end_matches(';');
            match db.run_sql(sql) {
                Ok(rows) => print_rows(&rows, fmt),
                Err(e) => {
                    had_error.store(true, Ordering::Relaxed);
                    print_error(&format!("SQL 错误: {}", e), fmt);
                }
            }
        }

        "param" => {
            // parts[2] = SQL（引号包裹整段，分词器已剥去引号）
            // parts[3..] = 参数值列表
            if parts.len() < 3 {
                had_error.store(true, Ordering::Relaxed);
                print_error(":sql param \"<sql>\" [v1 v2 ...]", fmt);
                return;
            }
            let sql = parts[2].trim_end_matches(';');
            let params: Vec<talon::Value> = parts[3..].iter().map(|s| parse_param(s)).collect();
            match db.run_sql_param(sql, &params) {
                Ok(rows) => print_rows(&rows, fmt),
                Err(e) => {
                    had_error.store(true, Ordering::Relaxed);
                    print_error(&format!("SQL 错误: {}", e), fmt);
                }
            }
        }

        other => {
            had_error.store(true, Ordering::Relaxed);
            print_error(
                &format!("未知 :sql 子命令 '{}': begin | commit | rollback | exec | param", other),
                fmt,
            );
        }
    }
}

// ── 内部工具函数 ─────────────────────────────────────────────────────────────

/// 执行事务控制语句（BEGIN / COMMIT / ROLLBACK），打印 OK 或错误。
fn run_control(db: &Talon, sql: &str, fmt: OutputFormat, had_error: &AtomicBool) {
    match db.run_sql(sql) {
        Ok(_) => {
            if fmt == OutputFormat::Json {
                println!("{}", serde_json::json!({"ok": true, "cmd": sql}));
            } else {
                println!("OK ({})", sql);
            }
        }
        Err(e) => {
            had_error.store(true, Ordering::Relaxed);
            print_error(&format!("SQL 错误: {}", e), fmt);
        }
    }
}

/// 打印查询结果行（Human / JSON 双格式）。
fn print_rows(rows: &[Vec<talon::Value>], fmt: OutputFormat) {
    if fmt == OutputFormat::Json {
        let json_rows: Vec<Vec<serde_json::Value>> = rows
            .iter()
            .map(|row| row.iter().map(value_to_json).collect())
            .collect();
        println!(
            "{}",
            serde_json::json!({
                "ok": true,
                "rows": json_rows,
                "count": json_rows.len(),
            })
        );
    } else if rows.is_empty() {
        println!("OK (0 行)");
    } else {
        for (i, row) in rows.iter().enumerate() {
            let cols: Vec<String> = row.iter().map(|v| format_value(v)).collect();
            println!("{:>4} | {}", i + 1, cols.join(" | "));
        }
        println!("({} 行)", rows.len());
    }
}

/// 打印错误（Human → stderr，JSON → stdout）。
fn print_error(msg: &str, fmt: OutputFormat) {
    if fmt == OutputFormat::Json {
        println!("{}", serde_json::json!({"ok": false, "error": msg}));
    } else {
        eprintln!("{}", msg);
    }
}

/// 将命令行字符串参数转换为 `talon::Value`。
/// 优先解析 i64 → Integer，再试 f64 → Float，否则 Text。
fn parse_param(s: &str) -> talon::Value {
    if let Ok(n) = s.parse::<i64>() {
        return talon::Value::Integer(n);
    }
    if let Ok(f) = s.parse::<f64>() {
        return talon::Value::Float(f);
    }
    talon::Value::Text(s.to_string())
}

/// 将 Talon Value 转换为 serde_json::Value。
fn value_to_json(v: &talon::Value) -> serde_json::Value {
    match v {
        talon::Value::Null => serde_json::Value::Null,
        talon::Value::Integer(n) => serde_json::json!(n),
        talon::Value::Float(f) => serde_json::json!(f),
        talon::Value::Text(s) => serde_json::json!(s),
        talon::Value::Boolean(b) => serde_json::json!(b),
        talon::Value::Jsonb(j) => j.clone(),
        talon::Value::Vector(vec) => serde_json::json!(vec),
        talon::Value::Timestamp(t) => serde_json::json!(t),
        talon::Value::GeoPoint(lng, lat) => serde_json::json!({"lng": lng, "lat": lat}),
        talon::Value::Date(d) => serde_json::json!(d),
        talon::Value::Blob(b) => {
            use std::fmt::Write;
            let mut hex = String::with_capacity(b.len() * 2);
            for byte in b {
                write!(hex, "{:02x}", byte).ok();
            }
            serde_json::json!({"_blob_hex": hex, "_len": b.len()})
        }
        // 兜底：未来新增的 Value 变体
        _ => serde_json::json!(format!("{}", v)),
    }
}
