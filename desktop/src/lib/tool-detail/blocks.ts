/** **明细块构造件**（台账 T36）：把一组组值拼成 `DetailBlock` 的那些小工厂——语言标签
 * `codeLanguage`、三个装配件 `pushProps` / `prop` / `truncatedRow`、共用形状 `shellDetail`
 * （shell / shell_read / sandbox_run 同源）、通用出口 `textFieldOut` / `genericOutProps` / `argsProps`。
 *
 * 命名按产出物叫 `blocks`（不是"按工具叫"）：这一片里没有一个函数认得具体工具名之外的规则，
 * 认工具名的是主分派那条 if 链——把链和件混在一个文件里，就是"加一个工具要同时动两张表"的由来。
 * 值格式化与标签查表来自 labels 片，参数取值来自 describe 片，形状来自 types 片。 */

import { diffFromReplacement, parseUnifiedDiff, type DiffRow } from "../chat/diff";
import { clip } from "../format";

import { str } from "./describe";
import { fmtEnum, labelFor } from "./labels";
import type { DetailBlock, PropRow, ToolDetailCtx, Tr } from "./types";
// ---------------------------------------------------------------------------
// 明细视图模型（按工具类型定制，§3.8 渲染表）
// ---------------------------------------------------------------------------

export function codeLanguage(path: string): string {
  const ext = path.includes(".") ? path.split(".").pop()! : "";
  const map: Record<string, string> = {
    ts: "typescript",
    tsx: "tsx",
    js: "javascript",
    mjs: "javascript",
    vue: "vue",
    rs: "rust",
    py: "python",
    json: "json",
    toml: "toml",
    md: "markdown",
    html: "html",
    css: "css",
    sh: "bash",
    yml: "yaml",
    yaml: "yaml",
  };
  return map[ext.toLowerCase()] ?? (ext || "text");
}

export function pushProps(
  blocks: DetailBlock[],
  rows: (PropRow | null | undefined)[],
) {
  const clean = rows.filter((r): r is PropRow => !!r && r.value !== "");
  if (clean.length > 0) blocks.push({ kind: "props", rows: clean });
}

export function prop(
  label: string,
  value: string,
  tone?: PropRow["tone"],
): PropRow | null {
  return value === "" ? null : { label, value, tone };
}

export function truncatedRow(v: unknown, tr: Tr): PropRow | null {
  return v === true ? prop(labelFor("truncated", tr), tr("tool.value.yes"), "danger") : null;
}

/** shell / shell_read / sandbox_run 共用输出形状（exec.rs 同源）。设计稿 §7.3：
 * 正文形态是**终端块**（`$ 命令` + 合并输出，深底），参数属性行只留异常信号
 * （超时 / 截断）；退出码细节随属性行给（不单开一行，§7.3 口径）。 */
export function shellDetail(a: Record<string, unknown>, o: Record<string, unknown>, ctx: ToolDetailCtx): DetailBlock[] {
  const { tr } = ctx;
  const blocks: DetailBlock[] = [];
  const command = str(a, "command") || str(o, "command");
  pushProps(blocks, [
    prop(tr("tool.label.exit"), String(o.exit_code ?? ""), (o.exit_code ?? 0) === 0 ? "ok" : "danger"),
    o.timed_out === true ? prop(tr("tool.label.timedOut"), tr("tool.value.yes"), "danger") : null,
    truncatedRow(o.truncated, tr),
  ]);
  const stdout = typeof o.stdout === "string" ? o.stdout.trim() : "";
  const stderr = typeof o.stderr === "string" ? o.stderr.trim() : "";
  const output = [stdout, stderr].filter(Boolean).join("\n");
  if (command || output) blocks.push({ kind: "term", command, output });
  return blocks;
}

/** 写/编辑类工具的内容块（设计稿 §7.1）：有 unified diff 就解析；只有
 * old/new 两段文本（edit_replace）或行区间 + 新内容（edit_lines）就退化构造；
 * 拿不到旧文本（fs_write 整文覆盖）返回 null——整文写入没有差异可言，
 * 调用方改用代码块。 */
export function writeDiffRows(
  tool: string,
  a: Record<string, unknown>,
): DiffRow[] | null {
  if (tool === "fs_write") return null;
  const patch = str(a, "patch");
  if (patch) {
    const rows = parseUnifiedDiff(patch);
    if (rows) return rows;
  }
  if (tool === "edit_replace") {
    return diffFromReplacement(str(a, "old_string"), str(a, "new_string"), null);
  }
  if (tool === "edit_lines") {
    const start = typeof a.start_line === "number" ? a.start_line : null;
    return diffFromReplacement("", str(a, "content"), start);
  }
  return null;
}

/** 字符串型输出的通用出口（git_* / search_tools 等带 text/stdout/output 的）。 */
export function textFieldOut(o: Record<string, unknown>): DetailBlock[] {
  for (const k of ["text", "stdout", "output", "content", "diff"]) {
    const v = o[k];
    if (typeof v === "string" && v.trim()) return [{ kind: "text", content: clip(v.trim(), 2000) }];
  }
  return [];
}

/** 对象输出 → 键值属性行（兜底渲染：跳过已知长文本字段与空值）。 */
export function genericOutProps(o: Record<string, unknown>, tr: Tr): DetailBlock[] {
  const skip = new Set(["text", "stdout", "output", "content", "diff", "body"]);
  const rows: (PropRow | null)[] = [];
  for (const [k, v] of Object.entries(o)) {
    if (skip.has(k) || v === null || v === undefined) continue;
    if (typeof v === "string") {
      if (!v.trim()) continue;
      rows.push(prop(labelFor(k, tr), clip(v.trim(), 160), k === "path" ? "mono" : undefined));
    } else if (typeof v === "number" || typeof v === "boolean") {
      const e = fmtEnum(k, v, tr);
      rows.push(prop(labelFor(k, tr), e ?? (v === true ? tr("tool.value.yes") : v === false ? tr("tool.value.no") : String(v))));
    } else if (Array.isArray(v)) {
      rows.push(prop(labelFor(k, tr), tr("tool.value.count", { n: v.length })));
    }
  }
  const blocks: DetailBlock[] = [];
  pushProps(blocks, rows);
  return blocks;
}

export function argsProps(a: Record<string, unknown>, tr: Tr): DetailBlock[] {
  const skip = new Set(["content", "body", "patch", "text"]);
  const rows: (PropRow | null)[] = [];
  for (const [k, v] of Object.entries(a)) {
    if (skip.has(k) || v === null || v === undefined) continue;
    if (typeof v === "string") {
      if (!v.trim()) continue;
      rows.push(prop(labelFor(k, tr), clip(v.trim(), 160), k === "path" || k === "command" || k === "repo" ? "mono" : undefined));
    } else if (typeof v === "number" || typeof v === "boolean") {
      rows.push(prop(labelFor(k, tr), v === true ? tr("tool.value.yes") : v === false ? tr("tool.value.no") : String(v)));
    } else if (Array.isArray(v)) {
      rows.push(prop(labelFor(k, tr), v.filter((x) => typeof x === "string").map((x) => clip(x as string, 24)).join(" · ") || tr("tool.value.count", { n: v.length })));
    }
  }
  const blocks: DetailBlock[] = [];
  pushProps(blocks, rows);
  return blocks;
}
