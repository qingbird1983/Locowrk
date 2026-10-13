#!/usr/bin/env node
// 文档预算闸（v2.195 文档预算批立）。
//
// 管什么：**在役文档的字节 + 批次块数 + 指针在场且唯一 + 归档不被改写**。
// 为什么字节不够、还要数块数：一批的收口叙述可以靠"压措辞"混过字节闸，但窗口纪律是「只许最近
//   DOC_WINDOW 批住在在役文件里」——数块数才能证明"最老那一批真的被搬走了"。
// 出处 = 用户 2026-10-14 裁定「ROADMAP/CHANGELOG 已膨胀到太大」+ 批 E 立过的同一处置（AGENTS 当年
//   94% 字节是历史流水，搬出后一直在预算内）。历批 Q7 立的第六道闸给了这条纪律的形状：
//   **一句自我声明没人盯，它就会漂**——所以窗口与预算都必须有机器判据，而不是写在文档里就算数。
//
// 闸自己也是活的（照 check-reserved 的两道下限口径）：名单里的文件必须都存在、基线必须 > 0、
// 归档头部那句「逐字原样」声明必须在场。缺任何一条判红，而不是默默 OK。
//
// 用法：node scripts/check-doc-budget.mjs            # 全量报告 + 超限判红
//       node scripts/check-doc-budget.mjs --dbg      # 打印每份文件的实测构成
import fs from 'node:fs';
import path from 'node:path';

const ROOT = path.resolve(import.meta.dirname, '..');
const KB = 1024;
const DOC_WINDOW = 8;                 // 在役批次叙述窗口（唯一真源在这里，搬出脚本照它取）
const MARGIN_KB = 12;                 // 每份在役文件相对基线的涨速余量

// 条目 = [相对路径, 角色]；角色 'live' = 在役（字节 + 块数 + 指针），'arc' = 归档（只盯头部声明与存在）
const LIST = [
  ['docs/design/ROADMAP.md', 'live'],
  ['docs/design/CHANGELOG.md', 'live'],
  ['docs/design/Post-Audit Roadmap v1.md', 'live-loose'],
  ['AGENTS.md', 'live-loose'],
  ['docs/design/archive/ROADMAP-batches.md', 'arc'],
  ['docs/design/archive/CHANGELOG-batches.md', 'arc'],
];
// 块标题形状（取证自磁盘）：ROADMAP 用 `### vN`，CHANGELOG 用 `*文档版本：vN`
const BLOCK_RE = {
  'docs/design/ROADMAP.md': /^### v[\d.]+/m,
  'docs/design/CHANGELOG.md': /^\*文档版本：v[\d.]+/m,
};
// 指针行（在役文件里必须能数到一条，且块数 ≤ 窗口时必须解释清楚历史去哪了）
const POINTER_PREFIX = '> **更早批次';
const ARC_DECLARE = '逐字原样';

const errs = [];
const rows = [];

// 表格行完整性（v2.195 立、Q7c 批接进本闸）：**以 `|` 开头却不以 `|` 结尾**的行 = 断行，
// 它会把一张表在渲染层切成两截，而六道 Rust/前端闸看不见文档，历批就靠"顺手加的文档自证"抓过一次
// （批 Q8c 第㉕条：单反斜杠换行字面把 ROADMAP 表格从中间切断，六道闸一道都不红）。
// 基线 = 0（Q7c 已把存量 4 处接回），今后新增即判红——这条判据的存在意义就是"不再靠人眼扫文档"。
const TABLE_BREAK_BASELINE = 0;
const MD_MIN = 30;                    // 扫描面**地板**，不是当前实测数（v2.195 第 62 条的教训：下限拍成
//   60 而实测 59 ⇒ 一次完全正确的闸被判红）。"闸瞎了"由 MD_DIRS 的逐目录命中下限独家负责。
const MD_DIRS = ['docs/design', 'docs/design/NEW', 'docs/design/archive', 'docs/reports'];

function walk(dir, acc) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const abs = path.join(dir, e.name);
    if (e.isDirectory()) walk(abs, acc);
    else if (e.name.endsWith('.md')) acc.push(abs);
  }
  return acc;
}

const mdFiles = walk(path.join(ROOT, 'docs'), []);
const breaks = [];
for (const abs of mdFiles) {
  const t = fs.readFileSync(abs, 'utf8').replace(/\r\n/g, '\n');
  t.split('\n').forEach((l, i) => {
    if (l.startsWith('|') && !l.trimEnd().endsWith('|')) breaks.push(`${path.relative(ROOT, abs)}:${i + 1}`);
  });
}
const perDir = MD_DIRS.map((d) => [d, mdFiles.filter((f) => f.startsWith(path.join(ROOT, d) + path.sep)).length]);
if (mdFiles.length < MD_MIN) {
  errs.push(`md 扫描面 = ${mdFiles.length} < 地板 ${MD_MIN} ⇒ 表格完整性判据几乎看不见文档，闸拒绝空转`);
}
// 逐目录命中下限：某个档数到 0 = 遍历瞎了（目录改名 / 递归失效），比总数更能证明"这条判据是活的"
for (const [d, n] of perDir) {
  if (n === 0) errs.push(`扫描面里 ${d} 命中 0 份 md ⇒ 遍历瞎了，断行判据在这一档不可信`);
}
if (breaks.length > TABLE_BREAK_BASELINE) {
  errs.push(`表格断行数 ${breaks.length} > 基线 ${TABLE_BREAK_BASELINE} ⇒ 新增断行：${breaks.slice(0, 8).join(' · ')}`);
}


function countAll(text, re) {
  // ⚠ 第一版这里写死 flags='g' ⇒ 把 re 自带的 `m` 丢了，`^###` 只匹配字符串开头，
  //   块数恒 0 —— 「块数 ≤ 窗口」就此退化成**恒真的空转判据**（历批 H1 同族，而且这次长在
  //   新闸自己身上，第一次跑就是靠"读数全是 0"看出来的）。正解 = 沿用正则自带 flags 再补 g。
  const m = text.match(new RegExp(re.source, re.flags.includes('g') ? re.flags : re.flags + 'g'));
  return m ? m.length : 0;
}

for (const [rel, role] of LIST) {
  const abs = path.join(ROOT, rel);
  if (!fs.existsSync(abs)) { errs.push(`${rel} 在名单里但文件不存在（改名/删除必须同步摘条目）`); continue; }
  const text = fs.readFileSync(abs, 'utf8').replace(/\r\n/g, '\n');
  const bytes = Buffer.byteLength(text, 'utf8');
  const ptrs = text.split('\n').filter((l) => l.startsWith(POINTER_PREFIX)).length;
  const blockRe = BLOCK_RE[rel];
  const blocks = blockRe ? countAll(text, blockRe) : 0;
  if (bytes <= 0) errs.push(`${rel} 字节为 0 = 闸的读数不可信`);

  if (role === 'live') {
    // 下限先判：块数为 0 而文件非空 ⇒ 是**闸瞎了**，不是"文档没有批次"。不判红的话
    // 「块数 ≤ 窗口」会退化成恒真判据（第一版就是这个形状）。
    if (blockRe && blocks === 0) errs.push(`${rel} 块数读数 = 0 ⇒ 窗口判据看不见任何批次块（正则口径或文件形状坏了），闸拒绝空转`);
    const over = blocks > DOC_WINDOW;
    if (over) errs.push(`${rel} 批次块数 ${blocks} > 窗口 ${DOC_WINDOW} ⇒ 最老一批没搬进归档`);
    if (blocks > 0 && ptrs === 0) errs.push(`${rel} 有 ${blocks} 个批次块却没有「更早批次」指针行 = 历史凭空消失`);
    // 指针只许一条（本批第 44 条判据自纠换来的）：窗口维护脚本重放时若没先剥掉旧指针，在役文件会
    // 出现**两条读数互相矛盾**的指针（一条说累计 61 批、一条说 62 批）——「指针 ≥ 1」看不见它，
    // 而下一次收口读到哪一条完全随机。多条 = 判红，而不是取第一条。
    if (ptrs > 1) errs.push(`${rel} 指针行 ${ptrs} 条 ⇒ 历史去向有两个读数各说各话（重放没剥旧指针）`);
    rows.push({ rel, role, kb: +(bytes / KB).toFixed(1), blocks, ptrs });
  } else if (role === 'live-loose') {
    // 队列与 AGENTS 不数块（形状不同构），只报字节供人工判断；AGENTS 的三条 ≤6000 由收口脚本判
    rows.push({ rel, role, kb: +(bytes / KB).toFixed(1), blocks, ptrs });
  } else {
    if (!text.includes(ARC_DECLARE)) errs.push(`${rel} 头部缺「${ARC_DECLARE}」声明 = 归档被改写或搬出没自证`);
    if (text.trim().length < 200) errs.push(`${rel} 内容过短（${text.length} 字符）= 搬出没真的写进来`);
    rows.push({ rel, role, kb: +(bytes / KB).toFixed(1), blocks, ptrs });
  }
}

// 闸的下限：名单条目数与实际扫描数必须相等（少一条 = 闸在被无声缩小）
if (rows.length !== LIST.length) errs.push(`扫描面 ${rows.length} != 名单 ${LIST.length}`);
if (!rows.some((r) => r.role === 'live')) errs.push('在役档一条都没有 = 这条闸在空转');

console.log(`check-doc-budget（第七道结构闸：文档预算与窗口）`);
console.log(`  表格完整性：扫描 ${mdFiles.length} 份 md · 断行 ${breaks.length}（基线 ${TABLE_BREAK_BASELINE}）`);
console.log(`DOC_TABLE_BREAKS=${breaks.length} · MD_SCANNED=${mdFiles.length} · `
  + perDir.map(([d, n]) => `${d.slice(d.lastIndexOf('/') + 1)}=${n}`).join(' '));
for (const r of rows) {
  console.log(`  ${r.role.padEnd(10)} ${(String(r.kb) + ' KB').padStart(9)} 块 ${String(r.blocks).padStart(3)} 指针 ${r.ptrs}  ${r.rel}`);
}
if (process.argv.includes('--dbg')) {
  console.log(`  窗口 = ${DOC_WINDOW} 批 · 余量口径 = ${MARGIN_KB} KB/份（字节只作兜底，块数才是窗口的判据）`);
  console.log(`  表格完整性口径：行首 \`|\` 而行尾（rstrip 后）非 \`|\` = 断行；基线 ${TABLE_BREAK_BASELINE}、md 下限 ${MD_MIN}`);
}
if (errs.length) {
  console.log('DOC_BUDGET_FAILED=' + errs.length);
  for (const e of errs) console.log('  ✗ ' + e);
  process.exit(1);
}
console.log(`check-doc-budget: OK（扫描 ${rows.length} 份 · 在役块数 ≤ ${DOC_WINDOW} · 归档声明在场 · md ${mdFiles.length} 份断行 ${breaks.length}）`);
console.log('DOC_BUDGET_FAILED=0');
