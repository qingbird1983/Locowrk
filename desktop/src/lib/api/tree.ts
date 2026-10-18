/** **记忆树的呈现与编辑面**（M3 §3.37④ / M5 §3.37 / M6.4 / 队列 Q14）：树投影清单、
 * 章节清单、原文切片、项目级联合视图、整组会话流，加上改标题摘要与删章节两条编辑。
 *
 * 只读那五条与编辑那两条同片 = 它们读的是同一份投影；"树是投影——只动投影不碰原文"这条
 * 纪律写在原文的段落注释里，跟着搬。 */

import { invoke } from "@tauri-apps/api/core";
import type {
  ProjectTreeView,
  StreamView,
  TreeChaptersView,
  TreeOriginalItem,
  TreeSummaryView,
} from "../../types";
// ---- 记忆树只读呈现（M3 §3.37④；直读投影，不占会话锁） ----

/** 树投影清单（会话切换器数据源，最近更新降序）。 */
export function memoryTreeSessions(): Promise<TreeSummaryView[]> {
  return invoke("memory_tree_sessions");
}

/** 选中会话的章节清单（投影缺/坏 = exists=false，前端走空态）。 */
export function memoryTreeChapters(
  sessionId: string,
  workspace: string,
): Promise<TreeChaptersView> {
  return invoke("memory_tree_chapters", { sessionId, workspace });
}

/** 章节原文切片（与记忆管理器材料同形；会话已删/章节不存在 = reject）。 */
export function memoryTreeOriginal(
  sessionId: string,
  workspace: string,
  chapterId: string,
): Promise<TreeOriginalItem[]> {
  return invoke("memory_tree_original", { sessionId, workspace, chapterId });
}

/** 项目级 L0 联合视图（M6.4；跨会话捆平铺 + v2.141 总结链，时间正序）。 */
export function memoryTreeProject(workspace: string): Promise<ProjectTreeView> {
  return invoke("memory_tree_project", { workspace });
}

/** 整组会话流（队列 Q14 · MB §2.3）：一轮总结区间（组键 `"from:to"`，取自
 * memory_tree_chapters 的 summaries）或 `"orphan"` 未归组的原文，按事件序一次
 * 汇成一块原子视图。投影缺失 / 会话日志读不出 / 组键不成立 = reject；
 * 单节切片失败在 section.error 里，不撤整组。 */
export function memoryTreeStream(
  sessionId: string,
  workspace: string,
  group: string,
): Promise<StreamView> {
  return invoke("memory_tree_stream", { sessionId, workspace, group });
}

// ---- 记忆树编辑面（M5 §3.37；树是投影——只动投影不碰原文） ----

/** 编辑章节标题/摘要（两字段独立可选；pending 占位 = reject）。 */
export function memoryTreeEditChapter(
  sessionId: string,
  workspace: string,
  chapterId: string,
  patch: { title?: string; summary?: string },
): Promise<void> {
  return invoke("memory_tree_edit_chapter", {
    sessionId,
    workspace,
    chapterId,
    title: patch.title ?? null,
    summary: patch.summary ?? null,
  });
}

/** 删除章节（不影响原文；被删区间停留已整理侧 = 有意不可见）。 */
export function memoryTreeDeleteChapter(
  sessionId: string,
  workspace: string,
  chapterId: string,
): Promise<void> {
  return invoke("memory_tree_delete_chapter", {
    sessionId,
    workspace,
    chapterId,
  });
}
