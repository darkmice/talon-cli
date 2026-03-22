#!/usr/bin/env python3
"""
Talon MCP Server — 让 AI 原生访问 Talon 数据库的全部引擎。

通过 TCP 帧协议连接到运行中的 Talon Server（SuperClaw 内置 127.0.0.1:7720），
提供结构化的 MCP Tool 调用。AI 不需要记命令语法，IDE 自动发现所有工具。

使用方式（SuperClaw 桌面端已在运行）：

    python talon_mcp.py --host 127.0.0.1 --port 7720

IDE 配置（settings.json / mcp.json）：

    {
        "mcpServers": {
            "talon": {
                "command": "python3",
                "args": ["<path>/talon_mcp.py", "--host", "127.0.0.1", "--port", "7720"]
            }
        }
    }
"""

import argparse
import json
import os
import socket
import struct
import sys
from typing import Any

from mcp.server.fastmcp import FastMCP

# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
#  Talon TCP 帧协议客户端
# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

# 全局连接配置
TALON_HOST: str = "127.0.0.1"
TALON_PORT: int = 7720
TALON_TOKEN: str | None = None


def _send_frame(sock: socket.socket, data: bytes) -> None:
    """发送帧：[4-byte big-endian length][payload]"""
    sock.sendall(struct.pack(">I", len(data)))
    sock.sendall(data)


def _recv_frame(sock: socket.socket) -> str:
    """接收帧：读取 4 字节长度 + payload"""
    len_buf = b""
    while len(len_buf) < 4:
        chunk = sock.recv(4 - len(len_buf))
        if not chunk:
            raise ConnectionError("连接已断开")
        len_buf += chunk
    length = struct.unpack(">I", len_buf)[0]
    if length > 16 * 1024 * 1024:
        raise ValueError(f"帧过大: {length} bytes")
    data = b""
    while len(data) < length:
        chunk = sock.recv(min(65536, length - len(data)))
        if not chunk:
            raise ConnectionError("读取数据中断")
        data += chunk
    return data.decode("utf-8")


def _send_cmd(cmd_json: dict) -> dict:
    """连接 Talon Server 发送 JSON 命令并返回响应。

    每次调用建立新连接（短连接），简单可靠。
    Talon TCP 协议是无状态的，连接开销极小（本地 ~0.1ms）。
    """
    try:
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        sock.settimeout(30)
        sock.connect((TALON_HOST, TALON_PORT))

        # 认证（如果有 token）
        if TALON_TOKEN:
            auth = json.dumps({"auth": TALON_TOKEN}).encode()
            _send_frame(sock, auth)
            resp = _recv_frame(sock)
            if "auth failed" in resp.lower():
                sock.close()
                return {"ok": False, "error": "认证失败"}

        # 发送命令
        payload = json.dumps(cmd_json).encode()
        _send_frame(sock, payload)

        # 接收响应
        resp_str = _recv_frame(sock)
        sock.close()

        # 解析 JSON
        try:
            return json.loads(resp_str)
        except json.JSONDecodeError:
            return {"ok": True, "raw": resp_str}

    except ConnectionRefusedError:
        return {"ok": False, "error": f"无法连接 Talon Server {TALON_HOST}:{TALON_PORT}。请确保 SuperClaw 正在运行。"}
    except Exception as e:
        return {"ok": False, "error": str(e)}


def _sql_cmd(sql: str) -> dict:
    """执行 SQL 命令。"""
    return _send_cmd({"module": "sql", "action": "query", "params": {"sql": sql}})


def _kv_cmd(action: str, **params) -> dict:
    """执行 KV 命令。"""
    return _send_cmd({"module": "kv", "action": action, "params": params})


def _mq_cmd(action: str, **params) -> dict:
    """执行 MQ 命令。"""
    return _send_cmd({"module": "mq", "action": action, "params": params})


def _fts_cmd(action: str, **params) -> dict:
    """执行 FTS 命令。"""
    return _send_cmd({"module": "fts", "action": action, "params": params})


def _graph_cmd(action: str, **params) -> dict:
    """执行图引擎命令。"""
    return _send_cmd({"module": "graph", "action": action, "params": params})


def _vec_cmd(action: str, **params) -> dict:
    """执行向量引擎命令。"""
    return _send_cmd({"module": "vector", "action": action, "params": params})


def _geo_cmd(action: str, **params) -> dict:
    """执行地理空间命令。"""
    return _send_cmd({"module": "geo", "action": action, "params": params})


def _ts_cmd(action: str, **params) -> dict:
    """执行时序命令。"""
    return _send_cmd({"module": "ts", "action": action, "params": params})


# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
#  MCP Server 定义
# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

mcp = FastMCP(
    "Talon Database",
    instructions=(
        "Talon 多模融合数据引擎 — 通过 TCP 连接到运行中的 Talon Server。"
        "支持 SQL、KV、MQ、向量、时序、全文搜索、图、地理空间等引擎。"
    ),
)


# ━━━━ SQL 引擎 ━━━━


@mcp.tool()
def talon_sql_query(sql: str) -> dict:
    """执行 SQL 查询或语句。

    支持 SELECT/INSERT/UPDATE/DELETE/CREATE TABLE/DROP TABLE 等全部 SQL 操作。
    连接到运行中的 Talon Server（SuperClaw 内置数据库服务）执行。

    示例:
        talon_sql_query("SELECT * FROM users WHERE age > 25")
        talon_sql_query("INSERT INTO users VALUES (1, 'Alice', 30)")
        talon_sql_query("CREATE TABLE logs (id INT, msg TEXT, ts INT)")
        talon_sql_query("SELECT count(*) as cnt FROM memories")
    """
    return _sql_cmd(sql)


# ━━━━ KV 引擎 ━━━━


@mcp.tool()
def talon_kv_get(key: str) -> dict:
    """读取 KV 存储中的值。

    KV 存储是 Talon 的键值引擎，支持自动序列化、TTL、前缀扫描等。
    """
    return _kv_cmd("get", key=key)


@mcp.tool()
def talon_kv_set(key: str, value: str) -> dict:
    """写入 KV 存储。"""
    return _kv_cmd("set", key=key, value=value)


@mcp.tool()
def talon_kv_delete(key: str) -> dict:
    """删除 KV 键。"""
    return _kv_cmd("del", key=key)


@mcp.tool()
def talon_kv_list(prefix: str = "") -> dict:
    """列出匹配前缀的所有 KV 键。不传 prefix 则列出全部。"""
    return _kv_cmd("keys", prefix=prefix)


@mcp.tool()
def talon_kv_scan(prefix: str, limit: int = 20) -> dict:
    """扫描 KV 存储，返回键值对。"""
    return _kv_cmd("scan", prefix=prefix, limit=limit)


@mcp.tool()
def talon_kv_count() -> dict:
    """返回 KV 存储中的总 key 数量。"""
    return _kv_cmd("count")


@mcp.tool()
def talon_kv_exists(key: str) -> dict:
    """检查 KV 键是否存在。"""
    return _kv_cmd("exists", key=key)


@mcp.tool()
def talon_kv_incr(key: str) -> dict:
    """原子自增 KV 计数器。"""
    return _kv_cmd("incr", key=key)


# ━━━━ MQ 引擎 ━━━━


@mcp.tool()
def talon_mq_topics() -> dict:
    """列出所有消息队列 topic。"""
    return _mq_cmd("topics")


@mcp.tool()
def talon_mq_length(topic: str) -> dict:
    """查看消息队列长度。"""
    return _mq_cmd("len", topic=topic)


@mcp.tool()
def talon_mq_publish(topic: str, message: str) -> dict:
    """向消息队列发布消息。"""
    return _mq_cmd("pub", topic=topic, message=message)


# ━━━━ 全文搜索 ━━━━


@mcp.tool()
def talon_fts_search(index_name: str, query: str) -> dict:
    """全文搜索（BM25 算法）。

    在指定的全文索引中搜索关键词。

    示例: talon_fts_search("articles", "rust async programming")
    """
    return _fts_cmd("search", name=index_name, query=query)


# ━━━━ 图引擎 ━━━━


@mcp.tool()
def talon_graph_count(name: str) -> dict:
    """查看图的节点和边数量。"""
    return _graph_cmd("count", name=name)


@mcp.tool()
def talon_graph_vertex(name: str, vertex_id: str) -> dict:
    """查看图中指定节点的详情（属性、标签等）。"""
    return _graph_cmd("vertex", name=name, id=vertex_id)


@mcp.tool()
def talon_graph_neighbors(name: str, vertex_id: str, direction: str = "out") -> dict:
    """查找图中节点的邻居。

    Args:
        direction: 方向，可选 "out"（出边）、"in"（入边）、"both"（双向）
    """
    return _graph_cmd("neighbors", name=name, id=vertex_id, direction=direction)


@mcp.tool()
def talon_graph_bfs(name: str, start_id: str, max_depth: int = 3) -> dict:
    """广度优先遍历图。从起始节点出发，按层遍历邻居。"""
    return _graph_cmd("bfs", name=name, start=start_id, depth=max_depth)


# ━━━━ 向量引擎 ━━━━


@mcp.tool()
def talon_vector_count(name: str) -> dict:
    """查看向量索引中的向量数量。"""
    return _vec_cmd("count", name=name)


# ━━━━ 地理空间 ━━━━


@mcp.tool()
def talon_geo_members(name: str) -> dict:
    """列出地理空间索引中的所有成员。"""
    return _geo_cmd("members", name=name)


@mcp.tool()
def talon_geo_count(name: str) -> dict:
    """查看地理空间索引中的成员数量。"""
    return _geo_cmd("count", name=name)


@mcp.tool()
def talon_geo_search(name: str, longitude: float, latitude: float, radius_meters: float) -> dict:
    """在地理空间索引中搜索指定坐标半径内的成员。

    Args:
        longitude: 经度（-180 ~ 180）
        latitude: 纬度（-90 ~ 90）
        radius_meters: 搜索半径（米）
    """
    return _geo_cmd("search", name=name, lng=longitude, lat=latitude, radius=radius_meters)


# ━━━━ 时序引擎 ━━━━


@mcp.tool()
def talon_ts_list() -> dict:
    """列出所有时序数据集名称。"""
    return _ts_cmd("list")


@mcp.tool()
def talon_ts_info(name: str) -> dict:
    """查看时序数据集的详情（数据点数量、时间范围等）。"""
    return _ts_cmd("info", name=name)


# ━━━━ 系统统计 ━━━━


@mcp.tool()
def talon_stats() -> dict:
    """查看数据库总体统计信息。

    返回 SQL 表数量与列表、KV key 数量、MQ topic 列表、时序集列表等。
    这是了解数据库全貌的最佳入口。
    """
    return _sql_cmd("SHOW TABLES")


@mcp.tool()
def talon_table_schema(table_name: str) -> dict:
    """查看 SQL 表的结构（列名、类型等）。

    示例: talon_table_schema("users")
    """
    return _sql_cmd(f"DESCRIBE {table_name}")


# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
#  Resources（让 AI 快速了解数据库概览）
# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━


@mcp.resource("talon://tables")
def get_tables() -> str:
    """Talon 数据库中所有 SQL 表的列表。"""
    result = _sql_cmd("SHOW TABLES")
    return json.dumps(result, ensure_ascii=False, indent=2)


@mcp.resource("talon://connection")
def get_connection_info() -> str:
    """当前 Talon 连接信息。"""
    return json.dumps({
        "host": TALON_HOST,
        "port": TALON_PORT,
        "has_token": TALON_TOKEN is not None,
    }, indent=2)


# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
#  入口
# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Talon MCP Server — 连接 Talon 数据库")
    parser.add_argument(
        "--host",
        default=os.environ.get("TALON_HOST", "127.0.0.1"),
        help="Talon Server 地址（默认 127.0.0.1）",
    )
    parser.add_argument(
        "--port",
        type=int,
        default=int(os.environ.get("TALON_PORT", "7720")),
        help="Talon Server 端口（默认 7720 — SuperClaw 内置）",
    )
    parser.add_argument(
        "--token",
        default=os.environ.get("TALON_TOKEN"),
        help="认证 token（可选）",
    )
    args, unknown = parser.parse_known_args()

    TALON_HOST = args.host
    TALON_PORT = args.port
    TALON_TOKEN = args.token

    # 测试连接
    try:
        test_sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        test_sock.settimeout(2)
        test_sock.connect((TALON_HOST, TALON_PORT))
        test_sock.close()
        print(f"✅ 已连接 Talon Server {TALON_HOST}:{TALON_PORT}", file=sys.stderr)
    except Exception:
        print(
            f"⚠️  无法连接 {TALON_HOST}:{TALON_PORT}。请确保 SuperClaw 正在运行。",
            file=sys.stderr,
        )
        print("   MCP Server 仍会启动，命令会返回连接错误。", file=sys.stderr)

    mcp.run()
