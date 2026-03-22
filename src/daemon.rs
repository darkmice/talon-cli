//! Daemon 模式 — Unix Socket 持久化连接。
//!
//! 数据库只打开一次，后续 `-c` 命令通过 Unix Socket 毫秒级通信。
//!
//! ## 架构
//!
//! ```text
//! [AI Agent]                    [Daemon]
//!   talon-cli -c "..."   →   Unix Socket   →   已打开的 Talon DB
//!   stdout ← 结果        ←   响应回传       ←   查询结果
//! ```
//!
//! ## 协议（极简行协议）
//!
//! 请求：`<format>\n<command>\n`  
//! 响应：`<output lines>\n\x04\n` (成功) 或 `<output>\n\x04ERR\n` (失败)

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::Ordering;

use crate::{engine, OutputFormat, HAD_ERROR};

/// 根据数据库路径生成 socket 文件路径。
pub fn socket_path(db_path: &str) -> PathBuf {
    let hash = simple_hash(db_path);
    PathBuf::from(format!("/tmp/talon-cli-{:x}.sock", hash))
}

/// 尝试连接已运行的 daemon 执行命令。
///
/// 返回 `true` 表示通过 daemon 成功执行。
/// 返回 `false` 表示无 daemon 运行，应 fallback 到嵌入模式。
pub fn try_daemon_exec(db_path: &str, cmd: &str, fmt: OutputFormat) -> bool {
    let sock = socket_path(db_path);
    if !sock.exists() {
        return false;
    }

    let stream = match UnixStream::connect(&sock) {
        Ok(s) => s,
        Err(_) => {
            // socket 文件存在但连不上 → 残留文件，清理
            std::fs::remove_file(&sock).ok();
            return false;
        }
    };
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(30)))
        .ok();

    let mut writer = stream.try_clone().expect("clone socket");
    let reader = BufReader::new(&stream);

    // 发送: format\ncmd\n
    let fmt_str = if fmt == OutputFormat::Json { "json" } else { "human" };
    write!(writer, "{}\n{}\n", fmt_str, cmd).ok();
    writer.flush().ok();

    // 读取响应
    let mut had_error = false;
    for line in reader.lines() {
        match line {
            Ok(l) => {
                if l == "\x04" {
                    break;
                }
                if l == "\x04ERR" {
                    had_error = true;
                    break;
                }
                println!("{}", l);
            }
            Err(_) => break,
        }
    }

    if had_error {
        HAD_ERROR.store(true, Ordering::Relaxed);
    }
    true
}

/// 启动 daemon — 监听 Unix Socket，保持数据库打开。
pub fn run_daemon(db_path: &str) {
    let sock_path = socket_path(db_path);

    // 清理旧 socket
    if sock_path.exists() {
        // 先试连接，如果能连通说明已有 daemon 在运行
        if UnixStream::connect(&sock_path).is_ok() {
            eprintln!("❌ 已有 Daemon 在运行 ({})", sock_path.display());
            std::process::exit(1);
        }
        std::fs::remove_file(&sock_path).ok();
    }

    // 打开数据库（只做一次！）
    let db = match talon::Talon::open(db_path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("❌ 打开数据库失败 '{}': {}", db_path, e);
            std::process::exit(1);
        }
    };

    let listener = match UnixListener::bind(&sock_path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("❌ 绑定 socket 失败: {}", e);
            std::process::exit(1);
        }
    };

    // 注册信号处理 — 退出时删除 socket 文件
    let cleanup_path = sock_path.clone();
    ctrlc::set_handler(move || {
        std::fs::remove_file(&cleanup_path).ok();
        std::process::exit(0);
    })
    .ok();

    eprintln!(
        "🚀 Talon CLI Daemon 已启动\n   数据库: {}\n   Socket: {}\n   Ctrl+C 停止",
        db_path,
        sock_path.display()
    );

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => handle_client(&db, stream),
            Err(e) => eprintln!("连接错误: {}", e),
        }
    }

    std::fs::remove_file(&sock_path).ok();
}

/// 处理单个客户端连接。
///
/// 核心技巧：将 stdout (fd=1) 临时重定向到 client socket，
/// 这样所有 `println!` 输出直接发送给客户端，无需修改引擎代码。
fn handle_client(db: &talon::Talon, stream: UnixStream) {
    let mut reader = BufReader::new(&stream);

    // 读取格式
    let mut fmt_line = String::new();
    if reader.read_line(&mut fmt_line).is_err() {
        return;
    }
    let fmt = if fmt_line.trim() == "json" {
        OutputFormat::Json
    } else {
        OutputFormat::Human
    };

    // 读取命令
    let mut cmd_line = String::new();
    if reader.read_line(&mut cmd_line).is_err() {
        return;
    }
    let cmd = cmd_line.trim().to_string();
    if cmd.is_empty() {
        return;
    }

    // 🔑 关键：重定向 stdout 到 socket
    let sock_fd = {
        use std::os::unix::io::AsRawFd;
        stream.as_raw_fd()
    };
    let saved_stdout = unsafe { libc::dup(1) };
    unsafe {
        libc::dup2(sock_fd, 1);
    }

    // 执行命令
    let had_error = std::sync::atomic::AtomicBool::new(false);
    for stmt in crate::split_commands(&cmd) {
        let trimmed = stmt.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with(':') {
            dispatch_engine(db, trimmed, fmt, &had_error);
        } else {
            engine::sql::handle(db, trimmed, fmt, &had_error);
        }
    }

    // flush stdout（现在指向 socket）
    std::io::stdout().flush().ok();

    // 恢复 stdout
    unsafe {
        libc::dup2(saved_stdout, 1);
        libc::close(saved_stdout);
    }

    // 发送结束标记（直接写 socket，不走 stdout）
    let mut writer = &stream;
    if had_error.load(Ordering::Relaxed) {
        write!(writer, "\x04ERR\n").ok();
    } else {
        write!(writer, "\x04\n").ok();
    }
    writer.flush().ok();
}

/// 引擎命令分发（与 main.rs 中 dispatch_engine 相同逻辑）。
fn dispatch_engine(
    db: &talon::Talon,
    input: &str,
    fmt: OutputFormat,
    had_error: &std::sync::atomic::AtomicBool,
) {
    let parts: Vec<&str> = input.splitn(4, ' ').collect();
    match parts[0] {
        ":help" | ":h" | ":?" => {
            if fmt == OutputFormat::Json {
                println!("{}", serde_json::json!({"ok":true,"type":"help"}));
            } else {
                crate::print_help();
            }
        }
        ":stats" => engine::stats::handle(db, fmt),
        ":kv" => engine::kv::handle(db, &parts, fmt, had_error),
        ":mq" => engine::mq::handle(db, &parts, fmt, had_error),
        ":fts" => engine::fts::handle(db, &parts, fmt, had_error),
        ":graph" => engine::graph::handle(db, &parts, fmt, had_error),
        ":geo" => engine::geo::handle(db, &parts, fmt, had_error),
        ":ts" => engine::ts::handle(db, &parts, fmt, had_error),
        ":vec" => engine::vec::handle(db, &parts, fmt, had_error),
        ":ai" => engine::ai::handle(db, &parts, fmt, had_error),
        ":evo" => engine::evo::handle(db, &parts, fmt, had_error),
        _ => {
            had_error.store(true, Ordering::Relaxed);
            if fmt == OutputFormat::Json {
                println!("{}", serde_json::json!({"ok":false,"error":format!("未知命令: {}", parts[0])}));
            } else {
                eprintln!("未知命令: {}", parts[0]);
            }
        }
    }
}

fn simple_hash(s: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in s.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}
