# Locowrk 审计修复执行计划（AUDIT-FIX-PLAN v1）

- 依据：`docs/reports/CODE-AUDIT-v1.md`（2026-10-10 全仓审计）
- 本计划制定时间：2026-10-10
- 制定时的仓库状态：`main` @ `fb6eb0d`（v2.145 权限模型重设计已提交），工作树干净
- 说明：本文只做排期与判据登记，不含任何已执行的改动。**待裁定项未批不动。**

---

## 0. P0 复核结论：已修，且修法与建议一致

逐行核过 `desktop/src-tauri/src/approval.rs`：

| 建议修法 | 现状（行号） | 判定 |
|---|---|---|
| `rest_starts_unc` 改字节判定 | `:495-497` = `cmd.as_bytes()[i..].starts_with(b"\\\\")` | ✅ |
| `i += 1` 改整字符步进 | `:486-490` = `i += cmd[i..].chars().next().map_or(1, char::len_utf8)`，带缺陷成因注释 | ✅ |
| 补含 CJK 的回归测试 | `desktop/src-tauri/src/tests_permission.rs:283-287` 三条断言：`echo 你好` / `git commit -m "修复"` / `type 中文文件.txt` | ✅ |

边界另核两处，均安全：`i < cmd.len()` 保证 `as_bytes()[i..]` 不越界；切点 `end` 由 `find(is_delim)` 得出，而 `is_delim` 集（`:464-469`）全是单字节 ASCII 字符，故 `cmd[i..end]` 恒在 char boundary 上。

补充一条**同类残留隐患**（审计未列，我复核时注意到）：`:481` 的 `trim_matches` 与 `:480` 的 `find` 现在安全，但整个函数依赖「步进点必为字符边界」这一隐式不变量。建议批 1 顺手把步进改为 `char_indices()` 单点驱动（行为逐字节不变），把不变量交给编译器而不是注释。

---

## 1. 批次划分（按「风险 ÷ 收益」排序，非按审计编号）

每批交付口径与项目现行纪律一致：最后一次编辑后全量复跑门禁（根 `cargo fmt --check` + `cargo clippy --workspace --all-targets` + 根 workspace 测试 + `desktop/src-tauri` 单独 `fmt --check`/`clippy --all-targets`/`cargo test` + `pnpm build` + `lint:style` + `check_length` 基线对比），先勾 ROADMAP 再写 CHANGELOG/AGENTS。

### 批 A｜零裁定、用户可见、纯收益（可立即开工）

| # | 条目 | 位置 | 动作 | 风险 |
|---|---|---|---|---|
| A1 | **4 个 i18n 键被引用但两语言包都没有**（审计 H3b） | `settings/LlmSection.vue:296`、`settings/MemoryTreePanel.vue:103`（两键）、`settings/model/ImportDialog.vue:110` | 补 `settings.proxySavedToast`、`developer.modeMemoryManager`、`developer.modeRule`、`common.close`（zh/en 各 4 条，纯新增） | 零 |
| A2 | `settings-index.ts:136` 搜索键写成 `settings.panelTitle`，实际键是 `developer.panelTitle` | `desktop/src/lib/settings-index.ts:136` | 改键名（⌘K 设置搜索命中恢复） | 零 |
| A3 | 孤儿整文件 `lib/side-effect.ts`（57 行，含退役的 `sandbox_run: "irreversible"`） | `desktop/src/lib/side-effect.ts` | 整文件删除 | 零（已核全仓零 import，副作用分级真源是后端 `side_effect` 字段） |
| A4 | 陈旧工具映射 `fs_edit`（插件 manifest 里不存在该工具名） | `desktop/src/lib/tool-summary.ts:18` | 删该行 | 零 |
| A5 | **UI 能力声明与代码相反**（审计 H8）：`tool-fetch` manifest 标 `Pure` 而代码 `SideEffect::Irreversible`；`tool-shell` manifest 仍在讲「白名单/denylist/解锁开关」 | `plugins-src/tool-fetch/manifest.json:34`；`plugins-src/tool-shell/manifest.json:12-13,24` | 两份源 manifest 的 `side_effect` 与 `description` 同步现网语义 + `bash scripts/build_plugins.sh` 重建暂存产物 | 低（展示面；模型可见描述本就来自 DLL describe，不受影响） |
| A6 | 两条拒绝报文合并，违背「错误信息要教模型下一步」（审计 M-10） | `plugins-src/tool-shell/src/policy.rs:89-93,131-133` | `split_command` 区分 `Empty` / `UnclosedQuote`，报文各指各的出路 | 低（需 tool-shell 版本 +0.0.1 与产物重建） |
| A7 | 报错文案把 Windows 路径写死，crate 声明三平台（审计 M-16） | `desktop/src-tauri/src/browser/tools.rs:119-123`（测试 `:186` 同源） | 平台中立表述或 `cfg!(windows)` 分支 | 低 |
| A8 | 陈旧抑制属性与注释腐烂（审计 M-17、M-25） | `state/app_state.rs:23-26,77`、`scheduler/mod.rs:9`、`scheduler/store.rs:2`、`crates/kernel-bin/src/main.rs:7`、`crates/session-store/src/persist.rs:166-168`、`crates/kernel-agent/src/middleware/mod.rs:62-63` | 删已失效的 `#[allow(dead_code)]`（`app` 字段确有消费者，已核 `commands.rs:110`/`scheduler/engine.rs:218`）、订正「experiences 先例」「三个监听器（实为 7 模块 9 处）」等腐烂注释 | 零 |
| A9 | 未使用依赖两处（审计 H6 的可直接删部分） | `crates/kernel-bin/Cargo.toml:12`（`plugin-service`）、`crates/memory-tree/Cargo.toml:15`（dev `tokio`） | 删两行 | 低（删后跑一次 clippy 即证） |
| A10 | `session.rs:411-412` 纯冗余 re-export + 其 `#[allow(unused_imports)]`（审计 M-3/M-24e） | `crates/session-store/src/session.rs:409-412` | 删（`lib.rs:22` 已直接导出） | 零 |

批次规模估计：10 项小改，前端 4 处 + Rust/插件 6 处，文档 0 处。

### 批 B｜门禁体系（审计 H1 + §7.1–7.5）——本次最值钱的一批

| # | 条目 | 现状证据（已复核） | 动作 |
|---|---|---|---|
| B1 | CI 完全不碰 desktop 与前端 | `.github/workflows/ci.yml:9-24` 只有 `ubuntu-latest` + 根 workspace 三步；`Cargo.toml` `exclude = ["desktop/src-tauri"]` | 新增 desktop 作业（`windows-latest`）：`cargo fmt --manifest-path desktop/src-tauri/Cargo.toml --check`、`clippy --all-targets -- -D warnings`、`cargo test`；再新增前端一步 `pnpm -C desktop run build`（内含 `vue-tsc --noEmit`）+ `pnpm -C desktop run lint:style` |
| B2 | 「行数门禁」两处空转 | `scripts/check_length.sh:27` 恒 `exit 0`；`ci.yml:23` 步骤名却写「File length limit (400)」；`.githooks/pre-commit:8` 写 `... \|\| exit 1` | 改**基线比较**：输出超限计数 + 清单，与基线（当前实测 **32**）比较，超过即红；CI 与 hook 同步；步骤名与行为一致 |
| B3 | 规模门禁覆盖面缺口 | 脚本只扫 `crates` + `desktop` 的 `*.rs\|*.vue`。我实测：纳入 `plugins-src` → **+2**（34）；纳入 `desktop/src/**/*.ts` → **再 +6**（locales `en.ts` 1153 / `zh-CN.ts` 1135 / `stores/chat.ts` 1064 / `types.ts` 1035 / `lib/api.ts` 764 / `lib/tool-detail.ts` 622，今日从未出现在门禁输出里） | 与 B2 同批改扫描范围，基线一次锁死为新口径；超限行拆不拆仍走「三问」 |
| B4 | i18n 无防线（历次「删组件不删文案」的根因） | 本次人工审计才抓出 59 孤儿键 + 4 缺失键 | 新 `scripts/check-i18n.mjs`：① zh/en 键集合双向相等（基线 949=949）② 静态无引用键报告（动态前缀白名单）③ **反向缺键**（`t()` 字面量键必须在包内——A1 那 4 条就是它要防的）；挂 pre-commit（仅 `.vue/.ts` 变更触发） |
| B5 | 命令注册表闭合无人守 | 85 `#[tauri::command]` ↔ `lib.rs` 注册 ↔ 前端 `invoke()` 目前完全闭合，纯靠纪律 | 一个 Node 脚本 + 一个 Rust 测试做三向比对，挂 CI |
| B6 | 未使用依赖无人守 | 3 处靠人工审计发现 | `cargo machete` 进 CI（不进 pre-commit，避免本机装工具依赖） |
| B7 | 三处「看起来在工作」的死机架（审计 M-9） | `scripts/check-style.mjs:24-28,75,81`（四条 RULES 全 error → `warns` 恒空 → 二级报告分支不可达）；`.gitignore:26-28` 三条被 `:37 plugins/*` 整体覆盖；`.zcodeignore` 缺 `docs/issue/` | 删/注释说明机架用途；`.gitignore` 去冗余三行；`.zcodeignore` 补一行同步 |
| B8 | 构建脚本非幂等（审计 M-5） | `scripts/build_plugins.sh:47-49` `cp -R` 叠出 `assets/assets/`——我实测 `plugins/` 下 10 个插件多版本目录命中 | 暂存前清目标目录，脚本可重复执行 |
| B9 | 「存在但从未接线」无人守（§7.2） | `CtxCompact`/`Retry`/`config/profile.toml`/`CallCtx` 取消全部靠人读出来 | 宿主装配处加**断言测试**：中间件组成与「声明清单」逐项相等，未接线项必须显式列在 `KNOWN_UNWIRED` 白名单，白名单为空即强制删除 |

风险与代价（如实）：desktop 作业跑在 `windows-latest` 上无缓存可复用，CI 时长会显著上升（本机 `target/` 41.7 GB + `desktop/src-tauri/target/` 23.9 GB 的规模即编译成本的旁证）。可先只接 desktop 的 fmt+clippy+test，前端步骤零成本；若 CI 时长不可接受，退路是把 desktop 作业挂 `workflow_dispatch`/每日定时而非每次 push。**GitHub Actions 的实际运行结果我无法在本机验证，这一条只能等你 push 后确认。**

### 批 C｜退役残留清扫（审计 H3）——✅ 已落（2026-10-11，证据见 §5 批 C 段）

| # | 条目 | 判据 | 待裁定的口径 |
|---|---|---|---|
| C1 | 59 个确证孤儿 i18n 键（zh+en 共 118 条死文案），分布 `settings.*` 23 / `approval.*` 8 / `subagent.*` 8 / `developer.stabilityRounds*` 2 等 | 台账 `_parts/1-i18n-orphans.md`；`app.modelFallback` 已由 Lead 撤回（`Composer.vue:99` 在用） | 删（推荐）；删前先人工复核 9 条「叶名以裸字符串出现过」的歧义项 |
| C2 | `sandbox_run` 前端残留 6 处：`lib/tool-summary.ts:14,125`、`lib/tool-detail.ts:62,123,332,526`、`lib/side-effect.ts:43-44`（A3 已删）、`locales/*` 两条 | 插件 v2.145 已整目录退役，新会话不可达；**历史会话日志重放仍会渲染这些工具行** | 二选一：① 一并删（推荐——审计已确认删后落既有兜底 `{type:"tool", icon:"plug"}`+插件徽章，不崩）；② 判为「历史渲染面」保留 |
| C3 | 三个死 API 包装 + 连带的 Rust 命令（审计 C2/B 域同源） | 已复核零 import：`lib/api.ts:105 assetGc`、`:419 llmListModels`、`:607 llmProbeWindow`；`types.ts:1030 ProviderModels` 孤儿。其中 `llm_probe_window` 的 UI 触发点（设置页自动检测）随 v2.137 四层链下线，`llm/probe.rs`（50 行）+ `probe/tests.rs`（27 行）整模块随之无消费者 | **这条不能只删包装**：删包装而留命令 = B5 那道三向闭合当场出现「后端有、前端无」红灯。三选一：① 连 Rust 命令一起退役（三处命令 + `lib.rs` 注册 + `probe.rs` 模块 + 相关测试）；② 只删 `llm_probe_window`/`llm_list_models` 两条真死的，`asset_gc` 保留（它是开发者手动 GC 入口，启动 + 24h 循环之外仍有用途）；③ 全保留并加「预留」注释 |

### 批 D｜死配置与哑 ABI（审计 H4/H5 + M-20/M-21/M-22/M-23）——✅ 已落（2026-10-11，证据见 §5 批 D 段；D2 转 批 H）

| # | 条目 | 现状（已复核/审计取证） | 建议口径 |
|---|---|---|---|
| D1 | `config/profile.toml`（审计 H4） | 实测文件 7 行全注释、零生效键；全仓 Rust 无任何读取路径（`grep profile.toml` 唯一命中是 `kernel-bin/src/main.rs:7` 的「阶段七补」文档注释）；README 与 `DESIGN.md:1256,1695` 按「已实现」描述 | **需裁定三选一**：① 删文件 + 从 README/DESIGN 移除（推荐，当前桌面端插件装载走 lockfile/fsRoots，这条路已不存在）；② 保留 + 文档改「规划中」；③ 真实现加载器（工作量最大且与现网插件机制重复） |
| D2 | `CallCtx` 取消设施（审计 H5） | `crates/plugin-api/src/tool.rs:180,193,197` 全仓零调用；唯一 `cancel` 字面命中是 `scheduler/tools.rs:51` 的描述文案 | **v2.148 批 D 改判：不在清理批里接线**。查证发现取消信号**过不去 C ABI 边界**（插件 `call` 是同步 `extern "C"`，`HostVtable` 五槽没有取消/轮询入口，调用还跑在 `spawn_blocking`）——只接宿主侧就是再造一处「实现只到一半」，正是本轮审计在罚的形态。处置 = 就地标 RESERVED + 写清硬事实与最小完整形态，真接线转 **批 H**（见 §1 批 H 表）。搭车查证：`deadline` 是**有消费者（Timeout 中间件）无生产者**，同一批处理 |
| D3 | session-store 磁盘快照 + upcaster 簇（审计 M-20，111 + 122 行） | `Snapshot`/`Upcaster`/`V1ToV2Upcaster`/`snapshot_at`/`from_snapshot` 全仓零生产引用；恢复走 `persist.rs:80-98` 逐行 JSONL 回放 | 先读 `DESIGN.md:1360-1380`（§9.5）定「未接线缺陷 vs 预留」再决定；⚠ 勿连带删内存态 `Session.snapshots`（活的另一套）；⚠ 删除要同时撤回 `CHANGELOG.md:28` / `ROADMAP.md:872` 的归档里程碑——**这一条我倾向标注 RESERVED + 注释，而不是删**（归档代价 > 111 行收益） |
| D4 | cordis-core `serial` 监听器机制（约 45 行 + 测试） | 命中全在 `event_bus/tests.rs` 与 `lib.rs:27` re-export；`serials` map 生产恒空；其自述用途已被宿主侧中间件取代（`middleware/audit.rs:5`） | 删整簇或标 RESERVED，同步 DESIGN §3.5 表 |
| D5 | `host_kv_put` 回 OK 但数据永不可读 + `ERR_UNKNOWN_TOOL` 幻影码（审计 M-21） | `plugin-host/src/host_vtable.rs:65-93`；`plugin-api/src/ffi.rs:16` | 让 `host_kv_put` 如实返回 `ERR_UNSUPPORTED`（ABI 签名不动，语义诚实化）；`-2` 补人类可读报文或标 RESERVED（`ffi.rs` 自述冻结契约，不宜删） |
| D6 | 仅自家测试消费的 pub 面（审计 M-22） | `runtime.rs:83-98` 四个观测方法、`lockfile.rs:154-157 clear_pending_delete`（与 `service.rs:277-287` 同语义两实现）、`registry.rs:210-216 graveyard_len`、`kernel.rs:109-113 with_retry_budget` | 降 `pub(crate)`/`#[cfg(test)]`；`deps_of` 直接删 |
| D7 | 幻影字段 + 双份校验规则（审计 M-23） | `memory-tree/src/trigger.rs:76 anchor_source` 只写不读；`valid_session_id` 两份规则**已分叉**（`session-store/persist.rs:23-29` 允许 `.` vs `memory-tree/storage.rs:15-22` 不允许且限长 64，注释却自称同源）→ 含 `.` 的会话 id 日志落盘成功而树投影静默失效。当前桌面 id 形如 `desktop-<毫秒>` 恰好都合法 = **陷阱不是现网缺陷** | 删 `anchor_source`；`valid_session_id` 下沉 `plugin_api`（有 `fsutil` 先例）单点定义 + 单测锁规则 |
| D8 | `cordis-core → plugin-api` 依赖（审计 H6/M-6.2 反向证据） | 代码零使用，但 `DESIGN.md:1266` 依赖图明写要求 | 删它必须同批改 `DESIGN.md:1264-1279`（§8），否则文档与清单反向不符。**倾向：连文档一起改，或干脆不动（这条是设计约束不是垃圾）** |

### 批 E｜权威文档真实化（审计 H9 + M-6/M-8/M-11/M-12）——✅ 已落（2026-10-11，证据见 §5 批 E 段；E4b「CHANGELOG 拆目录」未采纳并登记待裁定）

| # | 条目 | 建议动作 |
|---|---|---|
| E1 | `DESIGN.md` 2779 行 **0 处 v2.145**（mtime 2026-10-04）；`:1152` 决策表仍写 `sandbox_run`、`:2382` 仍写白名单 + `LOCOWRK_SHELL_ALLOW` + denylist 约 50 项，且缺 v2.145 新增的 ③项目 deny / ⑤项目 allow 两序 | 按 `docs/上下文与记忆工作流程报告.md` 头部「⚠ v2.13x 修订」格式逐条加退役注；决策表补 ③⑤ 并删 `sandbox_run`。**这条最危险——AGENTS 明写「改核心模块前先读 DESIGN.md」，照它实现会写出一套与代码相反的策略层** |
| E2 | `plugins/README.md:22-33` 整表四轴全错（八件实为 10、点号工具名、1.x 版本、fetch 标 Pure、shell 讲白名单） | 整表重写；与 A5 同批可互相印证 |
| E3 | `UI shell and patterns.md:193,202,883,999` 现状表仍列 v2.136/v2.137 已退役键与命令，与同文件版本条自相矛盾 | 逐条加退役注（只加注，不重写正文） |
| E4 | `AGENTS.md` 191 KB、项目概览占全文 **94%**、`:8` 与 `:37` 两个「当前进度」、v2.76–v2.124 整段缺失、且每次会话被 65536 字节预算截断 | 历史流水移入 `docs/design/CHANGELOG/`（按版本拆目录 + 索引表），`AGENTS.md` 只留「当前状态 + 最近 2–3 版 + 现行纪律 + 指针」。**注意本仓 `agents.md` 就是当前会话注入源，瘦身是收益最直接的一项** |
| E5 | `CHANGELOG.md:81` 与 `:86` 两条互不相关的批共用 v2.75（撞号），「按版本号追溯」失效 | 修撞号；随 E4 的归档一起做 |
| E6 | `docs/design/Sandbox Execution Design.md`（575 行）全仓零引用，主题已随 v2.145 退役且实际交付走的是受限令牌方案 | 移 `docs/design/archive/` + 首行「未采纳 + 已退役」两行；顺带建 `docs/design/INDEX.md`（权威/存档/已退役三类） |
| E7 | 版本注释三处过期（审计 M-12）：`build_plugins.sh:146` 收尾文案 + 9 个 `plugins-src/*/Cargo.toml` 的「按 1.0.0 构建」注释 | 版本真源收敛到 stage 表，注释统一指它 |
| E8 | `README.md` 仅 21 行且停在「阶段一」；`market/README.md:13` 仍描述 v1 manifest；`docs/design/BUG/*` 两份清单状态行与自身处理记录矛盾；`Reliability Reinforcement Design.md:755`、`Model Capability ... Phase Plan.md:154`、`plugins.md:27,152,206-213` 仍讲已退役的硬锁/sandbox | 逐处订正（纯文档，零代码风险） |

### 批 F｜重复实现收敛与性能（审计 M-1/M-14/M-18/M-19/M-24 + C 域 C8–C13）——◐ 四个子批已落（2026-10-11：**v2.150** 批 F-1 = F1/F2/F3/F14，**v2.151** 批 F-2a = F11 的 desktop 侧，**v2.152** 批 F-2b = F8 + F11 的根侧，**v2.153** 批 F-3 = F5 第一项（`init_state` 636 行 → 八簇）。至此 **F1/F2/F3/F8/F11/F14 六条 ✅ + F5 ◐**，各自证据见 §5 批 F-1 / F-2a / F-2b / F-3 四段；未动 = F4 热路径、F5 余四项（`organize/mod.rs` 614 / `runner::project_events` 237 / `settings/llm.rs` 682 / `session.rs` 410 + 巨型测试文件）、F6 前端重复、F7 plugin-api 共享基座（最大一刀）、F9 前端超大文件、F10 session-store 优化、F12 报错教学、F13 market manifest 通路；**F11c 本批新立未修**）

低风险优先，建议**一轮只动一类变量**（与项目纪律一致）：

- ✅ **F1** `desktop/src/timeutil.rs`：`now_ms()` 7 处 + `now_secs()` 2 处收敛（`scheduler/store.rs:65` 已是 `pub(crate)` 共享版）。**实测规模比计划大**：本地定义 9 份 + 内联链 8 处 = 17 段取时刻链（计划按函数名只数到 9）。
- ✅ **F2** `desktop/src/text.rs`：`clip()` 6 处 / 4 种语义收敛；⚠ `export.rs` 的输出是用户可见的 Markdown 字节，合并必须逐字节不变（先写等值断言再动）。**断言已先行**：等值测试以被删的 6 份原文为参考实现。
- ✅ **F3** Ollama 端点常量 3 份留 1 份（`llm/models.rs:21`、`llm/probe.rs:10`、`settings/embedding.rs:13`）。**搭车抓到同族第二对**：`PROBE_TIMEOUT` 也在 `models.rs`/`probe.rs` 各写一份 5s，同批收。
- F4 热路径（审计 M-18 a–g）：审批单次决策 ~4 次会话上下文解析 → `resolve` 内 memo（**不跨调用缓存**，`workspace.rs:39-46` 的「保存即生效」是有意设计）；项目规则 JSON 两读 → 新增 `verdicts()`；每轮 2 次 `LlmConfig` 全量克隆（含明文 Key 串）→ `with_llm(\|cfg\| …)` 闭包访问器 + 合并 `commands.rs` 三连取锁；`turn_id` 每轮全扫 → `rev().find_map` 折衷；`has_call_event` 懒建；`session_history` 锁内深克隆 → 锁内取快照、锁外投影 + 重 IO 命令包 `spawn_blocking`；`append_slice` 每条一次 open/write/close → 单句柄批量。
- ◐ **F5** 结构性拆分（审计 M-19/M-24e + M-4 超限榜首）：**第一项已落（v2.153 批 F-3，证据见 §5 批 F-3 段）**——`init_state` **636 行按子系统拆成八个簇文件**（`state/` 下 `boot` / `llm_stack` / `memory_stack` / `waterfall` / `kernel_stack` / `tools` / `plugins` / `background`），`state/mod.rs` 699 → **220 行**、函数 636 → **195 行**，497 条原语句由脚本证明**零丢失零重排**；计划原文点名的两条隐性契约实测订正 = `sink` 先于 `ApprovalHub` ✅ 与 `catalog` 先于 **`AppLlm`**（原文写「session_window」，磁盘上的真实依赖是模态门控读目录，不是窗口）。**余项未动**：`organize/mod.rs` 621 行拆 trigger/collapse/task；`runner.rs::project_events` 单函数 237 行拆 `runner/projection.rs`；`settings/llm.rs` 682 行拆 view/model（保持 `settings.rs` 的 14 条 `pub use` 不变）；`session.rs` 415 行抽 `session/project.rs`；巨型测试文件按既有先例（`tests_session_log/`）目录式拆分。
- F6 前端重复（C8–C13）：命令包装剥离/工作区相对化两份下沉；相对时间四份统一到 `format.ts`；`v-html` 复制委托两份合并；**弹层「外点关 + Esc」10 处各写一遍 → `use-popover` composable**；`stores/settings.ts` 保存样板 ×9（135 行里约 90 行同构，且 `setTimeout` 无句柄复用是同源小缺陷）。
- F7 `plugin-api` 共享基座（审计 M-1/S9，**本计划里最大的一刀**）：SSRF 防线两插件 361 行逐行同形、`zh_en`/`plugin_version`/`unsafe impl Send/Sync + SAFETY`/官方展示元数据样板各 ×10、`arg_u64` ×6（fs 与 git 两版已轻微漂移）→ 上收 `netsafe`/`args`/`manifest::official` + 宏，合计可减 600–700 行并消灭「同源实现分家后漂移」这类缺陷（v2.133 的 ENV_LOCK 分裂、v2.135 的 gc 序错都是同源产物）。
- ✅ **F8** `plugin-host/tests/` 五个集成测试的 `build_and_stage`/`workspace_dir`/`build_lock` 样板 → 照 `kernel-agent/tests/common/mod.rs` 先例共享。**实测规模（v2.152 批 F-2b）**：计划点名的三件只是代表，真实副本清单是 **11 个夹具函数 / 37 份定义**（`build_lock` ×5、`workspace_dir` ×5、`artifact_name` ×5、`build_and_stage` ×5、`call` ×5、`load_activate` ×4、`env_lock` ×2、`temp_root` ×2、`err_msg` ×2、`echo_error` ×1、`stage` ×1）→ **37 → 0**，收进新 `crates/plugin-host/tests/common/mod.rs`（187 行 / 13 个 `pub fn`）。最直白的证据 = `tools_batch2.rs` 与 `tools_batch3.rs` 的夹具区**除一个前缀字符串外逐字相同**。两处不是简单复制，按形态保留双入口（`load_activate` 两种签名、`echo_error` 借用形与 `err_msg` 取值形），证据见 §5 批 F-2b 段。
- F9 前端超大文件（C16–C19）：locale 拆命名域、`stores/chat.ts` 事件投影下沉纯函数、`types.ts` 拆目录 + 71 个只内部使用的导出收窄、`lab/BuiApprovalCard.vue`（635 行，仅 `LabPanel` 引用的演示夹具）移出生产树。
- F10 `session-store` 侧优化（M-24 a–d）：`pending_slot` 反向全扫 O(n²) → 命中即停；`tool_exposure` 每请求克隆全目录 + 两次序列化求长度 → 逐项累加 + `select` 返回下标；`search_sessions` 每文件读两遍 → 复用 `list_sessions` 单遍。
- ✅ **F11（本批新增，审计未列）测试临时目录/共享目录隔离收敛**：`grep temp_dir()` 全仓 ~20 处写的是 `{前缀}-{tag}-{pid}`——无每次运行 nonce、多数不先清，Windows 复用 pid 时会读到上次残留（批 B 实测撞红一次，机制见 §5 顺带发现）。修法 = 统一收进 `desktop/src-tauri/src/testutil.rs` 的 `tmp_dir(tag)`（pid + 纳秒 + `remove_dir_all` 先清，`memory_files/tests.rs` 已有正确实现可作蓝本），各处删本地副本。同族的还有一处**根 workspace 侧已知偶发红**：`crates/plugin-host/tests/wasm_plugins.rs` 用 `target/locowrk-test/...` 共享目录，并行套件互踩（本批末次全量跑当场抓到、单跑即绿；AGENTS 历批记的「wasm_plugins 已知偶发」就是它）——同一味药：每个测试独占带 nonce 的目录。**登记：`CODE-AUDIT-v1` 完全没有覆盖这一类**（它查的是死代码与重复实现，「测试脚手架脏」要靠真跑门禁才现形）。
  **实际规模与两刀（v2.151 批 F-2a 落 desktop 侧、v2.152 批 F-2b 落根侧）**：计划写的「~20 处」是估计数，HEAD 实测**测试面 44 处散在 36 个文件**（另 3 处是生产码降级兜底数据根，语义无关、明确不收）。desktop 侧已全清（44 → 0，真源 = `testutil.rs` 的 `work_dir` / `unique_dir` 两入口），证据见 §5 批 F-2a 段。根侧 `target/locowrk-test/` 共享目录随批 F-2b 改为 **`target/locowrk-test/{pid}/…`** 分层（`common/mod.rs::run_root`），证据见 §5 批 F-2b 段。**标 ✅ 的诚实前提（三条，全部写进 §5 与 `common/mod.rs` 头注，不在这里粉饰）**：①逐条查过当前调用表，两个测试二进制请求同一 `(plugin, version)` 的情况**并不存在** → pid 分层是**结构性隔离防今后新增重叠组合**，不是「修好了历批记的偶发红」；②批内完整跑抓到一次红（`fs_fetch_plugins` 4 failed / 1 passed，0.80s 内失败），此后 8 轮全量 + 4 轮定向并行全绿、**当时未复现 ⇒ 机制未确证**（这条"未确证"在 v2.153 批 F-3 期间被推翻：满跑抓到 `wasm_plugins` 用例红，报文 `failed to parse WebAssembly module: unexpected end-of-file (at offset 0x0)`，而路径 `target/locowrk-test/{pid}/wasm-ping/0.1.0/wasm_ping.wasm` **带着 pid 分层** ⇒ 病不在跨进程，而在**同一测试二进制内两个用例共用同一个 `(id, version)` 暂存文件**——`fs::copy` 是 truncate + 写，而 `registry.load()` 的读发生在 `build_lock` 之外，并发即读到 0 字节/半截（`offset 0x0` 正是 truncate 后的形状）。当时猜的「跨进程 cargo 产物窗口」不是本病成因；真修法见 **F11c**）；③不凭猜测造跨进程锁（那要单独定口径），替代动作 = 把失败报文补成 `(plugin_id, version, 暂存路径)` 三要素探针。**残留（不属 F11）**：`target/locowrk-test/{pid}/` 随运行次数累积（`cargo clean` 一并清，射程在批 G）。
- ⬜ **F11c（v2.153 批 F-3 期间机制确证后新立）`plugin-host` 暂存路径的同进程共享竞争**：`common::stage()` 的落点是 `run_root()/{pid}/{id}/{version}/{artifact}`——**pid 只隔离进程，不隔离同一测试二进制内的并行用例**。`wasm_plugins.rs` 的两个用例都往同一个 `wasm-ping/0.1.0/wasm_ping.wasm` 做 `fs::copy`（truncate + 写），而 `registry.load()` 的**读发生在 `build_lock` 之外** → 并发时读到 0 字节 / 半截文件，报文正是 `unexpected end-of-file (at offset 0x0)`。native 侧同形状风险面更广（`echo_plugin` / `tools_batch2` / `tools_batch3` / `fs_fetch` 中任何两个用例请求同一 `(id, version)` 都会撞）。**修法（一行级）** = `stage()` 目标再带一次调用级 nonce（`…/{id}/{version}-{nonce}/…`）：`(id, version)` 的**注册语义不变**，磁盘层级对 registry 无意义（它只认工件路径，`plugin_dir` 取其父目录）。**验收判据** = 定向并行 20 轮零红 + 变异验证（把 nonce 摘掉必须能复现红）。**本批未修的理由** = 本刀射程是 desktop 组装点的结构性拆分，这条属 plugin-host 测试面，两类变量不混一刀（AGENTS「一轮只动一类变量」）。风险定性：它造成的是 **CI 偶发红且看起来像真实回归**，代价是排查时间，不是产品正确性。
- **F12（批 E 从 `BUG/bug-report.md` Bug 3 开放面登记）** 报错教学覆盖不齐：`tool-fs` 的 ENOENT 分支（`fs_read`/`fs_grep`/`fs_list` 路径不存在）只透传系统原文，不指 `fs_list` 上级；`tool-shell` 的元字符拒绝（`policy.rs:92`）不指 `fs_grep`/`shell`；`proof.rs` 的 unterminated-quote 走通用 `structural()` 模板（病因可见、出路偏弱，与批 B 登记的 ⑤ 同族）。修法 = 照仓内三条 ✅ 先例补文案（`use fs_list` / `use the shell tool instead` / `copy old_string character-for-character`），零逻辑改动。
- **F13（批 E 从 `market/README.md` 订正登记）** 同一份插件元数据**两条通路**：`market` 的 v2 manifest 硬编码在 `scripts/build_market.sh`（`cat > manifest.json <<EOF`），而 `plugins/` 侧走「源 `manifest.json` + sed 版本号」；`plugins-src/tool-shout/` 根本没有 manifest 源文件。与 F7 同源病（分家后漂移），改元数据只改一处必然脱节。
- ✅ **F14（批 E 顺手发现，零风险小项）** `crates/kernel-bin/src/demo.rs` 的常量名 `V12 = "0.1.0"` / `V13 = "0.2.0"` 来自已作废的 1.2.0/1.3.0 时代（v2.56 版本基准重置的残影）；改名 `V_OLD/V_NEW` 即可，不改行为。**同族一起改了** `demo4.rs` 的 `V13` 与 `plugin-host/tests/echo_plugin.rs` 的 `v12/v13/p12/p13`；`registry/tests.rs` 的 `v12` 值真是 `1.2.0` = 名副其实不动。

### 批 G｜本地产物与环境（不入库，需你点头）

| 项 | 规模 | 动作 |
|---|---|---|
| `desktop/src-tauri/target/` | 23.956 文件 / **41.7 GB** | `cargo clean --manifest-path desktop/src-tauri/Cargo.toml`（代价：下次 desktop 编译从零开始，可能几十分钟） |
| `target/` | 41.085 文件 / **15.2 GB** | 按需 `cargo clean` |
| `tmp/` | 602 文件 / 20.8 MB（v126–v145 走查脚本+截图） | 整目录归档或只留 `v145-walk/` |
| `.mimosa/` | 79 文件（内含 `crates/*` 源码副本） | 可删 |
| 审计留下的取证脚本 | `tmp/audit-*.ps1` ×5、`tmp/paniccheck.rs` | **建议暂时保留**：P0 的复现证据，等本计划全部收口再删 |

两份 `target/` 合计 ≈ 57 GB，是本机最值钱的一项回收。**我不会自己跑 `cargo clean`**——这是不可逆的时间成本，等你明确同意。

---

### 批 H｜工具级取消与 deadline 真接线（D2 后置，需设计稿先行）

批 D 查证后单独立批（不在清理批里造半接线），完整口径见
`crates/plugin-api/src/tool.rs` 的 `CallCtx` 头注。

| 项 | 现状（v2.148 批 D 查证） | 要做的事 |
|---|---|---|
| H-1 `CallCtx::deadline` | **有消费者、无生产者**：`Timeout` 中间件每次读它取小，两个组装点从不写 → 现网恒 `None` | 调用前把「审批剩余 + 全局预算剩余」写进 deadline；与 ⑯/㉘·B 的超时口径对齐 |
| H-2 `CallCtx::cancel/is_cancelled` | **两端都没接**：没有活动调用登记表；用户「停止」只作用于 LLM 流式与重试等待（`StopFlags`，v2.75） | 内核加 `(session_id, call_id)` 活动调用登记表 → 停止请求逐个 cancel → 长任务工具真轮询 |
| H-3 C ABI 无取消通道 | 插件 `call` 是同步 `extern "C"`；`HostVtable` 五槽里没有取消/轮询入口；调用跑在 `spawn_blocking` | vtable **末尾**追加 `call_cancelled: extern "C" fn() -> u32`（append-only，⑭/㉘·A 先例）+ plugin-api 便捷封装 + 一个内置插件真实消费 |
| H-4 文档 | DESIGN §5.2 列了 `deadline / 取消信号` 却没写「未接线」；`plugins.md` 无取消契约 | 接线批同步 §5.2 与插件作者文档（不接线就照批 D 的方式标预留，两不混淆） |

判据提醒：这条批一动就是「行为 + ABI」两面，回归面比批 C/D 宽，需单独门禁 + 实机走查（用户点停止 → 长工具真早停 → 日志如实记 cancelled 而非 timeout）。

---

## 2. 明确不做（沿用审计 §7.8，我复核后无异议）

- 不为清 `Cargo.lock` 的 11 个多版本 crate 强行 `cargo update`（多为传递依赖，回归面大）。
- 不因行数拆高内聚文件（「三问」纪律原文如此；本计划只对混职责的文件给拆点）。
- 不把 `pdfium.dll` / `models.dev/api.json` 移出 git（随包语义已记档，动了破坏离线可用）。
- 不删 `docs/design/ui-prototype/`（仍被设计文档引用，标「历史原型」即可）。
- 不删 `ContextCompacted` 重放链路（历史渲染面，`runner.rs:416-424` 可达）。

---

## 3. 建议执行顺序与理由

1. **批 A**（半天，零裁定，先把用户看得见的 4 个键名和「纯/审批相反」两条 UI 谎报修掉）。
2. **批 B**（门禁；越早越好，批 C–F 每一步都靠它兜住不回归——尤其 B2/B3 的基线必须在大规模删除前先锁，否则「删了多少超限文件」无法自证）。
3. **批 C + 批 D 的裁定项**（清理面；B4 的 `check-i18n.mjs` 正好是 C1 删 59 键的验证器，顺序不能倒）。
4. **批 E**（文档；E1 的 DESIGN.md 与 E4 的 AGENTS.md 瘦身优先级最高，其余可搭车）。
5. **批 F**（收敛与性能，一轮只动一类变量，每子项独立门禁复跑）。
6. **批 G**（等你点头）。

一句话：**A 修谎报，B 装机闸，C/D 扫垃圾，E 让文档不再骗 agent，F 才是真重构。** 前三批就能收掉本次审计的全部「用户可见」与「无人守」两类问题。

---

## 4. 裁定结果（2026-10-10 用户拍板，已入执行队列）

| 编号 | 条目 | 裁定 |
|---|---|---|
| Q1 | 本轮范围 | **先做批 A + 批 B**；C–G 排队，其中已裁定的 C1 / D1 / H2 随批落地 |
| Q2 | H2 `CtxCompact` + `Retry` 未接线 | **删本体**（保留 `CompactionOutcome` 与重放面），`chain.rs`/`mod.rs` 装配注释同步与实际对齐 |
| Q3 | 批 C 删除范围（多选） | **仅勾「删 59 个孤儿 i18n 键」** → 执行；`sandbox_run` 前端 6 处**保留 + 加「历史渲染面」注释**；`assetGc`/`llmListModels`/`llmProbeWindow` 三包装与对应 Rust 命令**保留 + 加「预留/零消费者」注释**（因此 B5 的三向闭合基线不变 = 85↔85↔85） |
| Q4 | H4 `config/profile.toml` | **删文件 + 从 README / DESIGN.md 移除该路径描述**（连带 `kernel-bin/src/main.rs:7` 的「阶段七补 profile.toml」注释同批改） |
| Q6 | H5 `CallCtx` 取消设施 | 未单独问 → 按计划推荐**接线**（公开 ABI 删除属收窄；与 v2.144 `subagent_run` 超时契约同族）。批 D 开工前若你改口再调 |
| Q7 | M-20 `snapshot`/`upcaster` 簇 | 未单独问 → 按计划推荐**标 RESERVED + 注释**（删要撤回一条归档里程碑）。同上 |
| Q8 | 批 G 57 GB `target/` | 未裁定，**不动** |

一句话：**A 修谎报，B 装机闸，C/D 扫垃圾，E 让文档不再骗 agent，F 才是真重构。** 前三批就能收掉本次审计的全部「用户可见」与「无人守」两类问题。

---

## 5. 执行日志（2026-10-11 批 A + 批 B 落地）

### 批 A（10 项，全落）

| # | 落点 | 证据 |
|---|---|---|
| A1 | 补 4 键：`common.close`（zh/en）、`developer.modeMemoryManager` / `developer.modeRule`（zh「记忆管理器 / 规则切分」、en「memory manager / rule split」）、`settings.proxySavedToast`（zh「网络代理已保存」/ en「Network proxy saved」） | `check-i18n.mjs` 报 `zh=953 en=953`（原 949 + 本批 4）且「被引用的键在语言包里不存在」= 0 |
| A2 | `lib/settings-index.ts:136` `settings.panelTitle` → `developer.panelTitle`（⌘K 设置搜索命中恢复） | 反向缺键检查覆盖 searchKeys 数据位，随 A1 一起过 |
| A3 | 删孤儿整文件 `desktop/src/lib/side-effect.ts`（57 行，含退役 `sandbox_run` 分级表） | 删前复核全仓 `side-effect\|toolSideEffect\|\bSideEffect\b` 命中全在文件自身；`pnpm build`（vue-tsc）过 = 无悬空 import |
| A4 | 删 `tool-summary.ts:18` `fs_edit` 陈旧映射 | 删前全仓 grep `fs_edit` 仅此一处命中（插件 manifest / 代码零命中） |
| A5 | `tool-fetch/manifest.json` `fetch.side_effect` Pure→**Irreversible**；`tool-shell/manifest.json` 插件简介与两工具描述同步 v2.145 语义（删「白名单 / denylist 硬锁 / 宿主解锁开关」，补「无命令级禁令 + EXACT_ARGV 免审面」） | 暂存产物盘证：`plugins/tool-fetch/0.1.2/manifest.json` = `Irreversible`；`plugins/tool-shell/0.1.6/manifest.json` grep「白名单」= 0 命中。版本随批 tool-fetch **0.1.2** / tool-shell **0.1.6**（stage 清单已登记理由） |
| A6 | `policy.rs` `split_command` 返回 `Result<_, SplitError::{UnclosedQuote, Empty}>`，两条报文各指各的出路 | 新断言锁「引号不闭合报文含 unclosed quote 且不含 empty」；**变异验证**：把报文改回合并形态 → `structural_denies_remain` 当场红（`got shell command is empty — mutation probe`），精确还原后 `diff` 与备份逐字节一致、无残留、23 测绿 |
| A7 | `browser/tools.rs` 报错示例按 `cfg!(windows)` 分支（非 Windows 不再教模型写 `F:\dir\page.html`） | desktop clippy 首轮抓到多余 `&`（已按 `-D warnings` 修）；desktop 测试内含该路径形态断言 |
| A8 | 删陈旧 `#[allow(dead_code)]`（`AppState.app` 确有消费者：`commands.rs:110` + `scheduler/engine.rs` 六处）；注释订正 6 处（`app_state.rs` compact_cfg→timeout_cfg 类比、`scheduler/{mod,store}.rs` experiences 先例、`kernel-bin/main.rs` profile.toml、`persist.rs` 整理模式透出、`middleware/mod.rs` 三监听器→7 模块 9 处） | 删 allow 后 clippy 不再报 dead_code = 反向证明字段是活的 |
| A9 | 删两条未使用依赖：`kernel-bin` 的 `plugin-service`、`memory-tree` 的 dev `tokio` | 删前逐 crate grep 零命中（`plugin_service` 在 kernel-bin 源码 0 处；无 `#[tokio::test]`）；根 workspace clippy + test 全绿 |
| A10 | 删 `session-store/src/session.rs` 尾部冗余 re-export + 其 `#[allow(unused_imports)]` | `lib.rs:22` 已从 `event` 直接导出 `TurnId/TurnOutcome`；根 workspace 39 套件绿 |

### 批 B（9 项，全落）



| # | 落点 | 证据 |
|---|---|---|
| B1 | `.github/workflows/ci.yml` 重写为三作业：**kernel**（根 workspace）/ **desktop**（`windows-latest`：前端 build 供 `generate_context!` 吃 dist + lint:style + desktop fmt/clippy/test）/ **hygiene**（四道脚本闸） | 本机不能跑 Actions——**实跑结果待你 push 后确认**（诚实标注）。desktop 三项在 Windows runner 上是首次成文，`Swatinem/rust-cache` 指到 `desktop/src-tauri` 工作区 |
| B2 | `check_length.sh` 由「恒 exit 0 的报告」改为**基线比较**（`BASELINE=40`），超限才列清单并判红；pre-commit 的 `... \|\| exit 1` 自此真的可拦 | 现状实测 `40 == 基线`，exit 0；**变异验证**：塞一个 401 行文件 → 判红并点名该文件，删除后回绿、零残留 |
| B3 | 覆盖面扩两处：`plugins-src/`（+2）与 `desktop/src/**/*.ts`（+6：locales 1157/1139、chat.ts 1064、types.ts 1035、api.ts 764、tool-detail.ts 622——长期在门禁盲区）；同时把逐文件 `wc -l` 改批量 `xargs`（整闸从 ~15s 降到 2.3s，pre-commit 可用） | 基线 32→40 的账目逐项可复核（上面明细即来源） |
| B4 | 新 `scripts/check-i18n.mjs`：① zh/en 键集合双向相等 ② 反向缺键（`t()/$t()/te()/tm()/tr()` 字面量 + settings-index 的 labelKey/searchKeys）③ 无静态引用键报告（动态前缀整族豁免）。解析器写成 **token 级状态机**——审计里正则版把 prettier 折行误判成「zh 有 en 无」18 条，这道的首要设计约束就是不再犯 | **变异验证 ×2**：只在 zh 加一键 → `只在 zh：common.mutationOnlyZh` 判红；引用 `settings.proxySavedToastX` → `缺：…` 判红；两次都 sha 逐字节还原、无残留、回绿 |
| B5 | 新 `scripts/check-commands.mjs`：`#[tauri::command]` 定义 ↔ `generate_handler!` 注册 ↔ 前端 `invoke`，三向差集 + `RESERVED_NO_FRONTEND` 显式预留名单 | 本机复算 **定义=85 · 注册=85 · 前端引用=85**（与审计人工结论互证）；**变异验证**：注册表塞一条不存在的命令 → 同时报「注册了但没有定义」与「后端有命令、前端无入口」，sha 还原后回绿 |
| B6 | `cargo machete` 进 kernel 作业，首版标 `continue-on-error: true`（本机未能预验该工具在 CI 上的行为，跑绿一次后改成硬门） | 注释里写明这是 advisory，不当已成立的门禁卖 |
| B7 | 三处死机架：`check-style.mjs` 的 warns 分支保留但注明「当前无 warn 级规则，这是观察期机架」；`.gitignore` 删被 `plugins/*` 完全覆盖的 dll/so/dylib 三行并留「为何删除、何时加回」说明；`.zcodeignore` 同步同一改动 + 补 `docs/issue/` | `git check-ignore -v` 复测：dll 仍由 `plugins/*` 命中、market dll 由自身规则命中、`plugins/README.md` 与 `market/` 跟踪文件零变化；`git diff` 两文件只有我的 8/11 行（无整文件换行 churn） |
| B8 | `build_plugins.sh` 暂存幂等化：`cp -R assets` 前先 `rm -rf "$dst/assets"`（旧版每次运行叠一层 `assets/assets/`，M-5 根因）；顺带把收尾文案改为由 stage 清单生成（M-12/E7：旧文案硬编码版本号已经写出自相矛盾） | **连跑两次**：当前 stage 的 17 个版本目录 `assets/assets` = **0**（修复前 27/60 份图标错位）；历史遗留的 13 个旧版本目录（不再 stage）仍带嵌套，属本地产物不入库；新文案实测输出 `tool-shell {0.1.3,0.1.4,0.1.5,0.1.6}` 等分组正确 |
| B9 | 新 `scripts/check-middleware-wiring.mjs`：枚举 `impl Middleware / WaterfallListener` 的类型 → 在生产代码（跳过 tests 文件与 `#[cfg(test)]` 段）找构造点 → 未接线必须显式进 `KNOWN_UNWIRED` 并写理由；白名单条目一旦接上或类型消失 → 反向判红（双向不漂） | 本机跑 = 监听器实现 12 · 已接线 10 · 未接线 2，**独立复现了审计 H2 的 CtxCompact + Retry**（脚本自己找出来的，不是抄结论）；白名单暂登这两条并注明「批 C 删除后清空」；**变异验证**：摘掉 CtxCompact 条目 → 「未登记白名单」判红；把 ToolExposure 假登进白名单 → 「已过期」判红 |

### 顺带发现（登记，未修）

- `plugins-src/tool-shell/src/proof.rs:102-104` 的 `shell_read` 侧引号不闭合报文走通用 `structural()` 模板，正文讲的主要是管道/重定向，「补上右引号」这条出路只在开头的 `unterminated quote` 字样里——与 A6 同一族但轻得多（模型至少看见病因）。改它要动 `proof/tests.rs:74-75` 的断言口径，留给批 F。
- 本批未做**实机 CDP 走查截图**（H3b 四条裸键名的中/英文界面目检）。机制层证据 = `check-i18n.mjs` 的引用-存在性双向闭合 + `pnpm build` 过；要看图的的话我再起一轮走查（4 个位置：代理保存 toast、记忆树观测面板模式列、导入弹窗关闭钮无障碍名）。
- **审计没抓到的一类真 flake（本批自己撞出来的）**：`desktop/src-tauri/src/tests_session_log::truncate_after_resolves_pos_and_rewrites_disk` 满跑红、单跑绿——`tmp_dir(tag)` 目录名只带 `pid`、不带每次运行的 nonce、也不先清，而 Windows 会**回收复用 pid**，于是本模块读到上一次运行残留的 jsonl：`.undo.bak` 读出 6 条（期望 4）。这不是断言写错，是测试脚手架脏。本批已按 `memory_files/tests.rs` 先例修掉这一处（加纳秒 + `remove_dir_all` 先清）。全仓同形态的还有 ~20 处（`grep temp_dir()` 里只带 pid 不带 nanos 的：`assets/`、`llm/`、`media/`、`organize/assemble/`、`notifications/`、`browser/` 等）= 一整类偶发红的地雷，**列入批 F（F11：桌面测试临时目录统一收进 `testutil::tmp_dir(tag)` 单点实现）**，不在本批扩大改动面。


### 门禁终态（最后一次编辑之后全量复跑，2026-10-11）

```
根 cargo fmt --all --check            CLEAN
根 cargo clippy --workspace --all-targets -D warnings   0
根 cargo test --workspace             39 套件 · 443 passed · 0 FAILED
desktop cargo fmt --check             CLEAN
desktop cargo clippy --all-targets    0
desktop cargo test                    356 passed（与 v2.145 持平，零回归）
desktop pnpm build（vue-tsc 0 + vite）过
desktop pnpm run lint:style           0
check_length.sh     40 == 基线 40（零新增）
check-i18n.mjs      zh=953 en=953 · 引用键 723 + 索引键 49 全存在（缺键 0）· 无引用键 62（报告）
check-commands.mjs  定义=85 · 注册=85 · 前端引用=85（三向零差集）
check-middleware-wiring.mjs  实现=12 · 已接线=10 · 未接线=2（均在 KNOWN_UNWIRED）
```

一次诚实记录：末次全量跑首轮**判红**，两条红都不是本批改动造成的——① `tests_session_log` 的临时目录读到上次运行残留（已修，见上方「顺带发现」）；② `plugin-host/tests/wasm_plugins` 并行偶发红（历批已记，单跑 2/2 绿）。复跑全绿后才有上面这组数；`wasm_plugins` 的共享目录问题与①同族，一并登记 F11。

另：孤儿键口径在批内收紧了两次（133 → 62）。第一轮虚高是因为只扫 `t()` 调用点，漏掉 `PermissionCard.vue:64` 这种「先 `const path = \`approval.sensitive.${key}\`` 拼串、再传进 `t()`」的形态，把 11 条活文案误报成孤儿——**假孤儿比漏报危险**（人会照着清单删掉正在用的文案）。收紧后 62 条与台账「确证 50 + 待确认 9」基本对齐，批 C 执行时取两者交集、并优先信任台账（实际对账结果见下批 C 段：闸是台账的超集，多出的只有 2 个后批新产生的孤儿）。

### 批 C（2026-10-11，H2 摘本体 + C1/C2/C3 全落）

**C0 = Q2 裁定兑现：`CtxCompact` + `Retry` 摘本体**

| 落点 | 证据 |
|---|---|
| 删 `middleware/retry.rs`（52 行）+ `mod retry`/再导出；`compact.rs` 332→56 行；删其测试 `compact/tests/mod.rs` 405 + `tests/stability.rs` 104 行 | 净差 = `git diff --stat` 本批 **−1140 / +113**（另新建 `compact/tests.rs` 116 行未入 diff 统计）；死码面合计摘掉 **893 行** |
| `CompactionOutcome` + `estimate_tokens/estimate_one` 保留 | 新 `compact/tests.rs` 四测锁住：尺子口径（序列化字节 ÷4 精确等式 + 媒体部件平铺）/ CJK 低估是记档行为 / **旧日志载荷字面 JSON 读得进 + 字段名回序列化**（重放面不能漂）。平铺断言首轮就被自己抓红：`estimate_tokens` 的增量是「平铺值 + 部件自身 JSON 字节」= 实测 1046 而非 1024，写死 1024 是假断言 → 改成刻度层用 `est_tokens()` 精确锁 1024/1000/0，集成层锁「≥ 平铺值且不超出一个量级」 |
| 陈旧注释逐处订正（审计 H2 的「注释宣称有、实现里没有」） | `chain.rs` 装配序改真实（Tracing→Audit→Timeout→CircuitBreaker→审批门→Tool，并注明 Retry 从未注册）；`middleware/mod.rs` 头注重写；`lib.rs` 再导出瘦身（`CompactConfig/CompactStrategy/suggested_threshold/CtxCompact/Retry` 摘出）；`memory_inject.rs` 删虚假「headless 保留」；`waterfall.rs`/`kernel/request.rs` 的 `compacted`/`tree_directory` 由「ctx-compact 写回」改为**契约面**措辞（现网无生产写入点 ≠ 契约失效）；`timeout.rs` 的 `CompactConfig 同模式`→`ToolExposureConfig`；`fragments.rs` 同步 |
| **有意偏离审计 M-22** | `WaterfallDecision::Retry` / `RetryBudget` / `Kernel::with_retry_budget` **保留**——它们是 `agent/request-error` 重试环的**活机制**（`kernel/request.rs:211` 消费 + `tests/waterfall.rs` 两测在跑），与被删的洋葱件 `Retry` 同名不同物。审计把两者并列为「未接线」是名称误判，此处按查证结果处理 |
| 接线闸白名单清空 | `KNOWN_UNWIRED` 两条删除 → 现跑 = **实现 10 · 已接线 10 · 未接线 0 · 白名单 0**；**变异验证**（新形态=空清单必须仍能抓鬼）：临时落一个无构造点的 `MutateProbe` 监听器文件 → 当场判红「有定义、无生产构造点，且未登记白名单」→ 删探针复绿 → `grep MutateProbe` 全仓 = 0 残留 |
| 测试账 | 根 workspace 443 → **432** passed：删 12（旧 compact 测）+ 2（Retry 测）+ 1（`kernel/tests.rs` 的 `tree_session_skips_compaction_over_threshold`，唯一依赖 `CtxCompact::new` 的内核测）= 15，增 4（新 compact 测）→ 净 −11，逐数对得上，无「测试静默消失」 |

**C1 = 62 个孤儿 i18n 键删除（zh/en 共 134 条死文案）**

- 与台账对账（机械比对，不靠目测）：闸内 62 键 ⊇ 台账列名的 49 键（`*` 待确认 9 条叶名全部逐条复核）；闸比台账**多出的只有 2 个新孤儿** = `chat.newSession`（SessionSwitcher 退役后遗留，现文案走 `sidebar.newTask`）与 `turn.thinkingFor`（ThinkingRow 实际用 `tool.thinkingNow` + `tool.type.thinking`）；余差是台账自身的折叠写法（`subagent.region*`、`status.* 中的 ready`）与正文 59 的计数口径，**以闸为准**。
- 逐条复核后删：`subagent.*` 与 `error.*` 两个命名空间整体无引用 → 整块收敛（连带删除上方只为它们存在的 `// 子 Agent 卡（§3.10）` 注释行，zh/en 各一行）；`sidebar.menu.moveInto/moveOut` + `toast.moved*/associateFailed` 的死因有源可查——`SessionRow.vue:63` 记着 v4.11 用户裁定「移除移入其它项目」，功能先退役、文案后落灰，删它是对的。
- **假孤儿防线**（本批最要命的风险）：删除器的判据 = 闸的三条豁免（`t()` 字面量 / 源码任何带引号点分字面量 / 动态前缀整族）全不成立。动态族逐条手查：源码里全部 `t(\`…\`)` 调用点 = `idle. memory.time. market. notice.action. perm.short. perm.desc. tool.type. nav. chat.greeting. chat.greetingBare. effect.`，**没有一个被删键的前缀**，故无动态成员被动过。
- 工具面：写删除器时先用行级状态机，`en.ts` 里 `"...the provider's default endpoint"` 这种**折行值续行含裸撇号**被误判成未闭合字符串、整块结构错位 → 26 键「清单有但文件没有」。假孤儿风险当场暴露，改为与 `check-i18n.mjs` 同一套**字符级扫描器**（记行区间 + 键行起删，prettier 折行不误伤），两文件各自解析出 953 叶键 = 与门禁一致后才允许落盘。
- 删除器只删「键自身行 + 子键全删光的空块」，其余字节不动 → `git diff --stat` = 两文件各 **−67 行、零改写行**，diff 可直接审。
- 删后：`check-i18n.mjs` = **zh=891 en=891 · 缺键 0 · 无静态引用键 0**（双向差集仍为 0）+ `pnpm build`（vue-tsc）过 + `lint:style`/`check-style.mjs` 0。

**C2/C3 = 按裁定「保留 + 加注释」**

- C2 `sandbox_run` 前端 6 处：`tool-summary.ts:14` 与 `tool-detail.ts:62` 的「历史渲染面」注释在批 A+B 已落（本批复核确认在位），三处 `shell / shell_read / sandbox_run` 共用分支与 §333 文档行同属该面。
- C3 三个死 API 包装：`api.ts` 的 `assetGc` / `llmListModels` / `llmProbeWindow` 各补「**预留·零消费者**」注释（写清退役必须连 Rust 命令同批，否则 `check-commands.mjs` 三向闭合当场判红）+ `types.ts` 的 `ProviderModels` 标注唯一消费方。**零消费者是跑出来的**：`grep assetGc|llmListModels|llmProbeWindow` 全 `desktop/src` 命中数 = 1/1/1（全在自身定义行）；Rust 侧 `probe::` 唯一消费点 = `commands.rs:371` 那条命令本身。
- **纠正 v2.146 的一条过度声明**：ROADMAP ⑤ 当时写「三包装 = 保留并加注释」，实际只有 `tool-summary/tool-detail` 两处注释落了，`api.ts`/`types.ts` 的四条是本批才补上。文档领先于代码一格，登记在此。
- 闸数不变：`check-commands` 定义=85 · 注册=85 · 前端引用=85（承诺兑现）。

**顺带（登记批 E）**：`docs/上下文与记忆工作流程报告.md:26,75` 仍把 `CtxCompact` 列为流程第 ⑤ 步（现该本体已摘），属「文档描述现役机制」类失真，随 E 批真实化处理，本批不改文档。

**批 C 门禁（最后一次编辑之后全量复跑，2026-10-11）**

```
根 cargo fmt --all --check            CLEAN
根 cargo clippy --workspace --all-targets -D warnings   0
根 cargo test --workspace             39 套件 · 432 passed · 0 FAILED（对账见上）
desktop cargo fmt --check             CLEAN
desktop cargo clippy --all-targets -D warnings   0
desktop cargo test                    356 passed（与 v2.145/2.146 持平，零回归）
desktop pnpm build（vue-tsc 0 + vite）过
desktop pnpm run lint:style           0 ·  check-style.mjs 91 文件干净
check_length.sh     39 == 基线 39（本批净降 1：旧 compact 测试 405 行超限文件随葬 → BASELINE 随批收紧 40→39）
check-i18n.mjs      zh=891 en=891 · 缺键 0 · 无静态引用键 0
check-commands.mjs  85 · 85 · 85
check-middleware-wiring.mjs  实现=10 · 已接线=10 · 未接线=0 · 白名单=0
```

本批为纯代码/门禁面 + 死文案删除，**未做实机 CDP 走查**：判据 = 静态引用面（`t()` 字面量/索引键/动态前缀三路闭合）+ `vue-tsc` 构建 + 全量测试。要肉眼确认界面没有裸键名回潮，需另起一轮走查（列入批 V 验收）。

### 批 D（2026-10-11 落地，v2.148；D2 转批 H）

| # | 落点 | 证据（删除类一律给「改前 grep = 改后 grep」） |
|---|---|---|
| D1 | `config/profile.toml` **删文件**（`git rm`，7 行全注释零生效键）+ 文档描述移除：README 目录表一行、AGENTS 目录结构一行、DESIGN §8 目录树（`├── plugins/` 改 `└── plugins/` 保 ASCII 树正确）+ 树下更正注、§11.3 开发态/安装态段按真实机制重写（dev_scan + lockfile）、§末 backlog 一行撤销、ROADMAP backlog 行标 ❌ 已撤销、`kernel-bin/src/main.rs` 头注改完成时 | 改前 `grep -rn profile.toml` = README 1 / DESIGN 4 / ROADMAP 1 / AGENTS 2 / kernel-bin 1；改后 = **0**（历史批次日志与本计划、审计报告原文除外）。`ls config` = No such file or directory |
| D2 | **改判：不在清理批里接线**。`CallCtx` 头注标 RESERVED 并写清两处硬事实：`deadline` = 有消费者（`Timeout` 中间件）无生产者（现网恒 `None`）；`cancel/is_cancelled` = 两端都没接，且**取消信号过不去 C ABI**（`call` 同步 `extern "C"`、`HostVtable` 五槽无取消入口、调用跑在 `spawn_blocking`）。真接线立 **批 H**（§1 新增四行表） | 查证链：`grep "is_cancelled\|\.cancel()"` = 只有 tool.rs 定义行，零调用方；`HostVtable` 逐槽读实（log/kv_get/kv_put/notify/session_context）。计划原先「倾向接线」被这条边界证据推翻——只接宿主侧 = 第三方与 wasm 插件照样收不到 = 再造一处「实现只到一半」，正是本轮审计在罚的形态 |
| D3 | 磁盘快照 + upcaster 簇**保留并标 RESERVED**：`snapshot.rs` 头注重写（§9.5 与设计纪律 5 明写的机制、实现与测试在位、**磁盘面从未接**、恢复真源 = `persist.rs` 逐行回放、接上那天要做的三件事、⚠ 与内存态 `Session.snapshots` 区分） | `grep "from_snapshot\|snapshot_at\|V1ToV2Upcaster\|Upcaster"` 改后命中 = 定义 / re-export / 测试，生产调用点 **0**（与审计一致）；`Session.snapshots` 在 `session.rs:309/354/387` 有读写 = 活的，未碰 |
| D4 | cordis-core `parallel` + `serial` 两簇**保留并标 RESERVED**：`event_bus.rs` 头注写清「emit/waterfall 活的、parallel/serial 零生产调用点、审计链实际落 waterfall」；DESIGN §3.5 表加「现网接线」列（四行逐条 ✅/RESERVED）+ 表下注 | `grep "\.parallel(\|\.serial(\|on_serial"` 全仓命中 = `event_bus/tests.rs`（`serial_stops_at_first_veto` 等 4 测），生产 0。改动依据 = 该簇自述用途已被 `middleware/audit.rs` 头注逐条说明（落 waterfall） |
| D5 | `host_kv_put` **语义诚实化**：能力未定即如实回 `ERR_UNSUPPORTED`（不再写进读不回来的 `KV`），`static KV` 与 `LazyLock/HashMap` 导入随删；`ERR_UNKNOWN_TOOL = -2` **标 RESERVED**（写明本该谁返回、现网两端都没返回、未知工具实际走 `-3` + JSON 信封；数值槽位不删 = ABI 不重排） | 改前 `host_kv_put` 回 `code::OK` + 测试 `kv_put_stores_value` 断言 `KV` 有值 = **谎报成功**（get 恒 UNSUPPORTED）；改后测试改名 `kv_put_is_unsupported_until_ownership_protocol_lands`，断言 rc = `ERR_UNSUPPORTED`（null 参仍 `ERR_BAD_ARGS`）。plugin-api 35 测 + plugin-host 全 FFI/wasm 集成套件绿 = 无真实插件依赖 kv 写成功 |
| D6 | 仅自家测试消费的 pub 面逐条分判：`runtime::deps_of` **删**（零消费者）、`lockfile::clear_pending_delete` **删**（与 `startup_reconcile` 同语义两份实现，留两入口迟早分叉）、`registry::graveyard_len` **降 `#[cfg(test)]`**、`kernel::with_retry_budget` **保留 + 注明「活机制唯一注入口」**、cordis-core 四个运行态观测方法 **保留 + RESERVED**（§11.6 明写的 UI 投影面） | `grep "\bdeps_of("` 改前 = 仅定义行 → 改后 0；`clear_pending_delete` 改前 = 定义 + `lockfile/tests.rs:81`（断言随葬，追加/去重与清理路径各自已有测试覆盖）。`graveyard_len` 若只降 `pub(crate)` 会在无 cfg(test) 的 lib 构建触发 `dead_code`（实测），故用 `#[cfg(test)]` 才是干净收口 |
| D7 | `TriggerInput.anchor_source` 幻影字段**删**（只写不读；`AnchorSource` 枚举本身留 = 桌面观测面板在读）+ `valid_session_id` **下沉新模块 `plugin_api::naming` 单点定义**，两处本地实现删除，`memory-tree` 的 plugin-api 从 dev 依赖升为正式依赖 | 判据 = 两侧各加一条 `dotted_session_id_round_trips` 回归锁 + plugin-api 两条规则边界测（`..`/全点/65 字符/中文/冒号）。**变异验证**：规则改回「不认点号」→ plugin-api `real_id_shapes_are_accepted` + session-store + memory-tree **三处齐红**；逐字节还原（sha 比对一致）→ 复绿、零残留。分叉的实际后果照审计原文登记：含 `.` 的 id 日志落盘成功而树投影静默失效（现网 `desktop-<毫秒>` 恰好两边都合法 = 陷阱非现网缺陷） |
| D8 | `cordis-core → plugin-api` 依赖**不动**，在 `Cargo.toml` 依赖行上方写清「设计约束非冗余」并指向 §8 与 CI `cargo machete` advisory（防将来顺手删） | 查证 = `grep plugin_api crates/cordis-core/src` 命中 **0**，而 DESIGN:1266 依赖图明写要求；删它必须同批改 §8 + AGENTS 架构边界 = 设计决策，不是垃圾清理 |

顺带（登记，未修）：`memory-tree/src/storage.rs::fnv1a` 与 `plugin_api::memlayout::fnv1a64` 是同一算法两份实现（`scope_dir` 与 `project_dir_name` 亦同族重复）= 批 F「重复实现收敛」类，本轮不动（一轮只动一类变量）。

**批 D 门禁（代码最后一次编辑后全量复跑，2026-10-11）**

```
根 cargo fmt --all --check CLEAN · clippy --workspace --all-targets -D warnings 0
根 cargo test --workspace 39 套件 · 436 passed · 0 FAILED
   （432 + naming 2 + session-store 1 + memory-tree 1 = 436，与批 C 收尾数逐数对上）
desktop fmt --check CLEAN · clippy --all-targets 0 · cargo test 356 passed（持平）
check_length 39 == 基线 39 · check-middleware-wiring 10·10·0·白名单 0
check-i18n zh=891 en=891 · check-commands 85·85·85（本批未碰前端）
```

已知边界：①本批未跑 `pnpm build`/`lint:style`（无 `.ts/.vue` 改动，提交时按 pre-commit 暂存面自动跳过）；②D2 的接线口径（批 H）**用户尚未表态**，我按「不造半接线」的最低风险路径先做诚实化 + 立批，改主意随时可转；③`kv_put` 如实回 UNSUPPORTED 后，真有插件要用 kv 仍需先定缓冲区所有权协议（§15.1 末尾追加 `kv_free` 槽的路径不变）。

### 批 E（2026-10-11 落地，v2.149；E1–E8 全落 + 三条新登记）

**这一批的性质与 A–D 不同**：A–D 改代码，E 改的是「agent 读进去就照着写」的那一层。审计 H9 的判据是「文档领先/落后代码一格」，而本仓的文档里 **DESIGN.md 是 AGENTS.md 明写的「改核心模块前先读」对象**——它 0 处 v2.145 就意味着：下一个会话照它实现，会重新造出 denylist、`LOCOWRK_SHELL_ALLOW`、`sandbox_run` 和一整套五序决策表。所以 E 批的产出不看代码行数，看**「文档里还有多少句在描述已不存在的机制」**。

| # | 落点 | 证据（改前 → 改后） |
|---|---|---|
| E1 | `DESIGN.md` 五处规范正文按代码重写：§7.1 洋葱链（原写「本阶段实装 Tracing → Timeout → **Retry** → CircuitBreaker」→ 真实链 + 批 C 更正块 + **新增 waterfall 七监听器注册序段**）· §7.2 决策表补 ③项目 deny / ⑤项目 allow 重排七序 + row2 删 `sandbox_run` + v2.145 修订注（逐序抄自 `approval.rs::resolve`，含「`rule:project-allow` 不带 `mode:` 前缀 = 前端照常渲染记录条」这种细节）· §8 目录树删 `config/` 块 · §11.3 开发态/安装态重写为 dev_scan + lockfile · §3.5 表加「现网接线」列 · 实现备注 tool-shell / ctx-compact 各加 ⚠ 修订注 | 改前 `grep -c v2.145 DESIGN.md` = **0** → 改后 **6**（另 v2.147 = 4、v2.148 = 5）；`sandbox_run` 现存 2 处、`LOCOWRK_SHELL_ALLOW` 3 处、`denylist` 2 处**全部落在「已退役」语境的修订注内**（`:1173/:1183/:2418/:2424/:2431` 逐条眼验），零处当作现役机制陈述 |
| E2 | `plugins/README.md` 整表重写：件数「八件」→ **9 个 id**（`plugins-src/` 实有 10 目录，`tool-shout` 只进 `market/`）；工具名点号 → 下划线；1.x → 最新暂存版本（真源 = `build_plugins.sh` 的 `stage` 清单）；`fetch` Pure → **Irreversible**；`shell` 「白名单 + denylist」→ v2.145 两态；`side_effect` 的 Compensable 写成实际形状 `{kind, undo_tool}`；**新列「宿主内置工具不在本表」**（`search_tools`/`memory_expand`/`subagent_run`/`browser_*` 等走组装点，不经 `plugins/`）；安装态数据根补 `LOCOWRK_DATA_DIR` 口径 | 改前表内 `1.0.0/1.1.0/1.2.0` 字面 = 8 处、点号工具名 = 6 处 → 改后**全 0**；表逐格与 `plugins-src/*/manifest.json` + `stage` 清单机器对账（node 解析 manifest 打印 `name/side_effect/exposure` 全表核过，非目测） |
| E3 | `UI shell and patterns.md` 四处现状表**只加注不改正文**（它是规范文档，正文是当时的规格）：`:193` MemorySection 行 → 点明 autoExtract/handoffUpdate/experiences（v2.136）、defaultOrganizer（v2.137）四键 = 幻影键 + 列出**现役 memory 区键集**；`:202` DeveloperSection 行 → `context.stabilityRounds`、`developer.defaultOrganizer` 已退役；`:883`/`:999` `session_set_organizer` → 命令已随 v2.137 删除，本行保留为历史落点 | 三处「已退役」判据不靠记忆：`grep -rn session_set_organizer desktop/` = **0**、`stabilityRounds`/`defaultOrganizer` 只存在于 `settings/*` 的幻影键注释与 `settings/tests/memory.rs:351-369` 幻影回归锁内 |
| E4 | `AGENTS.md` 瘦身（**本批最直接改善 agent 体验的一项**）：32 条历史流水**逐字原样**搬入 `docs/design/archive/AGENTS-progress-archive.md`（零改写，脚本按 `git show HEAD:AGENTS.md` 行区间切出）；本文件只留「当前进度 + 前一版 + 归档指针」，并写死新的**写入纪律**（保留两条、每条 ≤2,000 字节、被挤出的整条搬走不留残段）。同批订正本文件内五处失真：**「定位」行**（原「模型适配器、工具、会话存储、**沙箱**、Agent 循环本身都是插件」→ 沙箱已退役 + LLM 是宿主内置服务 + "一切皆插件"约束的是无特权模块不是打包方式）、**权威文档行**（新增「动手前查 INDEX.md 分清权威/已兑现/存档」并说明该纪律的来源就是 H9）、**目录结构块**（补 `memory-tree` crate、`INDEX.md`、`NEW/`·`BUG/`·`archive/`、`scripts/`；`LLM.md` 描述从「三合一 genai 适配器」改为单线协议实况）、**常用命令块**（补齐批 B 立的四道结构闸 + desktop 三件 + 前端三件，demo 叙事的版本号 1.2.0/1.3.0 → **0.1.0/0.2.0** 与 `stage` 清单一致）、**设计纪律 4/5/6 三条就地加实况注**（④ 桌面是**每 step 批量落盘**不是 per-event，悬空只在 mid-flush 出现；⑤ 磁盘快照从未接线 = RESERVED；⑥ `Irreversible` 由「必须审批」改「**默认**审批」并列明五条免审路径 —— 这条不改就是让下一个 agent 以为弹卡是硬保证）；`check_length.sh` 措辞从「报告不拦截」改「基线比较闸」 | `wc -c AGENTS.md`：**206,707 → 22,080 字节（−89.3%）**；归档文件 195,039 字节，**逐字校验 = 33 行 vs `git show HEAD:AGENTS.md` 行区间，mismatch 0**；会话注入预算 65,536 字节此前被历史流水吃满，现在整份文件在预算内**且现行纪律全部可读** |
| E4b | **审计建议未采纳的一项（如实登记，不是遗漏）**：`CHANGELOG.md`（186 行 / 418 KB，每版一整行）**不拆成 `docs/design/CHANGELOG/` 目录**。理由三条：①它不进会话注入源（不是 AGENTS.md），拆分没有 E4 那种「挤爆预算」的收益；②150 个单文件目录 + 全仓几十处 `docs/design/CHANGELOG.md` 引用改写 = 纯迁移成本；③检索手段已经够用（`grep -n "^\*文档版本：v2.14" CHANGELOG.md` 一次命中）。改为在 `INDEX.md` 里把它标为「逐批落点唯一真源 + 检索口径」。**如你仍要拆，说一声我单独做一批** | 未做，登记待裁定 |
| E5 | `CHANGELOG.md` 撞号修实：`v2.75` 被「LLM 兼容层重构」（2026-10-05）与「前端统一标准 + 机械门禁」（2026-10-06）两条互不相关的批共用。处置 = **LLM 条条号沿用不动**（它承载全仓 40+ 处按号引用），前端标准条改占空号 **v2.76** 并从 v2.72 之后移到 v2.75 之前（位序按日期归正），两条各自加订正说明 | 改前 `grep -c "^\\*文档版本：v2.75（"` = **2** → 改后 **1**（v2.76 = 1）；改号前全仓 `grep -rn v2.75 docs AGENTS.md` 复算 = 47 命中，逐条判读**除被改的那条外全部指 LLM 兼容层批**，且前端标准批在其它文档一律按日期（「用户 2026-10-06 裁定」）而非按号引用 = **改号零牵动**；`git diff --numstat` = 3/3（只动三行，未波及其它 183 行） |
| E6 | `docs/design/Sandbox Execution Design.md` → `docs/design/archive/`（`git mv` 保留历史）+ 首行三行定性：**从未采纳从未实施**（全仓零引用、无 CubeSandbox 代码或依赖）+ **主题已整体退役**（实际沙箱路线 = v2.31 受限令牌 + JobObject，而该插件本身又已于 v2.145 删除）+ 现役安全面指路四文件；结尾一句「照本稿实现会造出与现网相反的东西」。**新立 `docs/design/INDEX.md`**：七节分类（权威规范 / 已兑现设计稿 / 待研究 / 对照留档 / 存档·未采纳 / 工单与台账 / 目录约定），逐篇标状态与兑现批次，含六词状态词汇表与「新建或退役 md 同批改本表」的维护纪律 | 该文档 761 行（**审计记 575 = 数字失实，按实测登记**），`grep -rln "Sandbox Execution Design"` 改前命中 = 仅 `docs/reports/` 三份审计文档、正文零引用；INDEX.md 里全部本地链接过自检脚本 = **broken 0**（两条剩余命中是审计报告里的正则代码片段 `[a-z0-9_]+` 被误当链接，非真链）；`icon replacement plan.md` 顺手查实**未采纳**（`grep -ri tabler desktop/src` = 0，现役 = 自绘 `lib/icons.ts`）并在 INDEX 标出 |
| E7 | 版本注释收敛（审计 M-12）：`plugins-src/{memory-sqlite,tool-edit,tool-fetch,tool-fs,tool-git,tool-notify,tool-shell}/Cargo.toml` 七条「build_plugins.sh 按 **1.0.0** 构建一次并暂存」+ `tool-echo`「按 **1.2.0 / 1.3.0** 各构建一次」+ `tool-shout`「build_market.sh 按 **1.0.0 / 1.1.0**」→ 统一改「**版本真源 = 脚本内 stage 调用清单**，此处不写死版本号」。`build_plugins.sh` 的收尾文案已在批 B 由 stage 清单生成，本批不再二次维护 | `grep -rn "1\\.0\\.0\\|1\\.2\\.0\\|1\\.3\\.0\\|1\\.1\\.0" plugins-src/*/Cargo.toml`：**9 → 0**；9 个文件各 1 行改动（`git diff --numstat` 逐文件 = 1/1），零行为改动 |
| E8 | README 与工单真实化：`README.md` 从 25 行「当前状态 = 阶段一（内核骨架 + 事件日志）」重写为 84 行现役面貌（版本 / 可用形态 / 规模参考 / 八 crate 结构 / 两套门禁命令 / 文档入口），并把「一切皆插件」的定性按 v2.75 架构裁定与 v2.145 沙箱退役改写；`market/README.md` 的 `manifest.json` 形状从 v1 三键 `{id,version,description}` 订正为 **packaging v2 全量**（并点明它硬编码在 `build_market.sh` 里、与 `plugins/` 侧「源 manifest + sed」是两条通路 → 登记 F13），版本基准偏差（夹具仍 1.x vs「全部插件 0.1.0 起步」）**登记待裁定不擅自动产物**；`BUG/Tool Layer Issues Todo List.md` 状态行「待处理」→「已处理（v2.116），唯余 shell 跨平台策略 ⏸」（它与自家 §六 处理记录自相矛盾）；`BUG/bug-report.md` 的 Bug 3 从「🆕 待上报」→「◐ 部分处理」并列明哪一支已修（v2.146 M-10 拆开引号不闭合 vs 空命令）、哪两支仍开放（ENOENT 不指 `fs_list`、元字符不指 `fs_grep`）→ 登记 F12；`plugins.md` **§一 加「现役状态」整列**（19 行逐项标 ✅/🔧/◐/❌/⬜ 并给依据），§二/§三/§四/§六 四处退役件加就地注（含 §四 表里「挂载点 `serial` on `tool/call`」这条**实际不存在的通路** —— 与批 D 给 DESIGN §3.5 标的 parallel/serial RESERVED 同源）；`NEW/Reliability Reinforcement Design.md:755` 与 `NEW/Model Capability and Multimodal Phase Plan.md:154` 的「硬锁/unlockRisky 全保留」加 v2.145 订正注；批 C 登记的随批项 `docs/上下文与记忆工作流程报告.md` 收口：§1.2 管线图与 §1.3「五监听器」表按代码重写为**七监听器现役序**（`state/mod.rs:215-300` 逐行核过，补齐 v2.139 三件、把 `CtxCompact` 移出现役表并写清它留下的两件活物），「规则切分降级」条的 ctx-compact 措辞改为「纪律源自它、现由树侧自持」，注里的行号指路改成锚名指路（行号会漂，这本身就是一种失真） | `README.md` 25 → 84 行、`plugins.md` 改动 111 行、报告 38 行；`plugins.md` 的「误删—补回」闭环：批 E 首次改 §一 表时连带删掉了 §二 节标题与 P0 表，**当场用 `git show HEAD` 原文补回并逐行核对** —— `git diff` 删除行清单 = 20 行 §一 旧表 + 2 行表头 + 1 句改写 + 1 行 `fs.restore` 订正，§二 四行**不在删除清单内** = 原文原样回归；报告表重写后**机器校验列数**（首轮 splice 把「行为」列错位，按 `split("|").length!==6` 判红抓到 → 修 73/75 两行 → 七行全 4 列）；CRLF 保持：`Model Capability...md` 改前改后 `crlf=213/213`（注插在 `\r` 之前，未被整文件转 LF） |

**新登记（批 F 段）**

- **F12**（来自 `bug-report.md` Bug 3 的开放面）：`fs_read`/`fs_grep`/`fs_list` 的 ENOENT 分支只透传系统原文（不指 `fs_list` 上级）、`shell_read` 元字符拒绝不指 `fs_grep`/`shell`、`proof.rs` 的 unterminated-quote 走通用 `structural()` 模板 —— 同族「错误信息要教模型下一步」，修复模板 = 仓内已有的三条 ✅ 先例（`use fs_list` / `use the shell tool instead` / `copy old_string character-for-character`）。
- **F13**（来自 `market/README.md` 订正）：`market` 的 v2 manifest **硬编码在 `scripts/build_market.sh` 里**（`cat > manifest.json <<EOF`），而 `plugins/` 侧走「源 manifest.json + sed 版本号」—— 同一份元数据两条通路，正是 F7「同源实现分家后漂移」的家族成员（v2.133 ENV_LOCK 分裂、v2.135 gc 序错同源）。
- **F14**（本批顺手发现，零风险小项）：`crates/kernel-bin/src/demo.rs` 的版本常量名仍是 `V12: &str = "0.1.0"` / `V13: &str = "0.2.0"` —— 名字来自已被作废的 1.2.0/1.3.0 时代，随批 F 改名 `V_OLD/V_NEW` 即可（不改行为）。

**批 E 门禁（文档最后一次编辑后全量复跑，2026-10-11）**

```
根 fmt CLEAN · clippy --workspace --all-targets -- -D warnings 0 · 39 套件 436 passed 0 failed
desktop fmt CLEAN · clippy --all-targets -- -D warnings 0 · 356 passed 0 failed
check_length 39==基线 · check-i18n zh=en=891 缺键0 无引用键0 · check-commands 85·85·85 · check-middleware-wiring 10·10·0·白名单0
markdown 本地链接自检 73 条 broken 0（脚本 tmp/v149-linkcheck.mjs，未入库）
```

（本段原先写的是「见下方实跑结果」而下面并没有结果块 = 文档领先事实的一句空指向，批 F-1 顺手补实数。）

已知边界：①E4b 的 `CHANGELOG.md` 拆分建议**未采纳并登记待裁定**；②`market/tool-shout` 仍 1.0.0/1.1.0 与 v2.56「全部插件 0.1.0 起步」存在**已知偏差**（夹具保留 1.x 恰好覆盖旧锁文件回退路径），是否重置待你裁定；③INDEX.md 的状态判定 = 我按当前代码逐个 grep 得出（如 `tabler` 0 命中判「未采纳」），**「已兑现」类的兑现批次号来自 AGENTS/ROADMAP 自述**，未逐批重跑验证；④本批未做界面/CDP 走查（纯文档批，无 `.ts/.vue` 改动）；⑤**链接自检脚本 `tmp/v149-linkcheck.mjs` 留在 tmp 未入库**——它抓到过本批自己写出的两处断链（ROADMAP 新块的 `../plugins/README.md` 解析到 `docs/plugins/`、INDEX.md 里含括号的文件名未做百分号编码），但升为第五道门禁要配套变异验证（塞坏链接见红 → 还原复绿）+ pre-commit 接线，属新增门禁面，**等你点头再入库**；⑥「表改写必须带列数断言」这条教训目前只写进 CHANGELOG，**没有机器化**——下一轮表格改写若人记不住就会再犯，可随批 V 一起决定是否落成脚本。

---

### 批 F-1（2026-10-11 落地，v2.150；F1/F2/F3/F14 全落，F4–F13 未动）

| 项 | 证据（改前 → 改后，计数全部机器数得） |
|---|---|
| F1 时间尺 | `desktop/src-tauri/src/timeutil.rs` 新（57 行）+ `timeutil/tests.rs`（107 行）。**本地定义 9 → 0**：`llm/catalog.rs:318`(u64) / `llm/diagnose.rs:206` / `media/mod.rs:289`(u128) / `memory.rs:515` / `memory_reconcile.rs:242` / `organize/mod.rs:112` / `organize/tree_tools.rs:280` / `runner.rs:231`（唯一 `unwrap_or_default` 形状）/ `scheduler/store.rs:65`（`pub(crate)`）。**内联链 8 → 0**：`browser/console.rs:61` / `notifications.rs:105`+`:114` / `commands/sessions.rs:297` / `commands/plugins.rs:99` / `autodisable.rs:20` / `usage.rs:275` / `memory_files.rs:91`（mtime 面走新增 `to_epoch_ms`，与 `now_ms` 同一把尺）。合并计数 = 本批触及文件里 `.duration_since(` **17 → 0**（计划 §1 按函数名只数到 9，实测含内联写开的 8 段）。`scheduler::store::now_ms` 三消费者改直连（`engine.rs:116` / `tests.rs:184` / `tools.rs:19` 的 import 拆名），**不留 re-export**。`loaded_ms: u64` 保持字段类型，调用点 `as u64` ×2。**两类故意不收**（写进头注防"顺手统一"）：纳秒造号（`settings/llm.rs::new_profile_id` + 13 处测试夹具 nonce）与 `llm/retry.rs::jitter_random` 的 `subsec_nanos` **熵源** |
| F2 截断尺 | `text.rs` 新（51 行）+ `text/tests.rs`（164 行）。**本地 `fn clip(` 定义 6 → 0**（`browser/console.rs:46` / `notifications.rs:80` / `export.rs:12` / `organize/search_titles.rs:235` / `scheduler/tools.rs:296` / `subagent_verify.rs:130`），**裸 `clip(` 引用 22 → 0**（22 = 6 定义行 + 16 调用点；`export.rs` 的 `clip_json` 不同名不同物保留）。旧实现不是「一种语义抄了六遍」而是**四种**：`clip_plain`（console/notifications 纯截断）/ `clip_ellipsis`（search_titles/subagent_verify 超长才补 `…`）/ `clip_trimmed`（scheduler/tools 先 trim）/ `clip_markdown`（export 补 `…（截断）` + 换行折平）——四份各归一名，**行为逐点对位**而非统一改写 |
| F3 缺省端点 | `llm/models.rs:21-23` 补「唯一真源」声明；`llm/probe.rs:10` 私有 `DEFAULT_OLLAMA_BASE` 与 `settings/embedding.rs:13` `pub OLLAMA_DEFAULT_BASE` 删除 → `grep localhost:11434` 全 desktop **3 → 1**。`embedding.rs` 原注释自称「与 llm/probe.rs 的探测兜底同源」而磁盘上是两份独立字面量 = 批 D D7「注释自称同源而实际已分叉」同款（这批**还没漂**，属于趁未漂收口）。搭车抓到同文件第二对同名单位：`PROBE_TIMEOUT` `models.rs`(pub 5s) 与 `probe.rs`(私有 5s) → **2 → 1** |
| F14 命名 | `demo.rs` `V12/V13` → `V_OLD/V_NEW` + 注（讲明旧名来自 v2.56 基准重置前的 1.2/1.3）；`demo4.rs` `V13` → `V_ACTIVE`；`plugin-host/tests/echo_plugin.rs` `v12/v13/p12/p13` → `v_old/v_new/p_old/p_new`（24 标识符）。crates 侧这类标识符 **37 → 2**（余 2 = 新注释里解释旧名的字面量，非代码）。`plugin-host/src/registry/tests.rs:6` 的 `v12` **不动**：它的值真是 `1.2.0`，名副其实 |
| 等值判据 | 两个新模块的测试都是**「被删旧代码原文照抄当参考实现」**（出处文件:行号标在注释里），不是手写期望值：`text/tests.rs` 六份旧实现 × 12 档宽度 × 35 条语料逐 `assert_eq`，另加「四语义确实互不相同」的判别测试（若将来被"顺手统一"成一条即红）与导出金样逐字节钉死；`timeutil/tests.rs` 五份旧形状（含 runner 的 `unwrap_or_default` 独形）× 9 个采样时刻（epoch 点、±1ms、1001ms、epoch 之前两种）全等 + epoch 前回落 0 + 两单位读同一个钟 200 次 |
| 变异验证 | 六个变异全部「弄坏 → 见红 → 逐字节还原 → sha 校验」：`text.rs`（ellipsis 无条件加 `…` / trimmed 丢 `trim` / markdown 不折平换行）与 `timeutil.rs`（epoch 前 `unwrap()` 由回落变 panic / 毫秒尺换微秒 / 秒换算多给 1 秒）。**变异脚本自身的一处缺陷当场被自己的 sha 校验抓到**：首版用「反向 replace」还原，锚点 `clip_plain(s, n)` 非唯一 → 把 `trim` 错插进另一个函数，手工逐字节复原（sha 回 `6cd171e213c6c26f`）后改成「写回整份备份」重跑三例全 PASS → 教训落 ROADMAP 边界③：**还原永远走备份，不走反向替换** |
| 测试账 | desktop **356 → 363**（+4 text / +3 timeutil，删 0）；根 workspace **436 持平**（`echo_plugin` 重命名后 2/2 绿）|
| 门禁 | 见 ROADMAP v2.150 门禁行：根 fmt CLEAN / clippy `--workspace --all-targets -- -D warnings` 0 / 39 套件 436 passed 0 failed；desktop fmt CLEAN / clippy 0 / 363 passed 0 failed；四道结构闸全等基线（`check_length` 39==39、`check-i18n` 891=891·无引用键 0、`check-commands` 85·85·85、`check-middleware-wiring` 10·10·0·0）|

**批 F-1 已知边界（记档）**：①**行尾风格陷阱第二次咬人**——`organize/search_titles.rs` 等是 CRLF，脚本 `lines[i] === "}"` 判块尾失配，而 `cat -A` 在本机工具输出里**不显示 `^M`**（`\r` 被输出通道吞掉，od 看同一行也显示 `\n`）：判换行风格只能靠 node 读原始字节。改法 = `split(/\r?\n/)` + 按原 eol 写回，验据 = `git diff --numstat` 每文件都是个位/十位数（整文件翻 LF 会是数百行）；②批量脚本首版半途抛错留半状态（前 3 文件已写盘），F1 脚本改「全内存变换 + 断言全过才统一写盘 + dry-run 预览锚点命中数」；③本批**未新增第五道门禁**防「端点字面量再分叉」——加闸属批 B 门禁体系范畴、需配套变异验证与 pre-commit 接线，等你点头；现防线 = `grep` 对账 + 等值测试锁形状；④本批零 `.ts/.vue` 改动故未跑 `pnpm build`/`lint:style`，未做界面走查（收敛判据 = 等值测试 + 全量编译/门禁，非视觉面）；⑤F4（热路径）/F5（结构性拆分）/F6（前端重复）/F7（plugin-api 共享基座，最大一刀 600–700 行）/F8–F13 各自独立成刀，本轮一律未动。

---

### 批 F-2a（2026-10-11 落地，v2.151；F11 的 desktop 侧全落，根侧 = F11b 未动）

| 项 | 证据（改前 → 改后，计数机器数得） |
|---|---|
| F11 desktop | 新 `desktop/src-tauri/src/testutil.rs`（71 行）+ `testutil/tests.rs`（83 行）。HEAD 实测**测试面 `std::env::temp_dir()` 44 处 / 36 个文件 → 0**（唯一实现点 1 处）。三档旧形态清点：**只带 pid 且不先清 17 处**（地雷本尊：Windows 回收复用 pid → 本次读到上次残留）/ pid + 先清 19 处（不脏读，同进程同 tag 互踩）/ 带 nonce 8 处（不脏读，目录名每次不同）。改造组成 = 脚本 36 处 + 手工 6 处；顺带消掉 **4 份逐字同构的 `fn temp_dir(tag)`**（`instruction_files` / `memory` / `memory_reconcile` / `repo_context` 各抄一遍）与 `tests_session_log` 里「为什么要 nonce」的重复解释（真源改指 testutil 头注 = 批 E「同一信息单点化」同款）。唯一名 = `locowrk-{tag}-{pid}-{纳秒}-{进程内 seq}`；`dir_name(tag, nanos, seq)` 抽成**纯函数**（第一版只测「200 次调用全不同」，把 seq 维删掉也未必红——不抽就判不动这一维）。两入口：`work_dir`（已建好、保证空）/ `unique_dir`（**故意不创建**，给「目录缺席 = 空快照」「fail-closed 越界拒绝」「缺文件出默认值」那一类） |
| 明确不收 | `data_root.rs:56`、`state/mod.rs:89/494` 三处 `temp_dir()` 是**生产码的降级兜底数据根**（`locowrk-data` / `locowrk-degraded-data`：主目录建不出来时退回临时目录保聊天可用），与测试夹具语义无关，且 `testutil` 是 `#[cfg(test)]` 模块、生产码根本不该引。脚本用双闸挡住：非测试文件不碰 + 非 `let NAME = temp_dir()` 开头不碰 |
| 变异验证 | 三例全「弄坏 → 见红 → sha 逐字节还原」：①目录名丢 seq 维 → `same_nanosecond_is_split_by_seq` 红 ②`prepared` 不先清 → `a_taken_name_is_cleared_before_it_is_handed_back` 红 ③`unique_dir` 也建目录 → `unique_dir_never_touches_the_filesystem` 红 |
| 事故与检测器（本刀最值钱的一条） | 批量脚本首版 `L.splice(i, j - i + 1, line)` **off-by-one 多吃一行**，把 `let path = dir.join(..)` 与函数返回行 `dir` / `(dir.clone(), Arc::new(..))` 一起删了。第一轮靠人眼审 diff 抓到并回滚，但**回滚保护名单把「脚本也碰过、我也手工改过」的两个文件排除在外**，坏状态被带进第二轮，最终以两个 `implicitly returns ()` 编译错误暴露（`media/tests.rs:22`、`organize/tests.rs:54`）。处置 = 恢复两文件到 HEAD 后重放 + 手工重放那两处内联，并把这类核对**机器化**成新脚本 `tmp/v151-nocollateral.mjs`：逐行判「被删的 198 行是否都属于取临时目录这件事」，7 行可疑逐条判读全部合法（3 处返回行合并成单表达式 + 4 行注释迁移到 testutil 头注），其余 191 行零可疑。另记两条脚本判据缺陷：`fs::create_dir_all`（文件内 `use std::fs` 写法）前缀匹配漏吸 → 靠 dry-run 预览发现；跨行 `format!(` 的模板抽取正则撑不住多重括号实参（`SystemTime::now().duration_since(..).unwrap().as_nanos()`）→ 改成只抓第一个字面量 + 第一个实参名 |
| 测试账 | desktop 363 → **368**（+5 `testutil::tests`，删 0）；根 workspace **436 持平**（本刀未碰 `crates/`，仍全量复跑取证）。**主判据 = 44 处目录取得方式全换、368 个测试仍绿**：这些测试全部依赖临时目录，"该建的没建 / 不该建的建了 / 名字撞车"任一条都会当场反映为红 |
| 门禁 | desktop fmt CLEAN、clippy `--all-targets -- -D warnings` 0、368 passed 0 failed；根 fmt CLEAN、clippy 0、39 套件 436 passed 0 failed；四道结构闸全等基线（`check_length` 39==39 零新增、`check-i18n` 891=891·无引用键 0、`check-commands` 85·85·85、`check-middleware-wiring` 10·10·0·白名单 0）；零 `.ts/.vue` 改动故未跑 `pnpm build`/`lint:style`。已跟踪文件 **+48 / −198 = 净 −150 行**，新模块 154 行（实现 71 + 测试 83） |
| 已知边界 | ①**治"脏读"不治"累积"**：带 nonce 的目录跑完仍留在 `%TEMP%`，要收这条得给每个调用点挂 Drop 守卫（44 处路径会被 move 进 store / 闭包，逐点判生命周期），与 F11 的病不是一回事——已写进 `testutil.rs` 头注并**登记待裁定**；②**F11b（根侧 `wasm_plugins` 的 `target/locowrk-test/` 共享目录）与 F8（plugin-host 测试样板）未动**，属下一刀 **（此句是 v2.151 当时的快照：下一刀 = v2.152 批 F-2b 已落，两条均 ✅，见下方批 F-2b 段）**；③**未新增第五道闸**防今后再手写 `temp_dir`——现防线只有"本批改完的 grep 对账"，它不是持续闸（与批 F-1 边界③是同一件待你点头的事：第五道闸要不要立）；④批量改造未做界面走查（纯测试面，判据 = 全量测试绿 + 无附带删除机器核对） |

---

### 批 F-2b（2026-10-11 落地，v2.152；F8 全落 + F11 的根侧 F11b 落，本批把 F11 整条收口）

**口径**：纯测试面。`crates/plugin-host/src/` **一行未动**（生产码零改动、零行为变化），desktop 与前端零改动。
已跟踪测试文件 **+44 / −428 = 净 −384 行**，新夹具模块 187 行 → **全批净 −197 行**。

| 项 | 证据（改前 → 改后，计数机器数得） |
|---|---|
| F8 夹具单点化 | 新 **`crates/plugin-host/tests/common/mod.rs`**（187 行 / **13 个 `pub fn`**：`build_lock` `env_lock` `workspace_dir` `run_root` `artifact_name` `stage` `build_and_stage` `load_activate` `stage_and_load` `call` `err_msg` `err_msg_ref` `temp_root`），照 `kernel-agent/tests/common/mod.rs` 先例（`mod common;` + 整模块 `#![allow(dead_code)]`，因每个测试二进制只用其中一部分）。**夹具函数定义副本 37 → 0**（`build_lock` ×5 / `workspace_dir` ×5 / `artifact_name` ×5 / `build_and_stage` ×5 / `call` ×5 / `load_activate` ×4 / `env_lock` ×2 / `temp_root` ×2 / `err_msg` ×2 / `echo_error` ×1 / `stage` ×1）；改后 `grep` 五个测试文件内这些 `fn` 定义 = **0 命中**，`locowrk-test` 字面量只剩 6 处注释 + `common/mod.rs` 内的路径拼装。最直白的实物证据 = `tools_batch2.rs` 与 `tools_batch3.rs` 的夹具区**除一个前缀字符串外逐字相同** |
| 形态合并的判据（不是复制粘贴就能收） | **`load_activate` 有两种签名**：batch 系列「往调用方 registry 上挂」与 fs_fetch「自建 registry 返回元组」——保留为 `load_activate(id, ver, &registry)` + `stage_and_load(id, ver) -> (Arc<Registry>, ToolSource)`，后者内部调前者，**不强行统一成一个形状**。**`echo_error(&ToolError)` 与 `err_msg(ToolError)`** 是借用/取值两种形态，合并成 `err_msg_ref(&e)`（实现体）+ `err_msg(e) = err_msg_ref(&e)`，行为逐字不变。断言消息取信息更全的那份（`cargo build -p {id} ({version}) failed`；batch 版原来不带 version，出错时看不出是哪个版本没编过） |
| F11b 暂存目录 pid 分层 | `stage()` 落点 `target/locowrk-test/{plugin}/{version}/` → **`target/locowrk-test/{pid}/{plugin}/{version}/`**（新 `run_root()`）。原形状下两个测试二进制可能同时 `create_dir_all` + `copy` 同一路径（`copy` 非原子，另一半可能读到写了一半的 dll；Windows 还会报「文件正在使用」——`wasm_plugins.rs` 那条 tmp 注释记的就是这个症状）。**诚实边界（这一条不能吹成"修好了偶发红"）**：逐条查过当前调用表，两个二进制请求同一 `(plugin, version)` 的情况**并不存在**（`tool-echo` 被 `echo_plugin` 用 0.1.0/0.2.0、被 `wasm_plugins` 用 1.0.0；`tool-fs` 被 `fs_fetch` 用 1.2.1、被 `tools_batch2` 用 0.1.5……），所以 pid 分层是**结构性隔离防今后新增重叠组合** |
| 偶发红压测与「未确证」的如实口径 | `cargo test -p plugin-host` 连续 **8 轮全绿**；再把 `fs_fetch_plugins` 与 `tools_batch2` 两个测试二进制**定向并行 4 轮**（8 个结果）全绿。批内一轮完整跑抓到过**一次红**（`fs_fetch_plugins` 4 failed / 1 passed，**0.80s 内失败 = 不像 5s 网络超时**），此后再未复现 → **机制未确证，本批不写"已修复"**。理论窗口 = `cargo build` 产物固定在共享 `target/debug/`，而 `LOCOWRK_PLUGIN_VERSION` 是编译期注入，并发构建同一插件的不同版本时，先构建方的「build 返回 → copy」窗口可能复制到手。但**不凭猜测造跨进程锁**（要单独定口径：fs-lock 类新依赖或自研锁文件 + 陈旧回收，属扩大改动面）。替代动作 = 把 `load_activate` 的失败报文从光秃秃的 `"load"` 补成 **`(plugin_id, version, 暂存路径)` 三要素探针**，下次再红能直接定性。**⚠ 订正（v2.153 批 F-3 期间）**：这条"未确证"后来被推翻了——真正机制既不是网络超时也不是跨进程 cargo 窗口，而是**同一测试二进制内两个用例共用同一个 `(id, version)` 暂存文件**（`fs::copy` truncate + 写，而 `registry.load()` 的读在 `build_lock` 之外），证据 = 红报文 `unexpected end-of-file (at offset 0x0)` 出现在**已带 pid 分层**的路径上 ⇒ 见 §1 **F11c**（未修，登记中）|
| 新夹具自己被抓到的红 | `run_root()` 首版**漏了 `create_dir_all`**，`stage_ping_guest` 先 `fs::write` 再 `stage()` → `NotFound` panic，并连锁把 `build_lock` 打中毒（第二个测试 `PoisonError`）→ **`wasm_plugins` 两个测试当场红**。夹具层改动没有生产码可看，这一红就是「测试是真判据」的实证。修法 = `run_root()` 访问即确保存在并写进 doc |
| 顺带 | `cargo fix --tests -p plugin-host` 清合并后变孤儿的 import（4 文件自动修，`echo_plugin.rs` 的 `use plugin_api::Tool` 手工删）；五个文件头注随批订正——原注释写「暂存到 `target/locowrk-test/`」与「所有 cargo 调用串行执行（同一把构建锁）」，后者在合并后**只对同一二进制内成立**（跨二进制不再有锁），"注释领先代码"正是批 E 在罚的形态，一并改如实措辞并指向 `mod common` |
| 测试账 | 根 workspace 39 套件 **436 passed 0 failed**（与 v2.149/v2.150/v2.151 持平，零回归）。`plugin-host` 侧 **37 个用例数量不变** = lib 单测 21 + 集成 16（`echo_plugin` 2 / `fs_fetch_plugins` 5 / `tools_batch2` 4 / `tools_batch3` 3 / `wasm_plugins` 2），**夹具换实现而断言一条未删**——这就是本刀的判据：37 个真跑 dlopen + FFI 调用的用例在只换脚手架的情况下全绿。desktop **368 持平**（本刀未碰） |
| 门禁 | 根 fmt CLEAN、clippy `--workspace --all-targets -- -D warnings` **0**、39 套件 436 passed 0 failed；四道结构闸全等基线（`check_length` **39==基线零新增**，新模块 187 行守 400 内、`check-i18n` 891=891、`check-commands` 85·85·85、`check-middleware-wiring` 10·10·0·白名单 0）；零 `.ts/.vue` 与 desktop Rust 改动故未跑 `pnpm build`/`lint:style` |
| 已知边界 | ①**偶发红机制未确证**（见上），跨进程「构建 + 复制」整段互斥是待裁定的独立一刀，登记在本段、§1 F11 残留项与 `common/mod.rs` 头注 ⚠ 三处；②`target/locowrk-test/{pid}/` 会随运行次数累积（`cargo clean` 一并清，属批 G 射程），与批 F-2a 边界①「治脏读不治累积」同一口径；③**同二进制内**并行用例仍共享 `build_lock`，某用例持锁 panic 会毒化其余（`lock().unwrap()`）——收敛前就是这个形状，本批未扩大改动面；④未新增第五道闸（与批 F-1 边界③、批 F-2a 边界③是同一件待你点头的事）；⑤纯测试面批，无界面走查（判据 = 37 个用例数量不变 + 8 轮全量绿） |

---

### 批 F-3（2026-10-11 落地，v2.153；F5 第一项落 = `init_state` 636 行拆成八簇；F5 余四项未动；新立并确证 **F11c**）

**口径**：生产码**语义零改动**——497 条原语句由脚本证明零丢失、零重排、零新增行为（判据见下表）。`state/mod.rs` 699 → **220 行**、`init_state` 636 → **195 行**；拆出 8 个簇文件 **1174 行**；已跟踪文件 `state/mod.rs` **+157/−635**、`llm/mod.rs` 1/1、`check_length.sh` 4/1 ⇒ **全批净 +696 行**（这笔账为什么值得，见「边界①」）。

| 项 | 证据 |
|---|---|
| 八簇划分 | 新 `desktop/src-tauri/src/state/` 下 **`boot.rs` 146**（Context/注册表/事件出口/通知 vtable 钩子/数据根/调度库/统计库/embedding/会话表 + 启动回放/SettingsStore 含降级/fsRoots 与记忆文件化 env）·**`llm_stack.rs` 120**（停止标记→重试通知闭包→模型覆盖→诊断账本→能力目录→资产→`AppLlm`，末尾 `LlmService` 与 `LlmStreamSink` 注册）·**`memory_stack.rs` 101**（fsRoots/开关供给闭包→会话上下文解析器含 plugin-host 全局注入→`MemoryStore`→权限模式→树存储与组装器 + `TreeContextSource` 注册）·**`waterfall.rs` 151**（七监听器注册序本体）·**`kernel_stack.rs` 102**（洋葱四件 + 粘性规则 + `ApprovalHub` + `AskHub` + `SubagentGate` + `Kernel`）·**`tools.rs` 242**（宿主内置工具 + `search_catalog`/`main_source` 两棵树 + `ToolSource` 注册）·**`plugins.rs` 132**（服务开启含降级 + 对账 + 开发态装载 + 广播 + 停用巡检）·**`background.rs` 180**（`Organizer` + `spawn_engine` + 在途重投 + 首启导出 + 资产 GC）。每簇 = `Init`（入参逐字段显名）+ `Stack`（出参字段名**等于**原局部变量名）+ `fn build`；调用点按名解构 ⇒ 下游语句与 `AppState` 字面量一行都不用改写 |
| 判据为什么必须新造 | `state/mod.rs` 原注释明写「**桌面单测不引用 state**」（为了不让 tauri 运行时符号连它的窗口栈 tao 进测试二进制；无 v6 清单的进程链接 tao 后 comctl32 缺 `SetWindowSubclass`/`TaskDialogIndirect`，加载即 `ENTRYPOINT_NOT_FOUND`）⇒ **368 个单测全绿对本刀零证明力**，而"顺序契约"这类错位编译器根本看不见（把 `startup_reconcile` 放到插件装载之后照样编过）。审计给 F5 的回归判据「启动日志 + desktop 全测」在这一层是不够的，所以本批自己立了语句级判据 |
| 等价判据（本刀核心） | `tmp/v153-equiv2.mjs`：**A = `git show HEAD:state/mod.rs` 里 `init_state` 的有序语句 = 497 条**；**B = 改后 `init_state` 体把八个调用点就地展开回各簇 `fn build` 体 = 548 条**。三条全零：**丢语句 0**（改前有、改后无）、**顺序错乱 0**（A 不是 B 的保序子序列的行数）、**可疑新增 0**（B\A = 16 行全部是 `Stack` 字面量字段名行与两个簇返回值行，属机械搬运）。搬运手法 = 四个脚本（`v153-splice.mjs` / `2` / `3` / `4`）一律「锚点定位不写死行号 + 全内存变换 + 断言「被删的每一行都能在新模块里原样找到」全过才统一写盘 + 写前备份」，两轮分别搬出 77 + 149 + 56/41/77/35 + 62/91 条语句，**每轮 missing 都是 0** |
| 判据脚本自己的两个 bug（如实） | ⑴首版只认 `::build(` 而 `tools` 簇是 `build_and_provide` → 整簇没展开，误报「丢 90 条 + 顺序错乱 281 条」；⑵块尾只认 `trim() === "});"`，而 13 字段的 `boot` 簇收尾是 `} = boot::build(boot::Init { app: app.clone() });`（同行形态）→ 一路吞到 GC 块的 `});`，误报「顺序错乱 5 条」。**两次都是判据自己错、却把正确的拆分说成失败**；若当时选择"信脚本"就会回滚一次本来正确的改动。教训 = **判据脚本自己也要验**；"往后 N 行窗口找匹配"这种写法不可靠（N=14 恰好被 15 行的块超出），改成只认两种块起点（同行调用 / `let mod::Stack {` 头）才是确定的 |
| 实机走查（state 唯一可行的端到端证据） | `tmp/v153-walk/walk.mjs`（隔离数据根 + 隔离 WebView2 目录 + CDP 9233 + 复制真实档走凭据库，Key 零明文落盘）：**两子批各跑一次 = 20/20 PASS**。W1 九插件装载逐 id 列出（memory-sqlite/tool-echo/tool-edit/tool-fetch/tool-fs/tool-git/tool-mcp/tool-notify/tool-shell）·W2 `boot: init_state done`（三次实测 991 / 3029 / 3723ms）·W3 零 panic / 零 `plugin service unavailable` / 零 ENTRYPOINT_NOT_FOUND·W4 窗口与 composer 真渲染·**W5b 真轮第一跳 `search_tools` 的目录里带回 screenshot/browser_open/subagent_run/ask_user**（= `with_builtin` 链在位）·**W5c/W5d 插件工具 `fs_list` 被真调、`plugin_id=tool-fs`、结果读到夹具文件名 `hello-from-f3a.txt`**（= 一条调用同时穿过插件装载簇与工具目录簇）·W6 该调用的 `tool_call` 先于 `tool_result` 落盘（纪律 4 的事件面）。走查后数据根按明文扫描（7 文件 0 命中）再删除，脚本/日志/截图留 `tmp/v153-walk/` |
| 走查环境三条教训（记档） | ①CDP 首匹配的 page target 是 **`about:blank`**（主窗导航前的空态），拿它当主窗则 DOM 全空 + `HTMLTextAreaElement` 取属性描述符抛 `Illegal invocation`；②**debug 构建的 `devUrl = http://localhost:5173`**（`tauri.conf.json`），直接跑 exe 而没人服务前端就是空白页——脚本改成先起 vite 并轮询就绪再起 exe（`netstat` 核实 5173 空闲，不与用户 dev 实例抢端口；vite 钉 `::1` 是 AGENTS 明写勿改双栈项），结束时一并 kill；③`evaljs` 遇异常一律**返回 null 而不抛**，否则半途退出漏掉 `proc.kill()` 会把 `locowrk-desktop.exe` 锁住（历批门禁"红了再查是取证脚本留的活进程"的同款坑）|
| 顺带 | `llm/mod.rs` 第 29 行 `pub use embed::{…}` 加 **`CooledEmbed`**（1 行；`tools.rs` 的 `Init` 字段要写 `Arc<CooledEmbed>` 而 `mod embed` 是 llm 私有子模块——并进既有 re-export 行而不是新开模式）；`cargo fix --lib` 清搬空后变孤儿的 import（两轮，F-3a 六条 + F-3b 若干，机器修无手改）；`scripts/check_length.sh` **`BASELINE` 39 → 38 并在脚本注释里记因**（M-4 超限榜首 `state/mod.rs` 从此消失）；四处新簇头注首版踩 clippy `doc_lazy_continuation`（编号列表后缺 `//!` 空行 → rustdoc 把下一段当列表续行），补分隔后 `-D warnings` 归零 |
| **F11c 的确证过程**（本批唯一的新缺陷发现） | 满跑根门禁时抓到 `wasm_plugin_staged_and_served_via_registry` 红：`failed to parse WebAssembly module: unexpected end-of-file (at offset 0x0)`，路径 `target\locowrk-test\28344\wasm-ping\0.1.0\wasm_ping.wasm`。**关键在路径里带着 pid**（F-2b 那层隔离正在生效）⇒ 病不是跨进程，而是同一二进制内 `wasm_plugins.rs` 两个用例都往**同一个** `wasm-ping/0.1.0/wasm_ping.wasm` 做 `fs::copy`（truncate + 写），而 `registry.load()` 的读发生在 `build_lock` **之外**。当时立即取证 = 单跑 2/2 绿 + 连续 6 轮 `-p plugin-host` 全绿 ⇒ 窗口小、偶发。这**推翻了批 F-2b 登记的「跨进程 cargo 产物窗口」猜测**，也推翻了"机制未确证"的状态：现在机制确证、修法明确（`stage()` 目标带调用级 nonce，`(id, version)` 注册语义不变），但**本批没修**——它属 plugin-host 测试面，与 desktop 组装点拆分不是一类变量（AGENTS「一轮只动一类变量」），已按「发现即登记」立为 **F11c** 并写清验收判据（定向并行 20 轮零红 + 摘掉 nonce 必须能复现红的变异验证）|
| 测试账 | desktop 3 套件 **368 持平**（本刀未碰任何被测函数，`state` 本就零单测）；根 workspace 39 套件 **436 持平**（根侧零改动，仍全量复跑取证）|
| 门禁（最后一次编辑后全量复跑） | 根 fmt CLEAN / clippy `--workspace --all-targets -- -D warnings` 0 / 39 套件 436 passed 0 failed；desktop fmt CLEAN / clippy `--all-targets` 0 / 368 passed 0 failed；四道结构闸全绿 = `check_length` **38 == 收紧后的基线**（八个新文件全在 400 内，最大 242）、`check-i18n` 891=891 缺键 0、`check-commands` 85·85·85 预留 0、`check-middleware-wiring` 10·10·0·白名单 0；等价判据 EXIT=0；走查 10/10 ×2。零 `.ts/.vue` 改动故未跑 `pnpm build`/`lint:style` |
| 已知边界 | ①**净增 +696 行是有意的**：八簇各带「次序契约」头注 + `Init`/`Stack` 结构体——把原本只存在于作者脑子里（以及散在行内注释里）的启动顺序知识写成机器可读的文件头，这是代价，也是本批真正买到的东西；不认这笔账就会退回"巨型函数最省行"的反面。②**F5 余四项未动**：`organize/mod.rs` 614 行拆 trigger/collapse/task、`runner::project_events` 237 行拆 `runner/projection.rs`、`settings/llm.rs` 682 行拆 view/model、`session.rs` 410 行抽 `session/project.rs`、巨型测试文件目录化。③`init_state` 仍有 195 行，其中 `AppState` 字面量 40 个字段占大头——再压就要动 `AppState` 形状 = 第三类变量。④**等价判据是 `tmp/` 下的一次性脚本，未入库也未成门禁**：它证明的是"这一刀"，不会自动防住下一刀重排（与「第五道闸」是同一件待你点头的事）。⑤簇 `Init` 参数面宽（`tools` 18 字段、`background` 21 字段）是**对原本就存在的数据流的忠实显式化**，不是新增耦合；真要收敛得给启动期句柄做分层容器，本批按一轮一类变量未做。⑥F11c 未修，CI 上仍可能出现那条偶发红，看到时按 F11c 判读而不是当回归查 |

---

## 6. 验收判据（每批交付时逐条给）

1. 门禁全量复跑结果（最后一次编辑之后）：根 `fmt --check`/`clippy`/`test` + desktop 三件 + `pnpm build`/`lint:style` + `check_length` 基线对比数字（改前/改后）。
2. 删除类改动：给出「改前 grep 计数 = 改后计数」的对照，证明不是误删活代码。
3. i18n 类改动：`check-i18n.mjs` 跑通 + 中/英界面目检（H3b 那 4 条必须截图证明不再显裸键名）。
4. 文档类改动：给出改前后原文对照。
5. ROADMAP 勾 + CHANGELOG/AGENTS 落账（项目现行纪律）。
