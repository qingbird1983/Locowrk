// 工具明细的人话视图模型（UI shell doc §3.8 v4.13，《Tool Execution Process
// Display》规划）：折叠行的类型词/字形分派 + 展开明细的按工具定制渲染。
// 三条红线：不出现卡片、列对齐、默认明细零裸 JSON——字段名翻中文标签、
// 枚举翻中文、字节带单位、时间转相对时间、路径相对化；原始 JSON 只活在
// 「原始数据」折叠入口里（ToolDetailView 承担）。
// 台账第三节 · 第三批第一刀（612 → 本文件留主分派）：五个面沉进 `tool-detail/`，
// `types`（视图模型类型）· `kinds`（折叠行的类型词与字形）· `describe`（折叠行那句话 +
// 三个参数读取小件）· `labels`（值格式化与标签表）· `blocks`（明细块构造件），
// 本文件只剩"按工具类型分派"那条链。消费点 `import { toolKind, toolDescribe, toolDetail}
// from "../../lib/tool-detail"` 一字不改：三个函数与六个类型都由上面那段 import/export 接面
// （TS 没有 `mod` + `pub use`，对账里这一刀的"接上面"判的是"每个片名都被主文件引用一次"）。
// 片里导出而主文件**不再导出**的 15 个 helper 不算公开面——组件取它们要写全路径，现状无人取。
import { clip, relativeTimeMs, relativeTimeSec } from "./format";
import { relativizeDetail } from "./tool-args";

import {
  argsProps,
  codeLanguage,
  genericOutProps,
  prop,
  pushProps,
  shellDetail,
  textFieldOut,
  truncatedRow,
} from "./tool-detail/blocks";
import { asRecord, baseName, str } from "./tool-detail/describe";
import { fmtBytes, fmtEnum, fmtTime } from "./tool-detail/labels";
import type { DetailBlock, ListRow, ToolDetail, ToolDetailCtx } from "./tool-detail/types";

export { toolKind } from "./tool-detail/kinds";
export { toolDescribe } from "./tool-detail/describe";
export type {
  DetailBlock,
  ListRow,
  PropRow,
  ToolDetail,
  ToolDetailCtx,
  Tr,
} from "./tool-detail/types";

/** 主入口：按工具类型产出人话明细。失败轮（error）由调用方把 error 文本
 * 作为 danger text 块追加——参数属性行照常（看得见模型想干什么）。 */
export function toolDetail(
  tool: string,
  args: unknown,
  output: unknown,
  ctx: ToolDetailCtx,
): ToolDetail {
  const { tr, roots } = ctx;
  const a = asRecord(args);
  const o = asRecord(output);
  const blocks: DetailBlock[] = [];

  // 子代理独立通道（§3.10 紧凑行）：标签值行（任务/状态/轮次/工具/摘要）
  // + 「查看完整对话」入口；父侧拿不到的字段不渲染（⑮ 黑盒边界不变）
  if (tool === "subagent_run") {
    const status = typeof o.status === "string" ? o.status : "";
    const statusKey =
      status === "completed"
        ? "completed"
        : status === "budget_exhausted"
          ? "budgetExhausted"
          : status === "timeout"
            ? "timeout"
            : "failed";
    const childId = typeof o.child_session_id === "string" ? o.child_session_id : null;
    const tools = Array.isArray(o.tools_used)
      ? (o.tools_used as unknown[]).filter((x) => typeof x === "string").join(tr("tool.value.joiner"))
      : "";
    pushProps(blocks, [
      prop(tr("tool.label.task"), clip(str(a, "prompt"), 120)),
      prop(tr("tool.label.state"), tr(`tool.value.${statusKey}`), statusKey === "completed" ? "ok" : "danger"),
      typeof o.turns === "number" ? prop(tr("tool.label.rounds"), String(o.turns)) : null,
      tools ? prop(tr("tool.label.tools"), tools) : null,
      typeof o.summary === "string" && o.summary.trim()
        ? prop(tr("tool.label.summary"), clip(o.summary.trim(), 200))
        : null,
    ]);
    if (childId) blocks.push({ kind: "entry", childId, title: str(a, "prompt") });
    return { blocks };
  }

  const pathIn = str(a, "path", "file");

  if (tool === "fs_read") {
    const p = typeof o.path === "string" ? o.path : pathIn;
    const size = typeof o.size === "number" ? fmtBytes(o.size) : "";
    pushProps(blocks, [
      prop(tr("tool.label.path"), p ? relativizeDetail(p, roots) : "", "mono"),
      prop(tr("tool.label.size"), size),
      truncatedRow(o.truncated, tr),
    ]);
    const content = typeof o.content === "string" ? o.content : "";
    if (content) blocks.push({ kind: "code", language: codeLanguage(p), content });
    return { blocks };
  }

  if (tool === "fs_list") {
    const p = typeof o.path === "string" ? o.path : pathIn;
    pushProps(blocks, [prop(tr("tool.label.path"), p ? relativizeDetail(p, roots) : "", "mono")]);
    const entries = Array.isArray(o.entries) ? o.entries : [];
    if (entries.length > 0) {
      const items: ListRow[] = entries.map((e) => {
        const rec = asRecord(e);
        const name = typeof rec.name === "string" ? rec.name : "";
        const kind = typeof rec.kind === "string" ? rec.kind : "file";
        const size = typeof rec.size === "number" && rec.size > 0 ? fmtBytes(rec.size) : "";
        return {
          icon: kind === "dir" ? "folder" : kind === "link" ? "link" : "file",
          title: name,
          meta: kind === "dir" ? tr("tool.value.dir") : size || tr("tool.value.file"),
        };
      });
      blocks.push({ kind: "list", items });
      pushProps(blocks, [prop(tr("tool.label.count"), tr("tool.value.count", { n: entries.length }))]);
    }
    return { blocks };
  }

  if (tool === "fs_grep") {
    const dir = str(a, "path");
    pushProps(blocks, [
      prop(tr("tool.label.query"), str(a, "query", "pattern")),
      dir ? prop(tr("tool.label.path"), relativizeDetail(dir, roots), "mono") : null,
      typeof o.scanned === "number" ? prop(tr("tool.label.scanned"), String(o.scanned)) : null,
      truncatedRow(o.truncated, tr),
    ]);
    const matches = Array.isArray(o.matches) ? o.matches : [];
    if (matches.length > 0) {
      blocks.push({
        kind: "list",
        items: matches.slice(0, 50).map((m) => {
          const rec = asRecord(m);
          const f = typeof rec.file === "string" ? rec.file : "";
          const line = typeof rec.line === "number" ? String(rec.line) : "";
          const text = typeof rec.text === "string" ? clip(rec.text.trim(), 120) : "";
          return { icon: "file" as const, title: `${baseName(f)}:${line}`, meta: text };
        }),
      });
    }
    return { blocks };
  }

  if (tool === "fs_write" || tool === "edit_replace" || tool === "edit_lines" || tool === "edit_diff") {
    const p = typeof o.path === "string" ? o.path : pathIn;
    const bytes = typeof o.bytes === "number" ? fmtBytes(o.bytes) : "";
    const match = fmtEnum("match_type", o.match_type, tr);
    pushProps(blocks, [
      prop(tr("tool.label.path"), p ? relativizeDetail(p, roots) : "", "mono"),
      prop(tr("tool.label.size"), bytes),
      typeof o.edits === "number" ? prop(tr("tool.label.edits"), String(o.edits)) : null,
      match ? prop(tr("tool.label.matchType"), match) : null,
      typeof o.existed === "boolean" ? prop(tr("tool.label.existed"), tr(o.existed ? "tool.value.overwrite" : "tool.value.created")) : null,
    ]);
    const backup = typeof o.backup === "string" ? o.backup : "";
    if (backup) pushProps(blocks, [prop(tr("tool.label.backup"), relativizeDetail(backup, roots), "mono")]);
    return { blocks };
  }

  if (tool === "fs_delete" || tool === "fs_restore") {
    const p = typeof o.path === "string" ? o.path : pathIn;
    pushProps(blocks, [
      prop(tr("tool.label.path"), p ? relativizeDetail(p, roots) : "", "mono"),
      prop(tr("tool.label.restored"), tr("tool.value.yes"), "ok"),
    ]);
    return { blocks };
  }

  if (tool === "shell" || tool === "shell_read" || tool === "sandbox_run") {
    return { blocks: shellDetail(a, o, ctx) };
  }

  if (tool === "fetch") {
    pushProps(blocks, [
      prop(tr("tool.label.url"), str(o, "url") || str(a, "url"), "mono"),
      typeof o.status === "number" ? prop(tr("tool.label.status"), String(o.status), o.status < 400 ? "ok" : "danger") : null,
      prop(tr("tool.label.contentType"), str(o, "content_type")),
      truncatedRow(o.truncated, tr),
    ]);
    const body = typeof o.body === "string" ? o.body.trim() : "";
    if (body) blocks.push({ kind: "text", content: clip(body, 800) });
    return { blocks };
  }

  if (tool === "memory_write" || tool === "memory_read") {
    const scope = fmtEnum("scope", o.scope, tr);
    const updated =
      typeof o.updated_at === "number"
        ? fmtTime(relativeTimeSec(o.updated_at), tr)
        : "";
    const value = str(o, "value") || str(a, "value");
    pushProps(blocks, [
      prop(tr("tool.label.key"), str(o, "key") || str(a, "key"), "mono"),
      value ? prop(tr("tool.label.value"), clip(value, 200)) : null,
      prop(tr("tool.label.tags"), str(o, "tags") || str(a, "tags")),
      updated ? prop(tr("tool.label.updatedAt"), updated) : null,
      scope ? prop(tr("tool.label.scope"), scope) : null,
      typeof o.found === "boolean" ? prop(tr("tool.label.found"), tr(o.found ? "tool.value.yes" : "tool.value.no")) : null,
    ]);
    return { blocks };
  }

  if (tool === "memory_search" || tool === "memory_list") {
    pushProps(blocks, [prop(tr("tool.label.query"), str(a, "query"))]);
    const matches = Array.isArray(o.matches) ? o.matches : [];
    if (matches.length > 0) {
      blocks.push({
        kind: "list",
        items: matches.slice(0, 30).map((m) => {
          const rec = asRecord(m);
          return {
            icon: "tool" as const,
            title: typeof rec.key === "string" ? rec.key : "",
            meta: typeof rec.value === "string" ? clip(rec.value.trim(), 100) : "",
          };
        }),
      });
    }
    return { blocks };
  }

  if (tool === "search_tools") {
    pushProps(blocks, [prop(tr("tool.label.query"), str(a, "query"))]);
    const results = Array.isArray(o.results) ? o.results : [];
    if (results.length > 0) {
      blocks.push({
        kind: "list",
        items: results.slice(0, 20).map((r) => {
          const rec = asRecord(r);
          const name = str(rec, "name", "tool", "id");
          return { icon: "tool" as const, title: name, meta: clip(str(rec, "description"), 100) };
        }),
      });
    }
    const catalog = Array.isArray(o.catalog) ? o.catalog : [];
    if (catalog.length > 0) {
      blocks.push({
        kind: "list",
        items: catalog.filter((x) => typeof x === "string").slice(0, 30).map((x) => ({ icon: "tool" as const, title: x as string })),
      });
    }
    return { blocks };
  }

  if (tool.startsWith("schedule_")) {
    const runAt =
      typeof o.run_at === "number" ? fmtTime(relativeTimeMs(o.run_at), tr) : "";
    const kind = fmtEnum("kind", o.kind, tr);
    pushProps(blocks, [
      prop(tr("tool.label.id"), str(o, "id"), "mono"),
      kind ? prop(tr("tool.label.kind"), kind) : null,
      runAt ? prop(tr("tool.label.nextRun"), runAt) : null,
    ]);
    return { blocks };
  }

  if (tool.startsWith("git_")) {
    blocks.push(...argsProps(a, tr));
    const text = textFieldOut(o);
    if (text.length > 0) blocks.push(...text);
    else blocks.push(...genericOutProps(o, tr));
    return { blocks };
  }

  // 兜底（mcp_* / 表外插件 / 未识别输出）：参数与输出各一组键值属性行
  blocks.push(...argsProps(a, tr));
  blocks.push(...genericOutProps(o, tr));
  return { blocks };
}
