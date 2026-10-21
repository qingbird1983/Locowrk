//! 会话落盘接线（⑲，DESIGN.md §4 落盘布局）：session_store::persist 在
//! 组装点侧的薄封装。磁盘 `<数据根>/sessions/<id>.jsonl` 是内存日志的
//! **镜像**——runner 每步把新增段追加进去，undo fork 截断后整体重写
//! （滚动备份留痕），启动时逐文件回放恢复内存会话表。
//!
//! 纪律对齐：日志只追加不删除（§4.3）在磁盘面的例外只有 undo 重写，
//! 且重写前必有 `.undo.bak` 备份——撤销的物理留痕。

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use session_store::{
    append_events, delete_session, list_sessions, load_session, rewrite_log, search_sessions,
    session_file, write_title, write_workspace, SessionMeta, SessionSearchHit,
};
use session_store::{Session, SessionEvent};

type SessionsMap = Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<Session>>>>>;

/// 启动恢复：逐个会话文件回放进内存表（坏文件/空文件跳过不拦启动）。
/// 返回恢复的会话数。必须在调度引擎等会话消费方启动之前调用。
pub fn restore_all(dir: &Path, sessions: &SessionsMap) -> usize {
    let mut restored = 0usize;
    for meta in list_sessions(dir) {
        let path = match session_file(dir, &meta.id) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(id = %meta.id, "session log 路径非法: {e}");
                continue;
            }
        };
        match load_session(&path) {
            Ok((s, skipped)) => {
                if skipped > 0 {
                    tracing::warn!(id = %meta.id, skipped, "session log 坏行跳过");
                }
                if s.is_empty() {
                    continue;
                }
                // G2 崩溃恢复观测：悬空且未裁决的调用数进启动日志（裁决
                // 的呈现走会话重放的 uncertain 卡，这里只留观测痕迹）。
                let scan = session_store::scan_uncertain(s.events());
                let pending = scan.unresolved(s.events()).len();
                if pending > 0 {
                    tracing::info!(id = %meta.id, pending, "session log 有未裁决的悬空调用（uncertain）");
                }
                sessions
                    .lock()
                    .expect("sessions poisoned")
                    .insert(meta.id, Arc::new(tokio::sync::Mutex::new(s)));
                restored += 1;
            }
            Err(e) => tracing::warn!(id = %meta.id, "session log 读取失败: {e}"),
        }
    }
    restored
}

/// **唯一落盘口**：把会话里「还没落盘的那一段」追加进磁盘镜像。
///
/// 起点**不由调用方给**——它住会话里（`Session::persisted` = 已落盘到第几条）。
/// 改前每个调用点自己算 `from`（runner 的 `log_start` / `before` / `len-1`、压缩层的
/// `from`、heal 的 `seq`），于是"磁盘上到哪儿了"有**好几个出处**：一处漏算就是
/// 缺口（那段永远不落盘），两处算重就是重影（同一条事件落两遍）。游标只有一个、
/// 住会话 ⇒ 谁落盘都从同一处接着写。
///
/// 写失败**不推进游标** ⇒ 下一趟重写这一段 ⇒ 至多一次、**绝不缺口**
/// （失败语义仍是「只告警不阻断聊天」：落盘是持久性保障，不该反过来卡死会话）。
pub fn persist_pending(dir: &Path, session_id: &str, session: &mut Session) {
    let from = session.persisted();
    if from >= session.len() {
        return; // 没有新东西（幂等：连着落两次不会重影）
    }
    let Ok(path) = session_file(dir, session_id) else {
        tracing::warn!(session_id, "session log 文件名非法，本段不落盘");
        return;
    };
    // 批 F-24b / M-18g：改前逐条 `append_event` = 每条事件一次 open/write/close（每轮
    // 3N–6N 次系统调用），现在一段一次。失败语义照旧「只告警不阻断聊天」，但**失败粒度
    // 从逐条变逐段**（`BufWriter` 缓冲内未 flush 的尾巴不落盘），已如实记进 persist.rs 的
    // `append_events` 文档注释与本批 ROADMAP 边界。
    if let Err(err) = append_events(&path, &session.events()[from..]) {
        tracing::warn!(session_id, "session log 追加失败: {err}");
        return; // **不推进游标**：下一趟把这一段补上
    }
    session.mark_persisted(session.len());
}

/// undo fork 截断后同步磁盘（重写 + 滚动备份）。会话尚无文件 = 无需重写。
///
/// 游标也跟着重写：重写之后**盘上就是整个 `session`** ⇒ `persisted` = 事件数
/// （不落定的话，下一次追加会把这整段再写一遍 = 重影）。
pub fn rewrite_after_undo(dir: &Path, session_id: &str, session: &mut Session) {
    let Ok(path) = session_file(dir, session_id) else {
        return;
    };
    if !path.exists() {
        return;
    }
    if let Err(e) = rewrite_log(&path, session) {
        tracing::warn!(session_id, "session log undo 重写失败: {e}");
        return;
    }
    session.mark_persisted(session.len());
}

/// 消息编辑重发的截断核心（v2.57 §3.34，纯内存）：`pos = Some(p)` 要求
/// `events[p]` 是 UserMessage 并截到 p 之前；`None` = 最后一条用户消息。
/// 返回实际截断位。rewind_to + fork（⑨ 前缀游标语义），物理重写由命令层
/// 调 [`rewrite_after_undo`]。
pub fn truncate_after(session: &mut Session, pos: Option<usize>) -> Result<usize, String> {
    let target = match pos {
        Some(p) => {
            let n = session.len();
            if p >= n {
                return Err(format!("截断位 {p} 越界（日志共 {n} 条事件）"));
            }
            if !matches!(session.events()[p], SessionEvent::UserMessage { .. }) {
                return Err("截断位不是用户消息".into());
            }
            p
        }
        None => session
            .events()
            .iter()
            .rposition(|e| matches!(e, SessionEvent::UserMessage { .. }))
            .ok_or_else(|| "会话里没有用户消息可编辑".to_string())?,
    };
    session.rewind_to(target);
    *session = session.fork(target);
    Ok(target)
}

/// 列表（mtime 降序）：直接透出 SessionMeta（Serialize）。
pub fn list(dir: &Path) -> Vec<SessionMeta> {
    list_sessions(dir)
}

/// 重命名（v2.49 会话运营）：写旁车 meta.json 标题（trim 后空 = 回落
/// 自动标题）。内存会话不动——标题只在列表层生效。
pub fn rename(dir: &Path, session_id: &str, title: &str) -> std::io::Result<()> {
    write_title(dir, session_id, title)
}

/// 工作区关联（v2.50 §2.3）：写旁车 meta.json `workspace`（None/空 = 移
/// 出）。内存会话不动——归属只在列表层生效。
pub fn associate(dir: &Path, session_id: &str, workspace: Option<&str>) -> std::io::Result<()> {
    write_workspace(dir, session_id, workspace)
}

/// 删除（v2.49 会话运营）：日志 + 旁车 + undo 备份一并移除（幂等）。
/// 内存条目与调度最近目标的剔除在命令层做（要碰 AppState）。
pub fn delete(dir: &Path, session_id: &str) -> std::io::Result<bool> {
    delete_session(dir, session_id)
}

/// 正文搜索（v2.49 会话搜索）：UserMessage / AssistantMessage 大小写
/// 不敏感子串扫描，mtime 降序限量。
pub fn search(dir: &Path, query: &str, limit: usize) -> Vec<SessionSearchHit> {
    search_sessions(dir, query, limit)
}
