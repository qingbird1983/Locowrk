#!/usr/bin/env node
// i18n 完整性门禁（审计报告 H3/H3b + §7.3）。三件事：
//   ① zh / en 键集合双向相等（缺一边 = 该语言界面显裸键名）；
//   ② 反向缺键：源码 t()/$t()/te()/tm()/tr() 的字面量键必须在**两**语言包
//      里都存在，settings-index.ts 的 labelKey/searchKeys 同理（vue-tsc
//      完全看不见这类错——本次审计的 4 条用户可见缺陷全是这一类）；
//   ③ 无引用键报告：只报告不拦截（删除与否是人的判断，与 check_length
//      同一口径）。动态键前缀（t(`effect.${kind}`)）按前缀整族豁免。
// 判红口径：① ② 失败 exit 1；③ 恒为信息。解析失败 exit 2（绝不静默放行）。
//
// 用法：node scripts/check-i18n.mjs [--verbose-orphans] [--strict-orphans]
//   --strict-orphans = ③ 也判红（批 C 清完孤儿键后 CI 可启用）。

import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const ROOT = process.argv[2] && !process.argv[2].startsWith("-") ? process.argv[2] : ".";
const flags = new Set(process.argv.slice(ROOT === "." ? 2 : 3));
const SRC = join(ROOT, "desktop", "src");
const LOCALES = { zh: join(SRC, "locales", "zh-CN.ts"), en: join(SRC, "locales", "en.ts") };

// ---------- 语言包解析（token 级，认 prettier 折行）----------
// 历史教训：正则版 `key:` 后必须同行取值，把 prettier 换行取值判成缺键，
// 一口气假报了 18 个「zh 有 en 无」。这里改成小状态机。
function parseLocale(path) {
  const text = readFileSync(path, "utf8");
  const keys = new Map(); // "a.b.c" -> true
  const stack = [];
  let i = 0;

  const skipTrivia = () => {
    for (;;) {
      while (i < text.length && /\s/.test(text[i])) i++;
      if (text[i] === "/" && text[i + 1] === "/") {
        while (i < text.length && text[i] !== "\n") i++;
      } else if (text[i] === "/" && text[i + 1] === "*") {
        const end = text.indexOf("*/", i + 2);
        if (end < 0) throw new Error(`${path}: 未闭合块注释`);
        i = end + 2;
      } else return;
    }
  };

  const readString = () => {
    const q = text[i];
    if (q !== '"' && q !== "'" && q !== "`") throw new Error(`${path}:${line()} 期望字符串`);
    i++;
    let out = "";
    while (i < text.length) {
      const c = text[i];
      if (c === "\\") {
        out += text[i + 1] ?? "";
        i += 2;
        continue;
      }
      if (c === q) {
        i++;
        return out;
      }
      out += c;
      i++;
    }
    throw new Error(`${path}: 字符串未闭合`);
  };

  const line = () => text.slice(0, i).split("\n").length;
  const readIdent = () => {
    let out = "";
    while (i < text.length && /[A-Za-z0-9_$]/.test(text[i])) out += text[i++];
    return out;
  };

  // 跳过非字符串叶子（数组 / 对象外的一切表达式取值），括号配对即可。
  const skipValue = () => {
    let depth = 0;
    while (i < text.length) {
      const c = text[i];
      if (c === '"' || c === "'" || c === "`") {
        readString();
        continue;
      }
      if (c === "(" || c === "[") depth++;
      else if (c === ")" || c === "]") {
        if (depth === 0) return;
        depth--;
      } else if (c === "," && depth === 0) return;
      i++;
    }
    throw new Error(`${path}: 值未结束`);
  };

  skipTrivia();
  // 入口：跳过 `export default`
  if (text.startsWith("export", i) || text.slice(i).match(/^export\b/)) {
    i += "export".length;
    skipTrivia();
    if (text.slice(i).match(/^default\b/)) i += "default".length;
    skipTrivia();
  }
  if (text[i] !== "{") throw new Error(`${path}: 期望对象字面量起手 \`{\`，实得 ${JSON.stringify(text[i])}`);
  i++; // 吃进根对象的 `{`
  stack.push(null); // 根对象哨兵：pop 到它 = 整包解析结束

  for (;;) {
    skipTrivia();
    if (text[i] === "}") {
      i++;
      const popped = stack.pop(); // 弹到根哨兵 = 整包结束
      skipTrivia();
      if (text[i] === ",") i++;
      if (popped === null) break;
      continue;
    }
    if (text[i] === ",") {
      i++;
      continue;
    }
    let key;
    if (text[i] === '"' || text[i] === "'" || text[i] === "`") key = readString();
    else {
      key = readIdent();
      if (!key) throw new Error(`${path}:${line()} 解析不出键名（实得 ${JSON.stringify(text[i])}）`);
    }
    skipTrivia();
    if (text[i] !== ":") throw new Error(`${path}:${line()} 键 ${key} 后期望 ':'`);
    i++;
    skipTrivia();
    const full = [...stack.filter(Boolean), key].join(".");
    if (text[i] === "{") {
      i++;
      stack.push(key);
      continue;
    }
    if (text[i] === '"' || text[i] === "'" || text[i] === "`") {
      readString();
      keys.set(full, true);
    } else {
      skipValue();
      keys.set(full, true);
    }
    skipTrivia();
    if (text[i] === ",") i++;
  }
  if (stack.length !== 0) throw new Error(`${path}: 对象未闭合`);
  return keys;
}

// ---------- 源码引用收集 ----------
const walk = (dir) => {
  const out = [];
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    const st = statSync(p);
    if (st.isDirectory()) {
      if (name === "node_modules" || name === "dist") continue;
      out.push(...walk(p));
    } else if (/\.(vue|ts)$/.test(name)) out.push(p);
  }
  return out;
};

const CALL_RE = /(?:^|[^\w$.])(?:\$t|t|te|tm|tr)\(\s*(['"`])([^'"`]*)\1/g;
const DOTTED_KEY_RE = /^[a-z][A-Za-z0-9_]*(?:\.[A-Za-z0-9_]+)+$/;

const refs = new Set(); // t() 字面量键（缺键判红的依据）
const credited = new Set(); // 「任何形式的带引号点分字面量，且能解析到真实键」——只用于孤儿豁免
const dynPrefixes = []; // 动态键前缀（'effect.' 这种）
const dataKeys = new Set(); // settings-index.ts 的 labelKey / searchKeys

for (const file of walk(SRC)) {
  const text = readFileSync(file, "utf8");
  for (const m of text.matchAll(CALL_RE)) if (DOTTED_KEY_RE.test(m[2])) refs.add(m[2]);
  // 间接引用形态（键名写在对象表 / 数组里，运行时才喂给 t()）：台账口径把
  // 「源码里带引号出现过且能对上真实键」一律算引用。孤儿判定宁可少报——
  // 误报孤儿 = 诱导人删掉活文案。语言包自身不参与豁免统计。
  if (!relative(SRC, file).replace(/\\/g, "/").startsWith("locales/"))
    for (const m of text.matchAll(/(['"`])([a-z][A-Za-z0-9_]*(?:\.[A-Za-z0-9_]+)+)\1/g))
      credited.add(m[2]);
  // 动态前缀提取按台账口径**不限 t() 调用点**：`const path = \`approval.
  // sensitive.${key}\`` 这种「先拼串再传进 t()」的写法，只扫调用点会漏，
  // 漏了就是一批假孤儿（假孤儿比漏报危险——人会照着清单删活文案）。
  for (const m of text.matchAll(/`([a-z][A-Za-z0-9_.]*)\$\{/g)) {
    const prefix = m[1];
    if (prefix.includes(".")) dynPrefixes.push(prefix.endsWith(".") ? prefix : prefix + ".");
  }
  if (relative(SRC, file).replace(/\\/g, "/") === "lib/settings-index.ts") {
    for (const m of text.matchAll(/(?:labelKey|group):\s*(['"])([^'"]+)\1/g))
      if (DOTTED_KEY_RE.test(m[2])) dataKeys.add(m[2]);
    for (const m of text.matchAll(/searchKeys:\s*\[([^\]]*)\]/g))
      for (const s of m[1].matchAll(/(['"])([^'"]+)\1/g))
        if (DOTTED_KEY_RE.test(s[2])) dataKeys.add(s[2]);
  }
}

// ---------- 判定 ----------
let zh, en;
try {
  zh = parseLocale(LOCALES.zh);
  en = parseLocale(LOCALES.en);
} catch (e) {
  console.error(`check-i18n: 解析失败（不放行）：${e.message}`);
  process.exit(2);
}

const setDiff = (a, b) => [...a.keys()].filter((k) => !b.has(k)).sort();
const onlyZh = setDiff(zh, en);
const onlyEn = setDiff(en, zh);
const have = (k) => zh.has(k) && en.has(k);

let fail = 0;
if (onlyZh.length || onlyEn.length) {
  fail = 1;
  console.error(`check-i18n: zh/en 键集合不等（zh=${zh.size} en=${en.size}）`);
  for (const k of onlyZh) console.error(`  只在 zh：${k}`);
  for (const k of onlyEn) console.error(`  只在 en：${k}`);
}

const missing = [...refs, ...dataKeys].filter((k) => !have(k)).sort();
if (missing.length) {
  fail = 1;
  console.error(`check-i18n: ${missing.length} 个被引用的键在语言包里不存在（界面直显裸键名）`);
  for (const k of missing) console.error(`  缺：${k}`);
  console.error("  → 在 desktop/src/locales/{zh-CN,en}.ts 同位补键，或改引用点键名。");
}

const covered = (k) => dynPrefixes.some((p) => k.startsWith(p));
const orphans = [...zh.keys()]
  .filter((k) => en.has(k) && !refs.has(k) && !dataKeys.has(k) && !credited.has(k) && !covered(k))
  .sort();
console.log(
  `check-i18n: zh=${zh.size} en=${en.size} · 引用字面量键=${refs.size} + 索引键=${dataKeys.size}` +
    ` · 间接字面量豁免=${credited.size} · 动态前缀=${dynPrefixes.length}` +
    ` · 无静态引用键=${orphans.length}（报告项，删除与否按三问）`,
);
if (flags.has("--verbose-orphans")) for (const k of orphans) console.log(`  无引用：${k}`);
if (flags.has("--strict-orphans") && orphans.length) {
  console.error(`check-i18n: --strict-orphans 下 ${orphans.length} 个无引用键判红`);
  fail = 1;
}

process.exit(fail);
