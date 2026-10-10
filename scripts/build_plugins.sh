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
# 0.1.4（2026-10-06，小迭代 +0.0.1）：发现性补丁（exposure 文档 §4.4，
# 真实模型实测反馈）——fs_grep 升 always（模型的眼睛；实测绕行 shell grep
# 的根因，全库零引用的最孤儿）+ fs_restore 升 always（全部四个改文件工具
# 的唯一撤销路径，意外时刻最不可能探索）+ fs_restore 描述改写（删「usually
# invoked by the compensation layer」劝退措辞，明确可拿原调用三件套手动自救）
stage tool-fs 0.1.4
# 0.1.5（2026-10-09，小迭代 +0.0.1）：G3 Revision Guard（可靠性稿 §3）——
# fs_read 回全量 revision（LF 归一 sha256，与截断窗口解耦）；fs_write
# expected_revision 缺省/null = 新建语义（已存在即拒盲覆盖）、给了必须
# 一致；fs_delete 同守卫；成功输出带新 revision 供链式；fs_restore 输出
# 带恢复后 revision（撤销后守卫不断链）。共享基座进 plugin_api::fsutil
stage tool-fs 0.1.5
# 0.1.1（2026-10-06，小迭代 +0.0.1）：fetch 副作用分级纠正（真实测试复测
# P0-1）——Pure → Irreversible（GET 也对外发起网络请求，服务端留痕、URL
# 可能带敏感 token，"只读"标法与事实不符）；描述同步如实（plugins.md §三
# 表格一并改）
stage tool-fetch 0.1.1
# 0.1.3（2026-10-06，小迭代 +0.0.1）：环境事实写进双工具描述（真实测试
# 复测·事实 2）——无 shell 直 spawn 经 PATH 解析，Windows 上 PATH 通常是
# Git 的 Unix coreutils：用 Unix 风格命令，cmd 内建（dir/del/rmdir /s /q）
# 不存在
stage tool-shell 0.1.3
# 0.1.5（2026-10-10，小迭代 +0.0.1）：权限模型重设计（设计稿《Permission
# Model Redesign (ZCode Alignment) v1》，对齐 ZCode）——shell 政策层收敛
# 两态：Deny 仅剩结构性（元字符/空命令/引号未闭合），**无命令级禁令**，
# 解释器/高危/网络/白名单外一律放行进审批卡（工具 Irreversible，放行
# 即审批；DENY_HARD/DENY_RISKY/BASE_ALLOW/LOCOWRK_SHELL_ALLOW/
# unlockRisky 全退役，LOCOWRK_SHELL_ALLOW/UNLOCK_RISK env 幻影化设置即
# warn）；shell_read 免审面新增 EXACT_ARGV 精确表（pwd / git --version /
# node -v|--version / python[3] --version，ZCode READONLY_EXACT_ARGV_
# COMMANDS 同款，逐 token 全等宁严勿松），git 全局选项守卫零例外
stage tool-shell 0.1.5
# 0.1.4（2026-10-09，小迭代 +0.0.1）：G4 只读证明（可靠性稿 §6）——
# shell_read 从「白名单匹配」升级为「证明只读」：新 proof.rs 证明层
# （词法引号感知——引号内元字符是数据（grep "a|b" 不再误拒），未加
# 引号/双引号内的 |&;()< >`$ 与换行 = 结构面拒 + 语法单条简单命令 +
# 语义（git 全局选项拒（-c 可借配置触发外部程序）/ 写与外部程序旗标
# 拒（--output/--ext-diff/--textconv）/ 敏感凭据路径拒（.env/.ssh/
# credentials/密钥材料））；三态 Safe/Unsafe/Unknown，Unknown 按
# Unsafe 处理（§6.4），证明不了一律拒绝并指路 shell 审批。硬锁/高危/
# unlockRisky/路径围栏/外部路径门全保留（§13 对照：解析器是白名单
# 之上的证明层）；manifest.json 随批补齐 shell_read 条目（此前只列
# shell，展示元数据与 DLL describe 脱节）
stage tool-shell 0.1.4
# 0.1.3（2026-10-06，小迭代 +0.0.1）：① 根提交显式拒绝（P0-2：此前
# `reset --soft HEAD~1` 隐性失败、报错晦涩，模型反复尝试；现在
# rev-list --parents 判父后拒绝并给出 update-ref 出路）+
# git_commit/git_undo_commit 描述同步；② require_ok 保留 stderr 末 6 条
# 非空行（P1-1：只取末行把 git push 无远端的 fatal 主因截掉，模型只见
# "and the repository exists."）
stage tool-git 0.1.3
# 0.2.0（2026-10-06，大改版 +0.1.0 经用户确认）：edit_replace 升级——
# 精确零命中后容错匹配（行尾空白 / CRLF / BOM 归一，缩进·大小写·编辑
# 距离不做）+ match_type/byte_range 如实回报 + 多重命中列行号的结构化
# 报错 + 原子写（临时文件+rename）+ exposure 翻 always（编辑首选；
# Timo 实测教训 = on_demand 截断下模型根本用不上它，退而整文件重写）。
# 设计文档 docs/design/edit_replace.md；工具描述与 edit_diff 互指优先级
stage tool-edit 0.2.0
# 0.2.1（2026-10-09，小迭代 +0.0.1）：G3 Revision Guard——三工具必带
# expected_revision（fs_read / 上一次编辑输出的 revision），陈旧观察拒绝
# 并指路重读（报文带双方 revision）；成功输出带新 revision 供链式编辑。
# 共享基座 plugin_api::fsutil（check_revision / arg_expected_revision）
stage tool-edit 0.2.1
# 0.1.1（2026-10-04，小迭代 +0.0.1）：双层记忆（㉘·A·fix）——memory_write
# +scope="global"（全局偏好层）；读/搜/列 = 项目 ∪ 全局并带层标；未归属
# 会话缺省写拒绝；全局行聊天工具禁删（管理页专属）；headless 语义不变
# 0.1.2（2026-10-05，小迭代 +0.0.1）：F1 记忆文件化——write/delete/undo
# 三落点成功后双写 ~/.Locowrk/memory/ 人类层 md（失败只带 md_warning
# 字段不破契约；总开关 env LOCOWRK_MEMORY_FILES=0 整体跳过）
# 0.1.3（2026-10-05，小迭代 +0.0.1）：F2 同步机械件——memlayout 写路径
# 带 per-file mtime 标记 + 冲突备份（人改 md 未合并时同键写入保留双方）
# + 原子写防撕裂
# 0.1.4（2026-10-06，小迭代 +0.0.1）：memory_undo 描述与行为对齐（真实
# 测试复测 P2-2）——删「usually invoked by the compensation layer」劝退
# 措辞（v2.115 在 fs_restore 上同款裁定），审批说法改「跟随会话权限档」
# （完全档直接放行不出卡，写死 requires approval 与事实不符）
# 0.1.5（2026-10-09，小迭代 +0.0.1）：提示词分层批（设计稿 §5.4）——
# memory_read / memory_search 描述尾补「Use on demand … not as an opening
# step」双保险句（主诱导在 system 块措辞，描述层随批收紧）
stage memory-sqlite 0.1.5
stage tool-mcp 0.1.0
stage tool-notify 0.1.0

echo "done: plugins/tool-echo/{0.1.0,0.2.0} + tool-edit/{0.2.0,0.2.1} + tool-fs/{0.1.3,0.1.4,0.1.5} + tool-fetch 0.1.1 + tool-shell/{0.1.3,0.1.4,0.1.5} + tool-git 0.1.3（tool-sandbox 已随 v2.145 退役不再 staging；tool-shell 0.1.5 = 权限模型重设计批） + memory-sqlite 0.1.5（0.1.5 = 提示词分层批；0.2.1/0.1.5 = G3 批，0.1.4 = G4 只读证明批）+ 其余见上注释"
echo "next: cargo run -p kernel-bin -- demo"
