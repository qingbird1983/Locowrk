//! **Memory-Inject 的检索源**（台账 T33）：`impl MemorySource for MemoryStore` 的
//! 全部方法（现役 `recall` / 缓存键 `scope_id` / 新鲜度指纹 `index_stamp` / 归属告知
//! `context_line` / 交接提醒段 `repo_context` / **两层索引** `global_index` 与
//! `project_index`）、组装点便利函数 `memory_source`，以及只被 `context_line` 用的
//! `today_local`——它的唯一调用点就在本片，所以跟着搬，不留第二个出处。
//!
//! 本片是 trait 实现，整块自包含（原文那对花括号就在此起此落，不需要补外壳）。用的都是
//! 别片已抬到 `pub(super)` 的入口（`open_conn` / `recall_quiet` / `log_inject_keys` 与
//! 那四个字段），加上原本就 `pub(crate)` 的 `md_index_body`；没有一处为"方便"而抬宽。
//!
//! ⚠ **S3 刀③b：学习档整族离场**。`recall_relevant`（trait 方法 + 这份宿主实现）、
//! `memory_learn/` 一族、设置页那格 `learnEnabled` 与四枚权重，按
//! `specs/10-context-injection-s3` 开工三问② 的裁定**同刀退役删除**——真源 = md 之后
//! "按影子库 `hit_count` 加权排序"是残留的第二真源，而它唯一的调用点（旧
//! `<persistent_memory>` 正文块）已在刀① 摘掉，留着跨三刀就是审计报告 H2 判过的
//! "存在但从未接线"那一格。**留下的东西**：`recall` 本体与它身后的 `recall_quiet`
//! 注入埋点——生产注入面（改吃两层索引）同样不再调它，这条口径欠账登记为 ⓠ，
//! 本刀不动它的形状（见 contract 侧那条同名注释）。

use std::sync::Arc;

use kernel_agent::{MemoryEntry, MemorySource};

use crate::timeutil::now_secs;

use super::store::MemoryStore;

/// 本地日期 ISO 形（`YYYY-MM-DD`，`<environment>` Today 行用）。chrono
/// 已在依赖树（tauri 传递引入，声明直依赖零新增编译成本）；取不到按
/// epoch UTC 如实（不发明时区）。
fn today_local() -> String {
    chrono::Local::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string()
}

/// Memory-Inject 的检索源（注册进 agent/request 决策环）。㉘·A·fix 另供两件：`scope_id`
/// （预取缓存键成分——归属变更自动重算）与 `context_line`（归属会话告知
/// 模型当前项目根，"看下这个项目"不再靠旧记忆猜路径）。
/// ⚠ v2.136 那句「注入只取全局层」管的是**正文通路**（`recall` 那条 top-K 全文注入）：
/// S3 刀① 之后进上下文的是**两层索引的指针行**（[`MemorySource::global_index`] +
/// [`MemorySource::project_index`]），正文一个字都不进 ⇒ 那句话描述的东西已经不存在，
/// 别照它推"项目层不进上下文"。队列 Q9 那条 `recall_relevant` 覆写（四信号加权 +
/// 命中写回）随刀③b 整族退役。
impl MemorySource for MemoryStore {
    fn recall(&self, session_id: Option<&str>, limit: usize) -> Vec<MemoryEntry> {
        // Q1（L0）：session_id 现在透传给 recall_quiet，但**只作观测字段**——
        // 召回恒取全局层（recall_quiet 头注的 v2.136 裁定），交付的条目集合
        // 与改前逐字节一致（等价判据在 memory/tests.rs）。
        self.recall_quiet(session_id, limit)
            .into_iter()
            .map(|r| MemoryEntry {
                key: r.key,
                value: r.value,
                tags: r.tags,
                updated_at: r.updated_at,
                // 只取全局层，层标恒 global（层标渲染在内核侧保留）
                scope: "global".into(),
            })
            .collect()
    }

    fn scope_id(&self, session_id: Option<&str>) -> String {
        // v2.136 起交接文件 mtime 一度退出缓存键（那时提醒段是静态文本、正文不注入，
        // 重算买不到任何东西）。**S3 刀② 把它接回来了**，但接在 `index_stamp` 那一段
        // 而不是这里——键位身份与新鲜度是两件事，混在一个返回值里就没法分别判读。
        self.scope_of_session(session_id)
    }

    /// S3 刀②：注入面三份输入的 mtime 指纹（`g` = 全局索引 / `p` = 项目索引 /
    /// `r` = 交接正文 `MEMORY.md`）。读不到记 0——"没这份文件"与"刚被改过"必须是
    /// 两个不同的串，否则第一条用例就能假绿。
    ///
    /// 三处路径**全部复用既有单点算式**（`memlayout::layers` / `memlayout::project_dir`
    /// / `Self::repo_root` + `repo_context::{DIR, HANDOFF}`）：这里再拼一遍 `global/`
    /// 或 `MEMORY.md` 字面量就是第二个出处，而 S1 那一整批就是在清这种重复。
    ///
    /// ⚠ 代价如实记在这里：`layers()` 会**枚举层目录**（不止 stat），所以这一档每 step
    /// 一次的开销是"读目录项"级。真成了热点，正解是往 `memlayout` 抬一个
    /// `global_index_path()`（新增 pub fn 不动既有签名），而不是在这里复制字面量。
    fn index_stamp(&self, session_id: Option<&str>) -> String {
        use plugin_api::memlayout::{layers, mtime_ms, project_dir};
        let root = self.files().root();
        let global = || {
            layers(root)
                .0
                .into_iter()
                .find(|layer| layer.hint.is_empty())
                .and_then(|layer| mtime_ms(&layer.index))
        };
        let project = || {
            let ws = self.scope_of_session(session_id);
            if ws.is_empty() {
                return None;
            }
            mtime_ms(&project_dir(root, &ws).ok()?.join("MEMORY.md"))
        };
        let handoff = || {
            let repo = self.repo_root(session_id)?;
            mtime_ms(
                &repo
                    .join(crate::repo_context::DIR)
                    .join(crate::repo_context::HANDOFF),
            )
        };
        format!(
            "g{}p{}r{}",
            global().unwrap_or(0),
            project().unwrap_or(0),
            handoff().unwrap_or(0)
        )
    }

    fn context_line(&self, session_id: Option<&str>) -> Option<String> {
        let ws = self.scope_of_session(session_id);
        if ws.is_empty() {
            return None;
        }
        // 提示词分层设计 §4.2/§5.3（v2.139 `<environment>` 内文）：项目根 +
        // 日期/平台 + 按需查路径（"list/read it first" 退役——「背景不是
        // 议程」）。日期/平台随块同吃会话预取 = 会话锁死（首请求定格，
        // 跨天不发提醒——桌面会话单日为主，宁缺勿假）。
        Some(format!(
            "Project workspace: {ws} — \"this project\" in the user's message \
             refers to this directory.\n\
             Today: {}. Platform: {}.\n\
             Look up project files when the request touches them; treat paths \
             recalled from memory as hints to verify, not facts.",
            today_local(),
            std::env::consts::OS
        ))
    }

    /// 交接段（v2.136 起有这一段；**S3 刀② 起注入真实正文**）：归属会话注入
    /// `<repo>/.Locowrk/MEMORY.md` 的**现状正文** + requirements 提醒行（方案稿 §F
    /// 那句"它就是为新会话一眼见现状而生的交接主入口"）。首次调用顺手引导建目录
    /// 与骨架（幂等，每仓库一次）；开关关 / 未归属 / 非目录 / 那份文件读不动
    /// = `None`（整段缺席，**不退回旧的静态提醒**）。
    /// 正文会随人编辑而变 ⇒ 它的 mtime 是 `index_stamp` 的第三档。
    fn repo_context(&self, session_id: Option<&str>) -> Option<String> {
        if !(self.repo_on)() {
            return None;
        }
        let repo = self.repo_root(session_id)?;
        {
            let mut set = self.repo_ensured.lock().expect("repo ensured poisoned");
            if set.insert(repo.clone()) {
                if let Err(e) = crate::repo_context::ensure_layout(&repo) {
                    tracing::warn!("交接目录引导失败 {}: {e}", repo.display());
                }
            }
        }
        crate::repo_context::build(&repo)
    }

    /// S3 刀①（方案稿 §A④「两个索引」里缺的那一个）：全局层记忆**索引**——
    /// `<数据根>/memory/global/MEMORY.md` 的指针行。它取代的是旧 `<persistent_memory>`
    /// 那块**正文** top-K：全局层从此也只出指针，与项目段同形、同吃一把钳。
    ///
    /// 层身份**不自己拼路径**：`global/` 这个目录名在 `memlayout` 里只有一处定义
    /// （`GLOBAL_DIR`），宿主再写一遍就是第二个出处——同一类坑 S1 刀②c-1 踩过
    /// （`projects/<可读名-哈希>` 曾被拼三遍）。所以这里从 [`plugin_api::memlayout::layers`]
    /// 的枚举结果里取"身份为空串的那一层"的索引文件，读不动那一层时它压根不在清单里
    /// ⇒ 整段如实缺席，与项目段"文件没指针行 = 没记过"同一个出口。
    ///
    /// 吃 `global_on` 那枚开关（§E 三枚行为开关之一）：关 = 该层不读、不注入。
    fn global_index(&self, _session_id: Option<&str>) -> Option<String> {
        if !(self.global_on)() {
            return None;
        }
        let root = self.files().root();
        let index = plugin_api::memlayout::layers(root)
            .0
            .into_iter()
            .find(|layer| layer.hint.is_empty())?
            .index;
        let body = std::fs::read_to_string(index).ok();
        // 陈旧标注的输入：全局层走 `facts_of(root, "")`（空键位 = 全局，与写侧同一条算式）。
        let ages = crate::memory_index::ages_of_layer(root, "").unwrap_or_default();
        crate::memory_index::from_md(
            body.as_deref(),
            &ages,
            now_secs(),
            crate::memory_index::LEAD_IN_GLOBAL,
        )
    }

    /// Q2（MR 稿 §4.1）：项目记忆**索引**（指针行，不含正文）。
    ///
    /// 未归属会话 = 没有"本项目"可言 ⇒ `None`（整节缺席）。S1 刀②b-3 把两档的关系
    /// 反了过来（md 里没指针行就是**这个项目确实没记过**，如实缺席），刀②c-1 把剩下
    /// 那档——"人类层没接线 ⇒ 去库里现合成"——连同 `index_rows` 一起删掉：真源 = 文件
    /// 之后，库里那份副本已经不是同一件事的答案了，而且现在也没有"没接线"这个状态。
    /// 取数失败同样 `None`，**绝不合成一条"看起来是空索引"的块**——那会把"读不到"
    /// 骗成"没东西可注入"。
    fn project_index(&self, session_id: Option<&str>) -> Option<String> {
        // 项目记忆层开关（S1 刀④b）：关 = 这一节整节缺席，与"未归属会话没有本项目"
        // 走的是同一个出口（`None` = 块不生成，不是生成一个空块）。
        // ⚠ 它**不**管 `context_line`：那行里是项目根与今天日期，不是记忆内容——
        // 把它一起关掉就是拿两个语义不同的东西共用一个开关。
        if !(self.project_on)() {
            return None;
        }
        let ws = self.scope_of_session(session_id);
        if ws.is_empty() {
            return None;
        }
        // 陈旧标注的输入 = 这一层「文件名主干 → updated_at」。取不到（算不出目录）
        // 就当没有 ages ⇒ 那一行不标注，**而不是整节缺席**：新鲜度是派生量，
        // 它缺席不该把索引本身一起带走。每 step 都会走到这里 ⇒ 不 warn（刷日志）。
        let ages = crate::memory_index::ages_of_layer(self.files().root(), &ws).unwrap_or_default();
        crate::memory_index::from_md(
            self.md_index_body(&ws).as_deref(),
            &ages,
            now_secs(),
            crate::memory_index::LEAD_IN_PROJECT,
        )
    }
}

/// 组装点便利：Arc 化并作为 trait 对象交给 MemoryInject。
pub fn memory_source(store: &Arc<MemoryStore>) -> Arc<dyn MemorySource> {
    Arc::clone(store) as Arc<dyn MemorySource>
}
