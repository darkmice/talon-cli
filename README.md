# Talon CLI

> 嵌入式 + 网络 + Daemon 三模数据库命令行工具 — AI 友好

Talon CLI 是 [Talon 多模融合数据引擎](https://github.com/darkmice/talon-docs) 的官方命令行客户端。覆盖全部 **11 大引擎**：SQL·KV·MQ·向量·时序·全文搜索·图·地理空间 + AI Session/Memory/RAG + EvoCore 进化引擎。

## ✨ 特性

- **三模运行** — 嵌入式 / 网络 / Daemon 模式
- **Daemon 加速** — `serve` 模式保持数据库打开，`-c` 命令自动走 Unix Socket（**7ms vs 539ms，77x 加速**）
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

### ⚡ Daemon 模式（AI 推荐 — 毫秒级响应）

```bash
# 1. 启动 daemon（数据库只打开一次）
talon-cli serve ./my-database &

# 2. 后续所有命令自动走 Unix Socket，毫秒级返回
talon-cli ./my-database -c "SELECT * FROM users"          # 7ms ✨
talon-cli ./my-database -c ":kv get greeting"              # 7ms ✨
talon-cli ./my-database --format json -c ":evo soul"       # 7ms ✨

# 无 daemon 时自动 fallback 到嵌入模式（~500ms）
```

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

> **参数引用规则**：含空格的值或 JSON 字面量请用引号包裹整段，如
> `:kv set greeting "hello world"`、`:fts index arts d1 '{"title":"hello world"}'`。
> 不含空格的紧凑 JSON 可裸传（`{"k":"v"}`）。
>
> **两模式覆盖**：所有命令在嵌入模式与网络模式（`--connect`）下行为一致。

### SQL

| 命令 | 说明 |
|------|------|
| `<SQL>;` | 直接执行裸 SQL（SELECT/INSERT/UPDATE/DELETE/CREATE/DROP/...） |
| `<SQL1>; <SQL2>` | 分号分隔多条 SQL（-c 模式） |
| `:sql begin` / `commit` / `rollback` | 事务控制（事务需在同一会话内） |
| `:sql exec <sql...>` | 执行裸 SQL（统一入口） |
| `:sql param "<sql with ?>" <v1> <v2> ...` | 参数化查询 |

### KV 引擎

| 命令 | 说明 |
|------|------|
| `:kv get <key>` | 读取 |
| `:kv set <key> <value> [--ttl <secs>]` | 写入（可选 TTL 秒） |
| `:kv del <key>` | 删除 |
| `:kv keys [prefix]` | 列出 key |
| `:kv scan <prefix> [limit]` | 扫描 key + value |
| `:kv count` | 总 key 数量 |
| `:kv exists <key>` | 检查是否存在 |
| `:kv incr <key>` | 原子自增 |
| `:kv incrby <key> <delta>` | 按 delta 自增（可负） |
| `:kv decrby <key> <delta>` | 按 delta 自减 |
| `:kv setnx <key> <value>` | 仅当不存在时设置 |
| `:kv expire <key> <secs>` | 设置已有 key 的过期时间 |
| `:kv ttl <key>` | 查看剩余 TTL |
| `:kv mset <k1> <v1> <k2> <v2> ...` | 批量写入（参数成对） |
| `:kv mget <k1> <k2> ...` | 批量读取 |
| `:kv rename <src> <dst>` | 重命名键 |

### MQ 引擎

| 命令 | 说明 |
|------|------|
| `:mq topics` | 列出所有 topic |
| `:mq create <topic> [max_len]` | 建 topic（max_len=0 无限制） |
| `:mq len <topic>` | 队列长度 |
| `:mq pub <topic> <msg>` | 发布消息 |
| `:mq poll <topic> <group> <consumer> [count]` | 消费消息（默认 10 条） |
| `:mq ack <topic> <group> <consumer> <msg_id>` | 确认消息已消费 |
| `:mq sub <topic> <group>` | 订阅 topic |
| `:mq unsub <topic> <group>` | 取消订阅 |

### 全文搜索

| 命令 | 说明 |
|------|------|
| `:fts create <name>` | 建立全文索引 |
| `:fts drop <name>` | 删除全文索引 |
| `:fts index <name> <doc_id> <fields_json>` | 写入/更新文档（fields 为 JSON object） |
| `:fts get <name> <doc_id>` | 取回文档字段 |
| `:fts del <name> <doc_id>` | 删除文档 |
| `:fts reindex <name>` | 重建索引 |
| `:fts search <name> <query>` | BM25 搜索 |

### 图引擎

| 命令 | 说明 |
|------|------|
| `:graph count <name>` | 节点/边计数 |
| `:graph vertex <name> <id>` | 查看节点 |
| `:graph neighbors <name> <id> [out\|in\|both]` | 邻居节点 |
| `:graph bfs <name> <start> [depth]` | BFS 遍历 |
| `:graph add-vertex <name> <label> [props_json]` | 加顶点，返回新 id |
| `:graph add-edge <name> <from> <to> <label> [props_json]` | 加边，返回新 id |
| `:graph update-vertex <name> <id> <props_json>` | 更新顶点属性 |
| `:graph del-vertex <name> <id>` | 删顶点（级联删关联边） |
| `:graph del-edge <name> <edge_id>` | 删边 |
| `:graph edges <name> <vertex_id> [out\|in]` | 出/入边列表 |
| `:graph path <name> <from> <to> [max_depth]` | 最短路径（默认深度 10） |
| `:graph pagerank <name> [limit]` | PageRank top-N（默认 10） |

### 向量引擎

| 命令 | 说明 |
|------|------|
| `:vec count <name>` | 向量索引数量 |
| `:vec insert <name> <id> <v1,v2,...>` | 插入向量（id=u64，逗号分隔 f32） |
| `:vec search <name> <k> <v1,v2,...>` | KNN 搜索（cosine，返回 top-k） |
| `:vec get <name> <id>` | 取回单条向量 |
| `:vec delete <name> <id>` | 删除向量 |
| `:vec rebuild <name>` | 重建 HNSW 索引 |

### 地理空间

| 命令 | 说明 |
|------|------|
| `:geo members <name>` | 列出成员 |
| `:geo count <name>` | 成员数量 |
| `:geo add <name> <key> <lng> <lat>` | 添加位置 |
| `:geo del <name> <key>` | 删除位置 |
| `:geo pos <name> <key>` | 查询坐标 |
| `:geo dist <name> <key1> <key2> [m\|km\|mi]` | 两点距离（默认 m） |
| `:geo hash <name> <key>` | 查询 Geohash |
| `:geo search <name> <lng> <lat> <r>` | 圆形搜索 |

### 时序引擎

| 命令 | 说明 |
|------|------|
| `:ts list` | 列出时序名称 |
| `:ts info <name>` | 查看时序详情 |
| `:ts create <name> <tag1,tag2> <field1,field2>` | 建时序表 |
| `:ts insert <name> <ts_ms> <k=v,...> <k=v,...>` | 写入数据点（毫秒时间戳） |
| `:ts query <name> [limit]` | 查询数据点 |
| `:ts agg <name> <field> <func> <interval_ms>` | 聚合（sum/avg/min/max/count/first/last/stddev，0=全局） |
| `:ts retention <name> <ms>` | 设保留策略（0=永久） |

### 集群运维

| 命令 | 说明 |
|------|------|
| `:cluster status` | 集群状态（角色/epoch/LSN/副本） |
| `:cluster role` | 当前节点角色 |
| `:cluster epoch` | 当前 epoch |
| `:cluster promote` | 提升为 Primary |
| `:cluster step-down <addr> <epoch>` | 降为 Replica |

### 备份恢复

| 命令 | 说明 |
|------|------|
| `:backup export <dir> [ks...]` | 导出快照（可选 keyspace，空=全部） |
| `:backup import <dir>` | 从目录导入 |
| `:backup snapshot <dir> [ts_ms]` | 一致性快照 PITR |
| `:backup restore <dir> latest\|snapshot\|lsn <n>\|ts <ms>` | 恢复到指定点 |

### OpLog

| 命令 | 说明 |
|------|------|
| `:oplog lsn` | 当前 LSN |
| `:oplog get <lsn>` | 取单条 |
| `:oplog range <from> <to> [limit]` | 取范围 |

### AI 引擎（talon-ai）

| 命令 | 说明 |
|------|------|
| `:ai sessions` | 列出活跃 Session |
| `:ai session <id>` | 查看 Session 详情 |
| `:ai history <sid> [limit]` | 对话历史 |
| `:ai memory count` | 记忆数量 |
| `:ai docs list` | 列出 RAG 文档 |
| `:ai docs count` | RAG 文档数量 |

### Agent 执行状态

| 命令 | 说明 |
|------|------|
| `:agent status trace <id>` | 按 TeamTrace ID 查询当前执行状态 |
| `:agent status plan <id>` | 按计划 ID 查询当前执行状态 |
| `:agent checkpoint trace <id> --consumer <id> [--group <id>]` | 查询 TeamTrace 流消费位点 |
| `:agent checkpoint plan <id> --consumer <id> [--group <id>]` | 查询计划流消费位点 |
| `:agent watch trace <id>` | 订阅 TeamTrace 执行状态流，持续输出直到终态 |
| `:agent watch plan <id>` | 订阅计划执行状态流，持续输出直到终态 |

示例：

```bash
# 查询当前快照
talon-cli ./db -c ":agent status trace team-trace-123"

# 订阅执行中的状态流
talon-cli ./db -c ":agent watch trace team-trace-123"

# 用稳定 consumer 做断点续消费
talon-cli ./db -c ":agent watch trace team-trace-123 --consumer desktop-pane-a"

# 查询某个 consumer 当前 checkpoint
talon-cli ./db -c ":agent checkpoint trace team-trace-123 --consumer desktop-pane-a"

# 从指定 seq 恢复或重放
talon-cli ./db -c ":agent watch trace team-trace-123 --consumer desktop-pane-a --from-seq 12"

# 回放最近 5 条已确认事件
talon-cli ./db -c ":agent watch plan plan-123 --consumer dashboard-main --replay-last 5"

# JSON 模式下输出完整流式事件 envelope
talon-cli ./db --format json -c ":agent watch plan plan-123"
```

`watch` 会输出增强后的流式事件契约，包含：

- MQ 顺序元信息（`seq`、`publishedAt`）
- 流协议元信息（`eventId`、`streamVersion`）
- 流作用域（`scope`、`scopeId`）
- 事件类型（`initial_snapshot`、`running_snapshot`、`progress_snapshot`、`final_snapshot`）
- 变更子任务集合（`changedTaskIds`）
- 稳定的 `ExecutionStatusResponse` 快照负载

消费语义：

- `--consumer` 用稳定 consumer 名称启用断点续消费；不指定时，CLI 会生成临时 consumer
- `:agent checkpoint ... --consumer <id>` 可直接查看该 consumer 的 `lastAckedSeq`、`nextSeq` 和 `pendingCount`
- `--from-seq <n>` 会把该 consumer 的下次消费位置重置到指定 seq
- `--replay-last <n>` 会基于该 consumer 最近一次已确认位置，回放最近 `n` 条已确认事件
- replay 会重新收到旧事件，消费侧应使用 `eventId` 做幂等去重

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
├── main.rs          # 入口、三模路由、REPL 循环
├── daemon.rs        # Daemon 模式（Unix Socket 持久化连接）
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

## ⚡ 性能

| 模式 | 响应时间 | 说明 |
|------|----------|------|
| Daemon（Unix Socket） | **~7ms** | 数据库已打开，纯 IPC 通信 |
| 嵌入式（直接打开） | ~500ms | 每次 journal recovery |
| Debug 编译 | ~8.4s | 仅开发环境 |

## 📄 License

Talon Community Dual License Agreement.
