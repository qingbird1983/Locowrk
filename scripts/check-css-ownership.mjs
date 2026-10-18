#!/usr/bin/env node
/**
 * 样式落点唯一闸（标准 §二⑰「落点唯一（红线）」）。
 *
 * 判据：下面这些类名前缀的**定义**（`.vue` 的 `<style>` 块、或 `.css` 文件里的选择器）
 * 只许出现在唯一的归属文件里。别的文件可以**用**这个类名（模板上挂 `class`），
 * 但自己再定义一套 = 判错——那正是「同一属性两种形态」的机械成因：
 *
 *   `.prow__*`    → desktop/src/components/chat/ProcessRow.vue
 *   `.tdetail__*` → desktop/src/components/chat/ToolDetailView.vue
 *   `.tool__*`    → desktop/src/components/chat/ToolRow.vue
 *   `.lw-expand`  → desktop/src/style.css（思考行与工具行共用的唯一展开体外壳）
 *
 * 为什么要钉住：用户 2026-10-18 裁定「同一行为、同一属性要求统一」；行骨几何一旦被
 * 复制到第二个组件，下一次改动只会改一处（AF-01 三角位漂移、AF-17 两套展开体外壳
 * 都是这么长出来的）。注释里的类名不算——注释常常就是在解释「谁用谁」，先剥注释。
 *
 * 全局样式表 `style.css` 是跨组件的公共面（选择策略、滚动等），行骨三族在它里面也算
 * 合法落点；闸抓的是**另一个组件**复制了行骨、或组件抢了共用外壳的定义。
 *
 * 输出恒为一行 `KEY=值 ...`：
 *   PREFIXES    受管前缀数，必须 = 4
 *   VIOLATIONS  越界定义的条数，必须 = 0
 *   USES        用到共用外壳 `.lw-expand` 的组件数，必须 ≥ 1（0 = 死代码/欠账）
 *
 * 用法：
 *   node scripts/check-css-ownership.mjs             常规判据
 *   node scripts/check-css-ownership.mjs --map       打印「前缀 → 定义文件」实测表
 *   node scripts/check-css-ownership.mjs --selftest  内存变异自证，不碰磁盘
 */
import { readFileSync, readdirSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const SRC = join(root, "desktop/src");

/** 受管前缀 → 唯一归属文件（相对 `desktop/src/`，正斜杠）。 */
const OWNERS = [
  {
    label: ".prow__",
    owned: (t) => t.startsWith("prow__"),
    file: "components/chat/ProcessRow.vue",
    alsoGlobal: true,
  },
  {
    label: ".tdetail__",
    owned: (t) => t.startsWith("tdetail__"),
    file: "components/chat/ToolDetailView.vue",
    alsoGlobal: true,
  },
  {
    label: ".tool__",
    owned: (t) => t.startsWith("tool__"),
    file: "components/chat/ToolRow.vue",
    alsoGlobal: true,
  },
  { label: ".lw-expand", owned: (t) => t === "lw-expand", file: "style.css", alsoGlobal: false },
];

const GLOBAL_SHEET = "style.css";

/** 剥注释：块注释 + 行注释。样式表里没有含 `/*` 的字符串，直接正则即可。 */
function stripComments(src) {
  return src.replace(/\/\*[\s\S]*?\*\//g, " ").replace(/^\s*\/\/.*$/gm, " ");
}

/** 定义面：`.vue` 取所有 `<style>` 块内容，其余（`.css`）取全文。 */
function styleSurface(rel, text) {
  if (!rel.endsWith(".vue")) return text;
  const out = [];
  for (const m of text.matchAll(/<style[^>]*>([\s\S]*?)<\/style>/g)) out.push(m[1]);
  return out.join("\n");
}

/** 非定义面：拿掉 `<style>` 块，只剩模板/脚本——用来数「谁在用共用外壳」。 */
function usageSurface(rel, text) {
  if (!rel.endsWith(".vue")) return text;
  return text.replace(/<style[^>]*>[\s\S]*?<\/style>/g, " ");
}

/**
 * 判据主体：吃 `[{ rel, text }]`，吐结果对象；**不打印也不退出**（`--selftest` 要喂变异面）。
 * `rel` 一律相对 `desktop/src/`，正斜杠。
 */
function audit(files) {
  const seen = new Map(OWNERS.map((o) => [o.label, new Set()]));
  const violations = [];
  let uses = 0;

  for (const { rel, text } of files) {
    const style = stripComments(styleSurface(rel, text));
    const tokens = new Set([...style.matchAll(/\.([a-zA-Z][\w-]*)/g)].map((m) => m[1]));
    for (const owner of OWNERS) {
      if (![...tokens].some(owner.owned)) continue;
      seen.get(owner.label).add(rel);
      if (rel === owner.file) {
        // 归属文件自己定义 = 正解
      } else if (owner.alsoGlobal && rel === GLOBAL_SHEET) {
        // 全局样式表是跨组件的公共面（选择策略、滚动…），行骨三族在这里也合法。
      } else {
        violations.push(`${owner.label} 定义在 ${rel}（归属 ${owner.file}）`);
      }
    }
    if (rel !== GLOBAL_SHEET && /lw-expand/.test(stripComments(usageSurface(rel, text)))) uses++;
  }

  if (uses === 0) violations.push("`.lw-expand` 定义了却没人用（死代码，或外壳又被拆回组件里了）");

  return {
    counts: { PREFIXES: OWNERS.length, VIOLATIONS: violations.length, USES: uses },
    seen,
    violations,
  };
}

/** 实测表：前缀 → 定义文件（`--map` 用，排查「到底谁定义了它」）。 */
function printMap(result) {
  for (const [label, rels] of result.seen) {
    console.log(`${label}  ->  ${[...rels].sort().join(" | ") || "（无人定义）"}`);
  }
}

function report(result, quiet = false) {
  const c = result.counts;
  console.log(`PREFIXES=${c.PREFIXES} VIOLATIONS=${c.VIOLATIONS} USES=${c.USES}`);
  if (!result.violations.length) {
    if (!quiet) {
      console.log(
        `样式落点唯一：${c.PREFIXES} 个受管前缀各自只有一个定义文件，共用外壳 ${c.USES} 处在用。`,
      );
    }
    return true;
  }
  if (quiet) return false;
  console.error("\ncheck-css-ownership 未通过：");
  for (const v of result.violations) console.error("  ✗ " + v);
  return false;
}

/** 扫盘：`desktop/src` 下所有 `.vue` / `.css`。 */
function scanSrc() {
  const out = [];
  const walk = (dir) => {
    for (const d of readdirSync(dir, { withFileTypes: true })) {
      const full = join(dir, d.name);
      if (d.isDirectory()) walk(full);
      else if (d.name.endsWith(".vue") || d.name.endsWith(".css")) {
        out.push({ rel: relative(SRC, full).replaceAll("\\", "/"), text: readFileSync(full, "utf8") });
      }
    }
  };
  walk(SRC);
  return out;
}

/**
 * 变异表：每条喂一份合成文件表，声明它该报红还是该放行（`expectRed`）。
 * 判据与预期不符 = 该项零证明力或判错，自证失败。
 */
const PRow = "components/chat/ProcessRow.vue";
const TRow = "components/chat/ToolRow.vue";
const Detail = "components/chat/ToolDetailView.vue";
const Think = "components/chat/ThinkingRow.vue";
const CSS = "style.css";

const MUTATIONS = [
  {
    what: "行骨几何被复制到第二个组件",
    files: [
      { rel: PRow, text: "<style>.prow__desc{flex:1}</style>" },
      { rel: TRow, text: "<style>.prow__desc{flex:0 1 auto}</style>" },
      { rel: Think, text: '<template><div class="lw-expand"/></template>' },
    ],
    expectRed: true,
  },
  {
    what: "共用外壳被搬进组件里定义",
    files: [
      { rel: Think, text: "<style>.lw-expand{margin-left:7px}</style>" },
      { rel: TRow, text: '<template><div class="lw-expand"/></template>' },
    ],
    expectRed: true,
  },
  {
    what: "共用外壳定义了却没人用（死代码/欠账）",
    files: [{ rel: CSS, text: ".lw-expand{margin-left:7px}" }],
    expectRed: true,
  },
  {
    what: "各归其位、外壳有人在用 —— 放行",
    files: [
      { rel: Detail, text: "<style>.tdetail__prop{display:grid}</style>" },
      { rel: CSS, text: ".lw-expand{margin-left:7px}" },
      { rel: Think, text: '<template><div class="lw-expand"/></template>' },
    ],
    expectRed: false,
  },
  {
    what: "行骨进全局样式表（跨组件公共面）不算越界",
    files: [
      { rel: CSS, text: ".lw-expand{margin-left:7px}\n.tool__detail{user-select:text}" },
      { rel: TRow, text: '<template><div class="lw-expand"/></template>' },
    ],
    expectRed: false,
  },
  {
    what: "注释里提到类名不算定义",
    files: [
      {
        rel: TRow,
        text: '/* 与 .prow__desc 同源 */\n<template><div class="tool__desc"/></template>',
      },
      { rel: CSS, text: ".lw-expand{margin-left:7px}" },
      { rel: Think, text: '<template><span class="lw-expand"/></template>' },
    ],
    expectRed: false,
  },
];

if (process.argv.includes("--map")) {
  printMap(audit(scanSrc()));
  process.exit(0);
}

if (!process.argv.includes("--selftest")) {
  process.exit(report(audit(scanSrc())) ? 0 : 1);
}

// ---- --selftest：内存变异自证，不碰磁盘 ----
let allCaught = true;
for (const { what, files, expectRed } of MUTATIONS) {
  const result = audit(files);
  const red = report(result, true) === false;
  const ok = red === expectRed;
  console.log(`${ok ? "✓" : "✗"} ${what}${ok ? "" : " —— 与预期不符"}`);
  if (!ok) allCaught = false;
}
console.log(
  allCaught ? "样式落点闸自证通过：六项判据都按预期报红/放行。" : "样式落点闸自证未通过。",
);
process.exit(allCaught ? 0 : 1);
