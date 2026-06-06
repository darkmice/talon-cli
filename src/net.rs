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
            // 用 serde_json 构造，避免 token 含特殊字符破坏帧格式
            let auth_cmd = serde_json::json!({ "auth": tok }).to_string();
            backend.send_frame(auth_cmd.as_bytes())?;
            let resp = backend.recv_frame()?;
            if resp.contains("auth failed") || resp.contains("auth_failed") {
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
        let len = u32::try_from(data.len())
            .map_err(|_| format!("帧过大（超过 4GB）: {} bytes", data.len()))?;
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

    // 用统一分词器（支持引号包裹、不限段数）替代写死的 splitn(5)。
    let owned = crate::cmdline::tokenize(trimmed);
    if owned.is_empty() {
        return Err("空命令".into());
    }
    let parts: Vec<&str> = owned.iter().map(|s| s.as_str()).collect();
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
        ":cluster" => cluster_to_json(&parts),
        ":backup" => backup_to_json(&parts),
        ":oplog" => oplog_to_json(&parts),
        ":sql" => sql_cmd_to_json(&parts),
        ":ai" => passthrough_to_json("ai", &parts),
        ":evo" => passthrough_to_json("evo", &parts),
        _ => Err(format!("未知命令: {}（网络模式）", engine)),
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
            require_arg(parts, 4, ":kv set <key> <value> [--ttl <secs>]")?;
            let ttl: Option<u64> = parts
                .iter()
                .position(|&p| p == "--ttl")
                .and_then(|i| parts.get(i + 1))
                .and_then(|s| s.parse().ok());
            if let Some(t) = ttl {
                serde_json::json!({"module":"kv","action":"set","params":{"key":parts[2],"value":parts[3],"ttl":t}})
            } else {
                serde_json::json!({"module":"kv","action":"set","params":{"key":parts[2],"value":parts[3]}})
            }
        }
        "expire" => {
            require_arg(parts, 4, ":kv expire <key> <secs>")?;
            let secs: u64 = parts[3].parse().map_err(|_| "secs 须为非负整数".to_string())?;
            serde_json::json!({"module":"kv","action":"expire","params":{"key":parts[2],"seconds":secs}})
        }
        "mset" => {
            let kv_args = &parts[2..];
            if kv_args.is_empty() || kv_args.len() % 2 != 0 {
                return Err(":kv mset <k1> <v1> ...（参数必须成对）".into());
            }
            let keys: Vec<&str> = kv_args.iter().step_by(2).copied().collect();
            let values: Vec<&str> = kv_args.iter().skip(1).step_by(2).copied().collect();
            serde_json::json!({"module":"kv","action":"mset","params":{"keys":keys,"values":values}})
        }
        "mget" => {
            require_arg(parts, 3, ":kv mget <k1> <k2> ...")?;
            let keys: Vec<&str> = parts[2..].to_vec();
            serde_json::json!({"module":"kv","action":"mget","params":{"keys":keys}})
        }
        "rename" => {
            require_arg(parts, 4, ":kv rename <src> <dst>")?;
            serde_json::json!({"module":"kv","action":"rename","params":{"src":parts[2],"dst":parts[3]}})
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
            let delta: i64 = parts[3].parse().map_err(|_| "delta 须为整数".to_string())?;
            serde_json::json!({"module":"kv","action":"incrby","params":{"key":parts[2],"delta":delta}})
        }
        "decrby" => {
            require_arg(parts, 4, ":kv decrby <key> <delta>")?;
            let delta: i64 = parts[3].parse().map_err(|_| "delta 须为整数".to_string())?;
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
        "create" => {
            require_arg(parts, 3, ":mq create <topic> [max_len]")?;
            let max_len: u64 = parts.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
            serde_json::json!({"module":"mq","action":"create","params":{"topic":parts[2],"max_len":max_len}})
        }
        "poll" => {
            require_arg(parts, 5, ":mq poll <topic> <group> <consumer> [count]")?;
            let count: u64 = parts.get(5).and_then(|s| s.parse().ok()).unwrap_or(10);
            serde_json::json!({"module":"mq","action":"poll","params":{
                "topic":parts[2],"group":parts[3],"consumer":parts[4],"count":count
            }})
        }
        "ack" => {
            require_arg(parts, 6, ":mq ack <topic> <group> <consumer> <msg_id>")?;
            let message_id: u64 = parts[5].parse().map_err(|_| "msg_id 须为整数".to_string())?;
            serde_json::json!({"module":"mq","action":"ack","params":{
                "topic":parts[2],"group":parts[3],"consumer":parts[4],"message_id":message_id
            }})
        }
        "sub" => {
            require_arg(parts, 4, ":mq sub <topic> <group>")?;
            serde_json::json!({"module":"mq","action":"subscribe","params":{"topic":parts[2],"group":parts[3]}})
        }
        "unsub" => {
            require_arg(parts, 4, ":mq unsub <topic> <group>")?;
            serde_json::json!({"module":"mq","action":"unsubscribe","params":{"topic":parts[2],"group":parts[3]}})
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
            let k: usize = parts[3].parse().map_err(|_| "k 须为正整数".to_string())?;
            let vec_vals: Vec<f32> = parts[4]
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect();
            if vec_vals.is_empty() {
                return Err("查询向量不能为空".into());
            }
            // server exec_vector 读 "vector" 字段（非 "query"）
            serde_json::json!({"module":"vector","action":"search","params":{
                "name":parts[2], "k":k, "vector": vec_vals, "metric":"cosine"
            }})
        }
        "insert" => {
            require_arg(parts, 5, ":vec insert <name> <id> <v1,v2,...>")?;
            let id: u64 = parts[3].parse().map_err(|_| "id 须为整数".to_string())?;
            let vec_vals: Vec<f32> = parts[4]
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect();
            serde_json::json!({"module":"vector","action":"insert","params":{
                "name":parts[2], "id":id, "vector":vec_vals
            }})
        }
        "delete" => {
            require_arg(parts, 4, ":vec delete <name> <id>")?;
            let id: u64 = parts[3].parse().map_err(|_| "id 须为整数".to_string())?;
            serde_json::json!({"module":"vector","action":"delete","params":{"name":parts[2],"id":id}})
        }
        "get" => {
            require_arg(parts, 4, ":vec get <name> <id>")?;
            let id: u64 = parts[3].parse().map_err(|_| "id 须为整数".to_string())?;
            serde_json::json!({"module":"vector","action":"get_vector","params":{"name":parts[2],"id":id}})
        }
        "rebuild" => {
            require_arg(parts, 3, ":vec rebuild <name>")?;
            serde_json::json!({"module":"vector","action":"rebuild_index","params":{"name":parts[2]}})
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
            let limit: Option<u64> = parts.get(3).and_then(|s| s.parse().ok());
            serde_json::json!({"module":"ts","action":"query","params":{"name":parts[2],"limit":limit}})
        }
        "create" => {
            require_arg(parts, 5, ":ts create <name> <tags> <fields>")?;
            let tags: Vec<&str> = parts[3].split(',').filter(|s| !s.is_empty()).collect();
            let fields: Vec<&str> = parts[4].split(',').filter(|s| !s.is_empty()).collect();
            serde_json::json!({"module":"ts","action":"create","params":{
                "name":parts[2],"tags":tags,"fields":fields
            }})
        }
        "insert" => {
            require_arg(parts, 6, ":ts insert <name> <ts_ms> <k=v,...> <k=v,...>")?;
            let ts_ms: i64 = parts[3].parse().map_err(|_| "ts_ms 须为整数".to_string())?;
            let to_obj = |s: &str| -> serde_json::Map<String, serde_json::Value> {
                s.split(',')
                    .filter_map(|kv| kv.split_once('='))
                    .map(|(k, v)| (k.to_string(), serde_json::Value::String(v.to_string())))
                    .collect()
            };
            let tags = serde_json::Value::Object(to_obj(parts[4]));
            let fields = serde_json::Value::Object(to_obj(parts[5]));
            serde_json::json!({"module":"ts","action":"insert","params":{
                "name":parts[2],
                "point":{"timestamp":ts_ms,"tags":tags,"fields":fields}
            }})
        }
        "agg" => {
            require_arg(parts, 6, ":ts agg <name> <field> <func> <interval_ms>")?;
            let interval: i64 = parts[5].parse().map_err(|_| "interval_ms 须为整数".to_string())?;
            serde_json::json!({"module":"ts","action":"aggregate","params":{
                "name":parts[2],"field":parts[3],"func":parts[4],
                "interval_ms": if interval > 0 { Some(interval) } else { None::<i64> }
            }})
        }
        "retention" => {
            require_arg(parts, 4, ":ts retention <name> <ms>")?;
            let ms: u64 = parts[3].parse().map_err(|_| "ms 须为非负整数".to_string())?;
            serde_json::json!({"module":"ts","action":"set_retention","params":{"name":parts[2],"retention_ms":ms}})
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
            // server exec_fts 读 "name"/"query" 字段（旧代码误用 "index"，已修正）
            serde_json::json!({"module":"fts","action":"search","params":{"name":parts[2],"query":parts[3]}})
        }
        "create" => {
            require_arg(parts, 3, ":fts create <name>")?;
            serde_json::json!({"module":"fts","action":"create_index","params":{"name":parts[2]}})
        }
        "drop" => {
            require_arg(parts, 3, ":fts drop <name>")?;
            serde_json::json!({"module":"fts","action":"drop_index","params":{"name":parts[2]}})
        }
        "index" => {
            require_arg(parts, 5, ":fts index <name> <doc_id> <fields_json>")?;
            let fields: serde_json::Value =
                serde_json::from_str(parts[4]).map_err(|e| format!("fields JSON 解析失败: {}", e))?;
            serde_json::json!({"module":"fts","action":"index","params":{
                "name":parts[2],"doc_id":parts[3],"fields":fields
            }})
        }
        "get" => {
            require_arg(parts, 4, ":fts get <name> <doc_id>")?;
            serde_json::json!({"module":"fts","action":"get","params":{"name":parts[2],"doc_id":parts[3]}})
        }
        "del" => {
            require_arg(parts, 4, ":fts del <name> <doc_id>")?;
            serde_json::json!({"module":"fts","action":"delete","params":{"name":parts[2],"doc_id":parts[3]}})
        }
        "reindex" => {
            require_arg(parts, 3, ":fts reindex <name>")?;
            serde_json::json!({"module":"fts","action":"reindex","params":{"name":parts[2]}})
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
            let start: u64 = parts[3].parse().map_err(|_| "start 须为整数".to_string())?;
            let depth: u64 = parts.get(4).and_then(|s| s.parse().ok()).unwrap_or(3);
            serde_json::json!({"module":"graph","action":"bfs","params":{
                "graph":parts[2],"start":start,"max_depth":depth
            }})
        }
        "add-vertex" => {
            require_arg(parts, 4, ":graph add-vertex <name> <label> [props_json]")?;
            let props = parse_props_value(parts.get(4).copied())?;
            serde_json::json!({"module":"graph","action":"add_vertex","params":{
                "graph":parts[2],"label":parts[3],"properties":props
            }})
        }
        "add-edge" => {
            require_arg(parts, 6, ":graph add-edge <name> <from> <to> <label> [props_json]")?;
            let from: u64 = parts[3].parse().map_err(|_| "from 须为整数".to_string())?;
            let to: u64 = parts[4].parse().map_err(|_| "to 须为整数".to_string())?;
            let props = parse_props_value(parts.get(6).copied())?;
            serde_json::json!({"module":"graph","action":"add_edge","params":{
                "graph":parts[2],"from":from,"to":to,"label":parts[5],"properties":props
            }})
        }
        "update-vertex" => {
            require_arg(parts, 5, ":graph update-vertex <name> <id> <props_json>")?;
            let id: u64 = parts[3].parse().map_err(|_| "id 须为整数".to_string())?;
            let props = parse_props_value(parts.get(4).copied())?;
            serde_json::json!({"module":"graph","action":"update_vertex","params":{
                "graph":parts[2],"id":id,"properties":props
            }})
        }
        "del-vertex" => {
            require_arg(parts, 4, ":graph del-vertex <name> <id>")?;
            let id: u64 = parts[3].parse().map_err(|_| "id 须为整数".to_string())?;
            serde_json::json!({"module":"graph","action":"delete_vertex","params":{"graph":parts[2],"id":id}})
        }
        "del-edge" => {
            require_arg(parts, 4, ":graph del-edge <name> <edge_id>")?;
            let id: u64 = parts[3].parse().map_err(|_| "edge_id 须为整数".to_string())?;
            serde_json::json!({"module":"graph","action":"delete_edge","params":{"graph":parts[2],"id":id}})
        }
        "edges" => {
            require_arg(parts, 4, ":graph edges <name> <vertex_id> [out|in]")?;
            let id: u64 = parts[3].parse().map_err(|_| "vertex_id 须为整数".to_string())?;
            let action = match parts.get(4).copied().unwrap_or("out") {
                "in" => "in_edges",
                _ => "out_edges",
            };
            serde_json::json!({"module":"graph","action":action,"params":{"graph":parts[2],"id":id}})
        }
        "path" => {
            require_arg(parts, 5, ":graph path <name> <from> <to> [max_depth]")?;
            let from: u64 = parts[3].parse().map_err(|_| "from 须为整数".to_string())?;
            let to: u64 = parts[4].parse().map_err(|_| "to 须为整数".to_string())?;
            let max_depth: u64 = parts.get(5).and_then(|s| s.parse().ok()).unwrap_or(10);
            serde_json::json!({"module":"graph","action":"shortest_path","params":{
                "graph":parts[2],"from":from,"to":to,"max_depth":max_depth
            }})
        }
        "pagerank" => {
            require_arg(parts, 3, ":graph pagerank <name> [limit]")?;
            let limit: u64 = parts.get(3).and_then(|s| s.parse().ok()).unwrap_or(10);
            serde_json::json!({"module":"graph","action":"pagerank","params":{
                "graph":parts[2],"damping":0.85,"iterations":20,"limit":limit
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
            // 分词后各参数独立成 token：name lng lat radius
            require_arg(parts, 6, ":geo search <name> <lng> <lat> <radius_m>")?;
            let lng: f64 = parts[3].parse().map_err(|_| "经度格式错误")?;
            let lat: f64 = parts[4].parse().map_err(|_| "纬度格式错误")?;
            let radius: f64 = parts[5].parse().map_err(|_| "半径格式错误")?;
            serde_json::json!({"module":"geo","action":"search","params":{
                "name": parts[2], "lng":lng, "lat":lat, "radius":radius, "unit":"m"
            }})
        }
        "add" => {
            require_arg(parts, 6, ":geo add <name> <key> <lng> <lat>")?;
            let lng: f64 = parts[4].parse().map_err(|_| "经度格式错误".to_string())?;
            let lat: f64 = parts[5].parse().map_err(|_| "纬度格式错误".to_string())?;
            serde_json::json!({"module":"geo","action":"add","params":{
                "name":parts[2],"key":parts[3],"lng":lng,"lat":lat
            }})
        }
        "del" => {
            require_arg(parts, 4, ":geo del <name> <key>")?;
            serde_json::json!({"module":"geo","action":"del","params":{"name":parts[2],"key":parts[3]}})
        }
        "pos" => {
            require_arg(parts, 4, ":geo pos <name> <key>")?;
            serde_json::json!({"module":"geo","action":"pos","params":{"name":parts[2],"key":parts[3]}})
        }
        "dist" => {
            require_arg(parts, 5, ":geo dist <name> <key1> <key2> [m|km|mi]")?;
            let unit = parts.get(5).copied().unwrap_or("m");
            serde_json::json!({"module":"geo","action":"dist","params":{
                "name":parts[2],"key1":parts[3],"key2":parts[4],"unit":unit
            }})
        }
        "hash" => {
            require_arg(parts, 4, ":geo hash <name> <key>")?;
            serde_json::json!({"module":"geo","action":"hash","params":{"name":parts[2],"key":parts[3]}})
        }
        sub => return Err(format!("未知 GEO 子命令: {}", sub)),
    };
    Ok(json.to_string())
}

/// 集群运维命令 → JSON。
/// server exec_cluster 支持 status/role/promote/replicas（无 params）；
/// epoch 走 status 取字段；step-down 发 new_primary/epoch 给 server。
fn cluster_to_json(parts: &[&str]) -> Result<String, String> {
    if parts.len() < 2 {
        return Err(":cluster 需要子命令".into());
    }
    let json = match parts[1] {
        "status" | "epoch" => serde_json::json!({"module":"cluster","action":"status","params":{}}),
        "role" => serde_json::json!({"module":"cluster","action":"role","params":{}}),
        "promote" => serde_json::json!({"module":"cluster","action":"promote","params":{}}),
        "replicas" => serde_json::json!({"module":"cluster","action":"replicas","params":{}}),
        "step-down" => {
            require_arg(parts, 4, ":cluster step-down <new_primary_addr> <epoch>")?;
            let epoch: u64 = parts[3].parse().map_err(|_| "epoch 须为非负整数".to_string())?;
            serde_json::json!({"module":"cluster","action":"step_down","params":{
                "new_primary":parts[2],"epoch":epoch
            }})
        }
        sub => return Err(format!("未知 cluster 子命令: {}", sub)),
    };
    Ok(json.to_string())
}

/// 备份命令 → JSON。
/// server exec_backup 支持 export/import/consistent/restore（PITR）。
fn backup_to_json(parts: &[&str]) -> Result<String, String> {
    if parts.len() < 2 {
        return Err(":backup 需要子命令".into());
    }
    let json = match parts[1] {
        "export" => {
            require_arg(parts, 3, ":backup export <dir> [ks...]")?;
            let keyspaces: Vec<&str> = parts[3..].to_vec();
            serde_json::json!({"module":"backup","action":"export","params":{
                "dir":parts[2],"keyspaces":keyspaces
            }})
        }
        "import" => {
            require_arg(parts, 3, ":backup import <dir>")?;
            serde_json::json!({"module":"backup","action":"import","params":{"dir":parts[2]}})
        }
        "snapshot" => {
            require_arg(parts, 3, ":backup snapshot <dir> [ts_ms]")?;
            let ts: u64 = parts.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
            serde_json::json!({"module":"backup","action":"consistent","params":{
                "dir":parts[2],"timestamp_ms":ts
            }})
        }
        "restore" => {
            require_arg(parts, 4, ":backup restore <dir> latest|snapshot|lsn <n>|ts <ms>")?;
            let mut params = serde_json::Map::new();
            params.insert("dir".into(), serde_json::json!(parts[2]));
            match parts[3] {
                "latest" => {
                    params.insert("target".into(), serde_json::json!("latest"));
                }
                "snapshot" => {
                    params.insert("target".into(), serde_json::json!("snapshot"));
                }
                "lsn" => {
                    let n: u64 = parts
                        .get(4)
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| "lsn 后需提供整数".to_string())?;
                    params.insert("lsn".into(), serde_json::json!(n));
                }
                "ts" => {
                    let ms: u64 = parts
                        .get(4)
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| "ts 后需提供毫秒整数".to_string())?;
                    params.insert("timestamp_ms".into(), serde_json::json!(ms));
                }
                other => return Err(format!("未知 restore 目标: {}（用 latest|snapshot|lsn|ts）", other)),
            }
            serde_json::json!({"module":"backup","action":"restore","params":params})
        }
        sub => return Err(format!("未知 backup 子命令: {}", sub)),
    };
    Ok(json.to_string())
}

/// SQL 事务命令 → JSON（begin/commit/rollback 翻成裸 SQL 帧）。
/// SQL 子命令 → JSON。begin/commit/rollback 走裸 SQL 帧；
/// param 走 exec_sql 的 bind 字段（服务端已支持参数化）。
fn sql_cmd_to_json(parts: &[&str]) -> Result<String, String> {
    let sub = parts.get(1).copied().unwrap_or("");
    match sub {
        "begin" | "commit" | "rollback" => {
            let sql = match sub {
                "begin" => "BEGIN",
                "commit" => "COMMIT",
                _ => "ROLLBACK",
            };
            Ok(serde_json::json!({"module":"sql","action":"query","params":{"sql":sql}}).to_string())
        }
        "param" => {
            // :sql param "<sql with ?>" <v1> <v2> ...
            require_arg(parts, 3, ":sql param \"<sql>\" [v1 v2 ...]")?;
            let sql = parts[2];
            // 把参数转成 Value 的 JSON 编码（{"Integer":n}/{"Float":f}/{"Text":s}）
            let bind: Vec<serde_json::Value> = parts[3..].iter().map(|s| value_to_bind(s)).collect();
            Ok(serde_json::json!({"module":"sql","action":"query","params":{
                "sql":sql,"bind":bind
            }})
            .to_string())
        }
        "exec" => Err(":sql exec 网络模式请直接输入裸 SQL（不加 :sql 前缀）".into()),
        other => Err(format!("未知 :sql 子命令: {}", other)),
    }
}

/// 把命令行字符串参数转成 talon::Value 的 JSON 编码（与 server Value Deserialize 对齐）。
/// 能 parse i64 → Integer，能 parse f64 → Float，否则 → Text。
fn value_to_bind(s: &str) -> serde_json::Value {
    if let Ok(n) = s.parse::<i64>() {
        serde_json::json!({ "Integer": n })
    } else if let Ok(f) = s.parse::<f64>() {
        serde_json::json!({ "Float": f })
    } else {
        serde_json::json!({ "Text": s })
    }
}

/// OpLog 命令 → JSON。server exec_oplog 支持 current_lsn/get/range。
fn oplog_to_json(parts: &[&str]) -> Result<String, String> {
    if parts.len() < 2 {
        return Err(":oplog 需要子命令".into());
    }
    let json = match parts[1] {
        "lsn" => serde_json::json!({"module":"oplog","action":"current_lsn","params":{}}),
        "get" => {
            require_arg(parts, 3, ":oplog get <lsn>")?;
            let lsn: u64 = parts[2].parse().map_err(|_| "lsn 须为整数".to_string())?;
            serde_json::json!({"module":"oplog","action":"get","params":{"lsn":lsn}})
        }
        "range" => {
            require_arg(parts, 4, ":oplog range <from> <to> [limit]")?;
            let from: u64 = parts[2].parse().map_err(|_| "from 须为整数".to_string())?;
            let to: u64 = parts[3].parse().map_err(|_| "to 须为整数".to_string())?;
            let limit: u64 = parts.get(4).and_then(|s| s.parse().ok()).unwrap_or(100);
            serde_json::json!({"module":"oplog","action":"range","params":{
                "from":from,"to":to,"limit":limit
            }})
        }
        sub => return Err(format!("未知 oplog 子命令: {}", sub)),
    };
    Ok(json.to_string())
}

/// AI / EvoCore 引擎命令透传 → JSON。
/// 这两个引擎在服务端 ffi_exec 已注册 handler（未授权时返回付费提示，cli 透传）。
/// 协议：`:ai <action> [k=v ...]` → {"module":"ai","action":<action>,"params":{...}}。
fn passthrough_to_json(module: &str, parts: &[&str]) -> Result<String, String> {
    if parts.len() < 2 {
        return Err(format!(":{} 需要子命令", module));
    }
    let action = parts[1];
    // 其余 k=v token 作为 params，裸 token 收进 args 数组
    let mut params = serde_json::Map::new();
    let mut args: Vec<&str> = Vec::new();
    for &tok in &parts[2..] {
        if let Some((k, v)) = tok.split_once('=') {
            params.insert(k.to_string(), serde_json::Value::String(v.to_string()));
        } else {
            args.push(tok);
        }
    }
    if !args.is_empty() {
        params.insert(
            "args".to_string(),
            serde_json::Value::Array(args.into_iter().map(|s| serde_json::json!(s)).collect()),
        );
    }
    Ok(serde_json::json!({"module":module,"action":action,"params":params}).to_string())
}

/// 把可选的 props_json 参数解析为 JSON Value object（图属性用）。
/// 未提供 → 空 object；提供但非法 JSON 或非 object → Err。
/// 与嵌入模式 cmdline::parse_props_json 的校验严格度保持一致。
fn parse_props_value(raw: Option<&str>) -> Result<serde_json::Value, String> {
    match raw {
        None => Ok(serde_json::json!({})),
        Some(s) => {
            let v = serde_json::from_str::<serde_json::Value>(s)
                .map_err(|e| format!("属性 JSON 解析失败: {}", e))?;
            if !v.is_object() {
                return Err("属性必须是 JSON object，如 {\"k\":\"v\"}".into());
            }
            Ok(v)
        }
    }
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
