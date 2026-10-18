//! 记忆库的**读写面**（台账 T33）：管理页列举 `list`、编辑 `update_value`、写入
//! `write`（生产写路径三处共用这条 upsert）、删除 `delete`、清空 `clear_all`，加上
//! 四条 `shadow_*`——sqlite 影子写。
//!
//! S1 刀②c-1 把这里的**次序反了过来**：旧形状是"先写 sqlite（真源）、成功后同步 md，
//! md 失败只 warn"；现在是"**先落 md**（真源，失败 = 整个操作失败并上抛）、成功后再更新
//! sqlite 影子（失败只 warn）"。这不是措辞变化，是同一句 `if let Err(e) = …` 换了主语：
//! 掉哪一份会导致什么，从"镜像旧了"变成"这条记忆没写进去"。四条写路径的存在性判据
//! 因此全部改成**文件在不在**（`update_value` / `delete` 旧判据是 SQL 影响行数）。
//!
//! 五条读写全部不再问 settings `memory.files`：真源没有可退的第二处（句柄由数据根算出，
//! 见 `store.rs`）。剩下还会走到 SQL 的只有 `shadow_*` 与 `reconcile.rs` 那份直写面。
//!
//! ⚠ 登记不合并：`key`/`value` 上限校验这一句话在本片出现两次、在 `reconcile.rs` 出现
//! 一次（三个出处），memories 的 upsert 同理两处——那是台账 T28 同族的欠账，本刀只搬
//! 不改（"搬动"和"改语义"混进一份 diff 就没法用行守恒判据了）。
//!
//! 首尾两行（`impl MemoryStore {` 与闭合）是本片自补的语法外壳，不是搬来的内容；
//! 字段与方法经同一棵模块树可见 ⇒ 本片不新增任何抬升。

use rusqlite::params;

use crate::timeutil::now_secs;

use super::shapes::{MemoryRow, KEY_MAX, LIST_MAX, VALUE_MAX};
use super::store::MemoryStore;

impl MemoryStore {
    /// 管理页列举（全键位）：query 空 = 全量（新→旧），否则 key/value/tags
    /// 子串过滤。**读源 = 真源 md，一条 SQL 都不发**（刀②b-1 换的读源，刀②c-1 起
    /// 这是唯一一档——原先"未接线/开关关 ⇒ 回库里查"那一档连墓碑一起拆了）。
    ///
    /// 返回 `Result` 而不是 `Vec`：管理页命令面的契约（Err 上抛成前端错误条）不因换源
    /// 而变。md 侧的故障形状是 `unreadable` 计数与 warn 日志，不是 Err——文件可以逐条
    /// 坏，坏的只是那几条。
    pub fn list(&self, query: &str, limit: u32) -> Result<Vec<MemoryRow>, String> {
        let limit = limit.clamp(1, LIST_MAX);
        Ok(Self::md_rows(self.files().root(), query, limit as usize))
    }

    /// 人类层列举。三条口径与退役掉的那条 SQL 通路**逐字对齐**，因为"换读源"不该
    /// 顺手改掉管理页的行为：
    /// ① 排序 = `updated_at DESC, key`（`memlayout::iter_facts` 里已按这条排，
    ///    对齐的理由与后果写在那片头注）；
    /// ② 过滤 = key / value / tags 任一子串命中，ASCII 大小写不敏感——这一维现在住在
    ///    [`plugin_api::memlayout::fact_matches`]（S1 刀③b 抽过去的）。原先这里是一份
    ///    本地闭包，而插件的 `memory_search` 改道 md 之后就是同一条谓词的第二个消费者：
    ///    两处各写一份迟早漂成"管理页搜得到、工具搜不到"，那种漂移在界面上看就是"记忆丢了"；
    /// ③ 条数上限 = 同一个钳过的 `limit`。
    /// `unreadable` 只 warn、不报错也不静默：列表照常出其余那些，但"没给看的条数"
    /// 必须落到日志里——它与"没记过"是两个意思（刀②c-1 起这计数也含整层扫不动）。
    fn md_rows(root: &std::path::Path, query: &str, limit: usize) -> Vec<MemoryRow> {
        let scan = plugin_api::memlayout::iter_facts(root);
        if scan.unreadable > 0 {
            tracing::warn!(
                "记忆 md 列举：{} 条/层读不动（未计进本次列表）",
                scan.unreadable
            );
        }
        scan.facts
            .iter()
            .filter(|f| plugin_api::memlayout::fact_matches(f, query))
            .take(limit)
            .map(MemoryRow::from)
            .collect()
    }

    /// 管理页编辑（§7.2 memory_update：只改 value，来源与键名不动）。
    /// 返回 false = 该键位不存在。
    ///
    /// 存在性判据 = **文件在不在**（旧判据是 `UPDATE` 的影响行数）。两者只在
    /// "库里有行、md 里没文件"的存量形状上给出相反答案，而新答案才是对的：真源是
    /// 文件，文件不在就是没这条记忆，库里那行只是一份没跟上的影子。
    ///
    /// tags / source 取的是**文件里的现值**（旧实现回读库里的）。列表也读文件 ⇒
    /// 用户在管理页看到的标签就是这里写回去的那一份 ⇒ 不会出现"编辑正文抹掉了他从
    /// 没见过的标签"。存量 md 缺 `tags:` 行时这里读回空串——那是已知的存量形状，
    /// 决策人 2026-10-18 裁定不回填、由他清理。
    pub fn update_value(&self, workspace: &str, key: &str, value: &str) -> Result<bool, String> {
        if key.is_empty() || key.len() > KEY_MAX {
            return Err(format!("memory key 长度需在 1..={KEY_MAX} 字节"));
        }
        if value.len() > VALUE_MAX {
            return Err(format!("memory value 超过 {VALUE_MAX} 字节上限"));
        }
        let Some(current) = plugin_api::memlayout::read_fact(self.files().root(), workspace, key)?
        else {
            return Ok(false);
        };
        let at = now_secs();
        self.write_md(workspace, key, value, &current.tags, &current.source, at)?;
        self.shadow_update_value(workspace, key, value, at);
        Ok(true)
    }

    /// 写入/覆盖一条记忆（store 基础 upsert）。生产写路径现有三处：
    /// memory-sqlite 插件的 `memory_write` 工具（模型主动写，键位已在
    /// `WsCtx::from_parts` 归一）、管理页 `update_value`，以及 **v2.186 起的
    /// 整理事实卡落库**（`organize/facts.rs`）。键位 = (workspace, key)。
    ///
    /// `source` 三档 = `"explicit"`（用户或模型亲手写）| `"extracted"`
    /// （v2.136 退役的旧自动提取遗留）| `"organized"`（v2.186 = 整理副产物，
    /// 前端 `SourceBadge` 单独一档，用户必须看得出这条不是我写的）。
    ///
    /// **键位在这里归一**（Q2 / MR §6 留的欠账由 v2.186 兑现）：本方法的
    /// workspace 参数来自宿主自己解析的串，不像插件侧已过归一 ⇒ 不接就会造回
    /// 「同项目双键位」。归一走 `workspace::scope_key`（与注入键位、观测
    /// `scope_id`、`md_index_body` 的目录名同一个函数）；`write_md` 拿的是同一个
    /// 归一串 ⇒ sqlite 键位与人类层目录名同形。空串 = 全局层，照原样传下去。
    pub fn write(
        &self,
        workspace: &str,
        key: &str,
        value: &str,
        tags: &str,
        source: &str,
    ) -> Result<(), String> {
        if key.is_empty() || key.len() > KEY_MAX {
            return Err(format!("memory key 长度需在 1..={KEY_MAX} 字节"));
        }
        if value.len() > VALUE_MAX {
            return Err(format!("memory value 超过 {VALUE_MAX} 字节上限"));
        }
        let ws = crate::workspace::scope_key(Some(workspace));
        let at = now_secs();
        // 真源先落：这一句失败就是**这条记忆没写进去**，Err 上抛（旧形状里它只 warn，
        // 因为那时库才是真源）。影子随后，失败只 warn——见 `shadow_upsert` 头注。
        self.write_md(&ws, key, value, tags, source, at)?;
        self.shadow_upsert(&ws, key, value, tags, source, at);
        Ok(())
    }

    /// 管理页删除单条（按键位）。返回 false = 不存在。
    ///
    /// no-op 判据 = **文件原本在不在**（`delete_fact` 的返回值；旧判据是库里删了几行）。
    /// 真源翻转后这条必须反过来，否则"库里没行、文件有"的那种形状会连文件都不碰——
    /// AC-S1-6 那三条遗忘验证能不能成立正卡在这一步（specs/05 刀② 前置取证④）。
    pub fn delete(&self, workspace: &str, key: &str) -> Result<bool, String> {
        let removed = self.delete_md(workspace, key)?;
        if removed {
            self.shadow_delete(workspace, key);
        }
        Ok(removed)
    }

    /// 管理页清空（§11.3 一键清空，UI 只加一次确认；全键位）。返回条数。
    ///
    /// 计数与判据都来自真源：`clear_facts` 报的是**删掉的事实文件数**（旧实现报的是
    /// 库里的行数）。有一层扫不动时它中止并 Err，一条也不动——"说清了其实没清"比
    /// 失败更难被发现。`obsolete/` 软删随刀④ 替换这里的直接删文件，签名不变。
    pub fn clear_all(&self) -> Result<usize, String> {
        let n = plugin_api::memlayout::clear_facts(self.files().root())?;
        self.shadow_clear();
        Ok(n)
    }

    // ---- sqlite 影子写（刀②c-1）----
    //
    // 真源已是 md，库里这一份**没有任何宿主读点**（`list` / `recall_quiet` /
    // `project_index` 三条读面全部改道）。它还留着的唯一理由是
    // `plugins-src/memory-sqlite` 那六枚工具仍按库寻址——刀③ 把它们改成内部调 md 之后，
    // 这一整块连同 `CREATE_TABLE` 一起退役（已登记 specs/05 挂账；不在这刀顺手删，
    // 否则中间态是"管理页写的记忆模型读不到"这种用户立刻能发现的裂缝）。
    //
    // 所以这里失败**只 warn**：掉的是副本，不是真源。把它 Err 上抛会让"写一条记忆"
    // 因为一份没人读的拷贝而失败——那是把影子抬成了契约，方向正好反了。

    fn shadow_upsert(&self, ws: &str, key: &str, value: &str, tags: &str, source: &str, at: i64) {
        let Ok(conn) = self.open_conn() else {
            tracing::warn!("记忆影子写跳过 `{key}`：sqlite 不可用");
            return;
        };
        let r = conn.execute(
            "INSERT INTO memories (workspace, key, value, tags, updated_at, source) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(workspace, key) DO UPDATE SET
                 value = excluded.value, tags = excluded.tags,
                 updated_at = excluded.updated_at, source = excluded.source",
            params![ws, key, value, tags, at, source],
        );
        note_shadow("写", key, r.map(|_| ()).map_err(|e| e.to_string()));
    }

    fn shadow_update_value(&self, ws: &str, key: &str, value: &str, at: i64) {
        let Ok(conn) = self.open_conn() else {
            tracing::warn!("记忆影子编辑跳过 `{key}`：sqlite 不可用");
            return;
        };
        let r = conn.execute(
            "UPDATE memories SET value = ?3, updated_at = ?4
             WHERE workspace = ?1 AND key = ?2",
            params![ws, key, value, at],
        );
        note_shadow("编辑", key, r.map(|_| ()).map_err(|e| e.to_string()));
    }

    fn shadow_delete(&self, ws: &str, key: &str) {
        let Ok(conn) = self.open_conn() else {
            tracing::warn!("记忆影子删跳过 `{key}`：sqlite 不可用");
            return;
        };
        let r = conn.execute(
            "DELETE FROM memories WHERE workspace = ?1 AND key = ?2",
            params![ws, key],
        );
        note_shadow("删", key, r.map(|_| ()).map_err(|e| e.to_string()));
    }

    fn shadow_clear(&self) {
        let Ok(conn) = self.open_conn() else {
            tracing::warn!("记忆影子清空跳过：sqlite 不可用");
            return;
        };
        let r = conn.execute("DELETE FROM memories", []);
        note_shadow("清空", "", r.map(|_| ()).map_err(|e| e.to_string()));
    }
}

/// 影子写失败的**唯一**文案出处（四类动作 × 一处措辞；分四处写迟早漂成四种说法，
/// 而日志里同族事件的说法一多，人就再也 grep 不动它）。
fn note_shadow(op: &str, key: &str, e: Result<(), String>) {
    if let Err(e) = e {
        let target = if key.is_empty() {
            String::new()
        } else {
            format!(" `{key}`")
        };
        tracing::warn!("记忆 sqlite 影子{op}{target}失败（真源已落，不影响本次操作）: {e}");
    }
}
