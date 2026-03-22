//! SQL 引擎命令处理。

use crate::format::format_value;
use crate::OutputFormat;
use std::sync::atomic::{AtomicBool, Ordering};
use talon::Talon;

/// 执行 SQL 语句并打印结果。
pub fn handle(db: &Talon, input: &str, fmt: OutputFormat, had_error: &AtomicBool) {
    let sql = input.trim_end_matches(';');
    match db.run_sql(sql) {
        Ok(rows) => {
            if fmt == OutputFormat::Json {
                let json_rows: Vec<Vec<serde_json::Value>> = rows
                    .iter()
                    .map(|row| row.iter().map(|v| value_to_json(v)).collect())
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
        Err(e) => {
            had_error.store(true, Ordering::Relaxed);
            if fmt == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({"ok":false,"error":format!("SQL 错误: {}", e)})
                );
            } else {
                eprintln!("SQL 错误: {}", e);
            }
        }
    }
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
