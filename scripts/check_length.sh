#!/usr/bin/env bash
# 单文件行数检查（AGENTS.md「模块与文件规模纪律」，2026-10-05 用户裁定）：
# 400 行是**建议值**不是硬上限——超限文件按「三问」自检：
#   1) 这个文件是否只做一件事？
#   2) 它的名字是否准确概括内容？
#   3) 修改它时是否经常牵动不相关的部分？
# 三问皆「是」= 高内聚，可以不拆；有任一「否」= 即使没到 400 行也应拆。
# 本脚本退化为**报告**（不拦截提交）：超限文件列在 stdout，拆分与否由
# 提交者按三问判断并在提交说明里给出理由。
set -u

SUGGEST=400
over=0
while IFS= read -r f; do
  lines=$(wc -l < "$f")
  if [ "$lines" -gt "$SUGGEST" ]; then
    echo "OVER SUGGESTION: $f ($lines lines > $SUGGEST) — 按「三问」自检是否拆分（见 AGENTS.md）"
    over=1
  fi
done < <(find crates desktop \( -name '*.rs' -o -name '*.vue' \) \
  -not -path '*/target/*' -not -path '*/node_modules/*' 2>/dev/null)

if [ "$over" -eq 0 ]; then
  echo "check_length: OK (suggestion $SUGGEST)"
fi
# 报告式门禁：恒不拦截（三问判断是人的职责，不是脚本可机械化的）
exit 0
