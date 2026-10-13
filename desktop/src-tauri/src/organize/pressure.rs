//! 轮内水位泄压（真机痛点 · 队列 Q8b）。
//!
//! 要治的是什么：常规整理挂在**轮次收束之后**（`after_turn`），而一轮之内
//! 内核可以连跑 N 个 step、每个 step 都把整段历史重发一遍。真机读数 = 会话
//! `desktop-1791871305177` 的 `context_usage.used = 633,122` / 阈值 100,000 /
//! 窗口 1,048,576——轮末那条通路再快也追不上「本轮第 3 个 step 就要发出去」
//! 的这 63 万 token，用户看到的就是一步卡死或直接超限报错。
//!
//! 裁定（用户 2026-10-13）= **等一次机械切，摘要后台补**：轮内命中水位时同步
//! 做一次规则切分（`rule_split_drafts`，纯 CPU + 一次投影写盘，毫秒级、
//! **不调模型**），把本轮之前的历史先归档成章节。⚠ **后半句「摘要后台补」本批没做**
//! （= Q8c）：游标一推，记录员那条通路在轮末看到的是"游标之后的新尾巴"，
//! **不会回头补这一截**——补写需要一条"保留 `chapter_id` 与 title、只换正文"的
//! 替换通路（目录行字节形是 `- [id]: [title]`，只换正文才不破前缀缓存）。
//! 所以本批的文案与日志一律写「摘要从简」，不写「稍后补」。这一刀换到的东西很具体：
//! **下一次组装请求时尾巴变短**。
//!
//! 三个刻意的形状：
//! ① **判定与常规整理共用 `trigger_input` + `should_auto_organize`**（同一把
//! 尺，批 Q6/Q7 立的口径）。这里变的只有**检查时机**（从轮末挪进步间边界）
//! 和**多一条水位条件**，不另写第二套阈值——自造口径正是 CB §11 点名的坑。
//! ② **切口的上界是本轮开始时的日志长度**（`turn_start_event`），本轮的事件
//! 一条都不许进章节：正在写的东西被归档走 = 模型下一 step 看不见自己刚说的
//! 话，那比超限更坏。
//! ③ **在途与熔断期让位**（`pending.is_some() || paused_auto` 直接返回）。
//! 泄压也是写路径，而单写者纪律（§16）不容两条通路同时推游标；代价如实记在
//! 边界里：整理在途的最坏 90s（批 Q6 已从 240s 压到 90s）内水位再高也不切。

use session_store::Session;

use memory_tree::{rule_split_drafts, OrganizeReason};

use crate::timeutil::now_ms;

/// 泄压结果（观测与测试面读的三样：切了几章、游标推进到哪、为什么没切）。
///
/// 类型留在本簇、**不在 `organize/mod.rs` 再导出**：`organize` 是 lib.rs 里的
/// 私有模块，`pub use pressure::Release;` 既出不了 crate、lib 侧又没有命名点
/// （runner 只调 `.chapters()` 不命名类型）⇒ clippy 把它判成一条没用到的导入
/// 而 BLOCK（本批 clippy 首跑就红在这里，两种写法都试过：`pub(crate) use` 与
/// `pub use` 同样红）。命名方是 organize 的后代（tests）与同模块的 runner，
/// 直连簇路径就够。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Release {
    /// 没切，`reason` = 如实的退出理由（日志与判据共用同一句，不靠"返回 0"
    /// 反推是哪一步拦的——十条早退支都返回 0 时，测试就无法区分"水位没到"
    /// 与"上限已用完"）。
    Skip(&'static str),
    /// 切了：`chapters` = 新落章节数，`until` = 推进后的游标（= 最后一条被归档
    /// 事件的**下一位**，恒 ≤ `turn_start_event`——本轮起点只是刀口的上界，
    /// 规则切分按 `rule_chunk_tokens` 分档，末档可能没切到上界）。
    Cut { chapters: usize, until: usize },
}

impl Release {
    /// 本次切掉的章节数（调用方 runner 只关心这个数）。
    pub fn chapters(&self) -> usize {
        match self {
            Release::Skip(_) => 0,
            Release::Cut { chapters, .. } => *chapters,
        }
    }
}

impl super::Organizer {
    /// 轮内水位泄压（step 边界、**持有会话锁的同步上下文**调用）。
    ///
    /// `window_tokens` = 会话生效窗口（`UsagePolicy` 那份，0 = 未知 ⇒ 不猜
    /// 口径，形状照 `auto_collapse`）；`released` = 本轮已经泄压几次（计数归
    /// 调用方持有，不落盘——它的生命周期就是一轮）。
    ///
    /// 写路径：投影落盘 + tree.md 同步 + 一条 `lw://organize-done`
    /// （`reason = "pressure_release"`，前端按它换文案）。
    pub(crate) fn release_pressure(
        &self,
        session: &Session,
        session_id: &str,
        window_tokens: u64,
        released: u32,
        turn_start_event: usize,
    ) -> Release {
        let dev = self.settings.developer();
        if released >= dev.organize_max_release {
            return Release::Skip("off-or-cap");
        }
        if window_tokens == 0 {
            return Release::Skip("window-unknown");
        }
        // ⚠ 这一刀**每个 step 都会问一次**，所以贵的判定放在便宜的水位之后：
        // 水位只读日志尾部的 `LlmUsage`（`anchor` 反向扫，均摊 O(1)），没到水位
        // 就一行投影 IO 都不做。两个条件是合取（&&），顺序不改变结论——变的只是
        // "哪个先拦下来"，而拦下来的理由仍然逐条如实返回。
        // 锚点 = `Organizer::anchor`（provider 真实 input，v2.138「进度条 =
        // 触发 = 自动收起」那把尺），比较式照 `auto_collapse` 的
        // `anchor×100 ≥ window×pct`——同一个乘比式不留两处（批 Q7 §11 口径）。
        let (anchor, _) = Self::anchor(session);
        if anchor.saturating_mul(100)
            < window_tokens.saturating_mul(dev.organize_release_watermark_pct as u64)
        {
            return Release::Skip("below-watermark");
        }
        let workspace = (self.workspace_of)(session_id).unwrap_or_default();
        let mut tree = self.load_tree(&workspace, session_id);
        // 游标越界先钳回并落盘（与 `plan_auto` 同一句、同一位置理由）：
        // 下面的切片要拿 `chaptered_until` 当起点。
        if Self::clamp_cursor(&mut tree, session.events().len()) {
            self.tree_store.save(&tree).ok();
        }
        if tree.pending.is_some() {
            return Release::Skip("in-flight");
        }
        if tree.paused_auto {
            return Release::Skip("circuit-broken");
        }
        // ① 该不该整理——**与轮末那条通路同一个纯函数**。`trigger_input` 装配
        // 的 items 在这里用不上（下一句就只要判定结果），但它与 `TriggerInput`
        // 是同一个函数产出的两样东西，拆开装配 = 重写第二把尺（批 Q6 的靶）。
        let (_judged_items, inp) = self.trigger_input(session, &tree);
        if memory_tree::should_auto_organize(&inp).is_none() {
            return Release::Skip("not-due");
        }
        // ② 刀口：只切「本轮之前」那截。`trigger_input` 的 items 含本轮（它
        // 服务的是判定，判定的尾巴定义就是"游标起的全部"），所以这里**第二次**
        // 切片不是重复劳动而是两件不同的事：尺子量全尾巴，刀只切本轮之前的部分。
        let events = session.events();
        let from = tree.chaptered_until;
        let to_exclusive = turn_start_event.min(events.len());
        if to_exclusive <= from {
            return Release::Skip("nothing-before-this-turn");
        }
        let items = memory_tree::build_items(&events[from..to_exclusive], from);
        if items.is_empty() {
            return Release::Skip("empty-slice");
        }
        let drafts = rule_split_drafts(&items, dev.rule_chunk_tokens);
        if drafts.is_empty() {
            return Release::Skip("no-drafts");
        }
        let first = items.first().unwrap().event;
        let last = items.last().unwrap().event;
        let chapters = tree.build_chapters(&drafts, "rule", now_ms());
        let n = chapters.len();
        // 与降级支同一形状：`apply_organized` 撤占位（此处已判空）、落真章节、
        // 推游标、记台账。summary = None（宁缺勿假，规则切分不编总结）。
        tree.apply_organized(
            chapters,
            last,
            Self::record(
                OrganizeReason::PressureRelease,
                "rule",
                "ok",
                first,
                last,
                n,
                memory_tree::MemoryManagerUsage::default(),
                0,
            ),
            None,
        );
        if self.tree_store.save(&tree).is_err() {
            // 投影写不进去就当这次没发生：游标只在内存里动过，磁盘仍是旧态，
            // 下一 step 组装读盘拿到的还是长尾巴——**不许**在这种状态下告知
            // 用户"已收起"（那会让界面与磁盘各说一套）。
            return Release::Skip("save-failed");
        }
        self.sync_tree_md(&workspace);
        // 告知复用 `notify_done`：它的载荷、`fragments` 置位（下一轮开场告诉
        // 模型"历史去哪了、怎么找回"）与三路同源语义都是现成的，泄压只是
        // 多一条 reason，不该有第二套告知面。
        self.notify_done(
            session_id,
            memory_tree::user_count(events),
            n,
            "rule",
            "ok",
            OrganizeReason::PressureRelease,
        );
        tracing::info!(
            session = %session_id,
            chapters = n,
            anchor,
            window = window_tokens,
            pct = dev.organize_release_watermark_pct,
            "轮内水位泄压：规则切分收起本轮之前的历史（不调模型，摘要从简 · 补写见 Q8c）"
        );
        Release::Cut {
            chapters: n,
            until: tree.chaptered_until,
        }
    }
}
