#!/usr/bin/env bash
# 单文件行数上限检查（AGENTS.md「模块与文件规模纪律」）。
# pre-commit 与 CI 共用；超限即失败，提示按职责拆子模块。
set -u

LIMIT=400
fail=0
while IFS= read -r f; do
  lines=$(wc -l < "$f")
  if [ "$lines" -gt "$LIMIT" ]; then
    echo "TOO LONG: $f ($lines lines > $LIMIT) — 按职责拆成子模块后再提交"
    fail=1
  fi
done < <(find crates desktop -name '*.rs' \
  -not -path '*/target/*' -not -path '*/node_modules/*' 2>/dev/null)

if [ "$fail" -eq 0 ]; then
  echo "check_length: OK (limit $LIMIT)"
fi
exit $fail
