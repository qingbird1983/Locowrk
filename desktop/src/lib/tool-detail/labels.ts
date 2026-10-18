/** **值格式化与标签表**（台账 T36）：字节数 `fmtBytes`、枚举/布尔翻中文 `fmtEnum`、
 * 时间戳 `fmtTime`（走 format 模块的 `relativeTimeLabel` 唯一落点）、通用键名翻译 `labelFor`。
 *
 * 这四件是"红线三条"里那两条的实现（字段名翻中文标签、枚举翻中文、字节带单位、时间转相对时间），
 * 与具体工具无关——按工具定制的都在 blocks 片与主分派里。`labelFor` 那张 60 行的表是
 * 明细与折叠行共用的**唯一**键名出处；`fmtTime` 头上那段"旧代码猜单位 / 拼了不存在的 time.* 前缀"
 * 的订正记录跟着搬，那是这段代码为什么长这样的凭据。 */

import { relativeTimeLabel, type RelativeTime } from "../format";
import type { Tr } from "./types";
// ---------------------------------------------------------------------------
// 值格式化
// ---------------------------------------------------------------------------

export function fmtBytes(n: number): string {
  if (!Number.isFinite(n)) return String(n);
  if (n >= 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`;
  if (n >= 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${n} B`;
}

/** 枚举/布尔值翻中文（翻不出的原样返回，宁缺勿假）。 */
export function fmtEnum(_key: string, v: unknown, tr: Tr): string | null {
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

/**
 * 时间戳 → 相对时间文案。v2.168 批 F-18 把三件事一次收干净：
 * ① 旧代码拼的是 `time.${key}`，而两份语言包**没有顶层 `time` 命名空间**（实测
 *    键面清单里就没有）⇒ 工具明细里的时间一直直显裸键名「time.hours」，与审计
 *    H3b 同族；它躲过了 `check-i18n` 是因为该闸对"动态拼的前缀"整族豁免——那条
 *    豁免本批补成断言（前缀必须在语言包里真实存在），见 §5 批 F-18 段。
 * ② 旧代码按 `v > 1e12` **猜**单位 ⇒ 改成调用点按数据源声明：memory.updated_at
 *    是秒（`memory.rs` 的 now_secs），schedule.run_at 是毫秒（`timeutil.rs:12`
 *    权威注释「sqlite run_at」+ `ScheduleView.run_at_ms`）。
 * ③ 渲染段与另外六处同形，收进 `relativeTimeLabel` 唯一落点。
 */
export function fmtTime(rt: RelativeTime, tr: Tr): string {
  return relativeTimeLabel(rt, tr);
}

/** 通用键名翻译（兜底渲染：翻得出就翻，翻不出保留原名）。 */
export function labelFor(key: string, tr: Tr): string {
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
