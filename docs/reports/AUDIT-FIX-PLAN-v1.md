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

### 批 C｜退役残留清扫（审计 H3）——**需裁定**

| # | 条目 | 判据 | 待裁定的口径 |
|---|---|---|---|
| C1 | 59 个确证孤儿 i18n 键（zh+en 共 118 条死文案），分布 `settings.*` 23 / `approval.*` 8 / `subagent.*` 8 / `developer.stabilityRounds*` 2 等 | 台账 `_parts/1-i18n-orphans.md`；`app.modelFallback` 已由 Lead 撤回（`Composer.vue:99` 在用） | 删（推荐）；删前先人工复核 9 条「叶名以裸字符串出现过」的歧义项 |
| C2 | `sandbox_run` 前端残留 6 处：`lib/tool-summary.ts:14,125`、`lib/tool-detail.ts:62,123,332,526`、`lib/side-effect.ts:43-44`（A3 已删）、`locales/*` 两条 | 插件 v2.145 已整目录退役，新会话不可达；**历史会话日志重放仍会渲染这些工具行** | 二选一：① 一并删（推荐——审计已确认删后落既有兜底 `{type:"tool", icon:"plug"}`+插件徽章，不崩）；② 判为「历史渲染面」保留 |
| C3 | 三个死 API 包装 + 连带的 Rust 命令（审计 C2/B 域同源） | 已复核零 import：`lib/api.ts:105 assetGc`、`:419 llmListModels`、`:607 llmProbeWindow`；`types.ts:1030 ProviderModels` 孤儿。其中 `llm_probe_window` 的 UI 触发点（设置页自动检测）随 v2.137 四层链下线，`llm/probe.rs`（50 行）+ `probe/tests.rs`（27 行）整模块随之无消费者 | **这条不能只删包装**：删包装而留命令 = B5 那道三向闭合当场出现「后端有、前端无」红灯。三选一：① 连 Rust 命令一起退役（三处命令 + `lib.rs` 注册 + `probe.rs` 模块 + 相关测试）；② 只删 `llm_probe_window`/`llm_list_models` 两条真死的，`asset_gc` 保留（它是开发者手动 GC 入口，启动 + 24h 循环之外仍有用途）；③ 全保留并加「预留」注释 |

### 批 D｜死配置与哑 ABI（审计 H4/H5 + M-20/M-21/M-22/M-23）——**部分需裁定**

| # | 条目 | 现状（已复核/审计取证） | 建议口径 |
|---|---|---|---|
| D1 | `config/profile.toml`（审计 H4） | 实测文件 7 行全注释、零生效键；全仓 Rust 无任何读取路径（`grep profile.toml` 唯一命中是 `kernel-bin/src/main.rs:7` 的「阶段七补」文档注释）；README 与 `DESIGN.md:1256,1695` 按「已实现」描述 | **需裁定三选一**：① 删文件 + 从 README/DESIGN 移除（推荐，当前桌面端插件装载走 lockfile/fsRoots，这条路已不存在）；② 保留 + 文档改「规划中」；③ 真实现加载器（工作量最大且与现网插件机制重复） |
| D2 | `CallCtx` 取消设施（审计 H5） | `crates/plugin-api/src/tool.rs:180,193,197` 全仓零调用；唯一 `cancel` 字面命中是 `scheduler/tools.rs:51` 的描述文案 | **需裁定**：接线（超时/用户停止时真 `cancel()`，插件可感知）或删三者改为「取消由 `deadline` 承担」。`CallCtx` 是公开 ABI，删除属收窄 → 倾向**接线**（v2.144 刚修过 `subagent_run` 超时契约，停止语义接上是同一族问题） |
| D3 | session-store 磁盘快照 + upcaster 簇（审计 M-20，111 + 122 行） | `Snapshot`/`Upcaster`/`V1ToV2Upcaster`/`snapshot_at`/`from_snapshot` 全仓零生产引用；恢复走 `persist.rs:80-98` 逐行 JSONL 回放 | 先读 `DESIGN.md:1360-1380`（§9.5）定「未接线缺陷 vs 预留」再决定；⚠ 勿连带删内存态 `Session.snapshots`（活的另一套）；⚠ 删除要同时撤回 `CHANGELOG.md:28` / `ROADMAP.md:872` 的归档里程碑——**这一条我倾向标注 RESERVED + 注释，而不是删**（归档代价 > 111 行收益） |
| D4 | cordis-core `serial` 监听器机制（约 45 行 + 测试） | 命中全在 `event_bus/tests.rs` 与 `lib.rs:27` re-export；`serials` map 生产恒空；其自述用途已被宿主侧中间件取代（`middleware/audit.rs:5`） | 删整簇或标 RESERVED，同步 DESIGN §3.5 表 |
| D5 | `host_kv_put` 回 OK 但数据永不可读 + `ERR_UNKNOWN_TOOL` 幻影码（审计 M-21） | `plugin-host/src/host_vtable.rs:65-93`；`plugin-api/src/ffi.rs:16` | 让 `host_kv_put` 如实返回 `ERR_UNSUPPORTED`（ABI 签名不动，语义诚实化）；`-2` 补人类可读报文或标 RESERVED（`ffi.rs` 自述冻结契约，不宜删） |
| D6 | 仅自家测试消费的 pub 面（审计 M-22） | `runtime.rs:83-98` 四个观测方法、`lockfile.rs:154-157 clear_pending_delete`（与 `service.rs:277-287` 同语义两实现）、`registry.rs:210-216 graveyard_len`、`kernel.rs:109-113 with_retry_budget` | 降 `pub(crate)`/`#[cfg(test)]`；`deps_of` 直接删 |
| D7 | 幻影字段 + 双份校验规则（审计 M-23） | `memory-tree/src/trigger.rs:76 anchor_source` 只写不读；`valid_session_id` 两份规则**已分叉**（`session-store/persist.rs:23-29` 允许 `.` vs `memory-tree/storage.rs:15-22` 不允许且限长 64，注释却自称同源）→ 含 `.` 的会话 id 日志落盘成功而树投影静默失效。当前桌面 id 形如 `desktop-<毫秒>` 恰好都合法 = **陷阱不是现网缺陷** | 删 `anchor_source`；`valid_session_id` 下沉 `plugin_api`（有 `fsutil` 先例）单点定义 + 单测锁规则 |
| D8 | `cordis-core → plugin-api` 依赖（审计 H6/M-6.2 反向证据） | 代码零使用，但 `DESIGN.md:1266` 依赖图明写要求 | 删它必须同批改 `DESIGN.md:1264-1279`（§8），否则文档与清单反向不符。**倾向：连文档一起改，或干脆不动（这条是设计约束不是垃圾）** |

### 批 E｜权威文档真实化（审计 H9 + M-6/M-8/M-11/M-12）

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

### 批 F｜重复实现收敛与性能（审计 M-1/M-14/M-18/M-19/M-24 + C 域 C8–C13）

低风险优先，建议**一轮只动一类变量**（与项目纪律一致）：

- F1 `desktop/src/timeutil.rs`：`now_ms()` 7 处 + `now_secs()` 2 处收敛（`scheduler/store.rs:65` 已是 `pub(crate)` 共享版）。
- F2 `desktop/src/text.rs`：`clip()` 6 处 / 4 种语义收敛；⚠ `export.rs` 的输出是用户可见的 Markdown 字节，合并必须逐字节不变（先写等值断言再动）。
- F3 Ollama 端点常量 3 份留 1 份（`llm/models.rs:21`、`llm/probe.rs:10`、`settings/embedding.rs:13`）。
- F4 热路径（审计 M-18 a–g）：审批单次决策 ~4 次会话上下文解析 → `resolve` 内 memo（**不跨调用缓存**，`workspace.rs:39-46` 的「保存即生效」是有意设计）；项目规则 JSON 两读 → 新增 `verdicts()`；每轮 2 次 `LlmConfig` 全量克隆（含明文 Key 串）→ `with_llm(\|cfg\| …)` 闭包访问器 + 合并 `commands.rs` 三连取锁；`turn_id` 每轮全扫 → `rev().find_map` 折衷；`has_call_event` 懒建；`session_history` 锁内深克隆 → 锁内取快照、锁外投影 + 重 IO 命令包 `spawn_blocking`；`append_slice` 每条一次 open/write/close → 单句柄批量。
- F5 结构性拆分（审计 M-19/M-24e + M-4 超限榜首）：`init_state` **636 行**按子系统抽私有构造器（注意 sink 先于 approval、catalog 先于 session_window 的构造顺序隐性契约，回归判据 = 启动日志 + desktop 全测）；`organize/mod.rs` 621 行拆 trigger/collapse/task；`runner.rs::project_events` 单函数 237 行拆 `runner/projection.rs`；`settings/llm.rs` 682 行拆 view/model（保持 `settings.rs` 的 14 条 `pub use` 不变）；`session.rs` 415 行抽 `session/project.rs`；巨型测试文件按既有先例（`tests_session_log/`）目录式拆分。
- F6 前端重复（C8–C13）：命令包装剥离/工作区相对化两份下沉；相对时间四份统一到 `format.ts`；`v-html` 复制委托两份合并；**弹层「外点关 + Esc」10 处各写一遍 → `use-popover` composable**；`stores/settings.ts` 保存样板 ×9（135 行里约 90 行同构，且 `setTimeout` 无句柄复用是同源小缺陷）。
- F7 `plugin-api` 共享基座（审计 M-1/S9，**本计划里最大的一刀**）：SSRF 防线两插件 361 行逐行同形、`zh_en`/`plugin_version`/`unsafe impl Send/Sync + SAFETY`/官方展示元数据样板各 ×10、`arg_u64` ×6（fs 与 git 两版已轻微漂移）→ 上收 `netsafe`/`args`/`manifest::official` + 宏，合计可减 600–700 行并消灭「同源实现分家后漂移」这类缺陷（v2.133 的 ENV_LOCK 分裂、v2.135 的 gc 序错都是同源产物）。
- F8 `plugin-host/tests/` 五个集成测试的 `build_and_stage`/`workspace_dir`/`build_lock` 样板 → 照 `kernel-agent/tests/common/mod.rs` 先例共享。
- F9 前端超大文件（C16–C19）：locale 拆命名域、`stores/chat.ts` 事件投影下沉纯函数、`types.ts` 拆目录 + 71 个只内部使用的导出收窄、`lab/BuiApprovalCard.vue`（635 行，仅 `LabPanel` 引用的演示夹具）移出生产树。
- F10 `session-store` 侧优化（M-24 a–d）：`pending_slot` 反向全扫 O(n²) → 命中即停；`tool_exposure` 每请求克隆全目录 + 两次序列化求长度 → 逐项累加 + `select` 返回下标；`search_sessions` 每文件读两遍 → 复用 `list_sessions` 单遍。
- **F11（本批新增，审计未列）测试临时目录/共享目录隔离收敛**：`grep temp_dir()` 全仓 ~20 处写的是 `{前缀}-{tag}-{pid}`——无每次运行 nonce、多数不先清，Windows 复用 pid 时会读到上次残留（批 B 实测撞红一次，机制见 §5 顺带发现）。修法 = 统一收进 `desktop/src-tauri/src/testutil.rs` 的 `tmp_dir(tag)`（pid + 纳秒 + `remove_dir_all` 先清，`memory_files/tests.rs` 已有正确实现可作蓝本），各处删本地副本。同族的还有一处**根 workspace 侧已知偶发红**：`crates/plugin-host/tests/wasm_plugins.rs` 用 `target/locowrk-test/...` 共享目录，并行套件互踩（本批末次全量跑当场抓到、单跑即绿；AGENTS 历批记的「wasm_plugins 已知偶发」就是它）——同一味药：每个测试独占带 nonce 的目录。**登记：`CODE-AUDIT-v1` 完全没有覆盖这一类**（它查的是死代码与重复实现，「测试脚手架脏」要靠真跑门禁才现形）。

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

另：孤儿键口径在批内收紧了两次（133 → 62）。第一轮虚高是因为只扫 `t()` 调用点，漏掉 `PermissionCard.vue:64` 这种「先 `const path = \`approval.sensitive.${key}\`` 拼串、再传进 `t()`」的形态，把 11 条活文案误报成孤儿——**假孤儿比漏报危险**（人会照着清单删掉正在用的文案）。收紧后 62 条与台账「确证 50 + 待确认 9」基本对齐，批 C 执行时取两者交集、并优先信任台账。

## 6. 验收判据（每批交付时逐条给）

1. 门禁全量复跑结果（最后一次编辑之后）：根 `fmt --check`/`clippy`/`test` + desktop 三件 + `pnpm build`/`lint:style` + `check_length` 基线对比数字（改前/改后）。
2. 删除类改动：给出「改前 grep 计数 = 改后计数」的对照，证明不是误删活代码。
3. i18n 类改动：`check-i18n.mjs` 跑通 + 中/英界面目检（H3b 那 4 条必须截图证明不再显裸键名）。
4. 文档类改动：给出改前后原文对照。
5. ROADMAP 勾 + CHANGELOG/AGENTS 落账（项目现行纪律）。
