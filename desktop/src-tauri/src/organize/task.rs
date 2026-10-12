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
        // 超时 + 重试（§5.4）
        let model = mem
            .memory_manager_model
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| self.settings.llm().model);
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
        let model_override = mem
            .memory_manager_model
            .clone()
            .filter(|s| !s.trim().is_empty());
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
