#!/usr/bin/env node
/**
 * 结构闸：记忆库 schema 两侧同源对账（原为 tmp/q5-parity.mjs，批 T8 收口时升进 scripts/）。
 *
 * 它管的是这一类错：`memories` 表有**两个实现**——宿主 `desktop/src-tauri/src/memory.rs`
 * 与插件 `plugins-src/memory-sqlite/src/store.rs`，两边开同一个 `memory.sqlite3`，
 * 建表串与迁移函数各写了一份。加一列时只改一侧，** cargo 与 vitest 都全绿**，
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
const F = { host: 'desktop/src-tauri/src/memory.rs', plug: 'plugins-src/memory-sqlite/src/store.rs' };
const read = (rel) => fs.readFileSync(path.join(ROOT, rel), 'utf8').replace(/\r\n/g, '\n');
const countOf = (s, re) => (s.match(new RegExp(re.source, 'g')) || []).length;
const code = (s) => s.split('\n').filter((l) => !/^\s*\/\//.test(l)).join('\n');

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

function snapshot() {
  const m = { host: read(F.host), plug: read(F.plug) };
  m.hc = body(m.host, 'const CREATE_TABLE: &str = ', ');";', '宿主 CREATE_TABLE');
  m.pc = body(m.plug, 'const CREATE_TABLE: &str = ', ');";', '插件 CREATE_TABLE');
  m.hm = body(m.host, 'fn ensure_learning_schema(conn: &Connection)', '\n}\n', '宿主迁移函数');
  m.pm = body(m.plug, 'fn ensure_learning_schema(conn: &Connection)', '\n}\n', '插件迁移函数');
  return m;
}

const eq = (a, b) => (a === b ? 'same' : `diff ${a.length}/${b.length}`);
const order = (s) => {
  const a = s.indexOf(CALL_WORKSPACE);
  const b = s.indexOf(CALL_LEARNING);
  return a >= 0 && a < b ? 'ok' : 'bad';
};
/** SET 列表里混进学习列 = 命中次数被整行覆盖回 0，用户表现为「记忆用过也不涨」。 */
const setSide = (s) => {
  const block = s.split('\n').filter((l) => /SET |excluded\./.test(l) && !/^\s*\/\//.test(l)).join('\n');
  return /SET[\s\S]{0,200}hit_count/.test(block) || /excluded\.hit_count/.test(block) ? 'dirty' : 'clean';
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
  ['宿主 memory.rs 有内容', m.host.length, 1000],
  ['插件 store.rs 有内容', m.plug.length, 1000],
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
  const dup = (s) => s;
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
    E1: (m) => (m.host = dup(m.host).replace(/(SET[\s\S]{0,120}?updated_at)/, 'hit_count = excluded.hit_count,\n    $1')),
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
for (const r of rows) console.log(`  ${r.bad ? '✗' : '·'} ${r.id} 实测 ${r.got} 期望 ${r.want}｜${r.desc}`);
const bad = rows.filter((r) => r.bad).length;
console.log(bad ? `SCHEMA_PARITY_FAIL(${bad})` : 'SCHEMA_PARITY_OK');
process.exit(bad ? 1 : 0);
