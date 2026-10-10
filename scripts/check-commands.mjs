#!/usr/bin/env node
// Tauri 命令三向闭合门禁（审计报告 §7.4）。
// 三向 = ① `#[tauri::command]` 定义  ② lib.rs 的 generate_handler! 注册表
//        ③ 前端 invoke("name")。
// 本次审计是**人工**比出「85 = 85 = 85 双向零漂移」的——这种闭合没有任何
// 东西在守，加一条命令忘了注册、或删了入口留下孤儿命令，都不会有任何东西
// 报警。判定：
//   ①↔② 必须完全相等（定义未注册 = 调不到；注册未定义 = 编译期即错，
//        但脚本层先看见，省得到 clippy 才炸）；
//   ③ 必须 ⊆ ②（前端调了不存在的命令 = 运行时 reject）；
//   ② 里有而 ③ 没有的 = 「后端有、前端无入口」，进 RESERVED 白名单才算过
//      （白名单为空时新增死命令直接判红，这是审计里 asset_gc /
//      llm_list_models / llm_probe_window 那类「包装壳活着、功能已死」形态
//      的机器防线）。
// 用法：node scripts/check-commands.mjs [--verbose]
// 判红 exit 1；解析不出注册表 = exit 2（绝不静默放行）。

import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const ROOT = process.argv[2] && !process.argv[2].startsWith("-") ? process.argv[2] : ".";
const verbose = process.argv.includes("--verbose");
const TAURI = join(ROOT, "desktop", "src-tauri", "src");
const FRONT = join(ROOT, "desktop", "src");

// ②「后端有、前端无」的显式预留名单（登记理由，随批清）。
const RESERVED_NO_FRONTEND = new Set([]);

const walk = (dir, re) => {
  const out = [];
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    const st = statSync(p);
    if (st.isDirectory()) {
      if (name === "target" || name === "node_modules" || name === "dist") continue;
      out.push(...walk(p, re));
    } else if (re.test(name)) out.push(p);
  }
  return out;
};

// ① 命令定义：`#[tauri::command]` 之后第一个 `fn <name>`（中间可能有
//   #[allow(..)] / 文档注释 / pub(async) 修饰，逐行往后找最稳）。
const defined = new Set();
for (const file of walk(TAURI, /\.rs$/)) {
  const lines = readFileSync(file, "utf8").split("\n");
  for (let i = 0; i < lines.length; i++) {
    if (!/#\[\s*tauri::command\b/.test(lines[i])) continue;
    for (let j = i + 1; j < lines.length && j < i + 12; j++) {
      const m = lines[j].match(/(?:pub\s+)?(?:async\s+)?fn\s+([a-z_][a-z0-9_]*)\s*\(/);
      if (m) {
        defined.add(m[1]);
        break;
      }
      if (/^\s*#\[\s*allow|^\s*\/\/|^\s*$|^\s*#\[serde/.test(lines[j])) continue;
      break; // 撞到别的东西 = 形态不认识，宁可漏报再由 ②↔① 差集兜住
    }
  }
}

// ② 注册表：lib.rs 的 tauri::generate_handler![ ... ]
const libPath = join(TAURI, "lib.rs");
const libText = readFileSync(libPath, "utf8");
const macroAt = libText.indexOf("generate_handler!");
if (macroAt < 0) {
  console.error("check-commands: lib.rs 里找不到 generate_handler!（解析失败，不放行）");
  process.exit(2);
}
let depth = 0;
let start = -1;
for (let i = macroAt + "generate_handler!".length; i < libText.length; i++) {
  const c = libText[i];
  if (c === "[") {
    if (depth === 0) start = i + 1;
    depth++;
  } else if (c === "]") {
    depth--;
    if (depth === 0) {
      const body = libText.slice(start, i);
      var registryText = body;
      break;
    }
  }
}
if (typeof registryText !== "string") {
  console.error("check-commands: generate_handler! 括号未配对（解析失败，不放行）");
  process.exit(2);
}
const registered = new Set(
  registryText
    .replace(/\/\/[^\n]*/g, "") // 行注释不参与切分
    .split(",")
    // 注册表写的是路径（`commands::agent_chat`），取末段命令名
    .map((s) => s.trim().split(/\s+/).pop().split("::").pop())
    .filter((s) => s && /^[a-z_][a-z0-9_]*$/.test(s)),
);

// ③ 前端引用：invoke("name") / invoke<T>("name")
const invoked = new Set();
for (const file of walk(FRONT, /\.(ts|vue)$/)) {
  const text = readFileSync(file, "utf8");
  for (const m of text.matchAll(/\binvoke(?:<[^>]*>)?\(\s*(['"`])([^'"`]+)\1/g))
    invoked.add(m[2]);
}

const diff = (a, b) => [...a].filter((x) => !b.has(x)).sort();
const defNotReg = diff(defined, registered);
const regNotDef = diff(registered, defined);
const invNotReg = diff(invoked, registered);
const regNotInv = diff(registered, invoked).filter((n) => !RESERVED_NO_FRONTEND.has(n));

let fail = 0;
const report = (label, list, hint) => {
  if (!list.length) return;
  fail = 1;
  console.error(`check-commands: ${label}（${list.length}）`);
  for (const n of list) console.error(`  ${n}`);
  if (hint) console.error(`  → ${hint}`);
};

report("定义了但没注册（前端调不到）", defNotReg, "在 desktop/src-tauri/src/lib.rs 的 generate_handler! 补登记");
report("注册了但没有定义（编译期即错）", regNotDef, "补回命令实现或从注册表摘掉");
report("前端 invoke 了不存在的命令", invNotReg, "改键名或在 Rust 侧补命令");
report("后端有命令、前端无入口（未登记 RESERVED）", regNotInv, "确属预留请加进脚本里的 RESERVED_NO_FRONTEND 并写明理由");

console.log(
  `check-commands: 定义=${defined.size} · 注册=${registered.size} · 前端引用=${invoked.size}` +
    ` · 预留名单=${RESERVED_NO_FRONTEND.size}`,
);
if (verbose) {
  console.log("定义全集：" + [...defined].sort().join(" "));
  console.log("前端未触达：" + [...diff(registered, invoked)].join(" "));
}
process.exit(fail);
