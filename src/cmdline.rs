//! 命令行分词与参数工具。
//!
//! 统一替换散落在 main.rs / net.rs 里写死段数的 `splitn(N, ' ')`。
//! 旧方案用 `splitn(4)` / `splitn(5)` 限制了参数个数，多参数命令
//! （如 `:geo add <name> <key> <lng> <lat>` 需要 6 段、`:graph add-vertex`
//! 带 JSON properties）只能靠 `parts[3].splitn(2)` 二次硬拆。
//!
//! 本模块提供：
//! - [`tokenize`]：按空白分词，支持双引号 / 单引号包裹整段为单 token，
//!   让 value、JSON 字面量能安全携带空格。
//! - 一组从 token 列表读取参数的小工具（[`arg`] / [`arg_parse`] / [`require`]）。
//! - [`parse_props_json`]：把 JSON object 字面量解析为 `BTreeMap<String,String>`
//!   （图顶点/边属性、FTS 文档字段用）。

use std::collections::BTreeMap;

/// 将一行命令分词为 token 列表。
///
/// 规则：
/// - 以空白分隔。
/// - 双引号 `"..."` 或单引号 `'...'` 包裹的内容作为单个 token，引号本身被剥离，
///   token 内部可含空格。
/// - 引号内支持 `\"` / `\\` 转义。
/// - 未闭合的引号：把行尾前的剩余内容全部并入当前 token（尽力而为，不报错）。
///
/// 示例：
/// ```text
/// :graph add-vertex social person {"name":"li ming"}
///   → [":graph", "add-vertex", "social", "person", "{\"name\":\"li ming\"}"]
/// :kv set greeting "hello world"
///   → [":kv", "set", "greeting", "hello world"]
/// ```
///
/// 注意：裸 `{...}` JSON（不带引号）也能作为单 token，因为大括号内不含顶层空格时
/// 自然成段；但若 JSON 内有空格（如 `{"a": 1}`），调用方应用引号包裹，或使用
/// 紧凑 JSON（`{"a":1}`）。
pub fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut in_token = false;
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            // 引号仅在「token 起始位置」才作为字符串定界符——这样裸 JSON
            // 字面量（如 {"name":"li"}）里出现在 token 中间的引号被当作普通字符，
            // 不会被吞掉。只有 `:kv set k "hello world"` 这种引号开头的整段才被
            // 视为带空格的单 token。
            '"' | '\'' if !in_token => {
                let quote = c;
                in_token = true;
                // 读到匹配的闭合引号
                while let Some(&nc) = chars.peek() {
                    chars.next();
                    if nc == '\\' {
                        // 转义：取下一个字符原样放入
                        if let Some(&esc) = chars.peek() {
                            chars.next();
                            cur.push(esc);
                        }
                    } else if nc == quote {
                        break;
                    } else {
                        cur.push(nc);
                    }
                }
            }
            c if c.is_whitespace() => {
                if in_token {
                    tokens.push(std::mem::take(&mut cur));
                    in_token = false;
                }
            }
            c => {
                in_token = true;
                cur.push(c);
            }
        }
    }
    if in_token {
        tokens.push(cur);
    }
    tokens
}

/// 读取第 `i` 个 token（从 0 计），越界返回 `None`。
#[inline]
pub fn arg(args: &[String], i: usize) -> Option<&str> {
    args.get(i).map(|s| s.as_str())
}

/// 读取并解析第 `i` 个 token 为类型 `T`，越界或解析失败返回 `None`。
#[inline]
pub fn arg_parse<T: std::str::FromStr>(args: &[String], i: usize) -> Option<T> {
    args.get(i).and_then(|s| s.parse::<T>().ok())
}

/// 要求至少有 `min` 个 token，否则返回 `Err(usage)`。
#[inline]
pub fn require(args: &[String], min: usize, usage: &str) -> Result<(), String> {
    if args.len() < min {
        Err(usage.to_string())
    } else {
        Ok(())
    }
}

/// 把 JSON object 字面量解析为 `BTreeMap<String,String>`。
///
/// 所有 value 统一转成字符串（数字 / 布尔 / 字符串都接受）。后端图属性、
/// FTS 文档字段的底层类型即 `BTreeMap<String,String>`。
///
/// 空串或 `{}` 返回空 map；解析失败返回 `Err`。
pub fn parse_props_json(s: &str) -> Result<BTreeMap<String, String>, String> {
    let trimmed = s.trim();
    if trimmed.is_empty() || trimmed == "{}" {
        return Ok(BTreeMap::new());
    }
    let v: serde_json::Value =
        serde_json::from_str(trimmed).map_err(|e| format!("属性 JSON 解析失败: {}", e))?;
    let obj = v
        .as_object()
        .ok_or_else(|| "属性必须是 JSON object，如 {\"k\":\"v\"}".to_string())?;
    let mut map = BTreeMap::new();
    for (k, val) in obj {
        let s = match val {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Null => continue,
            other => other.to_string(),
        };
        map.insert(k.clone(), s);
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_split() {
        assert_eq!(tokenize(":kv get k"), vec![":kv", "get", "k"]);
    }

    #[test]
    fn double_quoted_value_with_space() {
        assert_eq!(
            tokenize(":kv set greeting \"hello world\""),
            vec![":kv", "set", "greeting", "hello world"]
        );
    }

    #[test]
    fn single_quoted_json() {
        assert_eq!(
            tokenize(":graph add-vertex g person '{\"name\":\"li ming\"}'"),
            vec![":graph", "add-vertex", "g", "person", "{\"name\":\"li ming\"}"]
        );
    }

    #[test]
    fn compact_json_no_quotes() {
        assert_eq!(
            tokenize(":graph add-vertex g person {\"name\":\"li\"}"),
            vec![":graph", "add-vertex", "g", "person", "{\"name\":\"li\"}"]
        );
    }

    #[test]
    fn many_args_not_truncated() {
        assert_eq!(
            tokenize(":geo add cities beijing 116.4 39.9"),
            vec![":geo", "add", "cities", "beijing", "116.4", "39.9"]
        );
    }

    #[test]
    fn escaped_quote_inside() {
        assert_eq!(
            tokenize(r#":kv set k "a\"b""#),
            vec![":kv", "set", "k", "a\"b"]
        );
    }

    #[test]
    fn empty_input() {
        assert!(tokenize("").is_empty());
        assert!(tokenize("   ").is_empty());
    }

    #[test]
    fn props_json_mixed_types() {
        let m = parse_props_json(r#"{"name":"li","age":30,"vip":true}"#).unwrap();
        assert_eq!(m.get("name"), Some(&"li".to_string()));
        assert_eq!(m.get("age"), Some(&"30".to_string()));
        assert_eq!(m.get("vip"), Some(&"true".to_string()));
    }

    #[test]
    fn props_json_empty() {
        assert!(parse_props_json("").unwrap().is_empty());
        assert!(parse_props_json("{}").unwrap().is_empty());
    }
}
