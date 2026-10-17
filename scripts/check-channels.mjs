#!/usr/bin/env node
// 结构闸：事件通道对账（后审计队列 Q12·C1 立；闸序号 v2.209 起不再写——加一道就得全体重排）。
//
// 为什么必须新造而不是扩 `check-commands.mjs`：Q12 取证批实测（G7）——
// `check-commands.mjs` 里 `agent://` 与 `CHANNELS` 命中 **0**，它只管
// `#[tauri::command]` ↔ `generate_handler!` ↔ 前端 invoke 三向闭合，
// **事件通道至今没有任何机器对账**。api.ts 的 CHANNELS 是手抄清单，漏一条
// 就静默成死分支（v2.105 之前就真漏过 schedule-fired 一条，靠人眼发现）。
// 「UI 事件是会话日志的投影」是本仓纪律，但投影出来的事件有没有人接，没人盯。
//
// 四面（一条通道的一生）：
//   A events.rs 的 `channels::` 常量          —— 通道名的定义处
//   B AgentEvent::channel() 的 match 分支      —— 每个变体必须映到一个常量
//   C api.ts 的 CHANNELS 数组                  —— 前端订阅面
//   D stores 里的 `case "<kind>"`             —— 前端消费面
// 外加一张「有意不订阅 / 不消费」白名单，每条必须自带理由且必须实存。
//
// 两道「闸自己也得是活的」下限（同其他闸）：任一面的读数算成 0 就判红，
// 而不是 OK——扫描瞎掉长得跟一切正常一模一样。
import { readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..");
const R = (rel) => readFileSync(path.join(ROOT, rel), "utf8").replace(/\r\n/g, "\n");
const EV = "desktop/src-tauri/src/events.rs";
const API = "desktop/src/lib/api.ts";
const STORE_DIR = "desktop/src/stores";

let fail = 0;
const bad = (id, msg, detail = "") => {
  fail++;
  console.log(`CHANNEL_FAIL ${id} ${msg}${detail ? ` [${detail}]` : ""}`);
};
const good = (id, msg) => console.log(`CHANNEL_OK ${id} ${msg}`);

// ---------- A) 常量面 ----------
const ev = R(EV);
const modBlock = /pub mod channels \{([\s\S]*?)\n\}/.exec(ev);
if (!modBlock) {
  console.log("check-channels: BLOCK 取不到 `pub mod channels` 块（events.rs 形状变了）");
  process.exit(1);
}
const CONSTANTS = [...modBlock[1].matchAll(/pub const ([A-Z_]+): &str = "([^"]+)";/g)]
  .map((m) => ({ name: m[1], chan: m[2] }));
const BY_NAME = new Map(CONSTANTS.map((c) => [c.name, c.chan]));
const BY_CHAN = new Map(CONSTANTS.map((c) => [c.chan, c.name]));

// ---------- B) channel() 映射面 ----------
const implBlock = /impl AgentEvent \{([\s\S]*?)\n\}/.exec(ev);
if (!implBlock) {
  console.log("check-channels: BLOCK 取不到 `impl AgentEvent` 块");
  process.exit(1);
}
const MAPPED = [...implBlock[1].matchAll(/AgentEvent::([A-Za-z]+)[^=]*=> channels::([A-Z_]+),/g)]
  .map((m) => ({ variant: m[1], constant: m[2] }));

// 枚举变体面（同一文件里 enum 的顶层变体名，靠 serde tag 推 kind）
const enumBlock = /pub enum AgentEvent \{([\s\S]*?)\n\}/.exec(ev);
const VARIANTS = [...enumBlock[1].matchAll(/^ {4}([A-Z][A-Za-z]+)(\(| \{)/gm)].map((m) => m[1]);
const snake = (s) => s.replace(/([a-z0-9])([A-Z])/g, "$1_$2").toLowerCase();

// ---------- C) 前端订阅面 ----------
const api = R(API);
const chanBlock = /const CHANNELS = \[([\s\S]*?)\n\] as const;/.exec(api);
if (!chanBlock) {
  console.log("check-channels: BLOCK 取不到 api.ts 的 `const CHANNELS = [...] as const`");
  process.exit(1);
}
const SUBSCRIBED = [...chanBlock[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);

// ---------- D) 前端消费面（stores 目录全部 case 标签） ----------
const storeFiles = [];
const walk = (dir) => {
  for (const f of readdirSync(path.join(ROOT, dir))) {
    const rel = path.join(dir, f).split(path.sep).join("/");
    if (statSync(path.join(ROOT, rel)).isDirectory()) walk(rel);
    else if (f.endsWith(".ts")) storeFiles.push(rel);
  }
};
walk(STORE_DIR);
const HANDLED = new Set();
for (const rel of storeFiles) {
  for (const m of R(rel).matchAll(/case "([a-z_]+)":/g)) HANDLED.add(m[1]);
}

// ---------- 白名单：后端**永不 emit** 的通道，每条必须给理由且必须实存 ----------
// 判据一律"实算优先"：能在源码里找到 emit / listen 证据的，不进白名单。
// （本批第一版把 ASK_USER / ASK_RESOLVED / APPROVAL_REQUEST / PLUGIN_STATE 列进
//  白名单，实查 api.ts 才发现前两条**就在 CHANNELS 里**、后两条各有专用
//  `listen<...>()` ——白名单凭猜 = 把死分支洗白，第 147 条同族。）
const NEVER_EMIT = [
  ["USER_MESSAGE", "只在 session_history 全量投影产出，实时不 emit（events.rs 注释在场）"],
  ["TOOL_UNCERTAIN", "悬空只在崩溃后出现 ⇒ 只经重放数组回流，不 emit（同 USER_MESSAGE 纪律）"],
];
const NEVER_EMIT_NAMES = NEVER_EMIT.map(([n]) => n);
for (const [name, why] of NEVER_EMIT) {
  if (!BY_NAME.has(name)) bad("W1", `白名单引用了不存在的常量 ${name}（白名单自己漂了）`);
  if (!why || why.length < 12) bad("W2", `白名单条目 ${name} 的理由不够读`);
}

// 现算的两面证据：events.rs 里不经 AgentEvent 的直接 emit、api.ts 里的专用订阅。
const DIRECT_EMIT = new Set(
  [...ev.matchAll(/emit\(\s*channels::([A-Z_]+)/g)].map((m) => m[1]),
);
const DEDICATED_LISTEN = new Set(
  [...api.matchAll(/listen<[^>]*>\("([^"]+)"/g)].map((m) => m[1]),
);

// ---------- 下限：任何一面空转就判红 ----------
if (CONSTANTS.length < 20) bad("L1", "常量面读数过低（扫描瞎掉与一切正常长得一样）", CONSTANTS.length);
if (VARIANTS.length < 19) bad("L2", "变体面读数过低", VARIANTS.length);
if (SUBSCRIBED.length < 15) bad("L3", "订阅面读数过低", SUBSCRIBED.length);
if (HANDLED.size < 10) bad("L4", "消费面读数过低（stores 扫描没命中 case？）", HANDLED.size);

// ---------- R1 每个常量都得有 emit 出处（死常量 = 名字漂了） ----------
// 两条 legit 出处：经 AgentEvent::channel() 映射，或不经包装直接 emit
// （forward_plugin_states 就是后者——插件状态广播不是 AgentEvent）。
const usedConstants = new Set(MAPPED.map((m) => m.constant));
for (const name of BY_NAME.keys()) {
  if (usedConstants.has(name) || DIRECT_EMIT.has(name)) continue;
  bad("R1", `channels::${name} 既没被 channel() 映到、也没有直接 emit 出处`);
}

// ---------- R2 每个变体都得有 channel() 分支（编译器保证穷尽，这里核对条数） ----------
if (MAPPED.length !== VARIANTS.length) {
  bad("R2", "channel() 分支数与 enum 变体数不等", `${MAPPED.length} vs ${VARIANTS.length}`);
}
for (const v of VARIANTS) {
  if (!MAPPED.some((m) => m.variant === v)) bad("R2", `变体 ${v} 没有 channel() 分支`);
}

// ---------- R3 订阅的每条通道必须是真实常量值（防前端订阅幽灵通道） ----------
for (const ch of SUBSCRIBED) {
  if (!BY_CHAN.has(ch)) bad("R3", `api.ts CHANNELS 订阅了未定义的通道 ${ch}`);
}

// ---------- R4 每条映到的通道：要么被订阅，要么有专用订阅面，要么进白名单 ----------
const AGENT_ONLY = (c) => c.startsWith("agent://");
for (const m of MAPPED) {
  const ch = BY_NAME.get(m.constant);
  if (!AGENT_ONLY(ch)) continue; // plugin:// / lw:// 有自己的订阅面
  if (SUBSCRIBED.includes(ch) || DEDICATED_LISTEN.has(ch)) continue;
  if (NEVER_EMIT_NAMES.includes(m.constant)) continue;
  bad("R4", `${m.variant} → ${ch} 前端既不订阅也没有专用 listen（死分支形状）`);
}

// ---------- R5 被订阅且会 emit 的通道，前端必须真消费（kind 有 case） ----------
for (const m of MAPPED) {
  const ch = BY_NAME.get(m.constant);
  const reached = SUBSCRIBED.includes(ch) || DEDICATED_LISTEN.has(ch);
  if (!AGENT_ONLY(ch) || !reached) continue;
  const kind = snake(m.variant);
  // 专用订阅面的载荷直接进各自 store 的回调（approvals / asks / viewer…），
  // 不经 onEvent 开关 ⇒ 只核对「通用 CHANNELS 里的那一批必须有 case」。
  if (!SUBSCRIBED.includes(ch)) continue;
  if (!HANDLED.has(kind) && !NEVER_EMIT_NAMES.includes(m.constant)) {
    bad("R5", `通道 ${ch}（kind=${kind}）在 CHANNELS 里，但 stores 的开关表没有它`);
  }
}

// ---------- R6 ±N 的行数尺子只许一把（本批立的口径） ----------
const fsutil = R("crates/plugin-api/src/fsutil.rs");
const rulerDefs = (fsutil.match(/pub fn content_line_count/g) || []).length;
if (rulerDefs !== 1) bad("R6", "行数尺子的定义处不是唯一一处", rulerDefs);
for (const rel of ["plugins-src/tool-edit/src/replace.rs", "plugins-src/tool-fs/src/write.rs"]) {
  if (!R(rel).includes("content_line_count")) bad("R6", `${rel} 没用那把尺（自己 split 了一份）`);
}
// 反向：不许有文件绕开尺子自己数文本行数（`edit_lines` 数的是行数组，不是文本
// 行数，属另一语义，已在 projection 的注释里说明）。

console.log(
  `CHANNEL_CONSTANTS=${CONSTANTS.length} VARIANTS=${VARIANTS.length} MAPPED=${MAPPED.length} ` +
  `SUBSCRIBED=${SUBSCRIBED.length} HANDLED=${HANDLED.size} WHITELIST=${NEVER_EMIT_NAMES.length} ` +
  `STORES_SCANNED=${storeFiles.length} FAILED=${fail}`,
);
if (fail) {
  console.log("check-channels: FAILED");
  process.exit(1);
}
console.log("check-channels: OK");
process.exit(0);
