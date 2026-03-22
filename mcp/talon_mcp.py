#!/usr/bin/env python3
"""
Talon MCP Server — 让 AI 原生访问 Talon 数据库的全部 11 引擎。

底层调用 talon-cli（复用其全部能力：SQL/KV/MQ/FTS/Graph/Geo/Vec/TS/AI/EvoCore），
MCP 层只做 Tool 定义 + JSON 桥接。

连接模式：
    --connect host:port    通过 TCP 连接运行中的 Talon Server（如 SuperClaw :7720）
    --db-path <PATH>       通过本地嵌入模式（自动尝试 daemon 加速）

IDE 配置示例：

    {
        "mcpServers": {
            "talon": {
                "command": "/path/to/.venv/bin/python3",
                "args": [
                    "/path/to/talon-cli/mcp/talon_mcp.py",
                    "--connect", "127.0.0.1:7720"
                ]
            }
        }
    }
"""

import argparse
import json
import os
import subprocess
import sys
from typing import Any

from mcp.server.fastmcp import FastMCP

# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
#  talon-cli 后端
# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

# 全局配置
TALON_CLI: str = ""  # talon-cli 二进制路径
CONNECT_ADDR: str = ""  # --connect host:port
DB_PATH: str = ""  # 嵌入模式数据库路径
CONNECT_TOKEN: str | None = None


def _run_cli(cmd: str) -> dict[str, Any]:
    """调用 talon-cli 执行命令，返回 JSON 结果。

    自动选择连接模式（网络 / 嵌入），使用 --format json 保证结构化输出。
    """
    args = [TALON_CLI, "--format", "json"]

    if CONNECT_ADDR:
        args.extend(["--connect", CONNECT_ADDR])
        if CONNECT_TOKEN:
            args.extend(["--token", CONNECT_TOKEN])
    elif DB_PATH:
        args.append(DB_PATH)
    else:
        return {"ok": False, "error": "未配置连接方式。请指定 --connect 或 --db-path"}

    args.extend(["-c", cmd])

    try:
        result = subprocess.run(
            args,
            capture_output=True,
            text=True,
            timeout=30,
        )

        stdout = result.stdout.strip()
        if not stdout:
            if result.returncode != 0:
                err = result.stderr.strip() or "命令执行失败（无输出）"
                return {"ok": False, "error": err}
            return {"ok": True}

        # 可能有多行 JSON（多条命令），取最后一个有效 JSON
        lines = stdout.split("\n")
        for line in reversed(lines):
            line = line.strip()
            if line.startswith("{"):
                try:
                    return json.loads(line)
                except json.JSONDecodeError:
                    continue

        # 非 JSON 输出
        return {"ok": result.returncode == 0, "output": stdout}

    except subprocess.TimeoutExpired:
        return {"ok": False, "error": "命令超时（30s）"}
    except FileNotFoundError:
        return {"ok": False, "error": f"找不到 talon-cli: {TALON_CLI}"}
    except Exception as e:
        return {"ok": False, "error": str(e)}


# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
#  MCP Server
# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

mcp = FastMCP(
    "Talon Database",
    instructions=(
        "Talon 多模融合数据引擎 MCP Server。"
        "通过 talon-cli 连接运行中的 Talon Server 或本地数据库，"
        "支持 SQL、KV、MQ、向量、时序、全文搜索、图、地理空间、AI、EvoCore 等 11 大引擎。"
    ),
)


# ━━━━ SQL 引擎 ━━━━


@mcp.tool()
def talon_sql_query(sql: str) -> dict:
    """执行 SQL 查询或语句。

    支持 SELECT/INSERT/UPDATE/DELETE/CREATE TABLE/DROP TABLE 等全部 SQL 操作。

    示例:
        talon_sql_query("SELECT * FROM users WHERE age > 25")
        talon_sql_query("INSERT INTO users VALUES (1, 'Alice', 30)")
        talon_sql_query("CREATE TABLE logs (id INT, msg TEXT, ts INT)")
    """
    return _run_cli(sql)


@mcp.tool()
def talon_table_schema(table_name: str) -> dict:
    """查看 SQL 表的结构（列名、类型、主键等）。"""
    return _run_cli(f"DESCRIBE {table_name}")


# ━━━━ KV 引擎 ━━━━


@mcp.tool()
def talon_kv_get(key: str) -> dict:
    """读取 KV 存储中的值。"""
    return _run_cli(f":kv get {key}")


@mcp.tool()
def talon_kv_set(key: str, value: str) -> dict:
    """写入 KV 存储。"""
    return _run_cli(f":kv set {key} {value}")


@mcp.tool()
def talon_kv_delete(key: str) -> dict:
    """删除 KV 键。"""
    return _run_cli(f":kv del {key}")


@mcp.tool()
def talon_kv_list(prefix: str = "") -> dict:
    """列出匹配前缀的所有 KV 键。不传 prefix 则列出全部。"""
    return _run_cli(f":kv keys {prefix}")


@mcp.tool()
def talon_kv_scan(prefix: str, limit: int = 20) -> dict:
    """扫描 KV 存储，返回键值对。"""
    return _run_cli(f":kv scan {prefix} {limit}")


@mcp.tool()
def talon_kv_count() -> dict:
    """返回 KV 存储中的总 key 数量。"""
    return _run_cli(":kv count")


@mcp.tool()
def talon_kv_exists(key: str) -> dict:
    """检查 KV 键是否存在。"""
    return _run_cli(f":kv exists {key}")


@mcp.tool()
def talon_kv_incr(key: str) -> dict:
    """原子自增 KV 计数器。"""
    return _run_cli(f":kv incr {key}")


# ━━━━ MQ 引擎 ━━━━


@mcp.tool()
def talon_mq_topics() -> dict:
    """列出所有消息队列 topic。"""
    return _run_cli(":mq topics")


@mcp.tool()
def talon_mq_length(topic: str) -> dict:
    """查看消息队列长度。"""
    return _run_cli(f":mq len {topic}")


@mcp.tool()
def talon_mq_publish(topic: str, message: str) -> dict:
    """向消息队列发布消息。"""
    return _run_cli(f":mq pub {topic} {message}")


# ━━━━ 全文搜索 ━━━━


@mcp.tool()
def talon_fts_search(index_name: str, query: str) -> dict:
    """全文搜索（BM25 算法）。

    示例: talon_fts_search("articles", "rust async programming")
    """
    return _run_cli(f":fts search {index_name} {query}")


# ━━━━ 图引擎 ━━━━


@mcp.tool()
def talon_graph_count(name: str) -> dict:
    """查看图的节点和边数量。"""
    return _run_cli(f":graph count {name}")


@mcp.tool()
def talon_graph_vertex(name: str, vertex_id: str) -> dict:
    """查看图中指定节点的详情。"""
    return _run_cli(f":graph vertex {name} {vertex_id}")


@mcp.tool()
def talon_graph_neighbors(name: str, vertex_id: str, direction: str = "out") -> dict:
    """查找图中节点的邻居。direction: out/in/both"""
    return _run_cli(f":graph neighbors {name} {vertex_id} {direction}")


@mcp.tool()
def talon_graph_bfs(name: str, start_id: str, max_depth: int = 3) -> dict:
    """广度优先遍历图。"""
    return _run_cli(f":graph bfs {name} {start_id} {max_depth}")


# ━━━━ 向量引擎 ━━━━


@mcp.tool()
def talon_vector_count(name: str) -> dict:
    """查看向量索引中的向量数量。"""
    return _run_cli(f":vec count {name}")


# ━━━━ 地理空间 ━━━━


@mcp.tool()
def talon_geo_members(name: str) -> dict:
    """列出地理空间索引中的所有成员。"""
    return _run_cli(f":geo members {name}")


@mcp.tool()
def talon_geo_count(name: str) -> dict:
    """查看地理空间索引中的成员数量。"""
    return _run_cli(f":geo count {name}")


@mcp.tool()
def talon_geo_search(name: str, longitude: float, latitude: float, radius_meters: float) -> dict:
    """在地理空间索引中搜索指定坐标半径内的成员。"""
    return _run_cli(f":geo search {name} {longitude} {latitude} {radius_meters}")


# ━━━━ 时序引擎 ━━━━


@mcp.tool()
def talon_ts_list() -> dict:
    """列出所有时序数据集。"""
    return _run_cli(":ts list")


@mcp.tool()
def talon_ts_info(name: str) -> dict:
    """查看时序数据集的详情（数据点数量、时间范围等）。"""
    return _run_cli(f":ts info {name}")


# ━━━━ AI 引擎（talon-ai）━━━━


@mcp.tool()
def talon_ai_sessions() -> dict:
    """列出所有活跃的 AI 会话 Session。

    Session 包含对话上下文、消息历史等。可通过 session_id 查看详情和历史。
    """
    return _run_cli(":ai sessions")


@mcp.tool()
def talon_ai_session_detail(session_id: str) -> dict:
    """查看指定 AI Session 的详情（创建时间、消息数等）。"""
    return _run_cli(f":ai session {session_id}")


@mcp.tool()
def talon_ai_history(session_id: str, limit: int = 20) -> dict:
    """查看 AI 会话的对话历史。"""
    return _run_cli(f":ai history {session_id} {limit}")


@mcp.tool()
def talon_ai_memory_count() -> dict:
    """查看 AI 记忆存储中的条目数量。"""
    return _run_cli(":ai memory count")


@mcp.tool()
def talon_ai_docs_list() -> dict:
    """列出所有 RAG 文档。"""
    return _run_cli(":ai docs list")


@mcp.tool()
def talon_ai_docs_count() -> dict:
    """查看 RAG 文档数量。"""
    return _run_cli(":ai docs count")


# ━━━━ EvoCore 进化引擎 ━━━━


@mcp.tool()
def talon_evo_soul() -> dict:
    """查看 EvoCore 的 Soul 配置。

    Soul 是 AI 的身份核心，包含名称、人格类型、使命、核心真理、沟通风格等。
    """
    return _run_cli(":evo soul")


@mcp.tool()
def talon_evo_personality() -> dict:
    """查看 EvoCore 的个性维度和偏向值。

    个性维度包括：cautious↔aggressive、creative↔precise、proactive↔reactive 等。
    """
    return _run_cli(":evo personality")


@mcp.tool()
def talon_evo_history() -> dict:
    """查看 Soul 的进化历史记录。"""
    return _run_cli(":evo history")


@mcp.tool()
def talon_evo_proposals() -> dict:
    """查看待处理的 Soul 进化提议。"""
    return _run_cli(":evo proposals")


@mcp.tool()
def talon_evo_stats() -> dict:
    """查看 EvoCore 统计信息（学习次数、Soul 版本、提议数）。"""
    return _run_cli(":evo stats")


# ━━━━ 系统 ━━━━


@mcp.tool()
def talon_stats() -> dict:
    """查看数据库总体统计信息（表数量、KV 数量、MQ topic 数、时序数等）。

    这是了解数据库全貌的最佳入口。
    """
    return _run_cli(":stats")


# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
#  Resources
# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━


@mcp.resource("talon://stats")
def get_stats() -> str:
    """Talon 数据库概览 — 表列表、KV 数量、引擎状态。"""
    result = _run_cli(":stats")
    return json.dumps(result, ensure_ascii=False, indent=2)


@mcp.resource("talon://connection")
def get_connection_info() -> str:
    """当前 Talon 连接信息。"""
    info = {"talon_cli": TALON_CLI}
    if CONNECT_ADDR:
        info["mode"] = "network"
        info["address"] = CONNECT_ADDR
    elif DB_PATH:
        info["mode"] = "embedded"
        info["db_path"] = DB_PATH
    return json.dumps(info, indent=2)


# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
#  入口
# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━


def _find_talon_cli() -> str:
    """查找 talon-cli 二进制路径。"""
    # 1. 环境变量
    env = os.environ.get("TALON_CLI_PATH")
    if env and os.path.isfile(env):
        return env

    # 2. 同目录的 ../target/release/talon-cli
    script_dir = os.path.dirname(os.path.abspath(__file__))
    release = os.path.join(script_dir, "..", "target", "release", "talon-cli")
    if os.path.isfile(release):
        return os.path.abspath(release)

    # 3. debug 版本
    debug = os.path.join(script_dir, "..", "target", "debug", "talon-cli")
    if os.path.isfile(debug):
        return os.path.abspath(debug)

    # 4. PATH 中
    import shutil
    which = shutil.which("talon-cli")
    if which:
        return which

    return ""


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Talon MCP Server — 通过 talon-cli 访问 Talon 数据库")
    parser.add_argument(
        "--connect",
        default=os.environ.get("TALON_CONNECT"),
        help="连接到 Talon Server（格式: host:port，默认 SuperClaw 的 127.0.0.1:7720）",
    )
    parser.add_argument(
        "--db-path",
        default=os.environ.get("TALON_DB_PATH"),
        help="本地数据库路径（嵌入模式，自动走 daemon 加速）",
    )
    parser.add_argument(
        "--token",
        default=os.environ.get("TALON_TOKEN"),
        help="认证 token（可选）",
    )
    parser.add_argument(
        "--cli-path",
        default=os.environ.get("TALON_CLI_PATH"),
        help="talon-cli 二进制路径（默认自动查找）",
    )
    args, unknown = parser.parse_known_args()

    # 查找 talon-cli
    TALON_CLI = args.cli_path or _find_talon_cli()
    if not TALON_CLI:
        print("❌ 找不到 talon-cli。请通过 --cli-path 指定或先编译:", file=sys.stderr)
        print("   cd /path/to/talon-cli && cargo build --release", file=sys.stderr)
        sys.exit(1)

    CONNECT_ADDR = args.connect or ""
    DB_PATH = args.db_path or ""
    CONNECT_TOKEN = args.token

    if not CONNECT_ADDR and not DB_PATH:
        print("❌ 请指定连接方式:", file=sys.stderr)
        print("   --connect 127.0.0.1:7720    (连接 SuperClaw)", file=sys.stderr)
        print("   --db-path ~/.superclaw/talon_data  (本地嵌入模式)", file=sys.stderr)
        sys.exit(1)

    mode = f"网络模式 → {CONNECT_ADDR}" if CONNECT_ADDR else f"嵌入模式 → {DB_PATH}"
    print(f"🚀 Talon MCP Server 启动", file=sys.stderr)
    print(f"   talon-cli: {TALON_CLI}", file=sys.stderr)
    print(f"   模式: {mode}", file=sys.stderr)

    # 快速验证 talon-cli 可执行
    try:
        r = subprocess.run([TALON_CLI, "--version"], capture_output=True, text=True, timeout=5)
        ver = r.stdout.strip()
        if ver:
            print(f"   版本: {ver}", file=sys.stderr)
    except Exception:
        pass

    mcp.run()
