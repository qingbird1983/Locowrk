//! 落盘标识符卫生（审计《CODE-AUDIT-v1》M-23 的收敛点，v2.148 批 D）。
//!
//! 会话 id 会被两处直接拼成文件名：session-store 的 `<id>.jsonl` 与
//! memory-tree 的 `<scope 目录>/<id>.json`。批 D 之前**各写了一份校验**且
//! 已经分叉——persist.rs 放行 `.`、memory-tree 不放行且限长 64，而后者的
//! 注释自称「与 session_log 落盘命名同源」。后果：含 `.` 的会话 id 日志
//! 落盘成功、树投影静默失效（当前桌面 id 形如 `desktop-<毫秒>` 恰好两边
//! 都合法 = 陷阱而非现网缺陷）。规则单点化 = 两侧不可能再分叉。
//!
//! 判据：非空、≤ [`SESSION_ID_MAX`] 字符、只允许 ASCII 字母数字与
//! `-` `_` `.`，且整串不得全由点组成（`.` / `..` 拼上扩展名后其实不构成
//! 穿越，仍防御性一并拒）。

/// 会话 id 长度上限。取严不取松：Windows 路径余量（数据根 +
/// `memory-tree/<scope 48>/<id>.jsonl` 合计远低于 260），且 64 是分叉
/// 规则里较严那一侧（memory-tree）的原值。
pub const SESSION_ID_MAX: usize = 64;

/// 会话 id 是否可安全用作文件名（唯一真源，两侧共用）。
pub fn valid_session_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= SESSION_ID_MAX
        && !id.chars().all(|c| c == '.')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

#[cfg(test)]
mod tests;
