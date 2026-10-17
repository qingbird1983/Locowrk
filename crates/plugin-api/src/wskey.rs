//! workspace **键位归一**（MR 稿 §6，队列 Q2）：记忆主键 `(workspace, key)` 里的
//! `workspace` 那半，两侧（宿主 `workspace::scope_key` 与插件 `WsCtx`）共用这一份
//! 实现，同一个项目不管从哪条路进来都必须落成**同一个串**。
//!
//! **治的是什么**：现役键位 = 会话旁车里的原样字符串。Windows 路径**大小写不敏感**
//! 且分隔符可混用，于是 `F:\Warehouse\TITY`、`f:/warehouse/tity/`、
//! `F:\Warehouse\TITY\.` 是同一个项目的四种写法，而它们会是 sqlite 里的**四个不同
//! 主键**——同一项目的事实被劈成几份，谁也查不全谁的（MR 稿 §0 的实测断点之一）。
//! 人往设置里粘路径、模型按记忆里的旧写法填参数，都是真实来源。
//!
//! **为什么现在做而不是以后**：`memories` 表当前 **0 行**（2026-10-12 本机实测），
//! 归一化只改"以后写进去的串"，零迁移成本。等 M2 的事实卡开始落库，双键位就会
//! 变成**脏数据**，那时再归一要迁用户库（队列 §3 判据 2「0 行红利窗口」）。
//!
//! 归一三步（顺序有讲究）：
//! 1. `canonicalize`——解析符号链接/junction 与 `..`、`.`，并顺手统一分隔符；
//!    **失败（路径不存在）时回落 [`lexical_fold`]**：`.` 与 `..` 由词法折叠处理掉，
//!    其余一律原样。这一步两侧都需要——Windows 的路径 API 对不存在的中间段也会折叠，
//!    POSIX 的 realpath 遇缺失段直接报错，不折叠就会让同一个目录落成两个键位。
//! 2. 剥 Windows verbatim 前缀 `\\?\` / `\\?\UNC\`（canonicalize 在 Windows 必然
//!    加上它；留着会进管理页列表、md 目录名与模型可见的 `<environment>` 行，
//!    既难看又和旁车串不同形）。
//! 3. 按**本平台**收尾：Windows 统一反斜杠 + 大小写折叠 + 剥尾分隔符；POSIX
//!    只剥尾斜杠，**不动大小写**。空 / 纯空白 = 未归属键位 `""`（与 `scope_key`
//!    的既有契约同形，`workspace` 列 NOT NULL DEFAULT ''）。
//!
//! **为什么第 3 步必须分平台**：POSIX 上 `/Tmp/X` 与 `/tmp/x` 是两个真不同的目录，
//! 折叠它们 = 把两个项目的记忆合进同一个主键 = 数据损坏；同理反斜杠在 POSIX 是
//! 合法文件名字符，不能当分隔符剥。
//!
//! **不做的事（边界如实）**：
//! - 不做跨平台统一（同一台机器上同形即达成本刀目的；跨机器同步记忆库不在设计里）。
//! - 不动**记忆树**的 scope 编码（`memory-tree/src/storage.rs::scope_dir` 也吃
//!   workspace 串，归一化会让已有两份投影目录换名——树是投影可重建，但那是
//!   MR 稿 §8 的 **M3**，涉用户数据目录、要单独确认，不在本刀顺手做）。
//! - 不动**文件围栏**的根（`fs_roots` 下发仍是宿主 canonicalize 后的原样串；
//!   围栏比较吃 `starts_with`，把下发串折叠成小写会让插件侧的越界判定整体失配）。

use std::path::Path;

/// 键位归一主入口（未归属 / 空 / 纯空白 ⇒ `""`）。
pub fn normalize(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    // canonicalize 失败 = 项目目录不存在（被删 / 还没建），回落词法归一而不是报错：
    // 键位是标识符不是路径访问，"打不开"不该让记忆读写因此拒落，更不该把它
    // 归成 `""`（那等于把某个项目的记忆污染进全局偏好层）。
    let lexical = match Path::new(trimmed).canonicalize() {
        Ok(p) => strip_verbatim(&p.to_string_lossy()),
        // 路径不存在才走词法兜底。Windows 的路径 API 对**不存在的中间段**也会折叠
        // `..`，POSIX 的 realpath 遇缺失段直接报错 ⇒ 兜底必须自己做一遍折叠，否则
        // `/proj/x/../b` 与 `/proj/b` 会落成两个键位（Linux 实测见 specs/03 的 T18）。
        // 折叠是纯字符串操作，不碰大小写、不假装解析符号链接，所以两侧同源。
        Err(_) => lexical_fold(trimmed, cfg!(windows)),
    };
    if cfg!(windows) {
        win_form(&lexical)
    } else {
        posix_form(&lexical)
    }
}

/// Windows 收尾：分隔符统一成反斜杠 + 大小写折叠 + 剥尾部冗余分隔符。
///
/// 折叠用 `to_lowercase`（Unicode 简单映射，与文件系统的大小写无关性同档；不用
/// `eq_ignore_ascii_case` 是因为键位要落进主键，得是**一个确定的串**而不是比较规则）。
fn win_form(s: &str) -> String {
    let unified = s.replace('/', "\\").to_lowercase();
    let bare = unified.trim_end_matches('\\');
    if bare.is_empty() {
        // 整串都是分隔符（`\\` = UNC 主机名前缀）：剥光就不是路径了，原样留着
        return unified;
    }
    if is_drive_root(bare) {
        // `c:` 后面那个分隔符是路径的一部分：剥掉就变成"该驱动器上的相对路径"
        return format!(r"{bare}\");
    }
    bare.to_string()
}

/// POSIX 收尾：只剥尾斜杠，大小写与分隔符一律不动。
fn posix_form(s: &str) -> String {
    let bare = s.trim_end_matches('/');
    if bare.is_empty() {
        return "/".to_string(); // 根不许剥成空串（空串 = 未归属键位）
    }
    bare.to_string()
}

/// 纯词法折叠（只在 `canonicalize` 失败时兜底）：丢 `.` 段、让 `..` 抵消前一段。
/// 不碰大小写、不解析符号链接——它只回答"这几个字面段指向哪儿"。
///
/// `win` 由调用方**显式**给（`normalize` 传 `cfg!(windows)`），这样两种分隔符形态
/// 在同一台机器上都能被测；它是"这套规则本来就有两份"，不是平台分支。
///
/// 为什么 `pub(crate)`：文件围栏（[`crate::fsutil::resolve_for_write`]）也要同一套
/// 折叠——两处各写一份就必然漂，而它们判的是同一件事"这几个字面段指向哪儿"。
///
/// 三条刻意收窄的边界，都是为了不顺手改变没被测过的行为：
/// - 串里没有 `.` / `..` 段 ⇒ 原样返回；
/// - 出现连续分隔符（POSIX 的 `//x`、Windows 的 UNC `\\srv\share`）⇒ 原样返回，
///   折叠规则不碰这类怪形；
/// - 绝对路径弹到顶就 clamp 在根（`/a/..` → `/`），相对路径弹到顶时 `..` 留在串里
///   （`../a` 原样）——弹掉它会指到别处；`a/..` 全弹光时给 `.`，**绝不给空串**
///   （空串 = 未归属键位，会把某个项目的记忆污染成全局偏好层）。
pub(crate) fn lexical_fold(s: &str, win: bool) -> String {
    let is_sep = |c: char| c == '/' || (win && c == '\\');
    let parts: Vec<&str> = s.split(is_sep).collect();
    if !parts.iter().any(|p| matches!(*p, "." | "..")) {
        return s.to_string();
    }
    let leading = parts.first().is_some_and(|p| p.is_empty());
    let body = if leading { &parts[1..] } else { &parts[..] };
    if body.iter().any(|p| p.is_empty()) {
        return s.to_string();
    }
    let sep: &str = if win { "\\" } else { "/" };
    let mut out: Vec<&str> = Vec::new();
    for p in body {
        match *p {
            "." => {}
            ".." => {
                if let Some(pos) = out.iter().rposition(|q| !q.is_empty()) {
                    out.remove(pos);
                } else if !leading {
                    out.push("..");
                }
            }
            other => out.push(other),
        }
    }
    let joined = out.join(sep);
    if leading {
        format!("{sep}{joined}")
    } else if joined.is_empty() {
        ".".to_string()
    } else {
        joined
    }
}

/// 剥 Windows verbatim 前缀：`\\?\C:\x` → `C:\x`；`\\?\UNC\srv\share` → `\\srv\share`。
/// 其余形态（含本来就不带前缀的旁车原样串）原样返回。
fn strip_verbatim(s: &str) -> String {
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        return rest.to_string();
    }
    s.to_string()
}

/// `X:` 形态（单字母盘符 + 冒号，长度恰为 2）——只在 Windows 收尾里判。
fn is_drive_root(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 2 && b[0].is_ascii_alphabetic() && b[1] == b':'
}

#[cfg(test)]
mod tests;
