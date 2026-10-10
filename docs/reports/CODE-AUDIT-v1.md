# Locowrk 全仓代码审计报告（垃圾代码 / 死代码 / 无用代码 + 优化建议）

- 审计对象：`F:\Warehouse\AIwork\Locowrk` 当前工作树（含未提交的 v2.145 权限模型重设计改动）
- 审计时间：2026-10-10
- 审计方式：4 个并行子域深读（根 workspace crates / desktop Rust / 前端 / 插件与构建文档）+ Team Lead 独立机械取证与交叉验证
- 规模：Rust 388 文件、Vue 92、TS 52，代码约 **9.22 万行**（不含 `target/`、`node_modules/`、`dist/`）
- 证据台账（可逐条复核）：本目录 `_parts/0-mechanical.md`（Lead 亲测）、`_parts/1-i18n-orphans.md`（Lead 复算）、`_parts/A-root-crates.md`、`_parts/B-desktop-rust.md`、`_parts/C-frontend.md`、`_parts/D-plugins-build-docs.md`

---

## 0. 结论摘要

**⚠ 首先一条：本仓存在一个 P0 崩溃级缺陷。** `desktop/src-tauri/src/approval.rs:463-495` 的 `extract_windows_abs_paths` 用**字节下标**步进（`i += 1`）却做 `str` 切片（`cmd[i..]`），只要 shell 命令里出现非 ASCII 字符（中文）就会 `panic!("byte index N is not a char boundary")`。**我已用逐字复制该函数的最小程序机械复现**（`rustc 1.98.1`）：`echo 你好` / `git commit -m "修复"` / `type 中文文件.txt` 三种输入全部 PANIC，报文与 B 域审计员报告逐字一致；生产装配路径 `state/mod.rs:313-319`（`.with_modes(...)`）→ `approval.rs:286-289` 对**每次 shell 调用**无条件求值，且既有测试全用 ASCII 路径，所以门禁全绿也抓不到。详见 §1 **P0**。

除这一条外，这份代码库的**卫生状况远好于同规模项目**：零 `TODO/FIXME/HACK`、零 `dbg!/todo!/unimplemented!`、零注释掉的代码块、零孤儿 Vue 组件、零孤儿 Rust 模块、**85 个 Tauri 命令与前端 invoke 完全双向闭合**、zh/en 语言包 949 = 949 完全对齐、30+7+10 个依赖逐个取证全部真用。
其余问题集中在三类：**① 退役功能的残留（跨插件/内核/宿主/前端/i18n 多处）**、**② 存在但从未接线的机制（中间件、配置、ABI 设施）**、**③ 门禁体系的空转与覆盖缺口（CI 不管 desktop 与前端；两处「行数门禁」实际永不失败）**。

---

## 1. 高价值清单（建议直接排期处理）

### P0【崩溃级缺陷 · 已机械复现】任意含中文的 shell 命令都会 panic

> **落地注记（v2.145 提交时已修）**：`rest_starts_unc` 改字节判定、扫描步进改整字符步进（i 恒为 char boundary），`tests_permission.rs::scanner_extracts_and_judges_paths` 补 5 组 CJK 回归（纯中文零 token + 中文路径/UNC token 照常提取）。以下 H1/H2/H3 未动，待裁定。

- 位置：`desktop/src-tauri/src/approval.rs:463-495`（`extract_windows_abs_paths`）、`:493-495`（`rest_starts_unc`）。
- 缺陷代码（原文）：
  ```rust
  let bytes = cmd.as_bytes();
  let mut i = 0usize;
  while i < cmd.len() {
      let is_abs = rest_starts_unc(cmd, i)          // :474 每轮无条件求值（|| 短路左侧）
          || (i + 2 < cmd.len() && bytes[i].is_ascii_alphabetic() && ...);
      ...
      } else { i += 1; }                            // :487 按字节步进
  }
  fn rest_starts_unc(cmd: &str, i: usize) -> bool { cmd[i..].starts_with("\\\\") }   // :493-495
  ```
- 机理：`i` 逐字节自增，走到多字节 UTF-8 字符的**续字节**位置时 `cmd[i..]` 不是 char boundary → panic。
- **Lead 复现证据**（`rustc -O`，逐字复制上述两函数后运行）：
  | 输入 | 结果 |
  |---|---|
  | `node --version` | OK → `[]` |
  | `git status` | OK → `[]` |
  | `echo 你好` | **PANIC** `start byte index 6 is not a char boundary; it is inside '你' (bytes 5..8 of string)` |
  | `git commit -m "修复"` | **PANIC** `start byte index 16 ... inside '修'` |
  | `type 中文文件.txt` | **PANIC** `start byte index 6 ... inside '中'` |
- **可达性（Lead 复核）**：`desktop/src-tauri/src/state/mod.rs:313-319` 生产装配 `ApprovalHub::with_timeout(...).with_modes(...).with_workspace(...).with_rules(...)` → `approval.rs:282-291` 在 `modes.is_some()` 时对**每次工具决议**计算 `external = command_touches_outside(...)` → `:442` 对非 `shell` 工具直接返回 false，**对 `shell` 则 `:449` 无条件调用 `extract_windows_abs_paths(cmd)`**。即：中文语料项目里 `git commit -m "中文"`、中文路径等日常命令每次都会触发。
- 为什么门禁抓不到：现有测试（如 `tests_permission.rs:270`）全部使用 ASCII 路径。
- 建议修法（最小改动，ASCII 输入行为逐字节不变）：`rest_starts_unc` 改字节判定 `cmd.as_bytes()[i..].starts_with(b"\\\\")`，并把 `i += 1` 换成 `i += cmd[i..].chars().next().map_or(1, char::len_utf8)`；或整体改用 `char_indices()` 步进。**并补一条含 CJK 的回归测试。**
- 风险：无（纯 bug 修复）。

### H1【门禁空转 + 覆盖缺口】CI 与 pre-commit 的实际拦截力接近于零 —— 本次最值得动手的一项

证据：
- `.github/workflows/ci.yml:17-24` 只跑根 workspace：`cargo fmt --all --check`、`cargo clippy --workspace --all-targets`、`cargo test --workspace`、`bash scripts/check_length.sh`。
- `desktop/src-tauri` 被**排除在根 workspace 之外**（AGENTS 明载），因此 **CI 完全不编译、不 lint、不测试桌面端**；前端（`vue-tsc` / `pnpm build` / `stylelint`）也不在 CI 内。项目自己把 desktop fmt/clippy/test 与 vue-tsc/build 当门禁，但这些门禁**只存在于人工纪律里**。
- `scripts/check_length.sh:26` 明写 `exit 0`（报告式），而 `.github/workflows/ci.yml:23` 的步骤名却是「File length limit (400)」，`.githooks/pre-commit:8` 更写 `bash scripts/check_length.sh || exit 1`——**两处都指望它拦截，而它永不返回非零**，即「行数门禁」在两个地方都是空转。

建议（按性价比排序）：
1. CI 增三步：`cargo fmt --manifest-path desktop/src-tauri/Cargo.toml --check`、`cargo clippy --manifest-path desktop/src-tauri/Cargo.toml --all-targets -- -D warnings`、`cargo test --manifest-path desktop/src-tauri/Cargo.toml`（Windows runner 更贴合实际：pdfium.dll / WebView2 均为 Windows 资产）。
2. CI 增前端一步：`pnpm -C desktop run build`（内含 `vue-tsc --noEmit`）+ `pnpm -C desktop run lint:style`。
3. 把「行数」改成**基线比较**而非纯报告：脚本输出超限计数并打印清单，CI 步骤读该计数与基线（当前实测 28，项目自报 32）比较，超过即红；`.githooks/pre-commit` 同理。若坚持「三问靠人」，至少把步骤名改成「File length report」以免误导。

### H2【存在但从未接线的内核中间件 ×2】`CtxCompact` 与 `Retry`

证据（`_parts/0-mechanical.md` M2/M3）：
- `crates/kernel-agent/src/middleware/compact.rs`（306 行）+ 其测试（380 + 92 行）≈ **778 行**，构造点 `CtxCompact::new` 全仓只出现在测试里；桌面装配 `desktop/src-tauri/src/state/mod.rs:301-308` 只有 `Tracing/Audit/Timeout/CircuitBreaker`；`crates/kernel-bin` 全 crate 零命中。v2.137 早已把桌面压缩链整面退役。
- `crates/kernel-agent/src/middleware/retry.rs`（46 行）同样只在 `middleware/tests.rs:171,179` 出现；而 `chain.rs:3` 与 `middleware/mod.rs:4-5` 的注释仍宣称装配顺序含 `Retry`，**文档与实现不符**（实现里也没有 `Approval` 中间件，审批走 InnerApproval 通道）。
- 反例保护：`CompactionOutcome` 仍是活类型（`waterfall.rs:56`、`kernel/request.rs` 返回位），且旧会话日志重放链路可达（`session-store` 折叠 → `desktop/runner.rs:416-424` → 前端 `chat.ts:667` / `notifications.rs:240`）。**因此不能整簇删除**，只能摘掉未接线的中间件本体。

建议（需用户裁定、三选一）：① 删 `CtxCompact`/`CompactConfig`/`CompactStrategy`/`suggested_threshold` + 专属测试，保留 `CompactionOutcome` 与重放面（推荐，净减 ~700 行）；② 确有 headless 复用计划则收敛到 `#[cfg(feature = "compact")]` 并记档；③ 明确记档「保留待用」。无论哪种，`chain.rs`/`mod.rs` 的装配注释必须与实际对齐。

### H3【退役残留：前端与 i18n】`sandbox_run`（v2.145）与 59 个孤儿文案

证据：
- `sandbox_run` 前端 6 处：`desktop/src/lib/tool-summary.ts:14,125`、`tool-detail.ts:62,123,332,526`、`side-effect.ts:43-44`、`locales/zh-CN.ts:330 / en.ts:328`。插件已于 v2.145 整目录退役；**新会话不可达，但历史会话日志重放仍可能渲染该工具行**——属「历史渲染面」，删或留取决于产品口径（本报告不替用户拍板）。
- **59 个确证孤儿 i18n 键**（两语言共 118 条死文案；其中 9 条的叶名在代码里以裸字符串出现过，建议人工 30 秒复核）。完整键名与判定方法见 `_parts/1-i18n-orphans.md`，逐条退役依据见 `_parts/C-frontend.md` 的 C3/C4 表。分布与历次 UI 退役批次高度吻合：`settings.*` 23 个（provider 面板重构后遗留）、`approval.*` 8 个、`subagent.*` 8 个（v4.13 `SubagentCard.vue` 大卡片退役遗留）、`developer.stabilityRounds*` 2 个（v2.137 ctx-compact 稳定期退役遗留）。
- **反例撤回 ×2**：① 首轮机械比对曾报「zh 有 18 键 en 无」，经复核是解析器不认 prettier 折行（`key:` 后换行取值）造成的假阳性；修正后 **zh 949 = en 949，双向差集为 0**，翻译完整性无问题。② C 域台账把 `app.modelFallback` 计入孤儿，Lead grep 证实它仍在 `desktop/src/components/chat/Composer.vue:99` 使用，**应为剔除**——本报告采用 Lead 严格复算的 59 条为准。

建议：① 删 59 个确证键（先复核 9 条叶名歧义项）；② 新增 `scripts/check-i18n.mjs`（键集合双向相等 + 报告无引用键），挂进 pre-commit——这正是本次审计发现的**可持续防漂移机制**（历次「删组件不删文案」都源于缺这道门）。

### H3b【用户可见缺陷：4 个 i18n 键被引用但两语言包都没有】（除 P0 外的另一类现网可见问题）

Lead 独立复算（把源码中所有 `t()/$t()/te()/tm()/tr()` 的字面量键与语言包键集合做双向差集）：

| 缺失键 | 引用点 | 用户看到什么 |
|---|---|---|
| `settings.proxySavedToast` | `desktop/src/components/settings/LlmSection.vue:296` | 保存代理后 toast 直显 `settings.proxySavedToast` |
| `developer.modeMemoryManager` | `desktop/src/components/settings/MemoryTreePanel.vue:103` | 记忆树模式列直显键名 |
| `developer.modeRule` | 同上 | 同上 |
| `common.close` | `desktop/src/components/settings/model/ImportDialog.vue:110` | 导入弹窗关闭钮的无障碍名直显键名（`common` 块只有 back/cancel/refresh） |

另有一条更轻的：`desktop/src/lib/settings-index.ts:136` 的 `searchKeys` 里写了 `settings.panelTitle`，而实际键是 `developer.panelTitle`（`zh-CN.ts:545`）——只影响 ⌘K 设置搜索命中。**建议补 4 条文案（纯新增，零风险）；补完跑一次 `vue-tsc` 与中/英界面目检。**

### H4【死配置 + 文档虚述】`config/profile.toml`

证据：`config/profile.toml` 是 7 行、**全部被注释**的占位文件（无任何生效键），文件头却写「内核启动时按此 profile 加载插件；桌面端设置面板读写此文件」；`crates/kernel-bin/src/main.rs:7` 声称「阶段七补：profile.toml 加载…」；README 与 `DESIGN.md:1256,1695` 同样按「已实现」描述。全仓 Rust 代码中**没有任何读取 `profile.toml` 的路径**（对 `profile` 的全部命中都属于 LLM provider profile，与此无关）。

建议：三选一并保持一致——① 删文件 + 从 README/DESIGN 移除该路径描述；② 保留文件但把文档改成「规划中」；③ 真正实现加载器。**当前状态是「文档说已接线、代码零接线」，最容易误导后来的 agent/新人。**

### H5【真实但不致命的 API 哑面】`plugin-api::CallCtx` 取消设施

证据：`crates/plugin-api/src/tool.rs:180`（`cancelled: Arc<AtomicBool>`）、`:193 is_cancelled()`、`:197 cancel()`——全仓零调用点（唯一的 `cancel` 字面命中是 `scheduler/tools.rs:51` 的工具描述文案）。宿主从不置位，插件轮询恒 `false`，是**对外 ABI 上的一个永远无效果的承诺**。

建议：要么在宿主侧真接上（超时/用户停止时 `cancel()`，插件循环可感知），要么删除三者并在文档改为「取消由 `deadline` 承担」。风险：`CallCtx` 对插件公开，删除属 ABI 收窄，需按项目 ABI 纪律评估。

### H6【未使用依赖 3 处】删掉即可，零风险

| crate | 依赖 | 证据 |
|---|---|---|
| `crates/cordis-core` | `plugin-api` | `Cargo.toml:9`，crate 内 0 次出现 |
| `crates/kernel-bin` | `plugin-service` | `Cargo.toml:12`，crate 内 0 次出现 |
| `crates/memory-tree` | `tokio`（dev-dep） | `Cargo.toml:15`，crate 内 0 次出现（测试全同步，无 `#[tokio::test]`） |

判据：`cargo metadata --no-deps` 逐依赖 × 逐 crate 全源码文本比对；无 `extern crate`（全仓 0 处）。附带发现：`crates/memory-tree` 的 dev-dep `plugin-api`/`semver` **确有测试使用**（`material.rs:156-189`、`manager/tests.rs`），未误报。
**⚠ 一处例外（A 域反向证据）**：`crates/cordis-core → plugin-api` 这行**是 `docs/design/DESIGN.md:1266` 依赖图明写要求的**（虽然代码零使用）——删它必须同步改 `DESIGN.md:1264-1279`（§8），否则文档与清单反向不符；另两行（`kernel-bin/plugin-service`、`memory-tree/tokio`）可直接删。

### H8【UI 展示的能力声明与代码相反（真实缺陷，用户可见）】

Lead 已验证的原文对照：

| 位置 | 声明 | 事实 |
|---|---|---|
| `plugins-src/tool-fetch/manifest.json:34` | `"side_effect": "Pure"` | `plugins-src/tool-fetch/src/lib.rs:86` = `SideEffect::Irreversible`；`docs/design/plugins.md:41` 早已裁定「原 Pure 作废」 |
| `plugins-src/tool-shell/manifest.json:12-13,24` | 「其余白名单命令需审批；硬锁解释器/网络类」「unless the host unlock switch is on」 | v2.145 后 `tool-shell/src/lib.rs:90-101` 是「任何单条简单命令都允许、进审批卡；无命令级禁令」，白名单/denylist/unlockRisky 全部退役 |

消费链（决定了它不是「文档问题」而是用户可见问题）：`build_plugins.sh:45-46` 把源 manifest 原样出版本号拷进暂存 → `crates/plugin-service/src/display.rs:46,63,69` 读**暂存目录的 manifest** 作为展示元数据真源 → 前端 `PluginCard.vue:62` / `plugins/InstalledTab.vue:246` 渲染 `side_effects` 徽章与简介。**结果：tool-fetch 在插件卡上显示「纯」而实际每次调用都要审批；tool-shell 的简介还在讲已退役的硬锁。**
建议：改这两份 manifest 的 `side_effect` 与 `description` 与代码同步；根治见 §7.4 的「manifest 由 DLL describe 生成/校验」脚本（v2.135 曾在同一文件记过「manifest 只列一个工具」的同类欠账，说明单靠人工同步会复发）。

### H9【权威设计文档与现状相反（文档腐烂，高）】

- `docs/design/DESIGN.md`（2779 行）**通篇 0 处 `v2.145"`（Lead 已 grep 验证，mtime 2026-10-04）**：`:1152` 决策表仍写「`shell` / `sandbox_run` 命令文本扫出…强制弹卡」、`:2382` 仍写「内置白名单 `ls/cat/grep/git status`，`LOCOWRK_SHELL_ALLOW`…denylist 约 50 项永远赢」。而 AGENTS.md 明写「修改任何核心模块前先读 DESIGN.md」——**照文档实现会写出一套与代码相反的策略层**（决策表还缺 v2.145 新增的 ③项目 deny / ⑤项目 allow 两序）。
- `plugins/README.md:22-33` 整表失真：称「官方内置八件」（实为 10 个 crate，缺 `tool-notify`/`tool-shout`）、工具名带点号（`fs.read`/`memory.put`，v2.20 起已全改下划线）、版本写 1.0.0–1.3.0（v2.56 已重置到 0.1.x）、`fetch` 标 Pure、`shell` 标「白名单 + denylist」。
- `docs/design/UI shell and patterns.md:193,202,883,999` 的「现状表」仍列 v2.136/v2.137 已退役的 `memory.handoffUpdate`/`autoExtract`/`experiences`/`defaultOrganizer`/`context.stabilityRounds` 与 `session_set_organizer` 命令——**与同文件 v4.27/v4.30 版本条的退役宣告自相矛盾**。
建议：按 `docs/上下文与记忆工作流程报告.md` 头部的「⚠ v2.13x 修订」格式给这些段落逐条加退役注（该项目已有此惯例，是正面样本）；DESIGN.md 决策表补 ③⑤ 两序并删 `sandbox_run`。



---

## 2. 中价值清单（值得排期）

### M-1 重复实现三处（样板复制）
- `crates/plugin-host/tests/` 五个集成测试各持一份 `build_and_stage` / `workspace_dir` / `build_lock` / `artifact_name` / `load_activate`——项目已有 `crates/kernel-agent/tests/common/mod.rs` 先例，照搬即可（纯测试面、零生产风险）。
- 10 个插件的 `build.rs` **逐字节相同**（299 B，SHA256 全等），可用 workspace 级共享收敛。
- 插件 C ABI 样板（`zh_en` / `make_state` / `describe_manifest` / `log` / `arg_u64`）在每个插件里重复一遍；`kernel-bin/src/scaffold.rs` 的模板复制属设计，但已落成的 10 个插件可下沉公共助手（`plugin_api::fsutil` 先例）。

### M-2 三个「无消费者」的 pub 项
`crates/memory-tree/src/manager.rs:274 count_user_messages`（纯转发，生产面直调 `material::user_count`）、`crates/cordis-core/src/runtime.rs:52 deps_of`（自称「调试用」，零调用）、`crates/plugin-api/src/tool.rs:193/197`（见 H5）。判据：全仓标识符出现次数 == 定义处数。

### M-3 `#[allow]` 掩盖面 3 处（已逐条读，均非大问题）
`desktop/src-tauri/src/state/app_state.rs:25`（`app: AppHandle` 字段注释自承「暂无读取方」）；`crates/session-store/src/session.rs:411`（`TurnId/TurnOutcome` re-export 无消费者）；`desktop/src-tauri/src/plugins_root.rs:17`（`dll_name/find_artifact` re-export 被 `tests_plugins_root.rs` 使用，保留有理由）。

### M-4 规模门禁的覆盖缺口
`scripts/check_length.sh` 只扫 `crates` + `desktop` 的 `*.rs|*.vue`：**`plugins-src/` 不在覆盖面内**（`plugins-src/memory-sqlite/src/store.rs` 533 行、`tests.rs` 557 行——含空行口径——从未出现在门禁输出里）。Lead 按同规则重算 crates+desktop 超 400 行者 **32 个**（与项目 v2.145 记的「32」吻合；⚠ 首轮用 `Measure-Object -Line` 得 28，该方法不计空行会系统性少算，已自纠）。最大者（含空行口径）：`desktop/src-tauri/src/organize/tests.rs` 867、`crates/kernel-agent/src/kernel/tests.rs` 853、`desktop/src/components/chat/Composer.vue` 832、`desktop/src-tauri/src/llm/tests.rs` 786、`desktop/src-tauri/src/state/mod.rs` 698、`desktop/src-tauri/src/settings/llm.rs` 682。

### M-5 构建产物缺陷（未入库，但会传染安装态）
`plugins/*/<ver>/assets/assets/icon.svg` 双层嵌套目录（10 个插件均中招，内容与 `assets/icon.svg` SHA256 全等）：staging 脚本把 `assets/` 复制进了目标 `assets/`。建议核对 `plugin-service` 的图标路径约定并修 `scripts/build_plugins.sh`。

### M-6 文档体量与腐烂
- README 仅 21 行且状态停滞在「**阶段一**（内核骨架 + 事件日志）」，与 v2.145 现状（完整桌面端 + 权限模型重设计）严重脱节；结构清单也缺 `market/`、`plugins-src/`、`scripts/`、`docs/`。
- 大体量文档：`docs/design/CHANGELOG.md` 394 KB、`UI shell and patterns.md` 245 KB、`ROADMAP.md` 207 KB、`AGENTS.md` 191 KB（后者每次会话都会被注入，且当前已被 65536 字节预算截断——**代价是每个 agent 每次对话都要为历史付 token**）。建议把「历史进度流水」按版本归档到 `docs/design/CHANGELOG.md` / 独立归档文件，`AGENTS.md` 只保留「当前状态 + 现行纪律 + 指针」。

---

### M-7 五份 manifest 的 schema 未同步 G3/G2 语义（当前无消费方，属元数据不实）
`tool-fs`（`manifest.json:63-75,77-87`）、`tool-edit`（`:27-33,42-48,57-61`）共 5 个工具缺 v2.133 G3 的必填 `expected_revision`（manifest 内 0 命中 vs 代码 19 处）；`memory-sqlite` 的 `memory_write` 缺 v2.68 的 `scope`。运行时 schema 来自 DLL describe（`plugin-host/src/loader.rs:40-41`），`display.rs` 与市场只读 id/version → **不改变模型可见面**，但会让市场/盘点类消费者读到过期契约。建议随下次插件版本 bump 补齐，或并入 H8 的「manifest 由 DLL 生成/校验」方案。

### M-8 `AGENTS.md` / `CHANGELOG.md` 的结构性腐烂（比"体量大"更具体）
- `AGENTS.md:3-38`（项目概览）占全文 **94%**（99,965 / 106,546 字符）；`:8` 是当前进度 v2.145，**`:37` 又有一个标着「当前进度」的 v2.55 巨块（单行 27,663 字符）**；版本链 `:29` v2.125 → `:30` 直接跳 **v2.75**（v2.76–v2.124 共 49 个版本整段缺失）→ 按标签检索的人会被误导。
- `docs/design/CHANGELOG.md` 394,866 字节 / 109 行（每版一整行），`:81` 与 `:86` **两条互不相关的批共用 v2.75**（撞号）→ 「按版本号追溯」这一唯一检索手段失效；`:177` 还有行内短语重复。
- 建议见 §7.6：AGENTS 只留「当前 + 最近 2–3 版」，历史迁入 `docs/design/CHANGELOG/` 目录 + 索引表，并修撞号。

### M-9 四处小死代码 / 死规则（各自都"看起来在工作"）
| 位置 | 问题 |
|---|---|
| `scripts/check-style.mjs:24-28,75,81` | 四条 RULES 全为 `"error"` → `warns` 恒空 → `:81` 的二级报告分支**不可达**（历史迁移期遗留机架） |
| `.githooks/pre-commit:8` | `bash scripts/check_length.sh \|\| exit 1`：脚本恒 `exit 0` → 该分支**永不触发**（且注释仍称"行数上限检查"） |
| `.gitignore:26-28` | `plugins/**/*.dll/.so/.dylib` 三条被 `:37 plugins/*` 整体覆盖 → **永不生效**（`git check-ignore -v` 实测命中第 37 行） |
| `.zcodeignore` 同步区 | 缺 `.gitignore:40-41` 的 `docs/issue/` → 该目录（13 张 png + 1 份 md 的私有素材）对 ZCode 可见 |
建议：前三处能删就删（保留机架者加注释说明），第四处补一行同步。

### M-10 插件 `tool-shell` 的两条拒绝报文被合并，违背「错误信息要教模型下一步」
`plugins-src/tool-shell/src/policy.rs:131-133`（引号未闭合 → `return None`）与 `:89-93`（`None` → `"shell command is empty"`）：**输入 `git status`（缺右引号）会得到「命令为空」**，模型会去查空串而不是补引号。v2.145 文档把 Deny 面定义为三项（元字符/空命令/引号未闭合），实现却只有两条报文。建议 `split_command` 返回 `Empty` / `UnclosedQuote` 两种错误。

### M-11 零引用孤儿设计稿（575 行）
`docs/design/Sandbox Execution Design.md`（CubeSandbox 集成方案，6 个从未实现的 `sandbox_*` 工具）：**全仓 `git grep` 零命中**（Lead 已验证，文件确实入库）。主题已随 v2.145 退役，且实际交付走的是「Windows 受限令牌 + JobObject」而非本文方案。建议移入 `docs/design/archive/` + 首行加「未采纳 + 已退役」两行，并给 `docs/design/` 加一份 `INDEX.md`（权威 / 存档 / 已退役三类），避免再次出现无人指向的稿件。

### M-12 版本注释三处过期（第三次同源过期）
`scripts/build_plugins.sh:146` 的收尾文案（一行里 `0.1.5` 同时指 memory-sqlite 与 tool-shell、`0.2.1/0.1.5 = G3 批` 无主语、漏列 `tool-mcp`/`tool-notify`）+ 9 个 `plugins-src/*/Cargo.toml` 的「按 1.0.0 构建」注释（v2.56 版本基准重置后未改，`tool-echo` 写 1.2.0/1.3.0，现实 0.1.0/0.2.0）。建议注释统一改成「版本见 `scripts/build_plugins.sh` 的 stage 列表」，把版本真源收敛到一处。

---

### M-13 两个「零构造、零匹配」的日志事件变体（编译器看不见，Lead 独立发现）
`crates/session-store/src/event.rs:140 EffectApplied`、`:144 PluginActivated`：全仓 `EffectApplied|PluginActivated` 仅 4 处命中 = **枚举定义 2 处 + `docs/design/DESIGN.md:476-477` 的文档片段 2 处**——没有任何构造点、没有 match 分支、前端零引用（同类变体 `ManualIntervention` 有 12 处引用、`ContextCompacted` 走重放链路，均属活代码）。
判据：`SessionEvent::<Variant>` 全仓 occurrence = 0 且裸名 occurrence = 1（仅定义行）。
影响与建议：`SessionEvent` 是 **pub 日志格式枚举**，编译器对「pub 变体从未构造」不告警，故 clippy 全绿也发现不了。这两个变体是早期设计（§4.1「事件全集」）的化石；**若移除**，需先确认历史 jsonl 中不存在该类型（当前代码从未写入 → 风险低），或按项目口径「保留为全集完整性」加一行注释说明。同一检查顺带确认其余 14 个变体都活着。

---

### M-14 重复实现：小工具函数与常量（B 域，全在 desktop crate）
- `now_ms()` **7 处**（`media/mod.rs:289`、`llm/catalog.rs:318`、`llm/diagnose.rs:206`、`organize/mod.rs:112`、`runner.rs:231`、`scheduler/store.rs:65`、`organize/tree_tools.rs:280`）+ `now_secs()` **2 处**（`memory.rs:515`、`memory_reconcile.rs:242`），其中 5 处逐字节相同，而 `scheduler/store.rs:65` 早就是 `pub(crate)` 共享版 → 建 `src/timeutil.rs` 收敛。
- `clip()` **6 处 / 4 种语义**（`export.rs:12-18`、`browser/console.rs:46-48`、`notifications.rs:80-82`、`organize/search_titles.rs:235-241`、`subagent_verify.rs:130-136`、`scheduler/tools.rs:296-298`）→ 收敛 `src/text.rs` 两个函数；**`export.rs` 的输出对用户可见（导出的 Markdown 字节），合并时必须保持逐字节不变**。
- Ollama 端点常量 **3 份**（`llm/models.rs:21`、`llm/probe.rs:10`、`settings/embedding.rs:13`）→ 留一份。

### M-15 「仅为测试保活」的推测性 API ×2 + 测试专用再导出
- `desktop/src-tauri/src/settings/embedding.rs:40-49 is_ready()`：`#[cfg_attr(not(test), allow(dead_code))]`，注释自陈「留给…未来的 UI 就绪提示」，生产零消费者（`from_settings` 已覆盖裁决）→ 建议删。
- `desktop/src-tauri/src/memory.rs:269-273 write()`：注释自陈「当前只服务测试播种」→ 降为 `#[cfg(test)]`，让「生产写路径唯一 = 插件 `memory_write`」显式化。
- `desktop/src-tauri/src/plugins_root.rs:15-18`：为测试写法给生产模块挂 `#[allow(unused_imports)]` → 改 `#[cfg(test)] pub use`，或让测试直接 `use plugin_service::builtin::dll_name`。

### M-16 面向模型的报错把 Windows 路径示例写死（跨平台）
`desktop/src-tauri/src/browser/tools.rs:119-123` 回给模型的错误文本 `"path must be absolute (e.g. F:\\dir\\page.html)"`——该 crate 明确支持三平台（`Cargo.toml:80-85` 的 keyring 三 target），非 Windows 构建下示例是错的且会被模型照抄（同文件 `:186` 的测试也断言 `F:\a\b.html`）。建议平台中立表述或按 `cfg!(windows)` 分支。

### M-17 陈旧抑制属性与注释腐烂
- `desktop/src-tauri/src/state/app_state.rs:23-26`：`#[allow(dead_code)]` + 「字段暂无读取方」——**Lead 已验证 `commands.rs:110`（自动标题）与 `scheduler/engine.rs:218` 都在读 `app`** → 抑制属性与注释都已过期，删掉让编译器以后自己报警。
- `state/app_state.rs:77` 提 `compact_cfg`（v2.137 已退役）；`scheduler/mod.rs:9`、`scheduler/store.rs:2` 以「experiences 先例」措辞引用已删模块（v2.136）。

### M-18 热路径优化（均有明确调用点与低风险修法）
| # | 问题 | 位置 | 建议 |
|---|---|---|---|
| a | 单次审批决策解析会话上下文 **~4 次**（每次 1 次旁车 `<sid>.meta.json` 读盘 + N 次 `canonicalize` 系统调用），对**每个工具调用**发生 | `approval.rs:288,298,340,363` → `workspace.rs:52-57` | `resolve` 开头解析一次并复用（函数内 memo；**不要**跨调用缓存——`workspace.rs:39-46` 明载「保存即生效」是有意设计） |
| b | 同一份项目规则 JSON **读盘 + 反序列化 2 遍**（deny 一半、allow 一半） | `approval.rs:299,341` → `permission_rules.rs:107-122` | 新增 `verdicts()` 一次读、两判定；文件仍每次直读（撤销出口语义不变） |
| c | 每轮 **2 次** `LlmConfig` 全量克隆（含各档明文 Key 串） | `commands.rs:83-87`、`organize/mod.rs:355-358` → `settings.rs:210-212` | 增 `with_llm(\|cfg\| ...)` 闭包访问器；顺带合并 `commands.rs:82/84/101` 的三次独立取锁 |
| d | `turn_id` **每轮重扫全量日志** → 全会话 O(n²) | `runner.rs:73-80` | `Session` 维护 `turn_count`（跨域：`crates/session-store`）；折衷 = `rev().find_map` |
| e | `has_call_event` **无条件**构建 `HashSet<String>`（N 次克隆），而它只在崩溃恢复场景被查询 | `runner.rs:278-284` | 改懒建；另两处 `format!` 前缀判断改 `strip_prefix`（`approval.rs:541`、`permission_rules.rs:61`） |
| f | `session_history` 在 **per-session 锁内 + async 线程上**做 O(n) 深克隆投影；`session_search`/`export`/`asset_gc` 等同步 IO 直接跑在 async 命令里 | `commands.rs:167-183`、`commands/sessions.rs:228`、`commands/assets.rs:41` 等 | 锁内只取快照、锁外投影；重 IO 命令包 `spawn_blocking` |
| g | `append_slice` **每条事件一次 open/write/close**（每轮 3N–6N 次） | `session_log.rs:63-74` | 单句柄批量追加（可并入 session-store 的批式 API） |

### M-19 结构性问题：`init_state` 单函数 636 行 + 巨型测试文件
- **最严重**：`desktop/src-tauri/src/state/mod.rs:41-676`——`init_state` 单函数 **636 行**，串起数据根/设置/keyring 迁移/资产库/记忆库/会话恢复/插件服务/内核与中间件链/权限模式库/项目规则库/审批中心/调度/托盘/通知/浏览器控制台/EWS 记录器…（三问全否）。建议按子系统抽私有构造器，`init_state` 退化为 ~80 行编排；**注意字段间存在构造顺序隐性契约**（sink 先于 approval、catalog 先于 session_window），重构后以启动日志 + desktop 356 测试为回归判据。
- `desktop/src-tauri/src/organize/mod.rs` 621 行 / 19 顶层函数（触发策略 + 投影 IO + 泄压 + 通知 + 147 行异步管线混装）→ 拆 `trigger.rs`/`collapse.rs`/`task.rs`。
- `desktop/src-tauri/src/runner.rs` 592 行，其中 `project_events` 单函数 **237 行**（20 变体巨型 match）→ 拆 `runner/projection.rs`。
- `desktop/src-tauri/src/settings/llm.rs` 682 行 / 17 函数（头校验 + 目录合成视图 + 子代理钉档 + 档位持久态 + 更新 DTO 四事）→ 拆 `settings/llm/{view,model}.rs`，保持 `settings.rs` 的 14 条 `pub use` 不变。
- 测试文件：`organize/tests.rs` 867 / 31 函数（6 主题）、`llm/tests.rs` 786 / 22、`tests_approval.rs` 640 / 20 → 套用仓库已有先例（`tests_session_log/`、`organize/assemble/tests.rs`）改目录式拆分，共享脚手架（`temp_root`/`settings_with`/`session`/`good_reply`）上移 `mod.rs`。

### M-20 A 域 · 两个「零生产接线」的子系统（Lead 已复核）
- **session-store 磁盘快照 + upcaster 演进链**（`crates/session-store/src/snapshot.rs` 111 行 + `snapshot/tests.rs` 122 行）：`Snapshot`/`Upcaster`/`V1ToV2Upcaster`/`Upcasters`/`snapshot_at`/`from_snapshot` 的**全部**引用只落在定义文件、自家测试与 `lib.rs:29` re-export（Lead grep 复核：`Snapshot|Upcaster` 在 `crates/session-store` 之外零命中；其余 `*Snapshot` 命中属无关的 `ExposureSnapshot`/`CatalogSnapshot`）。恢复路径 `persist.rs:80-98 load_session` 是逐行 JSONL 回放，从不使用它。**⚠ 勿连带删**内存态 `Session.snapshots`（`session.rs:285,308-310,351-362`，那是活的另一套）。**⚠ 删除有归档代价**：`CHANGELOG.md:28`（v2.130）与 `ROADMAP.md:872` 把「快照 schema 1→2 + V1ToV2Upcaster」记为**已交付里程碑**，删=撤回一条归档记录——建议按 A 域 V2 的判据先读 `DESIGN.md:1360-1380`（§9.5）定「未接线缺陷 vs 预留」，再定「接线 / 标注 RESERVED / 删除」。
- **cordis-core `serial` 监听器机制**（`event_bus.rs:27,71,88,113,198` ≈ 45 行 + 测试）：`on_serial`/`serial(`/`SerialListener` 的命中**全部**在 `event_bus/tests.rs` 与 `lib.rs:27` re-export（Lead 复核，共 15 处无一生产点）；`serials` map 生产中恒空。其注释自述用途（顺序通知/可短路）已被宿主侧中间件取代（`kernel-agent/src/middleware/audit.rs:5` 明载 mw-audit 以宿主侧中间件落地）。建议删整簇或标注 RESERVED 并同步 DESIGN §3.5 表。

### M-21 A 域 · 哑 ABI：`kv_put` 回 OK 但数据永不可读 + 幻影错误码
- `crates/plugin-host/src/host_vtable.rs:21-23`（`static KV`）、`:75-93`（`host_kv_put` 真写入并返回 `code::OK`）、`:65-73`（`host_kv_get` 是**常量 `ERR_UNSUPPORTED` stub**）：`KV` 只有写入点、没有任何读取点 → 插件调 `kv_put` 拿到成功回执却永远读不回。Lead 复核 `kv_put|kv_get|static KV` 命中：定义 + `host_vtable/tests.rs`（测试名 `kv_put_stores_value` / `kv_get_is_unsupported_until_ownership_protocol_lands`），无生产调用方。**判读**：注释与测试都说明是「待所有权协议」的有意预留，所以不是「忘接线」，而是**语义不诚实的不对称**（同文件 `host_notify` 无桌面环境时如实返回 UNSUPPORTED）。建议让 `host_kv_put` 同样返回 `ERR_UNSUPPORTED`（ABI 签名冻结面不动）。
- `crates/plugin-api/src/ffi.rs:16 ERR_UNKNOWN_TOOL = -2`：全仓 3 处命中（定义 + 两个测试），**无生产者**——真实「工具名不存在」路径统一落 `ERR_CALL_FAILED`（`plugin_side.rs:151`），宿主侧 `instance/native.rs:97-107` 对非 OK 只做泛化映射，既不产也不认 `-2`。建议给 `-2` 补一条人类可读报文（兑现「错误信息要教模型下一步」纪律）或标注 RESERVED；**不宜直接删**（`ffi.rs` 自述冻结契约）。

### M-22 A 域 · 仅自家测试消费的 pub 面（`Runtime` 观测面 + 作用域助手 + 若干）
- `crates/cordis-core/src/runtime.rs:83/88/93/98`（`state_of`/`failure_of`/`provider_of`/`dependents_of`，注释自称「§11.6 UI 投影来源」——该消费方从未落地）；`context.rs:106/159`（`provide_scoped`/`use_service_scoped`，仅 `context/tests.rs` 用；**trait-object 孪生体 `provide_dyn_scoped`/`use_dyn_scoped` 有真实生产消费者，保留**）；`deps_of`（见 M5，连测试都不用）。
- `crates/plugin-service/src/lockfile.rs:154-157 clear_pending_delete`：唯一调用点是自家测试，真实清理在 `service.rs:277-287` 内联 `retain` → 同语义两实现，公开的那个零调用（建议复用或删）。
- `crates/plugin-host/src/registry.rs:210-216 graveyard_len`（注释自陈诊断/测试用）、`crates/kernel-agent/src/kernel.rs:109-113 with_retry_budget`（唯一调用点在 `tests/waterfall.rs:102`，与 M3「Retry 零接线」同源）→ 降 `#[cfg(test)]`/`pub(crate)` 或随 M3 一并裁定。

### M-23 A 域 · 幻影字段与双份校验规则
- `crates/memory-tree/src/trigger.rs:76 TriggerInput.anchor_source`：**只写不读**——生产唯一写入点 `desktop/src-tauri/src/organize/mod.rs:273-275`，而唯一决策函数 `should_auto_organize`（`trigger.rs:89-99`）完全不读它；观测面板另算一份（`organize/debug.rs:52-65`）。建议删字段（`AnchorSource` 枚举仍被 desktop 用，保留）。
- `valid_session_id` **两份且规则已分叉**：`crates/session-store/src/persist.rs:23-29`（允许 `.`）vs `crates/memory-tree/src/storage.rs:15-22`（不允许 `.`、多 64 长度上限），后者注释却自称「同源」→ 含 `.` 的会话 id 会日志落盘成功而**树投影静默失效**。当前桌面 id 形态 `desktop-<毫秒>` 恰好都合法（**陷阱而非现网缺陷**）。建议下沉 `plugin-api`（有 `fsutil` 先例）单点定义 + 单测锁规则。

### M-24 A 域 · 优化（按收益排序）
| # | 问题 | 位置 | 建议 |
|---|---|---|---|
| a | **7 个 request waterfall 每步 `payload.clone()` + 全量 `from_value::<RequestPayload>`**（payload 含整个会话尾巴 messages + 全工具目录 schema），多数随后再 `to_value` 回去 | `base_system.rs:62`、`compact.rs:177`、`fragments.rs:62`、`instruction_files.rs:51`、`memory_inject.rs:263`、`tool_exposure.rs:278`、`tree_context.rs:32` | 这**架空了** `event_bus.rs:31-32` 的设计意图（「Continue/Reject/Retry 都不付克隆」）。最小改动：`tree_context.rs:32`（只为读一个字段）改手工 `payload.get("tree_directory")`；3 个偶发改写者加前置判空后再解析 |
| b | `SessionState::pending_slot` **反向全扫到日志头** → `apply`/`state_at`/`load_session` 长会话 **O(n²)**（`SNAPSHOT_EVERY=64` 只压常数） | `crates/session-store/src/session.rs:141-149`（调用点 `:110,:121`，上层 `:351-362`、`persist.rs:92`） | 命中第一条 assistant 后即停（一行改动恢复 O(1) 均摊），或维护 `open_calls: HashMap<CallId,(usize,usize)>` |
| c | `tool_exposure` 每请求**克隆整个工具目录**（`ToolSpec` 含 schema）并**两次**全目录 JSON 序列化（只为求长度） | `tool_exposure.rs:204,217,309-318`；`spec_tokens` `:154-156` 调用点 `:319,:347` | `spec_tokens` 改逐项累加估算；`select` 返回下标而非克隆 `ToolSpec` |
| d | `search_sessions` 把**每个会话文件读两遍、JSON 解析两遍**（`list_sessions` 已读过一遍），且 `ci_fold` 双重分配 | `crates/session-store/src/persist.rs:232-289`（`list_sessions` `:125-194`；`ci_fold` `:293-301`） | 让搜索复用 `list_sessions` 的同一遍 IO+解析；`ci_fold` 去重分配 |
| e | `crates/session-store/src/session.rs` 415 行 = crates 内**唯一超 400 行的生产文件**，混三职责（事件折叠状态机 / 事件切片→消息投影 / 会话生命周期 + 一条冗余 re-export） | `session.rs`（`apply:80-139` + `project_messages:164-266` + `Session:290-395`） | 抽 ② 为 `session/project.rs`（纯函数），删 `:409-412` 的 `#[allow(unused_imports)]` re-export（`lib.rs:22` 已直接导出 `TurnId/TurnOutcome`）→ 回落到 ~300 行 |

### M-25 A 域 · 注释腐烂 3 处
`crates/session-store/src/persist.rs:166-168` 仍称旁车透出「整理模式（M4）」（`meta.rs:16-17` 明载 v2.137 已退役、字段已无）；`crates/kernel-agent/src/middleware/mod.rs:62-63` 说 `replace_decision` 是「三个 request 监听器共用」，实为 **7 个模块 9 处**；`crates/kernel-bin/src/main.rs:7` 仍称自身用 `PluginService`（全 crate 对 `plugin_service` 零引用 = M1 那条未使用依赖）。

---

## 3. 低价值 / 记录项

- `Cargo.lock` 11 个 crate 多版本共存（`bitflags/hashbrown/syn/thiserror/windows-sys/wasmparser` 等），基本来自传递依赖；**不建议**为清版本强行 `cargo update`（收益低、回归面大）。
- 入库大二进制：`pdfium.dll` 7.5 MB、`resources/models.dev/api.json` 5.3 MB（均为 v2.127/v2.131 有意随包，记档）。
- `docs/design/ui-prototype/`（index.html 92 KB + css/js 6 件，2026-10-02 原型）仍被设计文档引用 → 不建议删，但可标注「历史原型」。
- 本地（未入库）垃圾见 §5。

---

## 4. 方法学与「已排除的假阳性」（重要）

本报告执行「宁缺勿假」：以下都是机械扫描**先报后撤**的假阳性，特此记录，避免后续复核者重走弯路。

| 首轮机械结论 | 复核结果 | 撤销原因 |
|---|---|---|
| `plugins-src/tool-mcp/examples/fake_mcp_server.rs` 是孤儿模块 | 非问题 | cargo `examples/` 是入口，无需 `mod` 声明 |
| `` `subagent_transcript` `` 前端调用但后端未注册（疑似硬 bug） | **契约完全闭合** | 我的解析正则要求「标识符后跟逗号」，而注册表最后一项无逗号；实际 `lib.rs:248` 已注册。修正后 **85 个 `#[tauri::command]` = 85 个注册 = 前端全部引用**，双向零差集 |
| `ContextCompacted` 链路（Rust 事件 + 前端 store/通知/i18n）是死功能 | 仍有可达路径 | 旧会话日志重放仍会产出该事件（`runner.rs:416-424`），属历史渲染面，不是死代码 |
| zh 语言包 18 个键在 en 缺失 | 假阳性 | 解析器不认 prettier 折行；修正后两语言 949 = 949 |
| 136 个 i18n 键「无引用」 | 大部分假阳性 | 15 个动态前缀（`tool.type.${...}`、`memory.time.${...}` 等）；排除后按严格判据收敛为 **59 个确证孤儿**（另有 9 条叶名歧义项标注待人工确认） |
| `Retry` 中间件「无引用」（首轮 pub 扫描未标） | 确认为真 | 首轮扫描的「出现次数」被 `chain.rs` 注释里的 `Retry` 字样抬高，需按构造点判定——与本报告 M2/M3 结论一致 |

**环境限制（影响本次能给出的证据强度，如实记录）**：
1. 本会话 DSH 沙箱对工作区只授予**根目录**写权限，子目录（`target/`、`docs/`、`tmp/`）写操作被拒（`Access denied`）→ **`cargo clippy/test/fmt` 与 `bash scripts/*.sh` 无法在本会话复跑**（cargo 首先就写不了 `target/debug/.cargo-build-lock`）。因此本报告**不含任何「本次跑通门禁」的结论**，全部门禁相关表述都是「项目自报基线 + 静态证据」。
2. 但**独立编译小探针是可用的**（`%TEMP%` 可写）：P0 panic 就是用「逐字复制的缺陷函数 + `rustc -O`」机械复现的（见 §1），不是推理结论。这保证了最高危的一条有一手证据。
3. 建议在正常环境复跑一次全量门禁确认基线（AGENTS 记载上次全绿：根 workspace 39 套件 + desktop 356，fmt 双侧 CLEAN，clippy 0，vue-tsc 0）。
4. 会话开始时工作区根的权限授权失败（`SetNamedSecurityInfoW` Win32 5），已用捆绑脚本修复并验证（改动仅：为当前用户在 `F:\Warehouse\AIwork\Locowrk` 增加一条完全控制授权；备份与撤销脚本在 `F:\Warehouse\AIwork\Locowrk-acl-recovery\`）。

---

## 5. 本地环境清理（不影响仓库，但占地可观）

| 路径 | 规模 | 建议 |
|---|---|---|
| `desktop/src-tauri/target/` | 23,956 文件 / **41.7 GB** | desktop 独立 workspace 的第二份构建缓存；`cargo clean --manifest-path desktop/src-tauri/Cargo.toml` |
| `target/` | 41,085 文件 / **15.2 GB** | 根 workspace 缓存；按需 `cargo clean` |
| `tmp/` | 602 文件 / 20.8 MB | v126–v145 走查脚本与截图；建议整目录归档或只留 `v145-walk/` |
| `desktop/node_modules` + `dist` | 145.8 MB + 10.5 MB | 常规产物，按需清理 |
| `.mimosa/` | 79 文件 / 0.2 MB | 外部工具 hook 基线快照（内含 `crates/*` 源码副本），可删 |
| `tmp/audit-*.ps1`（5 个）、`tmp/paniccheck.rs` | 小 | **本次审计留下的一次性取证脚本**（B 域所建；报告刻意保留以便复核 P0：`rustc -O -o %TEMP%\paniccheck.exe tmp\paniccheck.rs` 即可重跑）。复核完可删 |

两份 target 合计 ≈ **57 GB**，是本机最值钱的一项空间回收。

---

## 6. 子域深读结论（A 根 workspace / B desktop Rust / C 前端 / D 插件构建文档）

> 本节由 4 个并行审计员的产出合并（原始台账见 `_parts/A-*.md` … `_parts/D-*.md`）。凡标「Lead 已验证」者，均由 Team Lead 独立 grep/复算确认过；标「域内自述」者保留审计员原始判据，细则在其台账文件里。

### 6.1 C 域 · 前端（`desktop/src`，92 `.vue` / 51 `.ts`）

**Lead 已验证的高价值项**
- **C1 孤儿模块整文件：`desktop/src/lib/side-effect.ts`（57 行）零 import。** Lead grep 复核：全 `desktop/src` 内除自身外零命中，`toolSideEffect()` 无消费方。该表还含已退役的 `sandbox_run: "irreversible"`（`:44`）。副作用分级现由后端字段承担（`types.ts:92 side_effect` → `components/SideEffectBadges.vue:9`，来自 `PluginCard.vue:62` / `plugins/InstalledTab.vue:246`）。**建议：整文件删除（含 `SideEffect` 类型）——这是本次唯一可以「删掉一个文件」的死代码。**
- **C2 三个死 API 包装（Lead 已验证）**：`lib/api.ts:105 assetGc`、`:419 llmListModels`、`:607 llmProbeWindow`，三者全仓零调用点（Lead grep 只命中定义行）。**连带结论**：Rust 侧 `asset_gc` / `llm_list_models` / `llm_probe_window` 三条命令随之失去唯一前端入口（我的契约比对之所以显示「全部有引用」，正是因为这个包装还在——属「包装壳活着、功能已死」的形态）。建议删三行包装，并评估 Rust 命令是否一并退役。
- **C5 → 见 H3b（4 个缺失键，用户可见）**。
- **C6 `sandbox_run` 活跃分支 6 处 + 1 条文案**：`lib/tool-detail.ts:62,123,332,526`、`lib/tool-summary.ts:14,125`、`locales/*:328-330`。与 H3 同源；域内判断与我一致——`tool.action.sandbox` **不会被孤儿扫描抓到**（因为 `tool-summary.ts:14` 仍字面引用它），只能靠工具清单比对发现。删除后历史日志回放会落到既有兜底路径（`{type:"tool", icon:"plug"}` + 插件徽章），不会崩。
- **C7 陈旧工具映射**：`lib/tool-summary.ts:18 fs_edit: "tool.action.edit"` —— `fs_edit` 在全部插件 manifest 中不存在（真实编辑工具是 `edit_replace/edit_lines/edit_diff`，已在 `:19-21` 映射）。建议删该行。

**中价值：重复实现（可机械收敛，均为纯展示层）**
| 编号 | 内容 | 位置 |
|---|---|---|
| C8 | 命令包装剥离两份、正则逐字相同（仅 48/40 截断长度不同） | `lib/tool-summary.ts:63-84` × `lib/tool-detail.ts:151-172` |
| C9 | 工作区相对化两份（tool-detail 版多一步剥 `\\?\` verbatim 前缀，应保留） | `tool-summary.ts:88-98` × `tool-detail.ts:176+` |
| C10 | 相对时间渲染四份，均建在同一 `format.ts:15 relativeTime` 之上 | `memory/ProjectTree.vue:74`、`memory/TreeSection.vue:225`、`MemoryPage.vue:149`、`shell/NotificationPopover.vue:99` |
| C11 | `v-html` 代码块复制委托处理器逐行相同（两者都已在用 `use-copy.ts`，只是这段没下沉） | `chat/MessageRow.vue:171-182` × `chat/ToolDetailView.vue:58-69` |
| C12 | 弹层「外点关 + Esc」三件套 **10 处各写一遍**（其中 3 处逐字相同） | `AttachButton.vue:40`、`UsageRing.vue:28`、`PermissionModePicker.vue:72`、`Composer.vue:60`、`PermissionCard.vue:78`、`WorkspacePicker.vue:96`、`SubagentsMenu.vue:118`、`SettingsSelect.vue:113`、`NotificationPopover.vue:46`、`RowMenu.vue:95` |
| C13 | `stores/settings.ts` 保存样板 ×9（约 135 行里 ~90 行同构；`setTimeout` 无句柄复用=同源小缺陷） | `settings.ts:66-80 / 116-130 / 133-147 / 150-164 / 167-181 / 184-198 / 216-230 / 233-247 / 248-262` |
| C14 | 未使用图标 `shield` / `star`（union + PATH 各两行） | `lib/icons.ts:28,46` |
| C15 | 注释腐烂 3 处 | `lib/settings-index.ts:45-47`（称「对话分区未实装」但 `:78-88` 已实装）、`tool-summary.ts:150`（为 C8 双份实现背书）、`side-effect.ts:3-4` |

**优化建议**
- C16 locale 单文件 1153 / 1135 行：先加「键集合双向相等」机械门禁（收益最高、风险≈0），再按命名域拆 `locales/zh-CN/{chat,tool,settings,…}.ts` + index 汇总。
- C17 `stores/chat.ts` 1064 行：actions 占约 780 行，混了会话生命周期 / 事件投影 / 附件管线 / 旁路状态四件事；建议先把「事件→item 投影」下沉为可单测纯函数，再考虑拆 store。
- C18 `types.ts` 1035 行：拆 `types/{agent-events,settings,memory,llm}.ts`；**附带发现 71 个导出仅内部使用**（过度 `export`，收窄后「谁在用」一眼可查）。
- C19 六个超长组件拆点已逐文件给出（`Composer.vue` 832 / `ProviderSetupDialog.vue` 650 / `lab/BuiApprovalCard.vue` 635 / `lib/tool-detail.ts` 622 / `ModelPanes.vue` 616 / `LlmSection.vue` 574）；其中 `lab/BuiApprovalCard.vue` 是仅 `LabPanel` 引用的演示夹具，**建议移出生产树**。
- C20 消息流无虚拟化 + 每条用户消息一个 `ResizeObserver`（`use-message-fold.ts:128-131`）：最小改动是给已收束历史轮次加 `content-visibility: auto` + `contain-intrinsic-size`；RO 可改共享单例。域内已澄清这**不是**「每帧重算」（流式 delta 不触发 `blocks` 重算），避免夸大。
- C21 附件就绪等待用 80ms 轮询（`stores/chat.ts:912-926`）：可事件化；现实现超时分支未处理仍在 `uploading` 的附件（对「超时=就绪」的语义缝）。

**C 域确认的健康面**：孤儿 `.vue` = 0；`package.json` 7 个 runtime + 10 个 dev 依赖**全部有用**（逐条给出引用点）；无 `TODO/@ts-ignore/as any`、无注释代码块、无 `v-if="false"`；`console.*` 仅 5 处且全在错误路径；无硬编码本机路径（`localhost:11434/1234` 与 `127.0.0.1:7890` 是产品默认值）；Pinia 内无深拷贝/`deep: true` 重 watcher。

**C 域不确定项（诚实标注）**：78 条动态键（`tool.type.${x}` 等 10 组前缀）本审计无法证明零引用，未计入 59 条；`tool-summary.ts` 动作名表缺 `memory_delete` / `memory_undo`（工具在 manifest 中存在）→ 界面回落裸 id，**待裁定**是有意还是漏补。

### 6.2 A 域 · 根 workspace `crates/`（8 crate / 136 `.rs`）

**域内高价值项已并入正文**：M-20（两个零接线子系统：`snapshot`+upcaster 簇、`serial` 监听器机制）、M-21（`kv_put` 哑 ABI + `ERR_UNKNOWN_TOOL` 幻影码）、M-22（仅测试消费的 pub 面）、M-23（幻影字段 `anchor_source` + `valid_session_id` 双份分叉）、M-24（优化 a–e）、M-25（注释腐烂 3 处）；重叠项（M1/M2/M4/M5/M6/M7/M9）由 A 域独立复核**全部成立**并补了边界证据。

**A 域补的关键反向证据（影响处置口径）**：`crates/cordis-core/Cargo.toml:9` 的 `plugin-api` 未使用依赖**是 `DESIGN.md:1266` 依赖图明写要求的**——删它必须同步改 `DESIGN.md:1264-1279`（§8 依赖图），否则文档与清单反向不符；`kernel-bin/plugin-service`、`memory-tree/tokio(dev)` 两行可直接删。另：`crates/session-store/src/session.rs:412` 的 re-export **纯冗余**（`lib.rs:22` 已从 `event` 直接导出 `TurnId/TurnOutcome`，且 `session` 是私有模块），删除零风险（`#[allow]` 同时消失）。

**A 域健康面（覆盖率声明）**：孤儿模块 **0**（136 文件 × `mod` 双向核对）；退役语义残留 **0**（`sandbox|unlockRisky|DENY_HARD|organizer_mode|autoExtract|experiences|handoff…` 在 `crates/` 零命中，v2.137/v2.145 的 crate 侧清理干净）；标记类垃圾 **0**（`TODO/FIXME/HACK/XXX/DEPRECATED`、`unimplemented!`、代码内 `todo!` 全 0）；注释掉的代码块 **0**；`#[allow]` 共 5 处**均正当**（宏内裸指针 / 多集成测试共享模块 / 待验证的 A10 / M6 那处）；268 个 `pub` 字段里 ≤2 次引用的**仅 1 个**（M-23 的 `anchor_source`）；591 个 `pub` 项全仓零引用 **3 个**（全部已列）；8 个 crate × 全部依赖逐个源码比对**仅 3 处零命中**（= M1）；workspace members 与目录一一对应且 `tool-sandbox` 无残留。

**A 域不确定项（附定论命令）**：V1 `plugin-host/src/wasm/backend.rs:42` 的 `clippy::too_many_arguments` allow 疑多余（函数恰好 7 参 = clippy 缺省阈值，需摘掉后跑一次 `clippy -p plugin-host --all-targets`）；V2 `snapshot` 簇属「未接线缺陷」还是「预留」需读 `DESIGN.md:1360-1380`（§9.5）定；V3 删 `SerialListener` 前建议 grep 仓外插件；V4 M-24a 缺实测绝对耗时（需在 100K token 会话打点）；V5 含 `.` 的会话 id 现网不可达（前端 id 形如 `desktop-<毫秒>`），未来引入即升级为真缺陷。

### 6.3 B 域 · `desktop/src-tauri/`（Rust 后端）

**域内最高价值项已单列**：§1 的 **P0 panic**（Lead 已机械复现）；B 域另独立得出「命令名层 85 = 85 = 85 双向零漂移」的结论，与 Lead 的机械比对互证。

**域内其余条目已并入正文**：M-14（`now_ms` ×7 / `clip` ×6 / 常量 ×3）、M-15（推测性 API 与测试专用再导出）、M-16（跨平台报错文案）、M-17（陈旧 `#[allow]` 与注释腐烂）、M-18（热路径 a–g）、M-19（`init_state` 636 行与巨型测试文件）。另有两条「包装壳活着、功能已死」的契约死代码，与 C 域独立发现同源：`api.ts:105/419/607` 的 `assetGc`/`llmListModels`/`llmProbeWindow` 零 import，连带 `desktop/src-tauri/src/llm/probe.rs`（50 行）+ `probe/tests.rs`（27 行）**整模块退役**（唯一消费者是孤儿命令 `llm_probe_window`，其 UI 触发点「设置页自动检测」随 v2.137 四层链下线），以及 `desktop/src/types.ts:1030 ProviderModels` 接口孤儿。

**B 域健康面（逐条取证）**：孤儿模块 **0**（105 个模块名与文件双向核对）；事件名双向闭合（20 个 `agent://`/`plugin://` + 5 个 `lw://` 与前端 `listen` 一一对应；两条「永不 emit」是**有意设计**且有注释）；`capabilities/default.json` 9 项权限**全部有消费者**；`TODO/FIXME` 仅 1 条且是测试跳过提示；注释代码块 0；生产路径无写死密钥/邮箱/端口/本机路径（`localhost:11434` 是 Ollama 官方默认端点常量）；**30 个 Cargo 依赖逐个取证全部真用**（低计数者已单点目检）；v2.145 `ShellSection` 退役干净（`AppSettings` 已无 `shell` 字段）；**幻影键已确认是有意设计**（`context.thresholdPct/strategy/stabilityRounds`、`memory.autoExtract/handoffUpdate/experiences/defaultOrganizer` 均有回归测试锁旧 JSON 可解析）——反向提醒：`memory.collapseThresholdPct` 看着像退役键，实为**活配置**（`settings.rs:323-326` → `organize/mod.rs:350/366`），误报即真 bug。

**B 域不确定项**：`settings.rs:174-190` 的窗口迁移只查活动档（多档场景会留幻影键；当前判无害）；`capabilities` 只覆盖 `main` 不含 `splash`（当前 splash 零脚本故健康，将来加脚本会静默失败）；`is_outside` 不处理 `\\?\UNC\` 形态（属「宁漏勿错杀」的有意边界，档位兜底）。

### 6.4 D 域 · 插件 / 构建 / 文档

**Lead 已验证的高价值项**（均已 grep/复算复核）
- **H8 的两条 UI 能力声明失真**（`tool-fetch` manifest `Pure` vs 代码 `Irreversible`、`tool-shell` manifest 仍讲硬锁/unlockRisky）——D 域定位了完整消费链（`build_plugins.sh:45` → `plugin-service/src/display.rs:46,63,69` → `PluginCard.vue:62`），Lead 逐条复核成立。
- **H9 的权威文档失真**：`DESIGN.md` 2779 行 **0 处 v2.145**（Lead 复算命中数 = 0）、`plugins/README.md:22-33` 整表四轴全错、`UI shell §3.2/§4.2` 与同文件版本条自相矛盾。
- **H1 的 CI 结论独立复现**：D 域从 `Cargo.toml:25 exclude = ["desktop/src-tauri"]` 推出「`--workspace` 不含 desktop → 项目宣称的 desktop/front 门禁在 CI 上不存在」，并指出 `check_length.sh:27 exit 0` 使 CI 那步成为装饰——与 Lead 独立结论一致（两条独立路径互证）。
- **M-5 根因**：`build_plugins.sh:47-49 cp -R` 非幂等（D 实测 27/60 份暂存图标落在 `assets/assets/icon.svg`；对照 `build_market.sh:80-81` 点名到文件故无此问题）——与 Lead 定位的机制一致。

**中价值补充**
- `plugins-src/tool-shell/Cargo.toml:21-24` 声明了 `Win32_Security` / `Win32_System_Threading` 两个 feature，代码里只有 `Foundation` + `JobObjects`（`src/exec.rs:284-288`）→ 疑为随 tool-sandbox 退役而失去引用的死 feature。**待验证**：删后跑 `cargo clippy -p tool-shell --all-targets`（本次沙箱无法编译）。
- 两个 build 脚本重复 **23 行**完全相同骨架（占 `build_market.sh` 的 26%）→ 可抽 `scripts/lib/stage.sh` 或合并为「吃一张 stage 表」的脚本。
- `market/README.md:13` 仍描述 v1 极简 manifest，而实际是 packaging.md v2 全量（模板见 `build_market.sh:42-79`）。
- `market/tool-shout/1.0.0/manifest.json` 与 1.1.0 共用同一 schema（含 `mode: repeat`），而 1.0.0 运行时会拒绝 `repeat`（`tool-shout/src/lib.rs:33,113,128`）→ 元数据比行为乐观（演示升级叙事的副作用，非清单脱节）。
- 文档腐烂组（口径统一建议加「> **vX.YZ 退役注**」）：`docs/design/BUG/*` 两份清单的「待处理」状态行与自身处理记录矛盾（`Tool Layer Issues Todo List.md:5` vs `:153-164`、`bug-report.md:3`）；`docs/design/NEW/Reliability Reinforcement Design.md:755` 与 `Model Capability and Multimodal Phase Plan.md:154` 仍称「硬锁/高危/unlockRisky 全保留」；`plugins.md:27,152` 两表仍把已退役的 `tool-sandbox` 列为 P2 待办（与同文件 `:200-204` 退役注冲突）、`:206-213` 仍以现状语气描述 `sandbox_run`。

**D 域优化建议（落文件）**：S1 版本真源收敛到 stage 表 → S2 新增 `--check-manifest`（从 DLL describe 生成/校验 manifest，一次修掉 H8 + M-7）→ S3 两 build 脚本合一 → S4 暂存流程幂等化 → S5 门禁覆盖补齐（CI 加 desktop、`check_length` 加 `plugins-src`、步骤改名）→ S6 文档分层（AGENTS 瘦身 + CHANGELOG 拆目录 + 修 v2.75 撞号）→ S7 退役残留清扫（统一退役注格式）→ S8 孤儿稿归档 + `docs/design/INDEX.md` → **S9 插件共享基座**（本域新发现，与 §2 M-1 合并）：SSRF 防线两插件 **361 行**逐行同形（仅 env 名与错误前缀不同）、`zh_en` ×10、`plugin_version` ×10、`unsafe impl Send/Sync + SAFETY` ×10、官方展示元数据样板 ×10、`arg_u64` ×6（fs 与 git 两版已轻微漂移）→ 可上收 `plugin_api`（`netsafe` / `args` / `manifest::official` + `declare_official_plugin!` 宏），**合计可减约 600–700 行重复代码**，并消灭「同源实现分家后漂移」这类缺陷（v2.133 的 ENV_LOCK 分裂、v2.135 的 gc 序错都是同源产物）。

**D 域确认的健康面**：`tool-sandbox` **功能性残留 0 处**（插件/暂存/市场/脚本/配置/CI/hook 全清，根 workspace members 无该 crate，唯一命中是有意留痕的 `build_plugins.sh:146`）；10 个插件 **29 个工具**的 manifest 名单与 DLL describe 逐个相符（v2.135 那类欠账无复发，唯一例外 = H8 的 tool-fetch 副作用）；插件依赖全部真用（唯一可疑项是 feature）；`plugins/` 入库面只有 `README.md`、`market/` 只有 README + tool-shout 两份 manifest/图标；`git ls-files -i -c --exclude-standard` 零输出（无「被跟踪却被忽略」）；`.gitattributes` 与现状自洽；`docs/上下文与记忆工作流程报告.md` 头部 6 条修订注是**活文档正面样本**（建议 H9 照此格式修）。

**D 域不确定项**：windows-sys 两 feature（需编译验证）；manifest schema 无消费方系代码阅读结论；`docs/design/BUG/bug-report.md` 的 Bug 3 是否已被独立复测；`check_length.sh` 纳入 `plugins-src` 后的确切超限数（实测该两文件 533/557 行，预期基线 32 → 34）。


---

## 7. 跨域优化与流程建议（按性价比排序）

### 7.1 让门禁真的拦得住
当前 CI 只有根 workspace 一条线，desktop 与前端完全在纪律之外；两处「行数门禁」因 `check_length.sh` 恒 `exit 0` 而永不失败（详见 H1）。落地清单：CI 增 desktop 的 fmt/clippy/test（Windows runner）+ 前端 `pnpm -C desktop run build`（内含 `vue-tsc`）与 `lint:style`；行数改基线比较；步骤名与实际行为一致。

### 7.2 三类死代码需要三类新防线（本次只靠人工审计发现的部分，都应有机器兜底）
| 死代码类型 | 本次实例 | 建议防线 |
|---|---|---|
| 未使用依赖 | 3 处（H6） | `cargo machete`（无需 nightly，逐 Cargo.toml 判定）纳入 CI；或 `cargo +nightly udeps` |
| **存在但从未接线**的机制 | `CtxCompact`（778 行）、`Retry`（46 行）、`config/profile.toml`、`CallCtx` 取消设施（H2/H4/H5） | 在宿主装配处加**断言测试**：`Vec<Arc<dyn Middleware>>` 的组成与「声明清单」逐项相等，未接线项必须出现在显式 `KNOWN_UNWIRED` 白名单里——白名单一旦为空即强制删除。这类问题编译器、clippy 都看不见，只有「声明 vs 装配」的交叉验证能抓住 |
| 退役功能残留 | 前端 `sandbox_run` 6 处 + 59 条 i18n 孤儿键 | `AGENTS.md` 增一节「退役六面清单」模板：① 插件源码目录 ② 暂存/市场产物 ③ 加载清单与 manifest ④ 内核/宿主契约与装配 ⑤ 前端映射与分支 ⑥ i18n 文案（含反向验证）。v2.145 的内核/宿主面清理是**满分**（Lead 全仓复查 `organizer_mode/experiences/extract_deps/unlock_risky/NO_BLANKET_PREFIX/ShellSection` 等零残留），漏的正是 ⑤⑥ 两面 |

### 7.3 i18n 门禁（单点收益最大）
`scripts/check-i18n.mjs`：① zh/en 键集合双向相等（当前 949=949 作为基线）；② 报告「无静态引用且不在动态前缀白名单内」的键；③ 反向：`t()` 字面量键必须在语言包内（本次抓出 4 个用户可见缺陷，见 H3b，`vue-tsc` 完全看不见）。挂 `.githooks/pre-commit`（仅 `.vue/.ts` 变更时触发）。

### 7.4 命令注册表契约测试
本次手工做的「85 个 `#[tauri::command]` ↔ `lib.rs` 注册表 ↔ 前端 `invoke()` 名」三向比对应脚本化（一个 Node 脚本 + 一个 Rust 测试即可）。当前**完全闭合**是纪律的功劳，但这种闭合没有任何东西在守。

### 7.5 构建脚本幂等性
`build_plugins.sh:47-49` 的 `cp -R` 重复执行会叠出 `assets/assets/`（M-5 已定位根因）。同类还有 `mkdir -p "$dst"` 不清理旧产物。建议改为「先删目标目录再复制」并让脚本可重复执行。

### 7.6 文档治理
- `AGENTS.md` 191 KB：每次会话注入且当前已被 65536 字节预算截断——**历史进度流水的成本按「每个 agent 每次对话」计价**。建议：把非当前的版本段落归档到 `docs/design/CHANGELOG.md` 或新 `docs/archive/`，`AGENTS.md` 只留「当前状态 + 现行纪律 + 指针」。
- `README.md` 仍写「处于阶段一」（H4 同源）；`config/profile.toml` 的「已接线」描述、`scheduler/{mod,store}.rs` 里「与 experiences 同纪律」的注释（模块已删）都属注释腐烂，随批订正。

### 7.7 磁盘与本地产物
两份 `target/` 合计 ≈ 57 GB（§5）；`tmp/` 602 个走查文件建议整目录归档。**注意**：`desktop/src-tauri/target` 是 desktop 独立 workspace 的缓存，若要与根共享可在两处 `[build] target-dir` 指向同一目录（收益：省一份依赖编译；代价：门禁串行，需评估）。

### 7.8 明确**不建议**做的事（避免过度治理）
- 不要为清 `Cargo.lock` 的 11 个多版本 crate 强行 `cargo update`（多为传递依赖，收益低、回归面大）。
- 不要因行数就拆高内聚文件（项目「三问」纪律原文如此；本报告只对**混职责**的文件给拆点）。
- 不要把 `pdfium.dll` / `models.dev/api.json` 移出 git（随包语义已记档，改动会破坏离线可用性）。
- 不要删 `docs/design/ui-prototype/`（仍被设计文档引用；标注「历史原型」即可）。

