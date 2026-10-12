//! 「硬门·越界」：Windows 绝对路径启发式扫描器（㉘·B 外部路径门）。
//!
//! 从 `approval.rs` 拆出（批 F-24c，v2.180）的理由 = 三问自检第①问答不出来：本文件做的
//! 是「从命令字符串里抠出盘符路径、和声明根做分量边界比较」，与 `ApprovalHub` 的七序决策表
//! 是两件事；原文件自己也用一条 `// ---- 硬门·越界 ----` 分隔线承认了这个边界。
//! 批 F-24a 是第③问的实证：为 M-18a 改 `resolve`（决策表）必须同时改这里两个函数的签名。
//!
//! 语义纪律（**本批一条没动**）：只做**字符串归一比较**、不碰文件系统（所以围栏根可以是不存在
//! 的路径）；根比较走分量边界而不是 `starts_with`（防 `F:\ws\ab` 命中根 `F:\ws\a`）；
//! UNC 与 verbatim 前缀的识别单独成函数，逐条有测试钉住。

use std::path::{Path, PathBuf};

use plugin_api::ffi::CallContext;

// ---- 硬门·越界（㉘·B 外部路径门，启发式命令扫描）---------------------------

/// 命令类工具（`shell`）的命令文本是否触及全局声明根之外
/// 的绝对路径（㉘·C 起围栏 = 全局声明根，与插件侧 fsutil 同源——声明根内
/// 跨项目畅通不弹卡）。结构化路径工具（fs/edit/git）不走这里——它们在
/// 插件内对出围栏路径 fail-closed 拒绝（既有契约，拒绝文案引导改走 shell
/// 审批路）。启发式：只识别 Windows 绝对路径形态（盘符 `X:\`·`X:/` 与
/// UNC `\\…`）；扫不出绝对路径 = 视为围栏内——漏报在 `询问` 档仍有弹卡
/// 兜底（shell 本就是 Irreversible），宁漏勿错杀。
/// （v2.145：`sandbox_run` 随插件退役移出扫描面。）
pub(crate) fn command_touches_outside(
    tool: &str,
    args: &serde_json::Value,
    ctx: Option<&CallContext>,
) -> bool {
    if tool != "shell" {
        return false;
    }
    let cmd = args
        .get("command")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let tokens = extract_windows_abs_paths(cmd);
    if tokens.is_empty() {
        return false;
    }
    let roots = scan_roots(ctx);
    // 围栏空（连数据根都判不出）= 无法判定，宁可问一次
    if roots.is_empty() {
        return true;
    }
    tokens.iter().any(|t| is_outside(t, &roots))
}

/// 命令文本里的 Windows 绝对路径 token：`X:\…` / `X:/…` / UNC `\\…`。
/// 分隔符取空格 / 引号 / `;` / `<` `>` `|` `&` / 换行。
pub(crate) fn extract_windows_abs_paths(cmd: &str) -> Vec<String> {
    let is_delim = |c: char| {
        matches!(
            c,
            ' ' | '\t' | '"' | '\'' | ';' | '<' | '>' | '|' | '&' | '\n' | '\r'
        )
    };
    let bytes = cmd.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < cmd.len() {
        let is_abs = rest_starts_unc(cmd, i)
            || (i + 2 < cmd.len()
                && bytes[i].is_ascii_alphabetic()
                && bytes[i + 1] == b':'
                && matches!(bytes[i + 2], b'\\' | b'/'));
        if is_abs {
            let end = cmd[i..].find(is_delim).map(|k| i + k).unwrap_or(cmd.len());
            let token = cmd[i..end].trim_matches(|c| c == '"' || c == '\'');
            if !token.is_empty() {
                out.push(token.to_string());
            }
            i = end;
        } else {
            // 按整字符步进，保 i 恒为 char boundary——字节步进会在多字节
            // 字符（中文命令）的续字节上 cmd[i..] 切片 panic
            i += cmd[i..].chars().next().map_or(1, char::len_utf8);
        }
    }
    out
}

fn rest_starts_unc(cmd: &str, i: usize) -> bool {
    cmd.as_bytes()[i..].starts_with(b"\\\\")
}

/// 扫描围栏（与插件侧 fsutil 语义逐值对齐，㉘·C）：会话上下文带 `fs_roots`
/// （= 全局声明根 `[数据根] ∪ settings.fsRoots`，`workspace.rs` resolver
/// 下发）就用它；空（未归属/headless）回落 env 全局（数据根 +
/// `LOCOWRK_FS_ROOTS`，即 `fsutil::allowed_roots` 同源）——同一集合。
/// M-18a：收**已解析的** `CallContext`（调用方一次决策只解析一次），不再
/// 自己拿 resolver + session_id 去解析。
pub(crate) fn scan_roots(ctx: Option<&CallContext>) -> Vec<PathBuf> {
    let data_root = plugin_api::fsutil::data_root()
        .ok()
        .map(|p| Path::new(&p).to_path_buf());
    if let Some(fs_roots) = ctx.map(|c| &c.fs_roots) {
        if !fs_roots.is_empty() {
            return data_root
                .into_iter()
                .chain(fs_roots.iter().map(PathBuf::from))
                .collect();
        }
    }
    let mut roots: Vec<PathBuf> = data_root.into_iter().collect();
    if let Ok(extra) = std::env::var("LOCOWRK_FS_ROOTS") {
        for seg in extra.split(';') {
            let t = seg.trim();
            if !t.is_empty() {
                roots.push(PathBuf::from(t));
            }
        }
    }
    roots
}

/// token 是否落在所有根之外。归一：`/`→`\`、去 verbatim 前缀、小写
/// （Windows 大小写不敏感）、去尾分隔；前缀比较按分量边界——`f:\ab` 不算
/// `f:\a` 内。
pub(crate) fn is_outside(token: &str, roots: &[PathBuf]) -> bool {
    let norm = |s: &str| -> String {
        let s = s.replace('/', "\\");
        let s = s.strip_prefix("\\\\?\\").unwrap_or(&s);
        s.trim_end_matches('\\').to_ascii_lowercase()
    };
    let t = norm(token);
    roots.iter().all(|r| {
        let r = norm(&r.to_string_lossy());
        // M-18e②：`strip_prefix` 代替 `format!("{r}\\")` + `starts_with`
        // ——少一次堆分配（每个根每次判定都新建一个 String），语义逐值等价：
        // 「t 以 r 开头且紧接一个分隔符」= 「剥掉 r 前缀后剩下的以分隔符开头」；
        // 剥不动（不以 r 开头）= 本就不匹配 ⇒ `is_none_or` 取真。分量边界
        // （`f:\ab` 不算 `f:\a` 内）由「剥完还得是分隔符」这一位
        // 保住，与改前同。
        t != r
            && t.strip_prefix(&r)
                .is_none_or(|rest| !rest.starts_with('\\'))
    })
}
