//! 轮末汇总（CF §10 · C1，队列 Q12·第二刀 v2.205）：本轮结束时由日志现推的三件事
//! ——**改了哪些文件**（`collect_file_changes`）、**本轮区间从哪开始**（`turn_slice_start`）、
//! **这轮真正干了多久**（`active_ms` = 墙钟区间 − Σ审批等待）。
//!
//! 为什么单独立一个文件而不是留在 `projection.rs`：本模块的三个算式都是「轮末汇总」，
//! 而 `projection.rs` 的主体是「逐条事件 → 一条 UI 事件」的映射；两件事的**承重面**不同
//! （前者按区间聚合、后者按单条搬运）。分开后 `projection.rs` 从 387 行降到 ~280，
//! 新增的等待时长算式也有了落点，不需要为凑 400 行建议线去挤任何一段代码。
//!
//! 铁律（历批立的，这里逐条钉住）：
//!   · **权威源在日志里**：±N 出自插件写类工具结果，等待出自 `ApprovalDecided` 的
//!     `asked_at`/`decided_at`（v2.205 新增）。宿主不读盘补 diff、不在读的时候编时刻。
//!   · **实时与重放同一把尺**：`finish_turn`（经 [`emit_turn_summary`]）与
//!     `project_events` 的 TurnEnd 支调的是**同一批函数**，专测 =
//!     `tests_session_log/file_changes.rs` 第 4 条与 `turn_header.rs` 第 4 条。
//!   · **有一半是猜的就不报**：任一次读不出数 ⇒ 整条给 `None`，UI 不渲染，绝不给 0。

use crate::events::{AgentEvent, EventSink, FileChangePayload, TurnState};
use serde_json::Value;
use session_store::{Session, SessionEvent, TurnId, TurnOutcome};
use std::collections::HashMap;

/// 一条工具结果 → 文件改动（kind + ±N），读不出是文件改动就返回 `None`。
///
/// **行数的权威源在插件侧**（后审计队列 Q12·C1 甲案）：`tool-edit` 的三个写
/// 工具与 `tool-fs` 的 fs_write 新建支在结果里报 `additions` / `deletions`，
/// 随 `ToolResult` 一起进会话日志 ⇒ 重放拿到的数与当场看到的同一个。宿主
/// **不读盘补算 diff**——读盘算出的数不在日志里，重放同一次会话会漂移。
///
/// `kind` 在这里定形而不让插件自报：四个写工具各报一套名词，前端就得各有
/// 分支（《Front-end Design Guidelines》同类只许一种形态）。`overwritten` =
/// 整文件覆盖，给不出 diff ⇒ 两个数留 `None`，是如实读数不是零（UI 不渲染）。
pub(crate) fn file_change_of(output: &Value) -> Option<(&'static str, Option<i64>, Option<i64>)> {
    let path_present = output.get("path").and_then(Value::as_str).is_some();
    if !path_present {
        return None;
    }
    if output.get("deleted").and_then(Value::as_bool) == Some(true) {
        return Some(("deleted", None, None));
    }
    // 写类结果的识别 = 载荷里**有这两个键**（值可以是 null）；fs_read 等只读
    // 工具带 path 但不带它们，因此不会混进来。
    if !["additions", "deletions"]
        .iter()
        .any(|k| output.get(*k).is_some())
    {
        return None;
    }
    let additions = output.get("additions").and_then(Value::as_i64);
    let deletions = output.get("deletions").and_then(Value::as_i64);
    let kind = match (
        output.get("existed").and_then(Value::as_bool),
        additions,
        deletions,
    ) {
        (Some(false), _, _) => "created",
        (Some(true), None, None) => "overwritten",
        _ => "modified",
    };
    Some((kind, additions, deletions))
}

/// 本轮在日志里的区间起点 = **最后一条 `TurnStart` 之后那一条**（实时路径用）。
/// 权威源取日志本身而不是让调用方记变量：①`finish_turn` 的参数面不随投影需求增长
/// （加到第八个参数会撞 `clippy::too_many_arguments`，历批已立过「收拢优于加参」的取向）；
/// ②调用方传的切口一旦传错就是**静默少聚合 / 多聚合**，而 `TurnStart` 是落了盘的事件，读它不会错。
/// 找不到 `TurnStart`（旧日志或缺轮界行的极端形态）⇒ 返回 `session.len()` = 空区间 =
/// 不发 `FileChanges`，**宁缺勿假**。
/// ⚠ 重放路径不叫这个函数，但**必须同语义**：`project_events` 在遍历中把游标停在
/// `TurnStart` 之后一条（`turn_start = pos`），两者切出的区间一致——由
/// `tests_session_log/file_changes.rs` 第 4 条「实时与重放逐字节同尺」钉住，不许漂。
pub(crate) fn turn_slice_start(session: &Session) -> usize {
    session
        .events()
        .iter()
        .rposition(|e| matches!(e, SessionEvent::TurnStart { .. }))
        .map(|i| i + 1)
        .unwrap_or(session.len())
}

/// 一段事件（本轮区间）→ 文件改动清单，顺序 = 首次出现顺序。
/// 同一文件本轮改多次聚合成一条：±N 累加，但只要有一次读不出数就**整条留
/// None**（有一半是猜的就不报），kind 取最后一次（"这次最终把它怎么样了"）。
/// 实时（`finish_turn`）与重放（[`project_events`] 的 TurnEnd 支）共用这一份
/// 实现——形状判据钉住"两处同一把尺"，跨文件判据补的是单文件测试碰不到的面。
pub(crate) fn collect_file_changes(events: &[SessionEvent]) -> Vec<FileChangePayload> {
    let mut order: Vec<String> = Vec::new();
    let mut acc: HashMap<String, (String, Option<i64>, Option<i64>)> = HashMap::new();
    for e in events {
        let SessionEvent::ToolResult { output, .. } = e else {
            continue;
        };
        let Some((kind, add, del)) = file_change_of(output) else {
            continue;
        };
        let path = output
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !acc.contains_key(path) {
            order.push(path.to_string());
        }
        let entry = acc
            .entry(path.to_string())
            .or_insert_with(|| (String::new(), Some(0), Some(0)));
        entry.0 = kind.to_string();
        entry.1 = match (entry.1, add) {
            (Some(a), Some(v)) => Some(a + v),
            _ => None, // 任一次给不出数 ⇒ 整条不给（有一半是猜的就不报）
        };
        entry.2 = match (entry.2, del) {
            (Some(d), Some(v)) => Some(d + v),
            _ => None,
        }
    }
    order
        .into_iter()
        .filter_map(|path| {
            acc.remove(&path)
                .map(|(kind, additions, deletions)| FileChangePayload {
                    path,
                    kind,
                    additions,
                    deletions,
                })
        })
        .collect()
}

/// 轮界反查（实时路径）：本轮的**墙钟起点** = [`turn_slice_start`] 前一条那颗
/// `TurnStart` 的 `ts`。刻意不另做第二次反向扫描 ⇒ 轮界在全模块只有一个定义处
/// （`turn_slice_start`），两处各扫一遍迟早会漂成两颗不同的事件。
/// `None` = 旧日志无轮界行或 `ts == 0` ⇒ 调用方不虚构读数。
pub(crate) fn turn_wall_start(session: &Session) -> Option<i64> {
    let prev = turn_slice_start(session).checked_sub(1)?;
    match session.events().get(prev)? {
        SessionEvent::TurnStart { ts, .. } => (*ts != 0).then_some(*ts),
        _ => None,
    }
}

/// 本轮区间内**等待用户审批**的总毫秒数 = Σ(`decided_at` − `asked_at`)。
///
/// 三档 `None`（都不许折算成 0）：①本轮有审批、但任一颗缺时刻 = 旧日志（v2.205 之前
/// 的会话全是这一档）；②`decided_at < asked_at` = 时钟回拨，这段等待有多久不知道；
/// ③……就这两档，第三条留给未来的取值扩展。
/// 本轮**压根没有审批** ⇒ `Some(0)`（与"有审批但读不出"必须可区分：前者是没等待，
/// 后者是不知道等了多久——内核在无决议者时写 `asked_at = None`，正是为了让这两档
/// 在数据层就分开）。
pub(crate) fn waited_ms(events: &[SessionEvent]) -> Option<i64> {
    let mut sum = 0i64;
    for e in events {
        if let SessionEvent::ApprovalDecided {
            asked_at,
            decided_at,
            ..
        } = e
        {
            match (asked_at, decided_at) {
                (Some(asked), Some(decided)) if decided >= asked => sum += decided - asked,
                _ => return None, // 读不出就不给整轮的数：等待段是扣减项，猜一次就全错
            }
        }
    }
    Some(sum)
}

/// 本轮「真正干活」的毫秒数 = 墙钟区间 − [`waited_ms`]。
///
/// `None` 的三档：轮起点缺、轮终点缺、等待段读不出（外加负区间这一防御）。
/// 起点与终点由调用方给（实时 = [`turn_wall_start`] 配 `finish_turn` 的收束时刻；
/// 重放 = `project_events` 里停在 `TurnStart` 上的游标配 `TurnEnd.ts`）——
/// **算式只在这里有一份**，两处调用方只负责切出同一颗轮界。
pub(crate) fn active_ms(
    wall_start: Option<i64>,
    wall_end: Option<i64>,
    events: &[SessionEvent],
) -> Option<i64> {
    let (start, end) = (wall_start?, wall_end?);
    let wall = end.checked_sub(start)?;
    if wall < 0 {
        return None; // 时钟回拨：连"这轮跑了多久"都没有权威答案
    }
    Some(wall.saturating_sub(waited_ms(events)?))
}

/// 实时路径的轮末汇总 emit（`finish_turn` 唯一调用点）：先发 [`AgentEvent::TurnHeader`]
/// （轮头读数），再发 [`AgentEvent::FileChanges`]（改动清单），**都早于 TurnDone**——
/// 前端把轮头与清单挂在轮尾，TurnDone 一到就冻结读数（与重放支的 push 顺序逐字同形）。
/// 空清单不发（宁缺勿假）；`turn_id` 与区间都由日志现推，调用方不记变量 ⇒
/// `finish_turn` 的参数面不随投影需求增长（第七参封顶，第八颗会撞 `too_many_arguments`）。
pub(crate) fn emit_turn_summary(
    session: &Session,
    session_id: &str,
    turn_id: TurnId,
    outcome: TurnOutcome,
    end_ts: i64,
    sink: &dyn EventSink,
) {
    let start = turn_slice_start(session);
    let turn = &session.events()[start..session.len() - 1];
    sink.emit(&AgentEvent::TurnHeader {
        session_id: session_id.into(),
        turn_id: Some(turn_id.0),
        state: TurnState::from(outcome),
        active_ms: active_ms(turn_wall_start(session), Some(end_ts), turn),
    });
    let files = collect_file_changes(turn);
    if !files.is_empty() {
        sink.emit(&AgentEvent::FileChanges {
            session_id: session_id.into(),
            turn_id: Some(turn_id.0),
            files,
        });
    }
}
