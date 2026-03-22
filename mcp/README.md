# Talon MCP Server

> 让 AI 原生访问 Talon 数据库全部 11 引擎 — 底层复用 talon-cli

## 架构

```
AI (IDE)  ←→  MCP Protocol  ←→  talon_mcp.py  ←→  talon-cli  ←→  Talon Server :7720
               (stdio)           (Python 桥接)     (Rust 全能力)    (SuperClaw 内置)
```

- **MCP 层** — 37 个 Tool 定义，JSON 桥接
- **talon-cli 层** — 全部 11 引擎能力（SQL/KV/MQ/FTS/Graph/Geo/Vec/TS/**AI/EvoCore**）
- **传输层** — TCP 帧协议连接运行中的 Talon Server

## 安装

```bash
# 1. 编译 talon-cli
cd /path/to/talon-cli && cargo build --release

# 2. 安装 MCP Python 依赖
cd mcp
uv venv .venv && source .venv/bin/activate
uv pip install "mcp[cli]>=1.0.0"
```

## IDE 配置

**WebStorm / Gemini Code Assist:**
```json
{
    "mcpServers": {
        "talon": {
            "command": "/path/to/mcp/.venv/bin/python3",
            "args": [
                "/path/to/talon-cli/mcp/talon_mcp.py",
                "--connect", "127.0.0.1:7720"
            ]
        }
    }
}
```

**环境变量方式（适合 CI/Docker）：**
```json
{
    "mcpServers": {
        "talon": {
            "command": "/path/to/mcp/.venv/bin/python3",
            "args": ["/path/to/talon-cli/mcp/talon_mcp.py"],
            "env": {
                "TALON_CONNECT": "127.0.0.1:7720",
                "TALON_CLI_PATH": "/path/to/talon-cli/target/release/talon-cli"
            }
        }
    }
}
```

## 连接模式

| 模式 | 参数 | 说明 |
|------|------|------|
| 网络模式 | `--connect 127.0.0.1:7720` | 连接运行中的 SuperClaw（推荐） |
| 嵌入模式 | `--db-path ~/.superclaw/talon_data` | 直接打开本地数据库 |

## 可用 Tools (37 个)

### SQL
- `talon_sql_query` — 执行任意 SQL
- `talon_table_schema` — 查看表结构

### KV (8 个)
- `talon_kv_get/set/delete/list/scan/count/exists/incr`

### MQ (3 个)
- `talon_mq_topics/length/publish`

### FTS / Graph / Vector / Geo / TS
- `talon_fts_search` — BM25 全文搜索
- `talon_graph_count/vertex/neighbors/bfs` — 图引擎
- `talon_vector_count` — 向量引擎
- `talon_geo_members/count/search` — 地理空间
- `talon_ts_list/info` — 时序引擎

### AI 引擎 (6 个)
- `talon_ai_sessions/session_detail/history` — 会话管理
- `talon_ai_memory_count/docs_list/docs_count` — 记忆和 RAG

### EvoCore 进化引擎 (5 个)
- `talon_evo_soul/personality/history/proposals/stats`

### 系统
- `talon_stats` — 数据库总览
