//! 章节编辑/删除（M5 编辑面，树 v2 §12.1「切流后开放编辑」+ §14 纪律 5
//! 「用户编辑后标记手动，不再被自动覆盖」）：命令层薄适配——校验、落盘、
//! tree.md 同步三步。只动投影（树是投影，原文一个字不改）；成功后投影
//! mtime 前移，人改保护自然放行下一次 tree.md 重建。

use memory_tree::chapter::{SUMMARY_MAX_CHARS, TITLE_MAX_CHARS};

use super::Organizer;

impl Organizer {
    /// 编辑章节标题/摘要（两字段独立可选，只动提供的那个）。成功 = 投影
    /// 落盘 + tree.md 同步（人读层照渲染重建——人改值已在投影里）。
    pub fn tree_edit_chapter(
        &self,
        workspace: &str,
        session_id: &str,
        chapter_id: &str,
        title: Option<&str>,
        summary: Option<&str>,
    ) -> Result<(), String> {
        let title = title.map(str::trim).filter(|s| !s.is_empty());
        let summary = summary.map(str::trim).filter(|s| !s.is_empty());
        if title.is_none() && summary.is_none() {
            return Err("标题与摘要至少提供一项".into());
        }
        if let Some(t) = title {
            if t.chars().count() > TITLE_MAX_CHARS {
                return Err(format!("标题超过 {TITLE_MAX_CHARS} 字上限"));
            }
        }
        if let Some(s) = summary {
            if s.chars().count() > SUMMARY_MAX_CHARS {
                return Err(format!("摘要超过 {SUMMARY_MAX_CHARS} 字上限"));
            }
        }
        let mut tree = self
            .tree_store
            .load(workspace, session_id)
            .ok_or_else(|| "树投影不存在".to_string())?;
        if !tree.edit_chapter(chapter_id, title, summary) {
            return Err(format!(
                "章节 {chapter_id} 不存在或整理在途（占位章节不可编辑）"
            ));
        }
        self.tree_store
            .save(&tree)
            .map_err(|e| format!("投影保存失败: {e}"))?;
        self.sync_tree_md(workspace);
        Ok(())
    }

    /// 删除章节（§12.1「删除捆，不影响原文」）：仅从投影摘除，游标不动
    /// ——被删区间停留在已整理侧，不再进目录/尾巴 = 有意不可见（不是
    /// 丢失：原文在会话日志，整树重建兜底会重新生成）。
    pub fn tree_delete_chapter(
        &self,
        workspace: &str,
        session_id: &str,
        chapter_id: &str,
    ) -> Result<(), String> {
        let mut tree = self
            .tree_store
            .load(workspace, session_id)
            .ok_or_else(|| "树投影不存在".to_string())?;
        if !tree.remove_chapter(chapter_id) {
            return Err(format!(
                "章节 {chapter_id} 不存在或整理在途（占位章节不可删除）"
            ));
        }
        self.tree_store
            .save(&tree)
            .map_err(|e| format!("投影保存失败: {e}"))?;
        self.sync_tree_md(workspace);
        Ok(())
    }
}
