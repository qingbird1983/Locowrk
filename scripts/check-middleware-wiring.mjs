#!/usr/bin/env node
// 「存在但从未接线」的中间件门禁（审计报告 §7.2 / H2）。
// 编译器对「写了但从没装进链」完全不告警：clippy 全绿而 CtxCompact（778 行）
// 与 Retry 自 v2.137 桌面压缩链退役后就再没有生产构造点，是靠四个审计员
// 人工读出来的。这道闸把这个类别交给机器守：
//   ① 枚举 kernel-agent 里实现了 Middleware / WaterfallListener 的类型；
//   ② 在生产代码（非 tests 路径、非 #[cfg(test)] 段）里找它们的构造点
//      —— `NAME::new(` 或 `Arc::new([路径::]NAME)`；
//   ③ 找不到 = 未接线，必须显式写进下面的 KNOWN_UNWIRED 并附理由，否则判红。
// 白名单清空后，任何新写的未接线中间件当场亮红；白名单里的条目一旦被人
// 真接上了，本脚本也会亮红（提示删条目）——双向都不许漂。
//
// 用法：node scripts/check-middleware-wiring.mjs [--verbose]

import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const ROOT = process.argv[2] && !process.argv[2].startsWith("-") ? process.argv[2] : ".";
const verbose = process.argv.includes("--verbose");

// 未接线白名单（每项都要写清理由与去处）。
// 批 C（2026-10-11）删掉 CtxCompact + Retry 本体后清空 —— 清单为空 = 此后
// 任何新写却忘接线的中间件件当场判红，不再有豁免位。
const KNOWN_UNWIRED = new Map();

const walk = (dir, out = []) => {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    const st = statSync(p);
    if (st.isDirectory()) {
      if (name === "target" || name === "node_modules" || name === "dist") continue;
      walk(p, out);
    } else if (name.endsWith(".rs")) out.push(p);
  }
  return out;
};

const FILES = [...walk(join(ROOT, "crates")), ...walk(join(ROOT, "desktop", "src-tauri", "src"))];
// 测试面 = tests/ 或 test/ 目录，或文件名以 test 开头（tests.rs /
// tests_approval.rs / tests_plugins_root.rs 等本仓命名法）。
const isTestFile = (f) => {
  const rel = relative(ROOT, f).replace(/\\/g, "/");
  return /(^|\/)tests?(\/|$)/.test(rel) || /\/test[^/]*\.rs$/.test(rel);
};

// ① 声明清单
const kinds = new Map(); // name -> "Middleware" | "WaterfallListener"
for (const f of FILES) {
  if (!f.includes(join("kernel-agent", "src"))) continue;
  if (isTestFile(f)) continue; // 测试里的 Spy 之类假监听器不算 shipped 中间件
  const text = readFileSync(f, "utf8");
  for (const m of text.matchAll(/^impl\s+(?:<[^>]*>\s*)?(\w+)\s+for\s+([A-Z]\w*)\s*\{/gm)) {
    if (m[1] === "Middleware" || m[1] === "WaterfallListener") kinds.set(m[2], m[1]);
  }
}
if (kinds.size < 8) {
  console.error(`check-middleware-wiring: 只枚举到 ${kinds.size} 个监听器实现，判为解析失效`);
  process.exit(2);
}

// ② 生产构造点（跳过 #[cfg(test)] 起的整段：粗略但足够——段内构造不算接线）
const prodCtor = new Map();
for (const f of FILES) {
  if (isTestFile(f)) continue;
  const lines = readFileSync(f, "utf8").split("\n");
  let inCfgTestMod = false;
  for (const line of lines) {
    if (/^\s*#\[cfg\(test\)\]/.test(line)) {
      inCfgTestMod = true;
      continue;
    }
    if (inCfgTestMod && /^\}/.test(line)) inCfgTestMod = false;
    if (inCfgTestMod) continue;
    for (const [name] of kinds) {
      const re = new RegExp(`(\\b${name}::new\\s*\\(|new\\s*\\((?:[a-z_]+::)*${name}\\b)`);
      if (re.test(line)) {
        if (!prodCtor.has(name)) prodCtor.set(name, []);
        prodCtor.get(name).push(relative(ROOT, f).replace(/\\/g, "/"));
      }
    }
  }
}

// ③ 判定
let fail = 0;
const unwired = [...kinds.keys()].filter((n) => !prodCtor.has(n));
const stale = [...KNOWN_UNWIRED.keys()].filter((n) => prodCtor.has(n) || !kinds.has(n));

for (const n of unwired) {
  if (!KNOWN_UNWIRED.has(n)) {
    console.error(`check-middleware-wiring: ${n}（${kinds.get(n)} 实现）有定义、无生产构造点，且未登记白名单`);
    fail = 1;
  } else {
    console.log(`  白名单内未接线：${n} —— ${KNOWN_UNWIRED.get(n)}`);
  }
}
for (const n of stale) {
  console.error(
    `check-middleware-wiring: 白名单条目 ${n} 已过期（${
      kinds.has(n) ? "现在有生产构造点：" + prodCtor.get(n).join(", ") : "类型已不存在"
    }），请从 KNOWN_UNWIRED 删掉`,
  );
  fail = 1;
}

console.log(
  `check-middleware-wiring: 监听器实现=${kinds.size} · 已接线=${kinds.size - unwired.length}` +
    ` · 未接线=${unwired.length} · 白名单=${KNOWN_UNWIRED.size}`,
);
if (verbose)
  for (const [n, files] of prodCtor) console.log(`  ${n} ← ${[...new Set(files)].join(", ")}`);
process.exit(fail);
