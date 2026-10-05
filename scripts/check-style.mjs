#!/usr/bin/env node
/**
 * 前端统一标准机械门禁——模板维度（《Front-end Design Guidelines.md》v2.0 §六）。
 * CSS 声明维度由 stylelint 负责（desktop/stylelint.config.cjs）；本脚本只查
 * stylelint 看不到的 template class/内联样式：
 *   一级判错 = tailwind 内建圆角类（rounded-sm/md/lg/xl/2xl/3xl、rounded-[..px]、裸 rounded）
 *   —— 必须写 rounded-[var(--lw-r-*)]；rounded-full 与 --lw-r-full 同值，放行。
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

const RULES = [
  ["tailwind内建圆角", /\brounded-(?:sm|md|lg|xl|2xl|3xl)\b/g],
  ["tailwind任意px圆角", /\brounded-\[\d+m?px\]/g],
  ["tailwind裸rounded", /(^|[\s"'`])rounded(?=[\s"'`])/g],
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
    if (line.includes("lw-waive")) continue;
    for (const [name, re] of RULES) {
      re.lastIndex = 0;
      for (const m of line.matchAll(re)) {
        hits.push({ rel: file.slice(root.length + 1), line: i + 1, name, text: line.trim().slice(0, 90) });
      }
    }
  }
}

const scope = staged ? "暂存 .vue（一级判错=提交被拒）" : "全量（收敛工作清单）";
console.log(`== check-style · 模板维度 · ${scope} · ${files.length} 文件 ==`);
if (hits.length) {
  for (const v of hits) console.log(`  ✗ ${v.rel}:${v.line} [${v.name}] ${v.text}`);
  console.log(`判错：${hits.length} 处（自发明一律判错——标准 §零.0）。改用 rounded-[var(--lw-r-*)]；豁免先登记标准 §七 + /* lw-waive: 理由 */。`);
} else {
  console.log("  干净。");
}
if (staged && hits.length) process.exit(1);
