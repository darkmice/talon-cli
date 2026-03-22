//! Value 格式化工具。

/// 将 Talon Value 转换为人类可读字符串。
///
/// Value 已实现 Display，此函数作为一层语义封装，
/// 后续可添加 CLI 专用的格式（如颜色、截断等）。
pub fn format_value(v: &talon::Value) -> String {
    format!("{}", v)
}
