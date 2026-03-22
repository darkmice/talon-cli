# Talon MCP Server

> 让 AI 原生访问 Talon 数据库 — 通过 TCP 连接运行中的 SuperClaw

## 架构

```
AI (IDE)  ←→  MCP Protocol (stdio)  ←→  talon_mcp.py  ←→  TCP 帧协议  ←→  SuperClaw :7720
                                          (Python)         (~0.1ms)        (Talon DB Server)
```

**零额外进程** — SuperClaw 桌面端启动时已自动在 `127.0.0.1:7720` 监听 Talon TCP Server。

## 安装

```bash
cd mcp
uv venv .venv && source .venv/bin/activate
uv pip install "mcp[cli]>=1.0.0"
```

## IDE 配置

**Gemini Code Assist (`.gemini/settings.json`):**
```json
{
    "mcpServers": {
        "talon": {
            "command": "/path/to/mcp/.venv/bin/python3",
            "args": ["/path/to/talon-cli/mcp/talon_mcp.py", "--host", "127.0.0.1", "--port", "7720"]
        }
    }
}
```

**Claude Desktop / Cursor / Windsurf:**
```json
{
    "mcpServers": {
        "talon": {
            "command": "/path/to/mcp/.venv/bin/python3",
            "args": ["/path/to/talon-cli/mcp/talon_mcp.py"],
            "env": {
                "TALON_HOST": "127.0.0.1",
                "TALON_PORT": "7720"
            }
        }
    }
}
```

## 可用 Tools

### SQL
| Tool | 说明 |
|------|------|
| `talon_sql_query` | 执行任意 SQL |
| `talon_table_schema` | 查看表结构 (DESCRIBE) |

### KV
| Tool | 说明 |
|------|------|
| `talon_kv_get` | 读取值 |
| `talon_kv_set` | 写入值 |
| `talon_kv_delete` | 删除键 |
| `talon_kv_list` | 列出键 |
| `talon_kv_scan` | 扫描键值对 |
| `talon_kv_count` | 总数 |
| `talon_kv_exists` | 检查存在 |
| `talon_kv_incr` | 原子自增 |

### MQ
| Tool | 说明 |
|------|------|
| `talon_mq_topics` | 列出 topic |
| `talon_mq_length` | 队列长度 |
| `talon_mq_publish` | 发布消息 |

### 全文搜索 / 图 / 向量 / 地理 / 时序
| Tool | 说明 |
|------|------|
| `talon_fts_search` | BM25 全文搜索 |
| `talon_graph_count/vertex/neighbors/bfs` | 图引擎 |
| `talon_vector_count` | 向量数量 |
| `talon_geo_members/count/search` | 地理空间 |
| `talon_ts_list/info` | 时序引擎 |

### 系统
| Tool | 说明 |
|------|------|
| `talon_stats` | 数据库总览 (SHOW TABLES) |

## Resources

| URI | 说明 |
|-----|------|
| `talon://tables` | 所有表列表 |
| `talon://connection` | 连接信息 |
