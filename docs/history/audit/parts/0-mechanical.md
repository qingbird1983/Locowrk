# 机械取证（Team Lead 亲测，全体结论以本文件为硬证据基准）

扫描面：全仓（`crates/`、`desktop/src-tauri/`、`desktop/src/`、`plugins-src/`、`scripts/`、`docs/`、根清单），
排除 `target/`、`node_modules/`、`dist/`、`.git/`。规模：Rust 388 / Vue 92 / TS 52 文件，代码约 9.22 万行（不含依赖与产物）。
工作树状态：`git status` 有 v2.145（权限模型重设计）未提交改动，本次审计对象 = 当前工作树。

---

## M1【未使用依赖 ×3】高置信

| crate | 依赖 | 证据 |
|---|---|---|
| `crates/cordis-core` | `plugin-api`（normal） | 全 crate `.rs` 内 `plugin-api`/`plugin_api` 出现 **0** 次；全仓唯一命中 = `crates/cordis-core/Cargo.toml:9` |
| `crates/kernel-bin` | `plugin-service`（normal） | 同上，唯一命中 = `crates/kernel-bin/Cargo.toml:12` |
| `crates/memory-tree` | `tokio`（dev） | 同上，唯一命中 = `crates/memory-tree/Cargo.toml:15`（无 `#[tokio::test]`，测试全部同步） |

判据：`cargo metadata --no-deps` 逐依赖 × 逐 crate 源码文本比对，出现次数为 0。
建议：删除这三行；`build.rs` 内亦无引用（已核对）。风险：低（删后跑一次 `cargo build --workspace --all-targets` 即可验证；若某依赖仅为链接期需要，编译器会立刻报错）。

## M2【零生产接线的内核中间件：ctx-compact 全簇】高置信 —— 本次最大发现

证据链：
- `crates/kernel-agent/src/middleware/compact.rs`（306 行）定义 `CtxCompact` / `CompactConfig` / `CompactStrategy` / `suggested_threshold` / `CompactionOutcome`。
- 构造点 `CtxCompact::new(...)` 全仓仅出现在 `crates/kernel-agent/src/middleware/compact/tests/mod.rs`、`compact/tests/stability.rs`、`crates/kernel-agent/src/kernel/tests.rs:663,694`——**全部是测试**。
- 宿主装配面：`desktop/src-tauri/src/state/mod.rs:301-308` 的 `Vec<Arc<dyn Middleware>>` 只有 `Tracing / Audit / Timeout / CircuitBreaker`；`crates/kernel-bin` 全 crate `compact|Compact` 命中 **0**。
- 桌面侧压缩链已在 v2.137 整面退役（AGENTS 记档「ctx-compact 桌面链退役 … 一切会话皆树」）。

代价：`compact.rs` 306 行 + `compact/tests/mod.rs` 380 + `compact/tests/stability.rs` 92 ≈ **778 行**常驻编译与维护面，生产路径零调用。
注意（勿误杀）：`CompactionOutcome` 仍被 `crates/kernel-agent/src/waterfall.rs:56`、`kernel.rs`、`kernel/request.rs` 用在返回类型上，且**旧会话日志重放**链路仍然可达（`crates/session-store/src/session.rs:132/249` 折叠、`desktop/src-tauri/src/runner.rs:416-424` 转事件、前端 `chat.ts:667` 渲染、`notifications.rs:240` 入账）。所以不能整簇删除，需按「保留重放面 / 删除未接线中间件」分界处理。
建议（三选一，需用户裁定）：① 删除 `CtxCompact`+`CompactConfig`+`CompactStrategy`+`suggested_threshold` 与其专属测试，保留 `CompactionOutcome`（类型面继续服务重放）；② 若确有 headless 复用计划，移到 `#[cfg(feature = "compact")]` 之后并在文档记档；③ 明确记档「保留待用」并接受 778 行维护面。风险：中（涉及 `lib.rs`/`middleware/mod.rs` 的 re-export 面）。

## M3【零生产接线的 Retry 中间件 + 文档与装配不符】高置信

- `crates/kernel-agent/src/middleware/retry.rs`（46 行）`Retry::new` 全仓仅在 `crates/kernel-agent/src/middleware/tests.rs:171,179` 使用。
- `desktop/src-tauri/src/state/mod.rs:301-308` 的实际装配顺序 = `Tracing → Audit → Timeout → CircuitBreaker`，**不含 Retry**。
- 而 `crates/kernel-agent/src/chain.rs:3` 与 `middleware/mod.rs:4-5` 文档仍写「装配次序 Tracing → Timeout → Retry → CircuitBreaker → Approval → Tool」——**文档与实现不符**（Audit 未写入文档，Retry 未进实现）。
说明：`WaterfallDecision::Retry` 是另一条路（`kernel/request.rs:209` 在 step 内闭环），与 `Retry` 中间件不是一回事，勿混淆。
建议：删 `retry.rs` 或接进链；无论哪种，`chain.rs`/`mod.rs` 的注释必须与实际装配对齐（文档腐烂，低风险高收益）。

## M4【plugin-api 取消设施是哑 API】高置信

- `crates/plugin-api/src/tool.rs:180`（`cancelled: Arc<AtomicBool>` 字段）、`:193 is_cancelled()`、`:197 cancel()`。
- 全仓 `\bcancel\b|cancelled|is_cancelled` 命中仅 7 处，全部在这段定义内（另有 1 处是 `scheduler/tools.rs:51` 的工具描述文案，无关）。
- 即：宿主从不 `cancel()`，插件侧 `is_cancelled()` 永远是 `false`。
建议：要么在宿主接上（真正驱动取消），要么删掉这三个成员并在 `Tool`/`CallCtx` 文档标注「取消由 deadline 承担」。风险：低（`CallCtx` 是对插件公开的 ABI，删除是 ABI 收窄，需按项目 ABI 纪律评估）。

## M5【无消费者的 pub 项 4 处】高置信

| 位置 | 项 | 说明 |
|---|---|---|
| `crates/memory-tree/src/manager.rs:274` | `pub fn count_user_messages` | 纯转发 `material::user_count`；注释称「编排层反弹账本用」，但生产消费方（`desktop/src-tauri/src/organize/{mod.rs:257,284,324,recovery.rs:64}`）直调 `memory_tree::user_count`——转发层零调用 |
| `crates/cordis-core/src/runtime.rs:52` | `pub fn deps_of` | 注释自称「调试/校验用」，全仓零调用 |
| `crates/plugin-api/src/tool.rs:193` | `pub fn is_cancelled` | 见 M4 |
| `crates/plugin-api/src/tool.rs:197` | `pub fn cancel` | 见 M4 |
判据：全仓标识符出现次数 == 定义处数（含 tests/docs/前端）。建议：删 M5 前两项（低风险）；后两项并入 M4 裁定。

## M6【`#[allow]` 掩盖面 3 处】逐条已读，均非隐藏大问题

| 位置 | 内容 | 判读 |
|---|---|---|
| `desktop/src-tauri/src/state/app_state.rs:25` | `#[allow(dead_code)] pub app: AppHandle`，注释「当前事件出口已是共享 sink，字段暂无读取方」 | **真死字段**：保留是给「阶段八窗口级命令」备用。建议：删字段或改注释为「计划用途」并在 ROADMAP 挂钩 |
| `crates/session-store/src/session.rs:411` | `#[allow(unused_imports)] pub use crate::event::{TurnId, TurnOutcome};` | 为「§4.1 事件全集类型完整」而 re-export，零外部消费者。建议：删或者由消费方（kernel 会话管理层）真正 import |
| `desktop/src-tauri/src/plugins_root.rs:17` | `#[allow(unused_imports)] pub use plugin_service::builtin::{dll_name, find_artifact};` | `dll_name` 被 `tests_plugins_root.rs` 经此路径引用，属**测试面 re-export**，保留有理由（也可改测试直接引原路径） |

## M7【清洁面：确认「没有」的东西】—— 用于避免过度报告

- `TODO / FIXME / HACK / XXX / DEPRECATED` 在 `*.rs *.vue *.ts`：**0 命中**。
- `dbg! / todo! / unimplemented! / unreachable!`：**0 命中**。
- 注释掉的代码块（启发式：`^\s*//\s*(pub|fn|let|if|for|while|match|use|struct|enum|impl|return|self\.|X.y(` ）：**0 命中**。
- 孤儿 `.rs` 模块：仅 `plugins-src/tool-mcp/examples/fake_mcp_server.rs`——cargo `examples/` 是入口，**非问题**。
- 前端孤儿 `.vue` 组件：**0**；`stores/*.ts` 全部有引用（最少 `notifications` 15 次）。
- `cfg(feature = ...)`：全仓 **0** 处（故 feature-gate 是可用手段，无历史包袱）。
- 空/极短源码文件：仅 `tmp/dbg_prefix.rs`（未入库）。

## M8【Tauri 命令契约：完全闭合】高置信

- `#[tauri::command]` 实现 **85** 个 = `desktop/src-tauri/src/lib.rs` 注册表 **85** 个 = 前端全部有引用（`desktop/src/**` 内 0 个「注册但前端从不提及」）。
- 前端 `invoke("…")` 出现的命令名全部已注册（无「调不存在的命令」）。
- 未被注册的两个 `pub fn`（`commands/llm.rs:44 enrich_with_catalog`、`commands/sessions.rs:359 open_in_explorer`）均为跨模块内部助手，有实际调用方（`commands.rs:242,358`、`commands/assets.rs:81`、`commands/memory.rs:87`）——**非死代码**，仅可见性可收窄为 `pub(crate)`。

## M9【重复实现（样板复制）】中置信

| 模式 | 规模 | 证据 |
|---|---|---|
| 插件 C ABI 样板 | 10 个插件 crate 内 `zh_en` / `make_state` / `describe_manifest` / `log` / `arg_u64` 各约 11 处同形定义 | 名称跨文件重复统计；`crates/kernel-bin/src/scaffold.rs` 是生成器，**模板复制属设计**，但已落成插件的样板可下沉到 `plugin-api` 辅助函数（`fsutil` 先例） |
| 插件 `build.rs` | 10 份**逐字节相同**（299 B） | SHA256 全等；可用 workspace 级共享（`include!` 或公共 crate）收敛 |
| 宿主集成测试道具 | `crates/plugin-host/tests/{echo_plugin,fs_fetch_plugins,tools_batch2,tools_batch3,wasm_plugins}.rs` 各持一份 `build_and_stage` / `workspace_dir` / `build_lock` / `artifact_name` / `load_activate` | 5 文件同形；项目已有 `tests/common/mod.rs` 先例（`crates/kernel-agent/tests/common/mod.rs`），可照搬 |
判据：`fn` 名称跨文件定义计数（已剔除 `new/default/len` 等通用名）。建议：先做测试道具收敛（纯测试面、零生产风险），插件样板收敛需评估 ABI 稳定性。

## M10【前端 v2.145 退役残留：sandbox_run】结论需裁定（非无脑删）

| 位置 | 内容 |
|---|---|
| `desktop/src/lib/tool-summary.ts:14,125` | `sandbox_run: "tool.action.sandbox"` 映射 + shell 分支处理 |
| `desktop/src/lib/tool-detail.ts:62,123,332,526` | 图标/类型/输出形状/分支四处 |
| `desktop/src/lib/side-effect.ts:43-44` | `sandbox_run: "irreversible"` |
| `desktop/src/locales/{zh-CN.ts:330, en.ts:328}` | `tool.action.sandbox` 文案 |
判读：`tool-sandbox` 插件已于 v2.145 整目录退役，**新会话不可达**；但**历史会话日志里仍有 `sandbox_run` 工具调用**，重放渲染路径可达（与 M2 的 `ContextCompacted` 同理）。
建议：若产品接受「旧会话里的沙箱行按兜底形态渲染」，可删这 6 处；若要求旧会话忠实还原，则保留并加注释标明「历史渲染专用」。**不宜由本报告替用户拍板。**

## M11【入库体积与文档体量】事实陈述

- 入库二进制：`desktop/src-tauri/resources/pdfium/pdfium.dll` 7.5 MB、`desktop/src-tauri/resources/models.dev/api.json` 5.3 MB（两者均为**有意随包**，v2.131 / v2.127 记档；记入 git 历史后不可逆）。
- 文档体量：`docs/design/CHANGELOG.md` 394 KB、`docs/design/UI shell and patterns.md` 245 KB、`docs/design/ROADMAP.md` 207 KB、`AGENTS.md` 191 KB、`docs/design/DESIGN.md` 139 KB、`docs/design/ui-prototype/index.html` 92 KB（+ css/js 6 件，2026-10-02 原型，仍被设计文档引用——见 D 域）。
- `Cargo.lock` 内 11 个 crate 存在多版本共存（`bitflags`/`hashbrown`/`syn`/`thiserror`/`windows-sys`/`wasmparser` 等），绝大多数来自传递依赖，**不建议**为清版本而强行 `cargo update`（收益低、回归面大）。

## M12【未入库的本地垃圾（诚实记录，不影响仓库）】高置信

| 路径 | 规模 | 说明 |
|---|---|---|
| `target/` | 41,085 文件 / **15.2 GB** | 根 workspace 构建缓存（gitignore） |
| `desktop/src-tauri/target/` | 23,956 文件 / **41.7 GB** | desktop 独立 workspace 的第二份 target，两者合计 ≈ 57 GB |
| `tmp/` | 602 文件 / 20.8 MB | v126–v145 走查脚本/截图/临时 py（gitignore，`tmp/v145-walk/` 等） |
| `desktop/node_modules/` `desktop/dist/` | 145.8 MB + 10.5 MB | 常规产物 |
| `.mimosa/` | 79 文件 / 0.2 MB | 外部工具 hook 基线快照（含 `crates/*` 源码副本，非仓库内容） |
| `plugins/`、`market/` | 未入库（`plugins/*` 被 gitignore，仅 `plugins/README.md` 入库） | 构建暂存产物 |
建议：`cargo clean` 于两个 target（如需保留增量可只清旧 profile）；`tmp/` 按批次归档或整目录删除（走查价值已完成提取，若要留证据建议只留 `v145-walk`）。

## M13【构建产物缺陷：图标被复制成双层目录】中发现

证据：`plugins/tool-shell/0.1.0/assets/icon.svg` 与 `plugins/tool-shell/0.1.0/assets/assets/icon.svg` 内容 SHA256 全等；`memory-sqlite`、`tool-edit`、`tool-fs`、`tool-fetch`、`tool-git`、`tool-mcp`、`tool-notify` 均有同款 `assets/assets/` 嵌套。
判读：**根因已定位**——`scripts/build_plugins.sh:47-49` 的 `cp -R "plugins-src/$id/assets" "$dst/assets"` 在 `$dst/assets` **已存在**时（第二次跑同一个 `stage`，或旧暂存目录残留）会把源目录整体复制**进**目标目录，于是多出 `assets/assets/`（再跑还会继续叠层）。同脚本 `mkdir -p "$dst"` 也不清理旧文件，属幂等性缺陷。
旁证：脚本现在 stage 的 tool-shell 版本是 0.1.3/0.1.4/0.1.5（`build_plugins.sh:84,94,106`），而磁盘上还留着 `plugins/tool-shell/0.1.0`~`0.1.2` 这些**脚本已不再生成**的旧暂存目录——同类残留。
另：`plugins-src/tool-shout` 没有 `manifest.json`，经核对**不是问题**（`build_plugins.sh` 从不 stage 它；市场侧由 `build_market.sh:41-42` 自行 heredoc 生成 manifest）——记为已排除假阳性。
建议：`rm -rf "$dst/assets"` 后再复制（或改 `cp -R src/assets/. "$dst/assets/"`）；是否需要顺带清理 `$dst` 下的陈旧版本目录，由构建流程决定。属工具链 bug（D 域已接手复核）。

## M14【规模门禁的覆盖缺口与基线口径】

- `scripts/check_length.sh` 只 `find crates desktop`（`*.rs *.vue`），**不扫 `plugins-src/`、不扫 `*.ts`**；且脚本恒 `exit 0`（报告式，不拦截）。
- 我按同一规则重算（crates+desktop 的 .rs/.vue，排除 target/node_modules）：**32 个文件 > 400 行**（**含空行口径**，与 AGENTS v2.145 记的「32」吻合）。⚠ 自纠：首轮用 `Measure-Object -Line` 得「28」，该方法**不计空行**，造成系统性少算 4 个（D 域独立复算发现并纠正；本文件相关数字已按含空行口径更新）。最大 = `desktop/src/components/chat/Composer.vue` 806、`desktop/src-tauri/src/organize/tests.rs` 803、`crates/kernel-agent/src/kernel/tests.rs` 782。
- 覆盖缺口实例：`plugins-src/memory-sqlite/src/store.rs` **533 行**、`plugins-src/memory-sqlite/src/tests.rs` **557 行**（含空行口径）**从未出现在门禁输出里**。注意 AGENTS 里「基线 33 个文件超 400 行（插件里 memory-sqlite/src/store.rs 508 行、tests.rs 484 行是最大的）」这句自相矛盾：这两个插件文件根本不在这条脚本的受检集内，且 508/484 与含空行真值不符。
建议：脚本加 `plugins-src`；是否把「超限计数」写成基线比较（见报告建议节）。

## M15【pub 枚举的死变体：编译器盲区】高置信

- `crates/session-store/src/event.rs:140 EffectApplied { effect_id, description }`、`:144 PluginActivated { plugin_id, from, to }`。
- 全仓 `EffectApplied|PluginActivated` 命中 **4 处** = 枚举定义 2 处 + `docs/design/DESIGN.md:476-477` 文档片段 2 处 → **零构造、零 match、零前端引用**。
- 判据：`SessionEvent::<V>` 出现次数 = 0，且裸名出现次数 = 1（仅定义行）。对照组：`ManualIntervention` 12 处、`ContextCompacted` 走完整重放链路。
- 为何 clippy 全绿也发现不了：`dead_code` lint 对 **pub 枚举变体**不告警（crate 外可见即视为可达）。
- 建议：删除两个变体（先用 `git log -S` 确认历史代码也从未写入过该类型），或按「§4.1 事件全集」口径保留并加注释；同类检查应纳入每次「事件枚举变更」批。
- 同一手法对 `AgentEvent`（19 个变体）复算结果：**全部有构造/匹配点**，无死变体。
