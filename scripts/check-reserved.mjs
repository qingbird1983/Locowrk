#!/usr/bin/env node
/**
 * check-reserved.mjs —— 第六道结构闸（v2.191 / 队列 Q7 立）
 *
 * 治什么：代码里写着「这块是预留待接、生产零调用点」，却没有任何机器盯着这句话。
 * 出处 = 批 Q7 取证《Context Budget Closure》§7.6 那份死代码清单：八行里四行早已不存在
 * （随审计批 C 删除）、两行**不是**死码（`CompactionOutcome` / `CONTEXT_COMPACTED` 是历史会话
 * 日志的重放面，四条通路在读）、只有两件真空着，而那两件其实是「未接线的预留件」。
 * 结论：一句"预留"的自我声明，既可能过期（早就接上线），也可能是将来坏掉的借口（从没规划过）。
 *
 * 判据（三向，全部打在磁盘事实上，不测措辞）：
 *   R1 形态成对：含「定性 = 预留待接」标注的文件必须给得出 `**预留件清单**：…` 行；
 *      只写清单不写标注同样判红（同类只许一种形态）。
 *   R2 标注不许领先代码：清单里每个符号在**生产代码**里的引用点必须为 0。
 *      什么不算"生产引用点"（首跑就是被这三条假红教做人的，见下方注释）：
 *        · 同一文件里 `#[cfg(test)]` 之后的全部行（测试模块）
 *        · 路径含 tests/ 或文件名 tests.rs 的文件
 *        · 声明面：`pub use` / `use` 语句（含跨行大括号续行）、`pub mod`、`fn 定义行`、
 *          `impl 目标行`——符号作为**参数类型**出现在函数签名里也不是调用它
 *      引用点 > 0 = 它已经接线而标注没摘 = 判红并列落点。
 *   R3 清单不许领先代码：清单里的符号必须在仓里还存在（删了没登记 = 判红）。
 *
 * 用法：node scripts/check-reserved.mjs（pre-commit 在暂存面含 .rs 时自动跑）
 */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const SCAN = ['crates', 'desktop/src-tauri/src', 'plugins-src'];
// token 必须是**完整形态**（v2.148 批 D 在 snapshot.rs 立的原句）。第一版用短前缀
// `定性 = 预留待接` 做子串匹配，自证 V4 把整句改成「定性 = 预留待接（token 漂了）」时
// 它仍然命中 ⇒ 假绿：形态纪律要判的是"是不是那一种形态"，不是"像不像"。
const TOKEN = '定性 = 预留待接，不是死码也不是缺陷';
const LEDGER = '**预留件清单**：';
// 防"空转而绿"：扫描面或 token 漂了的话本闸会数到 0 处标注然后 exit 0 = 一台不再检查的门禁
// （历批 H1 抓的正是恒 exit 0 的脚本）。本仓已知有 3 处（snapshot / manager::main_prompt /
// memory-tree::index），低于这个数就说明**闸自己坏了**。确实要摘掉标注时：改这个常数并给理由。
const MIN_ANNOTATIONS = 3;
const MIN_SCANNED = 200;

const files = [];
function walk(dir, rel) {
  if (!fs.existsSync(dir)) return;
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    if (e.name === 'target' || e.name === 'node_modules' || e.name === '.git' || e.name === 'dist') continue;
    const p = path.join(dir, e.name);
    const r = `${rel}/${e.name}`;
    if (e.isDirectory()) walk(p, r);
    else if (e.name.endsWith('.rs')) files.push(r);
  }
}
for (const sub of SCAN) walk(path.join(ROOT, sub), sub);

const isTestFile = (rel) => /(^|\/)tests(_[a-z0-9]+)?(\/|$)/.test(rel) || /(^|\/)tests\.rs$/.test(rel);
const cache = new Map();
/** 只返回"生产行"：去掉测试模块、注释与声明面，剩下的才是真引用点。 */
function prodLines(rel) {
  if (!cache.has(rel)) {
    const raw = fs.readFileSync(path.join(ROOT, rel), 'utf8').split('\n');
    const out = [];
    let inTestMod = false;
    let inUseStmt = false;
    raw.forEach((line, i) => {
      const t = line.trim();
      if (/^#\[cfg\(test\)\]/.test(t)) inTestMod = true;
      if (inTestMod || isTestFile(rel)) return;
      if (/^(!\/|\/\/|\/\*)/.test(t) || t.startsWith('//!')) return; // 注释不是代码
      // use 语句可能跨大行列：进入直到收尾分号，整段都算声明面
      if (inUseStmt) {
        if (t.includes(';')) inUseStmt = false;
        return;
      }
      if (/^(pub(\(\w+\))?\s+)?use\b/.test(t)) {
        if (!t.includes(';')) inUseStmt = true;
        return;
      }
      // 声明/实现行：fn 定义、impl 目标、trait/type/const 声明——签名里的类型出现不算调用
      if (/^(pub(\(\w+\))?\s+)?(async\s+)?(fn|impl|trait|type|struct|enum|const|static|mod)\b/.test(t)) return;
      out.push({ n: i + 1, t });
    });
    cache.set(rel, out);
  }
  return cache.get(rel);
}

const bad = [];
let annotated = 0;
let ledgerItems = 0;

for (const rel of files) {
  const src = fs.readFileSync(path.join(ROOT, rel), 'utf8');
  const hasToken = src.includes(TOKEN);
  const hasLedger = src.includes(LEDGER);
  if (hasToken && !hasLedger) bad.push(`R1 ${rel}：有「${TOKEN}」标注，却给不出「${LEDGER}」行`);
  if (hasLedger && !hasToken) bad.push(`R1 ${rel}：写了预留件清单，却没有「${TOKEN}」标注（同类只许一种形态）`);
  if (hasToken) annotated += 1; // 下限自检数的是**标注**（不是清单）：V2 只摘清单时它仍该是 3，
  //                          否则那次扰动会先撞下限、R1 正向那一档就没有独有点了
  if (!hasLedger) continue;
  const line = src.split('\n').find((l) => l.includes(LEDGER)) || '';
  const syms = line.split(LEDGER)[1].split(/[,，]/).map((s) => s.trim().replace(/`/g, '')).filter(Boolean);
  if (!syms.length) bad.push(`R1 ${rel}：清单行为空`);
  ledgerItems += syms.length;
  for (const sym of syms) {
    const hits = [];
    let definedSomewhere = false;
    for (const other of files) {
      const raw = fs.readFileSync(path.join(ROOT, other), 'utf8');
      // R3 用「定义面」判存在：pub fn / pub struct / pub type / impl 都算它还活着
      if (new RegExp(`^\\s*pub(\\(crate\\))?\\s*(fn|struct|enum|type|const|trait)\\s+${sym}\\b`, 'm').test(raw)) definedSomewhere = true;
      for (const { n, t } of prodLines(other)) {
        if (new RegExp(`\\b${sym}\\b`).test(t)) hits.push(`${other}:${n}  ${t.slice(0, 90)}`);
      }
    }
    if (!definedSomewhere) bad.push(`R3 ${rel}：清单里的 \`${sym}\` 在仓里已找不到定义（删了没登记）`);
    if (hits.length) bad.push(`R2 ${rel}：\`${sym}\` 标注为预留待接，却有 ${hits.length} 处生产引用点 →\n      ${hits.join('\n      ')}`);
  }
}

if (files.length < MIN_SCANNED) {
  console.log(`check-reserved: 判红——扫描面只有 ${files.length} 个 .rs（下限 ${MIN_SCANNED}）= 闸自己坏了，不是"没有预留件"`);
  process.exit(1);
}
// ⚠ 顺序是判据的一部分（本批自证第三轮才想清楚）：annotated 的下限若挡在 R1 之前，
// "token 漂走只剩清单"那次扰动就会被下限吃掉、R1 反向那一档失去独有点。
// 具体判据先出报告，下限只兜"什么都没抓到但标注数掉了"的情况。
if (bad.length) {
  console.log(`check-reserved: 判红 ${bad.length} 项（扫描 ${files.length} 个 .rs · 预留标注 ${annotated} 处 · 清单 ${ledgerItems} 件）`);
  bad.forEach((b) => console.log('  · ' + b));
  process.exit(1);
}
if (annotated < MIN_ANNOTATIONS) {
  console.log(`check-reserved: 判红——预留标注 ${annotated} 处 < 下限 ${MIN_ANNOTATIONS}（token 漂了 / 标注被摘而清单没登记）`);
  process.exit(1);
}
console.log(`check-reserved: OK（扫描 ${files.length} 个 .rs · 预留标注 ${annotated} 处 · 清单 ${ledgerItems} 件 · 越界引用 0 · 缺定义 0）`);
