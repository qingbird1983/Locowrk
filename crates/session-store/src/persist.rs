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

use serde::{Deserialize, Serialize};

use crate::error::StoreError;
use crate::event::SessionEvent;
use crate::session::Session;

/// undo 截断重写前的滚动备份后缀（`<file>.undo.bak`，每次覆盖）。
const UNDO_BACKUP_SUFFIX: &str = "undo.bak";

/// 会话 id 允许字符：ASCII 字母数字与 `.` `_` `-`——前端生成
/// `desktop-<毫秒>`，第三方形态留同族；拒绝路径分隔符防穿越。
fn valid_session_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

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

/// 会话文件元数据（列表用；`updated_at` = 文件 mtime 的 Unix 秒；
/// `title` = 首条用户消息截断（切换器选项文案，None = 还没有用户消息），
/// 截断宽度 [`SESSION_TITLE_CHARS`]）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMeta {
    pub id: String,
    pub updated_at: i64,
    pub events: usize,
    pub title: Option<String>,
}

/// 列表标题截断宽度（字符）。
pub const SESSION_TITLE_CHARS: usize = 40;

/// 列出目录下全部会话（`*.jsonl`，按 mtime 降序）；目录不存在 = 空表。
/// 单遍扫描：事件数 = 可解析行数，顺带抓首条用户消息做标题；旁车
/// `<id>.meta.json` 有重命名标题时优先（v2.49 会话运营）。
pub fn list_sessions(dir: &Path) -> Vec<SessionMeta> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut metas: Vec<(std::time::SystemTime, SessionMeta)> = entries
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
            let (events, title) = std::fs::File::open(&path)
                .map(|f| {
                    use std::io::BufRead;
                    let mut events = 0usize;
                    let mut title = None;
                    for line in std::io::BufReader::new(f).lines().map_while(Result::ok) {
                        if line.trim().is_empty() {
                            continue;
                        }
                        let Ok(ev) = serde_json::from_str::<SessionEvent>(&line) else {
                            continue;
                        };
                        events += 1;
                        if title.is_none() {
                            if let SessionEvent::UserMessage { content } = ev {
                                title = Some(content.chars().take(SESSION_TITLE_CHARS).collect());
                            }
                        }
                    }
                    (events, title)
                })
                .unwrap_or((0, None));
            // 重命名标题覆盖自动标题（v2.49）：缺文件 / 无字段时回落
            let title = read_meta(dir, &id).and_then(|m| m.title).or(title);
            Some((
                modified,
                SessionMeta {
                    id,
                    updated_at,
                    events,
                    title,
                },
            ))
        })
        .collect();
    metas.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.id.cmp(&b.1.id)));
    metas.into_iter().map(|(_, m)| m).collect()
}

/// 会话旁车元数据（`<id>.meta.json`，v2.49 会话运营）：存重命名标题；
/// `workspace` 留给 §2.3 会话↔工作区关联批次。缺文件 = 全缺省。
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SessionMetaFile {
    pub title: Option<String>,
    pub workspace: Option<String>,
}

/// 旁车元数据路径（id 校验同日志文件；`<id>.jsonl` → `<id>.meta.json`，
/// 后缀替换只动 `jsonl`，id 内含 `.` 不受影响）。
fn meta_file(dir: &Path, session_id: &str) -> Result<PathBuf, StoreError> {
    Ok(session_file(dir, session_id)?.with_extension("meta.json"))
}

/// 读旁车元数据：缺文件 / 坏 JSON = None（不致命——标题回落自动推导）。
pub fn read_meta(dir: &Path, session_id: &str) -> Option<SessionMetaFile> {
    let path = meta_file(dir, session_id).ok()?;
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn invalid_id(e: StoreError) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, e.to_string())
}

/// 写重命名标题（v2.49）：trim 后空串 = 删除旁车文件（回落自动标题）；
/// 非空 = 合并写入（保留 workspace 等既有字段）。
pub fn write_title(dir: &Path, session_id: &str, title: &str) -> std::io::Result<()> {
    let path = meta_file(dir, session_id).map_err(invalid_id)?;
    let t = title.trim();
    if t.is_empty() {
        return match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        };
    }
    let mut meta = read_meta(dir, session_id).unwrap_or_default();
    meta.title = Some(t.to_string());
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&meta).expect("meta serialize"),
    )
}

/// 删除会话的全部磁盘痕迹（v2.49 会话运营）：日志、旁车元数据、undo
/// 备份。文件缺失一律幂等跳过；返回删除前日志文件是否存在。
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

/// 正文搜索命中（v2.49 会话搜索）：不区分大小写子串扫 `UserMessage` /
/// `AssistantMessage` 内容，每会话记命中数与首处摘录。顺序复用
/// [`list_sessions`]（mtime 降序），取前 `limit` 条。
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
    use std::io::BufRead;
    let needle: Vec<char> = query.trim().to_lowercase().chars().collect();
    if needle.is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for meta in list_sessions(dir) {
        let Ok(path) = session_file(dir, &meta.id) else {
            continue;
        };
        let Ok(file) = std::fs::File::open(&path) else {
            continue;
        };
        let mut count = 0usize;
        let mut snippet: Option<String> = None;
        for line in std::io::BufReader::new(file).lines().map_while(Result::ok) {
            let Ok(ev) = serde_json::from_str::<SessionEvent>(&line) else {
                continue;
            };
            let text = match ev {
                SessionEvent::UserMessage { content } => content,
                SessionEvent::AssistantMessage { content, .. } => content,
                _ => continue,
            };
            let folded = ci_fold(&text);
            let chars: Vec<char> = folded.iter().map(|(c, _)| *c).collect();
            // 非重叠窗口从左往右数命中；首处命中留摘录
            let mut at = 0usize;
            while at + needle.len() <= chars.len() {
                match chars[at..].windows(needle.len()).position(|w| w == needle) {
                    Some(rel) => {
                        let pos = at + rel;
                        count += 1;
                        if snippet.is_none() {
                            snippet = Some(snippet_around(&text, folded[pos].1, &needle));
                        }
                        at = pos + needle.len();
                    }
                    None => break,
                }
            }
        }
        if count > 0 {
            hits.push(SessionSearchHit {
                id: meta.id.clone(),
                title: meta.title.clone(),
                snippet: snippet.unwrap_or_default(),
                match_count: count,
                updated_at: meta.updated_at,
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

#[cfg(test)]
mod tests;
