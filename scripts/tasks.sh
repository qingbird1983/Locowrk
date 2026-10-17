#!/bin/sh
# scripts/tasks.sh —— 任务实现的**唯一位置**（Makefile 只转发到这里）。
# 没装 make 的环境：`sh scripts/tasks.sh <target>`，参数完全等价。
#
# 三条自家规矩（都是这个仓库踩过的坑，写在这里而不是散在文档里）：
#  1. 每步单独跑、单独报结论；**不许把命令接进管道**（`cmd | tail` 的退出码是 tail 的 = 假绿）。
#  2. 未配置的 target 明确报错并列出可用项，不静默通过（恒 rc=0 的门禁等于没有门禁）。
#  3. 报红先修实现，不为了通过而放宽阈值或改基线；要改基线就在提交正文里给出理由。
set -e

cd "$(dirname "$0")/.."
ROOT=$(pwd)

ok()   { printf '\033[32m✓\033[0m %s\n' "$1"; }
say()  { printf '\n\033[1m▶ %s\033[0m\n' "$1"; }
die()  { printf '\033[31m✗\033[0m %s\n' "$1"; exit 1; }

run() { # run <显示名> <命令...>：失败就把名字和退出码一起报出来
  name=$1; shift
  if "$@"; then ok "$name"; else rc=$?; die "$name 失败（rc=$rc）"; fi
}

targets="help setup fmt fmt-check clippy type lint test gates check quick doc adr specs"

help() {
  cat <<'TXT'
可用 target（实现在 scripts/tasks.sh）：
  make setup        装依赖（pnpm install）+ 打印一次性需手动做的事
  make fmt          格式化（写盘）        make fmt-check   只检查
  make clippy       两处 clippy（-D warnings）
  make type         前端类型检查 + 构建   make lint        前端形态（stylelint + check-style）
  make test         根 / 桌面 / 前端三处测试
  make gates        全部结构闸            make doc         文档规则闸
  make check    ★ 提交前门禁：fmt-check + type + lint + gates + clippy + test
  make quick        秒级档（gates + lint），给 pre-commit 用
  make adr          新建一篇决策记录（复制模板）
  make specs        列出 specs/ 现有阶段
TXT
}

t_setup() {
  say "setup"
  run "pnpm install" pnpm -C desktop install
  say "一次性手动项（本脚本不改 git config，也不碰系统设置）"
  echo "  1) hooks：git config core.hooksPath .githooks"
  echo "  2) .env：本项目不入库密钥，模型接入走设置页"
}

t_fmt() {
  say "fmt（写盘）"
  run "cargo fmt --all" cargo fmt --all
  (cd desktop/src-tauri && run "cargo fmt (desktop)" cargo fmt)
}

t_fmt_check() {
  say "fmt-check"
  run "cargo fmt --all --check" cargo fmt --all --check
  (cd desktop/src-tauri && run "cargo fmt --check (desktop)" cargo fmt --check)
}

t_clippy() {
  say "clippy（-D warnings 必须在 -- 之后）"
  run "clippy workspace" cargo clippy --workspace --all-targets -- -D warnings
  (cd desktop/src-tauri && run "clippy desktop" cargo clippy --all-targets -- -D warnings)
}

t_type() {
  say "前端类型 + 构建（generate_context! 也要 dist/）"
  run "pnpm build" pnpm -C desktop run build
}

t_lint() {
  say "前端形态两闸"
  run "stylelint" pnpm -C desktop run lint:style
  # ⚠ 已知问题：check-style.mjs 不带 --staged 时是**报告档**（恒 rc=0），带 --staged 才拦截。
  #   全量档在这里只报不拦——G5 门禁刀会把它拆成两个明确入口，届时删掉这条注释。
  echo "  node scripts/check-style.mjs（全量 = 报告档，不拦截；拦截档见 pre-commit 的 --staged）"
  node scripts/check-style.mjs || true
}

t_test() {
  say "测试三面"
  run "cargo test (workspace)" cargo test --workspace
  (cd desktop/src-tauri && run "cargo test (desktop)" cargo test)
  run "vitest" pnpm -C desktop run test:run
}

t_gates() {
  say "结构闸（逐条单独跑，红在哪条点名哪条）"
  run "长度基线"     bash scripts/check_length.sh
  run "i18n 闭合"    node scripts/check-i18n.mjs
  run "命令三向"     node scripts/check-commands.mjs
  run "中间件接线"   node scripts/check-middleware-wiring.mjs
  run "拆分形状"     node scripts/check-split-equivalence.mjs
  run "预留对账"     node scripts/check-reserved.mjs
  run "文档规则"     node scripts/check-docs.mjs
  run "事件通道"     node scripts/check-channels.mjs
}

t_doc() {
  say "文档规则闸"
  run "文档规则" node scripts/check-docs.mjs
}

t_check() {
  t_fmt_check; t_type; t_lint; t_gates; t_clippy; t_test
  say "check 全绿"
}

t_quick() {
  t_gates; t_lint
  say "quick 全绿"
}

t_adr() {
  n=$(ls docs/adr 2>/dev/null | grep -c '^[0-9]' || true)
  id=$(printf '%04d' "$((10#$n + 1))")
  f="docs/adr/${id}-untitled.md"
  [ -e "$f" ] && die "$f 已存在（编号冲突）"
  cp docs/adr/0000-template.md "$f"
  echo "已建 $f —— 填标题与四节，然后把文件名里的 untitled 换成主题"
}

t_specs() {
  [ -d specs ] && ls -1 specs || echo "specs/ 还没有阶段目录（按需生长，不预留空壳）"
}

target=${1:-help}
case "$target" in
  help) help ;;
  setup) t_setup ;;
  fmt) t_fmt ;;
  fmt-check) t_fmt_check ;;
  clippy) t_clippy ;;
  type) t_type ;;
  lint) t_lint ;;
  test) t_test ;;
  gates) t_gates ;;
  doc) t_doc ;;
  check) t_check ;;
  quick) t_quick ;;
  adr) t_adr ;;
  specs) t_specs ;;
  build|release|deploy|publish) die "「$target」未配置：本项目没有发布通路，需要时先在 spec 里写清流程再实现" ;;
  *) die "未知 target「$target」。可用：$targets" ;;
esac
