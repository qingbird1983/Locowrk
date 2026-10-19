//! 微压缩（specs/09 刀③ · 方案稿 B 支撑层第二格）：把**很旧的可再生工具结果**清成
//! 一行占位，先于主层 LLM 压缩触发——它的定位不是"主层的兜底"，而是**前置减速**：
//! 清掉的是历史里最没有信息密度的字节，代价为零（模型要用的话重跑一次工具即可）。
//!
//! 常量与判据逐字照 ZCode `compact/microcompact.ts`（2026-10-19 按本地源码逐行核对）：
//! 触发线 = `max(0, min(floor(主层阈值 × 0.9), 主层阈值 − 2000))`（`:79-83`）、
//! 保留最近 5 组（`:16`）、最小节省 256 token（`:18`）、idle 60 分钟是第二条触发通路
//! （`:17` + `:181-192`）、**错误结果缺省不清**（`:208` `clearErrorResults === true` 才清）。
//! 这几颗数不许"我们觉得更合理"地改动：它们是四家产品里唯一被读实的机械层参数。
//!
//! 三件我方与参照不同、且每件都有理由的地方：
//!
//! 1. **白名单按我方真实工具名**（AC-S2-5 逐条列了映射）。写侧回执（`fs_write` /
//!    `edit_*`）**不放行**：我方写工具吃 `expected_revision`，把回执清掉等于让模型
//!    丢掉刚拿到的版本号，代价比"重写一次"高。MCP 那一族名字是动态拼的
//!    （`tool-mcp/src/registry.rs:157` `mcp_<server>_<local>`）⇒ 不可枚举 ⇒ 缺省不清。
//! 2. **幂等靠内容自证**：结果正文等于 [`CLEARED`] 就不进候选集（ZCode `:230` 同一条）。
//!    这条同时给出 AC-S2-4 要的**边界单调**——已清的占据不了"最近 5 组"的名额，
//!    下一轮只会清紧随其后的那一批，边界只会前进不会回收；而且不需要额外状态源，
//!    同一份输入重放必然得到同一份输出（前缀逐字节稳定）。
//! 3. **省不够就整批还原**（ZCode `:151-156`）：不做"清一半"，因为部分清理既拿不到
//!    节省的收益，又要付一次前缀形状变化。
//!
//! 会话日志一行不动：本模块只改**发给模型的那份投影**，账记在
//! `SessionEvent::ContextCompacted{strategy:"micro"}`——写回位用的是 `agent/request`
//! payload 里那个 `compacted` 契约（`kernel.rs:253` 据此落事件，内核零改动）。

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use cordis_core::{WaterfallDecision, WaterfallListener};
use kernel_agent::{points, take_request, CompactionOutcome};
use plugin_api::CallId;
use serde_json::Value;
use session_store::Message;

use crate::settings::SettingsStore;

/// 占位正文（单行；幂等判据就是与它**全等**，所以措辞一改就等于把历史里已清的
/// 那批重新变成候选——变更它要连带考虑重放稳定性）。
pub(crate) const CLEARED: &str = "[旧工具结果已清理]";

/// 保留最近多少**组**工具结果（一组 = 一次 assistant 发起的调用及其全部返回）。
/// ⚠ 这颗数**不住在这里**（刀⑥）：它是设置项 `context.microKeepGroups`，缺省那颗 5
/// 的唯一出处在 `settings/context.rs::MICRO_KEEP_GROUPS_DEFAULT`。本模块再写一遍
/// `= 5` 就是"界面上改了、行为照旧"那种双出处形状（判据 SS23a 拦它）。
/// 第二条触发通路：距上次收束超过多少分钟就清（不看 token 线）。
pub(crate) const IDLE_THRESHOLD_MINUTES: u64 = 60;
/// 省不到这个 token 数就整批还原（机械层不值得为一点点字节破一次前缀形状）。
pub(crate) const MIN_TOKEN_SAVINGS: u64 = 256;
/// 触发线两式取小的那两枚（ZCode `:19-20`）。
pub(crate) const THRESHOLD_RATIO: f64 = 0.9;
pub(crate) const THRESHOLD_BUFFER_TOKENS: u64 = 2_000;

/// 白名单：结果**可由同一调用重取得到**的工具（真实名，逐字来自注册面取证）。
/// 对照 ZCode 那九枚的映射与不放的写侧回执/MCP 动态名，都写在 AC-S2-5。
pub(crate) const COMPACTABLE_TOOLS: &[&str] = &[
    "fs_read",
    "fs_list",
    "fs_grep",
    "shell",
    "shell_read",
    "fetch",
    "git_status",
    "git_diff",
    "git_log",
];

/// 微压缩触发线：`max(0, min(floor(主层阈值 × 0.9), 主层阈值 − 2000))`。
/// 取小拿到的永远是**更早**那条线，两侧分工（2026-10-19 按 ZCode 原式重算订正，
/// 本模块注释原来写反了）：阈值 > 20K 时比例式更低（100K→90K、800K→720K），
/// < 20K 时定值式更低（10K→8K），阈值 ≤ 2000 直接归零 ⇒ 任何请求都在线上。
/// 也就是说日常量级下说话的是 0.9 那式，减 2000 那式只在极低阈值兜底。
pub(crate) fn micro_threshold(main: u64) -> u64 {
    let ratio = (main as f64 * THRESHOLD_RATIO).floor() as u64;
    ratio.min(main.saturating_sub(THRESHOLD_BUFFER_TOKENS))
}

/// 一次判定要吃的所有读数（**纯函数**，ring 只负责把它们凑齐）。
pub(crate) struct MicroInput<'a> {
    pub messages: &'a [Message],
    /// 主层阈值（= `context_budget::effective_threshold` 那颗，同一把尺）。
    /// `None` = 窗口未知 ⇒ token 压力这一通路**不参与**（裁定 A 的另一面：
    /// 没有可信的线就不清），idle 那一条仍可用。
    pub main_threshold: Option<u64>,
    /// 距上次活动多少毫秒（`None` = 本进程还没有这个会话的活动记录，也没有日志
    /// 文件可播种 ⇒ 没有 idle 可依）。
    pub idle_ms: Option<u64>,
    /// 保留最近多少**组**（设置项 `context.microKeepGroups`）。缺省那颗 5 的唯一
    /// 出处在 `settings/context.rs`——本模块不许再写一遍数字（判据 SS23a）。
    pub keep_groups: usize,
}

/// 判定结果：`cleared` 是要置为占位的 call_id 集合。
#[derive(Debug, Default)]
pub(crate) struct MicroPlan {
    pub cleared: Vec<CallId>,
    pub from_tokens: u64,
    pub to_tokens: u64,
    /// 未清的名额内组数（观测面用；= 保留的组数）。
    pub kept_groups: usize,
    pub reason: &'static str,
}

/// 结果正文是不是"已经清过"的那一行。
fn already_cleared(m: &Message) -> bool {
    matches!(m, Message::ToolResult { output, error, .. }
        if error.is_none() && output == &Value::String(CLEARED.to_string()))
}

/// 结果正文带媒体（`asset` / `files` 字段形状，与 `llm/wire/media.rs` 同一判法）：
/// 清了它，模型重新调用也未必还能拿到同一张图（截图是当下视口的东西）。
fn has_media(output: &Value) -> bool {
    fn hit(v: &Value) -> bool {
        match v {
            Value::Object(map) => map
                .iter()
                .any(|(k, inner)| k == "asset" || k == "assets" || hit(inner)),
            Value::Array(items) => items.iter().any(hit),
            _ => false,
        }
    }
    hit(output)
}

/// 候选分组：一条带 `tool_calls` 的 assistant 开一组，随后到达的 tool 结果归进这一组
/// （ZCode `:219-242` 同形）。返回**每组 call_id 列表**，顺序 = 历史顺序。
fn candidate_groups(messages: &[Message]) -> Vec<Vec<CallId>> {
    let whitelist: HashSet<&str> = COMPACTABLE_TOOLS.iter().copied().collect();
    let mut groups: Vec<Vec<CallId>> = Vec::new();
    let mut open = false; // 当前是否有开着的一组
    for m in messages {
        match m {
            Message::Assistant { tool_calls, .. } => {
                if !tool_calls.is_empty() {
                    groups.push(Vec::new());
                    open = true;
                }
                // 不带调用的纯正文不改变分组状态（一条 assistant 说一句就走一组）。
            }
            Message::ToolResult {
                call_id,
                tool,
                output,
                error,
                ..
            } => {
                if error.is_some() {
                    continue; // 错误结果缺省不清（实证①）
                }
                if !whitelist.contains(tool.as_str()) || has_media(output) || already_cleared(m) {
                    continue;
                }
                if open {
                    if let Some(last) = groups.last_mut() {
                        last.push(call_id.clone());
                    }
                } else {
                    // 孤儿结果（没有对应 assistant 调用记录，如恢复自旧日志的残段）：
                    // 自成一组，与 ZCode `:233-235` 的 `!currentGroup` 支同形。
                    groups.push(vec![call_id.clone()]);
                }
            }
            _ => {}
        }
    }
    groups.retain(|g| !g.is_empty());
    groups
}

/// 纯判定：要不要清、清哪些。任何一步不满足都如实带回 `reason`（观测面读它）。
pub(crate) fn plan(input: &MicroInput<'_>) -> MicroPlan {
    let from_tokens = kernel_agent::estimate_tokens(input.messages);
    let groups = candidate_groups(input.messages);
    let mut base = MicroPlan {
        from_tokens,
        to_tokens: from_tokens,
        kept_groups: groups.len(),
        cleared: Vec::new(),
        reason: "no-candidates",
    };

    // 触发（两条通路，idle 优先——ZCode `:184-196` 就是先看时间再看 token 线）。
    let idle_over = input
        .idle_ms
        .map(|ms| ms > IDLE_THRESHOLD_MINUTES * 60_000)
        .unwrap_or(false);
    let token_over = match input.main_threshold {
        Some(main) => from_tokens >= micro_threshold(main),
        None => false,
    };
    if !idle_over && !token_over {
        base.reason = "not-triggered";
        return base;
    }
    if groups.is_empty() {
        base.reason = "no-candidates";
        return base;
    }
    let keep = input.keep_groups.max(1);
    let clear_count = groups.len().saturating_sub(keep);
    if clear_count == 0 {
        base.reason = "nothing-to-clear";
        return base;
    }
    base.kept_groups = groups.len() - clear_count;

    let cleared: Vec<CallId> = groups[..clear_count].iter().flatten().cloned().collect();
    // 先算省了多少，不够就把整批还回去（不做"清一半"）。
    let cleared_set: HashSet<&CallId> = cleared.iter().collect();
    let projected: Vec<Message> = input
        .messages
        .iter()
        .map(|m| match m {
            Message::ToolResult {
                call_id, output, ..
            } if cleared_set.contains(call_id) => {
                let mut clone = m.clone();
                if let Message::ToolResult { output, .. } = &mut clone {
                    *output = Value::String(CLEARED.to_string());
                }
                clone
            }
            other => other.clone(),
        })
        .collect();
    let to_tokens = kernel_agent::estimate_tokens(&projected);
    let saved = from_tokens.saturating_sub(to_tokens);
    if saved < MIN_TOKEN_SAVINGS {
        base.reason = "below-min-savings";
        base.cleared = Vec::new();
        return base;
    }
    base.cleared = cleared;
    base.to_tokens = to_tokens;
    base.reason = "applied";
    base
}

/// 主层阈值的供给通路（组装点给的闭包：会话 id → 有效阈值，窗口未知 ⇒ `None`）。
/// 做成别名不是为了好看：clippy 的 `type_complexity` 在结构体字段、构造函数参数、
/// 组装点注解上三处同报同一个形状，把它们各自拆开反而等于把同一形状抄三遍——
/// 别名才是"一个出处"的写法（`OrganizeNotifier` 同族）。
pub(crate) type MainThreshold = Arc<dyn Fn(Option<&str>) -> Option<u64> + Send + Sync>;

/// 会话 → 「上一次见到它活动」的毫秒时刻（idle 通路的输入）。
///
/// 参照的是 ZCode 的 `lastAssistantCompletedAtMs`（"距上次收束多久"）。我方取的是
/// **上一次活动**，两者差一轮的时长（几秒到几分钟），相对 60 分钟阈值可忽略——
/// 但必须说破，不许把注释写成同一个东西。
///
/// 两处设计上的讲究：
/// * **进程内表 + 磁盘 mtime 播种**：本进程没见过这个会话时（冷启后第一次请求，
///   正是"隔天回来接着聊"那一格），用会话日志的 mtime 当"上次活动"——轮末会
///   append 事件，mtime 天然等于最后一次收束时刻，一次 stat 就够，
///   于是跨重启没有空窗，也不必把每个 turn 全量读进内存。
/// * 表放在监听器内部（同 `ThinkingLanguageInject` 的解析缓存一个形状），
///   **不新增跨层管道**：把时间戳从 runner 传到内核决策环要动一串签名与所有调用点，
///   而这里需要的只是一个数。
#[derive(Default)]
struct LastSeen {
    map: HashMap<String, u64>,
}

impl LastSeen {
    /// 返回"距上次活动多少毫秒"，并把 `now_ms` 记成新的活动时刻。
    /// `seed` 只在本进程**第一次**见到这个会话时调用（冷启播种那条通路）。
    ///
    /// 只往新走：`now_ms < last`（时钟回拨 / 同一请求内重入）不许把较新的时刻倒退回
    /// 去，此时差值取 0 ⇒ 不会凭空触发 idle。
    fn observe(&mut self, session_id: &str, now_ms: u64, seed: impl FnOnce() -> u64) -> u64 {
        let last = match self.map.get(session_id) {
            Some(ts) => *ts,
            None => {
                let ts = seed();
                self.map.insert(session_id.to_string(), ts);
                ts
            }
        };
        if now_ms >= last {
            self.map.insert(session_id.to_string(), now_ms);
            now_ms - last
        } else {
            0
        }
    }
}

/// `agent/request` 上的微压缩监听器（宿主内置，同 ThinkingLanguageInject 一族的
/// 注册方式）。链位：注册在**所有注入环之后**（`state/waterfall.rs` 末尾）——它动的是
/// messages，不碰 system；放在末尾是因为它要看到的是别的环改写完成后的最终消息列。
pub(crate) struct MicroCompact {
    settings: Arc<SettingsStore>,
    /// 会话日志目录（冷启后第一次见某个会话时用它的 mtime 播种 idle）。
    sessions_dir: PathBuf,
    seen: Mutex<LastSeen>,
    /// 主层阈值供给（组装点给：`context_budget::effective_threshold` + 会话生效窗口）。
    /// 注成闭包而不是让本模块自己去读 catalog/覆盖旁车——那份四层链已经有一个出处。
    main_threshold: MainThreshold,
}

impl MicroCompact {
    pub(crate) fn new(
        settings: Arc<SettingsStore>,
        sessions_dir: PathBuf,
        main_threshold: MainThreshold,
    ) -> Self {
        Self {
            settings,
            sessions_dir,
            seen: Mutex::new(LastSeen::default()),
            main_threshold,
        }
    }

    /// 会话日志的 mtime（轮末 append ⇒ 它就是最后一次收束时刻）。取不到就当作"现在"
    /// = 首次活动，不触发 idle（新会话与定位失败都落这一格，宁可不清也不误清）。
    fn mtime_ms(&self, session_id: &str) -> Option<u64> {
        let path = session_store::session_file(&self.sessions_dir, session_id).ok()?;
        let t = std::fs::metadata(path).ok()?.modified().ok()?;
        Some(crate::timeutil::to_epoch_ms(t).max(0) as u64)
    }

    /// 现在这一刻（epoch 毫秒；本模块一律用非负毫秒做差，负时钟不参与判定）。
    fn now_ms() -> u64 {
        crate::timeutil::now_ms().max(0) as u64
    }
}

#[async_trait]
impl WaterfallListener for MicroCompact {
    async fn handle(&self, key: &str, payload: &mut Value) -> WaterfallDecision {
        if key != points::REQUEST {
            return WaterfallDecision::Continue;
        }
        if !self.settings.context_micro_enabled() {
            return WaterfallDecision::Continue; // 开关关 = 完全不参与（设置 → 上下文）
        }
        let mut guard = match take_request(payload, "MicroCompact") {
            Ok(g) => g,
            Err(d) => return d,
        };
        let req = guard.req();
        let now_ms = MicroCompact::now_ms();
        let idle_ms = req.session_id.as_deref().map(|sid| {
            let mut seen = match self.seen.lock() {
                Ok(g) => g,
                // 锁中毒（曾经有 panic 穿过）⇒ 这一轮没有 idle 可依，
                // 但 token 通路照走：不因状态不可用就连减速层一起关掉。
                Err(_) => return 0,
            };
            seen.observe(sid, now_ms, || self.mtime_ms(sid).unwrap_or(now_ms))
        });
        // 保留组数**取一次存局部**：判定与下面那句记账文本要用同一个数，读两次设置
        // 就可能在中途被用户保存插队（同一轮里两个数 = 账对不上行为）。
        let keep_groups = self.settings.context_micro_keep_groups();
        let p = plan(&MicroInput {
            messages: &req.messages,
            main_threshold: (self.main_threshold)(req.session_id.as_deref()),
            idle_ms,
            keep_groups,
        });
        if p.reason != "applied" {
            tracing::debug!(reason = p.reason, from = p.from_tokens, "微压缩未介入");
            return WaterfallDecision::Continue;
        }
        let cleared: HashSet<&CallId> = p.cleared.iter().collect();
        let mut touched = 0usize;
        for m in &mut req.messages {
            if let Message::ToolResult {
                call_id, output, ..
            } = m
            {
                if cleared.contains(call_id) {
                    *output = Value::String(CLEARED.to_string());
                    touched += 1;
                }
            }
        }
        // 记账走的是 payload 里那个 `compacted` 带回位：kernel 据此落
        // `ContextCompacted` 事件（`kernel.rs:253`），本模块不碰会话日志。
        req.compacted = Some(CompactionOutcome {
            from_tokens: p.from_tokens,
            to_tokens: p.to_tokens,
            strategy: "micro".into(),
            summary: format!(
                "{CLEARED}：清 {touched} 条旧工具结果（保留最近 {} 组），节省 {} token",
                keep_groups,
                p.from_tokens.saturating_sub(p.to_tokens)
            ),
            kept_messages: req.messages.len(),
        });
        tracing::info!(
            cleared = touched,
            kept = p.kept_groups,
            from = p.from_tokens,
            to = p.to_tokens,
            idle = idle_ms.is_some_and(|ms| ms > IDLE_THRESHOLD_MINUTES * 60_000),
            "微压缩：旧工具结果清成占位"
        );
        WaterfallDecision::Continue
    }
}

#[cfg(test)]
mod tests;
