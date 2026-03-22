/*
 * Talon CLI — 网络后端 (TCP 帧协议客户端)
 *
 * 连接到 Talon Server 的 TCP 二进制协议端口，
 * 将所有命令转为 JSON 帧发送，接收 JSON 帧响应。
 *
 * 帧格式（与 superclaw-db server/tcp.rs 一致）：
 *   [4 bytes big-endian length] [payload bytes]
 */

use std::io::{Read, Write};
use std::net::TcpStream;

/// 网络后端 — 持有 TCP 连接。
pub struct NetBackend {
    stream: TcpStream,
}

impl NetBackend {
    /// 连接到 Talon Server。
    ///
    /// `addr` 格式: `host:port`（如 `127.0.0.1:7721`）。
    /// `token` 可选认证 token。
    pub fn connect(addr: &str, token: Option<&str>) -> Result<Self, String> {
        let stream = TcpStream::connect(addr).map_err(|e| format!("连接失败 {}: {}", addr, e))?;
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(30)))
            .ok();
        stream
            .set_write_timeout(Some(std::time::Duration::from_secs(10)))
            .ok();

        let mut backend = Self { stream };

        // 认证
        if let Some(tok) = token {
            let auth_cmd = format!(r#"{{"auth":"{}"}}"#, tok);
            backend.send_frame(auth_cmd.as_bytes())?;
            let resp = backend.recv_frame()?;
            if resp.contains("auth failed") {
                return Err("认证失败：token 错误".into());
            }
        }

        Ok(backend)
    }

    /// 发送 JSON 命令并接收响应。
    pub fn send_cmd(&mut self, json: &str) -> Result<String, String> {
        self.send_frame(json.as_bytes())?;
        self.recv_frame()
    }

    /// 发送帧：[4 bytes big-endian length][payload]。
    fn send_frame(&mut self, data: &[u8]) -> Result<(), String> {
        let len = data.len() as u32;
        self.stream
            .write_all(&len.to_be_bytes())
            .map_err(|e| format!("发送帧长度失败: {}", e))?;
        self.stream
            .write_all(data)
            .map_err(|e| format!("发送帧数据失败: {}", e))?;
        self.stream
            .flush()
            .map_err(|e| format!("flush 失败: {}", e))
    }

    /// 接收帧。
    fn recv_frame(&mut self) -> Result<String, String> {
        let mut len_buf = [0u8; 4];
        self.stream
            .read_exact(&mut len_buf)
            .map_err(|e| format!("读取帧长度失败: {}", e))?;
        let len = u32::from_be_bytes(len_buf) as usize;
        if len > 16 * 1024 * 1024 {
            return Err(format!("帧过大: {} bytes", len));
        }
        let mut buf = vec![0u8; len];
        self.stream
            .read_exact(&mut buf)
            .map_err(|e| format!("读取帧数据失败: {}", e))?;
        String::from_utf8(buf).map_err(|e| format!("UTF-8 解码失败: {}", e))
    }
}

/// 将 `:engine subcmd ...` 命令转换为 Talon TCP 协议 JSON。
///
/// 返回 JSON 字符串给 `send_cmd`。
pub fn input_to_json(input: &str) -> Result<String, String> {
    let trimmed = input.trim();

    // 非冒号开头 → SQL 语句
    if !trimmed.starts_with(':') {
        let sql = trimmed.trim_end_matches(';');
        let cmd = serde_json::json!({
            "module": "sql",
            "action": "query",
            "params": { "sql": sql }
        });
        return Ok(cmd.to_string());
    }

    let parts: Vec<&str> = trimmed.splitn(5, ' ').collect();
    let engine = parts[0];

    match engine {
        ":stats" => Ok(
            serde_json::json!({
                "module": "sql", "action": "query",
                "params": { "sql": "SHOW TABLES" }
            })
            .to_string(),
        ),
        ":kv" => kv_to_json(&parts),
        ":mq" => mq_to_json(&parts),
        ":vec" => vec_to_json(&parts),
        ":ts" => ts_to_json(&parts),
        ":fts" => fts_to_json(&parts),
        ":graph" => graph_to_json(&parts),
        ":geo" => geo_to_json(&parts),
        _ => Err(format!("未知命令: {}", engine)),
    }
}

fn kv_to_json(parts: &[&str]) -> Result<String, String> {
    if parts.len() < 2 {
        return Err(":kv 需要子命令".into());
    }
    let json = match parts[1] {
        "get" => {
            require_arg(parts, 3, ":kv get <key>")?;
            serde_json::json!({"module":"kv","action":"get","params":{"key":parts[2]}})
        }
        "set" => {
            require_arg(parts, 4, ":kv set <key> <value>")?;
            serde_json::json!({"module":"kv","action":"set","params":{"key":parts[2],"value":parts[3]}})
        }
        "del" => {
            require_arg(parts, 3, ":kv del <key>")?;
            serde_json::json!({"module":"kv","action":"del","params":{"key":parts[2]}})
        }
        "keys" => {
            let prefix = parts.get(2).copied().unwrap_or("");
            serde_json::json!({"module":"kv","action":"keys","params":{"prefix":prefix}})
        }
        "scan" => {
            let prefix = parts.get(2).copied().unwrap_or("");
            let limit = parts.get(3).and_then(|s| s.parse::<u64>().ok()).unwrap_or(20);
            serde_json::json!({"module":"kv","action":"scan","params":{"prefix":prefix,"limit":limit}})
        }
        "count" => {
            serde_json::json!({"module":"kv","action":"count","params":{}})
        }
        "exists" => {
            require_arg(parts, 3, ":kv exists <key>")?;
            serde_json::json!({"module":"kv","action":"exists","params":{"key":parts[2]}})
        }
        "incr" => {
            require_arg(parts, 3, ":kv incr <key>")?;
            serde_json::json!({"module":"kv","action":"incr","params":{"key":parts[2]}})
        }
        "incrby" => {
            require_arg(parts, 4, ":kv incrby <key> <delta>")?;
            let delta: i64 = parts[3].parse().unwrap_or(1);
            serde_json::json!({"module":"kv","action":"incrby","params":{"key":parts[2],"delta":delta}})
        }
        "decrby" => {
            require_arg(parts, 4, ":kv decrby <key> <delta>")?;
            let delta: i64 = parts[3].parse().unwrap_or(1);
            serde_json::json!({"module":"kv","action":"decrby","params":{"key":parts[2],"delta":delta}})
        }
        "setnx" => {
            require_arg(parts, 4, ":kv setnx <key> <value>")?;
            serde_json::json!({"module":"kv","action":"setnx","params":{"key":parts[2],"value":parts[3]}})
        }
        "ttl" => {
            require_arg(parts, 3, ":kv ttl <key>")?;
            serde_json::json!({"module":"kv","action":"ttl","params":{"key":parts[2]}})
        }
        sub => return Err(format!("未知 KV 子命令: {}", sub)),
    };
    Ok(json.to_string())
}

fn mq_to_json(parts: &[&str]) -> Result<String, String> {
    if parts.len() < 2 {
        return Err(":mq 需要子命令".into());
    }
    let json = match parts[1] {
        "topics" => serde_json::json!({"module":"mq","action":"list_topics","params":{}}),
        "len" => {
            require_arg(parts, 3, ":mq len <topic>")?;
            serde_json::json!({"module":"mq","action":"len","params":{"topic":parts[2]}})
        }
        "pub" => {
            require_arg(parts, 4, ":mq pub <topic> <message>")?;
            serde_json::json!({"module":"mq","action":"publish","params":{"topic":parts[2],"payload":parts[3]}})
        }
        sub => return Err(format!("未知 MQ 子命令: {}", sub)),
    };
    Ok(json.to_string())
}

fn vec_to_json(parts: &[&str]) -> Result<String, String> {
    if parts.len() < 2 {
        return Err(":vec 需要子命令".into());
    }
    let json = match parts[1] {
        "count" => {
            require_arg(parts, 3, ":vec count <name>")?;
            serde_json::json!({"module":"vector","action":"count","params":{"name":parts[2]}})
        }
        "search" => {
            require_arg(parts, 5, ":vec search <name> <k> <v1,v2,...>")?;
            let k: usize = parts[3].parse().unwrap_or(10);
            let vec_str = parts[4];
            let vec_vals: Vec<f32> = vec_str
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect();
            serde_json::json!({"module":"vector","action":"search","params":{
                "name":parts[2], "k":k, "query": vec_vals, "metric":"cosine"
            }})
        }
        sub => return Err(format!("未知 Vec 子命令: {}", sub)),
    };
    Ok(json.to_string())
}

fn ts_to_json(parts: &[&str]) -> Result<String, String> {
    if parts.len() < 2 {
        return Err(":ts 需要子命令".into());
    }
    let json = match parts[1] {
        "list" => serde_json::json!({"module":"ts","action":"list","params":{}}),
        "info" => {
            require_arg(parts, 3, ":ts info <name>")?;
            serde_json::json!({"module":"ts","action":"info","params":{"name":parts[2]}})
        }
        "query" => {
            require_arg(parts, 3, ":ts query <name>")?;
            serde_json::json!({"module":"ts","action":"query","params":{"name":parts[2]}})
        }
        sub => return Err(format!("未知 TS 子命令: {}", sub)),
    };
    Ok(json.to_string())
}

fn fts_to_json(parts: &[&str]) -> Result<String, String> {
    if parts.len() < 2 {
        return Err(":fts 需要子命令".into());
    }
    let json = match parts[1] {
        "search" => {
            require_arg(parts, 4, ":fts search <name> <query>")?;
            serde_json::json!({"module":"fts","action":"search","params":{"index":parts[2],"query":parts[3]}})
        }
        sub => return Err(format!("未知 FTS 子命令: {}", sub)),
    };
    Ok(json.to_string())
}

fn graph_to_json(parts: &[&str]) -> Result<String, String> {
    if parts.len() < 2 {
        return Err(":graph 需要子命令".into());
    }
    let json = match parts[1] {
        "count" => {
            require_arg(parts, 3, ":graph count <name>")?;
            serde_json::json!({"module":"graph","action":"count","params":{"name":parts[2]}})
        }
        "vertex" => {
            require_arg(parts, 4, ":graph vertex <name> <id>")?;
            serde_json::json!({"module":"graph","action":"get_vertex","params":{"name":parts[2],"id":parts[3]}})
        }
        "neighbors" => {
            require_arg(parts, 4, ":graph neighbors <name> <id>")?;
            let dir = parts.get(4).copied().unwrap_or("out");
            serde_json::json!({"module":"graph","action":"neighbors","params":{
                "name":parts[2],"id":parts[3],"direction":dir
            }})
        }
        "bfs" => {
            require_arg(parts, 4, ":graph bfs <name> <start>")?;
            let depth: u32 = parts.get(4).and_then(|s| s.parse().ok()).unwrap_or(3);
            serde_json::json!({"module":"graph","action":"bfs","params":{
                "name":parts[2],"start":parts[3],"depth":depth
            }})
        }
        sub => return Err(format!("未知 Graph 子命令: {}", sub)),
    };
    Ok(json.to_string())
}

fn geo_to_json(parts: &[&str]) -> Result<String, String> {
    if parts.len() < 2 {
        return Err(":geo 需要子命令".into());
    }
    let json = match parts[1] {
        "members" => {
            require_arg(parts, 3, ":geo members <name>")?;
            serde_json::json!({"module":"geo","action":"members","params":{"name":parts[2]}})
        }
        "count" => {
            require_arg(parts, 3, ":geo count <name>")?;
            serde_json::json!({"module":"geo","action":"count","params":{"name":parts[2]}})
        }
        "search" => {
            require_arg(parts, 5, ":geo search <name> <lng> <lat> <radius_m>")?;
            // 需要额外 part — 改成 splitn(6, ' ')
            // 但我们最多 5 个 part，所以 radius 从 parts[4] 中取第一个空格前的部分
            // 实际上 parts[4] 就是 "<lat> <radius>"，需要再拆分
            // 重新设计：使用 parts[3] = "lng", 找不到足够参数则提示
            let lng: f64 = parts[2].parse().map_err(|_| "经度格式错误")?;
            let lat: f64 = parts[3].parse().map_err(|_| "纬度格式错误")?;
            // parts[4] 可能包含 "radius" 或更多内容
            let radius: f64 = parts
                .get(4)
                .and_then(|s| s.split_whitespace().next())
                .and_then(|s| s.parse().ok())
                .unwrap_or(1000.0);
            serde_json::json!({"module":"geo","action":"search","params":{
                "name": parts.get(2).unwrap_or(&""),
                "lng":lng, "lat":lat, "radius":radius, "unit":"m"
            }})
        }
        sub => return Err(format!("未知 GEO 子命令: {}", sub)),
    };
    Ok(json.to_string())
}

#[inline]
fn require_arg(parts: &[&str], min: usize, usage: &str) -> Result<(), String> {
    if parts.len() < min {
        Err(usage.to_string())
    } else {
        Ok(())
    }
}

/// 打印网络模式的 JSON 响应（美化输出）。
pub fn print_net_response(resp: &str) {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(resp) {
        // 错误检查
        if let Some(false) = v.get("ok").and_then(|o| o.as_bool()) {
            if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
                eprintln!("错误: {}", err);
                return;
            }
        }
        // SQL 行结果
        if let Some(data) = v.get("data") {
            if let Some(rows) = data.get("rows").and_then(|r| r.as_array()) {
                if rows.is_empty() {
                    println!("(0 行)");
                } else {
                    for (i, row) in rows.iter().enumerate() {
                        if let Some(arr) = row.as_array() {
                            let cols: Vec<String> =
                                arr.iter().map(format_json_value).collect();
                            println!("{:>4} | {}", i + 1, cols.join(" | "));
                        } else {
                            println!("{:>4} | {}", i + 1, row);
                        }
                    }
                    println!("({} 行)", rows.len());
                }
                return;
            }
            // KV/MQ 等简单响应
            println!(
                "{}",
                serde_json::to_string_pretty(data).unwrap_or_else(|_| resp.to_string())
            );
        } else {
            println!(
                "{}",
                serde_json::to_string_pretty(&v).unwrap_or_else(|_| resp.to_string())
            );
        }
    } else {
        println!("{}", resp);
    }
}

/// 格式化 JSON Value 用于表格输出。
fn format_json_value(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "NULL".to_string(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Object(map) => {
            // Talon Value 格式：{"Integer": 42} / {"Text": "hello"} / "Null"
            if let Some(v) = map.get("Integer").and_then(|v| v.as_i64()) {
                v.to_string()
            } else if let Some(v) = map.get("Float").and_then(|v| v.as_f64()) {
                format!("{}", v)
            } else if let Some(v) = map.get("Text").and_then(|v| v.as_str()) {
                v.to_string()
            } else if let Some(v) = map.get("Timestamp").and_then(|v| v.as_i64()) {
                v.to_string()
            } else if map.contains_key("Null") {
                "NULL".to_string()
            } else {
                serde_json::to_string(v).unwrap_or_default()
            }
        }
        serde_json::Value::Array(_) => serde_json::to_string(v).unwrap_or_default(),
    }
}
