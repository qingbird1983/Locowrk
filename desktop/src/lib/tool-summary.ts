// 工具与动作的人类呈现（UI shell doc §3.18，用户 2026-10-01 裁定：动作
// 必须显示中文名、参数摘要必须加工过——严禁裸复制命令 / 绝对路径 / JSON）。
// 摘要态人话、明细态原样：原始性只在工具行的展开明细里出现。
// 命令剥包装与工作区相对化的**实现在 `lib/tool-args.ts`**（v2.168 批 F-18：与
// `tool-detail.ts` 各写一遍的同族重复收成一份，两种显示语义按出口名区分）。
import { clip } from "./format";
import { relativizeBare, stripCommandWrapping } from "./tool-args";
import type { AssetRefDto } from "../types";

/** 动作名 i18n 定位；null = 表外工具（回落工具 id + 「插件」徽章）。 */
export function toolAction(
  tool: string,
): { key: string; params?: Record<string, string> } | null {
  const table: Record<string, string> = {
    shell: "tool.action.run",
    shell_read: "tool.action.run",
    // sandbox_run：tool-sandbox 插件已于 v2.145 整目录退役，新会话不可达；
    // 此处保留只为**历史会话日志重放**仍能渲染旧工具行（用户 2026-10-10 裁定）。
    sandbox_run: "tool.action.sandbox",
    fs_read: "tool.action.read",
    fs_list: "tool.action.list",
    fs_write: "tool.action.write",
    edit_replace: "tool.action.edit",
    edit_lines: "tool.action.edit",
    edit_diff: "tool.action.edit",
    fs_delete: "tool.action.delete",
    fs_restore: "tool.action.restore",
    git_status: "tool.action.gitStatus",
    git_diff: "tool.action.gitDiff",
    git_commit: "tool.action.gitCommit",
    memory_write: "tool.action.memoryWrite",
    memory_read: "tool.action.memorySearch",
    memory_search: "tool.action.memorySearch",
    memory_list: "tool.action.memorySearch",
    search_tools: "tool.action.searchTools",
    subagent_run: "tool.action.subagent",
    subagent_verify: "tool.action.subagentVerify",
    schedule_create: "tool.action.scheduleCreate",
    schedule_list: "tool.action.scheduleList",
    schedule_delete: "tool.action.scheduleDelete",
    notify: "tool.action.notify",
    fetch: "tool.action.fetch",
    mcp_status: "tool.action.mcpStatus",
    screenshot: "tool.action.screenshot",
    read_image: "tool.action.readImage",
    pdf_to_images: "tool.action.pdfToImages",
  };
  if (table[tool]) return { key: table[tool]! };
  if (tool.startsWith("git_")) return { key: "tool.action.git" };
  if (tool.startsWith("mcp_")) {
    // mcp_{server}_{tool} 形态：server / tool 取真名做参数（尽力解析）
    const rest = tool.slice("mcp_".length);
    const sep = rest.indexOf("_");
    if (sep > 0) {
      return {
        key: "tool.action.mcp",
        params: { server: rest.slice(0, sep), tool: rest.slice(sep + 1) },
      };
    }
    return { key: "tool.action.mcpStatus" };
  }
  return null;
}

const OBJECT_KEYS = ["prompt", "query", "url", "key", "text", "pattern", "name"];

function objectSummary(a: Record<string, unknown>): string {
  const picked: string[] = [];
  for (const k of OBJECT_KEYS) {
    if (picked.length >= 2) break;
    const v = a[k];
    if (typeof v === "string" && v.trim()) {
      picked.push(`${k}：${clip(v.trim(), 24)}`);
    } else if (typeof v === "number" || typeof v === "boolean") {
      picked.push(`${k}：${String(v)}`);
    }
  }
  const rest = Object.keys(a).length - picked.length;
  if (picked.length === 0) return "";
  return picked.join(" · ") + (rest > 0 ? ` … +${rest}` : "");
}

/** 参数摘要加工（§3.18 按工具类型分派；兜底 = clip 第一个原始值 48）。 */
export function toolArgSummary(
  tool: string,
  args: unknown,
  roots: string[],
): string {
  const a = (args ?? {}) as Record<string, unknown>;
  if (tool === "shell" || tool === "shell_read" || tool === "sandbox_run") {
    const raw =
      typeof a.command === "string"
        ? a.command
        : Array.isArray(a.args)
          ? a.args.filter((v) => typeof v === "string").join(" ")
          : firstString(a);
    return raw ? stripCommandWrapping(raw, 48) : "";
  }
  if (tool.startsWith("fs_") || tool.startsWith("edit_")) {
    const p =
      typeof a.path === "string"
        ? a.path
        : typeof a.file === "string"
          ? a.file
          : typeof a.key === "string"
            ? a.key
            : "";
    if (!p) return firstString(a);
    return relativizeBare(p, roots);
  }
  if (tool.startsWith("mcp_")) {
    const s = typeof a.server === "string" ? a.server : "";
    const t = typeof a.tool === "string" ? a.tool : "";
    return [s, t].filter(Boolean).join(" / ");
  }
  const o = objectSummary(a);
  return o || firstString(a);
}

function firstString(a: Record<string, unknown>): string {
  for (const v of Object.values(a)) {
    if (typeof v === "string") return clip(v, 48);
    if (typeof v === "number" || typeof v === "boolean") return String(v);
  }
  return "";
}

/** 写/编辑类工具的变更统计（§3.13 折叠行尾 +N −M 与产物大小）：从输出里
 * 尽力提取常见字段，取不到就不渲染整块（不虚构）。 */
export function toolDiffStats(output: unknown): {
  added?: number;
  removed?: number;
  size?: string;
} {
  if (output === null || typeof output !== "object") return {};
  const o = output as Record<string, unknown>;
  const num = (...keys: string[]): number | undefined => {
    for (const k of keys) {
      const v = o[k];
      if (typeof v === "number") return v;
    }
    return undefined;
  };
  const added = num("lines_added", "added", "insertions", "additions");
  const removed = num("lines_removed", "removed", "deletions");
  const bytes = num("size_bytes", "bytes", "size");
  const size =
    bytes !== undefined
      ? bytes >= 1024
        ? `${(bytes / 1024).toFixed(1)} KB`
        : `${bytes} B`
      : undefined;
  return { added, removed, size };
}

/** 工具产物里的图片资产（批 E §8.1 投影约定）：`asset`（单图）或
 * `assets`（数组），mime `image/*` 才算——PDF 引用不是图片产物。无字段 /
 * 非对象输出 = 空表（工具行不渲染预览区）。 */
export function toolImageAssets(output: unknown): AssetRefDto[] {
  if (output === null || typeof output !== "object") return [];
  const o = output as Record<string, unknown>;
  const parse = (v: unknown): AssetRefDto | null => {
    if (v === null || typeof v !== "object") return null;
    const a = v as Record<string, unknown>;
    if (
      typeof a.id !== "string" ||
      typeof a.mime !== "string" ||
      typeof a.name !== "string" ||
      !a.mime.startsWith("image/")
    ) {
      return null;
    }
    return { id: a.id, mime: a.mime, name: a.name };
  };
  const single = parse(o.asset);
  const list = Array.isArray(o.assets)
    ? o.assets.map(parse).filter((a): a is AssetRefDto => a !== null)
    : [];
  return [...(single ? [single] : []), ...list];
}
