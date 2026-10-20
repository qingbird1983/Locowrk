#!/usr/bin/env bash
# 单文件规模闸（点名名单制，v2.209 改）。规则本体见 AGENTS.md「代码硬规则」与
# 《Front-end Design Guidelines》之外的这条：**400 行是建议值，不是硬上限**。
#
# 为什么是名单而不是计数：
#   旧版把「超限文件数 ≤ BASELINE」写在脚本里，BASELINE 是一颗每拆一刀都要手工改小的
#   数字（收紧史 40→22 共 16 次，原文见 docs/history/check-length-baseline.md，已只读）。
#   会漂的数字钉进判据 = 下一批它自己红，而且**红得和代码没关系**——只是忘了改注释。
#   名单制把决策点搬到真正的位置：加一个超限文件 = 必须在名单里加一行；拆净一个 = 删一行。
#   数不写在脚本里，由两边集合现算。
#
# 三问自检（人判，脚本不替人判）：①这个文件是否只做一件事？②名字是否准确概括内容？
#   ③修改它是否经常牵动不相关的部分？三问皆是 = 高内聚，可以留在名单里，理由写进提交正文；
#   任一为否 = 拆成目录式子模块，然后从名单删掉那一行。
#
# 覆盖面：crates / desktop / plugins-src 的 *.rs *.vue *.ts（排掉 target、node_modules、dist）。
# 审计 M-4 与批 B 之前的盲区是 plugins-src 与前端 .ts——只扫 .rs 的旧版让 locales 1157 行
# 与 stores/chat.ts 1064 行长期无人看见。
set -u

SUGGEST=400

# 名单 = 已按三问判定为「高内聚、暂不拆」的文件。一行一个，路径相对仓库根。
# `llm/app_llm.rs` 于 S2 刀② 入名单（394→411 行）：三问皆是——①只管一件事
# （把宿主设置面 + 注入句柄翻译成一次 LLM 请求与它的 caps/钩子）；②名字概括得住
# （AppLlm = 宿主侧 LlmService 适配器）；③改它牵动的都是同一件事（每枚能力都要进
# caps，这正是它的职责而不是无关部分）。刀② 只加了 17 行（一枚注入句柄 + 一个
# builder + caps 那一格），为凑 400 把 builder 面拆出去 = 在本刀里做无关重构，
# 且拆完仍是同一个 struct 的两半。若后续刀再加格，届时按三问重判并真拆。
# `compact/tests.rs`（422）与 `compact/tests_deep.rs`（434）于 S2 刀④b/④c 入名单，
# 两条各按三问判过：①各自只管一件事（前者 = 主层的**纯判据**：切点、目标位、提示词
# 装配与摘要验收；后者 = **接线半**：占用读数、摘要请求、落账、熔断与递减重试）；
# ②名字概括得住（就是 `compact` 那两爿的 tests 子模块，文件名直接点名被测的那一爿）；
# ③改它牵动的全是同一件事（动节名表必动前者、动落账形状必动后者，两者互不牵连——
# 这正是把它们分成两个文件的原因，而不是为了凑行数）。
# 再往下切就是按用例数拆文件：`plan()` 那六条 reason 与 `choose_cut()` 的四种边界本来
# 是同一件事的两侧（判据读者要一起看才知道"不动"的每一档都有人盯），拆开只会让人
# 在两个文件之间找一条红的那条。
# `runner/mod.rs`（414 行）于 S2 刀④c 入名单：三问皆是——①只管一件事（一轮对话的编排：
# 轮界事件、step 循环、步间两个检查、落盘游标、收束路径）；②名字概括得住；③刀④ 加的
# 那 16 行（参数一枚 + 头注一段 + 步间那三行调用）动的正是它本职的那件事。
# ⚠ 躲得过这一格的两条路都被否掉：把 `record_usage` / `finish_turn` 搬去子文件 = 为了
# 凑行数在文件间转字段（AGENTS 那条"不许为过闸搬代码"），把步间注释挪进 `compact.rs`
# = 让 runner 的读者看不见"为什么这一句必须同步等在 step 之前"。真拆的时机是它开始装第二件事
# （例如把工具执行也搬进来），届时按三问重判并真拆。
# S4 刀① 退役微压缩与泄压后，`microcompact*`、`settings/context.rs` 已回到建议线内，名单
# 随之删去——留在名单里等于给已经不超限的文件发豁免（本闸的"不超限请删名单"那行所防的）。
OVERLIST="
desktop/src/components/chat/Composer.vue
desktop/src/components/chat/ToolDetailView.vue
desktop/src/components/chat/ToolRow.vue
desktop/src/components/settings/model/ProviderSetupDialog.vue
desktop/src/components/settings/model/ModelPanes.vue
desktop/src/components/settings/LlmSection.vue
desktop/src-tauri/src/llm/client.rs
desktop/src-tauri/src/llm/app_llm.rs
desktop/src/components/chat/TranscriptFlow.vue
desktop/src-tauri/src/events.rs
desktop/src/components/settings/DeveloperSection.vue
desktop/src/components/memory/TreeSection.vue
desktop/src/components/chat/MessageRow.vue
desktop/src/components/chat/PermissionCard.vue
desktop/tests/chat-projection.spec.ts
desktop/src-tauri/src/compact/tests.rs
desktop/src-tauri/src/compact/tests_deep.rs
desktop/src-tauri/src/runner/mod.rs
"

# **扫描面只有一个出处**：下面这一条 find 的产物既用来数超限、也用来验下限。
# 分两处写就是"两处手写必漂"的同族——本闸第一版就是这样：下限循环另起一条 find，
# 把主 find 的三个根改成一个之后下限照样过，闸瞎了还反过来劝我把名单删空。
# `wc -l` 一次批量传（逐文件 fork 在这台 Windows 机器上要把整闸拖到十几秒，pre-commit 不可接受）。
FILES="$(find crates desktop plugins-src \( -name '*.rs' -o -name '*.vue' -o -name '*.ts' \) \
  -not -path '*/target/*' -not -path '*/node_modules/*' -not -path '*/dist/*' 2>/dev/null)"

scan_fail=0
for pair in "crates 100" "desktop 200" "plugins-src 30"; do
  r=${pair% *}; min=${pair#* }
  n=$(printf '%s\n' "$FILES" | grep -c "^$r/")
  # 一条永不命中的判据和没有判据是同一件事。这里的下限是"闸没瞎"的下限，不是规模要求：
  # 代码只会变多，某个根读数掉到不足一半 = 目录改名或 find 条件被改坏。
  if [ "$n" -lt "$min" ]; then
    echo "check_length: ✗ 扫描根 $r 只有 $n 个源文件（下限 $min）⇒ 判据无扫描面，视为闸失效"
    scan_fail=1
  fi
done
[ "$scan_fail" -eq 1 ] && exit 1

actual="$(printf '%s\n' "$FILES" | xargs -n 200 wc -l 2>/dev/null \
  | awk -v s="$SUGGEST" '$1 > s && $2 != "total" { print $2 }' | sed 's#^\./##' | sort -u)"

listed="$(printf '%s\n' "$OVERLIST" | sed '/^\s*$/d' | sort -u)"
newly="$(comm -23 <(printf '%s\n' "$actual") <(printf '%s\n' "$listed"))"
stale="$(comm -13 <(printf '%s\n' "$actual") <(printf '%s\n' "$listed"))"

fail=0
if [ -n "$newly" ]; then
  echo "check_length: ✗ 名单外新增超限文件（判红）"
  printf '%s\n' "$newly" | while read -r f; do
    printf '  OVER %s  %s 行 > %s\n' "$f" "$(wc -l < "$f")" "$SUGGEST"
  done
  echo "→ 要么拆（目录式子模块），要么按三问判定保留并把理由写进提交正文 + 在本脚本名单加一行。"
  echo "  加名单 = 一次显式决策；不许为通过而删判据或调 SUGGEST。"
  fail=1
fi

if [ -n "$stale" ]; then
  echo "check_length: ~ 名单里有 $(( $(printf '%s\n' "$stale" | grep -c .) )) 个文件现已不超限，删掉那几行收紧闸门："
  printf '%s\n' "$stale" | sed 's/^/    /'
fi

n_over=$(printf '%s\n' "$actual" | grep -c .)
n_list=$(printf '%s\n' "$listed" | grep -c .)
if [ "$fail" -eq 0 ]; then
  echo "check_length: OK（超限 $n_over · 名单 $n_list · 零新增 · 三个扫描根均在下限之上）"
fi
exit $fail
