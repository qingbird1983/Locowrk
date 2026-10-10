# D · 插件面 / 构建面 / 文档面健康度审计（只读）

> 范围：`plugins-src/`（10 插件 crate）、`plugins/`、`market/`、`scripts/`、`config/`、
> `.github/`、`.githooks/`、根 `Cargo.toml`/`Cargo.lock`/`README.md`/`.gitattributes`/`.zcodeignore`、`docs/`。
> 未触碰 `crates/`、`desktop/`（仅 grep 全仓验证引用）。
> 工作树基线：v2.145 未提交改动（tool-sandbox 整目录退役、tool-shell 0.1.5 政策层重构）。
>
> **方法与限制**（先验证后书写）：
> - 全程只读，未运行 `scripts/build_plugins.sh` / `build_market.sh`（会写 `plugins/`、`market/`）。
> - 本沙箱内 `bash` 不可执行（`E_ACCESSDENIED`），`check_length.sh` 的行为以**源码+等效重算**证明，
>   实际行数用 `(Get-Content f).Count`（含空行；`Measure-Object -Line` 不计空行，曾导致系统性少算 5%）。
> - 引用验证统一用 `git grep`（跟踪文件）＋ 目录列举（未跟踪产物）；宣布"无引用"前均全仓 grep。
> - 行号 = 当前工作树行号。

---

## 1. 【文档腐烂·高】`docs/design/DESIGN.md` 完全没跟上 v2.145（权威文档仍在讲已退役的权限模型）

- **位置**：`docs/design/DESIGN.md:1152`（决策表第 2 行）
  `| 2 | 硬门（隐私 + 越界） | … **越界门**…：`shell` / `sandbox_run` 命令文本扫出全局声明根外绝对路径 → 强制弹卡`
- **位置**：`docs/design/DESIGN.md:2382`
  `**tool-shell@1.0.0**（plugins.md §二 安全策略：内置白名单 `ls/cat/grep/git status`，`LOCOWRK_SHELL_ALLOW` init 时读一次扩展 …denylist（rm/sudo/curl/… 约 50 项）永远赢、不进审批`）
- **为什么是腐烂**：AGENTS.md 明写「权威设计文档：docs/design/DESIGN.md……修改任何核心模块前先读」。
  而该文件**通篇零处 "v2.145"**（`git grep -c v2.145 docs/design/DESIGN.md` → 无命中），mtime = 2026-10-04。
  v2.145 退役了 DENY_HARD/DENY_RISKY/unlockRisky/BASE_ALLOW/`LOCOWRK_SHELL_ALLOW`，并把 `sandbox_run` 移出越界扫描面；
  v2.145 还在决策表里插入了新的 ③项目 deny 规则 / ⑤项目 allow 规则两序（DESIGN 表仍只有 5 序）。
  读者按文档实现会写出一套与代码相反的策略层。
- **置信度**：高
- **建议动作**：决策表补 ③⑤ 两行、第 2 行删 `sandbox_run`；§14 实现备注的 tool-shell 段加 v2.145 修订注（指向
  `docs/design/NEW/Permission Model Redesign (ZCode Alignment) v1.md`）。§14 可按「历史落点」保留原文，只加注。
- **删除风险**：低（纯文档，只加注不删段）

## 2. 【文档腐烂·高】`plugins/README.md` 整张内置插件表与现状脱节（版本/工具名/副作用/件数全错）

- **位置**：`plugins/README.md:22` `当前官方内置八件（plugins.md 第一~三批）：`
  （现实：`plugins-src/` 下 10 个插件 crate；表里缺 `tool-notify`、`tool-shout`）
- **位置**：`plugins/README.md:26-33` 表格，例：
  - `:26` `| tool-echo | 1.2.0 / 1.3.0 | echo |`（现实暂存 0.1.0 / 0.2.0，v2.56 版本基准重置）
  - `:28` `| tool-fetch | 1.0.0 | fetch | Pure（GET；SSRF 防线） |`（现实 0.1.1 且 **Irreversible**——本仓 `plugins.md:41` 已裁定"原 Pure 作废"）
  - `:29` `| tool-shell | 1.0.0 | shell | Irreversible（白名单 + denylist） |`（现实 0.1.5、双工具 `shell`+`shell_read`、白名单/denylist 已整批退役）
  - `:27/:30/:31/:32/:33` 工具名一律带点号（`fs.read` / `edit.replace` / `memory.put`），而 v2.20 起全仓工具名改下划线（OpenAI 兼容接口拒收点号）
- **为什么是腐烂**：这是 `plugins/` 目录的门面文档，四轴（件数、版本、命名、副作用）全部与代码相反；
  按此表核对会把 `fetch` 当成免审批工具。
- **置信度**：高
- **建议动作**：表格按 `scripts/build_plugins.sh` 的 stage 列表 + 各源码 manifest/describe 重写；工具名去点号；
  `fetch` 改 Irreversible；shell 行补 `shell_read`。或直接改成「以 `plugins-src/*/manifest.json` 为准」的指针段落。
- **删除风险**：低

## 3. 【退役残留/清单脱节·高】`tool-shell` 的 `manifest.json` 未随 v2.145 更新，仍在描述已退役的硬锁/白名单/unlockRisky

- **位置**：`plugins-src/tool-shell/manifest.json:12-13`
  `"zh-CN": "分级命令执行：…其余白名单命令需审批（shell）；硬锁解释器/网络类"`
  `"en-US": "…whitelisted ones need approval (shell); interpreters/network are hard-locked"`
- **位置**：`plugins-src/tool-shell/manifest.json:24`
  `Only a small allowlist is permitted (ls, cat, grep, git status; more via host config). Denylist: interpreter/wrapper/network commands (python, bash, curl, xargs, ...) are never allowed; high-risk user-judgeable commands (rm, sudo, chmod, ...) are refused unless the host unlock switch is on`
- **代码侧（已随 v2.145 改）**：`plugins-src/tool-shell/src/lib.rs:90-101`
  `Any single simple command is allowed — it will ask the user for approval first … No command is banned outright`；
  `src/lib.rs:139-142` `其余命令进审批卡；无命令级禁令`
- **为什么是脱节**：源 manifest 不在本次改动集里（`git status` 无 `manifest.json` 改动），
  但 `build_plugins.sh:45-46` 会把这份源 manifest 原样 sed 出版本号后拷进 `plugins/tool-shell/0.1.5/manifest.json`（实测 staged 与源同文）；
  而 `crates/plugin-service/src/display.rs:46,63` 正是读**版本目录的 manifest.json** 作为展示元数据真源
  → 桌面插件列表里 tool-shell 的中文简介仍是"硬锁解释器/网络类"。
  这正是 v2.135 记档过的那类欠账（"manifest.json 此前只列 shell 一个工具（与 DLL describe 脱节的陈年欠账）"）在同一文件复发。
- **置信度**：高
- **建议动作**：把 `manifest.json` 的 `display.description` 与两个工具的 `description` 同步成 `lib.rs` 现文；
  **根治建议**：`build_plugins.sh` 增加一致性断言（用 `cargo run` 读出 DLL describe 的 tools 名单与 manifest.tools 比对），或干脆 manifest 的 tools 段由脚本从 DLL 生成。
- **删除风险**：低

## 4. 【清单脱节·高】`tool-fetch` 的 manifest 把 `side_effect` 写成 `Pure`，与 DLL describe（Irreversible）相反——UI 徽章错误

- **位置**：`plugins-src/tool-fetch/manifest.json:34` `"side_effect": "Pure"`
- **代码侧**：`plugins-src/tool-fetch/src/lib.rs:86` `side_effect: SideEffect::Irreversible,`
- **权威裁定**：`docs/design/plugins.md:41` `tool-fetch … | Irreversible（2026-10-06 真实测试复测裁定：…"只读"标法与事实不符；原 Pure 作废）`
- **为什么是脱节**：源 manifest 与 staged `plugins/tool-fetch/0.1.1/manifest.json` 逐字相同（实测），
  而 `display.rs:69` 用 `manifest.tools` 的 side_effect 生成 `side_effects`，前端 `desktop/src/components/PluginCard.vue:62`
  与 `plugins/InstalledTab.vue:246` 直接渲染徽章 → 用户看到的是"纯"，而该工具实际每次走审批。
- **置信度**：高
- **建议动作**：改 `Pure` → `Irreversible`，并同步该条 description（现描述也未提"会对外发起请求、需要审批"）。
- **删除风险**：低

## 5. 【清单脱节·中】5 个工具的 manifest schema 缺 G3 必填参数 `expected_revision`；`memory_write` 缺 `scope`

- **位置**：`plugins-src/tool-fs/manifest.json:63-75`（`fs_write`，properties 仅 path/content/create_dirs，required 无 expected_revision）、
  `:77-87`（`fs_delete`）；`plugins-src/tool-edit/manifest.json:27-33`（`edit_replace`）、`:42-48`（`edit_lines`）、`:57-61`（`edit_diff`）
- **证据**：`git grep -n expected_revision -- plugins-src/*/manifest.json` → **0 命中**；
  代码侧 `tool-fs/src/lib.rs` 9 处、`tool-edit/src/lib.rs` 10 处（v2.133 G3 批把 expected_revision 加进 schema 并设为 required）。
  另：`plugins-src/memory-sqlite/manifest.json:23-33`（memory_write 的 properties 只有 key/value/tags）无 `scope`，
  而 `src/lib.rs:87` 已有 `"scope": { "enum": ["project","global"] }`（v2.68 双层记忆）。
- **为什么是脱节但**（重要限定）**当前无消费方**：运行时 manifest 来自 DLL describe（`crates/plugin-host/src/loader.rs:40-41`），
  `display.rs` 只读 display_name/description/icon/author/license/origin/side_effects，不读 schema；
  `plugin-service/src/market.rs:71` 只交叉校验 id/version。→ 属"展示/市场元数据不实"，不改变模型可见 schema。
- **置信度**：高（文本脱节）/ 高（无消费方裁定）
- **建议动作**：随下次插件版本 bump 一并补齐（G3/G2 语义已在代码里，manifest 属漏同步）；
  或按第 3 条建议让脚本/DLL 生成 tools 段，从根上消灭双份 schema。
- **删除风险**：低

## 6. 【构建配置·高】CI 与"门禁全绿"口径不一致：desktop 完全不在 CI，`check_length` 步骤不可能失败

- **位置**：`.github/workflows/ci.yml:20-24`
  `- name: Clippy / run: cargo clippy --workspace --all-targets -- -D warnings`
  `- name: Tests / run: cargo test --workspace`
  `- name: File length limit (400) / run: bash scripts/check_length.sh`
- **位置**：`Cargo.toml:25` `exclude = ["desktop/src-tauri"]`
- **为什么**：`--workspace` 不含 desktop，CI 也没有 desktop/pnpm/vue-tsc 任务 → AGENTS 宣称的
  "clippy 根 0 + desktop --all-targets 0、desktop 356 绿、vue-tsc 0、pnpm build、lint:style 0"里
  除根 workspace 外的**全部门禁在 CI 上不存在**（CI 绿灯 ≠ 项目门禁绿）。
  且 `scripts/check_length.sh:27` 是 `exit 0`（脚本自述"报告式门禁：恒不拦截"），CI 里那一步恒定通过 = 装饰步骤。
- **置信度**：高
- **建议动作**：加 `desktop` job（`cargo fmt --check` / `clippy --all-targets -D warnings` / `test` / `pnpm build` / `vue-tsc`），
  或在 CI 注释里显式记档"desktop 与前端门禁为本地门禁"；`check_length` 步骤改名"（报告，不拦截）"或移出 required 步骤。
- **删除风险**：中（改 CI 影响远程流水线，本次只读未动）

## 7. 【构建配置·中】`check_length.sh` 不扫 `plugins-src/` —— 插件里最大的两个文件永远进不了基线

- **位置**：`scripts/check_length.sh:20-21`
  `done < <(find crates desktop \( -name '*.rs' -o -name '*.vue' \) -not -path '*/target/*' -not -path '*/node_modules/*' 2>/dev/null)`
- **实测**：受检集（crates+desktop）>400 行 = **32 个文件**（与 v2.145 "check_length 32 < 基线 33" 一致）；
  但插件侧未受检文件中 `plugins-src/memory-sqlite/src/tests.rs` = **557 行**、`src/store.rs` = **533 行**。
  也就是说 AGENTS 里"check_length 基线 33 个文件超 400 行（插件里 memory-sqlite/src/store.rs 508 行、tests.rs 484 行是最大的）"
  这句**自相矛盾**：这两个插件文件从未被该脚本扫描（文中的 508/484 也与含空行的真实行数不符，属不计空行的口径）。
- **置信度**：高
- **建议动作**：`find` 列表加 `plugins-src`（加入后基线 32 → 34，需按"三问"给出两条自检理由）或明确记档"插件暂不受 400 行建议值约束"。
- **删除风险**：低

## 8. 【文档腐烂·高】`AGENTS.md` 191KB：`## 项目概览` 占 94%，含**两个"当前进度"**且版本链断裂

- **位置**：`AGENTS.md:3-38` 合计 **99,965 / 106,546 字符 = 94%**（其余全部章节仅 6.6K 字符）
- **位置**：`AGENTS.md:8` `* **当前进度**：**权限模型重设计已完成（v2.145…`（正确）
- **位置**：`AGENTS.md:37` `* **当前进度**：**㉘·A 项目级记忆隔离 + 侧栏项目段常驻已完成（v2.55）**…`
  → **单行 27,663 字符**（全文件最大行，占 26%），标签仍是"当前进度"，位置却在 v2.58/v2.59/v2.63/v2.71/v2.75 之后
- **位置**：版本链逐行实测（行号:首个版本号）——
  `:8` v2.145（当前）→ `:9..:29` v2.144…v2.125（单调）→ **`:30` 直接跳 v2.75** → `:31` v2.72 → `:32` v2.71 → `:33` v2.64 → `:34` v2.63 → `:35` v2.59 → `:36` v2.58 → `:37` v2.55
  → **v2.76–v2.124 整段缺失（49 版），链条非单调**；同版本历史在 `docs/design/CHANGELOG.md` 已是第二份（两份文本各自维护，非逐字相同）
- **为什么是问题**：文件头部 94% 是历史流水，agent 每次读取都要吞 19 万字节（本次工作区指令预算就因此从
  191,297 被截到 65,142 字节）；"当前进度"出现两次、且第二处是 2.55 时代的巨块，任何按标签检索的读者都会被误导。
- **置信度**：高
- **建议动作（未动手，供裁定）**：
  1. `AGENTS.md` 只保留 `:8` 的"当前进度" + 最近 2–3 版"此前进度"，其余整段移出；
  2. 承接文件用已有的 `docs/design/CHANGELOG.md`，但**按版本拆成目录**（如 `docs/design/CHANGELOG/v2.145.md`，一条一文件），
     `CHANGELOG.md` 只留索引表（版本 / 日期 / 一句话 / 链接）；
  3. `:37` 的 v2.55 巨块拆回 v2.7–v2.55 的分段条目（它本身是"层层 此前：…"内联堆叠的产物）；
  4. 保留一句规则「AGENTS.md 只写当前 + 最近 N 版；更早查 CHANGELOG 索引」。
- **删除风险**：低（内容不丢，只迁址 + 建索引；但迁移需一次性人工核对版本号）

## 9. 【文档腐烂·中】`CHANGELOG.md` 同为巨型单行文件，且 `v2.75` 撞号、`:177` 有重复片段

- **位置**：`docs/design/CHANGELOG.md` 共 **394,866 字节 / 109 行**（平均 3.6KB/行——每版一整行）
- **位置**：`:81` `*文档版本：v2.75（**LLM 兼容层重构**…` 与 `:86` `*文档版本：v2.75（**前端统一标准 + 机械门禁**…`
  → **同一版本号两条互不相关的批**（`git grep -c` 复算：v2.75 出现 2 次，其余 80 个版本各 1 次）
- **位置**：`:177` 行内短语重复：`…v2.19 补记⑨上下文管理收口落点（提交 92f298b）：v2.19 补记⑨上下文管理收口落点（提交 92f298b）：`
- **为什么是问题**：与第 8 条同源；撞号会让"按版本号追溯"这一唯一检索手段失效。
- **置信度**：高
- **建议动作**：随第 8 条的拆分一并处理；撞号二者择一改号（建议前端标准那条改 v2.76 或标 `v2.75b`）并修正行内重复。
- **删除风险**：低

## 10. 【文档腐烂·中】`docs/design/Sandbox Execution Design.md` 是**零引用孤儿**，且主题（CubeSandbox + tool-sandbox）已整批退役

- **位置**：`docs/design/Sandbox Execution Design.md:1` `# Locowrk 沙箱执行设计：集成 CubeSandbox`（575 行，头部只写"版本：1.0"，无废弃标记）
- **引用核查**：`git grep -n "Sandbox Execution Design"` → **全仓零命中**（无任何文档/代码指向它）
- **内容核查**：`docs/design/plugins.md` 的 sandbox 段与 AGENTS v2.145 纪要都说明实际交付走的是
  「Windows 受限令牌 + JobObject」（v2.31），**不是**本文的 CubeSandbox REST/SDK 方案；
  文中 `sandbox_execute` / `sandbox_read_file` / `sandbox_write_file` / `sandbox_snapshot` / `sandbox_rollback` / `sandbox_status`
  六个工具从未实现，且整批已在 v2.145 退役（本仓退役注见 `plugins.md:200-204`）。
- **置信度**：高
- **建议动作**：移到 `docs/design/archive/` 并在首行加「未采纳方案 + 主题已于 v2.145 退役」两行说明；
  不建议直接删（含 CubeSandbox 能力矩阵，属可复用调研）。
- **删除风险**：中（真删会丢一份外部服务调研；归档零风险）

## 11. 【退役残留·中】`docs/design/plugins.md` 两处表格仍把 `tool-sandbox` 当待办，与同文件退役注自相矛盾

- **位置**：`docs/design/plugins.md:27` `| P2 | tool-sandbox | shell 在容器/WASM 里跑 |`（"一、总览"表）
- **位置**：`docs/design/plugins.md:152` `| tool-sandbox | shell 需要在容器/WASM 里跑 |`（"六、P2"表）
- **位置**：`docs/design/plugins.md:200-204`（同文件已有 `**v2.145 退役注**：本节所述 tool-sandbox 已整批退役…`）
- **连带**：`:206-213` 仍以现状语气描述单工具 `sandbox_run` 的参数/env/输出；
  `:18/:20/:21/:24/:26` 的 `tool-http`/`memory-vector`/`ctx-compact`/`mw-ratelimit`/`tool-browser` 也停留在"待做"口径，
  其中 `ctx-compact` 桌面链 v2.137 已退役、`tool-browser` 的能力由 v2.144 的 `browser_open`/`browser_console` + 浏览器页签承接。
- **置信度**：高
- **建议动作**：两张表加"状态"列（已落地 / 已退役 / 未排期），退役行删除线 + 指向退役注；`:206-213` 段首加"（已退役，留档）"。
- **删除风险**：低

## 12. 【文档腐烂·中】UI 权威文档的"现状表"仍列 v2.136/v2.137/v2.145 已退役的设置键与命令

- **位置**：`docs/design/UI shell and patterns.md:193`（§3.2 MemorySection 行）
  `…另挂交接更新开关行（memory.handoffUpdate 缺省开…）…且 autoExtract 缺省改开…M5 v4.19 起再挂缺省整理方式行（memory.defaultOrganizer…）+ 经历记忆开关行（memory.experiences…）`
- **位置**：`:202`（DeveloperSection 行）`…M1：… context.stabilityRounds 稳定期轮数 1–10 缺省 3…`
- **位置**：`:883` 与 `:999`（§4.2 命令表）`session_set_organizer(sessionId, mode?)`；`:265`/`:855`（整理模式两态菜单）
- **同文件版本条已宣告退役**：`:1409`（v4.27）`缺省整理方式行退役（标准整理整面退役，一切会话皆树）`、
  `:1406`（v4.30）`设置分区「命令终端」整区退役`——**版本条与 §3.2/§4.2 现状表互相打脸**。
  （退役事实：autoExtract/experiences/handoff 后台改写 = v2.136；thresholdPct/strategy/stabilityRounds/ctx-compact 桌面链、
  `session_set_organizer`/`memory.defaultOrganizer` = v2.137；`shell.unlockRisky`/ShellSection = v2.145。）
- **置信度**：高
- **建议动作**：按版本条已写明的语义回改 §3.2 分区表与 §4.2 命令表（这是"唯一权威 UI 文档"，表就是实现依据）。
- **删除风险**：低

## 13. 【文档腐烂·高】根 `README.md` 停在"阶段一"，结构清单缺 `plugins-src/`、`market/` 等且把已退役的沙箱列为插件类型

- **位置**：`README.md:10` `- 当前状态：工作区骨架已建，处于落地路径**阶段一**（内核骨架 + 事件日志）`（现实 v2.145，全部编号阶段与多批 UI/权限/记忆批已落地）
- **位置**：`README.md:5` `…模型适配器、工具、会话存储、沙箱、Agent 循环本身都是插件`（沙箱已 v2.145 退役）
- **位置**：`README.md:24-25` `plugins/            插件安装目录（多版本并存）` / `config/             profile.toml 插件组合配置`
  （`config/profile.toml` 全仓唯一"加载"证据是 `crates/kernel-bin/src/main.rs:7` 的文档注释；ROADMAP.md:940 仍记 `profile.toml 设置面板接线 | 看需求` → 未接线）
- **为什么是问题**：仓库第一入口文档给出的状态与结构都与现实不符（缺 `plugins-src/`、`market/`、`scripts/`、`docs/`）。
- **置信度**：高
- **建议动作**：`当前状态` 改为"v2.145（权限模型重设计）"并给 CHANGELOG 指针；结构补齐四目录；`profile.toml` 标注"骨架，未接线"。
- **删除风险**：低

## 14. 【构建面·中】`build_plugins.sh:48` 的 `cp -R` 非幂等 → 暂存目录出现 `assets/assets/icon.svg`（实测 27 份）

- **位置**：`scripts/build_plugins.sh:47-49`
  `if [[ -d "plugins-src/$id/assets" ]]; then` / `  cp -R "plugins-src/$id/assets" "$dst/assets"` / `fi`
- **证据**：`plugins/**/assets/assets/icon.svg` 实测 **27 份**（暂存图标共 60 份）：
  `plugins/memory-sqlite/0.1.0|0.1.1|0.1.3|0.1.4|0.1.5/…`、`plugins/tool-echo/0.1.0|0.2.0/…`、
  `plugins/tool-fs/0.1.0|0.1.2|0.1.3|0.1.4|0.1.5/…` 等。`cp -R src dst` 在 `dst` 已存在时把 `src` 目录整体放进 `dst` 内。
- **对照**：`scripts/build_market.sh:80-81` 用 `cp -f "plugins-src/$id/assets/icon.svg" "$dst/assets/icon.svg"`（点名到文件）→ 幂等，`market/` 无此现象。
- **为什么是垃圾**：多一层永不被引用的副本（`manifest.icon` = `assets/icon.svg`），每次重跑都会重建；目录结构误导人工核对。
- **置信度**：高
- **建议动作**：`mkdir -p "$dst/assets" && cp -f "plugins-src/$id/assets/"* "$dst/assets/"`（或 `cp -R "plugins-src/$id/assets/." "$dst/assets/"`）。
- **删除风险**：低（`plugins/` 全目录 gitignore，仅本地产物；改脚本后重跑自然净）

## 15. 【构建面·低】两个 build 脚本重复 23 行骨架，可合并为一个带版本表的脚本

- **位置**：`scripts/build_plugins.sh:14-51`（`root` 解析 / `find_artifact` / `stage`）与
  `scripts/build_market.sh:12-83`（同形骨架，仅 artifact 名推导与 manifest 生成方式不同）
- **实测**：两文件逐行交集中**完全相同**的行 = **23 行**（占 `build_market.sh` 89 行的 26%），
  差异集中在：`find_artifact` 参数化程度（plugins 版带 `${id//-/_}`，market 版写死 `tool_shout`）与 manifest 生成（`sed` 拷源 vs `cat >` heredoc）。
- **置信度**：高
- **建议动作**：合并为 `scripts/build_plugins.sh` 吃一张表（`id ver [desc] [manifest=copy|generate]`），
  或抽 `scripts/lib/stage.sh` 供两脚本 source；市场那两条 `stage 1.0.0/1.1.0` 变成表内两行。
- **删除风险**：中（脚本是开发者入口，合并需一次全量重跑核对产物布局）

## 16. 【死代码·低】`pre-commit` 对"恒不失败"的脚本做拦截判断

- **位置**：`.githooks/pre-commit:8` `bash "$root/scripts/check_length.sh" || exit 1`
- **证据**：`scripts/check_length.sh:26-27` `# 报告式门禁：恒不拦截…` / `exit 0`；hook 首行注释还写"①单文件行数上限检查"
- **为什么是死分支/矛盾**：`|| exit 1` 永不触发，且让人误以为 400 行是提交硬门（脚本语义已改成建议值）。
- **置信度**：高
- **建议动作**：删 `|| exit 1`，注释改"报告（不拦截）"；若确实想拦，应改为对"新增超限文件"判错再保留该分支。
- **删除风险**：低

## 17. 【死代码·低】`scripts/check-style.mjs` 的 `warn` 级别分支不可达

- **位置**：`scripts/check-style.mjs:24-28` 四条 RULES **全部** `"error"`；
  `:75` `const warns = hits.filter((v) => v.level === "warn");`；`:81` `if (warns.length) parts.push(…二级报告…)`
- **为什么是死代码**：没有任何规则产生 `warn` → `warns` 恒空、`:81` 分支与 `二级报告` 文案永不可达
  （`:8` 注释亦已说明"任意 px 字号…v2.87 起全站迁移完成"并已按 error 判）。
- **置信度**：高
- **建议动作**：删 `warn` 级别与相关分支，或保留机架但在 RULES 上方注明"当前无 warn 规则（历史迁移期遗留）"。
- **删除风险**：低

## 18. 【配置面·低】`.gitignore:26-28` 的三条插件二进制规则是死规则（被 `:37 plugins/*` 整体覆盖）

- **位置**：`.gitignore:26-28`
  `plugins/**/*.dll` / `plugins/**/*.so` / `plugins/**/*.dylib`
- **证据**：`git check-ignore -v plugins/tool-echo/0.2.0/tool_echo.dll`
  → `.gitignore:37:plugins/*`（命中的是第 37 行，不是第 26 行——目录被排除后 git 不再下探）
- **为什么是死规则**：`:37 plugins/*` + `:38 !plugins/README.md` 已经把 `plugins/` 下全部内容排除，
  上面三条永不生效（保留只起"文档"作用，且容易让人误以为 `plugins/` 只忽略二进制）。
- **置信度**：高
- **建议动作**：删三条并在 `:37` 注释里写明"整个 plugins/ 只保留 README.md"；或反过来删 `:37` 改逐类忽略（不建议，会引入漏 ignore 风险）。
- **删除风险**：低（改前用 `git status` 复核无意外解禁文件；`.gitattributes` 与 `git ls-files -i -c --exclude-standard` 实测当前无"被跟踪却被忽略"的文件）

## 19. 【配置面·低】`.zcodeignore` 未随 `.gitignore` 再同步：缺 `docs/issue/`

- **位置**：`.gitignore:40-41` `# 用户问题批次工作目录（docs/issue，不入库）` / `docs/issue/`
- **位置**：`.zcodeignore:37-40` 同步区止于 `!plugins/README.md`，紧跟 `# ===== ↑ 以上同步自 .gitignore…` 分隔行 → **无 `docs/issue/`**
- **实测**：`git diff --no-index .gitignore .zcodeignore` 的唯一"同步区"内容差异就是这一条
  （其余差异是 ZCode 默认规则块与提示行）
- **影响**：`docs/issue/` 现有 13 张 png + 1 份 md（用户问题批次素材），对 ZCode 可见（属私有工作材料）。
- **置信度**：高
- **建议动作**：在 `.zcodeignore` 同步区补 `docs/issue/`（或重跑一次"从 .gitignore 同步"）。
- **删除风险**：低

## 20. 【漂移·低】构建脚本收尾文案 + 9 个插件 `Cargo.toml` 的版本注释仍写 v2.56 之前的版本号

- **位置**：`scripts/build_plugins.sh:146`
  `echo "done: …（tool-sandbox 已随 v2.145 退役不再 staging；…）+ memory-sqlite 0.1.5（0.1.5 = 提示词分层批；0.2.1/0.1.5 = G3 批，0.1.4 = G4 只读证明批）+ 其余见上注释"`
  → 一行里 `0.1.5` 同时指 memory-sqlite 与 tool-shell，`0.2.1/0.1.5 = G3 批` 无主语；且未列 `tool-mcp`/`tool-notify`。
- **位置**：版本注释过期（v2.56 版本基准重置后未改）：
  `plugins-src/memory-sqlite/Cargo.toml:19`、`tool-edit/Cargo.toml:18`、`tool-fetch/Cargo.toml:19`、`tool-fs/Cargo.toml:17`、
  `tool-git/Cargo.toml:18`、`tool-notify/Cargo.toml:19`、`tool-shell/Cargo.toml:30` 均为
  `# （scripts/build_plugins.sh 按 1.0.0 构建一次并暂存）。`；`tools-echo` 版 `tool-echo/Cargo.toml:17`
  `# （scripts/build_plugins.sh 负责按 1.2.0 / 1.3.0 各构建一次）；`（现实 0.1.0 / 0.2.0）。
- **置信度**：高
- **建议动作**：注释一律改成"版本见 `scripts/build_plugins.sh` 的 stage 列表"（把版本真源收敛到一处，避免第三次过期）；
  `:146` 改成每插件一行的分组摘要。
- **删除风险**：低

## 21. 【死配置·中】`tool-shell` 声明了两个用不到的 `windows-sys` feature

- **位置**：`plugins-src/tool-shell/Cargo.toml:21-24`
  `features = [ "Win32_Foundation", "Win32_Security", "Win32_System_JobObjects", "Win32_System_Threading", ]`
- **代码侧**：`plugins-src/tool-shell/src/exec.rs:284-288` 只
  `use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};` + `use windows_sys::Win32::System::JobObjects::{…};`；
  全 crate 的 windows_sys 路径只出现这两处（`git grep -oE "windows_sys::[A-Za-z0-9_:]+"` 实测：
  仅 `Win32::Foundation::` 与 `Win32::System::JobObjects::`，API 集合 = Job 四件 + CloseHandle/HANDLE）。
- **说明**：v2.26 记档称"windows-sys 0.61 需补 `Win32_Security` feature——CreateJobObjectW 签名引 SECURITY_ATTRIBUTES"；
  但 `Win32_Security` 在**本插件现存代码**里已无引用（受限令牌那套随 tool-sandbox 一起退役了），`Win32_System_Threading` 亦无引用。
- **置信度**：中—高（静态扫描；未编译验证）
- **定论命令**：`cargo tree -p tool-shell -e features` 看两 feature 是否真被依赖链拉起；删后 `cargo clippy -p tool-shell --all-targets` 必须 0。
- **建议动作**：删 `Win32_Security`、`Win32_System_Threading`（若编译失败即恢复并补一句"签名需要"注释）。
- **删除风险**：中（需一次编译验证，本次未跑）

## 22. 【死代码·低】`policy.rs` 把"引号未闭合"与"空命令"混成同一条报文

- **位置**：`plugins-src/tool-shell/src/policy.rs:131-133`
  `if quote.is_some() { return None; // 引号不闭合 → 判空拒绝（宁拒勿猜） }`
- **位置**：`plugins-src/tool-shell/src/policy.rs:89-93`（`decide` 消费处）
  `let Some((program, rest)) = split_command(command) else { return Decision::Deny { reason: "shell command is empty".into() } }`
- **为什么是问题**：v2.145 文档把 Deny 面定义为三项（元字符 / 空命令 / 引号未闭合），但 `shell "git status`（引号未闭合）
  会收到 "shell command is empty" ——与 v2.135 确立的"错误信息要能教模型下一步"口径相悖（模型会去检查空串而不是补引号）。
- **置信度**：高（代码直读）
- **建议动作**：`split_command` 返回 `Result<_, SplitError>`（`Empty` / `UnclosedQuote`）或让 `decide` 先行判引号闭合，给两条不同报文。
- **删除风险**：低

## 23. 【文档腐烂·低】`docs/design/BUG/` 两份清单的"状态"行与自身处理记录矛盾

- **位置**：`docs/design/BUG/Tool Layer Issues Todo List.md:5` `> 状态：待处理`
  而 `:153-164` 的「六、处理记录（2026-10-06，v2.116 落地）」把 P0-1/P0-2/P1-1/P2-1/P2-2/事实 1/事实 2 全部标 ✅。
  另 `:116` 把 `sandbox_run` 列入"本轮测试确认正常的项"（历史记录可留，但建议加退役注）。
- **位置**：`docs/design/BUG/bug-report.md:3`
  `> **状态：Bug 1 / Bug 2 已修复…；Bug 3 为本轮实测新增、待上报。**`
  而 Bug 3（`git_undo_commit` 根提交）已由 tool-git 0.1.3 按"方向①显式拒绝"落地（见上清单 `:158`）；标题 `:1` 仍写 "plugin 0.2.0"。
- **为什么是腐烂**：两份文件都以"待办"为准绳指导排查，实际已闭环；读者可能重做已修项。
- **置信度**：高
- **建议动作**：两处状态行改「已处理（v2.116，含例外记档）」；`bug-report.md` 标题版本改"（历史存档）"。
- **删除风险**：低

## 24. 【文档腐烂·中】`docs/design/NEW/` 的"已落地"断言未随 v2.145 更新（仍称硬锁/高危/unlockRisky 全保留）

- **位置**：`docs/design/NEW/Reliability Reinforcement Design.md:755`
  `| 只读证明 | 与 v2.26 三级分类（硬锁/高危/白名单）+ shell_read 固定表共存 | ✅ **已落地（G4 独立小批 v2.135…）**…硬锁/高危/unlockRisky/路径围栏/外部路径门全保留（shell 管线零改动）`
- **位置**：`docs/design/NEW/Model Capability and Multimodal Phase Plan.md:154`
  `| G4 只读证明 | shell_read 白名单之上的解析器证明层；硬锁/围栏/unlockRisky 全保留 | ✅ **已落地**（独立小批 v2.135…）`
- **为什么是腐烂**：v2.145 把 DENY_HARD/DENY_RISKY/unlockRisky/BASE_ALLOW 全退役、`decide()` 只剩结构性拒绝，
  两处"全保留"现在是假陈述（`shell` 管线本身确实零改动，但"硬锁/高危/unlockRisky"这一半已被删除）。
- **置信度**：高
- **建议动作**：两行加 v2.145 修订注（"上述策略面已在 v2.145 退役；G4 证明层与路径围栏保留"）。
- **删除风险**：低

## 25. 【清单脱节·低】`tool-shout` 1.0.0 的 schema/市场 manifest 宣称 `mode: repeat`，但该版本运行时会拒绝

- **位置**：`plugins-src/tool-shout/src/lib.rs:33` `const REPEAT_SINCE: Version = Version::new(1, 1, 0);`
  与 `:53` 的 schema `"mode": { "type": "string", "enum": ["plain", "repeat"] }`；`:113` `"repeat" if repeat_supported(version) => …`（否则回落 `other` 分支报 `does not support mode`；门函数 `:128`）
- **位置**：`market/tool-shout/1.0.0/manifest.json`（`"enum": ["plain","repeat"]`）——该 manifest 由 `scripts/build_market.sh:42-79` 的 heredoc 生成，
  **1.0.0 与 1.1.0 共用同一 schema**，故 1.0.0 包自述能力 > 实际能力。
- **说明**：manifest 与 DLL describe 二者**彼此一致**（都来自同一份代码/模板），所以这不是"清单脱节"，
  而是"元数据比行为乐观"——演示升级叙事（0.1.0→0.2.0、1.0.0→1.1.0）的副作用。
- **置信度**：高
- **建议动作**：低优先；若要做，让 schema 按 `plugin_version()` 分支（<1.1.0 只列 `plain`），或在 1.0.0 描述里注明"repeat 需 1.1.0"。
- **删除风险**：低
- **同族小项（market/README.md）**：`market/README.md:13` `│   └── manifest.json       # 市场元数据 { id, version, description }`
  仍是 v1 极简形态描述，而实际 `market/tool-shout/*/manifest.json` 是 packaging.md v2 全量 manifest
  （`abi_version`/`kind`/双语言 `display_name`/`description`/`icon`/`author`/`origin`/`license`/`tools[]`，模板见 `scripts/build_market.sh:42-79`）。
  建议该行改为"packaging.md v2 全量 manifest（展示元数据 + tools 清单）"。置信度 高 / 删除风险 低。

---

## 待验证（不凑数，附定论命令）

| # | 事项 | 定论命令 / 判据 |
|---|---|---|
| T1 | 第 21 条 windows-sys 两 feature 是否真的无用 | `cargo clippy -p tool-shell --all-targets`（删 feature 后；失败即无用结论作废） |
| T2 | 第 5 条的 manifest schema 是否真无消费方 | `cargo test -p plugin-service`（若 `market/installer` 用例对 schema 有断言会现形） |
| T3 | `docs/design/BUG/bug-report.md` Bug 3 是否已被独立复测确认修复 | 按文末「附」节最小仓库重建 + `git_undo_commit` 传根提交，期望"根提交不可撤销 + update-ref 出路"报文（tool-git 0.1.3 语义） |
| T4 | `plugins/tool-shell/0.1.5/tool_shell.dll` 的 describe 是否真等于源码现文 | `cargo build -p tool-shell`（**会写 target/**，本次未跑）后比对 `plugin_host::loader` 读出的 tools；或跑 `bash scripts/build_plugins.sh` 后看 GUI 插件卡（**会写 plugins/**，本次未跑） |
| T5 | `plugins-src` 加入 `check_length.sh` 后确切超限文件数 | 在 `scripts/check_length.sh:20` 的 find 列表加 `plugins-src` 后重跑（预期 +2：memory-sqlite tests.rs 557 / store.rs 533） |

---

## 已核查为健康的部分

1. **`tool-sandbox` 退役面：插件/构建/配置面无代码残留（0 处）**。
   `plugins-src/tool-sandbox/` 已整目录删除（`git status` 13 个 `D`）；
   根 `Cargo.toml:3-22` 的 workspace members 无该 crate；`plugins/` 下无 `tool-sandbox/` 目录；
   `market/` 只含 tool-shout；`config/profile.toml` 无 sandbox 条目；
   `scripts/*.sh`、`.github/`、`.githooks/`、`.gitignore`/`.gitattributes`/`.zcodeignore` 均无 `tool-sandbox`/`sandbox_run` 的功能性引用
   （`scripts/build_plugins.sh:146` 仅有一句"已随 v2.145 退役不再 staging"的说明，属**有意留痕非残留**）。
   其余出现全部集中在 `docs/`（退役注、历史台账、第 10/11 条的失真处）。
2. **10 个插件的 manifest `tools` 名单与代码 DLL `describe` 名单逐个相符（29 个工具，差集 = 空）**：
   tool-echo(echo)、tool-fs(6)、tool-edit(3)、tool-fetch(fetch)、tool-git(6)、memory-sqlite(6)、tool-mcp(2)、tool-notify(notify)、
   tool-shell(shell+shell_read)、tool-shout(shout) —— v2.135 那类"manifest 只列一个工具"的欠账**已无复发**（`git grep 'name:' plugins-src/*/src/lib.rs` 与各 manifest 逐一比对）。
   各 manifest 的 `side_effect` 与代码一致，唯一例外 = tool-fetch（第 4 条）；
   `exposure` 仅 memory-sqlite manifest 有（write/read/search = always，与代码一致），其余插件未写但**当前无消费方**（`display.rs` 不读 exposure）。
3. **`plugin_api` 契约层复用良好、无孤儿**：静态扫描 10 个 crate 的 `pub`/`pub(crate)` 项与全部 `fn`，
   除 `#[test]` 函数（测试框架按名发现，属误报）外**未发现零引用项**；`policy.rs`/`proof.rs` 里
   `DENY_HARD`/`DENY_RISKY`/`BASE_ALLOW`/`unlockRisky` 的痕迹已清干净（`policy.rs:24-47` 只剩 READ_ONLY/EXACT_ARGV/METACHARS），
   退役 env 走显式静态告警（`policy.rs:59-78 retired_env_warns`）——"不静默"纪律守住了。
4. **依赖核对：全部真用，无未使用依赖**。
   `memory-sqlite` → `rusqlite`（`src/store.rs` 4 处 + tests）；`tool-fetch`/`tool-notify` → `ureq`（各 4/2 处）；
   `tool-shell` → `libc`（`src/exec.rs:273 libc::killpg`）、windows-sys（`src/exec.rs:284-288`）；其余 6 个插件**只依赖 `plugin-api`**。
   唯一可疑项是 Cargo feature 而非 crate（第 21 条）。
5. **`plugins-src` 无 TODO/FIXME/HACK/XXX、无注释掉的代码块**（`git grep -E "TODO|FIXME|HACK|XXX" -- plugins-src scripts config .github .githooks` → 零命中；
   `todo!()` 仅出现在 `tool-edit/src/tests.rs:43` 的测试夹具字符串里）。
6. **`plugins/` 与 `market/` 的跟踪入库面干净**：`git ls-files plugins` = **仅 `plugins/README.md`**（staged manifest/图标按设计不入库）；
   `git ls-files market` = README + tool-shout 两版的 `manifest.json` + `icon.svg`；
   `market/tool-shout/*/manifest.json` 与 `build_market.sh` heredoc **逐字段一致**（`display_name`/`description`/`icon`/`author`/`origin`/`license`/`tools` 全对，唯一偏差见第 25 条）；
   二进制正确被 ignore（`plugins/**/*.dll` 与 `market/**/*.dll` 实测命中）。
   `git ls-files -i -c --exclude-standard` **零输出** → 没有"被跟踪却被忽略"的文件。
7. **`.gitattributes` 与现状自洽**：`* text=auto` + `*.sh text eol=lf`（hook/脚本 LF 保证，与 pre-commit 执行需求一致）；
   `.gitignore` 中 `.mimosa/`、`.zcode/`、`/tmp/`、`desktop/src-tauri/gen/schemas` 对应的目录实测都存在（非"忽略已不存在的东西"）。
8. **SSRF 防线在 fetch/notify 两处虽为复制粘贴（第 3 类候选收敛项，见"优化建议"）
   但**内容同形、无漂移劣化**：`git diff --no-index` 显示两者仅差 env 常量名与错误前缀字符串（15 增 / 14 删），逐行逻辑与测试断言一致。
9. **v1 老设计稿并非"腐烂"而是显式存档**：`docs/design/Memory Tree Design.md` 与 `docs/design/KV Prefix Cache Design.md`
   都被对应 v2 文档明确声明"v1.0 原文保留作对照，两文档并存，以本文为准"——属**有意保留**；
   仅建议在 v1 头部补一行反向指针（"已被 v2 取代，见 xxx"），便于单独打开 v1 的人不入歧途。
10. **`docs/上下文与记忆工作流程报告.md` 的版本注记是"活文档"正面样本**：
    头部连续 6 条 `⚠ v2.136/v2.137/v2.138/v2.139…修订` 把被推翻的段落逐条标出（`autoExtract/experiences/handoff 三机制退役`、
    `thresholdPct/strategy/stabilityRounds 与 ctx-compact 桌面链退役`），并声明"与设计文档冲突时以文档为准"——建议第 1/12/24 条照此格式修。

---

## 优化建议（落到文件）

| 建议 | 落点文件 | 具体动作 |
|---|---|---|
| S1 收敛版本真源 | `scripts/build_plugins.sh` + 9 个 `plugins-src/*/Cargo.toml` 注释 | stage 表成为唯一版本真源；Cargo 注释改为指针（消灭第 20 条这类反复过期） |
| S2 manifest 由 DLL 生成或校验 | `scripts/build_plugins.sh`（新 `--check-manifest` 分支）或新 `scripts/check_manifests.sh` | 从 DLL describe 读出 tools（name/description/schema/side_effect）与源 manifest 比对，差异即失败；一次性修掉第 3/4/5 条 |
| S3 两个 build 脚本合一 | `scripts/build_plugins.sh`（吃表）、`scripts/build_market.sh`（薄包装或删除） | 见第 15 条；market 的 heredoc 改成表里一行 `tool-shout 1.0.0 "<desc>" generate` |
| S4 暂存流程幂等化 | `scripts/build_plugins.sh:47-49` | 见第 14 条；重跑即净 `assets/assets` |
| S5 门禁覆盖补齐 | `.github/workflows/ci.yml`、`scripts/check_length.sh:20` | 加 desktop job；find 列表加 `plugins-src`；`check_length` 步骤改名注明"报告不拦截" |
| S6 文档分层收敛 | `AGENTS.md`、`docs/design/CHANGELOG.md` → `docs/design/CHANGELOG/` 目录 | 见第 8/9 条：AGENTS 只留当前 + 最近 2–3 版；CHANGELOG 拆目录并建索引表；修 v2.75 撞号 |
| S7 退役残留清扫表 | `docs/design/plugins.md`、`docs/design/UI shell and patterns.md`、`docs/design/DESIGN.md`、`docs/design/NEW/*` | 按第 1/11/12/24 条逐表加"状态"列或修订注；建议统一格式「> **vX.YZ 退役注**：…（历史原文保留）」——本仓已有此惯例 |
| S8 孤儿稿归档 | `docs/design/Sandbox Execution Design.md` → `docs/design/archive/` | 见第 10 条；同时给 `docs/design/` 加一份 `INDEX.md`（列出"权威/存档/已退役"三类），避免下次再出现零引用孤儿 |
| S9 插件共享基座候选（本批新发现，供后续批裁定） | `crates/plugin-api`（新增 `netsafe` / `plugin_boilerplate` 或宏） | ①`ssrf.rs` + `ssrf/tests.rs` 两插件合计 361 行、仅差 env 名与错误前缀字符串（逐行同形）→ 可上收为 `plugin_api::netsafe::guard(url, prefix, env_allow)`；②10 份 `zh_en()`（各 5 行）、10 份 `plugin_version()`（3 行）、10 份 `unsafe impl Send/Sync + SAFETY`、10 份 `PluginManifest{author/origin/license/icon}` v2 展示元数据样板（各约 10 行）→ 可收成 `plugin_api::manifest::official(...)` + 一个 `declare_official_plugin!` 宏；③`arg_u64` 6 份（`tool-fs:270`/`tool-git:260`/`tool-shell/exec.rs:83`/`memory-sqlite:322`/`tool-fetch:137`/`tool-notify:183`，其中 fs 与 git 两版分支写法已轻微漂移）→ 收进 `plugin_api::args`。合计可减约 600–700 行重复代码，且消灭"两处同源实现漂移"这一类缺陷（v2.133 的 ENV_LOCK 分裂、v2.135 的 gc 序错都是同源代码分家的产物） |

---

*审计台账：`docs/reports/_parts/D-plugins-build-docs.md`（只读审计，未修改任何被审文件）*
