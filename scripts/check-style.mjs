#!/usr/bin/env node
/**
 * 前端统一标准机械门禁——模板维度（《Front-end Design Guidelines.md》v2.1 §六）。
 * CSS 声明维度由 stylelint 负责（desktop/stylelint.config.cjs）；本脚本只查
 * stylelint 看不到的 template class/内联样式：
 *   一级判错 = tailwind 内建圆角类（rounded-sm/md/lg/xl/2xl/3xl、rounded-[..px]、裸 rounded）
 *   —— 必须写 rounded-[var(--lw-r-*)]；rounded-full 与 --lw-r-full 同值，放行。
 *   任意 px 字号 text-[..px] 同为一级判错（v2.87 起 --lw-fs-* 令牌已建齐、
 *   全站迁移完成——标准 §二⑫/§五；特例走上一行 lw-waive 注释凭证）。
 *
 * 用法：node scripts/check-style.mjs            全量报告（不拦截）
 *       node scripts/check-style.mjs --staged   暂存 .vue 判错 = 退出 1（pre-commit）
 */
import { execSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const staged = process.argv.includes("--staged");

// [名称, 正则, 级别]：error = 一级判错；warn = 二级报告（收敛清单）。
// 分级机架说明（审计报告 M-9）：**当前四条规则全为 error**，因此下面的
// warns 分支暂无数据——不是死代码待删，而是「先二级观察、迁完翻判错」这条
// 流程的固定工位（任意 px 字号就走完过这一轮：warn → v2.87 全量迁移 →
// 翻 error）。新规则若要观察期，写 "warn" 即走该分支。
const RULES = [
  ["tailwind内建圆角", /\brounded-(?:sm|md|lg|xl|2xl|3xl)\b/g, "error"],
  ["tailwind任意px圆角", /\brounded-\[\d+m?px\]/g, "error"],
  ["tailwind裸rounded", /(^|[\s"'`])rounded(?=[\s"'`])/g, "error"],
  ["tailwind任意px字号", /\btext-\[\d+(?:\.\d+)?px\]/g, "error"],
];

function listFiles() {
  if (staged) {
    const out = execSync("git diff --cached --name-only --diff-filter=ACM", { cwd: root, encoding: "utf8" });
    return out.split("\n").filter((f) => f.endsWith(".vue")).map((f) => join(root, f));
  }
  return execSync('git ls-files "desktop/src/**/*.vue"', { cwd: root, encoding: "utf8" })
    .split("\n")
    .filter(Boolean)
    .map((f) => join(root, f));
}

const files = listFiles();
const hits = [];
for (const file of files) {
  let text;
  try {
    text = readFileSync(file, "utf8");
  } catch {
    continue;
  }
  const lines = text.split("\n");
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    // 豁免 = 本行起向上 3 行内含 lw-waive（容纳模板里唯一的合法注释位：
    // HTML 注释不能写进标签内，跨标签多行时注释离 class 行可隔 1-2 行）
    let waived = false;
    for (let j = Math.max(0, i - 3); j <= i; j++) {
      if (lines[j].includes("lw-waive")) {
        waived = true;
        break;
      }
    }
    if (waived) continue;
    for (const [name, re, level] of RULES) {
      re.lastIndex = 0;
      for (const m of line.matchAll(re)) {
        hits.push({ rel: file.slice(root.length + 1), line: i + 1, name, level, text: line.trim().slice(0, 90) });
      }
    }
  }
}

const scope = staged ? "暂存 .vue（一级判错=提交被拒）" : "全量（收敛工作清单）";
console.log(`== check-style · 模板维度 · ${scope} · ${files.length} 文件 ==`);
const errors = hits.filter((v) => v.level === "error");
const warns = hits.filter((v) => v.level === "warn");
for (const v of errors) console.log(`  ✗ ${v.rel}:${v.line} [${v.name}] ${v.text}`);
for (const v of warns) console.log(`  ⚠ ${v.rel}:${v.line} [${v.name}] ${v.text}`);
if (hits.length) {
  const parts = [];
  if (errors.length) parts.push(`判错 ${errors.length} 处（自发明一律判错——标准 §零.0；改用 rounded-[var(--lw-r-*)]，豁免先登记标准 §七 + /* lw-waive: 理由 */）`);
  if (warns.length) parts.push(`二级报告 ${warns.length} 处（任意 px 字号→text-[length:var(--lw-fs-*)]，标准 §二⑫，±0.5px 吸附；迁移完成后本项翻判错）`);
  console.log(parts.join("；") + "。");
} else {
  console.log("  干净。");
}
if (staged && errors.length) process.exit(1);
