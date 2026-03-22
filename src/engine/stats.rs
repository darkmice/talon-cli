//! 数据库统计信息。

use crate::format::format_value;
use crate::OutputFormat;
use talon::Talon;

/// 打印数据库总览统计。
pub fn handle(db: &Talon, fmt: OutputFormat) {
    if fmt == OutputFormat::Json {
        handle_json(db);
    } else {
        handle_human(db);
    }
}

fn handle_json(db: &Talon) {
    let mut stats = serde_json::json!({"ok": true});

    // KV
    if let Ok(kv) = db.kv_read() {
        let count = kv.key_count().unwrap_or(0);
        let disk = kv.disk_space();
        stats["kv"] = serde_json::json!({"key_count": count, "disk_bytes": disk});
    }

    // SQL
    match db.run_sql("SHOW TABLES") {
        Ok(rows) => {
            let names: Vec<String> = rows.iter().map(|r| format_value(&r[0])).collect();
            stats["sql"] = serde_json::json!({"table_count": names.len(), "tables": names});
        }
        Err(_) => {
            stats["sql"] = serde_json::json!({"table_count": 0, "tables": []});
        }
    }

    // MQ
    if let Ok(mq) = db.mq_read() {
        if let Ok(topics) = mq.list_topics() {
            stats["mq"] = serde_json::json!({"topic_count": topics.len(), "topics": topics});
        }
    }

    // TS
    if let Ok(names) = talon::list_timeseries(db.store()) {
        stats["ts"] = serde_json::json!({"count": names.len(), "names": names});
    }

    println!("{}", serde_json::to_string(&stats).unwrap_or_default());
}

fn handle_human(db: &Talon) {
    // KV
    match db.kv_read() {
        Ok(kv) => {
            let count = kv.key_count().unwrap_or(0);
            let disk = kv.disk_space();
            println!(
                "KV: {} keys, {:.2} MB on disk",
                count,
                disk as f64 / 1048576.0
            );
        }
        Err(e) => eprintln!("KV stats 错误: {}", e),
    }
    // SQL
    match db.run_sql("SHOW TABLES") {
        Ok(rows) => {
            let names: Vec<String> = rows.iter().map(|r| format_value(&r[0])).collect();
            println!("SQL Tables ({}): {}", names.len(), names.join(", "));
        }
        Err(_) => println!("SQL Tables: (无法读取)"),
    }
    // MQ
    match db.mq_read() {
        Ok(mq) => match mq.list_topics() {
            Ok(topics) => println!("MQ Topics ({}): {}", topics.len(), topics.join(", ")),
            Err(e) => eprintln!("MQ stats 错误: {}", e),
        },
        Err(e) => eprintln!("MQ stats 错误: {}", e),
    }
    // TS
    match talon::list_timeseries(db.store()) {
        Ok(names) => {
            println!("TimeSeries ({}): {}", names.len(), names.join(", "));
        }
        Err(e) => eprintln!("TS stats 错误: {}", e),
    }
}
