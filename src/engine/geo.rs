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
        "add" => {
            if parts.len() < 6 {
                report(had_error, fmt, ":geo add <name> <key> <lng> <lat>");
                return;
            }
            let lng: f64 = match parts[4].parse() {
                Ok(v) => v,
                Err(_) => {
                    report(had_error, fmt, &format!("经度解析失败: {}", parts[4]));
                    return;
                }
            };
            let lat: f64 = match parts[5].parse() {
                Ok(v) => v,
                Err(_) => {
                    report(had_error, fmt, &format!("纬度解析失败: {}", parts[5]));
                    return;
                }
            };
            match db.geo() {
                Ok(g) => match g.geo_add(parts[2], parts[3], lng, lat) {
                    Ok(()) => {
                        if fmt == OutputFormat::Json {
                            println!(
                                "{}",
                                serde_json::json!({"ok":true,"name":parts[2],"key":parts[3],"lng":lng,"lat":lat})
                            );
                        } else {
                            println!("已添加 {} → ({}, {})", parts[3], lng, lat);
                        }
                    }
                    Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
                },
                Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
            }
        }
        "del" => {
            if parts.len() < 4 {
                report(had_error, fmt, ":geo del <name> <key>");
                return;
            }
            match db.geo() {
                Ok(g) => match g.geo_del(parts[2], parts[3]) {
                    Ok(deleted) => {
                        if fmt == OutputFormat::Json {
                            println!(
                                "{}",
                                serde_json::json!({"ok":true,"name":parts[2],"key":parts[3],"deleted":deleted})
                            );
                        } else if deleted {
                            println!("已删除 {}", parts[3]);
                        } else {
                            println!("未找到成员: {}", parts[3]);
                        }
                    }
                    Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
                },
                Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
            }
        }
        "pos" => {
            if parts.len() < 4 {
                report(had_error, fmt, ":geo pos <name> <key>");
                return;
            }
            match db.geo_read() {
                Ok(g) => match g.geo_pos(parts[2], parts[3]) {
                    Ok(Some(p)) => {
                        if fmt == OutputFormat::Json {
                            println!(
                                "{}",
                                serde_json::json!({"ok":true,"name":parts[2],"key":parts[3],"lng":p.lng,"lat":p.lat})
                            );
                        } else {
                            println!("{}: ({}, {})", parts[3], p.lng, p.lat);
                        }
                    }
                    Ok(None) => {
                        if fmt == OutputFormat::Json {
                            println!(
                                "{}",
                                serde_json::json!({"ok":true,"name":parts[2],"key":parts[3],"found":false})
                            );
                        } else {
                            println!("未找到成员: {}", parts[3]);
                        }
                    }
                    Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
                },
                Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
            }
        }
        "dist" => {
            if parts.len() < 5 {
                report(had_error, fmt, ":geo dist <name> <key1> <key2> [m|km|mi]");
                return;
            }
            let unit_str = if parts.len() >= 6 { parts[5] } else { "m" };
            let unit = match unit_str {
                "km" => talon::GeoUnit::Kilometers,
                "mi" => talon::GeoUnit::Miles,
                _ => talon::GeoUnit::Meters,
            };
            let unit_label = match unit_str {
                "km" => "km",
                "mi" => "mi",
                _ => "m",
            };
            match db.geo_read() {
                Ok(g) => match g.geo_dist(parts[2], parts[3], parts[4], unit) {
                    Ok(Some(d)) => {
                        if fmt == OutputFormat::Json {
                            println!(
                                "{}",
                                serde_json::json!({"ok":true,"name":parts[2],"key1":parts[3],"key2":parts[4],"dist":d,"unit":unit_label})
                            );
                        } else {
                            println!("{:.4} {}", d, unit_label);
                        }
                    }
                    Ok(None) => {
                        if fmt == OutputFormat::Json {
                            println!(
                                "{}",
                                serde_json::json!({"ok":true,"name":parts[2],"key1":parts[3],"key2":parts[4],"found":false})
                            );
                        } else {
                            println!("未找到成员（一个或两个均不存在）");
                        }
                    }
                    Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
                },
                Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
            }
        }
        "hash" => {
            if parts.len() < 4 {
                report(had_error, fmt, ":geo hash <name> <key>");
                return;
            }
            match db.geo_read() {
                Ok(g) => match g.geo_hash(parts[2], parts[3]) {
                    Ok(Some(h)) => {
                        if fmt == OutputFormat::Json {
                            println!(
                                "{}",
                                serde_json::json!({"ok":true,"name":parts[2],"key":parts[3],"hash":h})
                            );
                        } else {
                            println!("{}", h);
                        }
                    }
                    Ok(None) => {
                        if fmt == OutputFormat::Json {
                            println!(
                                "{}",
                                serde_json::json!({"ok":true,"name":parts[2],"key":parts[3],"found":false})
                            );
                        } else {
                            println!("未找到成员: {}", parts[3]);
                        }
                    }
                    Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
                },
                Err(e) => report(had_error, fmt, &format!("错误: {}", e)),
            }
        }
        "search" => {
            if parts.len() < 6 {
                report(had_error, fmt, ":geo search <name> <lng> <lat> <radius_m>");
                return;
            }
            let lng: f64 = parts[3].parse().unwrap_or(0.0);
            let lat: f64 = parts[4].parse().unwrap_or(0.0);
            let radius: f64 = parts[5].parse().unwrap_or(1000.0);
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
