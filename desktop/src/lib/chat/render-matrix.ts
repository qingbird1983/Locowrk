/** 折叠行身份矩阵（《Conversation Flow Rendering Spec v1.md》§3.2 身份枚举 + §5.3 完整表）。
 *
 * 38 个身份 = 33 个 L1 精确名 + 4 个 L2 前缀族 + 1 个 L3 兜底（`fallback`）。一个身份决定
 * 八件事：图标 / 类型词 / 摘要来源 / 展开形态 / 右栏入口 / 是否可折 / 语言徽章 / diff。
 *
 * 本文件是**唯一登记面**：加一个工具 = 在这里加一行，别处不改。
 * `lib/tool-detail/kinds.ts` 的 `KIND_TABLE` 与 `toolKind()` 已并入此处并删除（设计稿 §3.3）。
 *
 * ⚠ `verb` 暂接现役 `tool.type.*` 的键名（见 locales/{zh-CN,en}/tools.ts）：设计稿 §5.3 的
 * 中文动词与该键集并非一一对应（如 `fs_list` 设计稿「列出」/ 现役键 `dir`「目录」），动词
 * 全集与六态文案统一归刀 6，本刀不动任何 i18n 文件。 */

import type { IconName } from "../icons";

/** 摘要取值来源（设计稿 §5.2）。 */
export type SummarySource =
  | "path"
  | "pathDir"
  | "cmd"
  | "query"
  | "url"
  | "prompt"
  | "key"
  | "title"
  | "count"
  | "field"
  | "none";

/** 展开形态（设计稿 §7 形态库）。 */
export type BodyShape = "code" | "list" | "diff" | "props" | "term" | "text" | "media";

/** 右栏入口（设计稿 §5.1；`none` = 不挂入口）。 */
export type Rail = "none" | "file:read" | "file:patch" | "subagent" | "browser";

/** 一个身份的全部渲染描述。 */
export interface ToolView {
  icon: IconName;
  /** 类型词 i18n 后缀，键 = `tool.type.${verb}`。 */
  verb: string;
  summary: SummarySource;
  body: BodyShape;
  rail: Rail;
  fold: "always" | "never";
  badge: "file" | "dir" | "none";
  diff: "show" | "none";
}

/** 38 个身份键（设计稿 §3.2；L1 精确名 33 + L2 前缀族 4 + L3 兜底 1）。 */
export type ToolIdentity =
  | "fs_read"
  | "fs_list"
  | "fs_grep"
  | "fs_write"
  | "fs_delete"
  | "fs_restore"
  | "edit_replace"
  | "edit_lines"
  | "edit_diff"
  | "shell"
  | "shell_read"
  | "fetch"
  | "memory_write"
  | "memory_delete"
  | "memory_undo"
  | "memory_read"
  | "memory_list"
  | "memory_search"
  | "memory_expand"
  | "memory_search_titles"
  | "memory_project_history"
  | "search_tools"
  | "subagent_run"
  | "subagent_verify"
  | "ask_user"
  | "notify"
  | "screenshot"
  | "read_image"
  | "pdf_to_images"
  | "browser_open"
  | "browser_console"
  | "mcp_status"
  | "dev_echo"
  | "git"
  | "schedule"
  | "mcp"
  | "memory_tree"
  | "fallback";

/** 身份 → 渲染描述。表序与设计稿 §5.3 一致，便于逐行对表。
 *
 * 三条硬规矩（设计稿 §5.3）：动词永不出现裸工具 id；`fold: "never"` 只三种
 * （notify / browser_open / dev_echo）；`diff: "show"` 只在四个写身份。 */
export const IDENTITY_TABLE: Record<ToolIdentity, ToolView> = {
  fs_read: {
    icon: "file", verb: "read", summary: "path", body: "code",
    rail: "file:read", fold: "always", badge: "file", diff: "none",
  },
  fs_list: {
    icon: "folder", verb: "dir", summary: "pathDir", body: "list",
    rail: "none", fold: "always", badge: "dir", diff: "none",
  },
  fs_grep: {
    icon: "search", verb: "search", summary: "query", body: "list",
    rail: "file:read", fold: "always", badge: "none", diff: "none",
  },
  fs_write: {
    icon: "edit", verb: "write", summary: "path", body: "diff",
    rail: "file:patch", fold: "always", badge: "file", diff: "show",
  },
  fs_delete: {
    icon: "trash", verb: "del", summary: "path", body: "props",
    rail: "none", fold: "always", badge: "file", diff: "none",
  },
  fs_restore: {
    icon: "undo", verb: "restore", summary: "path", body: "props",
    rail: "none", fold: "always", badge: "file", diff: "none",
  },
  edit_replace: {
    icon: "edit", verb: "write", summary: "path", body: "diff",
    rail: "file:patch", fold: "always", badge: "file", diff: "show",
  },
  edit_lines: {
    icon: "edit", verb: "write", summary: "path", body: "diff",
    rail: "file:patch", fold: "always", badge: "file", diff: "show",
  },
  edit_diff: {
    icon: "edit", verb: "write", summary: "path", body: "diff",
    rail: "file:patch", fold: "always", badge: "file", diff: "show",
  },
  shell: {
    icon: "terminal", verb: "terminal", summary: "cmd", body: "term",
    rail: "none", fold: "always", badge: "none", diff: "none",
  },
  shell_read: {
    icon: "terminal", verb: "terminal", summary: "cmd", body: "term",
    rail: "none", fold: "always", badge: "none", diff: "none",
  },
  fetch: {
    icon: "globe", verb: "web", summary: "url", body: "text",
    rail: "browser", fold: "always", badge: "none", diff: "none",
  },
  memory_write: {
    icon: "database", verb: "memory", summary: "key", body: "props",
    rail: "none", fold: "always", badge: "none", diff: "none",
  },
  memory_delete: {
    icon: "database", verb: "memory", summary: "key", body: "props",
    rail: "none", fold: "always", badge: "none", diff: "none",
  },
  memory_undo: {
    icon: "database", verb: "memory", summary: "key", body: "props",
    rail: "none", fold: "always", badge: "none", diff: "none",
  },
  memory_read: {
    icon: "database", verb: "memory", summary: "key", body: "props",
    rail: "none", fold: "always", badge: "none", diff: "none",
  },
  memory_list: {
    icon: "database", verb: "memory", summary: "count", body: "list",
    rail: "none", fold: "always", badge: "none", diff: "none",
  },
  memory_search: {
    icon: "database", verb: "memory", summary: "query", body: "list",
    rail: "none", fold: "always", badge: "none", diff: "none",
  },
  memory_expand: {
    icon: "database", verb: "memory", summary: "key", body: "text",
    rail: "none", fold: "always", badge: "none", diff: "none",
  },
  memory_search_titles: {
    icon: "database", verb: "memory", summary: "query", body: "list",
    rail: "none", fold: "always", badge: "none", diff: "none",
  },
  memory_project_history: {
    icon: "database", verb: "memory", summary: "title", body: "list",
    rail: "none", fold: "always", badge: "none", diff: "none",
  },
  search_tools: {
    icon: "search", verb: "search", summary: "query", body: "list",
    rail: "none", fold: "always", badge: "none", diff: "none",
  },
  subagent_run: {
    icon: "bot", verb: "subagent", summary: "prompt", body: "text",
    rail: "subagent", fold: "always", badge: "none", diff: "none",
  },
  subagent_verify: {
    icon: "shieldCheck", verb: "subagent", summary: "title", body: "props",
    rail: "subagent", fold: "always", badge: "none", diff: "none",
  },
  ask_user: {
    icon: "chat", verb: "tool", summary: "prompt", body: "text",
    rail: "none", fold: "always", badge: "none", diff: "none",
  },
  notify: {
    icon: "bell", verb: "notify", summary: "field", body: "props",
    rail: "none", fold: "never", badge: "none", diff: "none",
  },
  screenshot: {
    icon: "camera", verb: "screenshot", summary: "field", body: "media",
    rail: "none", fold: "always", badge: "none", diff: "none",
  },
  read_image: {
    icon: "eye", verb: "read", summary: "path", body: "media",
    rail: "file:read", fold: "always", badge: "file", diff: "none",
  },
  pdf_to_images: {
    icon: "file", verb: "read", summary: "path", body: "media",
    rail: "file:read", fold: "always", badge: "file", diff: "none",
  },
  browser_open: {
    icon: "globe", verb: "web", summary: "url", body: "text",
    rail: "browser", fold: "never", badge: "none", diff: "none",
  },
  browser_console: {
    icon: "terminal", verb: "terminal", summary: "cmd", body: "term",
    rail: "browser", fold: "always", badge: "none", diff: "none",
  },
  mcp_status: {
    icon: "plug", verb: "plugin", summary: "field", body: "props",
    rail: "none", fold: "always", badge: "none", diff: "none",
  },
  dev_echo: {
    icon: "plug", verb: "tool", summary: "field", body: "text",
    rail: "none", fold: "never", badge: "none", diff: "none",
  },
  git: {
    icon: "gitBranch", verb: "git", summary: "cmd", body: "term",
    rail: "none", fold: "always", badge: "none", diff: "none",
  },
  schedule: {
    icon: "clock", verb: "schedule", summary: "field", body: "list",
    rail: "none", fold: "always", badge: "none", diff: "none",
  },
  mcp: {
    icon: "plug", verb: "plugin", summary: "field", body: "props",
    rail: "none", fold: "always", badge: "none", diff: "none",
  },
  memory_tree: {
    icon: "database", verb: "memory", summary: "count", body: "list",
    rail: "none", fold: "always", badge: "none", diff: "none",
  },
  fallback: {
    icon: "plug", verb: "tool", summary: "field", body: "props",
    rail: "none", fold: "always", badge: "none", diff: "none",
  },
};

/** L1 精确名 → 身份。有些身份吃多个工具名（别名与现役退役名）。 */
const BY_NAME: Record<string, ToolIdentity> = {
  fs_read: "fs_read",
  fs_list: "fs_list",
  fs_grep: "fs_grep",
  fs_write: "fs_write",
  fs_delete: "fs_delete",
  fs_restore: "fs_restore",
  edit_replace: "edit_replace",
  edit_lines: "edit_lines",
  edit_diff: "edit_diff",
  shell: "shell",
  shell_read: "shell_read",
  // tool-sandbox 已于 v2.145 退役，此条只服务历史会话日志重放（用户裁定保留）。
  sandbox_run: "shell",
  fetch: "fetch",
  memory_write: "memory_write",
  memory_delete: "memory_delete",
  memory_undo: "memory_undo",
  memory_read: "memory_read",
  memory_list: "memory_list",
  memory_search: "memory_search",
  memory_expand: "memory_expand",
  memory_search_titles: "memory_search_titles",
  memory_project_history: "memory_project_history",
  search_tools: "search_tools",
  subagent_run: "subagent_run",
  subagent_verify: "subagent_verify",
  ask_user: "ask_user",
  notify: "notify",
  screenshot: "screenshot",
  read_image: "read_image",
  pdf_to_images: "pdf_to_images",
  browser_open: "browser_open",
  browser_console: "browser_console",
  mcp_status: "mcp_status",
  mcp_reload: "mcp_status",
  // `tool-echo` 是**插件 id**（`plugins-src/tool-echo/src/lib.rs:32`），它注册的工具名
  // 就叫 `echo`。设计稿 §3.2 把它与工具名并列写，是笔误。
  echo: "dev_echo",
  shout: "dev_echo",
};

/** L2 前缀族，顺序即优先级（更长的前缀在前，避免被短前缀抢走）。 */
const BY_PREFIX: readonly (readonly [string, ToolIdentity])[] = [
  ["git_", "git"],
  ["schedule_", "schedule"],
  ["memory_", "memory_tree"],
  ["mcp_", "mcp"],
];

/** 闸的扫描面承认不到、但已知真实存在的工具名（`scripts/check-render-matrix.mjs` 读它放行）。
 *
 * 每条都要写明来历：这是豁免，不是垃圾桶。闸会反向校验——一旦某名不再触发问题
 * （被登记进 `BY_NAME`，或 Rust 侧已消失），这里就得同步删，否则报「过期豁免」。 */
export const GATE_EXEMPT_TOOLS: Record<string, string> = {
  danger: "kernel-bin 命令行 demo 夹具（crates/kernel-bin/src/inline_tools.rs:23），不过桌面端",
  send_email: "阶段六内置演示工具（desktop/src-tauri/src/demo_tools.rs:17），落 fallback 属预期",
  subagent_run: "注册名取自 SELF_TOOL 常量（crates/kernel-agent/src/subagent/filter.rs:16），非字面量 .into()",
  sandbox_run: "tool-sandbox 于 v2.145 退役，此名只服务历史会话日志重放",
};

/** 工具名 → 身份。L1 精确名 → L2 前缀族 → L3 `fallback`，逐级匹配，不读插件名。 */
export function toolIdentity(tool: string): ToolIdentity {
  const hit = BY_NAME[tool];
  if (hit) return hit;
  for (const [prefix, identity] of BY_PREFIX) {
    if (tool.startsWith(prefix)) return identity;
  }
  return "fallback";
}

/** 工具名 → 渲染描述。表完整，任何工具名都有落点（最差是 `fallback`）。 */
export function toolView(tool: string): ToolView {
  return IDENTITY_TABLE[toolIdentity(tool)];
}

/** 五相位（设计稿 §4）：由 `ToolItem` 的既有字段组合得出，**不新增字段**。
 * 优先级 `unknown > awaiting > running > ok|failed`——三者在事件流里可能同时为真
 * （崩溃恢复的会话里 `uncertain` 会盖住已落盘的成功）。 */
export const PHASES = ["running", "awaiting", "ok", "failed", "unknown"] as const;
export type ToolPhase = (typeof PHASES)[number];

/** `ToolItem` → 相位。C2 的状态列与折叠层都读它，判定只此一处。 */
export function toolPhase(item: {
  status: string;
  ok?: boolean | null;
  uncertain?: boolean | null;
}): ToolPhase {
  if (item.uncertain) return "unknown";
  if (item.status === "awaiting_approval") return "awaiting";
  if (item.status === "running") return "running";
  return item.ok === false ? "failed" : "ok";
}
