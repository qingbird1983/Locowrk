//! 摘要补写通路（真机「整理跟不上」的第二半 · 队列 Q8c）。
//!
//! Q8b 把轮内泄压做成了「等一次机械切」，并如实写下后半句「摘要后台补」**当时
//! 没做**：规则切分产出的章节有标题、有区间，摘要却是一句
//! 「（规则切分）本章节涵盖事件 X–Y」。记录员在轮末看到的是**游标之后的新尾巴**，
//! 不会回头补已经归档的那截 ⇒ 泄压用得越多，目录里留下的机械摘要就越多。
//! 本簇就是那条"回头补"的通路。
//!
//! 四条刻意的形状，每条都有判据钉着（理由写在对应位置）：
//!
//! ① **只换正文**：落笔经 `MemoryTree::apply_summaries`，它只写 `summary`。
//!    章节的 `chapter_id` 与 `title` 一个字都不动——请求组装的目录行字节形是
//!    `\n- {id}: {title}`，标题一动就打断前缀缓存（`KV Prefix Cache Design v2`
//!    §…；端到端判据 `backfill_keeps_the_injected_directory_byte_identical`）。
//! ② **复用同一把在途锁**（`Organizer::inflight`）：**不新增并发面**（用户
//!    2026-10-13 对补跑给的同一口径在这里继续兑现）。补写与整理同一条链串行，
//!    抢不到锁就放弃——那条在跑的整理链收束时会自己补（`run_task` 末尾）。
//! ③ **不打扰用户**：不发 `notify_done`、不记 `OrganizeRecord` 台账（补写不是
//!    一次整理：不动游标、不改章节数，塞进台账会让"整理次数"读数虚增）。
//!    界面看到的仍是 Q8b 那句「摘要从简」，只是过了一会儿那句机械说明变成了
//!    真摘要——人读层 tree.md 跟着同步。
//! ④ **0 档零 IO**：`organize_summary_chapters == 0` 时在**任何**读盘之前返回
//!    （与泄压 0 档"连投影目录都不创建"同一口径，队列 §6 第 12 条）。

use std::sync::Arc;
use std::time::Duration;

use memory_tree::{
    build_items, family_of, parse_summaries, summarizer_prompt, transcript, ChapterSummary,
};

use super::Organizer;

impl Organizer {
    /// 轮末排一次补写（`after_turn` 里「这一轮判下来不该整理」那一支调用）。
    ///
    /// fire-and-forget 与 `spawn` 同形，但**先去重再跑**：抢不到 `inflight` 就
    /// 直接退出，等于把"同一会话同时只有一条写路径"这条单写者纪律（§16）
    /// 原样交给锁去保证，本函数不写第二套互斥。
    pub(super) fn schedule_summary_backfill(self: &Arc<Self>, session_id: &str) {
        let dev = self.settings.developer();
        if dev.organize_summary_chapters == 0 {
            return; // 关掉：连 spawn 都不发生（不占线程、不读盘）
        }
        let this = Arc::clone(self);
        let session_id = session_id.to_string();
        tauri::async_runtime::spawn(async move {
            if !this
                .inflight
                .lock()
                .expect("inflight poisoned")
                .insert(session_id.clone())
            {
                return; // 整理链正在跑，它收束时会补
            }
            let workspace = (this.workspace_of)(&session_id).unwrap_or_default();
            this.backfill_summaries(&workspace, &session_id).await;
            this.inflight
                .lock()
                .expect("inflight poisoned")
                .remove(&session_id);
        });
    }

    /// 补写本体。**调用方必须已持有该会话的 `inflight` 锁**（两条入口都保证：
    /// 本簇的 `schedule_summary_backfill` 先抢后进，`run_task` 从头持有到尾）。
    /// 任何一步不成立就如实 warn 后退出；失败的那批章节靠 `mark_summary_attempts`
    /// 退出候选池，不靠"下次也许就好了"的乐观重试。
    pub(super) async fn backfill_summaries(self: &Arc<Self>, workspace: &str, session_id: &str) {
        let dev = self.settings.developer();
        if dev.organize_summary_chapters == 0 {
            return; // 0 档在任何读盘之前退出（判据见模块头注④）
        }
        let tree = self.load_tree(workspace, session_id);
        let ids = tree.summary_candidates(
            dev.organize_summary_chapters as usize,
            dev.organize_summary_attempts,
        );
        if ids.is_empty() {
            return; // 绝大多数轮次走这里：无债 ⇒ 不重读会话、不发请求
        }
        // 材料从**磁盘日志**重读（与补跑、崩溃恢复同一把钥匙，队列 Q6 立的口径）。
        // ⚠ 如实边界：桌面是每 step 批量落盘，磁盘态最多落后内存半轮 ⇒ 章节区间
        // 若正好压在末尾那半轮上，这里可能取到较短的一段（方向保守，不丢历史）。
        let Some(session) = self.session_of(session_id) else {
            tracing::warn!(session = %session_id, chapters = ids.len(), "摘要补写放弃：会话日志不可读（已删或定位失败）");
            return;
        };
        let events = session.events();
        let mut user = String::new();
        let mut asked: Vec<String> = Vec::new();
        for id in &ids {
            let Some(chapter) = tree.chapters.get(id) else {
                continue;
            };
            // 区间是**绝对事件位**（`draft_from_items` 落的原值，泄压支与降级支
            // 同一条出稿路）。钳到日志范围内，越界一截就当没有。
            let from = chapter.event_range.0;
            let to = chapter.event_range.1.min(events.len().saturating_sub(1));
            if from > to || from >= events.len() {
                continue;
            }
            let items = build_items(&events[from..=to], from);
            if items.is_empty() {
                continue; // 整章都是过程事件：没东西可写，别为它烧一次调用
            }
            user.push_str(&format!("[章节 {id} · 事件 {from}–{to}]\n"));
            user.push_str(&transcript(&items));
            user.push('\n');
            asked.push(id.clone());
        }
        if asked.is_empty() {
            return; // 候选全取不到材料：不落 attempts（它们不该被计成"补写失败"）
        }

        let (model, model_override) = self.manager_model_pair();
        let system = summarizer_prompt(family_of(&model));
        let timeout = Duration::from_secs(dev.memory_manager_timeout_secs.max(1));
        let mut last_err = String::new();
        for _attempt in 0..=dev.memory_manager_max_retries {
            // 一次带超时的调用（超时口径与整理同一个数，见 `manager_call_once` 注释）
            let called = tokio::time::timeout(
                timeout,
                self.client
                    .complete(system, &user, model_override.as_deref()),
            )
            .await;
            let outcome = match called {
                Err(_) => Err("记忆管理器超时".to_string()),
                Ok(Err(e)) => Err(e),
                Ok(Ok((raw, _))) => parse_summaries(&raw, &asked).map_err(|e| e.to_string()),
            };
            match outcome {
                Ok(summaries) if !summaries.is_empty() => {
                    if self.write_summaries(workspace, session_id, &summaries) {
                        return;
                    }
                    return; // 写盘失败已在下面 warn，这里不再烧第二次调用
                }
                Ok(_) => last_err = "输出里没有能对上的章节".into(),
                Err(e) => last_err = e,
            }
        }
        // 整串尝试都没产出：把这批章节记一笔，它们下次就不再入选（否则每轮重烧）。
        tracing::warn!(
            session = %session_id,
            chapters = asked.len(),
            "摘要补写未收束（{last_err}）：这批章节计入尝试次数，到上限即不再重挑"
        );
        let refs: Vec<&str> = asked.iter().map(String::as_str).collect();
        let mut tree = self.load_tree(workspace, session_id);
        if tree.mark_summary_attempts(&refs) == 0 {
            return; // 章节已被删/重建：无需落盘
        }
        if self.tree_store.save(&tree).is_err() {
            tracing::warn!(session = %session_id, "摘要补写计数落盘失败（下次仍会重挑这批章节）");
        }
    }

    /// 落笔：重载最新投影（单写者补漏，形状照 `pipeline`——模型调用期间收起/整理
    /// 可以落盘，用旧快照回写会抹掉别人的改动）→ `apply_summaries` → 落盘 →
    /// 同步人读层 tree.md。返回 `false` = 投影写不进去（磁盘仍是机械摘要）。
    ///
    /// ⚠ 这里**不发 `notify_done`、不记 `OrganizeRecord`**（模块头注③）：判据
    /// `backfill_never_notifies_nor_writes_history` 钉的就是本簇这两个调用点为 0。
    fn write_summaries(
        self: &Arc<Self>,
        workspace: &str,
        session_id: &str,
        items: &[ChapterSummary],
    ) -> bool {
        let mut tree = self.load_tree(workspace, session_id);
        let applied = tree.apply_summaries(items);
        if applied == 0 {
            // 模型给的全是树里已不存在/不该写的章节：什么都不动（连 tree.md
            // 重建都不做——那会是一次无意义的全项目读写）
            tracing::warn!(session = %session_id, "摘要补写落空：产出与投影对不上号");
            return true;
        }
        if self.tree_store.save(&tree).is_err() {
            tracing::warn!(session = %session_id, applied, "摘要补写落盘失败（磁盘仍是机械摘要）");
            return false;
        }
        self.sync_tree_md(workspace);
        tracing::info!(session = %session_id, applied, "摘要补写收束：规则切分章节的 L1 已换成模型摘要（目录与游标未动）");
        true
    }
}
