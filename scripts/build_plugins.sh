#!/usr/bin/env bash
# 构建并暂存开发态插件（DESIGN.md §11.3 开发态目录 plugins/{id}/{version}/）。
#
# tool-echo 的 manifest 版本来自编译期环境变量 LOCOWRK_PLUGIN_VERSION，
# 因此按 1.2.0 / 1.3.0 各构建一次，得到多版本并存所需的双产物。
# kernel-bin 的 demo 与 plugin-host 的集成测试依赖这里的暂存布局。
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

find_artifact() {
  for cand in target/debug/tool_echo.dll target/debug/libtool_echo.so target/debug/libtool_echo.dylib; do
    if [[ -f "$cand" ]]; then
      printf '%s' "$cand"
      return 0
    fi
  done
  return 1
}

stage() {
  local ver="$1"
  echo "==> build tool-echo $ver"
  LOCOWRK_PLUGIN_VERSION="$ver" cargo build -p tool-echo --quiet

  local src
  if ! src="$(find_artifact)"; then
    echo "error: plugin artifact not found under target/debug/" >&2
    exit 1
  fi

  local dst="plugins/tool-echo/$ver"
  mkdir -p "$dst"
  cp -f "$src" "$dst/"
  echo "    staged -> $dst/$(basename "$src")"
}

stage 1.2.0
stage 1.3.0

echo "done: plugins/tool-echo/{1.2.0,1.3.0}"
echo "next: cargo run -p kernel-bin -- demo"
