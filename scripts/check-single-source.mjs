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
  memIndex: 'desktop/src-tauri/src/memory_index.rs',
  rsFont: 'desktop/src-tauri/src/settings/appearance.rs',
  tsFont: 'desktop/src/lib/ui-font.ts',
  css: 'desktop/src/style.css',
  appLlm: 'desktop/src-tauri/src/llm/app_llm.rs',
  settingsUpdate: 'desktop/src-tauri/src/settings/update.rs',
  settingsStore: 'desktop/src/stores/settings.ts',
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
 * 宿主记忆面（S1 刀②c-1 加，SS11 的扫描面）：`memory.rs` + `memory/` 整棵树（排 tests）
 * + `memory_index.rs` + `memory_files.rs`（这两处是目录式，各自只取主文件，tests 不进面）。
 *
 * 口径**刻意不含** `memory_learn/` 与 `recall_log/`：学习档仍按库算 `hit_count`
 * （specs/05 刀②c-2 待拍板），把它们的 SELECT 数进来这条判据就立不住，而把
 * "读源已换"这件事说成"全簇无库访问"是谎报。面为什么按名字列而不是整棵
 * `desktop/src-tauri/src` 扫：那棵树里 `organize/`、`state/`、`commands/` 各自有
 * 与兜底档无关的同名字段（`files_on` 在 `state/` 里 legitimately 仍存在），
 * 判据一宽就变成"这个名字全仓不许出现"——那不是这条判据要说的话。
 */
function hostMemoryFace() {
  const tree = walkExt('desktop/src-tauri/src/memory', /\.rs$/).filter(
    (rel) => !/(^|\/)tests(_?\.rs|\/)/.test(rel),
  );
  const heads = [
    'desktop/src-tauri/src/memory.rs',
    'desktop/src-tauri/src/memory_index.rs',
    'desktop/src-tauri/src/memory_files.rs',
  ].filter((rel) => fs.existsSync(path.join(ROOT, rel)));
  return [...tree, ...heads].map((rel) => read(rel)).join('\n');
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
 * 对账 / 启动导出面（S1 刀②c-2 加，SS12 的扫描面）：两份「读影子 + 写投影」编排模块的
 * 主文件（tests 不进面）。
 *
 * 口径**刻意不含** `memory/` 树：那里 `crud.rs` 走的是**正向**写路径（真源 md 先落、
 * 影子跟后），合法在场。把它数进来这条判据就变成"宿主不许写 md"，说的不是本刀的事；
 * 本刀要说的话只有一句：**副本不许反写文件**。
 */
function reconcileFace() {
  return [
    'desktop/src-tauri/src/memory_reconcile.rs',
    'desktop/src-tauri/src/memory_files.rs',
  ]
    .filter((rel) => fs.existsSync(path.join(ROOT, rel)))
    .map(read)
    .join('\n');
}

/**
 * 插件记忆面（S1 刀③b 加，SS13 的扫描面）：`plugins-src/memory-sqlite/src` 整棵树，
 * **排 tests** 且**剥掉行注释**后拼接。
 *
 * 剥注释是刻意的，而且是这条判据能不能立住的关键：本刀退役的东西在注释里留了大量痕
 * （`store.rs` 头注整段在写"退役掉了 schema.rs / rusqlite / mdsync"）。按字面数名字
 * 会得到一片假红——那是 SS11 第一版撞过的同一个洞（"数的是形状不是名字"）。
 * 而这条判据要说的是「**代码里**不再有打开那张表的通路」，形状本来就只存在于代码行。
 */
function plugMemoryCode() {
  return walkExt('plugins-src/memory-sqlite/src', /\.rs$/)
    .filter((rel) => !/(^|\/)tests(_?\.rs|\/)/.test(rel))
    .map((rel) =>
      read(rel)
        .split('\n')
        .filter((l) => !/^\s*\/\//.test(l))
        .join('\n'),
    )
    .join('\n');
}

/**
 * 宿主生产面（S1 刀③b-2 加，SS14 的扫描面）：`desktop/src-tauri/src` 整根、**排 tests**、
 * 剥行注释，返回 `[{rel, code}]`。`rel` 从 `desktop/src-tauri/src/` 起算——这条判据的读数
 * 是"**哪些文件**在写那张表的内容列"，所以文件名本身就是有信息量的形状，比计数强：
 * 白名单里那个文件被改名 ⇒ 红，别处多出一个出处 ⇒ 也红，两者都不会被"总数没变"糊过去。
 *
 * 排 tests 是刻意的：测试夹具里手写 `INSERT INTO memories` 是"造一个损坏态的库"的唯一办法
 * （`memory/tests/learning.rs` 那批就是这么造的），数进来这条判据永远红。
 * 剥注释与 SS11/SS13 同一个洞：本刀退役的说明要在头注里写清"被摘掉的是哪三种形状"，
 * 按字面数名字会得到一片假红。
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

/** `memories` 内容列的三种写形状（SS14 的三个靶子）。第二档**按列名收窄**是有意的：
 *  `memory_learn/mod.rs` 那条 `UPDATE memories SET hit_count = hit_count + 1` 合法在场，
 *  它学的是使用量、不是记忆内容——不收窄的话这条判据一上手就假红。 */
const MEM_INSERT = /INSERT INTO memories\s*\(/;
const MEM_CONTENT_UPDATE = /UPDATE memories\s+SET\s+(?:value|tags|source|updated_at)\b/;
const MEM_DELETE = /DELETE FROM memories/;
/** 命中形状的文件集合（排序拼接；一个都没有 = `none`）。 */
const filesWith = (list, re) => {
  const hit = list.filter((f) => re.test(f.code)).map((f) => f.rel);
  return hit.length ? [...new Set(hit)].sort().join('|') : 'none';
};
const memWriters = (m, re) => filesWith(m.hostDesktop, re);

/**
 * 硬删调用面（S1 刀④a 加，SS15a / SS15b 的扫描面）：全生产 .rs（`prodRustFiles` 已经排掉
 * tests），再**排掉 `plugin-api/src/memlayout` 自己**——那一片是 `delete_fact` / `clear_facts`
 * 的定义处与内部实现，把它们算进来这两个集合永远非空、判据就成了自证空转。剥行注释同族口径。
 *
 * 为什么"排定义处"而不是像 SS14 那样把定义处列进白名单：memlayout 内部调用硬删是**它自己的
 * 职责**（`obsolete.rs` 的 `purge_obsolete` 就在删文件），那片的路径一旦进集合，"回收区里
 * 谁能删文件"这条纪律就得靠一串会漂的路径来表达。
 */
function hardDeleteFace() {
  return prodRustFiles()
    .map((abs) => path.relative(ROOT, abs).replace(/\\/g, '/'))
    .filter((rel) => !rel.includes('plugin-api/src/memlayout'))
    .map((rel) => ({
      rel,
      code: read(rel)
        .split('\n')
        .filter((l) => !/^\s*\/\//.test(l))
        .join('\n'),
    }));
}

/** 硬删形状。`(?<!soft_)` 这段 lookbehind 是**判据的承重件**：`soft_delete_fact(` 里含着
 * `delete_fact(`，不剥前缀的话每一条合法软删都会被打红——第一版就是这么错的。 */
const HARD_DELETE_CALL = /(?<!soft_)delete_fact\s*\(/;
const HARD_CLEAR_CALL = /(?<!soft_)clear_facts\s*\(/;

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
/** 链位名单（提示词分层设计 §6）：注册序 = system 拼装序，SS8c 按此点名。 */
const RINGS = [
  'ToolExposure',
  'BaseSystemInject',
  'MemoryInject',
  'ThinkingLanguageInject',
  'TreeDirectoryInject',
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
    hostMemoryTree: hostMemoryFace(),
    reconcileTree: reconcileFace(),
    plugMemory: plugMemoryCode(),
    hostDesktop: hostDesktopFace(),
    hardDelete: hardDeleteFace(),
    budget: moduleFace(
      'desktop/src-tauri/src/context_budget.rs',
      'desktop/src-tauri/src/context_budget',
    ),
    organize: moduleFace(
      'desktop/src-tauri/src/organize/mod.rs',
      'desktop/src-tauri/src/organize',
    ),
    spill: moduleFace('desktop/src-tauri/src/spill.rs', 'desktop/src-tauri/src/spill'),
    micro: moduleFace(
      'desktop/src-tauri/src/microcompact.rs',
      'desktop/src-tauri/src/microcompact',
    ),
    compact: moduleFace('desktop/src-tauri/src/compact.rs', 'desktop/src-tauri/src/compact'),
    slide: moduleFace('desktop/src-tauri/src/slide.rs', 'desktop/src-tauri/src/slide'),
  };
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
      '（只剩 locale 与 shortcut 两个根字段各自的拒绝语句）、`.apply_update(` 转发 = 11' +
      '（T15：改前 310 行就地铺开在编排层，而批 Q6 早就把 developer 收进分区自己——同一件事两种写法，' +
      '且丑的那种正在长新字段；写回第三处就地校验 = 让下一个改设置的人再猜一次该放哪）',
    (m) =>
      [countOf(m.settingsUpdate, /return Err\(/), countOf(m.settingsUpdate, /\.apply_update\(/)].join('|'),
    '3|11',
  ],
  [
    'SS5b',
    '且那 11 次转发背后真有 11 个自持的 `fn apply_update` 定义（少一个 = 编排层在替某个分区做事，' +
      '多一个 = 有分区自己留着但已不在链上）',
    (m) => String(m.settingsParts.reduce((a, s) => a + countOf(s, /fn apply_update\(/), 0)),
    '11',
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
    '链上七个注入环**各自还在**（点名清单 = 提示词分层设计 §6 的链位序）。' +
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
    'SS10c',
    '且宿主注入侧取的是**再导出**而不是自己 `const` 一份（`memory_index.rs` 里那条 ' +
      '`pub(crate) use plugin_api::memlayout::{…}`）——退回自定义时 SS10 也会红，' +
      '但那条红说的是"全仓多了一份数字"，说的是另一件事',
    (m) =>
      String(countOf(m.memIndex, /pub\(crate\) use plugin_api::memlayout::\{[^}]*MAX_INDEX_LINES[^}]*\};/)),
    '1',
  ],
  [
    'SS11',
    '「sqlite 兜底档」这套概念在宿主记忆面里**调用/定义/字段三档形状**出现次数恒 0' +
      '（S1 刀②c-1 拆掉的五个名字：`files_face` / `with_files` / `files_on` 是真源可不可选的旧判据，' +
      '`index_rows` / `from_rows` 是"没接线就去库里现合成"那条通路本体）。' +
      '⚠ 数的是**形状**不是名字：注释里提旧名字是必要留痕（那五处头注都在解释为什么它没了），' +
      '按名字数会得到 8 条假红——SS6 早就为同一件事立过这条口径。' +
      '为什么用结构判据而不是行为测试："未接线的 store"在新形状里**根本无法构造**' +
      '（句柄由数据根算出），那条旧用例 `files_switch_off_keeps_pure_sqlite` 一删就没有任何' +
      '测试还挡得住有人把那一档塞回来 ⇒ 这三档 0 就是那块墓碑。' +
      '口径不含 `memory_learn/` 与 `recall_log/`：学习档仍按库算 `hit_count`，' +
      '那是**另一个功能**不是兜底档（specs/05 刀②c-2 待拍板），把它们数进来这条判据就立不住。' +
      '第三档数的是**字段被读**（`self.files_on`）而不是任何同名参数：' +
      '`memory_files.rs` 里 `spawn_initial_export` 的 `files_on` 是对账/首启导出的空转开关，' +
      '它决定"要不要跑后台那一轮"，不决定"数据存哪儿"⇒ 合法在场（判据第一版把它数进去过，' +
      '得到 1 条假红）',
    (m) =>
      [
        countOf(m.hostMemoryTree, /\.\s*(?:files_face|with_files|index_rows|from_rows)\s*\(/),
        countOf(m.hostMemoryTree, /\bfn\s+(?:files_face|with_files|index_rows|from_rows)\b/),
        // 第三档数的是**字段读取**形状而不是声明：`memory_files.rs` 里
        // `spawn_initial_export(… files_on: Arc<dyn Fn() -> bool …)` 那个参数是
        // **对账/首启导出的空转开关**（它决定要不要跑后台那一轮，不决定数据存哪儿），
        // 合法在场。兜底档的特征形状是"store 自己存了这个闭包并在读写路径上问它"。
        countOf(m.hostMemoryTree, /\bself\.files_on\b/),
      ].join('|'),
    '0|0|0',
  ],
  [
    'SS12',
    '「影子 → 文件」这个方向的反写在**对账 / 启动导出面**里：调用档恒 0、转发档恒 2' +
      '（两档一起报——`0|0` 那种"两边一起消失"的读法兜不住面瞎）。刀②c-2 停掉的旧通路正是' +
      '两条 `files.write_fact(...)`：①`memory_reconcile::process_file` 的 `healed` 支（文件没动' +
      '而索引先行 ⇒ 从索引愈合文件）、②`memory_files::run_initial_export` 的补洞段（库里有一行' +
      '而文件没有 ⇒ 按索引把文件写出来）。真源 = md 之后它们的含义反过来：**把人已经删掉的记忆' +
      '在下次启动写回来** ⇒ AC-S1-6 那三条遗忘验证不成立。转发档 2 = `MemoryFiles` 自己的' +
      '`memlayout::write_fact` / `memlayout::soft_delete_fact` 两条出口（第二条是刀④a 把' +
      '`delete_fact` 换成软删之后的形态，**条数不变**——这两档报的就是"对真源落笔的出口有几个"），' +
      '数字变了就是长出第二个' +
      '对真源落笔的出口，那是有意事件、要显式改这里。' +
      '与行为测试的分工：那两格各有用例钉着（`a_shadow_ahead_of_the_file_is_reported_but_never_written`' +
      '/ `initial_export_rebuilds_tree_md_and_writes_no_fact_files`），这条判据**独有**的是' +
      '"没人调用的一条新反写分支"——它不经过任何测试，只有秒级档看得见（`make quick` 不跑 cargo）',
    (m) =>
      [
        countOf(m.reconcileTree, /\b(?:files|self)\s*\.\s*(?:write_fact|(?:soft_)?delete_fact)\s*\(/),
        countOf(m.reconcileTree, /\bmemlayout::(?:write_fact|(?:soft_)?delete_fact)\s*\(/),
      ].join('|'),
    '0|2',
  ],
  [
    'SS13',
    '记忆插件（`plugins-src/memory-sqlite/src`，排 tests、**剥掉行注释**）里"打开那张表"的' +
      '三个形状恒 0：`rusqlite` 引用 / `… memories` 表名 SQL / `Connection::open`' +
      '（S1 刀③b 的墓碑 = AC-S1-9 要的那条"记忆写路径只许一个出处"）。' +
      '为什么这条必须是结构判据而不是行为测试：把某枚工具退回"先写库再同步文件"这种形状，' +
      '既有那 31 条接口用例**一条都不会红**——它们判的是工具契约（返回字段、双层纪律、' +
      'undo 还原），存储换成什么都照样绿；换句话说"换了存储"这件事在行为面上是不可见的，' +
      '那就只能由形状判据来守。剥注释的原因写在 `plugMemoryCode()` 上：本刀退役的东西' +
      '在注释里留痕极多（`store.rs` 头注整段在解释它删了什么），按名字数会得到一片假红' +
      '——与 SS11 第一版撞的是同一个洞。' +
      '口径不含宿主：宿主那一侧仍合法地开着这张表（影子 + 学习量两列，' +
      'specs/05 刀②c-2 第 2 件等拍板），要盯的是"插件不许把库捡回来"',
    (m) =>
      [
        countOf(m.plugMemory, /\brushqlite\b/),
        countOf(m.plugMemory, /(?:INSERT\s+INTO|UPDATE|DELETE\s+FROM|FROM)\s+memories\b/),
        countOf(m.plugMemory, /\bConnection::open\b/),
      ].join('|'),
    '0|0|0',
  ],
  [
    'SS14a',
    '宿主生产面（`desktop/src-tauri/src` 整根，排 tests、剥行注释）里 `INSERT INTO memories (` ' +
      '这一形状**只许出现在白名单文件里**（S1 刀③b-2：四条 `shadow_*` 退役之后，影子内容列' +
      '的唯一维护者 = `memory/reconcile.rs`，另一处是 `memory/schema.rs` 那份旧库整表重建迁移）。' +
      '读数用**文件集合**而不是计数：计数只挡得住"多一处"，挡不住"白名单里那两处搬到了别的文件"' +
      '与"某处消失了"——集合形状一变就红，红得能直接看见是谁。' +
      '与行为测试的分工同 SS12/SS13：往写路径塞一条**没被调用**的影子 upsert，cargo 全绿' +
      '（磁盘变异 CASE 40 就是这么证的），只有这一档看得见',
    (m) => memWriters(m, MEM_INSERT),
    'memory/reconcile.rs|memory/schema.rs',
  ],
  [
    'SS14b',
    '同一面上**没有任何**按列 UPDATE 记忆内容的形状（`UPDATE memories SET value|tags|source|' +
      'updated_at`）——这是 `shadow_update_value` 的墓碑。口径按列名收窄是有意的：' +
      '`memory_learn/mod.rs` 那条 `UPDATE memories SET hit_count = hit_count + 1` 合法在场，' +
      '它学的是使用量不是内容；不收窄的话这条判据一上手就假红。' +
      '期望 `none` 的两档（本档与 SS14c 的反面）都由 FLOORS 的面下限兜着，面瞎了不会读成绿',
    (m) => memWriters(m, MEM_CONTENT_UPDATE),
    'none',
  ],
  [
    'SS14c',
    '同一面上 `DELETE FROM memories` 只许 `memory/reconcile.rs` 一处（对账删幽灵行）。' +
      '`shadow_delete` 与 `shadow_clear` 随刀③b-2 消失 ⇒ 这一档现在也挡住"清空走 SQL"的回头路：' +
      '真源是文件，清空 = 删文件（`memlayout::clear_facts`），走 DELETE 就是拿副本当契约',
    (m) => memWriters(m, MEM_DELETE),
    'memory/reconcile.rs',
  ],
  [
    'SS15a',
    '硬删一条记忆的调用点在全生产面上**只许一个白名单文件**（S1 刀④a · AC-S1-7：删除改成' +
      '软删进 `obsolete/` 之后，"把文件真删掉"只剩一处合法身份 = `memory_undo` 撤销一次' +
      '**新建**（那条用户从来没见过，留在回收区会让那个目录变成补偿日志））。管理页删除、' +
      '工具删除、清空全部走 `soft_*`。形状是 `delete_fact(` 且**前面不是 `soft_`**（那段 ' +
      'lookbehind 是承重件：不剥前缀每条合法软删都会被打红）；面排掉 `memlayout` 自己' +
      '（那是定义处，`purge_obsolete` 在里面删回收区的文件是它的职责）',
    (m) => filesWith(m.hardDelete, HARD_DELETE_CALL),
    'plugins-src/memory-sqlite/src/store/undo.rs',
  ],
  [
    'SS15b',
    '硬删整层的调用点恒 0（`clear_facts(`，同样剥 `soft_` 前缀）：管理页"清空"自刀④a 起是' +
      '`soft_clear_facts`——一次动作影响整个记忆面，可逆与不可逆的差别必须落在实现里而不落在' +
      '按钮文案里。这一档期望 `none` ⇒ 由 FLOORS 的面下限兜"扫不到东西"那种假绿',
    (m) => filesWith(m.hardDelete, HARD_CLEAR_CALL),
    'none',
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
    'SS16b',
    'organize 面上**自己不再拿尺**：`reported_input_tokens` 与 `fn anchor(` 两档各恒 0' +
      '（刀① 的墓碑。搬完不许 organize 一边调中立锚点、一边留一条自己的取数通路——' +
      '那正是 S1 各批点过名的"两个出处"形状）。⚠ 数的是代码行：organize 的沿革注释里' +
      '必然提到这两个名字（`mod.rs` 头注就在解释搬家），按字面连注释数会得到一片假红——' +
      'SS11/SS13 立过的同一口径。期望 0 ⇒ 由 FLOORS 的点名下限兜"面瞎了"' +
      '（面为空与真的没有，在这一档读数上长得一模一样）',
    (m) => `${countOf(m.organize, /reported_input_tokens/)}|${countOf(m.organize, /\bfn anchor\s*\(/)}`,
    '0|0',
  ],
  [
    'SS16c',
    '有效阈值那三颗零件各只有一个定义：输出侧缺省预留 `32_000` / 预留上限 `21_000` /' +
      ' `fn effective_threshold(`（specs/09 AC-S2-2「这一式只有一个出处」的机械面）。' +
      '为什么盯常量而不盯数值：ZCode `policy.ts:9-10` 那两颗数是要逐字对齐的参照，' +
      '别处再写一份 `21_000` 就是第二条加减通路——表现是"进度条离满还差一截就压"或' +
      '反过来"撞窗了才压"，两种都靠人肉才发现。改动这里 = 显式改判据',
    (m) =>
      [
        countOf(m.rust.join('\n'), /const DEFAULT_OUTPUT_RESERVE_TOKENS: u64 = 32_000;/),
        countOf(m.rust.join('\n'), /const OUTPUT_RESERVE_CAP_TOKENS: u64 = 21_000;/),
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
    'SS18a',
    '微压缩那七颗参数**各只有一个定义处，而且钉的是值**（S2 刀③）：占位文本 + ' +
      '保留 5 组 + idle 60 分钟 + 最小节省 256 + 比例 0.9 + 定值缓冲 2000 + 九工具名单。' +
      '为什么连值一起钉（同 SS16c 的口径）：这七枚是逐字照 ZCode `microcompact.ts:12-31` ' +
      '读实来的（2026-10-19 按本地源码逐行核对），它们决定的是"清得太狠还是太松"——' +
      '漂一枚的表现是历史莫名缺一截工具输出、或者减速层从不工作，两种都得靠人肉跑长会话才发现。' +
      '别处再写一份同样的 `= 5` / `= 256` 就是第二条参数通路 ⇒ 改动这里 = 显式改判据。' +
      '**刀⑥ 的一次搬家已按本档自己的规矩改判**：「保留 5 组」抬成设置项，声明从 ' +
      '`microcompact.rs` 搬到 `settings/context.rs::MICRO_KEEP_GROUPS_DEFAULT`，' +
      '第二档钉的仍是那颗 5（值没漂，漂的只有住处）。' +
      '⚠ 数的是**全生产 .rs 原文**（含注释）：头注里若出现 `const MICRO_KEEP_GROUPS_DEFAULT: u64 = 5;` ' +
      '这种完整声明句式会被算进来，所以注释一律写成中文散文而不是复刻声明',
    (m) => {
      const all = m.rust.join('\n');
      return [
        countOf(all, /pub\(crate\) const CLEARED: &str = "\[旧工具结果已清理\]";/),
        countOf(all, /const MICRO_KEEP_GROUPS_DEFAULT: u64 = 5;/),
        countOf(all, /const IDLE_THRESHOLD_MINUTES: u64 = 60;/),
        countOf(all, /const MIN_TOKEN_SAVINGS: u64 = 256;/),
        countOf(all, /const THRESHOLD_RATIO: f64 = 0\.9;/),
        countOf(all, /const THRESHOLD_BUFFER_TOKENS: u64 = 2_000;/),
        countOf(all, /const COMPACTABLE_TOOLS: &\[&str\] = &\[/),
      ].join('|');
    },
    '1|1|1|1|1|1|1',
  ],
  [
    'SS18b',
    '微压缩这一层**自己也不拿尺**（SS16b 的同族墓碑）：`effective_threshold(` / ' +
      '`session_capacity(` / `window_tokens` 三档在 `microcompact` 模块面里各恒 0——' +
      '它吃的主层阈值必须由组装点经闭包供给。为什么这一档要紧：微压缩线是"主层阈值 × 0.9"，' +
      '一旦它自己去读窗口，就有了第二条窗口换算通路，表现是减速层与整理触发线**不再同尺**' +
      '（要么提前清得不像话，要么永远轮不到它清）。三档读数在同一个面 ⇒ 由 FLOORS 的 micro 面字节' +
      '下限兜"面瞎了"（面为空与真的没有在这一档读数上长得一模一样）',
    (m) =>
      `${countOf(m.micro, /effective_threshold\s*\(/)}|${countOf(m.micro, /session_capacity\s*\(/)}|${countOf(m.micro, /window_tokens/)}`,
    '0|0|0',
  ],
  [
    'SS18c',
    '接线与记账各只有一个落点：`MicroCompact::new(` 只住在 `state/waterfall.rs`（注册一次），' +
      '策略名 `strategy: "micro"` 只住在 `microcompact.rs`。前者防"接了两遍"——这一环每请求' +
      '都要重算并改写消息，注册两次就是同一批结果被清两遍、`ContextCompacted` 落两条事件；' +
      '后者防别处（前端或 organize）再发明一个 "micro" 字符串：`ContextCompacted{strategy}` ' +
      '是按名字渲染的，名字有了第二个出处，界面就会把同一件事显示成两件事。' +
      '数的是 `hostDesktop` 面（已剥行注释 ⇒ 沿革注释里提这两个名字不算在场，SS11/SS13 同一口径）',
    (m) =>
      `${filesWith(m.hostDesktop, /MicroCompact::new\s*\(/)}|${filesWith(m.hostDesktop, /strategy:\s*"micro"/)}`,
    'state/waterfall.rs|microcompact.rs',
  ],
  [
    'SS19a',
    '辅助补全的直呼客户端**只有一个定义处，而且不在 organize 里**（S2 刀④a 搬家）：' +
      '`struct AppLlmMemoryManager` 与那条 `impl memory_tree::MemoryManagerClient for …` ' +
      '都只住在 `llm/aux_client.rs`。为什么这一档要紧——它是"一次模型补全该走设置里哪一档"' +
      '这件事的唯一实现（服务档整档切换 / 只换模型名 / 都没配就走主模型，三档路由 + 未配置就 ' +
      'Err），整理章节总结与 S2 主层压缩共用这一条；而 organize 按 specs/01 D 节在 S4 整面退役，' +
      '共用件长在那里，退役那天要么把压缩一起拔断，要么被逼着复制一份路由（= 第二个出处）。' +
      '读数用**文件集合**：搬家搬对了红不到，搬回 organize 或再拆一份都当场红且能看见是谁（SS14a 同一条理由）',
    (m) =>
      `${filesWith(m.hostDesktop, /\bpub(\(crate\))? struct AppLlmMemoryManager/)}|${filesWith(
        m.hostDesktop,
        /impl memory_tree::MemoryManagerClient for AppLlmMemoryManager/,
      )}`,
    'llm/aux_client.rs|llm/aux_client.rs',
  ],
  [
    'SS19b',
    'organize 面上**只剩调用方、连所有权都没有**（刀④a 立、刀④c 收到底的那块墓碑）：' +
      '`struct AppLlmMemoryManager` 恒 0（定义不许回来）、`pub use client::` 恒 0（再导出' +
      '也不许留一条——它就是"organize 也拥有这个类型"的假象），而 `AppLlmMemoryManager::new(` ' +
      '**也恒 0**：刀④c 起 organize 连"造"都不做，客户端由组装点注入，整理章节总结与主层' +
      '压缩摘要共用同一个实例（两张实例 = 两份路由状态，而"连败三次"这件事会各自数各自的）。' +
      '第三档故意不收窄成限定路径：非限定的 `AppLlmMemoryManager::new(` 出现在 organize ' +
      '面里同样是自己拿了一条通路。为什么不数裸类型名：SS19a 已经盯住定义与实现块，' +
      '而裸名出现在注释里是必要留痕——数它会一片假红。⚠ 三档都数代码行（organize 头注正在' +
      '解释这次搬家）；三档期望 0 ⇒ 由 FLOORS 的 organize 面下限兜"面瞎了"（面为空与' +
      '真的没有在这一档读数上长得一模一样）',
    (m) =>
      `${countOf(m.organize, /struct AppLlmMemoryManager/)}|${countOf(m.organize, /\bpub use client::/)}|${countOf(m.organize, /AppLlmMemoryManager::new\s*\(/)}`,
    '0|0|0',
  ],
  [
    'SS19b2',
    '补 SS19b 的另一半：它三档全期望 0，于是"构造点被改名 / 整块搬走 / 组装点漏接"这类' +
      '漂法一条也报不出来（0|0|0 照样绿）。这一档改数**落点文件集合**：宿主生产面上 ' +
      '`AppLlmMemoryManager::new(` 必须恰好住在 `state/background.rs` 一个文件里。刀④c ' +
      '把它从 `organize::build_organizer` 提到组装点，为的是让共用件不归属任何一个使用方' +
      '（organize 在 S4 整面退役，压缩不留在那里）。用集合而不是计数：计数只挡得住"多一处"，' +
      '挡不住"搬去别处"与"消失"，集合一变当场看得见是谁（SS14a 同一条理由）。' +
      '⚠ 这一档不许与 SS21c 的 `Compactor::new(` 合并成一条：两个字符串住在同一个文件、' +
      '同一段代码里，合并之后删掉其中一次构造，另一条读数照样绿。',
    (m) => filesWith(m.hostDesktop, /AppLlmMemoryManager::new\s*\(/),
    'state/background.rs',
  ],
  [
    'SS19c',
    '路由那一步（`fn cfg_for`：按设置算出本次调用生效的 `LlmSettings`）只有一处实现 = ' +
      '`llm/aux_client.rs`。为什么单独盯它而不是盯 `flat_profile(` 的调用数：后者在发送侧' +
      '（`AppLlm::prepare`）本来就有合法在场，数它=假红；而 `cfg_for` 这个名字一旦在别处' +
      '再出现，就是有人为新调用方又写了一遍档位选择——表现是"整理走 A 档、压缩走 B 档"，' +
      '而设置页上只有一枚记忆管理器模型，没人能从界面上看出两份路由',
    (m) => filesWith(m.hostDesktop, /\bfn cfg_for\s*\(/),
    'llm/aux_client.rs',
  ],
  // ---------- S2 刀④：主层深度压缩 ----------
  [
    'SS20a',
    '折叠摘要消息开头那串标记 `[earlier conversation summarized]`：全生产 Rust 面只许 1 处' +
      '（真源 = `session-store` 的 `SUMMARIZED_MARKER`，宿主侧要同一条消息必须调 ' +
      '`summarized_message()`）。为什么盯这串字面量而不是函数名：宿主自己敲一份同样的文本，' +
      '编译器与行为测试都不红（两条字符串长得一样），但落进日志的摘要头一行与压缩层' +
      '`to_tokens` 量出来的那一条从此可以不同字——表现是"撤销后窗口对不上"以及' +
      '"界面上说省了 3 万，重放时只省 2.8 万"。',
    (m) => String(m.rust.reduce((a, s) => a + countOf(s, /\[earlier conversation summarized\]/), 0)),
    '1',
  ],
  [
    'SS20b',
    '`prompts/compact/` 那四份稿子各只被 `include_str!` 一次，且唯一读者是 ' +
      '`compact/prompt.rs`（刀④b 的"两套逐字条款不许合成一句话"在**结构面**那一半：' +
      '条款各有自己的文件，才谈得上"没被合并"）。同一个稿子被 include 两次 = 有人复制了' +
      '第二份装配表，日后改文案的人只改一处，表现是"设置里写的自定义压缩指令在某些会话里' +
      '像没生效"——其实生效的是另一份旧表。',
    (m) =>
      ['no-tools', 'template', 'keep-security', 'keep-verbatim']
        .map(
          (k) =>
            String(m.compact.match(new RegExp(`include_str!\\("[^"]*${k}\\.md"\\)`, 'g'))?.length ?? 0),
        )
        .join('|'),
    '1|1|1|1',
  ],
  [
    'SS21a',
    '压缩层写进日志的 `strategy` 字面量各只有一处：`"micro"` 在微压缩那一层，`"compact"` ' +
      '在主层（唯一出处 = `compact/deep.rs` 的 `STRATEGY` 常量）。前端按这个串分流文案 ' +
      '（`projection.ts` 把它原样放进 `notice.contextCompacted` 的参数），同一层出现两个名字 ' +
      '= 界面上"同一层压缩说两种说法"，而开发者面板按 strategy 计数会直接算错。',
    (m) =>
      `${countOf(m.micro, /strategy:\s*"micro"/)}|${countOf(m.compact, /STRATEGY:\s*&str\s*=\s*"compact"/)}`,
    '1|1',
  ],
  [
    'SS21b',
    '主层压缩面上**不许自己拿尺**：`effective_threshold(` 与 `session_capacity(` 各恒 0' +
      '（对微压缩的 SS18b 同族要求，也是刀①"organize 不许自己拿尺"的第三次落地）。触发线' +
      '由组装点算好经 `UsagePolicy` 交进来；这里再算一遍就是两个出处，改了滑杆压缩照旧阈值走。' +
      '⚠ 读 `policy.window_tokens` 是合法的（那是接组装点的数，不是自己算窗口），所以本档' +
      '故意不数 `window_tokens`——数了会红在正确的代码上。',
    (m) => `${countOf(m.compact, /effective_threshold\s*\(/)}|${countOf(m.compact, /session_capacity\s*\(/)}`,
    '0|0',
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
      '`compact.rs::is_fused`）。刀⑤ 起有两个读者要吃同一个判断——主层用它决定"还压不压"、' +
      '滑窗保护网用它决定"醒不醒"。两处各写一遍 `>= 3`，将来调那颗数就会出现"主层已熔断' +
      '而保护网还没醒"或反过来的空档，而这两档在界面上是同一个症状（请求发不出去），' +
      '没有一行日志会说清是谁没跟上。',
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
    '同一张表的写侧与读侧分工各只一处：`failures.of|record|clear` 只在 `compact/deep.rs`' +
      '（失败 +1、压成清零），`failures.fused(` 只在 `slide.rs`（保护网读它决定开不开）。' +
      '主层若开始读 `fused` = 在 [`SS21c`] 那一步之外又多一处"要不要压"的判断；保护网若' +
      '开始 record = 它自己把自己锁死，而这一层根本没有解锁通路（它不调模型，永远不' +
      '"压成"）。两种都在编译器与行为测试的眼皮底下。',
    (m) =>
      `${filesWith(m.hostDesktop, /self\.failures\.(?:of|record|clear)\s*\(/)}|${filesWith(m.hostDesktop, /failures\.fused\s*\(/)}`,
    'compact/deep.rs|slide.rs',
  ],
  [
    'SS23a',
    '刀⑥ 把两颗硬编码常量抬成设置项之后，**旧常量名不许留在原地**：`KEEP_RECENT_GROUPS` ' +
      '在微压缩面恒 0、`KEEP_RECENT_RUNS` 在保护网面恒 0（缺省那两份现在只住在 ' +
      '`settings/context.rs` 的 `MICRO_KEEP_GROUPS_DEFAULT` / `SLIDE_KEEP_RUNS_DEFAULT`）。' +
      '留着它的后果不是编译错而是**双出处**：有人拿界面值传进来、又有一处继续读常量，' +
      '表现是"改了保留组数，某一层的边界还是老样子"，而编译器与行为测试都说不出是谁。',
    (m) =>
      `${countOf(m.micro, /KEEP_RECENT_GROUPS/)}|${countOf(m.slide, /KEEP_RECENT_RUNS/)}`,
    '0|0',
  ],
  [
    'SS23b',
    '两枚新读面各只有一个调用点：`context_micro_keep_groups(` 只住 `microcompact.rs`、' +
      '`context_slide_keep_runs(` 只住 `slide.rs`。这条盯的不是"有没有接上"（接不上会编译错），' +
      '而是**同一轮里读了几次设置**：微压缩那一环刻意"取一次存局部"，因为判定与它下面那句' +
      '记账文本要用同一个数——读第二次就可能在中途被用户保存插队，账上写的组数与实际清掉的' +
      '组数不是同一个。多一个调用点就是把这个窗口重新打开。' +
      '⚠ 两条正则都**带前导点**：不带点会把 `settings.rs` 里那两枚 getter 自己的**定义**' +
      '也数成调用点（第一版就是这么红的）——本档要数的是"谁读了它"，不是"谁定义了它"。',
    (m) =>
      `${filesWith(m.hostDesktop, /\.context_micro_keep_groups\s*\(/)}|${filesWith(m.hostDesktop, /\.context_slide_keep_runs\s*\(/)}`,
    'microcompact.rs|slide.rs',
  ],
  [
    'SS24',
    '刀⑦ `/压缩` 的**判据名两侧对账**：后端交给界面的那批名字（`compact.rs` 里 `Plan::stop("…")` ' +
      '∪ `deep.rs` 的三枚 `WHY_*` ∪ `commands/compact.rs` 的 `REASON_OFF`）与前端 ' +
      '`COMPACT_SKIP_KEYS` 的键集合必须相等——但**手动通路让开的那两条除外**，而那两条也是从代码里 ' +
      '推出来的（`if !input.manual … Plan::stop("x")` 那两格），不写死在闸里。' +
      '为什么这条必须有：后端只回名字、人话住语言包（AGENTS「文案住语言包」+ 同类只许一种形态），' +
      '所以 Rust 侧新增一条 stop 而前端没补文案时，**编译与行为测试都说不出**——界面只会掉进兜底那条，' +
      '把一句本来有准确说法的判据显示成 `没压缩：brand-new-name`。反方向同样要红：前端多一条键 ' +
      '= 一句永远显示不出来的文案留在语言包里。' +
      '⚠ why 那三枚原来是就地敲的字符串，本档把它们收成 `WHY_*` 常量（与 `STRATEGY` 同一条纪律）——' +
      '不收的话，闸只能去匹配 `if overflow { "overflow" }` 这种句子，那式子一改措辞就假绿。',
    (m) => {
      const rust = new Set([
        ...[...m.compact.matchAll(/Plan::stop\("([a-z-]+)"\)/g)].map((x) => x[1]),
        ...[...m.compact.matchAll(/const WHY_[A-Z]+: &str = "([a-z-]+)"/g)].map((x) => x[1]),
        ...m.hostDesktop
          .flatMap((f) => [...f.code.matchAll(/const REASON_OFF: &str = "([a-z-]+)"/g)])
          .map((x) => x[1]),
      ]);
      // 手动让开的那两条：从 `!input.manual` 那两格的**结构**里推，不在这里抄名单
      const yielded = new Set(
        [...m.compact.matchAll(/!input\.manual[\s\S]{0,120}?Plan::stop\("([a-z-]+)"\)/g)].map(
          (x) => x[1],
        ),
      );
      const front = new Set(
        m.frontSrc
          .flatMap((s) => [...s.matchAll(/^ {2}"?([a-z-]+)"?: "notice\.compact/gm)])
          .map((x) => x[1]),
      );
      const want = new Set([...rust].filter((r) => !yielded.has(r)));
      const same =
        want.size === front.size && [...want].every((r) => front.has(r)) && rust.size === 9;
      return same
        ? `same(${want.size})`
        : `RUST=[${[...rust].sort()}] YIELDED=[${[...yielded].sort()}] FRONT=[${[...front].sort()}]`;
    },
    'same(7)',
  ],
  [
    'SS25',
    '前端 `COMPACT_SKIP_KEYS` 恰好 7 条（= 后端取值域 9 枚减去手动让开的那两条）。' +
      '与 [`SS24`] 分两档不是为了凑数：SS24 比的是**两侧相等**，它能为"后端多一条而前端没配"红，' +
      '但前端自己多敲一条键时它同样红，红出来的读数长得一样，看不出是谁漂的。这一档只数前端，' +
      '所以"语言包里躺着一句永远显示不出来的文案"这种漂移有它自己的红。' +
      '（两档都得活着：只留 SS24 的话，两侧同时各少一条会读成 `same(6)` 而它红不了——那正是 ' +
      '`rust.size === 9` 那一格在盯的形状。）',
    (m) =>
      String(
        new Set(
          m.frontSrc
            .flatMap((s) => [...s.matchAll(/^ {2}"?([a-z-]+)"?: "notice\.compact/gm)])
            .map((x) => x[1]),
        ).size,
      ),
    '7',
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
  // SS11 的期望是"0 次"——面瞎了（目录改名 / walkExt 路径写错 / 忘了 .map(read)）
  // 会把"什么都没扫到"读成"一个都没出现"= 恒绿。两条下限各挡一种瞎法：
  // 第一条挡面为空，第二条挡"面里连被换掉的那个函数本体都没有"（= 扫错了树）。
  ['宿主记忆面非空', (m) => (m.hostMemoryTree.length > 4000 ? 1 : 0), 1],
  ['宿主记忆面含列举实现', (m) => countOf(m.hostMemoryTree, /fn md_rows/), 1],
  // SS12 的扫描面是**两份**文件，而它的两档读数恰好都住在其中一份（`memory_files.rs`）：
  // 另一份改名/删掉时 0|2 照样绿。所以按文件各点一个本体函数，缺哪个红哪个。
  ['对账面含 process_file 本体', (m) => countOf(m.reconcileTree, /fn process_file/), 1],
  ['对账面含启动导出本体', (m) => countOf(m.reconcileTree, /fn run_initial_export/), 1],
  // SS13 期望的是三个 0——面如果因为路径写错而变空，"什么都没扫到"会被读成"一个都没有"。
  // 两条下限各挡一种瞎法：面为空、以及"扫错了树（连工具入口本体都不在）"。
  ['插件记忆面非空', (m) => (m.plugMemory.length > 2000 ? 1 : 0), 1],
  ['插件记忆面含工具入口本体', (m) => countOf(m.plugMemory, /fn memory_write/), 1],
  // SS14 的三档读数全是**文件名集合**：面瞎了（扫描根改名 / walkExt 写错 / tests 过滤器
  // 过宽）会得到 `none`，其中 SS14b 恰好期望 `none` ⇒ 那一档会"绿着瞎掉"。两条下限把面
  // 自己钉住：文件数（现值 200，取七成）+ 白名单那两处 INSERT 本体必须真在面上。
  ['宿主生产文件数（排 tests）', (m) => m.hostDesktop.length, 140],
  ['宿主生产面含影子维护本体', (m) => m.hostDesktop.filter((f) => MEM_INSERT.test(f.code)).length, 2],
  // SS15 两档同样是文件集合，SS15b 期望 `none` ⇒ "面瞎了"与"真的没有"在读数上一模一样。
  // 第一条下限挡"整个面塌掉"，第二条挡"把 plugins-src 一并排掉"那种改法（白名单点正在那儿）。
  ['硬删面文件数（全生产排 memlayout）', (m) => m.hardDelete.length, 150],
  ['硬删面含工具删除本体', (m) => m.hardDelete.filter((f) => /fn memory_delete/.test(f.code)).length, 1],
  // SS17 三档同样是文件集合，其中 SS17b 的右档期望 `none` ⇒ 靠下限把面钉住：
  // spill 面（现值 1 个主文件 + 将来的子树）剥注释后不会短，落盘本体必须在面里。
  ['spill 面含落盘本体', (m) => countOf(m.spill, /fn put\s*\(/), 1],
  ['style.css 含基座定义', (m) => countOf(m.css, /--lw-ui-font-size:\s*\d+px/), 1],
  // 现值 11（tests_permission + tests_approval 目录 10 个 .rs），下限取八成
  ['审批测试面文件数', (m) => m.apprTests.length, 8],
  // 现值 12（settings/ 树里的 .rs，含 memory/ 与 llm/ 子目录），下限取八成
  ['设置分区面文件数', (m) => m.settingsParts.length, 9],
  ['前端设置 store 含 applyInstant 定义', (m) => countOf(m.settingsStore, /async applyInstant\(/), 1],
  // 现值 10（内置插件目录数），下限取八成：plugins-src 整个改名或删掉时，SS7 的
  // "用例数 = 目录数"会变成 0 = 0 而照样绿——这种"两边一起消失"只能由下限兜。
  ['内置插件数', (m) => m.plugins, 8],
  // 对账用例的扫描面本身也要有下限：十份用例散在 10 个 src/ 树里，文件数掉一半 = 面瞎了
  ['插件 src 面文件数', (m) => m.pluginTests.length, 12],
  // 现值 7（六个内核环 + 宿主思考语言环）。SS8 的期望分母是"实现块数"，环整体被搬出
  // 扫描面时分子分母一起掉 ⇒ 只能由这条下限兜。下限取 6：留一点重构余量（把一个环
  // 拆成两个文件只会更多，不会更少）。
  ['注入环 impl 块数', (m) => ringImpls(m), 6],
  // 现值 99（components 树的 .vue）。SS9 的期望是"左=右"，扫描面整个塌掉时 0|0 照样绿
  // ——那种"一起消失"只能由下限兜（与 SS7 那条同一条理由）。
  ['前端组件 .vue 数', (m) => m.comps.length, 70],
  // 现值 6（挂了 document/window 级外点的浮层组件）。下限取一半多一档：少于这个数就说明
  // 扫描面或形状口径漂了，此时 SS9 的"左=右"没有意义。
  ['浮层外点站点数', (m) => dismissCount(m), 4],
  // 现值 178（desktop/src 的 .ts+.vue）：SS9b/SS9c 的分子分母都从这份面里数
  ['前端 src 扫描面文件数', (m) => m.frontSrc.length, 120],
  // S2 刀① 的两块地（SS16 的扫描面）。中立尺面期望的是"1 处定义"，organize 面期望的是
  // 两个 0——后者在"面瞎了"（目录改名 / walkExt 写错）与"真的没有"之间读数完全相同，
  // 所以两条下限各挡一种瞎法：面塌掉、以及"扫错了树（连本体都不在面里）"。
  // ⚠ 中立尺这条下限取的是**字节数现值 919 的四成**而不是点名定义本体：本模块的注释比
  // 代码长（整条理由就是"为什么要封顶、为什么预留是这两颗数"），剥掉注释只剩 919 字节，
  // 阈值取 1200 会天天假红；而拿 `fn anchor(` 当下限又与 SS16a 的右档同分子分母
  // （定义改名时报的是"闸瞎了"，说的却是假话）。拆树只会让面变大，所以字节数够用。
  ['中立尺面字节数（剥注释）', (m) => m.budget.length, 400],
  ['organize 面含轮次收束本体', (m) => countOf(m.organize, /fn after_turn/), 1],
  // S2 刀③（SS18b）期望三个 0 ⇒ 面塌掉（模块改名 / moduleFace 路径写错）与"真的没有"
  // 读数相同。两条下限各挡一种瞎法：面字节数现值 9326 取三成半；再加一条点名分组本体
  // ——它不是任何一档读数的分子，所以不会与 SS18b 同生同灭（"扫错了树"那种瞎法兜得住）。
  ['微压缩面字节数（剥注释）', (m) => m.micro.length, 3500],
  ['微压缩面含分组本体', (m) => countOf(m.micro, /fn candidate_groups/), 1],
  // S2 刀④（SS20/SS21）同一条瞎法：SS21b 期望两个 0、SS21a 期望 1|1，面塌掉时
  // "0|0"与"没扫到"读数相同。下限取现值 16288 的三成；再加一条点名判定本体
  // `fn plan(`——它不是这一簇任何一档读数的分子（SS20b 数 include、SS21a 数
  // STRATEGY、SS21c 数构造点与调用点），所以"扫错了树"这种瞎法兜得住。
  ['主层压缩面字节数（剥注释）', (m) => m.compact.length, 4800],
  ['主层压缩面含判定本体', (m) => countOf(m.compact, /fn plan\s*\(/), 1],
  // S2 刀⑤（SS22b/c）的期望是**文件名**与 1 ⇒ 面塌掉时 `none` 与"只有一个文件"、
  // 0 与"没有第二处"读数相同。同一条瞎法两条下限：字节数现值 5429 取三成；
  // 再加一条点名 `fn process_runs`——分段本体不被 SS22 任何一档当分子，
  // 所以"扫错了树 / 模块改名"这种瞎法只有它能报出来。
  ['滑窗保护网面字节数（剥注释）', (m) => m.slide.length, 1600],
  ['滑窗保护网面含分段本体', (m) => countOf(m.slide, /fn process_runs/), 1],
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
    // 索引预算三档各一条窄变异（S1 刀①②a）：SS10 数"全仓是不是只有一份数字"，
    // SS10b 数"那一份在不在 memlayout 子树"，SS10c 数"宿主是不是真的再导出"。
    // 三条互不遮挡靠的是同一机制：m.rust / m.memlayoutTree / m.memIndex 是三次独立读盘，
    // 改一份不动另一份——所以每条变异只该红自己那一项（刀②a 前 SS10b/SS10c 还是同一项，
    // 那时"宿主退回自定义"会同时打掉左右两档，分工证不成立，故拆）。
    SS10: (m) =>
      m.rust.push(
        'pub(crate) const MAX_INDEX_LINES: usize = 200;\npub(crate) const MAX_INDEX_CHARS: usize = 25_000;',
      ),
    SS10b: (m) => (m.memlayoutTree = m.memlayoutTree.replace('pub const MAX_INDEX_LINES: usize = 200;', '')),
    SS10c: (m) =>
      (m.memIndex = m.memIndex.replace(
        'pub(crate) use plugin_api::memlayout::{MAX_INDEX_CHARS, MAX_INDEX_LINES};',
        'pub(crate) const MAX_INDEX_LINES: usize = 200;',
      )),
    // 刀②c-1 的墓碑变异：把"没接线就回库里查"那一档塞回宿主记忆面。
    // 这条变异**必须存在**的理由写在 SS11 的说明里：旧用例 `files_switch_off_keeps_pure_sqlite`
    // 测的正是被本刀取消的行为，删掉它之后没有任何行为测试还能构造"未接线的 store"
    // （句柄由数据根算出 ⇒ 那个状态在类型上就不存在）⇒ 挡复活只剩结构判据这一层，
    // 而结构判据不配变异就等于"抄了一个当前恰好为 0 的数字"。
    SS11: (m) =>
      (m.hostMemoryTree +=
        '\nif self.files_face().is_none() { return self.index_rows(&ws, 200); }\n'),
    // 刀②c-2 的墓碑变异。它**故意长成一条没人调用的分支**：SS12 与那两条行为测试的分工
    // 就靠这个形状证明——行为测试对"新增一条未接线的反写函数"是全绿的（它不在任何调用
    // 路径上），只有结构档看得见。若把这条变异写成"把 healed 支塞回 process_file"，
    // 它同时会打掉行为测试，证不出 SS12 独有哪一档。
    SS12: (m) =>
      (m.reconcileTree +=
        '\nfn resurrect_from_shadow(r: &MemoryRow, files: &MemoryFiles) {\n    let _ = files.write_fact(&r.workspace, &r.key, &r.value, &r.tags, &r.source, r.updated_at);\n}\n'),
    // 刀③b 的墓碑变异：把"先写库、库才是真源"那一步塞回插件。它长成一段没人调用的函数
    // 是有意的（同 SS12 那一档的分工）——真正的红证是"既有 31 条接口用例对它全绿"，
    // 那句话只有结构档说得出。三档里命中两档就够报红，读数 1|1|0 ≠ 0|0|0。
    SS13: (m) =>
      (m.plugMemory +=
        '\nfn legacy_upsert(conn: &rusqlite::Connection, ws: &str, key: &str) {\n    let _ = conn.execute("INSERT INTO memories (workspace, key) VALUES (?1, ?2)", []);\n}\n'),
    // 刀③b-2 的三块墓碑（同一族形状，各自要能为自己的变异报红）。这里的 push 是
    // **自证**（判据不为空转），真正的红证是磁盘变异 CASE 40（真往 crud.rs 塞一条
    // 没人调用的影子写）——内存 push 与磁盘变异证的是不同的东西，前者证"读数会动"，
    // 后者证"读数盯的是真实代码形状，且行为面看不见它"。
    SS14a: (m) =>
      m.hostDesktop.push({
        rel: 'memory/crud.rs',
        code: 'let _ = conn.execute("INSERT INTO memories (workspace, key) VALUES (?1, ?2)", []);',
      }),
    SS14b: (m) =>
      m.hostDesktop.push({
        rel: 'memory/crud.rs',
        code: 'let _ = conn.execute("UPDATE memories SET value = ?3 WHERE key = ?2", []);',
      }),
    SS14c: (m) =>
      m.hostDesktop.push({
        rel: 'memory/crud.rs',
        code: 'let _ = conn.execute("DELETE FROM memories WHERE key = ?1", []);',
      }),
    // 刀④a 的两块墓碑。**故意用 `soft_` 反例形状**（一条没人调用的硬删）自证那个
    // lookbehind 在承重：写成 `soft_delete_fact(` 的话这两档必须不红，红了就是判据错。
    SS15a: (m) =>
      m.hardDelete.push({
        rel: 'desktop/src-tauri/src/memory/crud.rs',
        code: 'fn wipe_now(root: &std::path::Path, ws: &str, key: &str) {\n    let _ = memlayout::delete_fact(root, ws, key);\n}',
      }),
    SS15b: (m) =>
      m.hardDelete.push({
        rel: 'desktop/src-tauri/src/memory/crud.rs',
        code: 'fn wipe_all(root: &std::path::Path) {\n    let _ = memlayout::clear_facts(root);\n}',
      }),
    // S2 刀① 的三档变异。**互不遮挡是这组对照的全部要点**：SS16a 只加定义（organize 面
    // 不动 ⇒ SS16b 不红）、SS16b 只让 organize 自己再拿一次尺（不含 `fn anchor(` ⇒ SS16a
    // 不红）、SS16c 只再写一颗预留常量。三条都长成"没人调用的分支"是有意的（同
    // SS12/SS13 的分工）：cargo 对它们全绿，只有形状档看得见——而这正是刀① 之后
    // 唯一还剩的证明面（搬家本身没有新行为，行为测试无靶子）。
    SS16a: (m) =>
      m.rust.push(
        'impl OrganizerTwin {\n    fn anchor(session: &Session) -> (u64, u64) { (0, 0) }\n}\n',
      ),
    SS16b: (m) =>
      (m.organize +=
        '\nlet real = crate::runner::reported_input_tokens(session).unwrap_or(0);\n'),
    SS16c: (m) => m.rust.push('\npub const OUTPUT_RESERVE_CAP_TOKENS: u64 = 21_000;\n'),
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
    // 刀③ 三档各一条窄变异（每条只该红自己那一档，分工才证得出来）。
    // SS18a：别处再抄一颗参数值。挑 `MIN_TOKEN_SAVINGS` 是因为它的**数值**在场时
    // 只有这一档在数——推一份进全生产面就把左档从 1 顶成 2。
    SS18a: (m) => m.rust.push('\npub(crate) const MIN_TOKEN_SAVINGS: u64 = 256;\n'),
    // SS18b：微压缩那一环自己去拿尺（多一条窗口换算通路）。这一档只数 micro 面里的
    // `effective_threshold(` 等三个形状，SS16a/SS16c 数的是定义处 ⇒ 互不遮挡。
    SS18b: (m) =>
      (m.micro +=
        '\nfn ruler(slider: u64, w: Option<u64>) -> u64 { effective_threshold(slider, w, None) }\n'),
    // SS18c：环被接第二遍（同一批结果清两次、`ContextCompacted` 落两条）。
    // 左档数的是**哪些文件**在构造它，多一个文件就红，与"总数没变"无关。
    SS18c: (m) =>
      m.hostDesktop.push({
        rel: 'state/llm_stack.rs',
        code: 'let micro_ring = Arc::new(MicroCompact::new(s, d, f));\n',
      }),
    // 刀④a 三档各一条窄变异。SS19a：把定义抄回 organize（退役那天就会拔断压缩）。
    SS19a: (m) =>
      m.hostDesktop.push({
        rel: 'organize/client.rs',
        code:
          'pub(crate) struct AppLlmMemoryManager2 {}\nimpl memory_tree::MemoryManagerClient for AppLlmMemoryManager2 {}\n',
      }),
    // SS19b：organize 面上复活"所有权"的三种痕迹（再导出 + 自己的定义 + 自己造一个）。
    // 数的是 m.organize 那份独立读盘 ⇒ 不会与 SS19a（hostDesktop 面）互相遮挡。
    SS19b: (m) =>
      (m.organize +=
        '\npub use client::AppLlmMemoryManager;\npub struct AppLlmMemoryManager {}\nlet own = AppLlmMemoryManager::new(s);\n'),
    // SS19b2：宿主面多一个构造方（不进 organize 面）⇒ 只有落点集合会动。
    SS19b2: (m) =>
      m.hostDesktop.push({ rel: 'commands/organize.rs', code: 'AppLlmMemoryManager::new(s)\n' }),
    // SS19c：为新调用方又写一遍档位路由。
    SS19c: (m) =>
      m.hostDesktop.push({
        rel: 'compact.rs',
        code: 'fn cfg_for(&self, m: Option<&str>) -> LlmSettings { todo!() }\n',
      }),
    // ---------- S2 刀④ 五档各一条窄变异（每条只该红自己那一项）----------
    // SS20a：宿主侧再敲一遍摘要消息的标记文本（编译器与行为测试都看不出）。
    SS20a: (m) =>
      m.rust.push(
        'pub(crate) const SNEAKY_SUMMARY: &str = "[earlier conversation summarized]\\n{x}";\n',
      ),
    // SS20b：把某份稿子再 include 一次 = 出现第二份装配表。
    SS20b: (m) =>
      (m.compact += '\nconst TEMPLATE_AGAIN: &str = include_str!("../prompts/compact/template.md");\n'),
    // SS21a：主层的 strategy 名字出现第二处字面量定义（同一层两个名字）。
    SS21a: (m) => (m.compact += '\npub(crate) const STRATEGY: &str = "compact";\n'),
    // SS21b：压缩层自己拿尺再算一遍有效阈值。
    SS21b: (m) =>
      (m.compact += '\nlet line = effective_threshold(settings.organize_threshold_tokens(), window, max_out);\n'),
    // SS21c：多一个构造点 + 多一个调用点（两张熔断表 / 一轮压两遍）。
    SS21c: (m) =>
      m.hostDesktop.push({
        rel: 'commands/organize.rs',
        code: 'let extra = Compactor::new(settings, client);\n    let out = cp.run(&mut session, &id, &policy).await;\n',
      }),
    // S2 刀⑤（SS22a/b/c）三条窄变异，各打红一档且**不许连带**：熔断线在第二处再比
    // 一遍 / 组装点之外多 new 一枚表 / 保护网开始自己记失败（它没有解锁通路，记一次
    // 就永久锁死自己）。三条都长在编译器与行为测试看不见的位置上。
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
        rel: 'slide.rs',
        code: 'self.failures.record(sid);\n',
      }),
    // S2 刀⑥（SS23a/b）两条窄变异：旧常量名回到微压缩面 / 保护网多读一次设置。
    SS23a: (m) => (m.micro += '\npub(crate) const KEEP_RECENT_GROUPS: usize = 5;\n'),
    SS23b: (m) =>
      m.hostDesktop.push({
        rel: 'commands/chat.rs',
        code: 'let again = settings.context_slide_keep_runs();\n',
      }),
    // S2 刀⑦（SS24 / SS25）两条窄变异：后端多一条没人配文案的判据 / 前端多一句显示不出来的文案。
    // 分两档各埋一侧，是因为相等型读数一旦两侧同时漂就看不出是谁动的（SS25 说明里那条）。
    SS24: (m) =>
      (m.compact += '\nfn _drift() { Plan::stop("brand-new-reason"); }\n'),
    SS25: (m) =>
      m.frontSrc.push('const keys = {\n  "brand-new-reason": "notice.compactBrandNew",\n};\n'),
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
      // SS14a/b/c 同样 push 面元素 ⇒ 同一份拷贝纪律
      hostDesktop: [...snap.hostDesktop],
      hardDelete: [...snap.hardDelete],
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
