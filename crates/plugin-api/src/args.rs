//! 参数读取基座（审计 F7 点名的一族：`arg_u64` 六个插件各写一遍）。
//! Rust 便利层，**不进 C ABI 面**——只给 Rust 插件作者复用，符号约定与
//! describe 契约不变（与 `fsutil`/`memlayout` 同一先例）。
//!
//! 为什么这份要在基座里：六份实现**三种写法**（表达式形 / `let … ?` 形 /
//! 本基座采纳的表达式形）而语义逐字相同，错误报文 `field \`{key}\` must be a
//! non-negative number` 六处一模一样 ⇒ 报文一改必漏改（同源实现分家后漂移；
//! 本仓 v2.133 ENV_LOCK 分裂、v2.135 gc 序错都是这个形状）。教学报错面
//! （批 F-5/F-6 立的 F12/F12b）以后要动这条报文，只有一个出处。
use serde_json::Value;

/// 读一个可选的无符号整数参数：
/// - 键缺失 **或** 显式 `null` ⇒ 给 `default`（模型常把"不填"写成 `null`，
///   这一支不是装饰：`json!({"limit": null})` 走的就是它）
/// - 非数字（负数、小数、字符串、布尔）⇒ 报错并点名是哪个字段
/// - 超过 `max` ⇒ **截到 `max` 而不是报错**（上限是"少要点"的语义，与各工具
///   的 `max_bytes`/`limit` 现状一致；改成都报错属于行为变更，不在本基座射程）
pub fn arg_u64(args: &Value, key: &str, default: u64, max: u64) -> Result<u64, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(v) => v
            .as_u64()
            .map(|n| n.min(max))
            .ok_or_else(|| format!("field `{key}` must be a non-negative number")),
    }
}

#[cfg(test)]
mod tests;
