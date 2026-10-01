#!/usr/bin/env bash
# 构建并暂存开发态插件（DESIGN.md §11.3 开发态目录 plugins/{id}/{version}/）。
#
# tool-echo 的 manifest 版本来自编译期环境变量 LOCOWRK_PLUGIN_VERSION，
# 按 1.2.0 / 1.3.0 各构建一次，得到多版本并存所需的双产物。
# tool-fs 1.2.1（守卫裁决消息一致性）/ tool-fetch / tool-shell 单版本暂存。
# 第三批（tool-git / tool-mcp 1.1.0；memory-sqlite 1.2.0 = ⑪ 工具名对齐
# memory_write/memory_read + 记忆工具面 exposure=always）单版本暂存；
# tool-mcp 的 examples/fake_mcp_server 是测试替身，不进暂存目录。
# kernel-bin 的 demo、plugin-host 的集成测试与桌面端开发态装载
# 依赖这里的暂存布局。
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

find_artifact() {
  local id="$1" stem
  stem="${id//-/_}"
  for cand in "target/debug/${stem}.dll" "target/debug/lib${stem}.so" "target/debug/lib${stem}.dylib"; do
    if [[ -f "$cand" ]]; then
      printf '%s' "$cand"
      return 0
    fi
  done
  return 1
}

stage() {
  local id="$1" ver="$2"
  echo "==> build $id $ver"
  LOCOWRK_PLUGIN_VERSION="$ver" cargo build -p "$id" --quiet

  local src
  if ! src="$(find_artifact "$id")"; then
    echo "error: $id artifact not found under target/debug/" >&2
    exit 1
  fi

  local dst="plugins/$id/$ver"
  mkdir -p "$dst"
  cp -f "$src" "$dst/"
  # packaging.md v2：每个版本目录自包含 manifest.json（展示元数据真源，
  # 目录名与 DLL describe 的版本必须一致，registry 加载时校验）+ assets/
  sed "0,/\"version\": \"[^\"]*\"/s//\"version\": \"$ver\"/" \
    "plugins-src/$id/manifest.json" > "$dst/manifest.json"
  if [[ -d "plugins-src/$id/assets" ]]; then
    cp -R "plugins-src/$id/assets" "$dst/assets"
  fi
  echo "    staged -> $dst/$(basename "$src")"
}

stage tool-echo 1.2.0
stage tool-echo 1.3.0
stage tool-fs 1.2.1
stage tool-fetch 1.0.0
stage tool-shell 1.2.0
stage tool-edit 1.1.1
stage tool-git 1.1.0
stage memory-sqlite 1.2.0
stage tool-mcp 1.1.0
stage tool-notify 1.0.0

echo "done: plugins/tool-echo/{1.2.0,1.3.0} plugins/tool-fs/1.2.1 plugins/tool-fetch/1.0.0 plugins/tool-shell/1.2.0 plugins/tool-edit/1.1.1 plugins/{tool-git,tool-mcp}/1.1.0 plugins/memory-sqlite/1.2.0 plugins/tool-notify/1.0.0"
echo "next: cargo run -p kernel-bin -- demo"
