#!/usr/bin/env bash
# 构建并暂存开发态插件（DESIGN.md §11.3 开发态目录 plugins/{id}/{version}/）。
#
# 版本基准（packaging.md §9.4·补，2026-10-04 用户裁定）：全部插件重置
# 0.1.0 起步，小迭代 +0.0.1，大改版 +0.1.0（经用户确认）。tool-echo 的
# manifest 版本来自编译期环境变量 LOCOWRK_PLUGIN_VERSION，按 0.1.0 /
# 0.2.0 各构建一次，得到多版本并存（demo 升级/回退叙事）所需的双产物；
# 其余插件单版本 0.1.0 暂存。
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

stage tool-echo 0.1.0
stage tool-echo 0.2.0
# 0.1.2（2026-10-04，小迭代 +0.0.1）：拒绝文案随 ㉘·C 围栏让路更新——
# 围栏 = 全局声明根（数据根 + 设置 fsRoots，与权限档无关），报错列生效根
# 并指引到设置加根（v2.66 的"围栏跟会话归属"措辞过时）
# 0.1.3（2026-10-06，小迭代 +0.0.1）：fs_read 升 exposure always——
# edit_replace 0.2.0 的「fs_read 重读 → 逐字复制 old_string」闭环配套
# （v2.114，docs/design/edit_replace.md 评估：fs_read 留在 on_demand 时
# 重读闭环可能断在半路）
stage tool-fs 0.1.3
stage tool-fetch 0.1.0
stage tool-shell 0.1.2
stage tool-git 0.1.2
# 0.2.0（2026-10-06，大改版 +0.1.0 经用户确认）：edit_replace 升级——
# 精确零命中后容错匹配（行尾空白 / CRLF / BOM 归一，缩进·大小写·编辑
# 距离不做）+ match_type/byte_range 如实回报 + 多重命中列行号的结构化
# 报错 + 原子写（临时文件+rename）+ exposure 翻 always（编辑首选；
# Timo 实测教训 = on_demand 截断下模型根本用不上它，退而整文件重写）。
# 设计文档 docs/design/edit_replace.md；工具描述与 edit_diff 互指优先级
stage tool-edit 0.2.0
# 0.1.1（2026-10-04，小迭代 +0.0.1）：双层记忆（㉘·A·fix）——memory_write
# +scope="global"（全局偏好层）；读/搜/列 = 项目 ∪ 全局并带层标；未归属
# 会话缺省写拒绝；全局行聊天工具禁删（管理页专属）；headless 语义不变
# 0.1.2（2026-10-05，小迭代 +0.0.1）：F1 记忆文件化——write/delete/undo
# 三落点成功后双写 ~/.Locowrk/memory/ 人类层 md（失败只带 md_warning
# 字段不破契约；总开关 env LOCOWRK_MEMORY_FILES=0 整体跳过）
# 0.1.3（2026-10-05，小迭代 +0.0.1）：F2 同步机械件——memlayout 写路径
# 带 per-file mtime 标记 + 冲突备份（人改 md 未合并时同键写入保留双方）
# + 原子写防撕裂
stage memory-sqlite 0.1.3
stage tool-mcp 0.1.0
stage tool-notify 0.1.0
stage tool-sandbox 0.1.0

echo "done: plugins/tool-echo/{0.1.0,0.2.0} + tool-edit 0.2.0（edit_replace 容错匹配+always）+ tool-fs 0.1.3（fs_read always）+ 其余见上注释"
echo "next: cargo run -p kernel-bin -- demo"
