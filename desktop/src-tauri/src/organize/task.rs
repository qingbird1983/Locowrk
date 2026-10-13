//! 异步管道面（§5.3/§5.4）：`spawn`（后台 fire-and-forget，不阻塞会话返回值）→
//! `run_task`（进程内在途去重 + 收束如实记日志）→ `pipeline`（占位章节先落盘 →
//! 记忆管理器超时重试 → 输出校验 → 不合规降级规则切分 → 落树 + 事实卡落库 +
//! 同步 tree.md + 通知）。占位先落盘的意义 = 崩在任意点重启都能重投（§5.3）。
//!
//! `run_task` 原为私有方法，`organize/tests.rs` 直接驱动它跑全链路 ⇒ 拆簇后
//! `pub(super)`（可见面不变）；`pipeline` 只被同簇的 `run_task` 调用，**保持私有**。

use std::sync::Arc;
use std::time::Duration;

use memory_tree::{
    family_of, manager_prompt, parse_drafts, rule_split_drafts, MemoryManagerUsage, OrganizeReason,
};

use crate::timeutil::now_ms;

use super::{elapsed_ms, facts, TaskPayload};

/// 管道收束台账（run_task 返回值，观测日志用）。
type Outcome = (String, String, usize, MemoryManagerUsage);

impl super::Organizer {
    pub(super) fn spawn(self: &Arc<Self>, task: TaskPayload) {
        let this = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            this.run_task(task).await;
        });
    }

    /// 管道入口（进程内在途去重后跑 `pipeline`，收束如实记日志）。
    pub(super) async fn run_task(self: Arc<Self>, task: TaskPayload) {
        if !self
            .inflight
            .lock()
            .expect("inflight poisoned")
            .insert(task.session_id.clone())
        {
            return; // 同会话已在整理
        }
        let started = std::time::Instant::now();
        let outcome = self.pipeline(&task, started).await;
        // **收束即补跑**（《Context Budget Closure》§7.1② · 队列 Q6）：这一刀必须落在
        // `inflight` **释放之前**——去重锁还在手里，"补跑"与"下一轮 after_turn 又 spawn
        // 一个"天然互斥，稿内要求的「走去重路径、不新增并发面」就是这么实现的
        // （变异 M4 打的是"把这句挪到 remove 之后"，判据钉的是源码顺序而不是线程时序）。
        self.rerun_while_marked(&task.workspace, &task.session_id)
            .await;
        // **收束即补写**（队列 Q8c）：这一句同样必须落在 `inflight` **释放之前**，
        // 理由与上一句同族——锁还在手里，补写与「下一轮 `after_turn` 又排一条
        // `schedule_summary_backfill`」天然互斥，不新增并发面（用户 2026-10-13 对
        // 补跑给的口径继续兑现）。刚收束的若是记录员通路，它产出的章节
        // `source="recorder"` 不欠债（候选池为空，一句早退）；欠债的是本轮的泄压
        // 章节与这条降级支自己。
        self.backfill_summaries(&task.workspace, &task.session_id)
            .await;
        self.inflight
            .lock()
            .expect("inflight poisoned")
            .remove(&task.session_id);
        match outcome {
            Ok((mode, status, chapters, usage)) => tracing::info!(
                session = %task.session_id, %mode, %status, chapters,
                input = usage.input, output = usage.output,
                ms = elapsed_ms(&started), "整理收束"
            ),
            Err(e) => tracing::warn!(session = %task.session_id, "整理未收束: {e}"),
        }
    }

    /// 标记驱动的续跑循环（§7.1② + §12.2 的连续失败熔断）。三个刻意的形状：
    ///
    /// ① **串行而不是 spawn**：与首跑同一条 await 链、`inflight` 在手，测试可以
    ///    await 到终态再断言。历批 Q5 刚写过「并发用例的证据强度依赖并发压力」⇒
    ///    能做成确定性的就不要做成竞态。
    /// ② **材料从磁盘会话日志重读**（`Organizer::session_of`，与崩溃恢复同一把钥匙）：
    ///    续跑发生在后台任务里，手上没有 `&Session`。⚠ 如实边界：桌面是**每 step 批量
    ///    落盘**，磁盘态最多落后内存半轮 ⇒ 续跑判定读到的 anchor 只可能**偏小**，
    ///    方向是保守的（宁可少补一次，不会凭空多烧一次整理）。
    /// ③ **上限复位条件不是"时间过去了"**：`rerun_streak` 只在**新的自然触发**
    ///    （`after_turn` 那条路径）清零，不在本函数里清——否则补跑自己把自己的账
    ///    抹掉，上限形同虚设（变异 M3 打的就是这一刀）。到上限时**保留标记**：
    ///    熔断只该挡住"这一串续跑"，不该把已记账的触发再丢掉。
    async fn rerun_while_marked(self: &Arc<Self>, workspace: &str, session_id: &str) {
        let limit = self.settings.developer().organize_max_rerun;
        loop {
            let mut tree = self.load_tree(workspace, session_id);
            if !tree.organize_needed {
                return;
            }
            if tree.rerun_streak >= limit {
                tracing::warn!(
                    session = %session_id, streak = tree.rerun_streak, limit,
                    "整理连续补跑已达上限：本轮不再补跑（标记保留，等新触发的自然轮次）"
                );
                return;
            }
            tree.organize_needed = false;
            tree.rerun_streak += 1;
            if self.tree_store.save(&tree).is_err() {
                return; // 投影写不进去就别再烧一次整理
            }
            let Some(session) = self.session_of(session_id) else {
                tracing::warn!(session = %session_id, "补跑放弃：会话日志不可读（已删或定位失败）");
                return;
            };
            // 判定与首跑**共用 `plan_auto`**（同一个纯函数、同一条溢出/熔断顺序），
            // 这里不另写第二套阈值口径。
            let Some(task) = self.plan_auto(&session, session_id) else {
                continue; // 这一轮判下来不该整理（标记已消费，下一句就退出）
            };
            let started = std::time::Instant::now();
            let streak = tree.rerun_streak;
            match self.pipeline(&task, started).await {
                Ok((mode, status, chapters, usage)) => tracing::info!(
                    session = %session_id, %mode, %status, chapters, streak,
                    input = usage.input, output = usage.output,
                    ms = elapsed_ms(&started), "整理补跑收束"
                ),
                Err(e) => tracing::warn!(session = %session_id, streak, "整理补跑未收束: {e}"),
            }
        }
    }

    /// 管道本体：占位章节持久化 → 记忆管理器（超时 × 重试）→ 校验 → 降级规则
    /// 切分 → 落树。占位先落盘（崩在任意点 = 重启重投，§5.3）。
    async fn pipeline(
        self: &Arc<Self>,
        task: &TaskPayload,
        started: std::time::Instant,
    ) -> Result<Outcome, String> {
        let mem = self.settings.memory();
        let dev = self.settings.developer();
        let mut tree = self.load_tree(&task.workspace, &task.session_id);
        let from = task.items.first().unwrap().event;
        let to = task.items.last().unwrap().event;
        // pending 在位 = 本次运行正是它的执行者（崩溃重投，§5.3）——直接
        // 续跑；否则登记占位章节并落盘（此后任意点崩溃都可重投）。
        if tree.pending.is_none() {
            if tree.insert_placeholder(from, to, now_ms()).is_none() {
                return Err("占位登记失败（并发整理）".into());
            }
            self.tree_store.save(&tree)?;
        }

        // 记忆管理器调用：模型家族匹配提示词（§6.2；未配记忆管理器模型 = 主模型），
        // 超时 + 重试（§5.4）。模型对取自 `Organizer::manager_model_pair`（队列 Q8c
        // 收成一处：这两句原本在这里各写一遍，补写通路是第三个调用方）。
        let (model, model_override) = self.manager_model_pair();
        let system = manager_prompt(family_of(&model));
        // 事实卡（MR 稿 §5 提案 A · 队列 Q3 · v2.186）：**开关关掉时 user 段逐字节
        // 回到现役形状**——这条等价关系是判据 `facts_switch_off_…` 钉的对象，所以
        // 这里写成 `if` 分支而不是"永远拼一段可能为空的文本"。
        //
        // 要求文本进 **user 段**而不是 system：system 是 `include_str!` 的四份家族
        // 提示词，《Memory Tree Design v2》§6.5 立的是「角色提示词会话内逐字节
        // 稳定、无动态内容」；而"现有 key 清单"每次整理都不同（正是给模型查重用
        // 的，稿 §5 要求注入），塞进 system 既破那条纪律也打断记录员自己的前缀缓存。
        let mut user = memory_tree::transcript(&task.items);
        let fact_limit = if mem.fact_cards {
            mem.fact_card_max as usize
        } else {
            0
        };
        if fact_limit > 0 {
            user.push_str(&memory_tree::fact_request(
                &facts::existing_keys(&self.memory, &task.workspace),
                fact_limit,
            ));
        }
        let timeout = Duration::from_secs(dev.memory_manager_timeout_secs.max(1));
        let mut last_err = String::new();
        for _attempt in 0..=dev.memory_manager_max_retries {
            match tokio::time::timeout(
                timeout,
                self.client
                    .complete(system, &user, model_override.as_deref()),
            )
            .await
            {
                Err(_) => last_err = "记忆管理器超时".into(),
                Ok(Err(e)) => last_err = e,
                Ok(Ok((raw, usage))) => match parse_drafts(&raw, &task.items) {
                    Ok(drafts) => {
                        // 单写者补漏（走查实抓 F1 暴露的 M4 竞态）：收起可在
                        // 记忆管理器调用期间落盘——重载最新投影再产出，防旧快照
                        // 回写抹掉 collapsed_at（占位章节已持久化，重载即在）。
                        tree = self.load_tree(&task.workspace, &task.session_id);
                        let chapters = tree.build_chapters(&drafts, "recorder", now_ms());
                        let n = chapters.len();
                        // 轮级总结（Organize Summary Design v1 §3.4）：尾随
                        // 标记段，无 = None（成功无总结，目录照常）。
                        let run_sum = memory_tree::run_summary(&raw);
                        tree.apply_organized(
                            chapters,
                            to,
                            Self::record(
                                task.reason,
                                "recorder",
                                "ok",
                                from,
                                to,
                                n,
                                usage,
                                elapsed_ms(&started),
                            ),
                            run_sum,
                        );
                        // 溢出整理（模式 B 例行维护）不碰反弹基线——§5.5
                        // 只度量「整理后占用立刻再超」的烧钱信号
                        if !matches!(task.reason, OrganizeReason::WindowOverflow) {
                            tree.note_organized(task.users_total);
                        }
                        self.tree_store.save(&tree)?;
                        // 事实卡落库（整理副产物，稿 §5）：**放在主面已经安全之后**——
                        // 章节与总结此刻已落树并落盘，卡写失败只 warn 计数，不许把
                        // 已成功的整理拖倒（facts.rs 头注第 3 条）。`fact_limit`
                        // 为 0（开关关）时这里连解析都不做。
                        if fact_limit > 0 {
                            facts::persist(&self.memory, &task.workspace, &raw, fact_limit);
                        }
                        self.sync_tree_md(&task.workspace);
                        self.notify_done(
                            &task.session_id,
                            task.users_total,
                            n,
                            "recorder",
                            "ok",
                            task.reason,
                        );
                        return Ok(("recorder".into(), "ok".into(), n, usage));
                    }
                    Err(e) => {
                        last_err = format!("输出不合规: {e}");
                        tracing::warn!(session = %task.session_id, "{last_err}，重试或降级");
                    }
                },
            }
        }
        // 降级：规则切分（§5.4，复用 ctx-compact「切点落在 User 之前」纪律）。
        // 重载同上——降级路径同样不得用旧快照回写。
        tree = self.load_tree(&task.workspace, &task.session_id);
        let drafts = rule_split_drafts(&task.items, dev.rule_chunk_tokens);
        let chapters = tree.build_chapters(&drafts, "rule", now_ms());
        let n = chapters.len();
        tree.apply_organized(
            chapters,
            to,
            Self::record(
                task.reason,
                "rule",
                "degraded",
                from,
                to,
                n,
                MemoryManagerUsage::default(),
                elapsed_ms(&started),
            ),
            None, // 规则切分无总结（宁缺勿假，退化到目录行为）
        );
        // 降级支**不产事实卡**（稿 §5「降级语义：规则切分路径产不出就不产，诚实
        // 缺省，不硬造」）：这条支根本没有模型回复可解析，写 `persist` 调用点 = 0
        // 是判据 `rule_split_produces_no_fact_cards` 钉的形状。
        // 溢出整理不碰反弹基线（同上）
        if !matches!(task.reason, OrganizeReason::WindowOverflow) {
            tree.note_organized(task.users_total);
        }
        self.tree_store.save(&tree)?;
        self.sync_tree_md(&task.workspace);
        tracing::warn!(session = %task.session_id, "记忆管理器不可用（{last_err}），已降级规则切分");
        self.notify_done(
            &task.session_id,
            task.users_total,
            n,
            "rule",
            "degraded",
            task.reason,
        );
        Ok((
            "rule".into(),
            "degraded".into(),
            n,
            MemoryManagerUsage::default(),
        ))
    }
}
