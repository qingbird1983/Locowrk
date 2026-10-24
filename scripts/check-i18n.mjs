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

import { readFileSync, readdirSync, statSync, existsSync } from "node:fs";
import { join, relative, dirname } from "node:path";

const ROOT = process.argv[2] && !process.argv[2].startsWith("-") ? process.argv[2] : ".";
const flags = new Set(process.argv.slice(ROOT === "." ? 2 : 3));
const SRC = join(ROOT, "desktop", "src");

// 路径一律归一成 `/` 再输出与比较：本仓在 Windows 上开发，`node:path` 的 relative() 给反斜杠形，
// 直接拼进提示会写出 `locales\zh-CN.ts`（本闸里既有的三处比较早就各自做了归一，新加的这几处同口径）。
const rel = (p) => relative(SRC, p).replace(/\\/g, "/");

// ---------- 语言包入口：**跟随装配点**，不硬编码文件名（批 F-22b）----------
// 旧写法 = `join(SRC, "locales", "zh-CN.ts")` 一条死路径。locales 拆成目录后那条路径不存在，
// parseLocale 会 ENOENT 直接**崩**（不是判红也不是放行，是没人看得懂的崩）；更糟的是若有人把
// 崩记成"闸没报错 = 通过"，这道闸就静默瞎了。现在改成从装配点 i18n.ts 读它真正 import 了什么，
// 单文件与目录两种形态都读得到 ⇒ 以后语言包再怎么挪，闸跟着装配点走。
const localeFiles = (id) => {
  const asm = join(SRC, "i18n.ts");
  if (!existsSync(asm)) throw new Error(`装配点不存在：${asm}`);
  const src = readFileSync(asm, "utf8");
  const re = new RegExp(`import\\s+(\\w+)\\s+from\\s+["'](\\./locales/${id}[^"']*)["']`);
  const m = re.exec(src);
  if (!m) throw new Error(`装配点里找不到 ${id} 的 import（i18n.ts 的装配形态变了，闸必须有人来看）`);
  const spec = join(SRC, m[2].replace(/^\.\//, ""));
  if (existsSync(spec) && statSync(spec).isFile()) return [spec];
  if (existsSync(spec + ".ts")) return [spec + ".ts"];
  const idx = join(spec, "index.ts");
  if (existsSync(idx)) {
    const dir = spec;
    const parts = readdirSync(dir).filter((n) => /\.ts$/.test(n) && n !== "index.ts").sort();
    if (!parts.length) throw new Error(`${dir}/ 是目录但里面没有域文件`);
    // **漏展开断言**：目录里的域文件必须逐个被 index.ts import 并展开。
    // 少了 import 或漏了 `...x` ⇒ 那一族的键静默消失；两语言同时漏同一族时，
    // zh/en 键集仍然相等、被引用键也不缺 ⇒ 除这条断言外无人能发现（静默丢文案）。
    const idxSrc = readFileSync(idx, "utf8");
    const imported = new Set([...idxSrc.matchAll(/import\s+(\w+)\s+from\s+["']\.\/([\w-]+)["']/g)].map((x) => x[2]));
    const spread = new Set([...idxSrc.matchAll(/\.\.\.(\w+)/g)].map((x) => x[1]));
    const notImported = parts.map((n) => n.replace(/\.ts$/, "")).filter((n) => !imported.has(n));
    const localNames = new Map([...idxSrc.matchAll(/import\s+(\w+)\s+from\s+["']\.\/([\w-]+)["']/g)].map((x) => [x[2], x[1]]));
    const notSpread = parts.map((n) => n.replace(/\.ts$/, "")).filter((n) => !spread.has(localNames.get(n)));
    if (notImported.length || notSpread.length) {
      throw new Error(`${dir}/index.ts 未纳入域文件：未 import ${notImported.join(",") || "无"} · 未展开 ${notSpread.join(",") || "无"}`);
    }
    // 解析面 = 域文件本身，**不含 index.ts**：index 的根对象成员是 `...chat` 展开而不是
    // `chat: {…}`，交给状态机会被当成嵌套对象解析出一条假键 `chat`（或崩）。index 的正确性
    // 由上面那条「未 import / 未展开」断言负责，不靠解析它的内容。
    return parts.map((n) => join(dir, n));
  }
  throw new Error(`装配点指向 ${m[2]}，但单文件与目录两种形态都不存在`);
};


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

// ---------- 目录形态合并（批 F-22b 新立两条断言）----------
// ①**命名域跨文件唯一**：拆完一个语言包 = N 个域文件由 `index.ts` 用对象展开合并，
//   同一个命名域出现在两个文件里时 `{...a, ...b}` **静默覆盖**——键集照样相等（两语言
//   同表）、被引用的键也不缺 ⇒ 现有三条检查一条都看不见它，只能在这里钉死。
// ②**整键不许重复**：同一条 `chat.greeting.title` 出现两次说明分组表把同一个域塞了两份。
// 两条都是 throw（走 exit 2 = 响亮失败），不记进 fail 位：这类是"包结构坏了"，
// 与"某语言缺个键"不是一档，混在一个退出码里会让人误读严重程度。
function parseLocalePackage(paths) {
  const keys = new Map();
  const owner = new Map(); // 顶层命名域 -> 出处文件
  for (const p of paths) {
    const k = parseLocale(p);
    for (const full of k.keys()) {
      const ns = full.split(".")[0];
      if (owner.has(ns) && owner.get(ns) !== p) {
        throw new Error(`命名域 \`${ns}\` 同时定义在 ${rel(owner.get(ns))} 与 ${rel(p)}`
          + `（index.ts 用展开合并 ⇒ 后一份静默覆盖前一份，语言包会少一整族文案）`);
      }
      if (!owner.has(ns)) owner.set(ns, p);
      if (keys.has(full)) throw new Error(`键 ${full} 在语言包里出现两次（${rel(p)}）`);
      keys.set(full, true);
    }
  }
  if (paths.length > 1 && keys.size === 0) throw new Error(`${paths.length} 个域文件解析出 0 条键 = 判据空转`);
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
const credited = new Set(); // 源码里的带引号点分字面量（**原始**收集；收紧为「真键」在语言包解析后做，T37）
const dynPrefixes = []; // 动态键前缀（'effect.' 这种）
const dynWhere = new Map(); // 前缀 → 出处文件（'死前缀'断言要点名，v2.168 批 F-18）
const dataKeys = new Set(); // settings-index.ts 的 labelKey / searchKeys

for (const file of walk(SRC)) {
  const text = readFileSync(file, "utf8");
  for (const m of text.matchAll(CALL_RE)) if (DOTTED_KEY_RE.test(m[2])) refs.add(m[2]);
  // 间接引用形态（键名写在对象表 / 数组里，运行时才喂给 t()）：台账口径把
  // 「源码里带引号出现过且能对上真实键」一律算引用。孤儿判定宁可少报——
  // 误报孤儿 = 诱导人删掉活文案。语言包自身不参与豁免统计。
  if (!rel(file).replace(/\\/g, "/").startsWith("locales/"))
    for (const m of text.matchAll(/(['"`])([a-z][A-Za-z0-9_]*(?:\.[A-Za-z0-9_]+)+)\1/g))
      credited.add(m[2]);
  // 动态前缀提取按台账口径**不限 t() 调用点**：`const path = \`approval.
  // sensitive.${key}\`` 这种「先拼串再传进 t()」的写法，只扫调用点会漏，
  // 漏了就是一批假孤儿（假孤儿比漏报危险——人会照着清单删活文案）。
  // 注释行不参与动态前缀扫描（v2.168 批 F-18）：本批给 tool-detail 写的缺陷说明里就带
  // `time.` 加占位符那种示例字面，扫到会把自己判红——**注释里的键名不是引用**。口径按行首
  // （双斜杠 / 块注释续行的星号 / 块注释起始），行尾注释里的示例属少数情况，登记为已知边界；
  // refs 与缺键那两路仍扫原文：真正的引用漏报比误报贵。
  const codeOnly = text
    .split("\n")
    .filter((l) => {
      const s = l.trimStart();
      return !(s.startsWith("//") || s.startsWith("*") || s.startsWith("/*"));
    })
    .join("\n");
  for (const m of codeOnly.matchAll(/`([a-z][A-Za-z0-9_.]*)\$\{/g)) {
    const prefix = m[1];
    if (prefix.includes(".")) {
      const p = prefix.endsWith(".") ? prefix : prefix + ".";
      dynPrefixes.push(p);
      // 死前缀断言按**原始前缀**记账（不补尾点）：`market.origin_${group.key}` 的真实键是
      // `market.origin_official`，补点就变成永远对不上的 `market.origin_.`——首跑两条假红
      // 之一就是它（另一条是注释里的示例，见 codeOnly）。豁免仍用补点形 = 旧行为不动。
      if (!dynWhere.has(prefix)) dynWhere.set(prefix, new Set());
      dynWhere.get(prefix).add(rel(file).replace(/\\/g, "/"));
    }
  }
  if (rel(file).replace(/\\/g, "/") === "lib/settings-index.ts") {
    for (const m of text.matchAll(/(?:labelKey|group):\s*(['"])([^'"]+)\1/g))
      if (DOTTED_KEY_RE.test(m[2])) dataKeys.add(m[2]);
    for (const m of text.matchAll(/searchKeys:\s*\[([^\]]*)\]/g))
      for (const s of m[1].matchAll(/(['"])([^'"]+)\1/g))
        if (DOTTED_KEY_RE.test(s[2])) dataKeys.add(s[2]);
  }
}

// ---------- 判定 ----------
let zh, en, zhFiles, enFiles;
try {
  zhFiles = localeFiles("zh-CN");
  enFiles = localeFiles("en");
  zh = parseLocalePackage(zhFiles);
  en = parseLocalePackage(enFiles);
} catch (e) {
  console.error(`check-i18n: 解析失败（不放行）：${e.message}`);
  process.exit(2);
}
// 包形态读数打出来 = 让"闸确实读到了目录里的 N 个域文件"这件事在门禁日志里可见
// （历批口径：读数必须说破，不能只报一个 891=891 就当作它知道文件长什么样）。
console.log(`check-i18n: 语言包形态 = zh ${zhFiles.length} 文件 / en ${enFiles.length} 文件`
  + `（${zhFiles.length > 1 ? rel(dirname(zhFiles[0])) + "/" : rel(zhFiles[0])}）`);

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
  // 报错教学里的路径必须来自**实际读到的位置**（批 A 的 M-10 同族：写死的提示在拆完目录后
  // 会指向一个不存在的文件，把人引到错地方）。
  console.error(`  → 在 ${rel(zhFiles[0])}${zhFiles.length > 1 ? "（及其同目录的域文件）" : ""}`
    + ` 与 ${rel(enFiles[0])} 同位补键，或改引用点键名。`);
}

// 「死前缀」断言（v2.168 批 F-18 新立）：上面的豁免只解决「键是拼出来的、静态扫不见」，
// 但**扫不见 ≠ 存在**。本仓 `lib/tool-detail.ts` 曾拼 `time.${r.key}`，而两份语言包里
// 一个 `time.*` 键都没有 ⇒ 工具明细的时间戳一直直显裸键名「time.hours」（与审计 H3b 同族，
// 而缺键检查 missing 看不见它、孤儿检查又因豁免放行——只有"前缀必须命中真实键"堵得住）。
const deadDyn = [...dynWhere.keys()].filter((p) => ![...zh.keys()].some((k) => k.startsWith(p)));
if (deadDyn.length) {
  fail = 1;
  console.error(`check-i18n: ${deadDyn.length} 个动态前缀在语言包里一个键都对不上（拼出的键名必然直显裸键名）`);
  for (const p of deadDyn) console.error(`  死前缀：\`${p}\${…}\` ← ${[...dynWhere.get(p)].join(", ")}`);
}

const covered = (k) => dynPrefixes.some((p) => k.startsWith(p));
// 间接豁免收紧（specs/03 T37）：旧口径把源码里**任何**带引号点分字面量都算进 credited，
// 模块路径（`` `labels.ts` `` 这类文件名）也混进来——读数被污染（T36 实测多出的集合全是
// 文件名；假键名对不上真实键，遮不住真孤儿，但数字不再是「引用了几处键」）。现在兑现
// 237 行注释本来写明的契约：只认**真能对上语言包键**的串。孤儿判定逐值等价——
// 豁免集只在「不是真键的串」上变小，而这类串从来豁免不到任何键（孤儿表只遍历真键）。
const creditedReal = new Set([...credited].filter((k) => zh.has(k) && en.has(k)));
const orphans = [...zh.keys()]
  .filter((k) => en.has(k) && !refs.has(k) && !dataKeys.has(k) && !creditedReal.has(k) && !covered(k))
  .sort();
console.log(
  `check-i18n: zh=${zh.size} en=${en.size} · 引用字面量键=${refs.size} + 索引键=${dataKeys.size}` +
    ` · 间接字面量豁免=${creditedReal.size}/${credited.size} · 动态前缀=${dynPrefixes.length}` +
    ` · 无静态引用键=${orphans.length}（报告项，删除与否按三问）`,
);
if (flags.has("--verbose-orphans")) for (const k of orphans) console.log(`  无引用：${k}`);
if (flags.has("--strict-orphans") && orphans.length) {
  console.error(`check-i18n: --strict-orphans 下 ${orphans.length} 个无引用键判红`);
  fail = 1;
}

process.exit(fail);
