#!/usr/bin/env node
// 文档规则闸（取代 check-doc-budget.mjs，2026-10-17 文档治理刀）。
//
// 它只判**新规则**，每条都直白可判定：
//   R1 AGENTS.md 别写胖          —— ≤100 行
//   R2 地图别说谎                —— STRUCTURE.md 必须点名每一个在役目录 + 根入口件 + 所有 ★ 件
//   R3 决策记录格式              —— docs/adr/NNNN-短横线主题.md，编号唯一，四节齐
//   R4 阶段计划可测              —— specs/<id>/spec.md 必须有「验收标准」(含复选框) 与「非目标」
//   R5 不许再来万字长行          —— 在役文档单行 ≤2048B；存量按**文件名单**放行（数不写在脚本里）
//   R6 旧仪式不许偷偷回来        —— 在役文档里出现「在役窗口 / 进度三条 / 本次搬出 / 判据自纠 / 逐字原样搬出」判红
//   R7 历史层要有索引            —— docs/history/ 下每一份 .md 都要被 docs/history/README.md 点名
//   R8 断链                     —— 在役文档里的相对链接必须指得到一个真实文件（搬完忘改旧址 = 下一个人只能猜）
//   R9 design 层的形状          —— docs/design 下只许 archive/ 与 authority/ 两个子目录（状态不靠目录名表示）
//   R10 不入库指针要标注        —— 指向被 .gitignore 排除层的链接，同行必须写「不入库」（本机点得开、clone 是死路）
import { execFileSync } from 'node:child_process';
//
// 闸自己也是活的（同旧闸的教训）：扫描面读数为 0 直接判红，因为那只能是**闸瞎了**。
// 用法：node scripts/check-docs.mjs          全量报告 + 判红退出 1
//       node scripts/check-docs.mjs --dbg    打印每条判据的实测构成
import fs from 'node:fs';
import path from 'node:path';

const ROOT = path.resolve(import.meta.dirname, '..');
const rel = (p) => path.relative(ROOT, p).replace(/\\/g, '/');

function walk(dir, acc = []) {
  if (!fs.existsSync(dir)) return acc;
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    if (['node_modules', 'target', 'dist', '.git', 'tmp'].includes(e.name)) continue;
    const p = path.join(dir, e.name);
    if (e.isDirectory()) walk(p, acc);
    else acc.push(p);
  }
  return acc;
}

const read = (p) => fs.readFileSync(p, 'utf8').replace(/\r\n/g, '\n');
const errs = [];
const dbg = process.argv.includes('--dbg');
const bad = (m) => errs.push(m);

const mdAll = walk(path.join(ROOT, 'docs')).filter((f) => f.endsWith('.md'));
// 历史层两个去处，同等对待（不改写、不进在役检查面）：docs/history/ 装流水与旧进度表，
// docs/design/archive/ 装已兑现或已退役的设计稿。判据 = 路径前缀，不靠文件里的措辞。
const HIST_DIRS = ['docs/history/', 'docs/design/archive/'];
const isHist = (r) => HIST_DIRS.some((h) => r.startsWith(h));
const live = mdAll.filter((f) => !isHist(rel(f)));   // 在役文档
const hist = mdAll.filter((f) => isHist(rel(f)));    // 历史层
const rootDocs = ['AGENTS.md', 'STRUCTURE.md', 'README.md', 'CONTRIBUTING.md'].filter((f) => fs.existsSync(path.join(ROOT, f)));

// ---- R1 入口文档别写胖 ---------------------------------------------------------------
const agents = path.join(ROOT, 'AGENTS.md');
if (!fs.existsSync(agents)) bad('AGENTS.md 不存在 = agent 入口没了');
else {
  // 行数口径 = 换行符个数，与 `wc -l` 一致。用 split('\n').length 会让每个以换行结尾的文件
  // 都多报 1 行（实测 103 行的文件报成 104）——判据的读数错一位也是错，尤其这种"卡上限"的。
  const n = (read(agents).match(/\n/g) || []).length;
  if (n > 100) bad(`AGENTS.md ${n} 行 > 100 行上限（细则请放 docs/design/ 或 specs/，入口只留规则与指针）`);
  if (dbg) console.log(`  R1 AGENTS ${n} 行 / 100`);
}

// ---- R2 地图要点名每一个在役目录与每个 ★ 件 -----------------------------------------
const structPath = path.join(ROOT, 'STRUCTURE.md');
if (!fs.existsSync(structPath)) bad('STRUCTURE.md 不存在 = 全仓地图没了（模板：唯一的地图，不要另建一份）');
else {
  const t = read(structPath);
  const dirs = new Set();
  for (const f of mdAll) {
    const r = rel(f).split('/');
    if (r.length > 2) dirs.add(r.slice(0, r.length - 1).join('/'));
  }
  dirs.add('docs');
  dirs.add('docs/design');
  dirs.add('specs');
  dirs.add('docs/adr');
  for (const d of [...dirs].sort()) if (!t.includes(d)) bad(`STRUCTURE.md 没点名在役目录 ${d}/ = 地图漏了一层`);
  for (const f of rootDocs) if (!t.includes(f)) bad(`STRUCTURE.md 没点名 ${f}`);
  // ★ 件：凡文件里自称唯一事实源/权威/正身的，必须能在地图上找到
  for (const f of live) {
    const head = read(f).split('\n').slice(0, 12).join('\n');
    if (/唯一事实源|唯一权威|唯一进度|唯一需求|规范正身|唯一形态标准/.test(head) && !t.includes(path.basename(f)))
      bad(`${rel(f)} 自称唯一/权威/正身，却没出现在 STRUCTURE.md（两处真源 = 必然漂移）`);
  }
  if (dbg) console.log(`  R2 目录点名 ${dirs.size} · 根件 ${rootDocs.length} · 扫描在役 md ${live.length}`);
}

// ---- R3 ADR 格式与编号 ---------------------------------------------------------------
const adrDir = path.join(ROOT, 'docs/adr');
const adrFiles = fs.existsSync(adrDir) ? fs.readdirSync(adrDir).filter((f) => f.endsWith('.md')) : [];
const adrNums = [];
for (const f of adrFiles) {
  if (!/^\d{4}-[a-z0-9-]+\.md$/.test(f)) { bad(`ADR 文件名不合规则：docs/adr/${f}（要 NNNN-短横线主题.md）`); continue; }
  adrNums.push(Number(f.slice(0, 4)));
  if (f === '0000-template.md') continue;
  const t = read(path.join(adrDir, f));
  for (const need of ['## 背景', '## 决策', '## 被否决的方案', '## 后果', '状态：'])
    if (!t.includes(need)) bad(`ADR ${f} 缺节「${need.replace('## ', '')}」`);
}
if (new Set(adrNums).size !== adrNums.length) bad(`ADR 编号重复：${adrNums.join(',')}`);

// ---- R4 specs 必须可测 ---------------------------------------------------------------
const specFiles = walk(path.join(ROOT, 'specs')).filter((f) => f.endsWith('.md') && rel(f) !== 'specs/_template/spec.md');
for (const f of specFiles) {
  const t = read(f);
  const i = t.indexOf('## 验收标准');
  if (i < 0) bad(`${rel(f)} 缺「## 验收标准」——写不出可测验收就别开工`);
  else {
    const seg = t.slice(i, t.indexOf('\n## ', i) > 0 ? t.indexOf('\n## ', i) : t.length);
    if (!/- \[[ x]\]/.test(seg)) bad(`${rel(f)} 的验收标准里没有一条复选框（必须可勾选、可核对）`);
  }
  if (!t.includes('## 非目标')) bad(`${rel(f)} 缺「## 非目标」——防止顺手过度设计`);
}

// ---- R5 长行：按**文件名单**判，不写总数 ----------------------------------------------
// 与 check_length 同一条道理：把"现在有几处"钉进脚本 = 一颗每批都会变、且变得和文档
// 好坏无关的数字（v2.195 那版就写错过一次，写成 32 而实测 30，白送两条免费额度）。
// 名单 = 已知含 >2048B 长行的存量文件（多半是旧规范稿里没分段的整块）。
// 名单外任何在役文档写出一行 2KB 以上 = 判红；把某文件分行干净了就从名单里删一行。
const LONG_FILES = [
  'docs/design/Memory-KV Roadmap.md',
  'docs/design/UI shell and patterns.md',
  'docs/design/authority/Context and Memory.md',
];
const longByFile = new Map();
for (const f of [...live, ...rootDocs.map((d) => path.join(ROOT, d))]) {
  const n = read(f).split('\n').filter((l) => Buffer.byteLength(l) > 2048).length;
  if (n) longByFile.set(rel(f), n);
}
for (const [f, n] of longByFile) {
  if (!LONG_FILES.includes(f))
    bad(`${f} 有 ${n} 行超过 2048 字节（一份文件一行 1 万字 = 读不了、也比不出改动）⇒ 分行，别加名单`);
}
const staleLong = LONG_FILES.filter((f) => !longByFile.has(f));
if (staleLong.length) console.log(`  ~ 长行名单里 ${staleLong.length} 个文件现已干净，删掉这几行收紧：${staleLong.join(' · ')}`);

// ---- R6 旧仪式词汇不许回来 -----------------------------------------------------------
//   扫描面 = 入口 + 规范 + 计划（AGENTS.md / docs/design/** / specs/**）。
//   docs/history/ 里的旧措辞属于历史正文，按规矩不改写，所以不进这一条。
const OLD_WORDS = ['在役窗口', '进度三条', '本次搬出', '判据自纠', '逐字原样搬出'];
const r6Face = [...live.filter((f) => rel(f).startsWith('docs/design/') || rel(f).startsWith('specs/')), path.join(ROOT, 'AGENTS.md')];
for (const f of r6Face) {
  if (!fs.existsSync(f)) continue;
  const t = read(f);
  for (const w of OLD_WORDS) if (t.includes(w)) bad(`${rel(f)} 里出现已废除的旧规矩用语「${w}」——它属于 docs/history/，别再写进在役文档`);
}

// ---- R8 断链：在役文档里的相对链接必须指得到东西 --------------------------------------
// 为什么要有它：文档搬迁最大的风险不是搬错，是**搬完留下指向旧址的链接**——下一个 agent
// 顺着链接走过去查无此文件，就只能猜，而"猜"正是本仓库反复出事的那个根因。
// 口径（每条都是为了不假红）：只看 markdown 链接语法；**先剥掉行内代码** `...`
// ——文档描述别的系统的语法时常写成示例链接（本仓实测：`- [Title](file.md) — hook`
// 是在讲 MEMORY.md 的格式，不是指向本仓文件的引用）；排掉外链与纯锚点；
// 目标做 decodeURIComponent（含空格的文件名写作 %20）；去掉 # 之后的锚再判存在。
const BROKEN_BASELINE = [];  // 已知例外（一条一个「文件:行 → 目标」）；降它要真去修链接
const broken = [];
for (const f of [...live, ...rootDocs.map((d) => path.join(ROOT, d))]) {
  read(f).split('\n').forEach((l, i) => {
    for (const m of l.replace(/`[^`]*`/g, '').matchAll(/\[([^\]]*)\]\(([^)\s]+)\)/g)) {
      let t = m[2];
      if (/^(https?:|mailto:|file:|#)/.test(t)) continue;
      t = t.split('#')[0].split('?')[0];
      if (!t || t.startsWith('/')) continue;
      let target;
      try { target = decodeURIComponent(t); } catch { continue; }
      const abs = path.isAbsolute(target) ? target : path.join(path.dirname(f), target);
      if (!fs.existsSync(abs)) broken.push(`${rel(f)}:${i + 1} → ${target}`);
    }
  });
}
for (const b of broken) {
  if (!BROKEN_BASELINE.includes(b)) bad(`断链 ${b}（旧址还留着引用 = 下一个读的人只能猜）`);
}

// ---- R7 每个历史层都要有自己的索引 ---------------------------------------------------
// 判据从"只盯 docs/history"泛化成"每个历史目录都要有一份 README 点名它内部的每一件"：
// 现在有两个历史去处（docs/history/ 与 docs/design/archive/），只盯一个就等于给另一个开了
// 无人认领的口子——归档件没人点名，后来人就分不清它是不是还在用。
const HIST_INDEXES = ['docs/history/README.md', 'docs/design/archive/README.md'];
for (const idx of HIST_INDEXES) {
  const p = path.join(ROOT, idx);
  if (!fs.existsSync(p)) { bad(`${idx} 不存在 = 那一层历史没有索引`); continue; }
  const t = read(p);
  const own = hist.filter((f) => rel(f).startsWith(idx.replace(/README\.md$/, '')));
  if (!own.length) bad(`${idx} 存在但那一层查无归档件 ⇒ 判据空转（先查历史目录是不是改名或搬空了）`);
  for (const f of own) {
    if (rel(f) === idx) continue;
    if (!t.includes(path.basename(f))) bad(`历史件 ${rel(f)} 没被 ${idx} 点名`);
  }
  if (dbg) console.log(`  R7 ${idx} 点名面 ${own.length} 件`);
}

// ---- R9 design 层的形状不许长回原样 -------------------------------------------------
// 用户 2026-10-17 定的形状：docs/design 里只有两个文件夹 —— authority（现役正身）与
// archive（已兑现/已退役），其余有用稿散在 design 根。这条存在的理由很具体：
// 旧结构里 `NEW/` 这种**用目录名表示状态**的做法，时间一长就没人知道 NEW 里哪些已经做完了，
// 于是又得多写一份状态表。状态只在 docs/roadmap/ 那一行里，不靠目录名。
const designDir = path.join(ROOT, 'docs/design');
if (fs.existsSync(designDir)) {
  const sub = fs.readdirSync(designDir, { withFileTypes: true }).filter((e) => e.isDirectory()).map((e) => e.name).sort();
  const allow = ['archive', 'authority'];
  if (sub.join(',') !== allow.join(','))
    bad(`docs/design 下的子目录 = ${sub.join('/') || '（无）'}，规定的形状是 ${allow.join('/')} 两个；新目录要么并进这两个，要么搬到 docs/ 下别的位置`);
}

// ---- R10 指向「不入库层」的链接必须写明不入库 ----------------------------------------
// 用户 2026-10-17 裁定：`docs/design/` 根上的方案稿、`docs/design/archive/`、`docs/history/`、
// `docs/issue/`、`docs/references/` 移出 git 跟踪（`authority/` 留下）。于是多出一类 R8 看不见的错：
// 链接**在作者本机点得开、clone 之后是死路径** —— R8 只判存在性，它永远绿。
// 不入库名单绝不在这里写第二份：直接问 `git check-ignore`，`.gitignore` 是唯一出处。
// 豁免两处：`docs/adr/`（只增不改，老链接是当时的事实）、`DESIGN.md`（[LOCK]，那处指针已登记等点头）。
const IGNORE_EXEMPT = (r) => r.startsWith('docs/adr/') || r === 'docs/design/authority/DESIGN.md';
const pointerRows = [];
for (const f of live) {
  const r = rel(f);
  if (IGNORE_EXEMPT(r)) continue;
  read(f).split('\n').forEach((l, i) => {
    for (const m of l.replace(/`[^`]*`/g, '').matchAll(/\[([^\]]*)\]\(([^)\s]+)\)/g)) {
      let t = m[2];
      if (/^(https?:|mailto:|file:|#)/.test(t)) continue;
      t = t.split('#')[0].split('?')[0];
      if (!t || t.startsWith('/')) continue;
      let target;
      try { target = decodeURIComponent(t); } catch { continue; }
      const abs = path.isAbsolute(target) ? target : path.join(path.dirname(f), target);
      const rr = rel(abs).replace(/\\/g, '/');
      if (!rr || rr.startsWith('../') || rr === '.') continue;
      pointerRows.push({ r, line: i + 1, text: l, target: rr });
    }
  });
}
const probe = [...new Set([...pointerRows.map((p) => p.r), ...pointerRows.flatMap((p) => [p.target, path.posix.dirname(p.target)])])];
let ignoredSet = null;
try {
  const out = execFileSync('git', ['check-ignore', '--stdin'], { cwd: ROOT, input: probe.join('\n') + '\n', encoding: 'utf8' });
  ignoredSet = new Set(out.split('\n').filter(Boolean).map((s) => s.replace(/\\/g, '/')));
} catch (e) {
  if (e.status !== 1) bad(`R10 问不了 git（${e.message}）⇒ 这条判据作废，不许静默当绿`);
}
if (ignoredSet) {
  // 自己也住在不入库层里的文件（docs/issue、docs/references、archive）互链不算：
  // 那一层根本不会出现在别人机器上，标注是给**入库文档**的读者看的。
  const hits = pointerRows.filter((p) => !ignoredSet.has(p.r))
    .filter((p) => ignoredSet.has(p.target) || ignoredSet.has(path.posix.dirname(p.target)));
  if (!pointerRows.length) bad(`R10 扫描面为 0 条链接 ⇒ 判据空转（先查在役文档口径是不是变了）`);
  if (pointerRows.length && !hits.length) bad(`R10 一条都没命中忽略规则 ⇒ .gitignore 那五条排除规则或链接面变了，判据作废`);
  for (const p of hits) {
    if (!/不入库/.test(p.text)) bad(`${p.r}:${p.line} 指向不入库的 ${p.target}，却没写「不入库」——本机点得开、别人 clone 后是死路径`);
  }
  if (dbg) console.log(`  R10 链接 ${pointerRows.length} 条 · 落在不入库层 ${hits.length} 条`);
}

// ---- 下限：闸不许瞎 -----------------------------------------------------------------
if (mdAll.length < 10) bad(`md 扫描面只有 ${mdAll.length} 份 ⇒ 这条闸几乎看不见文档，拒绝空转`);
if (live.length < 10) bad(`在役文档读数 ${live.length} 过低 ⇒ 目录口径坏了`);

console.log(`check-docs（文档规则闸：入口厚度 / 地图 / ADR / spec / 长行 / 旧仪式 / 历史索引 / design 形状 / 不入库指针）`);
console.log(`DOC_FILES=${mdAll.length} LIVE=${live.length} HISTORY=${hist.length} ADR=${adrFiles.length} SPEC=${specFiles.length} LONG=${[...longByFile.values()].reduce((a, b) => a + b, 0)}处在${longByFile.size}个文件/名单${LONG_FILES.length}个`);
if (dbg) console.log(`  在役目录：${[...new Set(mdAll.map((f) => path.dirname(rel(f))))].join(' ')}`);
if (errs.length) {
  console.log('DOC_FAILED=' + errs.length);
  for (const e of errs) console.log('  ✗ ' + e);
  process.exit(1);
}
console.log(`check-docs: OK（全部判据通过 · 长行只在名单内 ${LONG_FILES.length} 个存量文件里）`);
console.log('DOC_FAILED=0');
