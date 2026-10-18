#!/usr/bin/env node
/**
 * 结构闸：记忆库 schema 两侧同源对账（原为 tmp/q5-parity.mjs，批 T8 收口时升进 scripts/）。
 *
 * 它管的是这一类错：`memories` 表有**两个实现**——宿主侧（`desktop/src-tauri/src` 整根，现役在
 * `memory.rs`）与插件侧（`plugins-src/memory-sqlite/src` 整根，现役在 `store.rs`），两边开同一个
 * `memory.sqlite3`，建表串与迁移函数各写了一份。加一列时只改一侧，**cargo 与 vitest 都全绿**，
 * 用户看到的是"走插件那条通路时命中次数永远是 0"或直接打不开库。
 * 注释里那句"两侧同源"以前只靠人记得，这条闸把它变成机器判据。
 *
 * 口径必须**剥掉注释行再比**：两侧 `ensure_learning_schema` 的既有事实就是插件多一行注释
 * （实测），带着注释比 = 判据一上手就假红。
 *
 * 用法：
 *   node scripts/check-schema-parity.mjs              对账现役代码
 *   node scripts/check-schema-parity.mjs --selftest   内存变异自测：每项必须能红
 *
 * ⚠ 自测不碰磁盘：改的是读进内存的快照，判据只吃这份快照（历批负对照打空的坑）。
 */
import fs from 'node:fs';
import path from 'node:path';

const ROOT = process.cwd();
const SELFTEST = process.argv.includes('--selftest');
// 扫描**根**，不是文件路径（台账 T23 · 2026-10-18）。旧写法 `F = { host: 'memory.rs', plug: 'store.rs' }`
// 把两侧钉死在两个文件上，两个后果：① 谁按 §3 拆那两个文件（它们都在超限名单里），第一个动的
// 不是代码而是这道闸，一刀塞两件事；② 更要紧的是它**只在被钉的那个文件里数**建表串 ——
// 别处再内联一份 `CREATE TABLE IF NOT EXISTS memories` 它看不见，B1/B2 那两档"只许一个出处"
// 当时其实只保证了"那一个文件里只有一个"。现在两侧各扫整根，判据变强两件事：
//   · 定义处计数档（P1–P4）：锚在整根里必须恰好命中 1 个文件，2 处 = 第二个真相，0 处 = 闸瞎了；
//   · B1/B2 的"恰 1 处"升级为整侧恰 1 处。
// 两个根允许环境变量覆盖，只为自证"根被换掉 ⇒ 定位必须响亮失败"这条路径真会走。
const HOST_ROOT = process.env.SP_SCAN_HOST || 'desktop/src-tauri/src';
const PLUG_ROOT = process.env.SP_SCAN_PLUG || 'plugins-src/memory-sqlite/src';
const read = (rel) => fs.readFileSync(path.join(ROOT, rel), 'utf8').replace(/\r\n/g, '\n');
const countOf = (s, re) => (s.match(new RegExp(re.source, 'g')) || []).length;
const code = (s) => s.split('\n').filter((l) => !/^\s*\/\//.test(l)).join('\n');

const filesUnder = (dir, ext) => {
  const out = [];
  const walk = (d) => {
    for (const f of fs.readdirSync(path.join(ROOT, d))) {
      const rel = path.join(d, f).split(path.sep).join('/');
      if (fs.statSync(path.join(ROOT, rel)).isDirectory()) walk(rel);
      else if (rel.endsWith(ext)) out.push(rel);
    }
  };
  walk(dir);
  return out.sort();
};

/** 一侧的扫描结果：整侧原文 + 某个锚的定义处（注释里的锚不算，与 code() 同口径）。 */
function makeSide(files, label) {
  const texts = files.map((rel) => ({ rel, raw: read(rel) }));
  const all = texts.map((t) => t.raw).join('\n');
  return {
    label,
    files,
    all,
    /** 锚在整侧里必须恰好命中 want 个文件，返回那个文件路径；不等就 throw ⇒ 走「扫描面变了，
     *  判据作废」那条出口。这一档**只能靠磁盘变异证**（真在另一个文件里放第二份定义）：
     *  内存里把计数改成 2 等于让判据吃自己编的数，零证明力，所以它不进 --selftest 的内存表。 */
    defFile(anchor, name, want = 1) {
      const hits = texts.filter((t) => code(t.raw).includes(anchor));
      if (hits.length !== want) {
        throw new Error(
          `${label} 侧「${name}」的定义处 = ${hits.length} 个文件（期望 ${want}）`
          + (hits.length ? `：${hits.map((h) => h.rel).join(' · ')}` : '（一个都没有 = 锚或扫描根变了）'),
        );
      }
      return hits[0].rel;
    },
  };
}

/** 取一段原文：锚取不到就是闸瞎，必须判红而不是返回 null 后炸在别处。 */
function body(s, start, endMark, name) {
  const i = s.indexOf(start);
  if (i < 0) throw new Error(`取不到「${name}」的起始锚：${start}`);
  const j = s.indexOf(endMark, i);
  if (j < 0) throw new Error(`取不到「${name}」的结尾锚：${endMark}`);
  return s.slice(i, j + endMark.length);
}

const ALTERS = [
  'ALTER TABLE memories ADD COLUMN hit_count INTEGER NOT NULL DEFAULT 0;',
  'ALTER TABLE memories ADD COLUMN last_hit_at INTEGER;',
];
const CREATE_FRAGS = ['hit_count', 'last_hit_at', 'IF NOT EXISTS'];
const MIGRATE_FRAGS = [
  'if has_col("hit_count")? == 0',
  'if has_col("last_hit_at")? == 0',
  'BEGIN IMMEDIATE',
  'if missing_sql()?.is_empty()',
];
const CALL_LEARNING = 'ensure_learning_schema(&conn)?;';
const CALL_WORKSPACE = 'ensure_workspace_schema(&conn)?;';

const CREATE_ANCHOR = 'const CREATE_TABLE: &str = ';
const MIGRATE_ANCHOR = 'fn ensure_learning_schema(conn: &Connection)';

function snapshot() {
  const host = makeSide(filesUnder(HOST_ROOT, '.rs'), '宿主');
  const plug = makeSide(filesUnder(PLUG_ROOT, '.rs'), '插件');
  // m.host / m.plug 现在装的是**整侧原文**（不再是那一个文件）⇒ A3/C2/D1/D2/E1/B1/B2 那几档
  // "这一侧只有一份真相"从此按整侧判，与文件怎么切分无关。
  const m = { host: host.all, plug: plug.all };
  // 定义处所在文件也进读数（`IN=` 那几条）：拆刀把建表串搬到别的文件时这里跟着变，判据不用改
  m.hcIn = host.defFile(CREATE_ANCHOR, 'CREATE_TABLE 建表串');
  m.pcIn = plug.defFile(CREATE_ANCHOR, 'CREATE_TABLE 建表串');
  m.hmIn = host.defFile(MIGRATE_ANCHOR, '迁移函数');
  m.pmIn = plug.defFile(MIGRATE_ANCHOR, '迁移函数');
  m.hc = body(read(m.hcIn), CREATE_ANCHOR, ');";', '宿主 CREATE_TABLE');
  m.pc = body(read(m.pcIn), CREATE_ANCHOR, ');";', '插件 CREATE_TABLE');
  m.hm = body(read(m.hmIn), MIGRATE_ANCHOR, '\n}\n', '宿主迁移函数');
  m.pm = body(read(m.pmIn), MIGRATE_ANCHOR, '\n}\n', '插件迁移函数');
  m.hostFiles = host.files.length;
  m.plugFiles = plug.files.length;
  return m;
}

const eq = (a, b) => (a === b ? 'same' : `diff ${a.length}/${b.length}`);
/** 调用顺序：比较两个调用串在**整侧**里首次出现的位置。扩面后的口径变化说破在这儿——
 *  旧版只在那一个文件里比，新版按"这一侧第一次出现谁"，实测两侧都仍是 workspace 在前
 *  （读数 ok/ok 与改前逐字相同）。若今后有别的文件先出现 ensure_learning_schema 调用，
 *  这一档会假红，那时要改成按 open 路径逐函数比，而不是放宽期望。 */
const order = (s) => {
  const a = s.indexOf(CALL_WORKSPACE);
  const b = s.indexOf(CALL_LEARNING);
  return a >= 0 && a < b ? 'ok' : 'bad';
};
/** SET 列表里混进学习列 = 命中次数被整行覆盖回 0，用户表现为「记忆用过也不涨」。
 *  取材必须**锚到语句本体**（`INSERT INTO memories … DO UPDATE SET …"`），不能按"整侧所有含
 *  SET 的行"拼 —— 上一版把扫描面从单文件扩到整侧之后就栽在这里：别的表的 upsert 合法带着
 *  hit_count，被行级拼接吸进来就成了假红（扩面时最该防的那种"判据变宽 = 判据变错"）。
 *  同一侧若抄了多份 memories upsert（宿主与插件现状各不止一处），每一份都得干净。
 *  锚取不到时返回读数 `no-anchor` 而不是 throw ⇒ 期望不等当场判红，且不会崩在别处。 */
const setSide = (s) => {
  const segs = [...s.matchAll(/INSERT INTO memories[\s\S]{0,400}?DO UPDATE SET([\s\S]*?)"/g)];
  if (!segs.length) return 'no-anchor';
  return segs.some((g) => /hit_count|last_hit_at/.test(g[1])) ? 'dirty' : 'clean';
};

/** 每项 = [编号, 漂了会看到什么, 实测(m)=>读数, 期望]。 */
const ASSERTS = [
  ['A1', '两侧建表串必须逐字节相同（不同 = 走哪条通路决定表长什么样）', (m) => eq(m.hc, m.pc), 'same'],
  ['A2', '建表串里两列与 IF NOT EXISTS 都在场（缺一个 = 老库上新表缺列）', (m) => CREATE_FRAGS.filter((x) => !m.hc.includes(x)).join('|') || 'all', 'all'],
  ['A3', '两条 ALTER 在两侧都原文在场（parity 只保证说同一句话，不保证话还在）', (m) => String(ALTERS.filter((s) => !(m.host.includes(s) && m.plug.includes(s))).length), '0'],
  ['B1', '宿主侧建表字面量只许一个出处（多一处内联 = 第二个真相）', (m) => String(countOf(m.host, /CREATE TABLE IF NOT EXISTS memories/g)), '1'],
  ['B2', '插件侧建表字面量只许一个出处', (m) => String(countOf(m.plug, /CREATE TABLE IF NOT EXISTS memories/g)), '1'],
  ['C1', '两侧迁移函数剥注释后逐字节相同（不同 = 半迁状态下一侧永远打不开库）', (m) => eq(code(m.hm), code(m.pm)), 'same'],
  ['C2', '逐列补缺与 BEGIN IMMEDIATE 四个承重片段两侧在场', (m) => String(MIGRATE_FRAGS.filter((f) => !(m.host.includes(f) && m.plug.includes(f))).length), '0'],
  ['D1', '两侧 open 路径各调用一次迁移（两次 = 重复迁移，零次 = 老库永不升级）', (m) => `${countOf(m.host, new RegExp(CALL_LEARNING.replace(/[()?]/g, '\\$&'), 'g'))}/${countOf(m.plug, new RegExp(CALL_LEARNING.replace(/[()?]/g, '\\$&'), 'g'))}`, '1/1'],
  ['D2', '调用顺序 = workspace 在前、learning 在后（反了 = 重建的新表不带列，探针却判通过）', (m) => `${order(m.host)}/${order(m.plug)}`, 'ok/ok'],
  ['E1', 'ON CONFLICT 的 SET 列表不含学习列（含了 = 每次写入把命中次数清零）', (m) => `${setSide(m.host)}/${setSide(m.plug)}`, 'clean/clean'],
];

const floorRows = (m) => [
  ['宿主侧整根有内容', m.host.length, 1000],
  ['插件侧整根有内容', m.plug.length, 1000],
  // 扫描面自己的下限：根还在但遍历被改坏（少扫一半）时，内容档可能照样"过"，这里必须掉
  ['宿主侧扫描文件数', m.hostFiles, 150],
  ['插件侧扫描文件数', m.plugFiles, 5],
  ['建表串取到且非空', m.hc.length > 60 && m.pc.length > 60 ? 1 : 0, 1],
  ['迁移函数取到且非空', m.hm.length > 60 && m.pm.length > 60 ? 1 : 0, 1],
];

let snap;
try {
  snap = snapshot();
} catch (e) {
  console.log(`  ✗ ${e.message} ⇒ 扫描面变了，判据作废`);
  console.log('SCHEMA_PARITY_FAIL（锚取不到 = 闸瞎了，不是"没有偏差"）');
  process.exit(1);
}
for (const [name, got, min] of floorRows(snap)) {
  if (got < min) {
    console.log(`  ✗ 下限不过：${name} = ${got}（下限 ${min}）⇒ 扫描面不足，判据作废`);
    console.log('SCHEMA_PARITY_FAIL（扫描面不足 = 闸瞎了，不是"没有偏差"）');
    process.exit(1);
  }
}

if (SELFTEST) {
  // 每条变异只破坏本档，其余档必须仍然绿 —— 否则"这项能红"是搭别项的便车。
  const MUT = {
    A1: (m) => (m.pc = m.pc.replace('hit_count', 'hit_count ')),
    A2: (m) => { m.hc = m.hc.replace('hit_count', ''); m.pc = m.pc.replace('hit_count', ''); },
    A3: (m) => (m.plug = m.plug.replace(ALTERS[1], '// 搬走了')),
    B1: (m) => (m.host = m.host.replace('fn open', 'fn open_unused_marker() {\n  let _ = "CREATE TABLE IF NOT EXISTS memories (id TEXT)";\n}\n\nfn open')),
    B2: (m) => (m.plug = m.plug.replace('fn open', 'fn open_unused_marker() {\n  let _ = "CREATE TABLE IF NOT EXISTS memories (id TEXT)";\n}\n\nfn open')),
    C1: (m) => (m.pm = m.pm.replace('let ', 'let _x = 0u8;\n    let ')),
    C2: (m) => { m.host = m.host.replaceAll('BEGIN IMMEDIATE', 'BEGIN'); m.plug = m.plug.replaceAll('BEGIN IMMEDIATE', 'BEGIN'); },
    D1: (m) => (m.host = m.host.replace(CALL_LEARNING, `${CALL_LEARNING}\n        ${CALL_LEARNING}`)),
    D2: (m) => (m.host = m.host.replace(CALL_WORKSPACE, 'ensure_workspace_moved(&conn)?;')),
    E1: (m) => (m.host = m.host.replace(
      /(INSERT INTO memories[\s\S]{0,400}?DO UPDATE SET\s*)/,
      '$1hit_count = excluded.hit_count, ',
    )),
  };
  const run = (m) => ASSERTS.map(([id, , got, want]) => {
    const v = got(m);
    const w = typeof want === 'function' ? want(m) : want;
    return { id, bad: v !== w, got: v };
  });
  const base = run(snap);
  const stillGreen = (m, id) => run(m).filter((r) => r.id !== id && r.bad).length === 0;
  let miss = 0;
  console.log(`== schema 两侧对账自证 · 基线 ${base.filter((r) => r.bad).length ? '有红项，先修实现再自证' : '全绿'} ==`);
  for (const { id, desc } of ASSERTS.map(([i, d]) => ({ id: i, desc: d }))) {
    const mut = MUT[id];
    if (!mut) { console.log(`  ?? ${id} 没有对照 ⇒ 该项证明力未知`); miss++; continue; }
    const copy = { ...snap };
    mut(copy);
    const row = run(copy).find((r) => r.id === id);
    const caught = row && row.bad;
    const alone = caught && stillGreen(copy, id);
    console.log(`  ${caught && alone ? '✓' : '✗'} ${id} 变异可见：${desc.slice(0, 30)}…${caught ? (alone ? '' : '（同时打红别项 = 档不独立）') : ' ⇒ 实测仍绿（对照打空）'}`);
    if (!caught || !alone) miss++;
  }
  console.log(miss ? `SELFTEST_MISSED=${miss}` : 'SELFTEST_ALL_CAUGHT');
  process.exit(miss ? 1 : 0);
}

const rows = ASSERTS.map(([id, desc, got, want]) => {
  const w = typeof want === 'function' ? want(snap) : want;
  const v = got(snap);
  return { id, desc, got: v, want: w, bad: v !== w };
});
console.log(`== schema 两侧对账 · 宿主 + 插件 memories 表 · ${rows.length} 档 ==`);
// 出处与扫描面（T23 新增）：这四条读数是**定位结果**而不是配置项 —— 拆刀把建表串或迁移函数
// 搬到同侧别的文件，它们跟着变，判据一行不用改；反之"整侧出现第二处定义"当场 BLOCK。
console.log(`  出处 建表 ${snap.hcIn} ⇄ ${snap.pcIn}｜迁移 ${snap.hmIn} ⇄ ${snap.pmIn}`
  + `｜扫描面 宿主 ${snap.hostFiles} 文件 · 插件 ${snap.plugFiles} 文件`);
for (const r of rows) console.log(`  ${r.bad ? '✗' : '·'} ${r.id} 实测 ${r.got} 期望 ${r.want}｜${r.desc}`);
const bad = rows.filter((r) => r.bad).length;
console.log(bad ? `SCHEMA_PARITY_FAIL(${bad})` : 'SCHEMA_PARITY_OK');
process.exit(bad ? 1 : 0);
