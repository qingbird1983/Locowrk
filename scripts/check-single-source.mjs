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
];

/** 闸自己不能瞎：扫描面下限，读数不够就判红而不是"零偏差"。下限取现值的七成 ——
 *  卡在现值边上（第一版写 300 而实测 299）会让闸日常红，那种红只会教人绕过它。 */
const FLOORS = [
  ['生产 .rs 文件数', (m) => m.rust.length, 200],
  ['快照里 memlayout 非空', (m) => (m.memlayout.length > 200 ? 1 : 0), 1],
  // 刀②a 起 memlayout 是目录式模块：子树面为空 = 目录改名或 walkExt 路径写错，
  // 那时 SS10b 的"读不到定义"会被读成"定义丢了"——先让下限把面本身的问题报出来。
  ['memlayout 子树面非空', (m) => (m.memlayoutTree.length > 2000 ? 1 : 0), 1],
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
    };
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
