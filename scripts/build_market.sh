#!/usr/bin/env bash
# 构建并暂存市场插件包（DESIGN.md §11.3 安装态 / §13.3 市场目录）。
#
# tool-shout 的 manifest 版本来自编译期环境变量 LOCOWRK_PLUGIN_VERSION，
# 按 1.0.0 / 1.1.0 各构建一次，manifest.json 由 **plugins-src/tool-shout/manifest.json**
# （展示元数据唯一真源）sed 版本号落成，连同工件一起暂存到
# market/tool-shout/{ver}/——这就是桌面端"发现"页看到的市场目录（本地
# 目录市场，真实市场服务出现前的替身）。二进制不入库（.gitignore）。
#
# plugin-service 的集成测试与桌面端市场 UI 都依赖这里的暂存布局。
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

id="tool-shout"

find_artifact() {
  for cand in target/debug/tool_shout.dll target/debug/libtool_shout.so target/debug/libtool_shout.dylib; do
    if [[ -f "$cand" ]]; then
      printf '%s' "$cand"
      return 0
    fi
  done
  return 1
}

stage() {
  local ver="$1"
  echo "==> build $id $ver"
  LOCOWRK_PLUGIN_VERSION="$ver" cargo build -p "$id" --quiet

  local src
  if ! src="$(find_artifact)"; then
    echo "error: plugin artifact not found under target/debug/" >&2
    exit 1
  fi

  local dst="market/$id/$ver"
  mkdir -p "$dst"
  cp -f "$src" "$dst/"
  # packaging.md v2 全量 manifest：**真源 = plugins-src/tool-shout/manifest.json**，
  # 这里只 sed 版本号——与 build_plugins.sh 的 plugins/ 侧同一条通路（审计报告 F13：
  # 此前市场 manifest 整份硬编码在本脚本里，是同一元数据的第二条通路，改一处必脱节；
  # tool-shout 也因此一度是十个内置 id 里唯一没有源 manifest.json 的那个）。
  sed "0,/\"version\": \"[^\"]*\"/s//\"version\": \"$ver\"/" \
    "plugins-src/$id/manifest.json" > "$dst/manifest.json"
  mkdir -p "$dst/assets"
  cp -f "plugins-src/$id/assets/icon.svg" "$dst/assets/icon.svg"
  echo "    staged -> $dst/"
}

stage 1.0.0
stage 1.1.0

echo "done. market/ 内容（二进制不入库，勿提交 dll）:"
find market -type f | sort
