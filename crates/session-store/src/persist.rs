//! 会话日志落盘（DESIGN.md §4 落盘布局，⑲）：JSONL 一行一个
//! `SessionEvent`，一个文件 = 一个会话。磁盘文件是内存日志的**镜像**——
//! 追加走 [`append_event`]，undo fork 截断走 [`rewrite_log`]（改写前滚动
//! 备份，"永不删除"在磁盘面以备份留痕），恢复 = [`load_session`] 逐行
//! 回放。挂到数据根 `sessions/` 目录由组装点决定（本 crate 只管文件面）。
//!
//! 时点语义：`Session::append` 在游标于末尾时随追加前移、已回退时不动
//! （§4.2）——落盘侧镜像"当前活跃日志"，因此**调用方须在 undo fork 之后
//! 用 rewrite 同步磁盘**，否则恢复会复活已撤销的事件。

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::StoreError;
use crate::event::SessionEvent;
use crate::session::Session;
// 会话 id 白名单单点实现（审计 M-23）：本 crate 与 memory-tree 曾各写一份
// 且已分叉（这边放行 `.`、那边不放行且限长 64，而那边注释自称同源），
// 后果是含 `.` 的会话 id 日志落盘成功、树投影静默失效。规则收敛进
// plugin-api 后两侧不可能再漂；相对本 crate 旧判据的**行为差异** = 新增
// ≤64 长度上限与「整串全点」拒绝（现网 id 形如 `desktop-<毫秒>`，够不着）。
use plugin_api::naming::valid_session_id;

/// undo 截断重写前的滚动备份后缀（`<file>.undo.bak`，每次覆盖）。
const UNDO_BACKUP_SUFFIX: &str = "undo.bak";

/// 会话 id → JSONL 文件路径（`<dir>/<id>.jsonl`）。id 非法 = [`StoreError::Invalid`]。
pub fn session_file(dir: &Path, session_id: &str) -> Result<PathBuf, StoreError> {
    if !valid_session_id(session_id) {
        return Err(StoreError::Invalid(format!("非法会话 id: {session_id}")));
    }
    Ok(dir.join(format!("{session_id}.jsonl")))
}

/// 追加一行事件（打开-写-关，同 audit.jsonl 模式；聊天量级开销可忽略，
/// 且进程崩溃最多丢最后一行）。目录不存在则创建。
pub fn append_event(path: &Path, event: &SessionEvent) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(
        f,
        "{}",
        serde_json::to_string(event).expect("event serialize")
    )
}

/// 全量重写（undo fork 截断后同步磁盘）：先把原文件滚动备份为
/// `<file>.undo.bak`（每次覆盖，只留最近一次撤销前的完整原文），再写入
/// 截断后的日志。文件不存在时直接写。
pub fn rewrite_log(path: &Path, session: &Session) -> std::io::Result<()> {
    if path.exists() {
        let mut bak = path.as_os_str().to_os_string();
        bak.push(".");
        bak.push(UNDO_BACKUP_SUFFIX);
        std::fs::copy(path, PathBuf::from(bak))?;
    }
    let mut out = String::new();
    for e in session.events() {
        out.push_str(&serde_json::to_string(e).expect("event serialize"));
        out.push('\n');
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, out)
}

/// 恢复：逐行 parse → append（游标随之走到末尾 = 停机时状态）。坏行
/// （崩溃留下的半行 / 手改）跳过计数不致命——前缀事件仍然完整可用。
/// 返回（会话, 跳过行数）。
pub fn load_session(path: &Path) -> std::io::Result<(Session, usize)> {
    use std::io::BufRead;
    let file = std::fs::File::open(path)?;
    let mut session = Session::new();
    let mut skipped = 0usize;
    for line in std::io::BufReader::new(file).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<SessionEvent>(&line) {
            Ok(e) => {
                session.append(e);
            }
            Err(_) => skipped += 1,
        }
    }
    Ok((session, skipped))
}

/// 会话文件元数据（列表用；`updated_at` = mtime Unix 秒；`title` = 首条用户消息截断
/// （None = 无用户消息），截断宽 [`SESSION_TITLE_CHARS`]；`workspace` = §2.3 工作区关联
/// （v2.50，None = 未关联）；`permission_mode` = ㉘·B 会话权限模式（None = 取设置缺省）；
/// `model`/`model_provider` = v2.58 会话级模型覆盖（None = 跟随设置缺省）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMeta {
    pub id: String,
    pub updated_at: i64,
    pub events: usize,
    pub title: Option<String>,
    pub workspace: Option<String>,
    pub permission_mode: Option<String>,
    pub model: Option<String>,
    pub model_provider: Option<String>,
    /// 会话级推理档位（批 B）；None = 未设置。
    pub thinking_level: Option<String>,
}

/// 列表标题截断宽度（字符）。
pub const SESSION_TITLE_CHARS: usize = 40;

/// 目录里一个合法会话文件的枚举项（**不打开 jsonl**）。
struct SessionEntry {
    id: String,
    path: PathBuf,
    modified: std::time::SystemTime,
    updated_at: i64,
}

/// 枚举目录下全部合法会话文件（`*.jsonl`），按 mtime 降序、同 mtime 按 id 升序；
/// 目录不存在 = 空表。只读目录项与文件元数据，**一个 jsonl 都不打开**——
/// 审计 M-24d 的病根是 [`list_sessions`] 与 [`search_sessions`] 各自把每个文件
/// 读一遍解析一遍（搜索只是为了继承次序与旁车标题），共用这一层之后每个文件
/// 在一次调用里只被读一遍。
fn list_entries(dir: &Path) -> Vec<SessionEntry> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<SessionEntry> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "jsonl"))
        .filter_map(|e| {
            let path = e.path();
            let id = path.file_stem()?.to_str()?.to_string();
            if !valid_session_id(&id) {
                return None;
            }
            let meta = e.metadata().ok()?;
            let modified = meta.modified().ok()?;
            let updated_at = modified
                .duration_since(std::time::UNIX_EPOCH)
                .ok()?
                .as_secs() as i64;
            Some(SessionEntry {
                id,
                path,
                modified,
                updated_at,
            })
        })
        .collect();
    out.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| a.id.cmp(&b.id)));
    out
}

/// 单遍读一个会话文件：每条解析成功的事件交给 `visit`，返回（可解析事件数,
/// 首条用户消息标题）。打不开 = `(0, None)`；空行与坏行跳过不致命（与
/// [`load_session`] 同口径：崩溃留下的半行只丢自己）。
fn scan_session_file(path: &Path, mut visit: impl FnMut(&SessionEvent)) -> (usize, Option<String>) {
    use std::io::BufRead;
    let Ok(file) = std::fs::File::open(path) else {
        return (0, None);
    };
    let mut events = 0usize;
    let mut title = None;
    for line in std::io::BufReader::new(file).lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(ev) = serde_json::from_str::<SessionEvent>(&line) else {
            continue;
        };
        events += 1;
        if title.is_none() {
            if let SessionEvent::UserMessage { content, .. } = &ev {
                title = Some(content.chars().take(SESSION_TITLE_CHARS).collect());
            }
        }
        visit(&ev);
    }
    (events, title)
}

/// 列出目录下全部会话（`*.jsonl`，按 mtime 降序）；目录不存在 = 空表。
/// 单遍扫描：事件数 = 可解析行数，顺带抓首条用户消息做标题；旁车
/// `<id>.meta.json` 有重命名标题时优先（v2.49 会话运营）。
pub fn list_sessions(dir: &Path) -> Vec<SessionMeta> {
    list_entries(dir)
        .into_iter()
        .map(|e| {
            let (events, auto_title) = scan_session_file(&e.path, |_| {});
            // 重命名标题覆盖自动标题（v2.49）+ 工作区归属透出（v2.50）+
            // 权限模式透出（㉘·B）+ 模型覆盖透出（v2.58）：旁车一次读，
            // 缺文件 / 无字段时各自回落（整理模式透出已随 v2.137 退役）
            let sidecar = read_meta(dir, &e.id);
            let title = sidecar
                .as_ref()
                .and_then(|m| m.title.clone())
                .or(auto_title);
            let workspace = sidecar.as_ref().and_then(|m| m.workspace.clone());
            let permission_mode = sidecar.as_ref().and_then(|m| m.permission_mode.clone());
            let model = sidecar.as_ref().and_then(|m| m.model.clone());
            let model_provider = sidecar.as_ref().and_then(|m| m.model_provider.clone());
            let thinking_level = sidecar.and_then(|m| m.thinking_level);
            SessionMeta {
                id: e.id,
                updated_at: e.updated_at,
                events,
                title,
                workspace,
                permission_mode,
                model,
                model_provider,
                thinking_level,
            }
        })
        .collect()
}

/// 删除会话的全部磁盘痕迹（v2.49 会话运营）：日志、旁车元数据、undo 备份。
/// 文件缺失一律幂等跳过；返回删除前日志文件是否存在。
pub fn delete_session(dir: &Path, session_id: &str) -> std::io::Result<bool> {
    let log = session_file(dir, session_id).map_err(invalid_id)?;
    let existed = log.exists();
    let mut bak = log.clone().into_os_string();
    bak.push(".");
    bak.push(UNDO_BACKUP_SUFFIX);
    let meta = meta_file(dir, session_id).map_err(invalid_id)?;
    for p in [log, meta, PathBuf::from(bak)] {
        match std::fs::remove_file(&p) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    Ok(existed)
}

/// 正文搜索命中（v2.49 会话搜索）：不区分大小写子串扫 UserMessage /
/// `AssistantMessage` 内容，每会话记命中数与首处摘录。顺序与 [`list_sessions`]
/// 同源（共用 [`list_entries`]：mtime 降序、同 mtime 按 id 升序），取前 `limit` 条。
///
/// 每个会话文件**只读一遍、只解析一遍**（审计 M-24d：旧写法先调 `list_sessions`
/// 把全部文件读一遍解析一遍，再逐个打开重读一遍；命中计数挂在同一遍的 `visit`
/// 回调上，因此既不重复 IO 也不把正文攒进内存）；旁车只在真有命中时读。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SessionSearchHit {
    pub id: String,
    pub title: Option<String>,
    pub snippet: String,
    pub match_count: usize,
    pub updated_at: i64,
}

/// 摘录前后保留的字符数。
const SNIPPET_CONTEXT_CHARS: usize = 24;

/// 正文搜索；query trim 后空 = 空表。
pub fn search_sessions(dir: &Path, query: &str, limit: usize) -> Vec<SessionSearchHit> {
    let needle: Vec<char> = query.trim().to_lowercase().chars().collect();
    if needle.is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for e in list_entries(dir) {
        let mut count = 0usize;
        let mut snippet: Option<String> = None;
        let (_, auto_title) = scan_session_file(&e.path, |ev| {
            let text = match ev {
                SessionEvent::UserMessage { content, .. } => content,
                SessionEvent::AssistantMessage { content, .. } => content,
                _ => return,
            };
            let folded = ci_fold(text);
            // 非重叠窗口从左往右数命中；首处命中留摘录。直接在 `folded` 上比字符，
            // 不再多分配一份等长 `Vec<char>`（审计 M-24d 的「`ci_fold` 双重分配」）。
            let mut at = 0usize;
            while at + needle.len() <= folded.len() {
                let hit = folded[at..]
                    .windows(needle.len())
                    .position(|w| w.iter().map(|(c, _)| *c).eq(needle.iter().copied()));
                match hit {
                    Some(rel) => {
                        let pos = at + rel;
                        count += 1;
                        if snippet.is_none() {
                            snippet = Some(snippet_around(text, folded[pos].1, &needle));
                        }
                        at = pos + needle.len();
                    }
                    None => break,
                }
            }
        });
        if count > 0 {
            let sidecar = read_meta(dir, &e.id);
            hits.push(SessionSearchHit {
                title: sidecar.and_then(|m| m.title).or(auto_title),
                id: e.id,
                snippet: snippet.unwrap_or_default(),
                match_count: count,
                updated_at: e.updated_at,
            });
            if hits.len() >= limit {
                break;
            }
        }
    }
    hits
}

/// 大小写折叠：逐字符小写并记回**原文**字节偏移（`to_lowercase` 可能变长，
/// 不能直接用折叠串的下标切片原文）。
fn ci_fold(text: &str) -> Vec<(char, usize)> {
    let mut folded: Vec<(char, usize)> = Vec::with_capacity(text.len());
    for (b, c) in text.char_indices() {
        for lc in c.to_lowercase() {
            folded.push((lc, b));
        }
    }
    folded
}

/// 摘录：命中处前后各留 [`SNIPPET_CONTEXT_CHARS`] 字符，两端越界加 `…`，
/// 换行压成空格（侧栏行单行省略）。
fn snippet_around(text: &str, byte_start: usize, needle: &[char]) -> String {
    let start_char = text[..byte_start].chars().count();
    let all: Vec<char> = text.chars().collect();
    let from = start_char.saturating_sub(SNIPPET_CONTEXT_CHARS);
    let to = (start_char + needle.len() + SNIPPET_CONTEXT_CHARS).min(all.len());
    let mut s: String = all[from..to].iter().collect();
    if from > 0 {
        s.insert(0, '…');
    }
    if to < all.len() {
        s.push('…');
    }
    s.replace('\n', " ")
}

mod meta;

pub use meta::{
    SessionMetaFile, read_meta, write_model_override, write_permission_mode, write_thinking_level,
    write_title, write_workspace,
};
use meta::{invalid_id, meta_file};

#[cfg(test)]
mod tests;
