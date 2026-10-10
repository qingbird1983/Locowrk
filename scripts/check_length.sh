#!/usr/bin/env bash
# 单文件行数门禁（AGENTS.md「模块与文件规模纪律」，2026-10-05 用户裁定
# 400 行 = 建议值，超限按「三问」自检：只做一件事？名字准确？改动不牵连？
# 三问皆是可以不拆，任一否就该拆）。
#
# 2026-10-10 审计报告 H1/B2 改写：旧版是**纯报告 + 恒 exit 0**，而 CI 步骤名
# 写「File length limit (400)」、pre-commit 写 `|| exit 1`——两处都指望它
# 拦截，它却永不失败 = 空转。现改为**基线比较**：
#   · 超限文件数 ≤ 基线 → 放行（拆分与否仍由人按三问判断，脚本不替人判）；
#   · 超过基线 → 判红，列出清单。即「不许新增超限文件」这条纪律有机器兜底，
#     而「必须拆到 400 以下」这种机械要求依然不存在。
#   · 真把某个文件拆净了 → 顺手把 BASELINE 调小，让闸门收紧。
#
# 覆盖面同时扩了两处（旧版只扫 crates + desktop 的 *.rs|*.vue）：
#   plugins-src/（审计报告 M-4 缺口）与 desktop/src/**/*.ts（同类盲区：
#   locales 1157/1139、stores/chat.ts 1064、types.ts 1035、lib/api.ts 764、
#   lib/tool-detail.ts 622 长期无人看见）。基线随新口径一次锁死 = 40。
set -u

SUGGEST=400
BASELINE=40

# 一次 `wc -l` 批量计数（逐文件 fork wc 在这台 Windows 机器上要把整闸拖到
# 十几秒，pre-commit 不可接受）。
over_list="$(find crates desktop plugins-src \( -name '*.rs' -o -name '*.vue' -o -name '*.ts' \) \
  -not -path '*/target/*' -not -path '*/node_modules/*' -not -path '*/dist/*' -print0 2>/dev/null \
  | xargs -0 -n 200 wc -l \
  | awk -v s="$SUGGEST" '$1 > s && $2 != "total" { print $1, $2 }' | sort -rn)"

count=0
if [ -n "$over_list" ]; then
  count=$(printf '%s\n' "$over_list" | wc -l)
fi

if [ "$count" -gt "$BASELINE" ]; then
  echo "check_length: 超限文件 $count > 基线 $BASELINE（建议值 $SUGGEST 行）"
  printf '%s\n' "$over_list" | sed 's/^/  OVER /'
  echo "→ 按「三问」自检：该拆的拆；确属高内聚不拆的，在提交说明里给理由，"
  echo "  并只在**净增**不可避免时把本脚本 BASELINE 显式改大（改基线 = 一次决策）。"
  exit 1
fi

if [ "$count" -lt "$BASELINE" ]; then
  echo "check_length: OK（超限 $count < 基线 $BASELINE）——顺手把 BASELINE 调到 $count 收紧闸门"
else
  echo "check_length: OK（超限 $count == 基线 $BASELINE，零新增）"
fi
exit 0
