/** **记忆管理面**（⑪，Context and Memory.md §7.2 / F2 / Q10 / Q15）：列举、改值、删除、
 * 清空、文件层对账、打开人类层目录（全局与本项目两个出口）、注入解释面板。
 *
 * 这里删的是"记忆条目"，与前端删除会话无关；按 (workspace, key) 键位定位的纪律写在原注释里。 */

import { invoke } from "@tauri-apps/api/core";
import type { InjectView, MemoryRow, ReconcileSummary } from "../../types";
// ---- 记忆管理（⑪，Context and Memory.md §7.2） ----

/** 列举记忆：query 空 = 全量（新→旧），否则子串过滤。 */
export function memoryList(query?: string, limit?: number): Promise<MemoryRow[]> {
  return invoke("memory_list", { query: query ?? null, limit: limit ?? null });
}

/** 编辑记忆值（只改 value；按 (workspace, key) 键位定位，键不存在后端报错）。 */
export function memoryUpdate(workspace: string, key: string, value: string): Promise<void> {
  return invoke("memory_update", { workspace, key, value });
}

/** 删除单条记忆（按 (workspace, key) 键位；不存在后端报错）。 */
export function memoryDelete(workspace: string, key: string): Promise<void> {
  return invoke("memory_delete", { workspace, key });
}

/** 清空全部记忆。返回删除条数。 */
export function memoryClearAll(): Promise<number> {
  return invoke("memory_clear_all");
}

/** F2 reconcile：diff 记忆文件 ↔ 索引，以文件为准回写（文件层关 = 零计数）。 */
export function memoryReconcile(): Promise<ReconcileSummary> {
  return invoke("memory_reconcile");
}

/** F2 人的编辑面入口：打开（必要时先建）记忆人类层目录。 */
export function memoryRevealFolder(): Promise<void> {
  return invoke("memory_reveal_folder");
}

/** 队列 Q15（MB §5 · 看板 T2）：打开**该项目**的人类层记忆目录。入参是
 * workspace 串而不是路径——路径由后端 `MemoryFiles::project_dir` 单点算出，
 * 前端递不进任意路径（与 `revealInExplorer` 的 fsRoots 白名单同向、更窄）。
 * `''`（未归属）后端报错，所以按钮在未归属档禁用（用例各钉一次）。 */
export function memoryRevealProject(workspace: string): Promise<void> {
  return invoke("memory_reveal_project", { workspace });
}

/** 队列 Q10（L4）：注入解释面板——最近一次真算过四信号的注入回执 + 观测表派生读数。
 * 只读：与 memory_tree_view 同款「打开拉一次 + 手动刷新」，不推送不轮询。 */
export function memoryInjectView(): Promise<InjectView> {
  return invoke("memory_inject_view");
}
