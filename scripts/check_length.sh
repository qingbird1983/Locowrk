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
#   lib/tool-detail.ts 622 长期无人看见）。基线随新口径一次锁死 = 40；
#   审计修复批 C（2026-10-11）删掉从未接线的 CtxCompact 及其 405 行测试，
#   超限数自然净降到 39 → 基线随批收紧为 39。
#   审计修复批 F-3（2026-10-11）把 desktop 组装点 init_state（636 行函数 /
#   698 行文件）按子系统拆成 boot/llm_stack/memory_stack/waterfall/kernel_stack/
#   tools/plugins/background 八簇（state/mod.rs 220 行），超限数净降到 38 →
#   基线收紧为 38。（注释原文写「六簇 / 350 行」是批 F-3 当时的形态，v2.156
#   批 F-6 顺手按实测改正：八簇、220 行。）
#   审计修复批 F-6（2026-10-11）把 settings/llm.rs（682 行）拆成 policy/
#   persist/synthesize/ipc/migrate 五簇（mod.rs 38 行，全簇守 400 线内），
#   超限数 38 → 37 ⇒ 基线收紧为 37。
#   审计修复批 F-7（2026-10-11）把 organize/mod.rs（614 行）拆成 events/
#   trigger/collapse/task 四簇 + 本体（mod.rs 229 行），超限数 37 → 36 ⇒
#   基线收紧为 36。
#   审计修复批 F-7（2026-10-11）把 organize/mod.rs（614 行）拆成 events/
#   trigger/collapse/task 四簇 + 本体（mod.rs 229 行），超限数 37 → 36 ⇒
#   基线收紧为 36。
#   审计修复批 F-8（2026-10-11）把 organize/tests.rs（865 行，拆前**全仓最长
#   文件**）拆成 tests/{mod(夹具 172)+pipeline+recovery+overflow+views+tools+
#   summary} 七文件（形状照 settings/tests/ 先例：夹具留父模块、子簇
#   `use super::*`，可见性零改动），超限数 36 → 35 ⇒ 基线收紧为 35。
set -u

SUGGEST=400
BASELINE=35

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
