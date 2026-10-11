#!/usr/bin/env node
/**
 * 第五道结构闸：拆分等价性守卫（v2.161 批 F-11 立，用户 2026-10-11 裁定「入库成第五道闸」）
 *
 * 为什么要有它：批 F-6…F-10 五刀的等价证明全写在 tmp/ 的一次性脚本里，跑完就没人再跑。
 * 批 F-10 的驱动自纠 (e) 就是这条边界的注脚——变异驱动把整个 `tree.rs` 压成一行，
 * 判据照样"红"，红得毫无意义。**证据链不入库，下一次没人会再跑它，也没人会发现它坏过。**
 *
 * 两种模式：
 *  ① 默认 = **形状不变量**（对活代码永远成立，可进 pre-commit 与 CI）
 *     S1 `mod` 声明 ↔ 兄弟文件双向闭合……文件写了没接线 = 那部分代码根本没参与编译，
 *        测试会"静默消失"（批 F-9/F-10 的 M8 就是这个形状）
 *     S2 测试目录的 `mod.rs` 内不许有用例……夹具与用例混住 = 拆分没做完
 *     S3 **私有项被兄弟簇引用**……Rust 私有项只对"定义模块及其后代"可见，兄弟簇看不见，
 *        这是编译器 `E0425` 的活，但本闸在编译之前就能点名（批 F-10 拆分器 U2 抓过 `ProbeTool`）
 *     S4 测试簇名不得与被测父模块已有的子模块同名……`super::wire::X` 会指向测试簇自己
 *        （批 F-9 首跑撞 `llm::wire`，E0255+E0425 共 12 条）
 *     S5 子簇引用了父模块私有项却没有 `use super::…` 引用句……Rust 允许"向上看"但要先
 *        把名字带进作用域（批 F-14 拆分器给每簇写 `use super::*;`，去掉它只有 cargo 会抓）
 *  ② `--split` = **一次性拆分等价证明**（七段，输入 = 原文 + 产物目录 + 归属登记表）
 *     供返工与新拆分使用；登记表随批入库（`scripts/split-ledger/*.json`），
 *     于是"这一刀怎么证明的"变成仓库里的东西，而不是对话里的话。
 *
 * 口径纪律（每条都对应一次真实踩坑）：
 *  - 判据消费的每份解析结果都打印条目数，并为它设硬闸：解析面为空 ⇒ 宣布相关段作废
 *  - 引用面匹配前**剥注释与字符串字面量**（批 F-6：注释里写了下沉项的名字 ⇒ "引用 0" 是假绿）
 *  - 计数型断言不许把"N 条 = 匹配面为空"和"N 条全部可解释"渲染成同一句 true
 *  - 归一化必须在两侧同文出现（批 F-9：拆分器去分号、判据没去 ⇒ 假红）
 */
import fs from 'node:fs';
import path from 'node:path';
import { execSync } from 'node:child_process';

const ROOT = process.cwd();
// 扫描根允许用环境变量覆盖，只为验证"扫到 0 个模块 ⇒ 判红"这条空转检测路径真的会走
// （一条永不生效的自我保护代码，和没有写是同一件事）。
const SCAN = (process.env.SPLIT_SCAN_ROOT || 'crates,desktop/src-tauri/src,plugins-src').split(',');
const SKIP = [/\/target\//, /\/node_modules\//, /\/dist\//];
const argv = process.argv.slice(2);
// `--dbg-tests`：打印每个候选父文件的判定过程。判据"静默不命中"是最难查的红（本函数
// 第一版就把三种父文件形状写错成两种，表现为测试目录名单少 3 个而不报任何错）。
const DBG = argv.includes('--dbg-tests');
const arg = (k, d) => { const i = argv.indexOf(k); return i >= 0 ? argv[i + 1] : d; };

/* ---------------- 单元模型（拆分器与判据共用同一口径，两侧同文） ---------------- */
const DECL = /^(?:(pub(?:\([^)]*\))?)\s+)?(async\s+)?(const\s+|static\s+)?(fn|struct|enum|impl|type|trait|mod|use)\b/;
const ATTACH = /^(\/\/\/|#\[|#!\[)/;

/**
 * 词法剥离器（逐字符状态机，输出**等长**字符串，非换行字符换空格）。
 * 为什么不用正则：正则版先漏了 raw string（`r#"{"choices":…"#`，把批 F-9 的 18 条用例数成 15 条），
 * 补完 raw 又漏字符字面量与嵌套块注释——**用正则去逼近一门语言的文法，漏多少种形态就报多少假信号**。
 * 覆盖：行注释 / **可嵌套**块注释 / 普通字符串（含转义、可跨行）/ raw 字符串 `r"…"`,`r#"…"#`,`r##…`
 *       / 字节串 `b"…"` / 字符字面量 `'x'` `'\n'`（而 `&'a str` 的生命周期不算定界符）。
 */
function stripCommentsStrings(src) {
  const out = [];
  const n = src.length;
  let i = 0;
  const blank = (s) => { for (const ch of s) out.push(ch === '\n' ? '\n' : ' '); };
  while (i < n) {
    const c = src[i], d = src[i + 1];
    if (c === '/' && d === '/') { const j = src.indexOf('\n', i); const seg = src.slice(i, j < 0 ? n : j); blank(seg); i += seg.length; continue; }
    if (c === '/' && d === '*') { // 嵌套块注释
      let depth = 0, j = i;
      while (j < n) { if (src[j] === '/' && src[j + 1] === '*') { depth++; j += 2; continue; } if (src[j] === '*' && src[j + 1] === '/') { depth--; j += 2; if (!depth) break; continue; } j++; }
      blank(src.slice(i, j)); i = j; continue;
    }
    if ((c === 'r' || c === 'b') && (d === '"' || (d === '#' && src.slice(i).match(/^r#{1,6}"/)))) {
      const m = /^([rb])(#{0,6})"/.exec(src.slice(i));
      if (m) { const hashes = m[2]; let j = i + m[0].length; const close = '"' + hashes; const k = src.indexOf(close, j); j = k < 0 ? n : k + close.length; blank(src.slice(i, j)); i = j; continue; }
    }
    if (c === '"') { let j = i + 1; while (j < n) { if (src[j] === '\\') { j += 2; continue; } if (src[j] === '"') { j++; break; } j++; } blank(src.slice(i, j)); i = j; continue; }
    if (c === "'") { // 字符字面量 = 引号 + (转义序列 | 单字符) + 引号；否则是生命周期，原样保留
      const m = /^'(?:\\.|.)'/.test(src.slice(i, i + 6)) ? /^'(?:\\.|[^'])'/.exec(src.slice(i)) : null;
      if (m) { blank(m[0]); i += m[0].length; continue; }
      out.push(c); i++; continue;
    }
    out.push(c); i++;
  }
  return out.join('');
}
const stripCache = new Map();
const blankOnce = (src) => { let v = stripCache.get(src); if (v === undefined) { v = stripCommentsStrings(src); stripCache.set(src, v); } return v; };
/**
 * 剥离可信度自检：剥完之后**代码里的花括号净额必须为 0**（合法 Rust 文件必配平）。
 * 不为 0 就说明还有定界符形态没被正确处理，此时 units() 的结果不可信。
 * （本闸第一版的自检写成"剥离前后净额相等"——那是把剥离的目的写反了：注释里本来就可能有不配对的花括号。）
 */
function stripReliable(src) {
  const d = (s) => (s.match(/\{/g) || []).length - (s.match(/\}/g) || []).length;
  const after = d(stripCommentsStrings(src));
  return after === 0 ? '' : `剥离后花括号净额 = ${after}（代码花括号必须配平 ⇒ 有定界符形态未被处理，units() 不可信）`;
}
const baseName = (n) => { const m = /^impl\s+\S+\s+for\s+(\S+)$/.exec(n); return m ? m[1] : (/^impl\s+(\S+)$/.test(n) ? n.replace(/^impl\s+/, '') : n); };
const norm = (s) => s.replace(/\s+/g, ' ').trim();

/** 源码 → 单元列表（含属性与前置 doc 注释；块体按大括号配平，`;` 结尾算单行项） */
function units(srcRaw) {
  const src = blankOnce(srcRaw); // 注释与字符串不再参与结构判断（含 `// mod x;` 这种被注释掉的声明）
  const lines = src.split(/\r?\n/);
  const out = [];
  for (let i = 0; i < lines.length; i++) {
    const m = DECL.exec(lines[i]);
    if (!m) continue;
    let s = i;
    while (s - 1 >= 0 && ATTACH.test(lines[s - 1])) s--;
    let depth = 0, body = false, j = i;
    for (; j < lines.length; j++) {
      const o = (lines[j].match(/\{/g) || []).length, c = (lines[j].match(/\}/g) || []).length;
      if (o) body = true;
      depth += o - c;
      if (body && depth <= 0) break;
      if (!body && /;\s*$/.test(lines[j])) break;
    }
    const kw = m[4];
    const vis = m[1] ? m[1].replace(/\s+/g, '') : '';
    const decl = lines[i];
    let name;
    const im = /^impl(?:\s*<[^>]*>)?\s+([\w:]+)(?:\s+for\s+([\w:]+))?/.exec(decl);
    if (kw === 'impl' && im) name = im[2] ? `impl ${im[1]} for ${im[2]}` : `impl ${im[1]}`;
    else if (kw === 'use') name = lines.slice(i, j + 1).join(' ').replace(/;+\s*$/, '').replace(/\s+/g, ' ').trim();
    else if (kw === 'mod') name = (/^[\w\s<>:'"()]*\bmod\s+([\w:]+)/.exec(decl) || [])[1] || `?${decl.slice(0, 30)}`;
    else {
      const mm = /^(?:(?:pub(?:\([^)]*\))?)\s+)?(?:async\s+)?(?:const\s+|static\s+)?(fn|struct|enum|trait|type)\s+([\w:]+)/.exec(decl);
      name = mm ? mm[2] : `?${decl.slice(0, 30)}`;
    }
    const head = lines.slice(s, i).join('\n');
    out.push({
      from: s, decl: i, end: j, kw, vis, name,
      isTest: /#\[tokio::test\]|#\[test\]/.test(head),
      body: lines.slice(s, j + 1).join('\n'),
    });
    i = j;
  }
  return out;
}

/** 目录里的兄弟：mod.rs + 若干 `x.rs` / `x/mod.rs` */
function siblings(dir) {
  const files = fs.readdirSync(dir, { withFileTypes: true });
  const clusters = [];
  for (const f of files) {
    if (f.isFile() && f.name.endsWith('.rs') && f.name !== 'mod.rs') clusters.push(f.name.replace(/\.rs$/, ''));
    else if (f.isDirectory() && fs.existsSync(path.join(dir, f.name, 'mod.rs'))) clusters.push(f.name);
  }
  return clusters.sort();
}
const readCluster = (dir, c) => {
  const flat = path.join(dir, `${c}.rs`);
  if (fs.existsSync(flat)) return fs.readFileSync(flat, 'utf8');
  return fs.readFileSync(path.join(dir, c, 'mod.rs'), 'utf8');
};
/** mod 声明表：名字 → 声明处的可见性/形态（`mod x;` / `pub mod x;` / 内联 `mod x {`） */
function declaredMods(modSrc) {
  const map = new Map();
  for (const u of units(modSrc)) if (u.kw === 'mod') map.set(u.name, u);
  return map;
}

/* ---------------- 模式 ①：形状不变量 ---------------- */
function shapeGate() {
  const dirs = [];
  const walk = (d) => {
    if (SKIP.some((r) => r.test(d.replace(/\\/g, '/')))) return;
    let st;
    try { st = fs.statSync(d); } catch { return; }
    if (!st.isDirectory()) return;
    if (fs.existsSync(path.join(d, 'mod.rs'))) dirs.push(d);
    for (const f of fs.readdirSync(d, { withFileTypes: true })) if (f.isDirectory()) walk(path.join(d, f.name));
  };
  for (const root of SCAN) {
    const abs = path.join(ROOT, root);
    if (fs.existsSync(abs)) walk(abs);
  }
  // `crates/x/tests/common/` 这类集成测试共享夹具目录：没有父模块声明它们，S1/S4 不适用
  const isIntegrationCommon = (d) => /[\\/]tests[\\/]common$/.test(d);
  /**
   * 目录的父级声明是否 `#[cfg(test)]`。父模块文件有**三种形状**，少一种就漏判（本函数
   * 第一版只写了前两种，`kernel/tests/`、`service/tests/`、`settings/tests/` 当场掉出
   * 测试目录名单——批 F-13 的 S4 候选列表是同一个坑的第一次）：
   *   a) `../<父目录名>.rs`——Rust 2018 兄弟文件式父模块：`src/kernel.rs` + `src/kernel/tests/`
   *   b) `../mod.rs`——目录式父模块：`src/organize/mod.rs` + `src/organize/tests/`
   *   c) `../lib.rs` / `../main.rs`——crate 根下的模块：`src/lib.rs` 里 `mod tests_session_log;`
   * 查不到声明 = 保守判否：误报的代价（批 F-11 边界①的 10 条假红）高于漏报。
   */
  const declHasCfgTest = (d) => {
    const name = path.basename(d);
    const parentDir = path.dirname(d);            // src/kernel/tests → src/kernel
    const parentName = path.basename(parentDir);  // kernel
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(name)) return false;
    for (const f of [
      // a) 兄弟文件式父模块：`src/kernel.rs` 声明 `mod tests;`，子文件放 `src/kernel/`
      //    （注意它在 parentDir **的上一级**——写成 parentDir/kernel.rs 就是第三次踩这个坑）
      path.join(parentDir, '..', `${parentName}.rs`),
      // b) 目录式父模块：`src/organize/mod.rs`
      path.join(parentDir, 'mod.rs'),
      // c) crate 根：`src/lib.rs` / `src/main.rs`
      path.join(parentDir, 'lib.rs'),
      path.join(parentDir, 'main.rs'),
    ]) {
      if (!fs.existsSync(f)) continue;
      for (const u of units(fs.readFileSync(f, 'utf8'))) {
        if (u.kw === 'mod' && u.name === name) {
          const hit = /#\[\s*cfg\s*\(\s*test\b/.test(u.body);
          if (DBG) console.log(`   [dbg] ${name} ← ${path.relative(ROOT, f)} body 首行=`, JSON.stringify(u.body.split(/\r?\n/)[0]), `⇒ cfg(test)=${hit}`);
          return hit;
        }
      }
      if (DBG) console.log(`   [dbg] ${name} ← ${path.relative(ROOT, f)} 有该文件但 units 里查无此 mod 名`);
    }
    return false;
  };

  let s0 = 0, s1 = 0, s2 = 0, s3 = 0, s4 = 0, s5 = 0, checked = 0, testDirs = 0;
  const observed = []; // 撞名但当前无歧义的形状：只报告不判红（见 S4 注释）
  const testList = []; // S2/S3/S4 管辖名单——口径漂移必须当场可见（批 F-9「显示层不许藏口径差」）
  const bad = (msg) => console.log('   !! ' + msg);
  for (const dir of dirs) {
    checked++;
    const modSrc = fs.readFileSync(path.join(dir, 'mod.rs'), 'utf8');
    const decl = declaredMods(modSrc);
    const sib = siblings(dir);
    const rel = path.relative(ROOT, dir).replace(/\\/g, '/');
    // 测试目录判据（批 F-14 立）= **父级声明处是否 `#[cfg(test)] mod <目录名>;`**。
    // 语言事实：cfg(test) 让整棵子树只存在于测试构建，其内的形状（夹具与用例混住、
    // 簇名撞被测子模块、私有项跨簇引用）才是 S2/S3/S4 该管的面。
    // 两次踩坑都写在这儿，别再走回头路：
    //  ① 旧口径按目录名字面 `tests` ⇒ `tests_approval/`、`tests_session_log/` 这批
    //     cfg(test) 主题测试模块整片不判（本刀就是 `tests_approval/`）；
    //  ② 中间态改成"目录里出现 `#[test]`"的内容判据 ⇒ `scheduler/`（生产目录，
    //     `engine.rs` 里 `for sched in deps.db.due(now)` 的**局部变量**撞了
    //     `scheduler/tests.rs` 夹具名 `sched`）被判成测试目录，S3 一次造出 **10 条假红**。
    //     批 F-11 边界①「生产目录里 `\bname\b` 会误报，宁可窄而准」就是这条。
    const isTests = !isIntegrationCommon(dir) && declHasCfgTest(dir);
    if (isTests) { testDirs++; testList.push(rel); }
    // S0 先验尺子：剥离注释/字符串**不该**改变花括号净额；变了就说明这个文件里有某种定界符
    // 没被正确处理（本闸第一版就栽在 raw string `r#"{"choices":…}"#` 上，把 18 条用例数成 15 条）。
    // 尺子坏 ⇒ 该文件判据作废并判红，绝不允许"坏尺子量出零偏差"。
    for (const [f, src] of [['mod.rs', modSrc], ...sib.map((c) => [`${c}.rs`, readCluster(dir, c)])]) {
      const why = stripReliable(src);
      if (why) { s0++; bad(`${rel}/${f}：${why}`); }
    }

    // S1 双向闭合
    if (!isIntegrationCommon(dir)) {
      for (const c of sib) if (!decl.has(c)) { s1++; bad(`${rel}/${c}.rs 存在但没有 \`mod ${c};\` 声明（不参与编译）`); }
      for (const [name, u] of decl) {
        const childDir = path.join(dir, name);
        const hasFile = fs.existsSync(path.join(dir, `${name}.rs`)) || (fs.existsSync(childDir) && fs.existsSync(path.join(childDir, 'mod.rs')));
        const inline = /\{/.test(u.body.replace(/\/\/[^\n]*/g, '')) && !/;\s*$/.test(u.body.trim().split(/\r?\n/).pop());
        if (!hasFile && !inline) { s1++; bad(`${rel}/mod.rs 声明了 \`mod ${name};\` 但查无文件`); }
      }
    }
    // S2 测试目录的 `mod.rs` 内不许有用例（夹具与用例混住 = 拆分没做完）。
    // **历史基线**（与 check_length 的 BASELINE 同一手法：只拦新增，基线只能降不能升）：
    //  · `settings/tests` 6 条 = 批 F-11 立闸时实测（v2.58 时代形状，该文件 253 行）；
    //  · `tests_session_log` 6 条 = **批 F-14 扩识别口径后才第一次看到它**（它是
    //    `#[cfg(test)] mod tests_session_log;`，旧口径按目录名字面 `tests` 匹配不上，
    //    这道闸从没判过它）。该文件 336 行未超 400 线，把用例搬进子簇属另一刀，
    //    不在本批顺手扩大改动面。
    const KNOWN_MIXED = {
      'desktop/src-tauri/src/settings/tests': 6,
      'desktop/src-tauri/src/tests_session_log': 6,
    };
    if (isTests) {
      const inMod = [...units(modSrc)].filter((u) => u.isTest).length;
      const want = KNOWN_MIXED[rel] || 0;
      if (inMod > want) {
        s2 += inMod - want;
        for (const u of units(modSrc).filter((x) => x.isTest)) bad(`${rel}/mod.rs 里住着用例 ${u.name}（夹具与用例不许混住；基线 ${want}）`);
      } else if (inMod < want) {
        console.log(`   ~ ${rel} 混住用例已降到 ${inMod} < 基线 ${want}：顺手收紧 KNOWN_MIXED`);
      }
    }
    // S3 私有项被兄弟簇引用 —— **只对测试目录生效**（这里的名字都是本地夹具，字段名/宏名冲突面小），
    // 且只取"真正的定义点"（struct/enum/trait/type/fn/const）：`impl X` 没有可见性是语言事实，
    // 把它当定义点会把 `pub struct X` 误报成私有项（第一版就在这里造出 30+ 条误报）。
    // 宿主在 `mod.rs` ⇒ 该父模块的私有项对子簇可见（Rust：私有项对定义模块及其后代可见），不判。
    if (isTests) {
      const clusterSrcs = sib.map((c) => ({ c, src: readCluster(dir, c) }));
      const hostDefs = [];
      for (const { c, src } of [{ c: 'mod', src: modSrc }, ...clusterSrcs]) {
        for (const u of units(src)) {
          if (u.isTest || u.kw === 'mod' || u.kw === 'use' || u.kw === 'impl') continue;
          if (!/^(fn|struct|enum|trait|type|const|static)$/.test(u.kw)) continue;
          hostDefs.push({ c, name: u.name, vis: u.vis });
        }
      }
      for (const d of hostDefs) {
        if (d.c === 'mod' || d.vis) continue;
        const bare = baseName(d.name);
        if (!/^[A-Za-z_]\w*$/.test(bare)) continue;
        const re = new RegExp(`\\b${bare}\\b`);
        for (const { c, src } of clusterSrcs) {
          if (c === d.c) continue;
          const stripped = stripCommentsStrings(src);
          for (const u of units(stripped)) {
            if (u.kw === 'use' || u.kw === 'mod' || u.kw === 'impl') continue;
            if (u.name === d.name) continue; // 自身定义不算引用
            if (re.test(u.body)) {
              s3++; bad(`${rel}: 私有项 \`${bare}\`（宿主 ${d.c}）被 ${c}/${u.name} 引用 ⇒ 兄弟簇看不见它（E0425 的成因；处置 = 上收 mod.rs 或提 pub(super)，别改正文）`);
            }
          }
        }
      }
    }
    // S5 「子簇引用父模块私有项但没有引用句」= `E0425` 的另一半成因（批 F-14 立）。
    // S3 管"兄弟簇之间看不见"；S5 管"向上看"：Rust 语言事实 = 子模块**可以**用父模块的
    // 私有项，但必须有一条 `use super::…`（glob 或具名）把名字带进作用域。
    // 批 F-14 的拆分器给每个簇写 `use super::*;`（共享夹具靠它），删掉这句只有 cargo 会抓
    // —— 而"要编译过才知道"正是本闸要消掉的成本（批 F-10 的 M9/M10 就是这个形态）。
    if (isTests) {
      const modDefs = [...units(modSrc)]
        .filter((u) => !u.vis && !u.isTest && /^(fn|struct|enum|trait|type|const|static)$/.test(u.kw))
        .map((u) => baseName(u.name))
        .filter((n) => /^[A-Za-z_]\w*$/.test(n));
      for (const c of sib) {
        if (!modDefs.length) break;
        const src = readCluster(dir, c);
        if (/^\s*use\s+super::/m.test(src)) continue; // 有引用句（glob 或具名都算）
        const stripped = stripCommentsStrings(src);
        const hit = modDefs.filter((n) => new RegExp(`(?<!::)\\b${n}\\b`).test(stripped));
        if (hit.length) { s5++; bad(`${rel}/${c}.rs 引用父模块私有项 ${hit.slice(0, 3).join(', ')} 但没有 \`use super::…\` ⇒ E0425（补 glob 或具名引用句，别改正文）`); }
      }
    }
    // S4 测试簇名不得与被测父模块已有的子模块同名 —— 但**撞名本身不是缺陷**，
    // 只有真的产生歧义才红（批 F-9 首跑 `wire` 撞 `llm::wire` 就是这两种形态之一）：
    //  (i) `tests/mod.rs` 里另有 `use super::<name>` 绑定 ⇒ 与 `mod <name>;` 在同一定义两次 = `E0255`
    //  (ii) 任一簇正文出现 `super::<name>…` 路径 ⇒ 解析到测试簇自己 = `E0425`
    // 只撞名、两条都不触发的（organize/tests 的 recovery/summary）记为观察项不判红——
    // 判据一旦开始误报历史形状，人们就会绕过它（项目口径：一级判错不可绕过 ⇒ 闸必须准）。
    if (isTests) {
      const parentDir = path.dirname(dir);
      const parentName = path.basename(parentDir);
      // 父模块文件有两种布局：目录式（`parent/mod.rs`）与 Rust 2018 的**兄弟文件式**
      // （`parent.rs` + `parent/`）—— `desktop/src-tauri/src/settings.rs` 配
      // `desktop/src-tauri/src/settings/tests/` 正是后者，它的父模块文件在 tests 的
      // **上一级**目录里。第一版只找前者 ⇒ `parentFile` 取不到 ⇒ **S4 整段静默跳过**
      // （修好后 `settings/tests` 的 llm/chat/memory/tools/appearance/developer/window
      //  七个簇名全部撞 `settings.rs` 的 mod 名单 ⇒ 集体进观察项 = 漏检被暴露成读数暴增；
      //  这条自纠本身就是"闸入库了也照样会写漏一种形态"的现场证据）。
      const parentCandidates = [
        path.join(parentDir, `${parentName}.rs`),
        path.join(parentDir, 'mod.rs'),
        path.join(parentDir, '..', `${parentName}.rs`),
      ];
      const parentFile = parentCandidates.find((p) => fs.existsSync(p));
      if (parentFile && parentFile !== path.join(dir, 'mod.rs')) {
        const pmods = new Set([...declaredMods(fs.readFileSync(parentFile, 'utf8')).keys()]);
        const sibSrcs = sib.map((c) => ({ c, src: readCluster(dir, c) }));
        for (const c of sib) {
          if (!pmods.has(c)) continue;
          const useBind = new RegExp(`use\\s+super::${c}\\b|use\\s+super::\\{[^}]*\\b${c}\\b`).test(modSrc);
          // 路径形态必须排掉 `super::super::<name>` —— 那是**合法的两级上跳**（指回父模块的
          // 子模块），而 `\bsuper::<name>\b` 会在它的后半段上命中 ⇒ 假红。本闸修完父模块
          // 布局漏检后立刻在 `settings/tests/{developer,tools}.rs` 上造出两条假红（现役代码、
          // 编译与 438 测试全绿），靠负向后行断言才分清"歧义"与"本来就该这么写"。
          const pathUse = sibSrcs.some(({ src }) => new RegExp(`(?<!super::)\\bsuper::${c}\\b`).test(stripCommentsStrings(src)));
          if (useBind || pathUse) {
            s4++; bad(`${rel}: 测试簇名 \`${c}\` 与被测父模块 ${parentName} 的子模块同名，且${useBind ? '`tests/mod.rs` 里有 `use super::' + c + '`（E0255）' : '簇正文出现 `super::' + c + '::`（E0425）'} ⇒ 改名或按职责重命名`);
          } else observed.push(`${rel}/${c}`);
        }
      }
    }
  }
  console.log(`check-split-equivalence: 目录式模块 ${dirs.length} 个（测试目录 ${testDirs} 个）`);
  console.log(`  测试目录名单（= S2/S3/S4 管辖面，判据 = 父声明带 #[cfg(test)]）：${testList.join(' · ') || '（空）'}`);
  console.log(`  S0 解析面不可靠文件 = ${s0} | S1 mod 声明↔文件闭合偏差 = ${s1} | S2 测试 mod.rs 内用例 = ${s2} | S3 私有项跨簇引用 = ${s3} | S4 簇名撞父模块子模块 = ${s4} | S5 子簇缺 \`use super::\` 引用句 = ${s5}`);
  // 命中统计必须打出来：判据扫到 0 个对象时"零偏差"是空转不是干净（批 F-8 教训）。
  console.log(`  S4 观察项（撞名但当前无歧义引用；正文一旦出现 \`super::同名\` 即判红）= ${observed.length}${observed.length ? ': ' + observed.join(', ') : ''}`);
  const total = s0 + s1 + s2 + s3 + s4 + s5;
  if (!checked) { console.log('  !! 扫到 0 个目录式模块 ⇒ 本闸空转（判据失效，不是"没有偏差"）'); return 1; }
  return total ? 1 : 0;
}

/* ---------------- 模式 ②：一次性拆分等价证明（七段） ---------------- */
function splitGate() {
  const ledgerPath = arg('--ledger');
  if (!ledgerPath) { console.log('--split 需要 --ledger <scripts/split-ledger/x.json>（登记 = {ref, orig, dir, clusters, map, head, registeredNew, cargoPrefix}）'); return 2; }
  const led = JSON.parse(fs.readFileSync(ledgerPath, 'utf8'));
  const origSrc = execSync(`git show ${led.ref}:${led.orig}`, { encoding: 'utf8', maxBuffer: 64e6 });
  const rok = stripReliable(origSrc);
  if (rok) { console.log('   !! 原文不可解析：' + rok + ' ⇒ 本登记表判据全部作废'); return 1; }
  const L = (s) => s.split(/\r?\n/);
  const orig = L(origSrc);
  const ou = units(origSrc);
  const tests = ou.filter((u) => u.isTest);
  const fixtures = ou.filter((u) => !u.isTest && u.kw !== 'mod' && u.kw !== 'use'); // 与产物侧同口径（两侧口径不一致就是批 F-9 教训 (a) 的复发）
  const claimed = led.map;
  const unitKeys = ou.map((u) => norm(u.name.replace(/;+$/, '')));
  unitKeys.forEach((n) => { if (/^(pub )?use\b/.test(n)) claimed[n] = 'mod'; });
  const distinct = new Set(unitKeys).size;
  const unowned = unitKeys.filter((n) => !claimed[n]);
  const bad = [];
  const say = (k, v) => console.log(`${k} = ${v}`);
  say('登记表条目', `${Object.keys(claimed).length}（去重单元名 ${distinct}，单元 ${ou.length} 个含同名）`);
  say('   无归属单元', unowned.length + (unowned.length ? ' → ' + unowned.join(', ') : ''));
  // 登记表必须**双向**核对（批 F-9 教训 (b)：只判一侧的话，过期条目就是免检通行证）
  const staleKeys = Object.keys(claimed).filter((k) => !unitKeys.includes(k));
  say('   登记表里原文查无的键', staleKeys.length + (staleKeys.length ? ' → ' + staleKeys.slice(0, 6).join(', ') : ''));
  bad.push(staleKeys.length);
  if (Object.keys(claimed).length !== distinct || unowned.length) { console.log('   !! 登记表没覆盖全部单元 ⇒ 段1/段5/段7 同时作废'); bad.push(1, unowned.length); }

  const perFile = new Map(), pf = new Map();
  // `excludeMod`（批 F-13 新形态）：本刀是**同目录兄弟簇的部分搬出**，`dir` 里的 `mod.rs`
  // 是**既有目录的共享区**（`settings/tests/mod.rs` 里那 6 条混住用例属 `KNOWN_MIXED`
  // 历史形状，与这一刀无关）。把它计入产物面 ⇒ 段1"多出 6"、段3"多出 temp_root"、
  // 段4"新增代码 62 行未登记"三条假红一起爆。缺省不带此字段 = 批 F-8/F-9/F-10 的
  // "整文件搬进新目录"口径，一字未动。
  for (const c of [...led.clusters, ...(led.excludeMod ? [] : ['mod'])]) {
    const src = c === 'mod' ? fs.readFileSync(path.join(ROOT, led.dir, 'mod.rs'), 'utf8') : fs.readFileSync(path.join(ROOT, led.dir, `${c}.rs`), 'utf8');
    perFile.set(c, L(src)); pf.set(c, units(src));
  }
  const foundTests = [];
  for (const [c, us] of pf) for (const u of us) if (u.isTest) foundTests.push({ c, name: u.name });
  const origNames = tests.map((t) => t.name).sort();
  const foundNames = foundTests.map((t) => t.name).sort();
  const missT = origNames.filter((n) => !foundNames.includes(n));
  const extraT = foundNames.filter((n) => !origNames.includes(n));
  const wrongHome = foundTests.filter((t) => claimed[t.name] && claimed[t.name] !== t.c);
  say('段1 测试名', `原 ${origNames.length} / 产物 ${foundNames.length} / 丢失 ${missT.length} / 多出 ${extraT.length} / 归属错 ${wrongHome.length}`);
  if (missT.length || extraT.length) console.log('   丢失/多出:', missT.concat(extraT).join(', '));
  if (wrongHome.length) console.log('   归属错:', wrongHome.map((t) => `${t.name}→${t.c}`).join(', '));
  bad.push(missT.length + extraT.length + wrongHome.length);

  const cntA = (b) => b.split(/\r?\n/).filter((l) => /^\s*assert[a-z_]*!/.test(l)).length;
  const origAssert = tests.reduce((a, t) => a + cntA(t.body), 0);
  const foundAssert = foundTests.reduce((a, t) => a + cntA(pf.get(t.c).find((u) => u.name === t.name).body), 0);
  say('段2 断言行', `原 ${origAssert} / 产物 ${foundAssert}`);
  bad.push(origAssert === led.asserts ? 0 : 1, foundAssert === origAssert ? 0 : 1);
  if (origAssert !== led.asserts) console.log(`   !! 原文断言 ${origAssert} != 登记表 ${led.asserts}（原文或登记表变了，先查哪一侧）`);
  const perDelta = tests.map((t) => {
    const f = foundTests.find((x) => x.name === t.name);
    return { n: t.name, a: cntA(t.body), b: f ? cntA(pf.get(f.c).find((u) => u.name === t.name).body) : -1 };
  }).filter((r) => r.a !== r.b);
  say('   逐条断言数不一致的测试', perDelta.length);
  if (perDelta.length) perDelta.forEach((r) => console.log('     ', r.n, r.a, '→', r.b));
  bad.push(perDelta.length);

  const fxOrig = fixtures.map((u) => u.name).sort();
  const foundFx = [];
  for (const [c, us] of pf) for (const u of us) if (!u.isTest && u.kw !== 'mod' && u.kw !== 'use') foundFx.push({ c, name: u.name });
  const fxNames = foundFx.map((f) => f.name).sort();
  const fxMiss = fxOrig.filter((n) => !fxNames.includes(n));
  const fxExtra = fxNames.filter((n) => !fxOrig.includes(n));
  const fxWrong = foundFx.filter((f) => claimed[f.name] && claimed[f.name] !== f.c);
  const modTests = pf.has('mod') ? [...pf.get('mod')].filter((u) => u.isTest).length : 0;
  say('段3 夹具/类型', `原 ${fxOrig.length} / 产物 ${fxNames.length} / 丢失 ${fxMiss.length} / 登记外多出 ${fxExtra.length} / 归属错 ${fxWrong.length} / mod.rs 内用例 ${modTests}${pf.has('mod') ? '' : '（excludeMod：mod.rs 不属本刀产物面）'}`);
  if (fxMiss.length || fxExtra.length) console.log('   ', fxMiss.concat(fxExtra).slice(0, 8).join(', '));
  bad.push(fxMiss.length + fxExtra.length + fxWrong.length + modTests);

  const clusterLines = [];
  for (const [c, ls] of perFile) clusterLines.push(...ls);
  const headSet = new Set([...(led.head.mod || []), ...(led.head.clustersFlat || [])].map(norm));
  for (const c of led.clusters) { (led.head[c] || []).forEach((l) => headSet.add(norm(l))); headSet.add(norm(`mod ${c};`)); }
  const clusterSet = new Set(clusterLines.map(norm));
  const nonEmptyOrig = orig.filter((l) => l.trim());
  // `rewroteOut`（批 F-14 新字段）= 本刀**有意改写掉的原文行**白名单。成因：`use` 按引用面
  // 分派时，一条 `use p::{A, B, C}` 在只用 B 的面上必须重写成 `use p::{B};`（整条复制 =
  // 另两个绑定在该面是 `unused_imports`，clippy `-D warnings` 直接红），于是原条不再逐字出现。
  // 它与 `registeredNew` 成对：那边登记"产物新增了什么"，这边登记"原文改写掉了什么"，
  // 段4 两侧仍双向闭合。口径纪律同批 F-13 教训 (b)：**登记的行必须原文真实存在**——
  // 允许登记不存在的行等于提前给任意删改发免检证。缺省空数组 = 历史三刀口径一字未动。
  const rewroteOut = (led.rewroteOut || []).map(norm);
  const origNormList = nonEmptyOrig.map(norm);
  const staleOut = rewroteOut.filter((r) => !origNormList.includes(r));
  const lost = nonEmptyOrig.filter((l) => !clusterSet.has(norm(l)) && !rewroteOut.includes(norm(l)));
  say('段4 原文非空行', `${nonEmptyOrig.length} / 未原样命中 ${lost.length}（登记改写 ${rewroteOut.length} − 原文查无 ${staleOut.length} = 实际生效 ${rewroteOut.length - staleOut.length}）`);
  if (lost.length) lost.slice(0, 6).forEach((l) => console.log('     丢:', l.slice(0, 90)));
  if (staleOut.length) console.log('     登记改写但原文查无:', staleOut.slice(0, 4).join(' | '));
  const origNorm = new Set(nonEmptyOrig.map(norm));
  const extraLines = clusterLines.filter((l) => l.trim() && !origNorm.has(norm(l)));
  const isStructural = (l) => /^\s*\/\//.test(l) || /^\s*(pub )?mod \w+;\s*$/.test(l) || /^use super::\*;/.test(l) || headSet.has(norm(l));
  const newCode = extraLines.filter((l) => !isStructural(l));
  const reg = (led.registeredNew || []).map(norm);
  const unregistered = newCode.filter((l) => !reg.includes(norm(l)));
  const staleReg = reg.filter((r) => !newCode.map(norm).includes(r));
  say('段4 产物新增行', `${extraLines.length}（结构性可指回 ${extraLines.length - newCode.length}）/ 新增代码行 ${newCode.length} = 登记 ${reg.length} ⇒ 未登记 ${unregistered.length} / 登记未出现 ${staleReg.length}`);
  if (unregistered.length) unregistered.slice(0, 6).forEach((l) => console.log('     新增代码无登记:', l.slice(0, 90)));
  if (staleReg.length) console.log('     登记了却没出现:', staleReg.join(' | '));
  bad.push(lost.length + unregistered.length + staleReg.length + staleOut.length);

  let orderBad = 0;
  for (const c of led.clusters) {
    const want = tests.filter((t) => claimed[t.name] === c).map((t) => t.name);
    const got = pf.get(c).filter((u) => u.isTest).map((u) => u.name);
    if (got.join(',') !== want.join(',')) { console.log(`   ${c} 次序不一致:`, got.join(','), '≠', want.join(',')); orderBad++; }
  }
  say('段5 簇内次序倒挂', orderBad);
  bad.push(orderBad);

  const modDecl = [...declaredMods(fs.readFileSync(path.join(ROOT, led.dir, 'mod.rs'), 'utf8')).keys()].sort();
  const files = siblings(path.join(ROOT, led.dir));
  const dMiss = files.filter((f) => !modDecl.includes(f)).length;
  const dExtra = modDecl.filter((d) => !files.includes(d)).length;
  say('段6 mod 声明', `声明 ${modDecl.length} / 文件 ${files.length} / 有文件没声明 ${dMiss} / 有声明没文件 ${dExtra}`);
  bad.push(dMiss + dExtra);

  if (led.cargoPrefix && !argv.includes('--no-cargo')) {
    const out = execSync(`cargo test ${led.cargoPackage || '--workspace'} --lib -- ${led.cargoPrefix} 2>&1 || true`, { encoding: 'utf8', maxBuffer: 64e6 });
    const running = Number((/running (\d+) tests/.exec(out) || [])[1] ?? -1);
    const seen = [...new Set((out.match(new RegExp(led.cargoPrefix.replace(/[^\w]/g, '\\$&') + '::[\\w:]+', 'g')) || []))].sort();
    const want = origNames.map((n) => `${led.cargoPrefix}::${claimed[n]}::${n}`);
    const notSeen = want.filter((p) => !seen.includes(p));
    // `cargoExpectRunning`（可选）：登记表的用例**只是该前缀下的一部分**时用（批 F-13 那刀
    // 是"同目录兄弟簇部分搬出"，前缀 `settings::tests` 下还有别的簇 ⇒ running 天然大于本刀
    // 的用例数）。此时判据改成两条同时成立：**总数守恒**（running == cargoExpectRunning，
    // 拆前拆后一样多）+ **本刀每条路径都在编译器里存在**（notSeen == 0）。缺省仍按
    // `running == 本刀用例数`（批 F-8/F-9/F-10 那三刀的口径，一字未动）。
    const expectRunning = led.cargoExpectRunning ?? origNames.length;
    say('段7 cargo 注册表', `running ${running} / 期望 ${expectRunning}${led.cargoExpectRunning ? '（前缀下总数）' : '（= 本刀用例数）'} / 名单匹配 ${seen.length} / 原文应有 ${origNames.length} / 编译器里查无 ${notSeen.length}`);
    if (notSeen.length) notSeen.slice(0, 6).forEach((p) => console.log('     查无:', p));
    if (running <= 0) console.log('   !! running<=0 = 一条都没跑，本段结论无效（不许当"绿"）');
    bad.push(notSeen.length, running === expectRunning ? 0 : 1);
  } else say('段7 cargo 注册表', led.cargoPrefix ? '跳过（--no-cargo：CI 上由 kernel 作业的 cargo test 覆盖，此处不重编）' : '跳过（登记表未写 cargoPrefix）');

  const total = bad.reduce((a, b) => a + b, 0);
  console.log(`\n判据合计偏差 = ${total}`);
  return total ? 1 : 0;
}

let rc;
if (argv.includes('--split')) rc = splitGate();
else rc = shapeGate();
console.log(rc === 0 ? (argv.includes('--split') ? 'SPLIT_OK' : 'SPLIT_SHAPE_OK') : `SPLIT_FAIL(${rc})`);
process.exit(rc);
