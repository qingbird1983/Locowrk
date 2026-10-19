#!/usr/bin/env node
/**
 * 结构闸：记忆库学习面（`memories` 表）的**宿主单侧形状**与出处唯一。
 *
 * ⚠ 脚本名里的 "parity"（两侧对账）是**历史名**，它原来的存在理由随 S1 刀③b 消失了：
 * 那张表曾经有两个实现（宿主 `desktop/src-tauri/src` 与插件 `plugins-src/memory-sqlite/src`
 * 各写一份建表串与迁移函数，开的是同一个 `memory.sqlite3`），加一列只改一侧时
 * cargo 与 vitest 都全绿，用户看到的是"走插件那条通路时命中次数永远是 0"或直接打不开库。
 * 刀③b 之后插件不再开库（`rusqlite` 依赖与 `store/schema.rs` 一起退役），"两侧"塌成
 * 一侧 ⇒ A1/C1/D1/D2 那几档"两侧逐字节相同 / 两侧各调一次"没有对象了，一起摘掉；
 * 留下的五档管的仍是同一件事：**影子表形状只有一个真相，且学习量不被写路径冲掉**
 * （影子现在唯一的维护者是宿主 `memory_reconcile` —— 原先并列点名的 `memory_learn`
 * 在 S3 刀③b 整族退役，`hit_count` / `last_hit_at` 两列至此**没人读写**，去处随 S4 一起裁。
 * H7 那条因此从"防 upsert 冲掉学习档正在累积的数"变成"防 upsert 冲掉一列遗物"——
 * 判据照旧承重：列的形状与写它的语句都是既成事实，改哪一边都要显式来改这里）。
 * "插件不许把这张表再捡回来"那一维不在本闸，它是 `check-single-source.mjs` 的 `SS13`。
 * 为什么不改名：改名要动 `.github/workflows/ci.yml` 里的步骤名——那是 AGENTS §5 禁区。
 *
 * 口径仍然**剥掉注释行再判**（`code()`）：判据吃的是代码，注释里提一句旧形状不该红。
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
// 扫描**根**而不是文件（台账 T23）：钉死单文件的判据会在有人按 §3 拆文件时先红在闸上，
// 而且"整侧只许一个出处"这句话只有扫整根才真的说得出。环境变量覆盖只为自证
// "根被换掉 ⇒ 定位必须响亮失败"这条路径真会走。
const HOST_ROOT = process.env.SP_SCAN_HOST || 'desktop/src-tauri/src';
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

/** 锚在整根里必须恰好命中 want 个文件，返回那个文件路径；不等就 throw ⇒ 走
 *  「扫描面变了，判据作废」那条出口。这一档**只能靠磁盘变异证**（真在另一个文件里
 *  放第二份定义）：内存里把计数改成 2 等于让判据吃自己编的数，零证明力，所以它不进
 *  --selftest 的内存表。 */
function defFile(texts, anchor, name, want = 1) {
  const hits = texts.filter((t) => code(t.raw).includes(anchor));
  if (hits.length !== want) {
    throw new Error(
      `宿主侧「${name}」的定义处 = ${hits.length} 个文件（期望 ${want}）`
      + (hits.length ? `：${hits.map((h) => h.rel).join(' · ')}` : '（一个都没有 = 锚或扫描根变了）'),
    );
  }
  return hits[0].rel;
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
  const files = filesUnder(HOST_ROOT, '.rs');
  const texts = files.map((rel) => ({ rel, raw: read(rel) }));
  const all = texts.map((t) => t.raw).join('\n');
  const m = { host: all };
  m.hcIn = defFile(texts, CREATE_ANCHOR, 'CREATE_TABLE 建表串');
  m.hmIn = defFile(texts, MIGRATE_ANCHOR, '迁移函数');
  m.hc = body(read(m.hcIn), CREATE_ANCHOR, ');";', '宿主 CREATE_TABLE');
  m.hm = body(read(m.hmIn), MIGRATE_ANCHOR, '\n}\n', '宿主迁移函数');
  m.hostFiles = files.length;
  return m;
}

/** 调用顺序：workspace 在前、learning 在后（反了 = 重建的新表不带列，探针却判通过）。
 *  按"整根里第一次出现谁"比；若今后有别的文件先出现 ensure_learning_schema 调用，
 *  这一档会假红，那时要改成按 open 路径逐函数比，而不是放宽期望。 */
const order = (s) => {
  const a = s.indexOf(CALL_WORKSPACE);
  const b = s.indexOf(CALL_LEARNING);
  return a >= 0 && a < b ? 'ok' : 'bad';
};
/** SET 列表里混进学习列 = 命中次数被整行覆盖回 0，用户表现为「记忆用过也不涨」。
 *  取材必须**锚到语句本体**（`INSERT INTO memories … DO UPDATE SET …"`），不能按"整侧所有含
 *  SET 的行"拼 —— 扩面之后就栽过：别的表的 upsert 合法带着 hit_count，被行级拼接吸进来
 *  就成了假红（扩面时最该防的那种"判据变宽 = 判据变错"）。每一份 memories upsert 都得干净。
 *  锚取不到时返回读数 `no-anchor` 而不是 throw ⇒ 期望不等当场判红，且不会崩在别处。 */
const setSide = (s) => {
  const segs = [...s.matchAll(/INSERT INTO memories[\s\S]{0,400}?DO UPDATE SET([\s\S]*?)"/g)];
  if (!segs.length) return 'no-anchor';
  return segs.some((g) => /hit_count|last_hit_at/.test(g[1])) ? 'dirty' : 'clean';
};

/** 每项 = [编号, 漂了会看到什么, 实测(m)=>读数, 期望]。 */
const ASSERTS = [
  ['H1', '建表串里学习两列与 IF NOT EXISTS 都在场（缺一个 = 新建的影子表缺列）', (m) => CREATE_FRAGS.filter((x) => !m.hc.includes(x)).join('|') || 'all', 'all'],
  ['H2', '两条 ALTER 原文在场（parity 摘掉之后"话还在不在"只能自己点名）', (m) => String(ALTERS.filter((s) => !m.host.includes(s)).length), '0'],
  ['H3', '建表字面量只许一个出处（多一处内联 = 第二个真相）', (m) => String(countOf(m.host, /CREATE TABLE IF NOT EXISTS memories/g)), '1'],
  ['H4', '迁移函数四个承重片段在场（逐列补缺 + BEGIN IMMEDIATE + 等锁后二次探针）', (m) => String(MIGRATE_FRAGS.filter((f) => !m.hm.includes(f)).length), '0'],
  ['H5', 'open 路径调用迁移恰好一次（两次 = 重复迁移，零次 = 老库永不升级）', (m) => String(countOf(m.host, new RegExp(CALL_LEARNING.replace(/[()?]/g, '\\$&'), 'g'))), '1'],
  ['H6', '调用顺序 = workspace 在前、learning 在后（反了 = 重建的新表不带列而探针判通过）', (m) => order(m.host), 'ok'],
  ['H7', 'ON CONFLICT 的 SET 列表不含学习列（含了 = 每次写入把命中次数清零）', (m) => setSide(m.host), 'clean'],
];

const floorRows = (m) => [
  ['宿主侧整根有内容', m.host.length, 1000],
  // 扫描面自己的下限：根还在但遍历被改坏（少扫一半）时，内容档可能照样"过"，这里必须掉
  ['宿主侧扫描文件数', m.hostFiles, 150],
  ['建表串取到且非空', m.hc.length > 60 ? 1 : 0, 1],
  ['迁移函数取到且非空', m.hm.length > 60 ? 1 : 0, 1],
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
    H1: (m) => { m.hc = m.hc.replace('hit_count', ''); },
    H2: (m) => { m.host = m.host.replace(ALTERS[1], '// 搬走了'); },
    H3: (m) => { m.host = m.host.replace('fn open', 'fn open_unused_marker() {\n  let _ = "CREATE TABLE IF NOT EXISTS memories (id TEXT)";\n}\n\nfn open'); },
    H4: (m) => { m.hm = m.hm.replace('BEGIN IMMEDIATE', 'BEGIN'); },
    H5: (m) => { m.host = m.host.replace(CALL_LEARNING, `${CALL_LEARNING}\n        ${CALL_LEARNING}`); },
    H6: (m) => { m.host = m.host.replace(CALL_WORKSPACE, 'ensure_workspace_moved(&conn)?;'); },
    H7: (m) => { m.host = m.host.replace(
      /(INSERT INTO memories[\s\S]{0,400}?DO UPDATE SET\s*)/,
      '$1hit_count = excluded.hit_count, ',
    ); },
  };
  const run = (m) => ASSERTS.map(([id, , got, want]) => {
    const v = got(m);
    const w = typeof want === 'function' ? want(m) : want;
    return { id, bad: v !== w, got: v };
  });
  const base = run(snap);
  const stillGreen = (m, id) => run(m).filter((r) => r.id !== id && r.bad).length === 0;
  let miss = 0;
  console.log(`== schema 形状自证 · 基线 ${base.filter((r) => r.bad).length ? '有红项，先修实现再自证' : '全绿'} ==`);
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
console.log(`== 记忆影子表形状 · 宿主单侧 · ${rows.length} 档 ==`);
// 出处与扫描面（T23 新增）：这两条读数是**定位结果**而不是配置项 —— 拆刀把建表串或迁移
// 函数搬到同侧别的文件，它们跟着变，判据一行不用改；反之"整侧出现第二处定义"当场 BLOCK。
console.log(`  出处 建表 ${snap.hcIn}｜迁移 ${snap.hmIn}｜扫描面 宿主 ${snap.hostFiles} 文件`);
for (const r of rows) console.log(`  ${r.bad ? '✗' : '·'} ${r.id} 实测 ${r.got} 期望 ${r.want}｜${r.desc}`);
const bad = rows.filter((r) => r.bad).length;
console.log(bad ? `SCHEMA_PARITY_FAIL(${bad})` : 'SCHEMA_PARITY_OK');
process.exit(bad ? 1 : 0);
