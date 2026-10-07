// 工具明细的人话视图模型（UI shell doc §3.8 v4.13，《Tool Execution Process
// Display》规划）：折叠行的类型词/字形分派 + 展开明细的按工具定制渲染。
// 三条红线：不出现卡片、列对齐、默认明细零裸 JSON——字段名翻中文标签、
// 枚举翻中文、字节带单位、时间转相对时间、路径相对化；原始 JSON 只活在
// 「原始数据」折叠入口里（ToolDetailView 承担）。
import { clip, relativeTime } from "./format";
import type { IconName } from "./icons";

/** 渲染期翻译函数（调用方从 useI18n 传入，lib 层不持有 i18n 实例）。 */
export type Tr = (key: string, params?: Record<string, unknown>) => string;

export interface ToolDetailCtx {
  /** 数据根 + fsRoots（工作区相对化用，与折叠行摘要同一来源）。 */
  roots: string[];
  tr: Tr;
}

export interface PropRow {
  label: string;
  value: string;
  /** mono = 等宽（命令/路径/代码值）；ok = 绿；danger = 红；缺省 = 正文灰。 */
  tone?: "mono" | "ok" | "danger";
}

export interface ListRow {
  icon: "file" | "folder" | "link" | "tool";
  /** 主文本（文件名 / 文件:行号 / 工具名）。 */
  title: string;
  /** 次文本（大小 / 行内容摘引 / 键值），等宽小字。 */
  meta?: string;
}

export type DetailBlock =
  | { kind: "props"; rows: PropRow[] }
  /** 代码内容 → `.md` 代码块组件复用（语言标签头行 + 复制钮）。 */
  | { kind: "code"; language: string; content: string }
  | { kind: "text"; content: string; danger?: boolean }
  | { kind: "list"; items: ListRow[] }
  /** 子代理转录入口（查看完整对话文本钮）。 */
  | { kind: "entry"; childId: string; title: string };

export interface ToolDetail {
  blocks: DetailBlock[];
}

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
  sandbox_run: { type: "terminal", icon: "terminal" },
  fetch: { type: "web", icon: "globe" },
  memory_write: { type: "memory", icon: "database" },
  memory_read: { type: "memory", icon: "database" },
  memory_search: { type: "memory", icon: "database" },
  memory_list: { type: "memory", icon: "database" },
  search_tools: { type: "search", icon: "search" },
  subagent_run: { type: "subagent", icon: "bot" },
  notify: { type: "notify", icon: "bell" },
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

// ---------------------------------------------------------------------------
// 折叠行描述（一句话 ≤40 字，单行省略）
// ---------------------------------------------------------------------------

function baseName(path: string): string {
  const norm = path.replace(/\\/g, "/");
  return norm.split("/").filter(Boolean).pop() ?? norm;
}

function asRecord(v: unknown): Record<string, unknown> {
  return v !== null && typeof v === "object" ? (v as Record<string, unknown>) : {};
}

function str(a: Record<string, unknown>, ...keys: string[]): string {
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
  if (tool === "shell" || tool === "shell_read" || tool === "sandbox_run") {
    const raw =
      typeof a.command === "string"
        ? a.command
        : Array.isArray(a.args)
          ? a.args.filter((v) => typeof v === "string").join(" ")
          : "";
    if (!raw) return "";
    return clip(stripCommandWrapping(raw), 40);
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

/** 剥 cd 前缀 / bash -lc 包装，&& 串联只留首段（与旧 toolArgSummary 同规）。 */
function stripCommandWrapping(raw: string): string {
  let s = raw.trim();
  let hidden = 0;
  for (;;) {
    const cd = s.match(/^cd\s+(?:"[^"]*"|\S+)\s*&&\s*/);
    if (cd) {
      s = s.slice(cd[0].length);
      hidden++;
      continue;
    }
    const sh = s.match(/^(?:bash|sh)\s+(?:-l\w*\s+)*-c\s+"([\s\S]*)"\s*$/);
    if (sh) {
      s = sh[1]!.trim();
      continue;
    }
    break;
  }
  const parts = s.split(" && ");
  const first = parts[0]!.trim();
  hidden += Math.max(0, parts.length - 1);
  return clip(first, 40) + (hidden > 0 ? ` … +${hidden}` : "");
}

/** 工作区相对化（明细属性行用，比折叠行的裸文件名多留目录信息）；
 * 剥 Windows 扩展路径前缀 `\\?\`（插件 display 带它，纯显示噪声）。 */
function relativize(path: string, roots: string[]): string {
  const norm = path.replace(/\\\\\?\\/g, "").replace(/\\/g, "/");
  for (const root of roots) {
    const r = root.replace(/\\/g, "/").replace(/\/+$/, "");
    if (r && norm.toLowerCase().startsWith(r.toLowerCase() + "/")) {
      return norm.slice(r.length + 1);
    }
  }
  return norm;
}

// ---------------------------------------------------------------------------
// 值格式化
// ---------------------------------------------------------------------------

function fmtBytes(n: number): string {
  if (!Number.isFinite(n)) return String(n);
  if (n >= 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`;
  if (n >= 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${n} B`;
}

/** 枚举/布尔值翻中文（翻不出的原样返回，宁缺勿假）。 */
function fmtEnum(_key: string, v: unknown, tr: Tr): string | null {
  if (typeof v === "boolean") return tr(v ? "tool.value.yes" : "tool.value.no");
  if (typeof v !== "string") return null;
  const map: Record<string, string> = {
    dir: "dir",
    file: "file",
    link: "link",
    completed: "completed",
    budget_exhausted: "budgetExhausted",
    timeout: "timeout",
    failed: "failed",
    exact: "exact",
    fuzzy: "fuzzy",
    project: "project",
    global: "global",
    once: "once",
    delay: "delay",
    interval: "interval",
  };
  const hit = map[v];
  return hit ? tr(`tool.value.${hit}`) : null;
}

/** 时间戳转相对时间：秒级 unix（memory updated_at）；毫秒级（schedule run_at）。 */
function fmtTime(v: number, tr: Tr): string {
  const sec = v > 1e12 ? Math.floor(v / 1000) : v;
  const r = relativeTime(sec);
  return tr(`time.${r.key}`, { n: r.n });
}

/** 通用键名翻译（兜底渲染：翻得出就翻，翻不出保留原名）。 */
function labelFor(key: string, tr: Tr): string {
  const table: Record<string, string> = {
    path: "path",
    file: "path",
    size: "size",
    bytes: "size",
    kind: "kind",
    type: "kind",
    truncated: "truncated",
    query: "query",
    scanned: "scanned",
    matches: "matches",
    command: "command",
    program: "program",
    exit_code: "exit",
    success: "success",
    timed_out: "timedOut",
    stdout: "stdout",
    stderr: "stderr",
    url: "url",
    status: "status",
    content_type: "contentType",
    edits: "edits",
    match_type: "matchType",
    byte_range: "byteRange",
    backup: "backup",
    existed: "existed",
    key: "key",
    value: "value",
    tags: "tags",
    updated_at: "updatedAt",
    scope: "scope",
    found: "found",
    id: "id",
    run_at: "nextRun",
    deleted: "deleted",
    restored: "restored",
    removed: "removed",
    turns: "rounds",
    tools_used: "tools",
    summary: "summary",
    child_session_id: "session",
    note: "note",
    channel: "channel",
    catalog: "catalog",
    results: "results",
    entries: "entries",
    added: "added",
    removed_lines: "removed",
    lines_added: "addedLines",
    lines_removed: "removedLines",
  };
  const hit = table[key];
  return hit ? tr(`tool.label.${hit}`) : key;
}

// ---------------------------------------------------------------------------
// 明细视图模型（按工具类型定制，§3.8 渲染表）
// ---------------------------------------------------------------------------

function codeLanguage(path: string): string {
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

function pushProps(
  blocks: DetailBlock[],
  rows: (PropRow | null | undefined)[],
) {
  const clean = rows.filter((r): r is PropRow => !!r && r.value !== "");
  if (clean.length > 0) blocks.push({ kind: "props", rows: clean });
}

function prop(
  label: string,
  value: string,
  tone?: PropRow["tone"],
): PropRow | null {
  return value === "" ? null : { label, value, tone };
}

function truncatedRow(v: unknown, tr: Tr): PropRow | null {
  return v === true ? prop(labelFor("truncated", tr), tr("tool.value.yes"), "danger") : null;
}

/** shell / shell_read / sandbox_run 共用输出形状（exec.rs 同源）。 */
function shellDetail(a: Record<string, unknown>, o: Record<string, unknown>, ctx: ToolDetailCtx): DetailBlock[] {
  const { tr } = ctx;
  const blocks: DetailBlock[] = [];
  pushProps(blocks, [
    prop(tr("tool.label.command"), str(a, "command") || str(o, "command"), "mono"),
    prop(tr("tool.label.exit"), String(o.exit_code ?? ""), (o.exit_code ?? 0) === 0 ? "ok" : "danger"),
    o.timed_out === true ? prop(tr("tool.label.timedOut"), tr("tool.value.yes"), "danger") : null,
    truncatedRow(o.truncated, tr),
  ]);
  const stdout = typeof o.stdout === "string" ? o.stdout.trim() : "";
  const stderr = typeof o.stderr === "string" ? o.stderr.trim() : "";
  if (stdout) blocks.push({ kind: "text", content: stdout });
  if (stderr) blocks.push({ kind: "text", content: stderr, danger: true });
  return blocks;
}

/** 字符串型输出的通用出口（git_* / search_tools 等带 text/stdout/output 的）。 */
function textFieldOut(o: Record<string, unknown>): DetailBlock[] {
  for (const k of ["text", "stdout", "output", "content", "diff"]) {
    const v = o[k];
    if (typeof v === "string" && v.trim()) return [{ kind: "text", content: clip(v.trim(), 2000) }];
  }
  return [];
}

/** 对象输出 → 键值属性行（兜底渲染：跳过已知长文本字段与空值）。 */
function genericOutProps(o: Record<string, unknown>, tr: Tr): DetailBlock[] {
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

function argsProps(a: Record<string, unknown>, tr: Tr): DetailBlock[] {
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
      prop(tr("tool.label.path"), p ? relativize(p, roots) : "", "mono"),
      prop(tr("tool.label.size"), size),
      truncatedRow(o.truncated, tr),
    ]);
    const content = typeof o.content === "string" ? o.content : "";
    if (content) blocks.push({ kind: "code", language: codeLanguage(p), content });
    return { blocks };
  }

  if (tool === "fs_list") {
    const p = typeof o.path === "string" ? o.path : pathIn;
    pushProps(blocks, [prop(tr("tool.label.path"), p ? relativize(p, roots) : "", "mono")]);
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
      dir ? prop(tr("tool.label.path"), relativize(dir, roots), "mono") : null,
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
      prop(tr("tool.label.path"), p ? relativize(p, roots) : "", "mono"),
      prop(tr("tool.label.size"), bytes),
      typeof o.edits === "number" ? prop(tr("tool.label.edits"), String(o.edits)) : null,
      match ? prop(tr("tool.label.matchType"), match) : null,
      typeof o.existed === "boolean" ? prop(tr("tool.label.existed"), tr(o.existed ? "tool.value.overwrite" : "tool.value.created")) : null,
    ]);
    const backup = typeof o.backup === "string" ? o.backup : "";
    if (backup) pushProps(blocks, [prop(tr("tool.label.backup"), relativize(backup, roots), "mono")]);
    return { blocks };
  }

  if (tool === "fs_delete" || tool === "fs_restore") {
    const p = typeof o.path === "string" ? o.path : pathIn;
    pushProps(blocks, [
      prop(tr("tool.label.path"), p ? relativize(p, roots) : "", "mono"),
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
    const updated = typeof o.updated_at === "number" ? fmtTime(o.updated_at, tr) : "";
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
    const runAt = typeof o.run_at === "number" ? fmtTime(o.run_at, tr) : "";
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
