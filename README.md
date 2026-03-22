# Talon CLI

> 嵌入式 + 网络双模数据库命令行工具 — AI 友好

Talon CLI 是 [Talon 多模融合数据引擎](https://github.com/darkmice/talon-docs) 的官方命令行客户端。覆盖全部 **11 大引擎**：SQL·KV·MQ·向量·时序·全文搜索·图·地理空间 + AI Session/Memory/RAG + EvoCore 进化引擎。

## ✨ 特性

- **双模运行** — 嵌入式直接打开本地数据库 / 网络模式连接运行中的 Talon Server
- **AI 友好** — `--format json` 输出结构化 JSON，AI Agent 可直接解析
- **Exit Code** — 命令失败返回非零退出码，AI 可通过 `$?` 判断成败
- **多条命令** — `-c "SQL1; SQL2"` 分号分隔批量执行
- **11 引擎全覆盖** — SQL、KV、MQ、FTS、Graph、GEO、Vector、TimeSeries、AI、EvoCore、Stats

## 📦 安装

```bash
# 从源码编译
git clone https://github.com/darkmice/talon-cli.git
cd talon-cli
cargo build --release
# 二进制在 target/release/talon-cli
```

## 🚀 快速开始

### 嵌入模式（直接打开数据库）

```bash
# 交互 REPL
talon-cli ./my-database

# 单条命令
talon-cli ./my-database -c "CREATE TABLE users (id INT, name TEXT, age INT)"
talon-cli ./my-database -c "INSERT INTO users VALUES (1, 'Alice', 30)"
talon-cli ./my-database -c "SELECT * FROM users"

# KV 操作
talon-cli ./my-database -c ":kv set greeting hello"
talon-cli ./my-database -c ":kv get greeting"

# AI 引擎
talon-cli ./my-database -c ":ai sessions"
talon-cli ./my-database -c ":evo soul"
```

### 网络模式（连接 Talon Server）

```bash
# 连接到运行中的 Talon Server
talon-cli --connect 127.0.0.1:7721

# 带认证
talon-cli --connect 127.0.0.1:7721 --token YOUR_TOKEN

# 单次执行
talon-cli --connect 127.0.0.1:7721 -c "SELECT * FROM users"
```

### JSON 输出（AI Agent 推荐）

```bash
# 所有命令都支持 JSON 输出
talon-cli ./db --format json -c "SELECT * FROM users"
# {"ok":true,"rows":[[1,"Alice",30]],"count":1}

talon-cli ./db --format json -c ":kv get greeting"
# {"ok":true,"key":"greeting","value":"hello"}

talon-cli ./db --format json -c ":evo soul"
# {"ok":true,"name":"dark","personality_type":"Professional",...}

# 错误也是 JSON
talon-cli ./db --format json -c "INVALID SQL"
# {"ok":false,"error":"SQL 错误: ..."}
```

## 📋 命令参考

### SQL

```
<SQL>;              执行 SQL（SELECT/INSERT/UPDATE/DELETE/CREATE/DROP/...）
<SQL1>; <SQL2>      分号分隔多条 SQL（-c 模式）
```

### KV 引擎

| 命令 | 说明 |
|------|------|
| `:kv get <key>` | 读取 |
| `:kv set <key> <value>` | 写入 |
| `:kv del <key>` | 删除 |
| `:kv keys [prefix]` | 列出 key |
| `:kv scan <prefix> [limit]` | 扫描 key + value |
| `:kv count` | 总 key 数量 |
| `:kv exists <key>` | 检查是否存在 |
| `:kv incr <key>` | 原子自增 |
| `:kv ttl <key>` | 查看剩余 TTL |

### MQ 引擎

| 命令 | 说明 |
|------|------|
| `:mq topics` | 列出所有 topic |
| `:mq len <topic>` | 队列长度 |
| `:mq pub <topic> <msg>` | 发布消息 |

### 全文搜索

| 命令 | 说明 |
|------|------|
| `:fts search <name> <query>` | BM25 搜索 |

### 图引擎

| 命令 | 说明 |
|------|------|
| `:graph count <name>` | 节点/边计数 |
| `:graph vertex <name> <id>` | 查看节点 |
| `:graph neighbors <name> <id> [dir]` | 邻居节点 |
| `:graph bfs <name> <start> [depth]` | BFS 遍历 |

### 向量引擎

| 命令 | 说明 |
|------|------|
| `:vec count <name>` | 向量索引数量 |

### 地理空间

| 命令 | 说明 |
|------|------|
| `:geo members <name>` | 列出成员 |
| `:geo count <name>` | 成员数量 |
| `:geo search <name> <lng> <lat> <r>` | 圆形搜索 |

### 时序引擎

| 命令 | 说明 |
|------|------|
| `:ts list` | 列出时序名称 |
| `:ts info <name>` | 查看时序详情 |

### AI 引擎（talon-ai）

| 命令 | 说明 |
|------|------|
| `:ai sessions` | 列出活跃 Session |
| `:ai session <id>` | 查看 Session 详情 |
| `:ai history <sid> [limit]` | 对话历史 |
| `:ai memory count` | 记忆数量 |
| `:ai docs list` | 列出 RAG 文档 |
| `:ai docs count` | RAG 文档数量 |

### EvoCore 进化引擎

| 命令 | 说明 |
|------|------|
| `:evo soul` | 查看 Soul 配置 |
| `:evo personality` | 查看个性维度 |
| `:evo history` | Soul 进化历史 |
| `:evo proposals` | 进化提议列表 |
| `:evo stats` | 学习统计 |

### 系统

| 命令 | 说明 |
|------|------|
| `:stats` | 数据库统计信息 |
| `:help` | 显示帮助 |
| `:quit` / `:exit` | 退出 |

## 🏗️ 架构

```
talon-cli/src/
├── main.rs          # 入口、双模路由、REPL 循环
├── format.rs        # Value 格式化工具
├── net.rs           # 网络后端（TCP 帧协议客户端）
└── engine/
    ├── mod.rs       # 模块注册
    ├── sql.rs       # SQL 引擎
    ├── kv.rs        # KV 引擎
    ├── mq.rs        # 消息队列
    ├── fts.rs       # 全文搜索
    ├── graph.rs     # 图引擎
    ├── geo.rs       # 地理空间
    ├── ts.rs        # 时序引擎
    ├── vec.rs       # 向量引擎
    ├── ai.rs        # AI 引擎（Session/Memory/RAG）
    ├── evo.rs       # EvoCore 进化引擎
    └── stats.rs     # 统计信息
```

## 📄 License

Talon Community Dual License Agreement.
