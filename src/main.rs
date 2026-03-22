/*
 * Talon CLI — 嵌入式 + 网络双模数据库命令行工具
 *
 * 用法：
 *   talon-cli <DB_PATH>                       嵌入式 REPL（直接打开本地数据库）
 *   talon-cli --connect <host:port>           网络模式（连接运行中的 Talon Server）
 *   talon-cli --connect <host:port> --token T 带认证的网络模式
 *   talon-cli <DB_PATH> -c "SQL语句"          嵌入式单次执行
 *   talon-cli <DB_PATH> -c "SQL1; SQL2"       多条 SQL 分号分隔
 *   talon-cli --format json -c ":kv get k"    JSON 输出（AI 友好）
 *
 * 架构：
 *   main.rs      — 入口、双模路由、REPL 循环
 *   format.rs    — Value 格式化工具（嵌入模式）
 *   net.rs       — 网络后端（TCP 帧协议客户端）
 *   engine/      — 嵌入模式引擎命令处理（kv, mq, fts, graph, geo, ts, vec, sql, stats）
 */

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
    about = "Talon CLI — 嵌入式 + 网络双模数据库工具（AI 友好）"
)]
struct Args {
    /// 数据库目录路径（嵌入模式）
    db_path: Option<String>,

    /// 连接到运行中的 Talon Server TCP 端口（格式: host:port）
    #[arg(long)]
    connect: Option<String>,

    /// 网络模式认证 token
    #[arg(long)]
    token: Option<String>,

    /// 直接执行命令后退出（支持分号分隔多条 SQL）
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

    // 双模路由
    if let Some(ref addr) = args.connect {
        run_net_mode(addr, args.token.as_deref(), args.cmd.as_deref(), fmt);
    } else if let Some(ref db_path) = args.db_path {
        run_embedded_mode(db_path, args.cmd.as_deref(), fmt);
    } else {
        eprintln!("用法: talon-cli <DB_PATH>             (嵌入模式)");
        eprintln!("      talon-cli --connect <host:port>  (网络模式)");
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

    // 单次执行（支持分号分隔多条命令）
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
    let parts: Vec<&str> = input.splitn(4, ' ').collect();

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
        ":evo" => engine::evo::handle(db, &parts, fmt, &HAD_ERROR),
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
fn split_commands(input: &str) -> Vec<&str> {
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

fn print_help() {
    println!(
        r#"Talon CLI v0.1.0 — 嵌入式/网络双模数据库工具（AI 友好）

  启动模式：
    talon-cli <DB_PATH>                        嵌入式（直接打开本地数据库）
    talon-cli --connect <host:port>            网络（连接 Talon Server TCP）
    talon-cli --connect <host:port> --token T  带认证的网络模式
    talon-cli --format json ...                JSON 输出（AI/脚本友好）

  <SQL>;              执行 SQL（SELECT/INSERT/UPDATE/DELETE/CREATE/DROP/...）
  <SQL1>; <SQL2>      分号分隔多条 SQL（-c 模式）

── KV 引擎 ──
  :kv get <key>              读取
  :kv set <key> <value>      写入
  :kv del <key>              删除
  :kv keys [prefix]          列出 key（可选前缀过滤）
  :kv scan <prefix> [limit]  扫描并显示 key + value
  :kv count                  总 key 数量
  :kv exists <key>           检查是否存在
  :kv incr <key>             原子自增
  :kv ttl <key>              查看剩余 TTL

── MQ 引擎 ──
  :mq topics                 列出所有 topic
  :mq len <topic>            队列长度
  :mq pub <topic> <msg>      发布消息

── 全文搜索 ──
  :fts search <name> <query> BM25 搜索

── 图引擎 ──
  :graph count <name>                          节点/边计数
  :graph neighbors <name> <id> [out|in|both]   邻居节点 ID
  :graph bfs <name> <start> [depth]            BFS 遍历
  :graph vertex <name> <id>                    查看节点

── 向量引擎 ──
  :vec count <name>          向量索引数量

── 地理空间 ──
  :geo members <name>                          列出成员名
  :geo count <name>                            成员数量
  :geo search <name> <lng> <lat> <radius_m>    圆形搜索

── 时序引擎 ──
  :ts list                   列出时序名称
  :ts info <name>            查看时序详情

── AI 引擎（talon-ai）──
  :ai sessions               列出活跃 Session
  :ai session <id>           查看 Session 详情
  :ai history <sid> [limit]  对话历史
  :ai memory count           记忆数量
  :ai docs list              列出 RAG 文档
  :ai docs count             RAG 文档数量

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
