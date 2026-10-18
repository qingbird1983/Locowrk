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

targets="help setup fmt fmt-check clippy type lint test gates selftest ledger check quick doc adr specs"

help() {
  cat <<'TXT'
可用 target（实现在 scripts/tasks.sh）：
  make setup        装依赖（pnpm install）+ 打印一次性需手动做的事
  make fmt          格式化（写盘）        make fmt-check   只检查
  make clippy       两处 clippy（-D warnings）
  make type         前端类型检查 + 构建   make lint        前端形态（stylelint + check-style）
  make test         根 / 桌面 / 前端三处测试
  make gates      ★ 全部结构闸（清单只在这里列一次：CI 的 hygiene 作业调的就是它）
  make selftest   闸的自证：给判据施内存变异，它必须报红（打空 = 该项零证明力）
  make doc        文档规则闸
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
  run "check-style（模板维度）" node scripts/check-style.mjs
}

t_test() {
  say "测试三面"
  run "cargo test (workspace)" cargo test --workspace
  (cd desktop/src-tauri && run "cargo test (desktop)" cargo test)
  run "vitest" pnpm -C desktop run test:run
}

t_gates() {
  say "结构闸（逐条单独跑，红在哪条点名哪条）"
  run "长度名单"     bash scripts/check_length.sh
  run "i18n 闭合"    node scripts/check-i18n.mjs
  run "命令三向"     node scripts/check-commands.mjs
  run "中间件接线"   node scripts/check-middleware-wiring.mjs
  run "拆分形状"     node scripts/check-split-equivalence.mjs
  run "预留对账"     node scripts/check-reserved.mjs
  run "文档规则"     node scripts/check-docs.mjs
  run "事件通道"     node scripts/check-channels.mjs
  run "同源对账"     node scripts/check-single-source.mjs
  run "库表两侧对账" node scripts/check-schema-parity.mjs
  run "渲染矩阵"     node scripts/check-render-matrix.mjs
  run "样式落点"     node scripts/check-css-ownership.mjs
}

t_selftest() {
  say "闸的自证（判据必须能为变异报红；打空 = 该项零证明力）"
  run "同源对账自测" node scripts/check-single-source.mjs --selftest
  run "库表对账自测" node scripts/check-schema-parity.mjs --selftest
  run "文档规则自测" node scripts/check-docs.mjs --selftest
  run "渲染矩阵自测" node scripts/check-render-matrix.mjs --selftest
  run "样式落点自测" node scripts/check-css-ownership.mjs --selftest
}

t_doc() {
  say "文档规则闸"
  run "文档规则" node scripts/check-docs.mjs
  run "文档规则自测" node scripts/check-docs.mjs --selftest
}

t_check() {
  t_fmt_check; t_type; t_lint; t_gates; t_selftest; t_clippy; t_test
  say "check 全绿"
}

t_quick() {
  t_gates; t_selftest; t_lint
  say "quick 全绿"
}

t_adr() {
  # 编号 = 现有最大编号 + 1。此前用「文件条数 + 1」，把 0000-template 也数进去，
  # 于是下一篇会跳到 0004 而 0003 永远空着（实测：docs/adr 三份时它报 0004）。
  last=$(ls docs/adr 2>/dev/null | grep -oE '^[0-9]{4}' | sort -n | tail -1)
  id=$(printf '%04d' "$((10#${last:-0000} + 1))")
  f="docs/adr/${id}-untitled.md"
  [ -e "$f" ] && die "$f 已存在（编号冲突）"
  cp docs/adr/0000-template.md "$f"
  echo "已建 $f —— 填标题与四节，然后把文件名里的 untitled 换成主题"
}

t_specs() {
  [ -d specs ] && ls -1 specs || echo "specs/ 还没有阶段目录（按需生长，不预留空壳）"
}

t_ledger() {
  say "拆分登记表重放（一次性历史证明，**不进常驻门禁**）"
  # 它比的是「git 历史里的原文 vs 当前产物」，而产物在拆分之后继续演化是正常开发不是事故。
  # 实测：9 份登记表里 8 份已红（红在「新增代码无登记」）。所以 v2.209 把它从 CI 撤到这里，
  # 只在「重做某一刀」或「把旧拆分在新形状下再验一遍」时手动跑：make ledger LEDGER=<name>
  if [ -z "${LEDGER:-}" ]; then
    ls -1 scripts/split-ledger/*.json 2>/dev/null | sed 's/^/  /'
    die "用法：make ledger LEDGER=settings-llm（不带 LEDGER 只列可用登记表，不静默全跑）"
  fi
  f=scripts/split-ledger/${LEDGER}.json
  [ -f "$f" ] || die "查无登记表 $f"
  echo "  说明：这份登记表证明的是**当年那一刀**（历史原文 vs 当时产物）。"
  echo "  若它红在「新增代码无登记」，那是那一刀之后代码继续演化的正常结果，不代表现在有问题。"
  node scripts/check-split-equivalence.mjs --split --ledger "$f"
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
  selftest) t_selftest ;;
  ledger) t_ledger ;;
  doc) t_doc ;;
  check) t_check ;;
  quick) t_quick ;;
  adr) t_adr ;;
  specs) t_specs ;;
  build|release|deploy|publish) die "「$target」未配置：本项目没有发布通路，需要时先在 spec 里写清流程再实现" ;;
  *) die "未知 target「$target」。可用：$targets" ;;
esac
