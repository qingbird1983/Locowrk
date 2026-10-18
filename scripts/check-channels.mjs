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
//   A 后端根里的 `pub mod channels` 常量       —— 通道名的定义处（整根定位，不钉文件名）
//   B AgentEvent::channel() 的 match 分支      —— 每个变体必须映到一个常量
//   C 前端 lib 根里的 CHANNELS 数组            —— 订阅面（同样整根定位）
//   D stores 里的 `case "<kind>"`             —— 前端消费面
// 外加一张「有意不订阅 / 不消费」白名单，每条必须自带理由且必须实存。
//
// 「闸自己也得是活的」共七档下限：四面读数各一档，再加三个**扫描根**的文件数一档（T23 新增）。
// 任一档掉下去就判红而不是 OK——扫描瞎掉长得跟一切正常一模一样，而钉死文件路径的旧版
// 一旦那个文件被拆成目录，它连"瞎了"都不会报，只会安静少一条（台账 T23 要堵的就是这个）。
import { readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..");
const R = (rel) => readFileSync(path.join(ROOT, rel), "utf8").replace(/\r\n/g, "\n");
// 扫描**根**，不是文件路径（台账 T23 · 2026-10-18）。旧写法把 `events.rs` 与 `api.ts` 两条
// 路径钉死，后果不是"拆文件不方便"，而是**锁死拆刀 + 安静少一条**：谁拆那两个文件，第一个
// 要动的是闸；而 `emit(channels::…` 这类证据一旦跟着 sink 搬出被钉死的那个文件，旧闸不会判红，
// 只会读不到 —— 假绿。现在四面都在整根里找，并新增「定义处恰好 1 个文件」这一档：
// 多一处 = 第二个真相，零处 = 闸瞎了，两种都响亮失败（不是返回 null 后炸在别处）。
// 三个根允许用环境变量覆盖，只为在自证里验证"根被换掉 ⇒ 定位必须 BLOCK"这条路径真的会走
// （一条永不生效的自我保护代码，和没写是同一件事 —— check-split-equivalence 同一条手法）。
const RUST_ROOT = process.env.CH_SCAN_RUST || "desktop/src-tauri/src";
const FRONT_ROOT = process.env.CH_SCAN_FRONT || "desktop/src/lib";
const STORE_DIR = process.env.CH_SCAN_STORES || "desktop/src/stores";

const filesUnder = (dir, ext) => {
  const out = [];
  const walk = (d) => {
    for (const f of readdirSync(path.join(ROOT, d))) {
      const rel = path.join(d, f).split(path.sep).join("/");
      if (statSync(path.join(ROOT, rel)).isDirectory()) walk(rel);
      else if (f.endsWith(ext)) out.push(rel);
    }
  };
  walk(dir);
  return out.sort();
};
/** 定位用的文本剥掉注释行：`const CHANNELS = [` 这种锚写进注释就把定义处判成 2 处
   （与 check-schema-parity 的「剥注释再比」同一条口径）。对账读数仍吃原文，见下。 */
const codeOf = (rel) => R(rel).split("\n").filter((l) => !/^\s*(\/\/|\/\/\/|#)/.test(l)).join("\n");
const rustFiles = filesUnder(RUST_ROOT, ".rs");
const frontFiles = filesUnder(FRONT_ROOT, ".ts");
const storeFiles = filesUnder(STORE_DIR, ".ts");

/** 在某扫描根里定位一个锚的唯一定义处，返回文件路径；命中数不等于期望就 BLOCK 退出。 */
const locate = (files, anchor, label, want = 1) => {
  const hits = files.filter((rel) => codeOf(rel).includes(anchor));
  if (hits.length !== want) {
    console.log(`check-channels: BLOCK ${label} 的定义处 = ${hits.length} 个文件（期望 ${want}）`
      + (hits.length ? `：${hits.join(" · ")}` : "（一个都没有 = 扫描根或锚变了）"));
    process.exit(1);
  }
  return hits[0];
};

let fail = 0;
const bad = (id, msg, detail = "") => {
  fail++;
  console.log(`CHANNEL_FAIL ${id} ${msg}${detail ? ` [${detail}]` : ""}`);
};
const good = (id, msg) => console.log(`CHANNEL_OK ${id} ${msg}`);

// ---------- A) 常量面 ----------
const evFile = locate(rustFiles, "pub mod channels {", "后端通道常量表");
const ev = R(evFile);
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
// enum 与 impl 分别定位（拆刀后它们可能分家，但同一张契约表的两半不能各自漂成两份）
const enumBlock = /pub enum AgentEvent \{([\s\S]*?)\n\}/.exec(
  R(locate(rustFiles, "pub enum AgentEvent {", "AgentEvent 枚举")),
);
if (!enumBlock) {
  console.log("check-channels: BLOCK 取不到 `pub enum AgentEvent` 块");
  process.exit(1);
}
const VARIANTS = [...enumBlock[1].matchAll(/^ {4}([A-Z][A-Za-z]+)(\(| \{)/gm)].map((m) => m[1]);
const implBlock = /impl AgentEvent \{([\s\S]*?)\n\}/.exec(
  R(locate(rustFiles, "fn channel(", "AgentEvent::channel 映射")),
);
if (!implBlock) {
  console.log("check-channels: BLOCK 取不到 `impl AgentEvent` 块（含 fn channel 的那个文件里找）");
  process.exit(1);
}
const MAPPED = [...implBlock[1].matchAll(/AgentEvent::([A-Za-z]+)[^=]*=> channels::([A-Z_]+),/g)]
  .map((m) => ({ variant: m[1], constant: m[2] }));

const snake = (s) => s.replace(/([a-z0-9])([A-Z])/g, "$1_$2").toLowerCase();

// ---------- C) 前端订阅面 ----------
const apiFile = locate(frontFiles, "const CHANNELS = [", "前端通道表");
const chanBlock = /const CHANNELS = \[([\s\S]*?)\n\] as const;/.exec(R(apiFile));
if (!chanBlock) {
  console.log(`check-channels: BLOCK 取不到 ${apiFile} 的 \`const CHANNELS = [...] as const\``);
  process.exit(1);
}
const SUBSCRIBED = [...chanBlock[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);

// ---------- D) 前端消费面（stores 根里全部 case 标签） ----------
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

// 现算的两面证据：不经 AgentEvent 的直接 emit、不经通用表的专用订阅。
// 这两面**整根扫**（旧版只扫被钉死的那一个文件）⇒ 证据搬到哪儿都数得到，这正是解锁要买的东
// 西：拆 `events.rs` 把 `forward_plugin_states` 挪进 sink 之后，旧闸会安静少一条而不是判红。
const DIRECT_EMIT = new Set();
for (const rel of rustFiles)
  for (const m of R(rel).matchAll(/emit\(\s*channels::([A-Z_]+)/g)) DIRECT_EMIT.add(m[1]);
const DEDICATED_LISTEN = new Set();
for (const rel of frontFiles)
  for (const m of R(rel).matchAll(/listen<[^>]*>\("([^"]+)"/g)) DEDICATED_LISTEN.add(m[1]);

// ---------- 下限：任何一面空转就判红 ----------
if (CONSTANTS.length < 20) bad("L1", "常量面读数过低（扫描瞎掉与一切正常长得一样）", CONSTANTS.length);
if (VARIANTS.length < 19) bad("L2", "变体面读数过低", VARIANTS.length);
if (SUBSCRIBED.length < 15) bad("L3", "订阅面读数过低", SUBSCRIBED.length);
if (HANDLED.size < 10) bad("L4", "消费面读数过低（stores 扫描没命中 case？）", HANDLED.size);
// 三个**扫描面**自己的下限（T23 新增）：定义处定位已经保证"锚命中数 = 1"，但根整个空掉时
// locate 会先 BLOCK；这三档是给"根还在、遍历条件被改坏"那种半瞎形状兜底的第四层。
if (rustFiles.length < 150) bad("L5", "后端扫描根文件数过低（目录改名或遍历条件坏掉）", rustFiles.length);
if (frontFiles.length < 15) bad("L6", "前端订阅根文件数过低", frontFiles.length);
if (storeFiles.length < 10) bad("L7", "stores 扫描根文件数过低", storeFiles.length);

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
  `STORES_SCANNED=${storeFiles.length} ` +
  // 出处报出来 = "闸究竟在哪儿读到这一面"在日志里可见（历批口径：读数必须说破）。
  // 注意这两条是**定位结果**不是配置项：拆刀把常量表搬到别的文件，这里跟着变，判据一行不用改。
  `DEF_IN=${evFile} , ${apiFile} RUST_SCANNED=${rustFiles.length} FRONT_SCANNED=${frontFiles.length} ` +
  `DIRECT_EMIT=${DIRECT_EMIT.size} DEDICATED=${DEDICATED_LISTEN.size} FAILED=${fail}`,
);
if (fail) {
  console.log("check-channels: FAILED");
  process.exit(1);
}
console.log("check-channels: OK");
process.exit(0);
