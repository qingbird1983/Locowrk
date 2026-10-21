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
  settingsUpdate: 'desktop/src-tauri/src/settings/update.rs',
  settingsStore: 'desktop/src/stores/settings.ts',
  // specs/13 刀①：自动压缩阈值的缺省与三档预置，两侧各一份（同一条声明的两个投影）。
  contextSettings: 'desktop/src-tauri/src/settings/context.rs',
  autoCompactPct: 'desktop/src/lib/auto-compact-pct.ts',
};

/**
 * 审批测试面（台账 T7 的扫描面）：`tests_permission.rs` + `tests_approval/` 整个目录。
 * 目录式分簇**动态取**，以后新增分簇文件自动在面内——写死文件名的名单会随拆分失效，
 * 而失效的方向是"少扫一个文件 = 闸瞎了"，不是报错。
 */
function approvalTestFiles() {
  const out = ['desktop/src-tauri/src/tests_permission.rs'];
  const dir = path.join(ROOT, 'desktop/src-tauri/src/tests_approval');
  if (fs.existsSync(dir)) {
    for (const f of fs.readdirSync(dir)) if (f.endsWith('.rs')) out.push(`desktop/src-tauri/src/tests_approval/${f}`);
  }
  return out.filter((rel) => fs.existsSync(path.join(ROOT, rel)));
}

/**
 * 设置分区面（台账 T15 的扫描面）：`settings/` 整棵树，排掉测试目录。
 * 也是动态取——以后再加一个分区（或把一个分区拆成目录），它自动在面内。
 */
function settingsPartitionFiles() {
  const out = [];
  const base = 'desktop/src-tauri/src/settings';
  const walk = (rel) => {
    for (const f of fs.readdirSync(path.join(ROOT, rel), { withFileTypes: true })) {
      const p = `${rel}/${f.name}`;
      if (f.isDirectory()) walk(p);
      else if (f.name.endsWith('.rs') && !/(^|\/)tests(_?\.rs|\/)/.test(p)) out.push(p);
    }
  };
  if (fs.existsSync(path.join(ROOT, base))) walk(base);
  return out;
}

const countOf = (s, re) => (s.match(new RegExp(re.source, 'g')) || []).length;

/**
 * 浮层收口面（台账 T10 的扫描面）：`desktop/src/components` 整棵树的 .vue，动态取——
 * 新增一个浮层不必改判据（写死名单的失效方向是"少扫一个文件 = 闸瞎了"）。
 *
 * 口径：左 = 挂了 **document/window 级** `pointerdown` 外点收口的组件数
 * （族 A 四处走 `useDismissOnOutside`，本文件里没有这两串字符 ⇒ 不进这条账，它们由
 * SS9b/SS9c 与 `popover-esc.spec.ts` 盯）；右 = 其中同一文件内出现 `Escape`
 * 或引用 `lib/use-popover` 的数。标准 §二③「一律 Esc + 外点自持」的机械面就是这个
 * 左=右；差一个就是"补了外点忘了补 Esc"——T10 逐站取证实测那种（三处缺 Esc）。
 */
function walkExt(base, re) {
  const out = [];
  const walk = (rel) => {
    for (const f of fs.readdirSync(path.join(ROOT, rel), { withFileTypes: true })) {
      const p = `${rel}/${f.name}`;
      if (f.isDirectory()) walk(p);
      else if (re.test(f.name)) out.push(p);
    }
  };
  if (fs.existsSync(path.join(ROOT, base))) walk(base);
  return out;
}
function popoverFace() {
  const comps = walkExt('desktop/src/components', /\.vue$/).map(read);
  return { comps, frontSrc: walkExt('desktop/src', /\.(ts|vue)$/).map(read) };
}

/**
 * memlayout 子树面（S1 刀②a 加）。口径 = 整棵树的生产 .rs，排掉 `tests.rs`。
 *
 * 为什么不能再吃单文件：目录式模块把一件事拆成了好几个文件（索引面住 `index.rs`、
 * 读回面住 `read.rs`），写死 `mod.rs` 的扫描面会**随拆分瞎掉**——本次实测就是常量搬到
 * `index.rs` 后左值从 2 变 0，报了一条"定义丢了"的红。那条红方向对、结论假，
 * 而假红的下场和真瞎一样是被人绕过去（SS6 立的教训同一头）。动态取整棵树以后，
 * memlayout 再加文件不必改判据；失效方向也从"少扫一个文件"变成"扫多一个无害文件"。
 */
function memlayoutFace() {
  return walkExt('crates/plugin-api/src/memlayout', /\.rs$/)
    .filter((rel) => !/(^|\/)tests(_?\.rs|\/)/.test(rel))
    .map(read)
    .join('\n');
}

/**
 * 模块面通用取法（S2 刀① 加，SS16 的扫描面）：`主文件 + 同名目录子树`，排 tests、剥行注释。
 *
 * 为什么要"主文件 + 子树"两个入口一起给：`context_budget.rs` 现在只有一个文件，但本仓
 * 的惯例是超 400 行就拆目录（`settings/context.rs` 那种"主文件 + settings/tests/"也算），
 * 写死单文件的扫描面会在下一次拆分时**瞎掉**——SS10b 与 SS1b 都栽过这一个洞，口径订正
 * 已经写过两次了，这里直接按学到的形状取。剥注释同 SS11/SS13 那条教训：搬家那一刀的
 * 沿革注释里必然提到被搬走的那个名字，按字面数就是一片假红。
 */
const stripComments = (s) => s.split('\n').filter((l) => !/^\s*\/\//.test(l)).join('\n');
/** 前端面的剥注释：TS 的 `//` 行注释、块注释的起始行与 ` * ` 续行、Vue 模板注释同一档。
 * Rust 侧只需要 `stripComments`（它没有块注释这种写法）。
 * ⚠ 这一档**必须**存在：`desktop/src/types/memory.ts` 里解释"解释档为什么没了"的那段
 * JSDoc 合法提到 `InjectExplain` 与 `explain`——不剥块注释的话墓碑判据一上手就假红
 * （SS6/SS11/SS13 为同一件事各踩过一次，这是第四次）。 */
const stripFrontComments = (s) =>
  s
    .split('\n')
    .filter((l) => !/^\s*(\/\/|\/\*|\*|<!--|-->)/.test(l))
    .join('\n');
/** 学习族墓碑要点名的七个名字（SS29 的靶面）。整族退役 = 这七处形状一处都不许在生产面复活。 */
const LEARN_GHOST =
  /\blearn_enabled\b|\blearnEnabled\b|\brecall_relevant\b|\bLearnWeights\b|\bRecallQuery\b|\bmemory_learn\b|\bbreakdown_json\b/;
/**
 * 测试面的形状口径（S2 刀④c 的口径订正，`hostDesktopFace()` 与 `moduleFace()` 共用）：
 * `tests.rs` / `tests/` 目录 / `tests_*.rs` / `*_tests.rs` 四种都是测试，一个都不许进
 * 生产面。先前两处各写了一遍 `/(^|\/)tests(_?\.rs|\/)/`，那个式子只排得掉前两种，
 * 排不掉 `tests_approval/hub_basic.rs`、`tests_hotpath.rs`、`compact/tests_deep.rs`
 * 这两族 ⇒ "宿主生产面"里一直混着测试代码。今天之前没造成假红，是因为那些文件里恰好
 * 没有被判据点名的形状；刀④c 一新增 `compact/tests_deep.rs` 就当场把 SS21c 读成两个
 * 文件——那是一条**真红**（判据说的"只有一个落点"没被满足），但红的原因在闸自己身上。
 */
const TEST_SHAPED = /(^|\/)(tests\.rs$|tests\/|tests_|_tests\.rs$)/;
function moduleFace(mainRel, dirRel) {
  // Set 去重：目录式模块（`organize/mod.rs`）的主文件本来就在子树里，数两遍会让
  // "定义唯一"这种读数凭空 +1——那是假红，而假红的下场和真瞎一样是被人绕过去。
  const rels = new Set();
  if (mainRel && fs.existsSync(path.join(ROOT, mainRel))) rels.add(mainRel);
  if (dirRel && fs.existsSync(path.join(ROOT, dirRel))) {
    for (const r of walkExt(dirRel, /\.rs$/)) {
      if (!TEST_SHAPED.test(r)) rels.add(r);
    }
  }
  return [...rels].map((rel) => stripComments(read(rel))).join('\n');
}

/**
 * 宿主生产面（S1 刀③b-2 加）：`desktop/src-tauri/src` 整根、**排 tests**、剥行注释，
 * 返回 `[{rel, code}]`。`rel` 从 `desktop/src-tauri/src/` 起算——本闸多数读数是
 * "**哪些文件**落了这个形状"，所以文件名本身就是有信息量的形状，比计数强：
 * 白名单里那个文件被改名 ⇒ 红，别处多出一个出处 ⇒ 也红，两者都不会被"总数没变"糊过去。
 *
 * 排 tests 是刻意的：测试夹具里造形状（例如手写建表串）数进来会让判据永远红。
 * 剥注释是为了不让沿革说明里的旧名字变成假红（SS6 立的口径）。
 */
function hostDesktopFace() {
  return walkExt('desktop/src-tauri/src', /\.rs$/)
    .filter((rel) => !TEST_SHAPED.test(rel))
    .map((rel) => ({
      rel: rel.replace(/^desktop\/src-tauri\/src\//, ''),
      code: read(rel)
        .split('\n')
        .filter((l) => !/^\s*\/\//.test(l))
        .join('\n'),
    }));
}

/**
 * 内核生产面（specs/13 刀① 加）：`crates/kernel-agent/src` 整棵树的 .rs，排 tests、剥行注释。
 *
 * 为什么单独开一个面而不是复用 `m.rust`：`m.rust` 是一摞文件内容、没有文件名，
 * 而本闸读数大多是"**哪些文件**落了这个形状"（`filesWith`）——内核侧那条
 * 「`prepare_request(` 只许一个调用方」的判据要的就是文件名。
 */
function kernelAgentFace() {
  return walkExt('crates/kernel-agent/src', /\.rs$/)
    .filter((rel) => !TEST_SHAPED.test(rel))
    .map((rel) => ({
      rel: rel.replace(/^crates\/kernel-agent\/src\//, ''),
      code: stripComments(read(rel)),
    }));
}

/** 命中形状的文件集合（排序拼接；一个都没有 = `none`）。 */
const filesWith = (list, re) => {
  const hit = list.filter((f) => re.test(f.code)).map((f) => f.rel);
  return hit.length ? [...new Set(hit)].sort().join('|') : 'none';
};

/** 外点收口形状 = 挂到 document/window 上的 `pointerdown`（口径见上方注释）。 */
const DISMISS_SHAPE = /(?:document|window)\.addEventListener\(\s*"pointerdown"/;
const dismissCount = (m) => m.comps.filter((s) => DISMISS_SHAPE.test(s)).length;
const withEscCount = (m) =>
  m.comps.filter((s) => DISMISS_SHAPE.test(s) && /Escape|use-popover/.test(s)).length;

/**
 * 浮层优先级那一侧的两个读数（SS9b / SS9c）。口径写在 `popoverFace()` 注释里：
 * 定义侧数"计数器在几个文件"，消费侧数"除定义处还有几个文件在问 `anyLayerOpen()`"。
 * 期望 `1|1` 是有意的窄：将来真出现第二个消费者（另一条自己的 Esc 链），改这一行
 * 必须显式改判据，而不是让闸默认通过——与 SS8c 点名名单同一条理由。
 */
const LAYER_DEF = /export function anyLayerOpen/;
const layerDefs = (m) => m.frontSrc.filter((s) => /let openLayers/.test(s)).length;
const layerConsumers = (m) =>
  m.frontSrc.filter((s) => /anyLayerOpen\(\)/.test(s) && !LAYER_DEF.test(s)).length;

/**
 * 插件对账面（台账 T11）：`plugins-src/<id>/src/` 下的全部 .rs + 每个插件的 `manifest.json` 在场数。
 *
 * 期望值**取插件目录数而不是写死 10**：第十一个插件没配上对账用例就当场红，而写死数字的话
 * 它可以永久没人对账。`plugins-src` 整个改名/删掉会让两边都数到 0 而"相等"——那种空转由
 * FLOORS 里「内置插件数」那条下限兜住。
 */
function pluginDrift() {
  const base = path.join(ROOT, 'plugins-src');
  const ids = fs.existsSync(base)
    ? fs
        .readdirSync(base, { withFileTypes: true })
        .filter((d) => d.isDirectory())
        .map((d) => d.name)
    : [];
  const testSrc = [];
  let manifests = 0;
  for (const id of ids) {
    if (fs.existsSync(path.join(base, id, 'manifest.json'))) manifests++;
    const srcDir = path.join(base, id, 'src');
    if (!fs.existsSync(srcDir)) continue;
    const walk = (d) => {
      for (const f of fs.readdirSync(d, { withFileTypes: true })) {
        const p = path.join(d, f.name);
        if (f.isDirectory()) walk(p);
        else if (f.name.endsWith('.rs')) {
          testSrc.push(read(path.relative(ROOT, p).replace(/\\/g, '/')));
        }
      }
    };
    walk(srcDir);
  }
  return { count: ids.length, testSrc, manifests };
}

/**
 * 注入环面（台账 T2 的扫描面）：内核中间件树 + 宿主 src 树的生产 .rs（排测试目录）。
 *
 * 口径要写清它**不含 kernel-bin**：`approval_plugin` 是"外部决策方只能 clone + Replace"
 * 的活样本，那条 clone 按设计就该留着（否则 `Replace` 语义在本仓没有真实使用者）。
 * 把它扫进来只会逼人把样本改成假形状。
 */
function ringFiles() {
  const bases = ['crates/kernel-agent/src/middleware', 'desktop/src-tauri/src'];
  const out = [];
  const walk = (rel) => {
    for (const f of fs.readdirSync(path.join(ROOT, rel), { withFileTypes: true })) {
      const p = `${rel}/${f.name}`;
      if (f.isDirectory()) walk(p);
      else if (f.name.endsWith('.rs') && !/(^|\/)tests(_?\.rs|\/)/.test(p)) out.push(p);
    }
  };
  for (const b of bases) if (fs.existsSync(path.join(ROOT, b))) walk(b);
  return out.map(read);
}

/**
 * T2 两条读数的口径 = **环文件的代码行**，两道收窄各挡一类假红：
 *   ① 只取面内含 `impl WaterfallListener for` 的那几份——`browser/tools.rs` 也写
 *      `payload.clone()`，但那是往事件通道发载荷，与 `agent/request` 无关；
 *   ② 剥掉行注释——沿革注释里出现 `payload.clone()` 是正常的（SS6 立过的教训：
 *      连注释一起数 ⇒ 假红，而假红只会教人绕闸）。
 * 分母（实现块数）也在剥注释后的文本上数，两侧口径必须同一个出处。
 */
const RING_SHAPE = /impl\s+WaterfallListener\s+for\s+\w+/;
/**
 * 链位名单（提示词分层设计 §6）：注册序 = system 拼装序，SS8c 按此点名。
 *
 * ⚠ 改名即改判据，每一次改动都记在这里：S3 刀③a 摘掉 `TreeDirectoryInject`；S4 刀③
 * 摘掉 `MemoryInject`（内核 `middleware/memory_inject` 整目录随记忆整线退役——
 * 提示词层不再有索引注入块，指令文件层仍由 `InstructionFilesInject` 承担）。
 */
const RINGS = [
  'ToolExposure',
  'BaseSystemInject',
  'ThinkingLanguageInject',
  'InstructionFilesInject',
  'FragmentInject',
];
function ringCode(m) {
  return m.rings
    .filter((s) => RING_SHAPE.test(s))
    .map((s) => s.replace(/\/\/[^\n]*/g, ''))
    .join('\n');
}
const ringImpls = (m) => countOf(ringCode(m), RING_SHAPE);

/** 一次读盘，快照进内存；之后所有判据只读快照。 */
function snapshot() {
  const files = prodRustFiles();
  const pd = pluginDrift();
  const pf = popoverFace();
  const snap = {
    files,
    rust: files.map((f) => read(path.relative(ROOT, f).replace(/\\/g, '/'))),
    apprTests: approvalTestFiles().map(read),
    settingsParts: settingsPartitionFiles().map(read),
    rings: ringFiles(),
    plugins: pd.count,
    pluginTests: pd.testSrc,
    pluginManifests: pd.manifests,
    comps: pf.comps,
    frontSrc: pf.frontSrc,
    memlayoutTree: memlayoutFace(),
    hostDesktop: hostDesktopFace(),
    kernel: kernelAgentFace(),
    budget: moduleFace(
      'desktop/src-tauri/src/context_budget.rs',
      'desktop/src-tauri/src/context_budget',
    ),
    spill: moduleFace('desktop/src-tauri/src/spill.rs', 'desktop/src-tauri/src/spill'),
    compact: moduleFace('desktop/src-tauri/src/compact.rs', 'desktop/src-tauri/src/compact'),
  };
  // S3 刀③b 墓碑两档（SS29 / SS29b）的扫描面：两档都从**已有面**派生，
  // 不另开一次读盘（同一棵树读两遍 = 两遍之间可以漂）。
  snap.learnGhostRust = snap.rust.map(stripComments).join('\n');
  snap.learnGhostFront = snap.frontSrc.map(stripFrontComments).join('\n');
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
    '且那 1 处必须落在 memlayout **子树**的 `PROJECTS_DIR` 常量上（漂到别处就是第二个出处）。' +
      '⚠ 口径订正（S1 刀②c-1）：这条原先写死扫 `memlayout/mod.rs` 单文件，而寻址面拆进 ' +
      '`paths.rs` 之后左值 2→0、报了一条"定义丢了"的**假红**——与刀②a 时 `SS10b` 撞过的' +
      '同一个洞（目录式模块面会把写死单文件的扫描面搞瞎），这次直接按学到的改，不再留第二处',
    (m) => String(countOf(m.memlayoutTree, /const PROJECTS_DIR: &str = "projects";/)),
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
  [
    'SS4',
    '审批测试面里"轮询 pending 找卡片"只许 1 个出处 = `tests_approval/mod.rs` 的 `Decider::spawn` 内部' +
      '（T7：改前 9 处各写一份**无界** loop，其中 3 处被 await ⇒ 卡片没出现就永挂、' +
      '4 处所在 hub 的超时给 0 ⇒ 没人回填 oneshot 就永等；写回第二份就等于把挂起脆弱性再种一遍）',
    (m) => String(m.apprTests.reduce((a, s) => a + countOf(s, /\.pending\(\)\.first\(\)/), 0)),
    '1',
  ],
  [
    'SS5',
    '设置分区的"校验 + 落值"只有一个写法：编排层 `settings/update.rs` 里 `return Err(` = 3' +
      '（只剩 locale 与 shortcut 两个根字段各自的拒绝语句）、`.apply_update(` 转发 = 10' +
      '（T15：改前 310 行就地铺开在编排层，而批 Q6 早就把 developer 收进分区自己——同一件事两种写法，' +
      '且丑的那种正在长新字段；写回第三处就地校验 = 让下一个改设置的人再猜一次该放哪。' +
      '⚠ S4 刀③c 删 memory 区：转发从 11 降到 10，这一行按本档"改判据 = 显式改"的规矩跟着改）',
    (m) =>
      [countOf(m.settingsUpdate, /return Err\(/), countOf(m.settingsUpdate, /\.apply_update\(/)].join('|'),
    '3|10',
  ],
  [
    'SS5b',
    '且那 10 次转发背后真有 10 个自持的 `fn apply_update` 定义（少一个 = 编排层在替某个分区做事，' +
      '多一个 = 有分区自己留着但已不在链上）',
    (m) => String(m.settingsParts.reduce((a, s) => a + countOf(s, /fn apply_update\(/), 0)),
    '10',
  ],
  [
    'SS6',
    '「即时生效」三份只走同一个 `applyInstant`，且失败回滚读的是**进来时那份投影**：' +
      '`this.applyInstant(` 委托 = 3、`const before = this.view;` = 1、`this.view = before;` = 1、' +
      '`if (before) apply(before);` = 1' +
      '（T16 抓到的洞：三份都写 `catch` 里照 `this.view` 回滚，而 `setSettings` 已成功、' +
      '呈现层自己抛错时 `this.view` 已是新值 ⇒ "回滚"回到新投影 = 没回滚。' +
      '判据数的是**正形状成对在场**而不是"缺陷字面缺席"——注释里提到那个旧形状是正常的，' +
      '数反面会假红，假红教人绕闸）',
    (m) =>
      [
        countOf(m.settingsStore, /this\.applyInstant\(/),
        countOf(m.settingsStore, /const before = this\.view;/),
        countOf(m.settingsStore, /this\.view = before;/),
        countOf(m.settingsStore, /if \(before\) apply\(before\);/),
      ].join('|'),
    '3|1|1|1',
  ],
  [
    'SS7',
    '每个内置插件都有一条「磁盘清单 ↔ 代码 describe」对账用例（T11：这两处各写各的，' +
      '2026-10-18 实测漂的是**能力**字段——`fs_write`/`fs_delete` 与三个 `edit_*` 的 ' +
      '`expected_revision`、`memory_write` 的 `scope`、四处 `exposure: always` 在清单里根本不存在；' +
      'UI 与审批各读一份真相。条数期望 = 插件目录数，加插件不结对账当场红；' +
      '数的是 `fn 名字(` 这个**定义形状**而不是裸名字——改名成 `…_retired` 仍含裸名字，' +
      '第一版对照就是这样打空的（判据自己太宽，盯不住任何东西）',
    (m) => `${m.pluginTests.reduce((a, s) => a + countOf(s, /fn manifest_json_and_describe_agree\s*\(/), 0)}|${m.plugins}`,
    (m) => `${m.plugins}|${m.plugins}`,
  ],
  [
    'SS7b',
    '且每个插件目录里真有那份 `manifest.json`（对账用例读它；缺一个 = 那条对账在替空气报绿）',
    (m) => `${m.pluginManifests}|${m.plugins}`,
    (m) => `${m.plugins}|${m.plugins}`,
  ],
  [
    'SS8',
    '每个 `agent/request` 注入环都走共用的取还形状（T2：改前七环各写一遍 `payload.clone()` + ' +
      'Replace，一次请求 = 七份"全部对话历史 + 工具目录"的深拷贝）。' +
      '左 = 环文件代码行里的 `take_request(` 数，右 = `impl WaterfallListener for` 块数；' +
      '**分母用实现块数而不是写死 7**，第八个环漏接就当场红。' +
      '环整体消失时分子分母一起掉 ⇒ 那种"环静静没了"由 SS8c 点名兜，FLOORS 的下限只挡扫描面整体塌掉。' +
      '口径见 `ringCode()`：只数环文件、只数代码行',
    (m) => {
      const j = ringCode(m);
      return `${countOf(j, /take_request\(/)}|${countOf(j, /impl\s+WaterfallListener\s+for\s+\w+/)}`;
    },
    (m) => {
      const impls = countOf(ringCode(m), /impl\s+WaterfallListener\s+for\s+\w+/);
      return `${impls}|${impls}`;
    },
  ],
  [
    'SS8b',
    '且环文件里没有一处 `payload.clone()` 代码行（SS8 管"环有没有接上"，这条管' +
      '"有没有一边接上又一边退回整包克隆"——两个方向各要一条能单独报红的读数）。' +
      '面外的 clone 是有意留的活样本：`kernel-bin/approval_plugin.rs` 代表"外部决策方' +
      '只能 clone + Replace"，`browser/tools.rs` 是事件通道载荷，都不属于这条账',
    (m) => String(countOf(ringCode(m), /payload\.clone\(\)/)),
    '0',
  ],
  [
    'SS8c',
    '链上注入环**各自还在**（点名清单 = 提示词分层设计 §6 的链位序；条数由名单自己给，' +
      '不在描述里写死数字——刀③a 摘掉一环时描述就与名单漂开过一回）。' +
      'SS8 数的是"接上取还形状的环数 = 实现块数"，一个环被整体改名/搬走时它分子分母一起掉、' +
      '照样绿——那种"环静静消失"只能靠点名兜。名单写死是有意的：动这行 = 改链位，' +
      '必须让人显式改判据而不是让闸替他默认通过',
    (m) => {
      const j = ringCode(m);
      return String(RINGS.filter((n) => new RegExp(`impl\\s+WaterfallListener\\s+for\\s+${n}\\b`).test(j)).length);
    },
    String(RINGS.length),
  ],
  [
    'SS9',
    '挂了 document/window 级 `pointerdown` 外点收口的浮层组件，**同一文件必须有 Esc**' +
      '（《Front-end Design Guidelines》§二③「一律 Esc + 外点自持」的机械面）。' +
      '左 = 外点站点数，右 = 其中带 `Escape` 判定或引用 `lib/use-popover` 收口件的数。' +
      '台账 T10 逐站取证的缺陷形状正是这个差（6 个外点站点里 3 个没有 Esc）——' +
      '族 A 那四处走 `useDismissOnOutside`、文件里没有这两串字符，由 `popover-esc.spec.ts` 盯',
    (m) => `${dismissCount(m)}|${withEscCount(m)}`,
    (m) => `${dismissCount(m)}|${dismissCount(m)}`,
  ],
  [
    'SS9b',
    '浮层优先级的**让位判定还在**：`Composer.vue` 那条输入框 Esc 链必须问 `anyLayerOpen()`' +
      '（T10 (a) 路线的全部承重——这行退回改前的 `if (showModels.value) return;`，' +
      '那三处补的 Esc 立刻重新变成"按一次键关一层 + 顺手拒一次审批"）。' +
      '左 = 计数器定义所在文件数，右 = 除定义处以外还在读 `anyLayerOpen()` 的文件数',
    (m) => `${layerDefs(m)}|${layerConsumers(m)}`,
    '1|1',
  ],
  [
    'SS9c',
    '同一条优先级的**计数器只许一个出处**（左 = 定义文件数，期望 1）。' +
      '第二个 `let openLayers` 一旦出现在别处，"谁开着"就有两本账：两边各自增减，' +
      '漏还的那本会让输入框的 Esc 永久失效而不报错——与 SS1 那条"算式只许一个出口"同形',
    (m) => String(layerDefs(m)),
    '1',
  ],
  [
    'SS10',
    '索引预算那两个数字在全生产 Rust 面各只许 1 处定义（S1 刀①：钳搬到写侧后，落盘的 ' +
      '`MEMORY.md` 与注入侧共用同一份预算——两处各写一份迟早漂成"文件里 210 行、注入说只有 200 行"，' +
      '而人打开文件夹看到的那份就不再是模型看到的那份）',
    (m) =>
      [
        countOf(m.rust.join('\n'), /const MAX_INDEX_LINES: usize = [\d_]+/),
        countOf(m.rust.join('\n'), /const MAX_INDEX_CHARS: usize = [\d_]+/),
      ].join('|'),
    '1|1',
  ],
  [
    'SS10b',
    '且那两处定义确实落在 memlayout 子树（刀②a 起 = `index.rs`，不再在 `mod.rs`；' +
      '真源住在"谁产出这份文件"那一侧，宿主与插件都从它吃数字）',
    (m) => String(countOf(m.memlayoutTree, /pub const MAX_INDEX_(?:LINES|CHARS): usize =/)),
    '2',
  ],
  [
    'SS16a',
    '占用锚点这把尺**只有一个定义处**，且那个定义落在中立模块里' +
      '（S2 刀①：它原来长在 `organize/mod.rs`，而 organize 按 specs/01 D 节在 S4 整面退役——' +
      '压缩触发线要吃锚点，锚点押在一个要删的模块上就是依赖方向错）。' +
      '左 = 全生产 .rs 面里 `fn anchor(` 的定义数，右 = 中立面里同一形状数；两档一起报是' +
      '为了分清"多了一份"与"那份搬出了中立模块"这两种不同的漂法',
    (m) => `${m.rust.reduce((a, s) => a + countOf(s, /\bfn anchor\s*\(/), 0)}|${countOf(m.budget, /\bfn anchor\s*\(/)}`,
    '1|1',
  ],
  [
    'SS16c',
    '有效阈值那三颗零件各只有一个定义：输出侧缺省预留 `32_000`、**唯一** `= 21_000` 的' +
      '预留上限常量、`fn effective_threshold(`（specs/09 AC-S2-2 的机械面）。为什么盯常量' +
      '而不盯数值：ZCode `policy.ts:9-10` 那两颗数是要逐字对齐的参照，别处再写一份 ' +
      '`21_000` 就是第二条加减通路——表现是"进度条离满还差一截就压"或反过来"撞窗了才压"，' +
      '两种都靠人肉才发现。**S4 刀① 按本档自己的规矩改判**：预留上限不再是设置项，' +
      '`OUTPUT_RESERVE_CAP_TOKENS = 21_000` 回到 `context_budget.rs` 写死；`reserve_reading`' +
      '（量 + 来源）随浮层读数链一起删除 ⇒ 第四档从判据表里退场。改动这里 = 显式改判据',
    (m) =>
      [
        countOf(m.rust.join('\n'), /const DEFAULT_OUTPUT_RESERVE_TOKENS: u64 = 32_000;/),
        countOf(m.rust.join('\n'), /const \w+: u64 = 21_000;/),
        countOf(m.rust.join('\n'), /\bfn effective_threshold\s*\(/),
      ].join('|'),
    '1|1|1',
  ],
  [
    'SS17a',
    '「一条工具输出超预算该怎么办」**在宿主生产面只有一个解释者** = ' +
      '`llm/wire/payload.rs` 的 `project_tool_output`，而改前那枚 `clip_tool_output` 是墓碑（恒 none）。' +
      '这条说的是"一条判定、两种后果"：落盘与截断若拆成两层，界面上那一枚「工具结果预算」就同时管两层' +
      '（"24K 到底是截断线还是落盘线"没人答得出），而两层迟早漂成"截断层说 24K、落盘层说 20K"。' +
      '用文件集合而不是计数：计数挡不住"搬家到别的文件"，集合一变就红得能看见是谁（SS14a 同一条理由）',
    (m) =>
      `${filesWith(m.hostDesktop, /\bfn project_tool_output\s*\(/)}|${filesWith(m.hostDesktop, /\bclip_tool_output\b/)}`,
    'llm/wire/payload.rs|none',
  ],
  [
    'SS17b',
    'spill 目录名只有一个定义（`spill.rs` 里那枚 `SPILL_DIR`），且**全宿主面零处手拼 ' +
      '`join("spill")`**（SS1 那条"路径算式只许一个出口"的同族）。组装点只许把**数据根**交给 ' +
      '`SpillStore::new`；它要是自己 `join("spill")`，将来改目录名就是"构造侧与读侧各半个真相"，' +
      '表现为落盘成功而回捞/清理找不到地方。期望含 none 的那一档 ⇒ 由 FLOORS 兜"面瞎了"',
    (m) =>
      `${filesWith(m.hostDesktop, /const SPILL_DIR: &str = "spill";/)}|${filesWith(m.hostDesktop, /join\("spill"\)/)}`,
    'spill.rs|none',
  ],
  [
    'SS17c',
    '发给模型的两句文本各只有一个出处：`<persisted-output>` 模板与降级那句 ' +
      '`fn clipped` 都只住在 `spill.rs`（宿主面文件集合两个都 = `spill.rs`）。' +
      '为什么盯文本而不是盯调用：这两句是模型行为的一部分（它照 `path=` 去 fs_read、' +
      '照"0 = 不限制"知道设置在哪），漂成两个变体就等于对模型撒两次不一样的谎；' +
      '而 `clip_tool_output` 里那段原文在沿革注释里出现是必要留痕，所以数的是代码行' +
      '（`hostDesktop` 面已剥行注释，SS11/SS13 同一口径）',
    (m) =>
      `${filesWith(m.hostDesktop, /<persisted-output/)}|${filesWith(m.hostDesktop, /\bfn clipped\s*\(/)}`,
    'spill.rs|spill.rs',
  ],
  [
    'SS19a',
    '辅助补全的**直呼通路整条退役**（specs/13 刀①）：`llm/aux_client.rs` 那个 ' +
      '`AppLlmSummarizer` 客户端不许再出现——摘要改走主对话那条管线之后' +
      '（`Kernel::prepare_request` 装配 + `AppLlm::complete_compaction` 发送），' +
      '"摘要该走哪一档模型"的答案就是**会话的那一档**，没有第二个取档逻辑可抄。' +
      '为什么留一道墓碑而不是把这一档删掉：删掉之后"有人把第二份取档逻辑抄回来"就' +
      '没有任何读数会红，而那一版的症状极难查——主轮好好地在跑，只有摘要报' +
      '「摘要模型没登上」（specs/13 根因第①条）。' +
      '墓碑数的是**代码行**（剥行注释）：沿革注释里提到旧名字是正常叙述（SS6 立过的口径）。',
    (m) => String(countOf(m.rust.map(stripComments).join('\n'), /\bAppLlmSummarizer\b/)),
    '0',
  ],
  [
    'SS19b2',
    '补 SS19a 的另一半：那一档盯的是**类型名**，这一档盯的是**模块路径**——' +
      '`mod aux_client;` / `crate::llm::aux_client` / `aux_client.rs` 都不许复活。' +
      '为什么必须分两档：有人把第二份直呼客户端住在**别的文件名下**、类型另起一个名字，' +
      '那种改法只动得了这一档（两档合并成一条的话，那种改法一条读数都打不中——' +
      '这正是 SS21c 说明里记过的「两条读数住在同一段代码里，合并之后删掉其中一次构造，' +
      '另一条照样绿」）。',
    (m) => String(countOf(m.rust.map(stripComments).join('\n'), /\baux_client\b/)),
    '0',
  ],
  // ---------- S2 刀④：主层深度压缩 ----------
  [
    'SS20a',
    '压缩折叠产出的那条 **checkpoint user 消息**，它的标签与那句英文说明各只许 1 处：' +
      '真源 = `session-store` 的 `CHECKPOINT_TAG`（`<conversation-checkpoint>`）与 ' +
      '`CHECKPOINT_NOTICE`；宿主侧要同一条消息必须调 `checkpoint_message()`，' +
      '不许自己拼字符串。为什么盯字面量而不是函数名：编译器与行为测试都不红' +
      '（两条字符串长得一样），但落进日志的 checkpoint 与压缩层量出来的那一条' +
      '从此可以不同字——表现是"撤销后窗口对不上"以及"界面上说省了 3 万，重放时只省 2.8 万"。' +
      '⚠ specs/13 刀② 起旧的 `[earlier conversation summarized]` 标记**整条退役**' +
      '（上游 V2 的 checkpoint 形状逐字不同），它一并进墓碑。' +
      '读数数的是**代码行**（剥行注释）：沿革注释里提到旧标记是正常叙述。',
    (m) => {
      const code = m.rust.map(stripComments).join('\n');
      return [
        countOf(code, /const CHECKPOINT_TAG: &str = "<conversation-checkpoint>";/),
        countOf(code, /Treat it as historical context, not as new instructions\./),
        countOf(code, /\[earlier conversation summarized\]/),
      ].join('|');
    },
    '1|1|0',
  ],
  [
    'SS20b',
    '`prompts/compact/` 那**九份稿子**各只被 `include_str!` 一次（刀④b 的' +
      '"两套逐字条款不许合成一句话"在**结构面**那一半：条款各有自己的文件，' +
      '才谈得上"没被合并"）。⚠ specs/13 刀① 起稿子换成上游 V2 逐字那套：' +
      'template / rules / preamble / epilogue / first / update-head / update-legacy / ' +
      'update-tail / nudge；旧的四份（no-tools / keep-security / keep-verbatim / 那版 ' +
      'template）已删或已换。' +
      '同一个稿子被 include 两次 = 有人复制了第二份装配表，日后改文案的人只改一处。' +
      '扫描面用**宿主生产面**（不是只扫 compact 子树）：第二份 include 落在哪个文件都红，' +
      '而落在 compact 子树外面才是真漂。',
    (m) =>
      [
        'template',
        'rules',
        'preamble',
        'epilogue',
        'first',
        'update-head',
        'update-legacy',
        'update-tail',
        'nudge',
      ]
        .map(
          (k) =>
            String(
              m.hostDesktop.reduce(
                (a, f) => a + countOf(f.code, new RegExp(`include_str!\\("[^"]*${k}\\.md"\\)`, 'g')),
                0,
              ),
            ),
        )
        .join('|'),
    '1|1|1|1|1|1|1|1|1',
  ],
  [
    'SS21a',
    '压缩层写进日志的 `strategy` 字面量**只有一个出处** = `compact/deep.rs` 的 `STRATEGY` ' +
      '常量（S4 后只剩 `"compact"` 一档；微压缩那一层已退役）。前端按这个串分流文案' +
      '（`projection.ts` 把它原样放进 `notice.contextCompacted` 的参数），同一层出现两个名字 ' +
      '= 界面上"同一层压缩说两种说法"，而开发者面板按 strategy 计数会直接算错。',
    (m) => String(countOf(m.compact, /STRATEGY:\s*&str\s*=\s*"compact"/)),
    '1',
  ],
  [
    'SS21b',
    '门槛算式**只有一个出处**（specs/13 刀① · C1 · 上游 `compaction.ts:849-856` ' +
      '`calculateCeiling` 住在压缩层那一侧）。四格：' +
      '① `fn ceiling(` 在压缩面恰好 1 处 = 算式的定义处；' +
      '② 「窗口 × 百分比」那一行恰好 1 处 = 用户那枚百分比**只乘一次**；' +
      '③ `effective_threshold(` 的落点集合 = `context_budget.rs`（定义）∪ `compact/plan.rs`（唯一调用方）' +
      '⇒ **进度条那一侧不许自己再算一遍**（它经 `UsagePolicy::assemble` 调 `plan::ceiling`）；' +
      '④ 压缩面里 `session_capacity(` 恒 0 = 压缩层不许自己去查窗口（窗口由组装点查好交进来）。' +
      '⚠ 这一档从 specs/09 刀① 那条「压缩面不许自己拿尺」**反过来了**：上游的门槛算式' +
      '本来就在压缩层里，而本仓多一个**进度条分母**要吃同一颗数 ⇒ 危险从"压缩面自己算"变成了' +
      '"两个地方各算一遍"（症状是"看到的 ≠ 触发用的"，v2.138 那条）。' +
      '⚠ 第②格盯的是**写法**（`窗口 × 百分比` 那一行）：它现在长在 `plan.rs::ceiling` 里，' +
      '若将来换写法须显式改判据——这是本闸一贯的"改判据 = 显式改判据"。',
    (m) => {
      return [
        countOf(m.compact, /fn ceiling\s*\(/),
        countOf(m.compact, /saturating_mul\(pct/),
        filesWith(m.hostDesktop, /effective_threshold\s*\(/),
        countOf(m.compact, /session_capacity\s*\(/),
      ].join('|');
    },
    '1|1|compact/plan.rs|context_budget.rs|0',
  ],
  [
    'SS21c',
    '两处跨文件关系各只许一处：`Compactor::new(` 只在组装点 `state/background.rs`，' +
      'step 边界那一句 `cp.run(` 只在 `runner/mod.rs`。多一个构造点 = 两张熔断表（"连败三次' +
      '就不再试"这件事变成每个调用方各数各的，界面上看不出这一层已经坏了）；多一个调用点 ' +
      '= 一轮压两遍，摘要模型的钱花两遍而窗口只缩一次。行为测试对这两种漂法都是绿的' +
      '（假客户端被调两次它只会数次数，说不出该是谁），所以这两条只能长在结构面上。',
    (m) =>
      `${filesWith(m.hostDesktop, /Compactor::new\s*\(/)}|${filesWith(m.hostDesktop, /\bcp\.run\s*\(/)}`,
    'state/background.rs|runner/mod.rs',
  ],
  [
    'SS22a',
    '熔断线只许**一处比较**：`>= MAX_COMPACT_FAILURES` 在整个压缩簇里恒 1（唯一出处 = ' +
      '`compact.rs::is_fused`）。主层用它决定"还压不压"；两处各写一遍 `>= 3`，将来调那颗数' +
      '就会出现空档，而界面上是同一个症状（请求发不出去），没有一行日志会说清是谁没跟上。',
    (m) => `${countOf(m.compact, />=\s*MAX_COMPACT_FAILURES/)}`,
    '1',
  ],
  [
    'SS22b',
    '熔断表全宿主只许在组装点 new 一枚：`Failures::default()` 只住在 `state/mod.rs`。' +
      '多一处 = 两张表，而这张表的全部意义就是"两个使用方看同一份账"（刀④a/④c 把直呼' +
      '通路提到组装点是同一条纪律，这一步搬的是状态）。行为测试对这种漂法完全无感：' +
      '两张表各自连败三次照样各压各的，只有结构面看得见。',
    (m) => filesWith(m.hostDesktop, /Failures::default\s*\(/),
    'state/mod.rs',
  ],
  [
    'SS22c',
    '同一张表的写侧与读侧分工各只一处：`failures.of|record` 只在 `compact/deep.rs`（读计数 + 失败 +1），' +
      '`failures.clear` 只在 `compact/deep/settle.rs`（压成清零），而 `failures.fused(` **零处**——' +
      '熔断判断只由 `plan` 吃计数做，不许再有第二个读者（S4 刀① 退役滑窗保护网后那一格也随之删除）。' +
      '主层若多一条 `fused` 读取 = 同一件事有两个判断点，两种都在编译器与行为测试的眼皮底下。' +
      '⚠ specs/13 刀① 起 `compact/deep.rs` 按 400 行纪律拆成「主文件 + `compact/deep/` 子树」' +
      '（`send.rs` = 发一趟并验货、`settle.rs` = 落账与清零），⇒ 写侧的两半**分住两个文件**：' +
      '读侧与失败记账在主文件、清零在落账那一爿。这一档跟着拆开盯，别让它退回「都在主文件」的旧形状。',
    (m) =>
      `${filesWith(m.hostDesktop, /self\.failures\.(?:of|record)\s*\(/)}|${filesWith(
        m.hostDesktop,
        /self\.failures\.clear\s*\(/,
      )}|${filesWith(m.hostDesktop, /failures\.fused\s*\(/)}`,
    'compact/deep.rs|compact/deep/settle.rs|none',
  ],
  [
    'SS24',
    '失败判据名**两侧对账**（specs/13 契约 C3）：后端那张表（`compact/outcome.rs` 的 ' +
      '`REASONS` 常量，九条）与前端 `COMPACT_SKIP_KEYS` 的键集合必须**逐条相等**。' +
      '为什么这条必须有：后端只回名字、人话住语言包（AGENTS「文案住语言包」），' +
      '所以 Rust 侧新增一条而前端没补文案时，**编译与行为测试都说不出**——界面只会掉进' +
      '兜底那条，把一句本来有准确说法的判据显示成「没压缩：brand-new-name」。' +
      '反方向同样要红：前端多一条键 = 一句永远显示不出来的文案留在语言包里。' +
      '⚠ 后端取值域**只认 `REASONS` 那一行**（字面量只许出现那一次，别处走 ' +
      '`REASONS[i]` 索引）：不认的话，闸只能去匹配 `Plan::stop("x")` 这种句子，' +
      '而那式子一改措辞就假绿。' +
      '⚠ 这一档同时盯"字面量只在表里出现那一次"：有人在别处再敲一遍 ' +
      '`"nothing-to-compact"` ⇒ 那处迟早与表里那枚漂开，而两枚长得一模一样、' +
      '编译器与行为测试都无感（SS20a 同一条理由）。' +
      '⚠ 声明长度（`[&str; 9]`）也核：两侧同时各少一条时读数会是 `same(8)`，' +
      '靠声明长度与表里条数互校才红得出来（SS25 说明里那条老教训）。',
    (m) => {
      const arr = m.compact.match(/const REASONS: \[&str; (\d+)\] = \[([\s\S]*?)\];/);
      if (!arr) return 'NO_REASONS_TABLE';
      const declared = Number(arr[1]);
      const names = [...arr[2].matchAll(/"([a-z-]+)"/g)].map((x) => x[1]);
      // 表以外不许再出现这九枚字面量。
      const outside = m.compact.replace(arr[0], '');
      const dup = names.filter((n) => countOf(outside, new RegExp(`"${n}"`, 'g')) > 0);
      const front = new Set(
        m.frontSrc
          .flatMap((s) => [...s.matchAll(/^ {2}"?([a-z-]+)"?: "notice\.compact/gm)])
          .map((x) => x[1]),
      );
      const same =
        names.length === 9 &&
        declared === 9 &&
        dup.length === 0 &&
        front.size === names.length &&
        names.every((n) => front.has(n));
      return same
        ? `same(${names.length})`
        : `DECL=${declared} RUST=[${names.sort()}] DUP=[${dup.sort()}] FRONT=[${[...front].sort()}]`;
    },
    'same(9)',
  ],
  [
    'SS25',
    '前端 `COMPACT_SKIP_KEYS` 恰好 9 条（= 后端 `REASONS` 那张表的九条）。' +
      '与 [`SS24`] 分两档不是为了凑数：SS24 比的是**两侧相等**，它能为"后端多一条而前端没配"红，' +
      '但前端自己多敲一条键时它同样红，红出来的读数长得一样，看不出是谁漂的。这一档只数前端，' +
      '所以"语言包里躺着一句永远显示不出来的文案"这种漂移有它自己的红。' +
      '（两档都得活着：只留 SS24 的话，两侧同时各少一条会读成 `same(8)` 而它红不了——' +
      'SS24 那一格里声明长度与表里条数互校盯的正是那个形状。）',
    (m) =>
      String(
        new Set(
          m.frontSrc
            .flatMap((s) => [...s.matchAll(/^ {2}"?([a-z-]+)"?: "notice\.compact/gm)])
            .map((x) => x[1]),
        ).size,
      ),
    '9',
  ],
  [
    'SS31',
    '压缩请求的装配**只有一个出处**（specs/13 刀① · C4）：内核那个 `prepare_request(` ' +
      '（**装配但不发送**：跑 `derive_messages → waterfall_pre → tool_catalog → ' +
      'waterfall_request` 那条与主对话轮完全相同的链）只住在 `kernel/request.rs`，' +
      '而全仓**唯一的调用方**是宿主压缩面 `compact/deep.rs`；桌面端那个 ' +
      '`complete_compaction(`（带总时长预算 / 不推 `agent://llm-retry` / delta 走自己的回调）' +
      '只住在 `llm/app_llm.rs`，唯一调用方同样是 `compact/deep.rs`。' +
      '为什么这条要有：**第二份拼装**正是这一刀要治的病根——摘要绕过主对话请求装配，' +
      '于是"主轮能用 ≠ 摘要能用"（同一时刻两套配置解析、两套 system、两套工具目录）。' +
      '各只许一个调用点：多一个 = 有人另开了一条摘要通路（症状是"压缩总是不成"或' +
      '"摘要比主对话慢一大截"，而编译与行为测试都对它无感）；少一个 = 压缩面没接上主管线。' +
      '⚠ 用**文件集合**而不是计数：集合一变当场看得见是谁（SS14a 同一条理由）。' +
      '⚠ 2026-10-22（刀⑦b · 400 行纪律）：压缩面那一侧的**落点文件名**从 `compact/deep.rs` ' +
      '变成 `compact/deep/send.rs`（发送口那个 trait 与它的生产面绑定挪进了"发送面"那一半）——' +
      '**个数与语义不变**（`complete_compaction(` 仍只有"定义 + 唯一调用方"两个文件），' +
      '变的只是调用方住在哪个文件。',
    (m) =>
      [
        filesWith(m.kernel, /prepare_request\s*\(/),
        filesWith(m.hostDesktop, /prepare_request\s*\(/),
        filesWith(m.hostDesktop, /complete_compaction\s*\(/),
      ].join('|'),
    'kernel/request.rs|compact/deep.rs|compact/deep/send.rs|llm/app_llm.rs',
  ],
  [
    'SS32',
    '自动压缩阈值的**缺省与三档预置两侧对账**（specs/13 契约 C1）：后端 ' +
      '`settings/context.rs` 的 `AUTO_COMPACT_PCT_DEFAULT` / `AUTO_COMPACT_PCT_PRESETS` ' +
      '（**校验权威**：写不进这三档的更新会被后端拒收）与前端 ' +
      '`lib/auto-compact-pct.ts` 的同名两份必须**逐个相等**。' +
      '为什么要有：它们是同一条声明的**两个投影**（不是两个出处），漂了的表现是' +
      '"选了却存不进去"（后端拒收）或"当前值不在候选里"（分段控件没有选中片）——' +
      '两者都不会让任何一条行为测试变红。' +
      '⚠ 这一对与摘要预算那对（`SUMMARY_BUDGET_PRESETS_MS`）同形，但**那边没有对账闸**' +
      '（登记在 specs/13 的遗留项里；不在这一刀顺手修无关代码）。',
    (m) => {
      const rustDefault =
        /AUTO_COMPACT_PCT_DEFAULT:\s*u64\s*=\s*(\d+)/.exec(m.contextSettings)?.[1] ?? 'MISS';
      const rustPresets =
        /AUTO_COMPACT_PCT_PRESETS:\s*\[u64;\s*3\]\s*=\s*\[([^\]]*)\]/.exec(m.contextSettings)?.[1] ??
        'MISS';
      const rustList = [...rustPresets.matchAll(/\d+/g)].map((x) => x[0]);
      const frontDefault =
        /AUTO_COMPACT_PCT_DEFAULT\s*=\s*(\d+)/.exec(m.autoCompactPct)?.[1] ?? 'MISS';
      const frontPresets =
        /AUTO_COMPACT_PCT_PRESETS:[^=]*=\s*\[([^\]]*)\]/.exec(m.autoCompactPct)?.[1] ?? 'MISS';
      const frontList = [...frontPresets.matchAll(/\d+/g)].map((x) => x[0]);
      const same = rustDefault === frontDefault && rustList.join(',') === frontList.join(',');
      return same
        ? `same(${rustList.length})`
        : `RUST=${rustDefault}|${rustList.join(',')} FRONT=${frontDefault}|${frontList.join(',')}`;
    },
    'same(3)',
  ],
  [
    'SS26',
    '刀⑧ 的**写入点分工**：`.refusal_note(` 的调用只住在 `llm/app_llm.rs`，且恒 3 处' +
      '（三条发送收口各一次，与同文件里 `.diagnose(` 那条惯例同形）。为什么行为簇不够：' +
      '`llm/tests/refusals.rs` 那三条是把这方法**直接调**出来的，它证"该写的时候写不写、' +
      '不该写的时候别乱写"——把生产里那三处调用全删掉，四条照样全绿，而那颗读数永远不会' +
      '被记，台账 ⓜ 那个自锁原样回来，且比改前更难发现（表、字段、判定、测试全在场，' +
      '看着就是"有人写过这个机制"）。次数恒 3 盯的是另一半：漏掉一条收口 = 流式那两路' +
      '不记账，界面上的症状恰好就是长跑腿 4 实测到的那个（每轮失败而开关开着）。',
    (m) =>
      `${filesWith(m.hostDesktop, /\.refusal_note\s*\(/)}|${m.hostDesktop.reduce(
        (a, f) => a + ((f.code.match(/\.refusal_note\s*\(/g) || []).length),
        0,
      )}`,
    'llm/app_llm.rs|3',
  ],
  [
    'SS27',
    'S4 刀③ 的注入面墓碑：三枚记忆注入块标签（`<persistent_memory>` / ' +
      '`<project_memory_index>` / `<memory_index>`）在**全生产 .rs**（已排 tests、剥行注释）' +
      '里命中恒 0。记忆注入整线退役之后，写入方（内核 `memory_inject`）、读取方' +
      '（`memory_index.rs`）与观测切块（`diagnose.rs` 的 SYSTEM_MARKERS）一起删除——' +
      '这三个标签从此没有任何合法出处。判据守的是"有人把其中一段接回去"：只要出现一处' +
      '字面量，说明注入面又长回来了，而行为测试与编译都不会为此变红。' +
      'SS6 立过的那条口径同样适用：数的是**代码行**不是注释，退役说明里提旧标签是正常叙述。',
    (m) => String(countOf(m.rust.map(stripComments).join('\n'), /<persistent_memory>|<project_memory_index>|<memory_index>/)),
    '0',
  ],
  [
    'SS29',
    'S3 刀③b「学习档整族退役」墓碑·第一档（Rust 侧形状）：七个名字（`learn_enabled` / ' +
      '`learnEnabled` / `recall_relevant` / `LearnWeights` / `RecallQuery` / `memory_learn` / ' +
      '`breakdown_json`）在**全生产 .rs**（已排 tests、剥行注释）里命中恒 0。' +
      '为什么这一刀非要留一条结构判据而不是删完就走：学习档在 S1 之后就是**残留的第二真源**' +
      '（真源 = md，它按影子库 `hit_count` 现算加权排序），而它离"被谁顺手接回去"只差一个 ' +
      'trait 方法 + 一格设置开关 ⇒ 那种改法在行为面上测不出来（内核那批用例全拿假件源跑，' +
      '宿主那批只会断言自己构造的 query）。数的是**形状**：`memory_learn` 这类名字在退役' +
      '注释里合法在场（六处头注都在解释它为什么没了），连注释一起数会得到一片假红' +
      '——SS6/SS11/SS13 立过三次的那条口径。',
    (m) => String(countOf(m.learnGhostRust, LEARN_GHOST)),
    '0',
  ],
  [
    'SS29b',
    '同一墓碑·第二档（前端形状）：`learnEnabled` 属性 / `relevanceWeight` / `.explain` ' +
      '属性访问在 `desktop/src` 的 .ts/.vue（剥 `//`、JSDoc 与 `<!-- -->`）里命中恒 0。' +
      '不与第一档合并是因为**两种漂法不同**：Rust 侧漂了是"通路被捡回来"，前端侧漂了是' +
      '"界面长回一格拨了没反应的开关"或"读一个后端已经不发的字段"（后者静默读成 ' +
      '`undefined`，界面上就是一个空格）。关键词只取代码形状：`InjectExplain` 这类**类型名**' +
      '在 `types/memory.ts` 的退役注释里合法在场，把它数进来当场假红（SS6 那条口径）。',
    (m) => String(countOf(m.learnGhostFront, /\blearnEnabled\b|\brelevanceWeight\b|\.explain\b/)),
    '0',
  ],
  [
    'SS30',
    'S3 刀④a · 台账 ⓟ「占用读数一个出处」的在场档（三段读数）。① `fn occupancy(` 的定义' +
      '只住在 `context_budget.rs`；② `budget::occupancy(` 的调用在生产面恰 **2** 处 = ' +
      '进度条（`runner::record_usage`）与压缩判定（`Compactor::used`）那两个消费点；' +
      '③ `UsagePolicy::assemble(` 的调用集合恰在 `commands/chat.rs` 与 `scheduler/engine.rs`' +
      '（挂账 ⓛ 的机械面：两个组装点的装配口必须是一个）。' +
      '为什么行为簇不够：三档取最大的算式在 `context_budget/tests.rs` 有五条用例钉着，' +
      '它们证"函数自己算得对"；把 `record_usage` 那一次调用换成 `anchor(session).0`，' +
      '五条一条不红、进度条当场回到"第二把尺"——那正是 ⓟ 登记时写的症状（环与触发不同数，' +
      '界面上是"离满还差一截就压"或"看着满线不动却一直在压"）。所以这一档数**调用点**，' +
      '而且用文件集合而不是只数总数（SS14a 同一条理由：计数挡不住"搬去第三个文件"）。',
    (m) =>
      [
        filesWith(m.hostDesktop, /\bfn occupancy\s*\(/),
        String(
          m.hostDesktop.reduce(
            (a, f) => a + ((f.code.match(/budget::occupancy\s*\(/g) || []).length),
            0,
          ),
        ),
        filesWith(m.hostDesktop, /UsagePolicy::assemble\s*\(/),
      ].join('|'),
    'context_budget.rs|2|commands/chat.rs|scheduler/engine.rs',
  ],
  [
    'SS30b',
    '同一件事的缺席档（两把旧尺都不许回来，恒 `none`）。① `UsagePolicy {` 字面量在生产面' +
      '零处——两个组装点一律经 `assemble`；数之前先把三处**合法在场**剔掉（`pub struct ' +
      'UsagePolicy {` 定义、`impl UsagePolicy {` 装配口、`-> UsagePolicy {` 返回类型），不剔' +
      '就红在正确代码上（本表里"正则要收窄"的同款教训）。② `.context_tokens(` 零处 = ' +
      '`TreeAssembler::context_tokens`（目录 + 尾巴那把第二尺）连方法带 `run_turn` 的 `tree` ' +
      '参数一起摘掉了，不许复活：它一旦回来，"进度条与压缩同一个分子"就只剩注释在承诺。' +
      '两档都数代码行（`hostDesktop` 面已剥行注释）：`occupancy` / `context_tokens` 这两个' +
      '名字在本刀的退役注释里有好几处合法在场，连注释数会当场假红（SS11/SS13 立过三次的那条口径）。' +
      '期望含两个 `none` ⇒ "面瞎了"与"真的没有"读数相同，由 FLOORS 的宿主文件数下限兜住。',
    (m) => {
      // 三处**合法在场**先剔掉再数构造字面量：结构体定义、它的 impl 块、返回类型箭头。
      // 不剔就会红在正确代码上（本表里"带前导点"的同款教训）。
      const face = m.hostDesktop.map((f) => ({
        ...f,
        code: f.code
          .replace(/pub struct UsagePolicy \{/, '')
          .replace(/impl UsagePolicy \{/, '')
          .replace(/-> UsagePolicy \{/, ''),
      }));
      return `${filesWith(face, /\bUsagePolicy\s*\{/)}|${filesWith(face, /\.context_tokens\s*\(/)}`;
    },
    'none|none',
  ],
  [
    'SS33',
    '失败**档位**只有一个出处 + 压缩面那三枚文本判据**不许回来**（specs/13 刀⑦b · 挂账 ①）。' +
      '四段读数：① `enum FailureKind` 的定义只住在 `llm/failure.rs`（档位是**一份声明**，' +
      '不是两处各写一遍）；② `is_output_limit` / `is_content_filter` / ' +
      '`is_payload_too_large` / `is_budget_exhausted` 四个名字在宿主生产面命中恒 `none`' +
      '（压缩层按**档**认失败，不再从文案里猜关键词——这四个函数随刀⑦b 退役）；' +
      '③ `fn mentions_content_policy(` 的定义只住在 `llm/retry.rs`（内容策略的关键词表' +
      '**全仓只剩那一份**，住接入面 = 看得见原文的地方）；④ 那几个关键词串（"content policy" / ' +
      '"content_policy" / "safety filter" / "moderation"）在宿主生产面也只出现在 ' +
      '`llm/retry.rs`。' +
      '为什么要有：判据从"文案"改成"档位"这件事**本身**没有行为可测——把某一枚退回文本' +
      '匹配，界面那句"为什么没压成"会**说错原因**，而压缩面照跑不误（行为测试全绿）；' +
      '这一档盯的是**形状**（还有没有人在猜文案），编译器与行为测试都看不见。' +
      '⚠ ②的期望是 `none` ⇒ "面瞎了"与"真的没有"读数相同，由 FLOORS 的宿主文件数下限兜住；' +
      '①③④ 各自点名一个文件，面塌掉时它们读成 `none` ⇒ 同样当场红。',
    (m) =>
      [
        filesWith(m.hostDesktop, /enum\s+FailureKind/),
        filesWith(
          m.hostDesktop,
          /is_output_limit|is_content_filter|is_payload_too_large|is_budget_exhausted/,
        ),
        filesWith(m.hostDesktop, /fn mentions_content_policy\s*\(/),
        filesWith(m.hostDesktop, /"content policy"|"content_policy"|"safety filter"|"moderation"/),
      ].join('|'),
    'llm/failure.rs|none|llm/retry.rs|llm/retry.rs',
  ],
];

/** 闸自己不能瞎：扫描面下限，读数不够就判红而不是"零偏差"。下限取现值的七成 ——
 *  卡在现值边上（第一版写 300 而实测 299）会让闸日常红，那种红只会教人绕过它。 */
const FLOORS = [
  ['生产 .rs 文件数', (m) => m.rust.length, 200],
  ['快照里 memlayout 非空', (m) => (m.memlayout.length > 200 ? 1 : 0), 1],
  // 刀②a 起 memlayout 是目录式模块：子树面为空 = 目录改名或 walkExt 路径写错，
  // 那时 SS10b 的"读不到定义"会被读成"定义丢了"——先让下限把面本身的问题报出来。
  ['memlayout 子树面非空', (m) => (m.memlayoutTree.length > 2000 ? 1 : 0), 1],
  // SS17/19/21c/22/26/30 的读数都吃 `hostDesktop`：面瞎了（扫描根改名 / tests 过滤器
  // 过宽）会得到 `none` 或空集，其中 SS17b/SS22c 恰好期望 `none` ⇒ "绿着瞎掉"。
  // 文件数下限取现值 144 的七成。
  ['宿主生产文件数（排 tests）', (m) => m.hostDesktop.length, 100],
  // SS17 三档同样是文件集合，其中 SS17b 的右档期望 `none` ⇒ 靠下限把面钉住：
  // spill 面（现值 1 个主文件 + 将来的子树）剥注释后不会短，落盘本体必须在面里。
  ['spill 面含落盘本体', (m) => countOf(m.spill, /fn put\s*\(/), 1],
  ['style.css 含基座定义', (m) => countOf(m.css, /--lw-ui-font-size:\s*\d+px/), 1],
  // 现值 11（tests_permission + tests_approval 目录 10 个 .rs），下限取八成
  ['审批测试面文件数', (m) => m.apprTests.length, 8],
  // 现值 17（settings/ 树里的 .rs，S4 刀③c 已删 memory/ 子目录），下限取七成
  ['设置分区面文件数', (m) => m.settingsParts.length, 12],
  ['前端设置 store 含 applyInstant 定义', (m) => countOf(m.settingsStore, /async applyInstant\(/), 1],
  // 现值 9（plugins-src 目录数，含市场夹具 tool-shout；S4 刀③c 删 memory-sqlite 后
  // 构建清单是 8 个内置插件 + 1 个市场夹具）。下限取八成：plugins-src 整个改名或删掉时，
  // SS7 的"用例数 = 目录数"会变成 0 = 0 而照样绿——这种"两边一起消失"只能由下限兜。
  ['内置插件数', (m) => m.plugins, 8],
  // 对账用例的扫描面本身也要有下限：全是散在各 id 的 src/ 树里，文件数掉一半 = 面瞎了
  // （现值 44，下限取七成）。
  ['插件 src 面文件数', (m) => m.pluginTests.length, 30],
  // 现值 5（S4 刀③ 删掉 MemoryInject 后的五个内核环）。SS8 的期望分母是"实现块数"，
  // 环整体被搬出扫描面时分子分母一起掉 ⇒ 只能由这条下限兜。下限取 4：留一点重构余量
  // （把一个环拆成两个文件只会更多，不会更少）。
  ['注入环 impl 块数', (m) => ringImpls(m), 4],
  // 现值 85（components 树的 .vue，已删记忆面板）。SS9 的期望是"左=右"，扫描面整个塌掉时
  // 0|0 照样绿——那种"一起消失"只能由下限兜（与 SS7 那条同一条理由）。
  ['前端组件 .vue 数', (m) => m.comps.length, 60],
  // 现值 6（挂了 document/window 级外点的浮层组件）。下限取一半多一档：少于这个数就说明
  // 扫描面或形状口径漂了，此时 SS9 的"左=右"没有意义。
  ['浮层外点站点数', (m) => dismissCount(m), 4],
  // 现值 197（desktop/src 的 .ts+.vue，已删记忆包装层与面板）：SS9b/SS9c 的分子分母
  // 都从这份面里数，下限取七成。
  ['前端 src 扫描面文件数', (m) => m.frontSrc.length, 140],
  // S2 刀① 的扫描面（SS16 系列）。S4 刀③b 删掉 organize 面那条下限：那个面随
  // organize 整面退役而不存在了（SS16b 同刀退场）。留下的中立尺面在"面瞎了"
  // （目录改名 / walkExt 写错）与"真的没有"之间读数完全相同，所以仍要一条下限兜。
  // ⚠ 中立尺这条下限取的是**字节数现值 919 的四成**而不是点名定义本体：本模块的注释比
  // 代码长（整条理由就是"为什么要封顶、为什么预留是这两颗数"），剥掉注释只剩 919 字节，
  // 阈值取 1200 会天天假红；而拿 `fn anchor(` 当下限又与 SS16a 的右档同分子分母
  // （定义改名时报的是"闸瞎了"，说的却是假话）。拆树只会让面变大，所以字节数够用。
  ['中立尺面字节数（剥注释）', (m) => m.budget.length, 400],
  // S2 刀④（SS20/SS21）同一条瞎法：SS21b 期望两个 0、SS21a 期望 1，面塌掉时
  // "0|0"与"没扫到"读数相同。下限取现值 16288 的三成；再加一条点名判定本体
  // `fn plan(`——它不是这一簇任何一档读数的分子（SS20b 数 include、SS21a 数
  // STRATEGY、SS21c 数构造点与调用点），所以"扫错了树"这种瞎法兜得住。
  ['主层压缩面字节数（剥注释）', (m) => m.compact.length, 4800],
  ['主层压缩面含判定本体', (m) => countOf(m.compact, /fn ceiling\s*\(/), 1],
  // 内核生产面（SS31 的扫描面）：文件数下限 —— 面塌掉时 `filesWith` 读成 'none'，
  // 与"真的没有调用方"读数相同，那正是 SS30b 说明里记过的老坑。
  ['内核生产面文件数', (m) => m.kernel.length, 25],
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
    SS1b: (m) =>
      (m.memlayoutTree = m.memlayoutTree.replace(
        'const PROJECTS_DIR: &str = "projects";',
        'const PROJECTS_DIR: &str = "proj";',
      )),
    // 索引预算两档各一条窄变异（S1 刀①②a）：SS10 数"全仓是不是只有一份数字"，
    // SS10b 数"那一份在不在 memlayout 子树"。两条互不遮挡靠的是同一机制：
    // m.rust / m.memlayoutTree 是两次独立读盘，改一份不动另一份。
    // （SS10c「宿主再导出」随 `memory_index.rs` 在 S4 刀③c 删除而退场。）
    SS10: (m) =>
      m.rust.push(
        'pub(crate) const MAX_INDEX_LINES: usize = 200;\npub(crate) const MAX_INDEX_CHARS: usize = 25_000;',
      ),
    SS10b: (m) => (m.memlayoutTree = m.memlayoutTree.replace('pub const MAX_INDEX_LINES: usize = 200;', '')),
    // S2 刀① 的变异（SS16a/SS16c）。SS16a 只加定义 ⇒ 右档（中立面）不动，
    // SS16c 只再写预留常量。两条都长成"没人调用的分支"是有意的（同
    // SS12/SS13 的分工）：cargo 对它们全绿，只有形状档看得见——而这正是刀① 之后
    // 唯一还剩的证明面（搬家本身没有新行为，行为测试无靶子）。
    // ⚠ S4 刀① 改判后 SS16c 只剩三档，变异只打"再写一颗 `= 21_000` 的常量"——
    // 其余两档（定义处数 32_000 / 有效阈值函数）不动，分工仍然成立。
    // S4 刀③b：SS16b（organize 面自己不许拿尺）随那个面一起退场，变异同删。
    SS16a: (m) =>
      m.rust.push(
        'impl OrganizerTwin {\n    fn anchor(session: &Session) -> (u64, u64) { (0, 0) }\n}\n',
      ),
    SS16c: (m) =>
      m.rust.push('\npub const OUTPUT_RESERVE_CAP_TWIN: u64 = 21_000;\n'),
    // S2 刀② 的三档变异，同样各打中自己那一项：
    //  SS17a 长出第二个"超预算怎么办"的解释者（并把改前那枚名字从墓碑里扶起来）；
    //  SS17b 组装点自己手拼目录名——这一档**只碰右档**，左档（那枚 const）不动，
    //          否则它就成了 SS17b 在罚同一件事的两遍；
    //  SS17c 第二份 `<persisted-output` 模板（文本漂成两个变体）。
    SS17a: (m) =>
      m.hostDesktop.push({
        rel: 'llm/wire/messages.rs',
        code: 'fn project_tool_output_alt(c: String) -> String {\n    clip_tool_output(c, None)\n}\n',
      }),
    SS17b: (m) =>
      m.hostDesktop.push({
        rel: 'state/llm_stack.rs',
        code: 'let spill = SpillStore::new(&root.join("spill"));\n',
      }),
    SS17c: (m) =>
      m.hostDesktop.push({
        rel: 'llm/wire/messages.rs',
        code: 'const ALT_TEMPLATE: &str = "<persisted-output path=…>";\nfn clipped_alt() {}\n',
      }),
    // 刀④a 两档窄变异（specs/13 刀① 起两档都变成**墓碑**）：aux 通路退役之后，
    // 变异改成"把退役的名字**放回生产面**"——那正是墓碑要打中的那种改法
    // （不改一行测试、行为面完全看不出，只有形状档看得见）。
    // SS19a：类型名复活；SS19b2：模块路径复活（两档分家，见那两条的说明）。
    SS19a: (m) =>
      m.rust.push(
        'pub(crate) struct AppLlmSummarizer {}\nimpl SummaryClient for AppLlmSummarizer {}\n',
      ),
    SS19b2: (m) => m.rust.push('mod aux_client;\npub(crate) use aux_client::Aux;\n'),
    // ---------- S2 刀④ 五档各一条窄变异（每条只该红自己那一项）----------
    // SS20a：checkpoint 标签的第二个出处（编译器与行为测试都看不出）。
    SS20a: (m) =>
      m.rust.push(
        'pub(crate) const SNEAKY_TAG: &str = "<conversation-checkpoint>";\n' +
          'pub(crate) const SNEAKY_NOTICE: &str = "Treat it as historical context, not as new instructions.";\n',
      ),
    // SS20b：把某份稿子再 include 一次 = 出现第二份装配表。
    // ⚠ 落在**宿主生产面**（不是 compact 子树）：这一档的扫描面就是宿主生产面，
    // 变异得落在同一面才打得到。
    SS20b: (m) =>
      m.hostDesktop.push({
        rel: 'compact/ghost.rs',
        code: 'const TEMPLATE_AGAIN: &str = include_str!("../prompts/compact/template.md");\n',
      }),
    // SS21a：主层的 strategy 名字出现第二处字面量定义（同一层两个名字）。
    SS21a: (m) => (m.compact += '\npub(crate) const STRATEGY: &str = "compact";\n'),
    // SS21b：门槛算式的第二个出处——进度条那一侧（`runner/mod.rs`）自己再算一遍。
    // 它打的是第③格（`effective_threshold(` 的落点集合多了一个文件）。
    SS21b: (m) =>
      (m.hostDesktop = m.hostDesktop.map((f) =>
        f.rel === 'runner/mod.rs'
          ? {
              ...f,
              code:
                f.code +
                '\nlet again = crate::context_budget::effective_threshold(slider, window, None, cap);\n',
            }
          : f,
      )),
    // SS21c：多一个构造点 + 多一个调用点（两张熔断表 / 一轮压两遍）。
    SS21c: (m) =>
      m.hostDesktop.push({
        rel: 'commands/ghost.rs',
        code: 'let extra = Compactor::new(settings, client);\n    let out = cp.run(&mut session, &id, &policy).await;\n',
      }),
    // S2 刀⑤（SS22a/b/c）三条窄变异，各打红一档且**不许连带**：熔断线在第二处再比
    // 一遍 / 组装点之外多 new 一枚表 / 多一个 `fused` 读者。三条都长在编译器与
    // 行为测试看不见的位置上；S4 刀① 退役保护网后，第三条从"保护网开始记账"改判为
    // "再长出一个熔断判断读者"。
    SS22a: (m) =>
      (m.compact +=
        '\nfn sneaky_fused(n: u32) -> bool {\n    n >= MAX_COMPACT_FAILURES\n}\n'),
    SS22b: (m) =>
      m.hostDesktop.push({
        rel: 'commands/chat.rs',
        code: 'let second_table = crate::compact::Failures::default();\n',
      }),
    SS22c: (m) =>
      m.hostDesktop.push({
        rel: 'commands/chat.rs',
        code: 'let fused = self.failures.fused(sid);\n',
      }),
    // S2 刀⑦（SS24 / SS25）两条窄变异：后端多一条没人配文案的判据 / 前端多一句显示不出来的文案。
    // 分两档各埋一侧，是因为相等型读数一旦两侧同时漂就看不出是谁动的（SS25 说明里那条）。
    // SS24：后端那张 `REASONS` 表多一枚（前端没配 ⇒ 界面会掉进兜底那条）。
    SS24: (m) =>
      (m.compact = m.compact.replace(
        /const REASONS: \[&str; \d+\] = \[[\s\S]*?\];/,
        'const REASONS: [&str; 10] = ["nothing-to-compact","template-mismatch","no-summary",' +
          '"output-limit","cannot-shrink","provider-error","content-filter","interrupted","budget",' +
          '"brand-new-reason"];',
      )),
    SS25: (m) =>
      m.frontSrc.push('const keys = {\n  "brand-new-reason": "notice.compactBrandNew",\n};\n'),
    // SS31（specs/13 刀①）：压缩面之外多一个 `prepare_request(` 调用点 = 有人另开了
    // 一条摘要装配通路（第二份拼装）。它打的是"调用点集合多了一个文件"那一格。
    SS31: (m) =>
      m.hostDesktop.push({
        rel: 'commands/ghost.rs',
        code: 'let req = kernel.prepare_request(session, Some(sid)).await;\n',
      }),
    // SS32（specs/13 刀①）：界面那份候选清单与后端那份校验权威漂了一档。
    // 它打的是"两侧逐个相等"那一格（缺省那颗另算，同样会红）。
    SS32: (m) =>
      (m.autoCompactPct = m.autoCompactPct.replace('= [25, 50, 85]', '= [25, 50, 80]')),
    // 窄变异（刀⑧）：把三条发送收口里的**第一条** `refusal_note` 调用删掉 ⇒ 次数 3→2 而
    // 文件集合不变，证的正是这一档独有的那一格：漏一条收口 = 那一路请求被拒之后不记账，
    // 自锁照旧存在。用 map 换新对象而不是原地改 `f.code`：selftest 的快照只做了浅拷贝，
    // 原地改会让这一档污染后面每一档（SS9/SS9c 那对注释记过同一个坑）。
    SS26: (m) => {
      m.hostDesktop = m.hostDesktop.map((f) =>
        f.rel === 'llm/app_llm.rs'
          ? {
              ...f,
              code: f.code.replace('        self.refusal_note(sid.as_deref(), est, &out);\n', ''),
            }
          : f,
      );
    },
    // 窄变异（S3 刀①）：在**生产面**复活旧标签。它证的正是这一档独有的那一格——
    // 内核那 20 条注入用例全拿假件源跑，把 `<persistent_memory>` 再包一处它们一条都不红，
    // 只有这条读数会红，且红话说的是"旧标签回来了"。
    SS27: (m) =>
      m.rust.push(
        'pub fn _revive(s: &str) -> String {\n    format!("<persistent_memory>\\n{s}\\n</persistent_memory>")\n}\n',
      ),
    // 窄变异（刀③b 墓碑两档，各证自己那一格）：
    // SS29 = 把 trait 方法与契约载荷**塞回宿主生产 Rust**（"通路被捡回来"那种改法：
    //        它不改一行测试、行为面完全看不出，只有形状档看得见）。
    SS29: (m) =>
      (m.learnGhostRust +=
        '\nfn recall_relevant(&self, q: &RecallQuery) -> Vec<MemoryEntry> { Vec::new() }\n'),
    // SS29b = 同一件事在前端那一侧的形状：设置面长回一格开关 + 组件读一个后端已经不发的
    //         字段（后者静默读成 undefined，界面上就是少一个数——本刀正是被这种缺陷撞出来的）。
    SS29b: (m) =>
      (m.learnGhostFront +=
        '\nexport const learnEnabled = true\nconst shown = view.explain.present\n'),
    // （SS29c 的正对照随 `recall` 通路在 S4 刀③c 整条删除而退场：那档原本守的是
    //  "前两档的 0 不能是删光换来的"，现在通路就是被有意删光——0 = 真删净。）
    // SS30 窄变异（S3 刀④a · ⓟ）：把**进度条那一次**调用换成锚点直取。改后 `fn occupancy(`
    // 的定义仍在（①不红）、两个组装点仍走 assemble（③不红），只有②从 2 掉到 1 ——这一档
    // 独有的形状就是"消费点少了一个"。为什么这一格非要窄变异：`context_budget/tests.rs`
    // 那五条 occupancy 用例证的是函数自己算得对，把调用方换掉它们一条都不红。
    SS30: (m) =>
      (m.hostDesktop = m.hostDesktop.map((f) =>
        f.rel === 'runner/mod.rs'
          ? {
              ...f,
              code: f.code.replace(
                'context_budget::occupancy(session, u.refused_tokens)',
                'context_budget::anchor(session).0',
              ),
            }
          : f,
      )),
    // SS30b 宽变异（同一刀的缺席档）：把两把旧尺各塞一条回生产面——装配口漂成手写字面量
    // （ⓛ 那个形状）+ 组装后口径复活（ⓟ 那个形状）。两档各红各的，push 一次同时打中。
    SS30b: (m) =>
      m.hostDesktop.push({
        rel: 'state/ghost.rs',
        code:
          'let p = UsagePolicy { threshold_tokens: 1, window_tokens: 2, tools_est_tokens: 3, refused_tokens: 0 };\n' +
          'let again = asm.context_tokens(&s, sid);',
      }),
    SS2: (m) => (m.css = m.css.replace('--lw-ui-font-size: 14px', '--lw-ui-font-size: 15px')),
    SS2b: (m) => (m.css = m.css.replace('--lw-ui-font-size: 14px;', '--lw-ui-font-size: 14px;\n  --lw-ui-font-size: 16px;')),
    // 窄变异：把 T5 刚去掉的那次"为读三个字段而全量克隆档位区"塞回去。
    // 判据要能为它报红，否则 SS3 只是抄了个数字、不在盯任何东西。
    SS3: (m) =>
      (m.appLlm = m.appLlm.replace(
        'let image_pin = facts.supports_image;',
        'let _dup = self.settings.llm_config();\n        let image_pin = facts.supports_image;',
      )),
    // 窄变异（T7）：把改前那种"在自己用例里手写一份轮询"塞回测试面。
    // SS4 要能为它报红，否则这条档只在盯一个数字、不在盯挂起脆弱性有没有回潮。
    SS4: (m) =>
      m.apprTests.push(
        '#[tokio::test]\nasync fn sneaky_decider() {\n    let r = decider.pending().first().cloned();\n}\n',
      ),
    // 窄变异（T15）：把一段"就地铺开"的校验塞回编排层——这正是本刀搬空的那形状。
    // SS5 要能为它报红，否则 '3|11' 只是抄了两个当下的数字。
    SS5: (m) => (m.settingsUpdate = m.settingsUpdate.replace(
      '        if let Some(u) = embedding {',
      '        if let Some(u) = embedding {\n            if u.model.is_none() {\n                return Err("模型未填".into());\n            }',
    )),
    // 另一侧的变异：把某个分区的自持实现改名（转发还在、定义没了 = 编排层在替它做事）
    SS5b: (m) => (m.settingsParts = m.settingsParts.map((s) => (s.includes('fn apply_update(&mut self, u: ToolsUpdate') ? s.replace('fn apply_update(&mut self, u: ToolsUpdate', 'fn apply_update_renamed(&mut self, u: ToolsUpdate') : s))),
    // 窄变异（T16）：把"失败时按 this.view 回滚"那个缺陷形状原样写回去。
    // SS6 要能为它报红，否则 '3|0|1' 只是在抄三个当下的数字。
    SS6: (m) => (m.settingsStore = m.settingsStore.replace('if (before) apply(before);', 'if (this.view) apply(this.view);')),
    // 窄变异（T11）：把**一个**插件的对账用例改名——清单与 describe 都还在，但那条对账
    // 不再跑，于是"UI 读的那份"与"审批读的那份"又可以各写各的。只摘第一个：全摘会让
    // 实测与期望一起掉（期望是插件数、不随用例变），反而更像"少一个"的真形状。
    SS7: (m) => {
      const hit = m.pluginTests.findIndex((s) => s.includes('fn manifest_json_and_describe_agree('));
      m.pluginTests = m.pluginTests.map((s, i) =>
        i === hit
          ? s.replace('fn manifest_json_and_describe_agree', 'fn manifest_json_and_describe_agree_retired')
          : s,
      );
    },
    // 另一侧：一个插件的清单被删走（用例还在跑、读的是空气）——SS7b 要单独能为它红
    SS7b: (m) => (m.pluginManifests -= 1),
    // 窄变异（T2）：把一个环摘回"自己 from_value"的旧形状——take_request 调用数掉 1，
    // 实现块数不动。只摘第一个：全摘会把两侧一起改掉，反而看不出少了谁。
    SS8: (m) => {
      const hit = m.rings.findIndex((s) => s.includes('take_request(payload,'));
      m.rings = m.rings.map((s, i) =>
        i === hit ? s.replace('take_request(payload,', 'take_request_legacy(payload,') : s,
      );
    },
    // 本刀真正的缺陷形状：环还接共用取还，却又加一次整包克隆。SS8b 要单独能为它红
    // （SS8 看不见它——两侧读数都不变），这条对照就是为了证明"两个方向各有人盯"。
    SS8b: (m) => {
      const hit = m.rings.findIndex((s) => s.includes('take_request(payload,'));
      m.rings = m.rings.map((s, i) =>
        i === hit
          ? s.replace('let mut guard = match', 'let _dup = payload.clone();\n        let mut guard = match')
          : s,
      );
    },
    // 窄变异（T2）：一个环**整体改名**（impl 块还在、take_request 也还在，只有名单点不到它）。
    // SS8 对这种变异两侧一起掉、看不见——那条对照就是为了把这个"分子分母同源"的盲区
    // 摆出来，并证明点名确实补上了它。
    SS8c: (m) => {
      const name = RINGS.find((n) => m.rings.some((s) => s.includes(`impl WaterfallListener for ${n}`)));
      if (!name) return;
      m.rings = m.rings.map((s) =>
        s.includes(`impl WaterfallListener for ${name}`)
          ? s.replace(`impl WaterfallListener for ${name}`, `impl WaterfallListener for ${name}_retired`)
          : s,
      );
    },
    // 窄变异（T10）：新增一个"只挂外点、没挂 Esc"的浮层——正是逐站取证抓到的那三处的
    // 形状。SS9 要能为它报红，否则这条判据只是在抄两个当下的数字。
    SS9: (m) =>
      m.comps.push(
        '<script setup>\nconst open = ref(false)\nonMounted(() => document.addEventListener("pointerdown", onDoc))\n</script>\n',
      ),
    // 窄变异（T10）：把输入框那条链的让位判定退回改前的 `showModels`——优先级面失去唯一
    // 消费者，那三处补的 Esc 立刻又变成"按一次键关一层 + 顺手拒一次审批"。
    // SS9b 要单独能为它红（SS9 看不见它：外点与 Esc 两侧都没动）。
    SS9b: (m) => {
      const hit = m.frontSrc.findIndex((s) => s.includes('if (anyLayerOpen()) return;'));
      m.frontSrc = m.frontSrc.map((s, i) =>
        i === hit ? s.replace('if (anyLayerOpen()) return;', 'if (showModels.value) return;') : s,
      );
    },
    // 另一侧：别处再写一份计数器 = 两本账，漏还不报错。SS9c 要单独能为它红。
    SS9c: (m) =>
      m.frontSrc.push('let openLayers = 0\nexport function anyLayerOpen() { return openLayers > 0 }\n'),
    // 刀⑦b（specs/13 挂账 ①）：把**退役的文本判据**放回压缩面 = 那种改法
    // （不改一行行为测试、界面照样跑，只是"为什么没压成"会说错原因）。
    // ⚠ 落在**宿主生产面**（这一档的扫描面就是它），变异得落在同一面才打得到。
    SS33: (m) =>
      m.hostDesktop.push({
        rel: 'compact/deep/send.rs',
        code: 'fn is_output_limit(msg: &str) -> bool {\n    msg.contains("finish_reason=length")\n}\n',
      }),
  };
  let miss = 0;
  for (const { id, desc } of runChecks(snap)) {
    const mut = MUT[id];
    if (!mut) { console.log(`  ?? ${id} 没有对照 ⇒ 该项证明力未知`); miss++; continue; }
    const copy = {
      ...snap,
      rust: [...snap.rust],
      apprTests: [...snap.apprTests],
      settingsParts: [...snap.settingsParts],
      rings: [...snap.rings],
      pluginTests: [...snap.pluginTests],
      // SS9 与 SS9c 会往这两份面里 push：不各拷一份的话，第一档变异会污染后面每一档
      comps: [...snap.comps],
      frontSrc: [...snap.frontSrc],
      // SS17/SS19/SS21/SS22/SS26/SS30 同样 push 面元素 ⇒ 同一份拷贝纪律
      hostDesktop: [...snap.hostDesktop],
      kernel: [...snap.kernel],
    };
    mut(copy);
    const after = runChecks(copy);
    const row = after.find((r) => r.id === id);
    const caught = row && row.bad;
    // 分工读数：这条变异**还**打红了谁。只有自己一条 = 窄变异（证这一档独有）；
    // 连带别条 = 宽变异（证整体承重，但要说破，不然会误以为每档各有人盯）。
    const also = after.filter((r) => r.bad && r.id !== id).map((r) => r.id);
    console.log(
      `  ${caught ? '✓' : '✗'} ${id} 变异可见：${desc.slice(0, 34)}…${caught ? '' : ` ⇒ 实测仍 = ${row?.got}（对照打空）`}${also.length ? `｜同时打红 ${also.join(',')}` : ''}`,
    );
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
