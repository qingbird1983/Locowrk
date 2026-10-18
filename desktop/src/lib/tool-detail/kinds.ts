/** 折叠行的**类型词 + 字形分派**（§3.8 五列栅格前两行）：`KIND_TABLE` 那张按工具名的表 +
 * `toolKind()` 的四条前缀回落（`git_` / `schedule_` / `mcp_` / 兜底）。
 *
 * 表里 `sandbox_run` 那条是 v2.145 退役后**为历史会话重放留的**（用户裁定保留，注释在原处）；
 * 判定表点的"shell 系三个名字在 67/131/512 三处各写一遍"这一刀**没有并**——那是语义改动，
 * 归 T34 同族的重复项，搬行刀不夹带。 */

import type { IconName } from "../icons";
// ---------------------------------------------------------------------------
// 折叠行：类型词 + 字形分派（§3.8 五列栅格前两列）
// ---------------------------------------------------------------------------

const KIND_TABLE: Record<string, { type: string; icon: IconName }> = {
  fs_read: { type: "read", icon: "file" },
  fs_list: { type: "dir", icon: "folder" },
  fs_write: { type: "write", icon: "edit" },
  fs_delete: { type: "del", icon: "trash" },
  fs_restore: { type: "restore", icon: "undo" },
  fs_grep: { type: "search", icon: "search" },
  edit_replace: { type: "write", icon: "edit" },
  edit_lines: { type: "write", icon: "edit" },
  edit_diff: { type: "write", icon: "edit" },
  shell: { type: "terminal", icon: "terminal" },
  shell_read: { type: "terminal", icon: "terminal" },
  // tool-sandbox 已于 v2.145 退役，此条只服务历史会话日志重放（用户裁定保留）。
  sandbox_run: { type: "terminal", icon: "terminal" },
  fetch: { type: "web", icon: "globe" },
  memory_write: { type: "memory", icon: "database" },
  memory_read: { type: "memory", icon: "database" },
  memory_search: { type: "memory", icon: "database" },
  memory_list: { type: "memory", icon: "database" },
  search_tools: { type: "search", icon: "search" },
  subagent_run: { type: "subagent", icon: "bot" },
  notify: { type: "notify", icon: "bell" },
  screenshot: { type: "screenshot", icon: "camera" },
  read_image: { type: "read", icon: "eye" },
  pdf_to_images: { type: "read", icon: "file" },
};

/** 类型词 i18n 后缀 + 字形（i18n key = `tool.type.${type}`；表外回落 工具/plug）。 */
export function toolKind(tool: string): { type: string; icon: IconName } {
  const hit = KIND_TABLE[tool];
  if (hit) return hit;
  if (tool.startsWith("git_")) return { type: "git", icon: "gitBranch" };
  if (tool.startsWith("schedule_")) return { type: "schedule", icon: "clock" };
  if (tool.startsWith("mcp_")) return { type: "plugin", icon: "plug" };
  return { type: "tool", icon: "plug" };
}
