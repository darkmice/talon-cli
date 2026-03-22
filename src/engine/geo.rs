//! GEO 地理空间引擎命令处理。

use crate::OutputFormat;
use std::sync::atomic::{AtomicBool, Ordering};
use talon::Talon;

/// 处理 `:geo` 子命令。
pub fn handle(db: &Talon, parts: &[&str], fmt: OutputFormat, had_error: &AtomicBool) {
    if parts.len() < 2 {
        report(had_error, fmt, ":geo 需要子命令。输入 :help 查看。");
        return;
    }
    match parts[1] {
        "members" => {
            if parts.len() < 3 {
                report(had_error, fmt, ":geo members <name>");
                return;
            }
            match db.geo_read() {
                Ok(g) => match g.geo_members(parts[2]) {
                    Ok(members) => {
                        if fmt == OutputFormat::Json {
                            println!(
                                "{}",
                                serde_json::json!({"ok":true,"members":members,"count":members.len()})
                            );
                        } else {
                            for m in &members {
                                println!("  {}", m);
                            }
                            println!("({} 个成员)", members.len());
                        }
                    }
                    Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
                },
                Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
            }
        }
        "count" => {
            if parts.len() < 3 {
                report(had_error, fmt, ":geo count <name>");
                return;
            }
            match db.geo_read() {
                Ok(g) => match g.geo_count(parts[2]) {
                    Ok(n) => {
                        if fmt == OutputFormat::Json {
                            println!(
                                "{}",
                                serde_json::json!({"ok":true,"name":parts[2],"count":n})
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
        "search" => {
            if parts.len() < 4 {
                report(had_error, fmt, ":geo search <name> <lng> <lat> <radius_m>");
                return;
            }
            let sub: Vec<&str> = parts[3].splitn(3, ' ').collect();
            if sub.len() < 3 {
                report(had_error, fmt, ":geo search <name> <lng> <lat> <radius_m>");
                return;
            }
            let lng: f64 = sub[0].parse().unwrap_or(0.0);
            let lat: f64 = sub[1].parse().unwrap_or(0.0);
            let radius: f64 = sub[2].parse().unwrap_or(1000.0);
            match db.geo_read() {
                Ok(g) => {
                    match g.geo_search(parts[2], lng, lat, radius, talon::GeoUnit::Meters, Some(20))
                    {
                        Ok(members) => {
                            if fmt == OutputFormat::Json {
                                let results: Vec<serde_json::Value> = members
                                    .iter()
                                    .map(|m| {
                                        serde_json::json!({
                                            "key": m.key,
                                            "lng": m.point.lng,
                                            "lat": m.point.lat,
                                            "distance_m": m.dist.unwrap_or(0.0),
                                        })
                                    })
                                    .collect();
                                println!(
                                    "{}",
                                    serde_json::json!({"ok":true,"results":results,"count":results.len()})
                                );
                            } else {
                                for m in &members {
                                    println!(
                                        "  {} ({}, {}) dist={:.1}m",
                                        m.key,
                                        m.point.lng,
                                        m.point.lat,
                                        m.dist.unwrap_or(0.0)
                                    );
                                }
                                println!("({} 个结果)", members.len());
                            }
                        }
                        Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
                    }
                }
                Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
            }
        }
        _ => report(
            had_error,
            fmt,
            &format!("未知 geo 子命令: {}", parts[1]),
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
