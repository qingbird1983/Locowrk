#!/usr/bin/env bash
# 构建并暂存开发态插件（DESIGN.md §11.3 开发态目录 plugins/{id}/{version}/）。
#
# 版本基准（packaging.md §9.4·补，2026-10-04 用户裁定）：全部插件重置
# 0.1.0 起步，小迭代 +0.0.1，大改版 +0.1.0（经用户确认）。
#
# **F15-A（用户 2026-10-11 裁定 =《AUDIT-FIX-PLAN-v1》§4 Q9，落批 v2.162）：
# 开发态每个 id 只 stage 一个版本。** 此前这里给 tool-fs / tool-shell / tool-edit /
# tool-git / tool-fetch / memory-sqlite 各 stage 了 2–6 个「历史版本」，而 stage()
# 用的永远是**当前 HEAD** 的源码——`LOCOWRK_PLUGIN_VERSION` 只改 DLL 内嵌的版本串、
# 不改码。实测（批 F-5 期间字节级抽查）：`plugins/tool-fs/0.1.5/tool_fs.dll` 与
# `0.1.6/tool_fs.dll` **长度相同、逐字节仅 21 处不同**（正落在版本串上），且改后的
# 新报文**同时存在于两个目录**。那种共存不是历史，是「看起来能回退」的假象：桌面端
# 「回退」入口（`plugin-helpers.ts` 的 `rollbackTargets` 读 registry 的
# `loaded_versions`）因此会指向一份与当前版本行为一致的旧字节。真实的历史字节只存在
# 于**安装态**——installer / 市场复制出去的那一份才是当时的产物。
#
# 例外 = **机制夹具**：`tool-echo` 按 0.1.0 / 0.2.0 各构建一次。它的双版本有真实行为
# 差异，因为 `plugins-src/tool-echo/src/lib.rs` 是**运行时**按内嵌版本串判定的
# （`reverse_supported(v) = v >= REVERSE_SINCE`）⇒ 「加载 0.1.0 → 激活 0.2.0 → 回退
# 0.1.0」确实能观察到 reverse 由可用变报错。所以该叙事证明的是**版本切换机制 + 插件
# 自维的版本闸门**，不是「旧二进制被保存下来了」；普通业务插件不该指望后者。市场侧的
# tool-shout 1.0.0/1.1.0 是同一类夹具（见 scripts/build_market.sh）。
# tool-mcp 的 examples/fake_mcp_server 是测试替身，不进暂存目录。
# kernel-bin 的 demo、plugin-host 的集成测试与桌面端开发态装载
# 依赖这里的暂存布局（集成测试自己往 target/locowrk-test/ 暂存，不吃本脚本产物）。
set -euo pipefail

# 版本真源 = 下面的 PLAN 清单（审计报告 M-12/E7：收尾文案曾手抄版本号，改一次漏一次，
# 最后连「0.1.5 同时指 memory-sqlite 与 tool-shell」这种自相矛盾的话都写出来了；
# 现在文案由清单生成，清单本身也由预校验把守）。
PLAN=(
  # 机制夹具双版本（见头注；demo 与 plugin-host 集成测试的升级/回退叙事都吃这两份）
  "tool-echo 0.1.0"
  "tool-echo 0.2.0"
  # 0.1.6（2026-10-11）：报错教学（审计 F12）——共享基座 plugin_api::fsutil::
  # resolve_existing 的 NotFound 分支补出路「先 fs_list 上级」。同一段文本也编进
  # tool-edit / tool-git 的产物，故那两个 id 同批 +0.0.1（版本 = 产物出处）
  "tool-fs 0.1.6"
  # 0.1.2（2026-10-10）：展示元数据纠正（审计 H8）——manifest 的 fetch.side_effect
  # 仍写 Pure 而代码早在 0.1.1 就改 Irreversible；display.rs 读暂存 manifest 作徽章
  # 真源 ⇒ 插件卡显示「纯」而每次调用都弹审批卡（用户可见的谎报）
  "tool-fetch 0.1.2"
  # 0.1.8（2026-10-11）：F12b 报错教学补最后一支——shell / shell_read 的 cwd 校验
  # 分病因（那是个文件 vs 那里什么都没有）并给两条真出路；此前只宣布
  # "is not a directory"。批 A（0.1.6）与 G4 只读证明（0.1.4）的报文面同批在册
  "tool-shell 0.1.8"
  # 0.1.4（2026-10-11）：本 id 自身零改动，只因共享基座 plugin_api::fsutil 的
  # NotFound 报文随批变化（resolve_existing 用于路径归属校验）⇒ 产物字节不同，随批
  "tool-git 0.1.4"
  # 0.2.2（2026-10-11）：同上随批理由（diff/lines/replace 三处都调 resolve_existing）；
  # 0.2.0 是大改版（edit_replace 容错匹配 + 原子写 + exposure 翻 always，经用户确认）
  "tool-edit 0.2.2"
  # 0.1.5（2026-10-09）：提示词分层批（设计稿 §5.4）——memory_read / memory_search
  # 描述尾补「Use on demand … not as an opening step」双保险句
  "memory-sqlite 0.1.5"
  "tool-mcp 0.1.0"
  "tool-notify 0.1.0"
)

# 允许同一 id 多版本的机制夹具；其余 id 重复出现在清单里直接判错（F15-A 的机器判据：
# 「同码不同串的假历史共存」不可能再被顺手写出来，不靠人眼复查）。
FIXTURE_IDS=(tool-echo)

is_fixture() {
  local x
  for x in "${FIXTURE_IDS[@]}"; do [[ "$x" == "$1" ]] && return 0; done
  return 1
}

declare -A plan_count=()
for entry in "${PLAN[@]}"; do
  id="${entry% *}"
  plan_count["$id"]=$(( ${plan_count["$id"]:-0} + 1 ))
done
for id in "${!plan_count[@]}"; do
  if (( plan_count["$id"] > 1 )) && ! is_fixture "$id"; then
    echo "error: $id 在 PLAN 清单里出现 ${plan_count["$id"]} 次。" >&2
    echo "       开发态每个 id 只 stage 一个版本（F15-A）：本脚本永远用当前 HEAD 重编，" >&2
    echo "       多 stage 只会造出「版本串不同、字节相同」的假历史目录，并把桌面端" >&2
    echo "       「回退」入口指到一份行为如新的产物。真历史在安装态。" >&2
    echo "       确需多版本行为差异 = 做成机制夹具：插件侧按内嵌版本串做运行时判定，" >&2
    echo "       并把该 id 加进 FIXTURE_IDS（先例 = tool-echo）。" >&2
    exit 1
  fi
done

# 暂存记录（收尾文案由它生成）。
STAGED=()

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
    # 幂等：先清旧 assets 再复制。`cp -R src/assets dst/assets` 在目标已
    # 存在时把源目录塞进 dst/assets/assets（第二次运行起每次叠一层），
    # 曾致 10 个插件的暂存图标全部落错路径（审计报告 M-5）。
    rm -rf "$dst/assets"
    cp -R "plugins-src/$id/assets" "$dst/assets"
  fi
  echo "    staged -> $dst/$(basename "$src")"
  STAGED+=("$id $ver")
}

for entry in "${PLAN[@]}"; do
  # shellcheck disable=SC2086  # 清单条目固定为 "id ver" 两段
  stage $entry
done

# 开发态目录收敛（F15-A 的另一半）：删掉清单里不存在的版本目录。
# 本脚本是 plugins/ 的开发态生成器，产物已 gitignore，可整目录重建；不删的话
# 旧的假历史目录会一直躺在暂存面上，registry 扫到就多版本并存。
stale=0
for dir in plugins/*/*/; do
  [[ -d "$dir" ]] || continue
  id="$(basename "$(dirname "${dir%/}")")"
  ver="$(basename "${dir%/}")"
  hit=0
  for entry in "${PLAN[@]}"; do
    [[ "$entry" == "$id $ver" ]] && { hit=1; break; }
  done
  if (( hit == 0 )); then
    echo "==> prune stale $id $ver"
    rm -rf "${dir%/}"
    stale=$(( stale + 1 ))
  fi
done
for dir in plugins/*/; do
  [[ -d "$dir" ]] || continue
  rmdir --ignore-fail-on-non-empty "$dir" 2>/dev/null || true
done

summary="$(printf '%s\n' "${STAGED[@]}" \
  | sort -u \
  | awk '{ if ($1 != prev) { if (prev != "") printf "%s}\n", cur; prev = $1; cur = $1 " {" $2 } else cur = cur "," $2 } END { if (prev != "") printf "%s}\n", cur }')"
echo "done: plugins/ 暂存 ${#STAGED[@]} 份（清单 ${#PLAN[@]} 项，本次清理陈旧 $stale 份）"
printf '%s\n' "$summary" | sed 's/^/  /'
echo "  （版本真源 = 本脚本的 PLAN 清单；每 id 单版本，仅机制夹具 tool-echo 双版本——见头注 F15-A）"
echo "  （tool-sandbox 已随 v2.145 退役不再 staging；安装态的多版本历史由 plugin-service 保留）"
echo "next: cargo run -p kernel-bin -- demo"
