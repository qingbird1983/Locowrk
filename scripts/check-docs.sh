#!/usr/bin/env bash
# Documentation gate: every docs page is registered, no personal absolute paths,
# required root entries exist. Run from anywhere; exits non-zero on violation.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fail=0

# 1. No orphan documents: every docs/**/*.md except the map itself is listed in docs/README.md.
while IFS= read -r -d '' file; do
  rel="${file#"$root"/docs/}"
  [ "$rel" = "README.md" ] && continue
  if ! grep -qF "$rel" "$root/docs/README.md"; then
    echo "orphan doc (not registered in docs/README.md): $rel"
    fail=1
  fi
done < <(find "$root/docs" -name '*.md' -print0)

# 2. No personal absolute paths in tracked prose.
if grep -rInE '([A-Za-z]:\\Users\\|/home/[A-Za-z0-9._-]+/|/Users/[A-Za-z0-9._-]+/)' \
  "$root/docs" "$root/.agents" "$root/README.md" "$root/README.zh-CN.md" "$root/AGENTS.md" 2>/dev/null; then
  echo "personal absolute path found (use a repo-relative path or a URL)"
  fail=1
fi

# 3. Required root entries.
for entry in README.md README.zh-CN.md AGENTS.md CHANGELOG.md LICENSE .agents/CONTEXT.md; do
  if [ ! -e "$root/$entry" ]; then
    echo "missing required entry: $entry"
    fail=1
  fi
done

if [ "$fail" -ne 0 ]; then
  exit 1
fi
echo "check-docs: OK"
