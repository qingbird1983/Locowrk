/** 折叠行的**那一句话**（§3.18 加工规则在栅格形态下的收窄版）：`toolDescribe()` 按工具类型
 * 取"最该看见的那个值"——文件只显文件名、命令只显首命令、子代理 = 任务截断、检索 = 搜索词。
 *
 * 三个参数读取小件 `baseName` / `asRecord` / `str` 与它同时出现（主分派与 `blocks` 片也读），
 * 所以住在这一片：它们是"从 unknown 的 args 里稳妥取个值"这一件事，再拆就是为行数造结构。
 * `stripCommandWrapping` 的实现在 `lib/tool-args.ts`（v2.168 批 F-18 收成一份），这里只调它。
 * ⚠ 那句外层 `clip` 冗余是**改前既有形态**，注释里已登记 F6 顺带发现，本刀逐字保住不动它。 */

import { clip, clipName } from "../format";
import { stripCommandWrapping } from "../tool-args";
// ---------------------------------------------------------------------------
// 折叠行描述（一句话，单行省略）
// ---------------------------------------------------------------------------

/** 摘要文本的截断上限（**唯一口径**，台账 AF-15）：真正的截断由**实际可用宽度**
 * 决定（行壳 `.prow__desc` 可收缩 + 行内 `.tool__desc` 出省略号），这里只挡病态长值
 * （几 KB 的 patch / prompt）把行撑爆。旧口径按工具逐个写死 40/32/24，同一属性四种
 * 行为——用户 2026-10-18 驳回「同一属性两种形态」。 */
export const SUMMARY_CLIP = 40;

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
    // 内层已 clip 到 SUMMARY_CLIP，外层再 clip 会把 ` … +N` 后缀吃掉——这条冗余已
    // 登记为《AUDIT-FIX-PLAN-v1》F6 顺带发现①（改它 = 改明细行宽度行为，另刀另证）。
    return clip(stripCommandWrapping(raw, SUMMARY_CLIP), SUMMARY_CLIP);
  }
  if (tool === "subagent_run") return clip(str(a, "prompt"), SUMMARY_CLIP);
  if (tool === "fs_grep") return clip(str(a, "query", "pattern"), SUMMARY_CLIP);
  if (tool === "fetch") return clip(str(a, "url"), SUMMARY_CLIP);
  if (tool.startsWith("fs_") || tool.startsWith("edit_")) {
    const p = str(a, "path", "file", "key");
    // 中段省略保扩展名（旧 `clip` 会把 `.ts` 切掉，折叠行就看不出文件类型了）
    if (p) return clipName(baseName(p), SUMMARY_CLIP);
  }
  if (tool.startsWith("git_")) return clip(str(a, "repo") || (Array.isArray(a.args) ? a.args.filter((v) => typeof v === "string").join(" ") : ""), SUMMARY_CLIP);
  // 兜底：第一个有信息量的原始值
  for (const v of Object.values(a)) {
    if (typeof v === "string" && v.trim()) return clip(v.trim(), SUMMARY_CLIP);
    if (typeof v === "number" || typeof v === "boolean") return String(v);
  }
  return "";
}

// 命令剥包装与工作区相对化的实现在 `lib/tool-args.ts`（v2.168 批 F-18 收敛：
// 与 tool-summary.ts 各写一遍的同族重复收成一份；本文件只用 `relativizeDetail`
// 这个"多留目录信息 + 剥 verbatim 前缀"的出口，折叠行那个裸文件名出口留在对面）。
