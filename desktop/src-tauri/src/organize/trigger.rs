//! 触发与决策面：`plan_auto`（轮次收束后的自动判定，含 M4 模式 B 溢出整理与
//! §5.5 反弹熔断）/ `plan_manual`（`/整理`，不受熔断限制且复位熔断态）。两者都是
//! **同步**决策、可测：写路径含反弹账本与占位章节落盘，产物是管道输入 TaskPayload
//! （材料项克隆自触发瞬间的会话日志——事件流 append-only，区间恒定有效）。
//! 执行管道在 task.rs，空间急救在 collapse.rs。

use memory_tree::{
    build_items, should_auto_organize, MaterialItem, MemoryTree, OrganizeReason, Pacing,
    ReboundDecision, TriggerInput,
};
use session_store::Session;

use crate::settings::TailStrategy;

use super::assemble::window_start_event;

/// 决策产物：管道输入（材料项克隆自触发瞬间的会话日志——事件流
/// append-only，区间恒定有效）。
pub(crate) struct TaskPayload {
    pub session_id: String,
    pub workspace: String,
    pub items: Vec<MaterialItem>,
    pub users_total: u64,
    pub reason: OrganizeReason,
}

impl super::Organizer {
    /// 自动触发决策（轮次收束后、持有会话锁的同步上下文调用）。写路径：
    /// 反弹账本落投影文件；返回 None = 不整理。
    pub(crate) fn plan_auto(&self, session: &Session, session_id: &str) -> Option<TaskPayload> {
        let mem = self.settings.memory();
        let dev = self.settings.developer();
        let workspace = (self.workspace_of)(session_id).unwrap_or_default();
        let mut tree = self.load_tree(&workspace, session_id);
        // Q3b（v2.187）：越界游标当场钳回并落盘，放在**最前面**让三个分支（溢出 /
        // 熔断 / 常规）共用同一个合法起点——溢出支那句裸切片 `&events[chaptered_until..start]`
        // 在游标越界时本来会 panic，钳过之后不会。
        if Self::clamp_cursor(&mut tree, session.events().len()) {
            self.tree_store.save(&tree).ok();
        }
        // M4 模式 B 溢出整理（§3.38②）：固定窗口档，窗口滑出的
        // 孤儿轮（chaptered_until < 窗口起点）既不在尾巴也不在目录，必须及时
        // 归章节——例行维护：**先于熔断判定**（熔断暂停的是阈值/增量烧钱信号，
        // 不是固定节奏）且**不记反弹**（管道按 reason 跳过 note_organized）。
        if self.settings.tail_config().0 == TailStrategy::Window && tree.pending.is_none() {
            let start = window_start_event(session.events(), mem.tail_window_turns);
            if tree.chaptered_until < start && start > 0 {
                let events = session.events();
                let items = build_items(&events[tree.chaptered_until..start], tree.chaptered_until);
                if !items.is_empty() {
                    let users_total = memory_tree::user_count(session.events());
                    return Some(TaskPayload {
                        session_id: session_id.to_string(),
                        workspace,
                        items,
                        users_total,
                        reason: OrganizeReason::WindowOverflow,
                    });
                }
            }
        }
        if tree.paused_auto {
            // 熔断暂停期间连「待整理」标记都不记：§8.4 立的是「补跑不是绕过熔断的
            // 后门」，记了标记就等于给暂停中的会话排了一次自动续跑。
            return None;
        }
        if tree.pending.is_some() {
            // **在途期不丢触发**（《Context Budget Closure》§7.1 · 队列 Q6）。这句原来是
            // `if tree.paused_auto || tree.pending.is_some() { return None; }` 的一半，
            // 等于「整理在跑的最坏 240s 里新到的触发直接蒸发——不排队、不记账、不补跑」。
            // 现在命中判定就把 `organize_needed` 记进投影，任务收束后立即补跑（task.rs）。
            //
            // ⚠ 判定**复用同一个纯函数**而不是照稿内 §8.2 的措辞在这里另写一句
            // 「anchor 超阈值」：那句话只覆盖 `Batch` 档，`IncrementalChunk` 档的触发
            // 条件是尾巴增量 token，自造第二套口径正是稿 §11 点名的「阈值口径不一致」
            // 那个坑（变异 M7 打的就是"这里退成只看 anchor"这一刀）。
            let (_, inp) = self.trigger_input(session, &tree);
            if should_auto_organize(&inp).is_some() {
                tree.organize_needed = true;
                self.tree_store.save(&tree).ok();
            }
            return None;
        }
        // ⚠ **尾巴切片**而不是全量事件（Q3b 主修，见 `Organizer::tail_material`）：
        // 这句原来写的是 `build_items(session.events(), tree.chaptered_until)`，等于
        // 「把整段对话当未整理的尾巴」——第二次整理实测 items=7、`event` 区间 [6,12]、
        // 收束后游标 13（会话只有 7 条事件）。附带后果是 `tail_tokens` / `tail_events`
        // 两项读数失实 ⇒ `should_auto_organize` 的阈值与增量档判定跟着失实。
        let (items, inp) = self.trigger_input(session, &tree);
        let reason = should_auto_organize(&inp)?;
        // 反弹熔断（§5.5）：连续 m 次反弹 → 暂停本会话自动整理并跳过本次
        let users_total = memory_tree::user_count(session.events());
        if tree.on_auto_trigger(users_total, dev.organize_rebound_k, dev.organize_rebound_m)
            == ReboundDecision::Pause
        {
            tracing::warn!(session = %session_id, "整理反弹熔断：暂停本会话自动整理（/整理 不受限）");
            self.tree_store.save(&tree).ok();
            // plan_auto 在 paused_auto 已置位时早退，走到这里即翻转瞬间
            self.notify_paused(
                session_id,
                tree.rebound.count,
                dev.organize_rebound_k,
                dev.organize_rebound_m,
            );
            return None;
        }
        self.tree_store.save(&tree).ok();
        Some(TaskPayload {
            session_id: session_id.to_string(),
            workspace,
            items,
            users_total,
            reason,
        })
    }

    /// 触发判定的输入装配（队列 Q6 收成一处）：**在途支与常规支共用同一个**
    /// `(材料项, TriggerInput)`——稿 §11 立的纪律是「整理用绝对 token、自动收起用
    /// 百分比，改一处要两处对齐」，同一把尺子写两遍就是它的反面。常规支拿 items 进
    /// `TaskPayload`，在途支只取判定结果（items 传出去也没人用）。
    ///
    /// ⚠ 队列 Q8b 起可见面由私有抬到 `pub(super)`：**第三个调用方是 `pressure.rs`
    /// 的轮内泄压**，它必须吃同一个 `TriggerInput` 才能保证「轮末该整」与「轮内该切」
    /// 是同一个判定（另写一套 = 批 Q6 变异 M7 打的那种第二把尺）。抬一级不是放宽到
    /// 全 crate：`pub(super)` = `organize` 及其后代，跨出 organize 仍然不可见。
    pub(super) fn trigger_input(
        &self,
        session: &Session,
        tree: &MemoryTree,
    ) -> (Vec<MaterialItem>, TriggerInput) {
        let mem = self.settings.memory();
        let (tail, base) = Self::tail_material(session.events(), tree.chaptered_until);
        let items = build_items(tail, base);
        // 尾巴 token 只许一个算法（队列 Q7）：`material::tail_tokens` 此前是"生产零调用点
        // 的死函数"，而这里与 `organize/debug.rs` 各手算了一遍 `items.iter().map(|i| i.tokens).sum()`
        // ⇒ 同一读数三个出处。处置不是删死函数，而是把它**救活成唯一出处**，两处手算改调它——
        // 稿 §11「改一处要两处对齐」的纪律靠结构而不是靠人记。
        let tail_tokens = memory_tree::material::tail_tokens(&items);
        // 先取长度再移交 `items`：结构体字面量里 `items` 已经是移动进返回值的第一位，
        // 同一表达式里再 `items.len()` 就是 use-after-move（E0382，本批编译期抓到的）。
        let tail_events = items.len();
        let (anchor_tokens, _) = crate::context_budget::anchor(session);
        (
            items,
            TriggerInput {
                anchor_tokens,
                threshold_tokens: self.budget_threshold(&tree.session_id),
                pacing: Pacing::parse(&mem.organize_pacing),
                chunk_tokens: mem.organize_chunk_tokens,
                tail_tokens,
                tail_events,
            },
        )
    }

    /// 手动 `/整理`（不受熔断限制且复位熔断态，§3.36①）。
    pub(crate) fn plan_manual(
        &self,
        session: &Session,
        session_id: &str,
    ) -> Result<TaskPayload, String> {
        let workspace = (self.workspace_of)(session_id).unwrap_or_default();
        let mut tree = self.load_tree(&workspace, session_id);
        if tree.pending.is_some() {
            return Err("已有整理任务在途".into());
        }
        // 同 `plan_auto`：越界游标先钳回并落盘（`/整理` 是用户亲手发起的，损坏更该
        // 在这一刀被修掉，而不是等下一次自动触发）
        if Self::clamp_cursor(&mut tree, session.events().len()) {
            self.tree_store.save(&tree).ok();
        }
        // ⚠ **尾巴切片**而不是全量事件（Q3b 主修，见 `Organizer::tail_material`）
        let (tail, base) = Self::tail_material(session.events(), tree.chaptered_until);
        let items = build_items(tail, base);
        if items.is_empty() {
            return Err("没有可整理的尾巴".into());
        }
        let users_total = memory_tree::user_count(session.events());
        tree.mark_manual(users_total);
        self.tree_store.save(&tree).ok();
        Ok(TaskPayload {
            session_id: session_id.to_string(),
            workspace,
            items,
            users_total,
            reason: OrganizeReason::Manual,
        })
    }
}
