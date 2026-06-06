/*
 * Talon CLI — 嵌入式 + 网络 + Daemon 三模数据库命令行工具
 *
 * 用法：
 *   talon-cli <DB_PATH>                       嵌入式 REPL
 *   talon-cli <DB_PATH> -c "SQL"              单次执行（自动走 daemon 加速）
 *   talon-cli serve <DB_PATH>                 启动 daemon（数据库只打开一次）
 *   talon-cli --connect <host:port>           网络模式（TCP 帧协议）
 *   talon-cli --format json -c ":kv get k"    JSON 输出（AI 友好）
 *
 * 架构：
 *   main.rs      — 入口、三模路由、REPL 循环
 *   daemon.rs    — Daemon 模式（Unix Socket 持久化连接）
 *   format.rs    — Value 格式化工具
 *   net.rs       — 网络后端（TCP 帧协议）
 *   engine/      — 引擎命令处理（11 引擎）
 */

mod cmdline;
mod daemon;
mod engine;
mod format;
mod net;

use clap::Parser;
use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};

/// 全局错误标志 — `-c` 模式下任何命令执行失败时设为 true，
/// 进程退出时据此返回非零 exit code，让 AI 能通过 `$?` 判断成败。
static HAD_ERROR: AtomicBool = AtomicBool::new(false);

/// 输出格式。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// 人类可读的表格/文本格式（默认）。
    Human,
    /// JSON 格式 — 每个命令输出一行 JSON，AI 友好。
    Json,
}

#[derive(Parser)]
#[command(
    name = "talon-cli",
    version = "0.1.0",
    about = "Talon CLI — 嵌入式 + 网络 + Daemon 三模数据库工具（AI 友好）"
)]
struct Args {
    /// 数据库目录路径（嵌入模式），或 "serve" 启动 daemon
    db_path: Option<String>,

    /// serve 模式下的数据库路径
    serve_db_path: Option<String>,

    /// 连接到运行中的 Talon Server TCP 端口（格式: host:port）
    #[arg(long)]
    connect: Option<String>,

    /// 网络模式认证 token
    #[arg(long)]
    token: Option<String>,

    /// 直接执行命令后退出（自动走 daemon 加速）
    #[arg(short, long)]
    cmd: Option<String>,

    /// 输出格式: human（默认）或 json（AI 友好）
    #[arg(long, default_value = "human")]
    format: String,
}

fn main() {
    let args = Args::parse();

    let fmt = match args.format.as_str() {
        "json" => OutputFormat::Json,
        _ => OutputFormat::Human,
    };

    // 三模路由
    if let Some(ref addr) = args.connect {
        // 模式 1: 网络模式（TCP 帧协议）
        run_net_mode(addr, args.token.as_deref(), args.cmd.as_deref(), fmt);
    } else if let Some(ref db_path) = args.db_path {
        if db_path == "serve" {
            // 模式 2: Daemon 模式 — 启动后台服务
            let serve_path = args.serve_db_path.as_deref().unwrap_or_else(|| {
                eprintln!("用法: talon-cli serve <DB_PATH>");
                std::process::exit(1);
            });
            daemon::run_daemon(serve_path);
        } else {
            // 模式 3: 嵌入模式（-c 命令自动走 daemon 加速）
            run_embedded_mode(db_path, args.cmd.as_deref(), fmt);
        }
    } else {
        eprintln!("用法: talon-cli <DB_PATH>               (嵌入模式)");
        eprintln!("      talon-cli serve <DB_PATH>          (daemon 模式)");
        eprintln!("      talon-cli --connect <host:port>    (网络模式)");
        eprintln!("\n使用 --help 查看详细帮助。");
        std::process::exit(1);
    }

    // -c 模式下，有错误则非零退出，让 AI 能通过 $? 判断成败。
    if HAD_ERROR.load(Ordering::Relaxed) {
        std::process::exit(1);
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  嵌入模式 — 直接打开本地数据库
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn run_embedded_mode(db_path: &str, cmd: Option<&str>, fmt: OutputFormat) {
    // -c 模式：优先尝试 daemon 连接（毫秒级）
    if let Some(c) = cmd {
        if daemon::try_daemon_exec(db_path, c, fmt) {
            return; // daemon 搞定了，不用打开数据库
        }
        // 无 daemon → fallback 到嵌入模式（需要打开数据库）
    }

    let db = match talon::Talon::open(db_path) {
        Ok(db) => db,
        Err(e) => {
            if fmt == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({"ok":false,"error":format!("打开数据库失败: {}", e)})
                );
            } else {
                eprintln!("❌ 打开数据库失败 '{}': {}", db_path, e);
            }
            std::process::exit(1);
        }
    };

    // 单次执行
    if let Some(c) = cmd {
        for stmt in split_commands(c) {
            let trimmed = stmt.trim();
            if trimmed.is_empty() {
                continue;
            }
            execute_embedded(&db, trimmed, fmt);
        }
        return;
    }

    // 交互模式
    if fmt == OutputFormat::Human {
        println!("Talon CLI v0.1.0 — 嵌入模式，已打开: {}", db_path);
        println!("输入 SQL (以 ; 结尾) 或 :help 查看引擎命令。:quit 退出。\n");
    }

    let stdin = std::io::stdin();
    let reader = std::io::BufReader::new(stdin.lock());
    let mut lines = reader.lines();

    loop {
        if fmt == OutputFormat::Human {
            print!("talon> ");
            std::io::stdout().flush().ok();
        }

        let line = match lines.next() {
            Some(Ok(l)) => l,
            _ => break,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == ":quit" || trimmed == ":exit" || trimmed == ":q" {
            break;
        }

        execute_embedded(&db, trimmed, fmt);
    }
    if fmt == OutputFormat::Human {
        println!("\n再见！");
    }
}

fn execute_embedded(db: &talon::Talon, input: &str, fmt: OutputFormat) {
    if input.starts_with(':') {
        dispatch_engine(db, input, fmt);
    } else {
        engine::sql::handle(db, input, fmt, &HAD_ERROR);
    }
}

/// 路由 `:engine subcmd ...` 到对应的引擎处理模块。
fn dispatch_engine(db: &talon::Talon, input: &str, fmt: OutputFormat) {
    // 用统一分词器（支持引号包裹、不限段数）替代写死的 splitn(4)。
    let owned = cmdline::tokenize(input);
    if owned.is_empty() {
        return;
    }
    let parts: Vec<&str> = owned.iter().map(|s| s.as_str()).collect();

    match parts[0] {
        ":help" | ":h" | ":?" => {
            if fmt == OutputFormat::Json {
                println!("{}", serde_json::json!({"ok":true,"type":"help","message":"Use :help in human mode for full command list"}));
            } else {
                print_help();
            }
        }
        ":stats" => engine::stats::handle(db, fmt),
        ":kv" => engine::kv::handle(db, &parts, fmt, &HAD_ERROR),
        ":mq" => engine::mq::handle(db, &parts, fmt, &HAD_ERROR),
        ":fts" => engine::fts::handle(db, &parts, fmt, &HAD_ERROR),
        ":graph" => engine::graph::handle(db, &parts, fmt, &HAD_ERROR),
        ":geo" => engine::geo::handle(db, &parts, fmt, &HAD_ERROR),
        ":ts" => engine::ts::handle(db, &parts, fmt, &HAD_ERROR),
        ":vec" => engine::vec::handle(db, &parts, fmt, &HAD_ERROR),
        ":ai" => engine::ai::handle(db, &parts, fmt, &HAD_ERROR),
        ":agent" => engine::agent::handle(db, &parts, fmt, &HAD_ERROR),
        ":evo" => engine::evo::handle(db, &parts, fmt, &HAD_ERROR),
        ":sql" => engine::sql::handle_cmd(db, &parts, fmt, &HAD_ERROR),
        ":cluster" => engine::cluster::handle(db, &parts, fmt, &HAD_ERROR),
        ":backup" => engine::backup::handle(db, &parts, fmt, &HAD_ERROR),
        ":oplog" => engine::backup::handle_oplog(db, &parts, fmt, &HAD_ERROR),
        _ => {
            let msg = format!("未知命令: {}。输入 :help 查看帮助。", parts[0]);
            report_error(&msg, fmt);
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  网络模式 — 连接到 Talon Server TCP 协议
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn run_net_mode(addr: &str, token: Option<&str>, cmd: Option<&str>, fmt: OutputFormat) {
    let mut backend = match net::NetBackend::connect(addr, token) {
        Ok(b) => b,
        Err(e) => {
            if fmt == OutputFormat::Json {
                println!("{}", serde_json::json!({"ok":false,"error":e}));
            } else {
                eprintln!("❌ {}", e);
            }
            std::process::exit(1);
        }
    };

    // 单次执行（支持分号分隔多条命令）
    if let Some(c) = cmd {
        for stmt in split_commands(c) {
            let trimmed = stmt.trim();
            if trimmed.is_empty() {
                continue;
            }
            execute_net(&mut backend, trimmed, fmt);
        }
        return;
    }

    // 交互模式
    if fmt == OutputFormat::Human {
        println!("Talon CLI v0.1.0 — 网络模式，已连接: {}", addr);
        println!("输入 SQL (以 ; 结尾) 或 :help 查看引擎命令。:quit 退出。\n");
    }

    let stdin = std::io::stdin();
    let reader = std::io::BufReader::new(stdin.lock());
    let mut lines = reader.lines();

    loop {
        if fmt == OutputFormat::Human {
            print!("talon@{}> ", addr);
            std::io::stdout().flush().ok();
        }

        let line = match lines.next() {
            Some(Ok(l)) => l,
            _ => break,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == ":quit" || trimmed == ":exit" || trimmed == ":q" {
            break;
        }
        if trimmed == ":help" || trimmed == ":h" || trimmed == ":?" {
            if fmt == OutputFormat::Human {
                print_help();
            } else {
                println!("{}", serde_json::json!({"ok":true,"type":"help"}));
            }
            continue;
        }

        execute_net(&mut backend, trimmed, fmt);
    }
    if fmt == OutputFormat::Human {
        println!("\n再见！");
    }
}

fn execute_net(backend: &mut net::NetBackend, input: &str, fmt: OutputFormat) {
    let json = match net::input_to_json(input) {
        Ok(j) => j,
        Err(e) => {
            report_error(&format!("命令解析错误: {}", e), fmt);
            return;
        }
    };

    match backend.send_cmd(&json) {
        Ok(resp) => {
            if fmt == OutputFormat::Json {
                // 网络模式已经返回 JSON，直接透传
                println!("{}", resp);
            } else {
                net::print_net_response(&resp);
            }
        }
        Err(e) => report_error(&format!("通信错误: {}", e), fmt),
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  公共工具函数
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// 输出错误信息（根据格式决定输出方式），并标记全局错误标志。
pub fn report_error(msg: &str, fmt: OutputFormat) {
    HAD_ERROR.store(true, Ordering::Relaxed);
    if fmt == OutputFormat::Json {
        println!("{}", serde_json::json!({"ok":false,"error":msg}));
    } else {
        eprintln!("{}", msg);
    }
}

/// 将 `-c` 输入拆分为多条命令。
///
/// 规则：
/// - 冒号命令（`:kv set ...`）本身就是一条完整命令
/// - SQL 以 `;` 分隔多条
pub(crate) fn split_commands(input: &str) -> Vec<&str> {
    let trimmed = input.trim();
    if trimmed.starts_with(':') {
        // 引擎命令不拆分
        vec![trimmed]
    } else {
        // SQL 按分号拆分
        trimmed.split(';').collect()
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  帮助信息
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub(crate) fn print_help() {
    println!(
        r#"Talon CLI v0.1.0 — 嵌入式 / 网络 / Daemon 三模数据库工具（AI 友好）

  启动模式：
    talon-cli <DB_PATH>                        嵌入式（直接打开本地数据库）
    talon-cli serve <DB_PATH>                  启动 Daemon（数据库只打开一次，极速）
    talon-cli --connect <host:port>            网络（连接 Talon Server TCP）
    talon-cli --connect <host:port> --token T  带认证的网络模式
    talon-cli --format json ...                JSON 输出（AI/脚本友好）

  参数引用：含空格的值或 JSON 用引号包裹整段，例：
    :kv set greeting "hello world"
    :fts index arts d1 '{{"title":"hello world"}}'
  不含空格的紧凑 JSON 可裸传，例：{{"k":"v"}}。
  所有命令在嵌入模式与网络模式（--connect）下行为一致。

── SQL 引擎 ──
  <sql>;                                       直接执行裸 SQL
  :sql begin / commit / rollback               事务控制
  :sql exec <sql...>                           执行裸 SQL（统一入口）
  :sql param "<sql with ?>" <v1> <v2> ...      参数化查询（? 占位绑定）

── KV 引擎 ──
  :kv get <key>                                读取
  :kv set <key> <value> [--ttl <secs>]         写入（可选 TTL 秒）
  :kv del <key>                                删除
  :kv keys [prefix]                            列出 key（可选前缀过滤）
  :kv scan <prefix> [limit]                    扫描并显示 key + value
  :kv count                                    总 key 数量
  :kv exists <key>                             检查是否存在
  :kv incr <key>                               原子自增
  :kv incrby <key> <delta>                     按 delta 自增（可负）
  :kv decrby <key> <delta>                     按 delta 自减
  :kv setnx <key> <value>                      仅当不存在时设置
  :kv expire <key> <secs>                      设置已有 key 的过期时间
  :kv ttl <key>                                查看剩余 TTL
  :kv mset <k1> <v1> <k2> <v2> ...             批量写入（参数成对）
  :kv mget <k1> <k2> ...                       批量读取
  :kv rename <src> <dst>                       重命名键

── MQ 引擎 ──
  :mq topics                                   列出所有 topic
  :mq create <topic> [max_len]                 建 topic（max_len=0 无限制）
  :mq len <topic>                              队列长度
  :mq pub <topic> <msg>                        发布消息
  :mq poll <topic> <group> <consumer> [count]  消费消息（默认 10 条）
  :mq ack <topic> <group> <consumer> <msg_id>  确认消息已消费
  :mq sub <topic> <group>                      订阅 topic
  :mq unsub <topic> <group>                    取消订阅

── 全文搜索 ──
  :fts create <name>                           建立全文索引
  :fts drop <name>                             删除全文索引
  :fts index <name> <doc_id> <fields_json>     写入/更新文档（fields 为 JSON object）
  :fts get <name> <doc_id>                     取回文档字段
  :fts del <name> <doc_id>                     删除文档
  :fts reindex <name>                          重建索引
  :fts search <name> <query>                   BM25 搜索

── 图引擎 ──
  :graph count <name>                          节点/边计数
  :graph vertex <name> <id>                    查看节点
  :graph neighbors <name> <id> [out|in|both]   邻居节点 ID
  :graph bfs <name> <start> [depth]            BFS 遍历
  :graph add-vertex <name> <label> [props_json]        加顶点，返回新 id
  :graph add-edge <name> <from> <to> <label> [props_json]  加边，返回新 id
  :graph update-vertex <name> <id> <props_json>        更新顶点属性
  :graph del-vertex <name> <id>                删顶点（级联删关联边）
  :graph del-edge <name> <edge_id>             删边
  :graph edges <name> <vertex_id> [out|in]     出/入边列表
  :graph path <name> <from> <to> [max_depth]   最短路径（默认深度 10）
  :graph pagerank <name> [limit]               PageRank top-N（默认 10）

── 向量引擎 ──
  :vec count <name>                            向量索引数量
  :vec insert <name> <id> <v1,v2,...>          插入向量（id=u64，逗号分隔 f32）
  :vec search <name> <k> <v1,v2,...>           KNN 搜索（cosine，返回 top-k）
  :vec get <name> <id>                         取回单条向量
  :vec delete <name> <id>                      删除向量
  :vec rebuild <name>                          重建 HNSW 索引

── 地理空间 ──
  :geo members <name>                          列出成员名
  :geo count <name>                            成员数量
  :geo add <name> <key> <lng> <lat>            添加位置
  :geo del <name> <key>                        删除位置
  :geo pos <name> <key>                        查询坐标
  :geo dist <name> <key1> <key2> [m|km|mi]     两点距离（默认 m）
  :geo hash <name> <key>                       查询 Geohash
  :geo search <name> <lng> <lat> <radius_m>    圆形搜索

── 时序引擎 ──
  :ts list                                     列出时序名称
  :ts info <name>                              查看时序详情
  :ts create <name> <tag1,tag2> <field1,field2>        建时序表
  :ts insert <name> <ts_ms> <k=v,...> <k=v,...>        写入数据点（毫秒时间戳）
  :ts query <name> [limit]                     查询数据点
  :ts agg <name> <field> <func> <interval_ms>  聚合（sum/avg/min/max/count/first/last/stddev，0=全局）
  :ts retention <name> <ms>                    设保留策略（0=永久）

── 集群运维 ──
  :cluster status                              集群状态（角色/epoch/LSN/副本）
  :cluster role                                当前节点角色
  :cluster epoch                               当前 epoch
  :cluster promote                             提升为 Primary
  :cluster step-down <addr> <epoch>            降为 Replica

── 备份恢复 ──
  :backup export <dir> [ks...]                 导出快照（可选 keyspace，空=全部）
  :backup import <dir>                         从目录导入
  :backup snapshot <dir> [ts_ms]               一致性快照 PITR
  :backup restore <dir> latest|snapshot|lsn <n>|ts <ms>  恢复到指定点

── OpLog ──
  :oplog lsn                                   当前 LSN
  :oplog get <lsn>                             取单条
  :oplog range <from> <to> [limit]             取范围

── AI 引擎（talon-ai）──
  :ai sessions               列出活跃 Session
  :ai session <id>           查看 Session 详情
  :ai history <sid> [limit]  对话历史
  :ai memory count           记忆数量
  :ai docs list              列出 RAG 文档
  :ai docs count             RAG 文档数量

── Agent Runtime（talon-agent）──
    :agent discover [query] [--category <name>] [--readonly] [--concurrency-safe] [--limit <n>]  查询内置工具目录
    :agent status trace <id>   按 TeamTrace ID 查询执行状态
    :agent status plan <id>    按 Plan ID 查询最新执行状态
    :agent checkpoint trace <id> --consumer <id> [--group <id>]  查询 TeamTrace 流消费位点
    :agent checkpoint plan <id> --consumer <id> [--group <id>]   查询 Plan 流消费位点
    :agent watch trace <id> [--consumer <id>] [--from-seq <n>|--replay-last <n>]  订阅或回放 TeamTrace 执行状态流
    :agent watch plan <id> [--consumer <id>] [--from-seq <n>|--replay-last <n>]   订阅或回放 Plan 执行状态流

── EvoCore 进化引擎 ──
  :evo soul                  查看 Soul 配置
  :evo personality           查看个性维度
  :evo history               Soul 进化历史
  :evo proposals             进化提议列表
  :evo stats                 学习统计

── 系统 ──
  :stats                     数据库统计信息
  :help                      显示本帮助
  :quit / :exit              退出"#
    );
}
