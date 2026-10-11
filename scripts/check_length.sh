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
#   审计修复批 F-8（2026-10-11）把 organize/tests.rs（865 行，拆前**全仓最长
#   文件**）拆成 tests/{mod(夹具 172)+pipeline+recovery+overflow+views+tools+
#   summary} 七文件（形状照 settings/tests/ 先例：夹具留父模块、子簇
#   `use super::*`，可见性零改动），超限数 36 → 35 ⇒ 基线收紧为 35。
#   审计修复批 F-9（2026-10-11）把 llm/tests.rs（785 行）拆成 tests/{mod(夹具
#   86)+payload 307+transport 88+dispatch 181+assets 163} 五文件，超限数 35 → 34
#   ⇒ 基线收紧为 34。**簇名有一条硬约束**：不能与被测父模块已有的子模块同名
#   （首跑用 `wire` 撞了 `llm::wire` ⇒ 子簇正文的 `super::wire::…` 指向测试簇
#   自己，E0255 + E0425 共 12 条），故该簇改名 `payload`。
#   审计修复批 F-10（2026-10-11）把 kernel/tests.rs（783 行）拆成 tests/{mod(共享件
#   146)+approval 208+prefix 117+tree 350} 四文件，超限数 34 → 33 ⇒ 基线收紧为 33。
#   本批是 F5「巨型测试文件目录化」的第四件，也是根 workspace 侧第一件：拆分等价性
#   由 tmp/v160-verify.mjs 七段判据 + 12 条变异证明（既有测试对结构搬用零证明力）。
#   审计修复批 F-13（2026-10-12）把 settings/tests/llm.rs（681 行 = 巨型测试余 6 件榜首）
#   拆成同目录兄弟簇 {llm 163 + llm_profile 319 + llm_provider 149 + network_proxy 68}，
#   超限数 33 → 32 ⇒ 基线收紧为 32。这一刀与前几件**形状不同**：父目录 settings/tests/
#   早已是目录式，所以不做"tests.rs → tests/"的模块加深，而是把超长簇**横着**分成兄弟簇
#   ⇒ 每个新簇的 `super` 仍 = settings::tests ⇒ 用例正文与可见性一行都不用改。代价 =
#   第五道闸要新增 excludeMod 口径（既有 mod.rs 的共享区不属本刀产物面，否则段1/3/4
#   一起爆假红）+ 分区注释必须随单元搬走（段4「未原样命中 7」抓出来的）。
#   审计修复批 F-14（2026-10-12）把 desktop/src-tauri/src/tests_approval.rs（638 行）拆成
#   tests_approval/{mod 101(共享假件 6)+hub_basic 194+session_timeout 216+project_rules 123
#   +child_session 43}，超限数 32 → 31 ⇒ 基线收紧为 31。形状 = 模块加深一层（`tests.rs →
#   tests/` 同族），但带一条前几刀没有的改动面：**`use` 按引用面分派**（整条复制 = 未用到
#   的绑定在该面成 `unused_imports`，clippy `-D warnings` 实测报 7 条），于是第五道闸的段4
#   需要原文侧白名单 `rewroteOut`（与 `registeredNew` 成对，两侧仍双向闭合）。
#   审计修复批 F-15（2026-10-12）把 plugins-src/memory-sqlite/src/tests.rs（557 行）拆成
#   tests/{mod 46(共享件 5)+kv_basic 205(用例 10)+project_isolation 127(3)+scope_two_layer
#   159(5)+human_md_layer 51(1)}，超限数 31 → 30 ⇒ 基线收紧为 30。闸侧三件事随批：DECL 补
#   `const|static`（顶层 `static ENV_LOCK` 这类共享夹具此前进不了单元集，而夹具段的过滤
#   条件早就写了 static = 两侧口径不齐）、单元名不再吃掉类型标注的冒号（`static ENV_LOCK:
#   Mutex` 曾取成 `ENV_LOCK:`）、新增 S6「glob 传导链断裂」（`store` 只由 mod.rs 的 import
#   提供、四个簇经 `use super::*;` 用它，摘掉那条 import 此前只有 cargo 会断）。
set -u

SUGGEST=400
BASELINE=30

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
