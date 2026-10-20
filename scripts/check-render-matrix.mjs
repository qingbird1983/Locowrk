#!/usr/bin/env node
/**
 * 对话流身份矩阵完备性闸（spec 06 AC-1；设计稿 §3.2 身份枚举 / §4 相位 / §5.3 矩阵表）。
 *
 * 四项判据，任一不合就失败（输出恒为一行 `KEY=值 ...`，便于肉眼与脚本两头读）：
 *   IDENTITIES   身份表条数，必须 = 28 —— 往表里加一行身份立刻报红
 *                （S4 刀③c 删记忆族 10 个身份后从 38 降到 28；改这个数必须显式改判据）
 *   PHASES       相位表条数，必须 = 5
 *   UNREGISTERED 真实工具名与矩阵对不上的条数，必须 = 0。**双向**查：
 *                ① Rust 侧注册了、矩阵却落到 `fallback`（真话：新加了工具没登记）
 *                ② 矩阵登记了、Rust 侧查无此名（真话：写了不存在的工具名）
 *   MISSING_VERB 身份动词在 `tool.type.*` 里没有键的条数，必须 = 0
 *
 * 真实工具名的权威来源 = Rust 侧 `name: "xxx".into()`：这是 `ToolSpec` 注册的唯一形态，
 * 插件（`plugins-src/<name>/src/lib.rs`）与宿主内置（`desktop/src-tauri/src/**`）都走它。
 * 测试文件里的 mock 名（`"mock"` `"x"` `"a1"`）不算，路径含 `test` 一律跳过。
 * 扫描面承认不到的真名写进矩阵的 `GATE_EXEMPT_TOOLS`（豁免表在矩阵里，闸不藏私货）。
 *
 * 用法：
 *   node scripts/check-render-matrix.mjs               常规判据
 *   node scripts/check-render-matrix.mjs --selftest    内存变异自证：六项判据必须各自报红
 */
import { readFileSync, readdirSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const MATRIX = join(root, "desktop/src/lib/chat/render-matrix.ts");
const LOCALE = join(root, "desktop/src/locales/zh-CN/tools.ts");

const EXPECT_IDENTITIES = 28;
const EXPECT_PHASES = 5;

/** 取一块源码：从锚点起到下一个块尾（默认顶格 `};`）。 */
function block(src, anchor, terminator = "\n};") {
  const i = src.indexOf(anchor);
  if (i < 0) throw new Error(`找不到锚点：${anchor}`);
  const j = src.indexOf(terminator, i + anchor.length);
  if (j < 0) throw new Error(`锚点 ${anchor} 之后找不到块尾 ${terminator}`);
  return src.slice(i, j);
}

/** 判据主体：吃两份源码文本，吐结果对象，**不打印也不退出**（`--selftest` 要喂变异面）。 */
function audit(matrixSrc, localeSrc, rustNames) {
  const unionStart = matrixSrc.indexOf("export type ToolIdentity =");
  const unionBlock = matrixSrc.slice(unionStart, matrixSrc.indexOf(";", unionStart));
  const identities = [...unionBlock.matchAll(/"([a-z_]+)"/g)].map((m) => m[1]);

  const tableBlock = block(matrixSrc, "export const IDENTITY_TABLE");
  const tableKeys = [...tableBlock.matchAll(/^ {2}([a-z_]+): \{$/gm)].map((m) => m[1]);
  const verbs = [...new Set([...tableBlock.matchAll(/verb: "([a-z_]+)"/g)].map((m) => m[1]))];

  const nameBlock = block(matrixSrc, "const BY_NAME");
  const byName = new Map();
  for (const m of nameBlock.matchAll(
    /^ {2}(?:([a-z_][a-z0-9_-]*)|"([^"]+)"): "([a-z_]+)",$/gm,
  )) {
    byName.set(m[1] ?? m[2], m[3]);
  }

  const prefixBlock = block(matrixSrc, "const BY_PREFIX", "\n];");
  const byPrefix = [...prefixBlock.matchAll(/\["([a-z_]+)", "([a-z_]+)"\]/g)].map((m) => [m[1], m[2]]);

  const phaseBlock = block(matrixSrc, "export const PHASES = [", "]");
  const phases = [...phaseBlock.matchAll(/"([a-z_]+)"/g)].map((m) => m[1]);

  const exemptBlock = block(matrixSrc, "export const GATE_EXEMPT_TOOLS");
  const exempt = new Set(
    [...exemptBlock.matchAll(/^ {2}(?:([a-z_][a-z0-9_-]*)|"([^"]+)"):/gm)].map((m) => m[1] ?? m[2]),
  );

  function toolIdentity(tool) {
    const hit = byName.get(tool);
    if (hit) return hit;
    for (const [prefix, identity] of byPrefix) if (tool.startsWith(prefix)) return identity;
    return "fallback";
  }

  const usedExempt = new Set();
  const unregistered = [];
  for (const [name, where] of rustNames) {
    if (toolIdentity(name) !== "fallback") continue;
    if (exempt.has(name)) {
      usedExempt.add(name);
      continue;
    }
    unregistered.push(`Rust 有、矩阵兜底：${name} (${where})`);
  }
  for (const name of byName.keys()) {
    if (rustNames.has(name)) continue;
    if (exempt.has(name)) {
      usedExempt.add(name);
      continue;
    }
    unregistered.push(`矩阵有、Rust 查无：${name}`);
  }
  // 反向校验：豁免是欠账，还清一条就得删一条，否则这张表会变成垃圾桶。
  const staleExempt = [...exempt].filter((n) => !usedExempt.has(n));

  const typeStart = localeSrc.indexOf("type: {");
  const typeInner = localeSrc.slice(typeStart, localeSrc.indexOf("\n  },", typeStart));
  const localeVerbs = new Set(
    [...typeInner.matchAll(/(?:^|\s)([a-z_][a-z0-9_]*):\s*"/g)].map((m) => m[1]),
  );
  const missingVerb = verbs.filter((v) => !localeVerbs.has(v));

  const tableOnly = identities.filter((id) => !tableKeys.includes(id));
  const unionOnly = tableKeys.filter((id) => !identities.includes(id));

  const failures = [];
  if (tableKeys.length !== EXPECT_IDENTITIES) {
    failures.push(`IDENTITIES=${tableKeys.length}，期望 ${EXPECT_IDENTITIES}`);
  }
  if (phases.length !== EXPECT_PHASES) {
    failures.push(`PHASES=${phases.length}，期望 ${EXPECT_PHASES}`);
  }
  if (tableOnly.length || unionOnly.length) {
    failures.push(
      `联合类型与表体不一致：表缺 [${tableOnly.join(", ")}] · 联合缺 [${unionOnly.join(", ")}]`,
    );
  }
  if (unregistered.length) failures.push("UNREGISTERED");
  if (missingVerb.length) failures.push(`MISSING_VERB：${missingVerb.join(", ")}`);
  if (staleExempt.length) failures.push(`过期豁免：${staleExempt.join(", ")}`);

  return {
    counts: {
      IDENTITIES: tableKeys.length,
      PHASES: phases.length,
      UNREGISTERED: unregistered.length,
      MISSING_VERB: missingVerb.length,
    },
    failures,
    unregistered,
    staleExempt,
    missingVerb,
  };
}

/** 扫 Rust 侧真实工具名，与矩阵无关，全会话只扫一次。 */
function scanRustTools() {
  const walkRust = (dir, out = []) => {
    let entries;
    try {
      entries = readdirSync(dir, { withFileTypes: true });
    } catch {
      return out;
    }
    for (const e of entries) {
      const p = join(dir, e.name);
      if (e.isDirectory()) {
        if (["target", "node_modules", "dist"].includes(e.name)) continue;
        walkRust(p, out);
      } else if (e.name.endsWith(".rs")) {
        const rel = relative(root, p).replace(/\\/g, "/");
        if (/(^|\/)tests?(\/|\.|_)/.test(rel)) continue;
        out.push({ path: p, rel });
      }
    }
    return out;
  };

  const names = new Map();
  for (const dir of ["plugins-src", "crates", "desktop/src-tauri/src"]) {
    for (const { path, rel } of walkRust(join(root, dir))) {
      const text = readFileSync(path, "utf8");
      for (const m of text.matchAll(/name: "([a-z_][a-z0-9_-]*)"\.into\(\)/g)) {
        if (!names.has(m[1])) names.set(m[1], rel);
      }
    }
  }
  return names;
}

/** 打印一次判据结果；返回是否通过。`quiet` 供自证时收起逐条明细。 */
function report(result, quiet = false) {
  const c = result.counts;
  console.log(
    `IDENTITIES=${c.IDENTITIES} PHASES=${c.PHASES} ` +
      `UNREGISTERED=${c.UNREGISTERED} MISSING_VERB=${c.MISSING_VERB}`,
  );
  if (!result.failures.length) {
    if (!quiet) console.log(`身份矩阵完备：${c.IDENTITIES} 身份 × ${c.PHASES} 相位，工具名双向对得上。`);
    return true;
  }
  if (quiet) return false;
  console.error("\ncheck-render-matrix 未通过：");
  for (const f of result.failures) console.error("  ✗ " + f);
  for (const u of result.unregistered.slice(0, 20)) console.error("  · " + u);
  return false;
}

/**
 * 变异表：每条把矩阵源码改成"该坏"的样子，并声明它必须让哪项判据报红。
 * 判据打空（变异后仍然全绿）= 该项零证明力，自证失败。
 */
const MUTATIONS = [
  {
    what: "身份表少一行",
    apply: (s) => s.replace(/^ {2}fetch: \{$/m, ""),
    caught: (r) => r.counts.IDENTITIES !== EXPECT_IDENTITIES,
  },
  {
    what: "相位表少一项",
    apply: (s) => s.replace('"unknown"] as const', "] as const"),
    caught: (r) => r.counts.PHASES !== EXPECT_PHASES,
  },
  {
    what: "矩阵登记一个不存在的工具名",
    apply: (s) => s.replace("  fetch: \"fetch\",", '  fetch: "fetch",\n  zzz_ghost_tool: "fallback",'),
    caught: (r) => r.unregistered.some((u) => u.includes("zzz_ghost_tool")),
  },
  {
    what: "身份用了一个没有 i18n 的动词",
    apply: (s) => s.replace('verb: "search"', 'verb: "zzz_ghost_verb"'),
    caught: (r) => r.missingVerb.includes("zzz_ghost_verb"),
  },
  {
    what: "豁免表里塞一条没人用的（欠账还清了没删）",
    apply: (s) => s.replace('  danger: "', '  zzz_ghost_exempt: "",\n  danger: "'),
    caught: (r) => r.staleExempt.includes("zzz_ghost_exempt"),
  },
  {
    what: "联合类型与表体脱节",
    apply: (s) => s.replace('  | "fetch"', ""),
    caught: (r) => r.failures.some((f) => f.includes("联合类型与表体不一致")),
  },
];

const matrixSrc = readFileSync(MATRIX, "utf8");
const localeSrc = readFileSync(LOCALE, "utf8");
const rustNames = scanRustTools();

if (!process.argv.includes("--selftest")) {
  process.exit(report(audit(matrixSrc, localeSrc, rustNames)) ? 0 : 1);
}

// ---- --selftest：内存变异自证，不碰磁盘 ----
let allCaught = true;
for (const { what, apply, caught } of MUTATIONS) {
  const mutated = apply(matrixSrc);
  if (mutated === matrixSrc) {
    console.error(`✗ ${what}：变异没生效（锚点文本改了？），该项零证明力`);
    allCaught = false;
    continue;
  }
  const result = audit(mutated, localeSrc, rustNames);
  const red = report(result, true) === false;
  const ok = red && caught(result);
  console.log(`${ok ? "✓" : "✗"} ${what}${red ? "" : " —— 判据打空，零证明力"}`);
  if (!ok) allCaught = false;
}
console.log(allCaught ? "身份矩阵闸自证通过：六项判据都能为变异报红。" : "身份矩阵闸自证未通过。");
process.exit(allCaught ? 0 : 1);
