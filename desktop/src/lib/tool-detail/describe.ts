/** 折叠行的**那一句话**（§3.18 加工规则在栅格形态下的收窄版）：`toolDescribe()` 按工具类型
 * 取"最该看见的那个值"——文件只显文件名、命令只显首命令、子代理 = 任务截断、检索 = 搜索词。
 *
 * 三个参数读取小件 `baseName` / `asRecord` / `str` 与它同时出现（主分派与 `blocks` 片也读），
 * 所以住在这一片：它们是"从 unknown 的 args 里稳妥取个值"这一件事，再拆就是为行数造结构。
 * `stripCommandWrapping` 的实现在 `lib/tool-args.ts`（v2.168 批 F-18 收成一份），这里只调它。
 * ⚠ 那句外层 `clip` 冗余是**改前既有形态**，注释里已登记 F6 顺带发现，本刀逐字保住不动它。 */

import { clip } from "../format";
import { stripCommandWrapping } from "../tool-args";
// ---------------------------------------------------------------------------
// 折叠行描述（一句话 ≤40 字，单行省略）
// ---------------------------------------------------------------------------

export function baseName(path: string): string {
  const norm = path.replace(/\\/g, "/");
  return norm.split("/").filter(Boolean).pop() ?? norm;
}

export function asRecord(v: unknown): Record<string, unknown> {
  return v !== null && typeof v === "object" ? (v as Record<string, unknown>) : {};
}

export function str(a: Record<string, unknown>, ...keys: string[]): string {
  for (const k of keys) {
    const v = a[k];
    if (typeof v === "string" && v.trim()) return v;
  }
  return "";
}

/** 折叠行描述（§3.18 加工规则在栅格形态下的收窄版）：文件只显文件名、
 * 命令只显首命令、子代理 = 任务截断、检索 = 搜索词。 */
export function toolDescribe(tool: string, args: unknown): string {
  const a = asRecord(args);
  if (tool === "screenshot") {
    const d = typeof a.display === "number" ? a.display : 0;
    return `display ${d}`;
  }
  if (tool === "read_image" || tool === "pdf_to_images") {
    const name = baseName(str(a, "path"));
    if (!name) return "";
    if (tool === "pdf_to_images" && typeof a.max_pages === "number") {
      return `${name} · ${a.max_pages}p`;
    }
    return name;
  }
  if (tool === "shell" || tool === "shell_read" || tool === "sandbox_run") {
    const raw =
      typeof a.command === "string"
        ? a.command
        : Array.isArray(a.args)
          ? a.args.filter((v) => typeof v === "string").join(" ")
          : "";
    if (!raw) return "";
    // 外层 `clip` 是**改前既有形态**，本批按"收敛不动显示语义"的纪律逐字保住：
    // 内层已 clip 到 40，外层再 clip 会把 ` … +N` 后缀吃掉——这条冗余已登记为
    // 《AUDIT-FIX-PLAN-v1》F6 顺带发现①（改它 = 改明细行宽度行为，另刀另证）。
    return clip(stripCommandWrapping(raw, 40), 40);
  }
  if (tool === "subagent_run") return clip(str(a, "prompt"), 24);
  if (tool === "fs_grep") return clip(str(a, "query", "pattern"), 24);
  if (tool === "fetch") return clip(str(a, "url"), 40);
  if (tool.startsWith("memory_")) return clip(str(a, "key", "query"), 32);
  if (tool.startsWith("fs_") || tool.startsWith("edit_")) {
    const p = str(a, "path", "file", "key");
    if (p) return clip(baseName(p), 32);
  }
  if (tool.startsWith("git_")) return clip(str(a, "repo") || (Array.isArray(a.args) ? a.args.filter((v) => typeof v === "string").join(" ") : ""), 32);
  // 兜底：第一个有信息量的原始值
  for (const v of Object.values(a)) {
    if (typeof v === "string" && v.trim()) return clip(v.trim(), 32);
    if (typeof v === "number" || typeof v === "boolean") return String(v);
  }
  return "";
}

// 命令剥包装与工作区相对化的实现在 `lib/tool-args.ts`（v2.168 批 F-18 收敛：
// 与 tool-summary.ts 各写一遍的同族重复收成一份；本文件只用 `relativizeDetail`
// 这个"多留目录信息 + 剥 verbatim 前缀"的出口，折叠行那个裸文件名出口留在对面）。
