#!/usr/bin/env node
/**
 * 结构闸：同源对账（v2.209 批 G5 立）。
 *
 * 它管的是**同一件事在仓库里有几个出处**这一类错——单测碰不到、编译器也碰不到：
 * 把 `memlayout::project_dir` 换成手写路径，memlayout 的测试全绿、调用方的测试也全绿，
 * 而磁盘上从此有两个目录名算式，界面上表现为"打开记忆目录"点开一个空文件夹。
 *
 * 为什么只有一道脚本而不是每批建一道：历批这类跨文件判据全写在 tmp/ 的一次性脚本里
 * （实测 283 份），跑完就没人再跑。本脚本是它们的**收口处**——今后新增"同源"类对账
 * 是往 CHECKS 里加一项，不是再立一道闸。加项的门槛：必须写清"漂了会看到什么错误"，
 * 说不清就别加（这条要求本身就是本闸要防的"越加越多"）。
 *
 * 用法：
 *   node scripts/check-single-source.mjs              对账现役代码
 *   node scripts/check-single-source.mjs --selftest   内存变异自测：每项必须能红
 *
 * ⚠ 自测**不碰磁盘**：它改的是读进内存的快照，判据函数只吃这份快照。
 *   扫磁盘再比对是历批踩过的坑（负对照改内存串、判据重读磁盘 ⇒ 对照永远打不中）。
 */
import fs from 'node:fs';
import path from 'node:path';

const ROOT = process.cwd();
const SELFTEST = process.argv.includes('--selftest');
const read = (rel) => fs.readFileSync(path.join(ROOT, rel), 'utf8');

/** 生产 .rs 面：排掉测试目录与构建产物（口径与拆分形状闸一致）。 */
function prodRustFiles() {
  const out = [];
  const walk = (d) => {
    if (/[/\\](target|node_modules|dist)([/\\]|$)/.test(d)) return;
    for (const f of fs.readdirSync(d, { withFileTypes: true })) {
      const p = path.join(d, f.name);
      if (f.isDirectory()) walk(p);
      else if (f.name.endsWith('.rs') && !/([/\\]tests[/\\]|[/\\]tests_|_tests\.rs$|[/\\]tests\.rs$)/.test(p)) out.push(p);
    }
  };
  for (const r of ['crates', 'desktop', 'plugins-src']) {
    const abs = path.join(ROOT, r);
    if (fs.existsSync(abs)) walk(abs);
  }
  return out;
}

const F = {
  memlayout: 'crates/plugin-api/src/memlayout/mod.rs',
  rsFont: 'desktop/src-tauri/src/settings/appearance.rs',
  tsFont: 'desktop/src/lib/ui-font.ts',
  css: 'desktop/src/style.css',
  appLlm: 'desktop/src-tauri/src/llm/app_llm.rs',
};

const countOf = (s, re) => (s.match(new RegExp(re.source, 'g')) || []).length;

/** 一次读盘，快照进内存；之后所有判据只读快照。 */
function snapshot() {
  const files = prodRustFiles();
  const snap = { files, rust: files.map((f) => read(path.relative(ROOT, f).replace(/\\/g, '/'))) };
  for (const [k, rel] of Object.entries(F)) snap[k] = read(rel);
  return snap;
}

/**
 * 判据表。每项 = [编号, 说明, 实测(m) => 字符串, 期望]。
 * 实测与期望都渲染成字符串 ⇒ 加一项只需要写一个读数表达式，"空转"由 SELFTEST 逼出来。
 */
const CHECKS = [
  [
    'SS1',
    '人类层记忆目录名里的那一段 `"projects"`：全生产 Rust 面只许 1 处（算式唯一出口 = memlayout::project_dir）',
    (m) => String(m.rust.reduce((a, s) => a + countOf(s, /"projects"/), 0)),
    '1',
  ],
  [
    'SS1b',
    '且那 1 处必须落在 memlayout/mod.rs 的 PROJECTS_DIR 常量上（漂到别处就是第二个出处）',
    (m) => String(countOf(m.memlayout, /const PROJECTS_DIR: &str = "projects";/)),
    '1',
  ],
  [
    'SS2',
    '界面字号基座三面同数（后端校验常量 / 前端回落常量 / style.css 首帧兜底）',
    (m) => {
      const g = (s, re) => (re.exec(s) || [])[1] ?? 'MISS';
      return [
        g(m.rsFont, /UI_FONT_DEFAULT:\s*u32\s*=\s*(\d+)/),
        g(m.tsFont, /export const UI_FONT_DEFAULT\s*=\s*(\d+)/),
        g(m.css, /--lw-ui-font-size:\s*(\d+)px/),
      ].join('|');
    },
    (m) => {
      const g = (s, re) => (re.exec(s) || [])[1] ?? 'MISS';
      const a = g(m.rsFont, /UI_FONT_DEFAULT:\s*u32\s*=\s*(\d+)/);
      return `${a}|${a}|${a}`;
    },
  ],
  [
    'SS2b',
    'CSS 里那颗基座变量的定义恰好 1 处（两处 = 首帧与 JS 写者会打架，表现为字号一闪再跳）',
    (m) => String(countOf(m.css, /--lw-ui-font-size:\s*\d+px/)),
    '1',
  ],
  [
    'SS3',
    '模型热路径每个请求只取一次 llm 区快照：`with_llm_config(` 1 次、`.llm_config()` 与 `.llm()` 各 0 次' +
      '（后两者各返回**含各档明文 Key 的全量副本**，为读几个字段调它 = 又取锁又克隆）',
    (m) =>
      [
        countOf(m.appLlm, /self\.settings\.with_llm_config\(/),
        countOf(m.appLlm, /\.llm_config\(\)/),
        countOf(m.appLlm, /self\.settings\.llm\(\)/),
      ].join('|'),
    '1|0|0',
  ],
];

/** 闸自己不能瞎：扫描面下限，读数不够就判红而不是"零偏差"。下限取现值的七成 ——
 *  卡在现值边上（第一版写 300 而实测 299）会让闸日常红，那种红只会教人绕过它。 */
const FLOORS = [
  ['生产 .rs 文件数', (m) => m.rust.length, 200],
  ['快照里 memlayout 非空', (m) => (m.memlayout.length > 200 ? 1 : 0), 1],
  ['style.css 含基座定义', (m) => countOf(m.css, /--lw-ui-font-size:\s*\d+px/), 1],
];

function runChecks(m) {
  const rows = [];
  for (const [id, desc, got, want] of CHECKS) {
    const w = typeof want === 'function' ? want(m) : want;
    const v = got(m);
    rows.push({ id, desc, got: v, want: w, bad: v !== w });
  }
  return rows;
}

const snap = snapshot();
const failedFloors = FLOORS.filter(([, f, min]) => f(snap) < min);
if (failedFloors.length) {
  for (const [name, f, min] of failedFloors) console.log(`  ✗ 下限不过：${name} = ${f(snap)}（下限 ${min}）⇒ 扫描面不足，判据作废`);
  console.log('SINGLE_SOURCE_FAIL（扫描面不足 = 闸瞎了，不是"没有偏差"）');
  process.exit(1);
}

if (SELFTEST) {
  // 每项一条内存变异：判据必须为它变红。打不中 ⇒ 对照无效；不红 ⇒ 该项零证明力。
  const MUT = {
    SS1: (m) => m.rust.push('pub fn sneaky() -> String { format!("{}/x", "projects") }'),
    SS1b: (m) => (m.memlayout = m.memlayout.replace('const PROJECTS_DIR: &str = "projects";', 'const PROJECTS_DIR: &str = "proj";')),
    SS2: (m) => (m.css = m.css.replace('--lw-ui-font-size: 14px', '--lw-ui-font-size: 15px')),
    SS2b: (m) => (m.css = m.css.replace('--lw-ui-font-size: 14px;', '--lw-ui-font-size: 14px;\n  --lw-ui-font-size: 16px;')),
    // 窄变异：把 T5 刚去掉的那次"为读三个字段而全量克隆档位区"塞回去。
    // 判据要能为它报红，否则 SS3 只是抄了个数字、不在盯任何东西。
    SS3: (m) =>
      (m.appLlm = m.appLlm.replace(
        'let image_pin = facts.supports_image;',
        'let _dup = self.settings.llm_config();\n        let image_pin = facts.supports_image;',
      )),
  };
  let miss = 0;
  for (const { id, desc } of runChecks(snap)) {
    const mut = MUT[id];
    if (!mut) { console.log(`  ?? ${id} 没有对照 ⇒ 该项证明力未知`); miss++; continue; }
    const copy = { ...snap, rust: [...snap.rust] };
    mut(copy);
    const row = runChecks(copy).find((r) => r.id === id);
    const caught = row && row.bad;
    console.log(`  ${caught ? '✓' : '✗'} ${id} 变异可见：${desc.slice(0, 34)}…${caught ? '' : ` ⇒ 实测仍 = ${row?.got}（对照打空）`}`);
    if (!caught) miss++;
  }
  console.log(miss ? `SELFTEST_MISSED=${miss}` : 'SELFTEST_ALL_CAUGHT');
  process.exit(miss ? 1 : 0);
}

const rows = runChecks(snap);
console.log(`== check-single-source · 同源对账 · 生产 .rs ${snap.rust.length} 个 ==`);
for (const r of rows) {
  console.log(`  ${r.bad ? '✗' : '·'} ${r.id} 实测 ${r.got} 期望 ${r.want}｜${r.desc}`);
}
const bad = rows.filter((r) => r.bad).length;
console.log(bad ? `SINGLE_SOURCE_FAIL(${bad})` : 'SINGLE_SOURCE_OK');
process.exit(bad ? 1 : 0);
