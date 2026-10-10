# B · desktop Rust 健康度审计（`desktop/src-tauri/`）

- 审计对象：`F:\Warehouse\AIwork\Locowrk\desktop\src-tauri\`（含 `src/` 全部 Rust、tests、`Cargo.toml`、`tauri.conf.json`、`capabilities/`、`build.rs`）——当前工作树，含未提交的 v2.145 权限模型重设计改动
- 审计时间：2026-10-10
- 范围边界：**不含** `desktop/src`（前端由 C 域负责）。跨域验证时 grep 全仓（`crates/`、`plugins-src/`、`desktop/src`、`docs/`）以判定「无消费者」
- 规模：`src/` 105 个模块声明 / 全部 `.rs` 已逐一核对；`pub` 定义 **652** 条（脚本枚举）
- 判定口径：宣布「无引用」前均全仓 grep；同名多处定义时按「出现次数 == 定义处数」归一（见 §复现命令）
- **只读声明**：本报告是本次审计唯一交付物。审计期间在 `tmp/` 下新建了 5 个一次性 PowerShell 取证脚本（`tmp/audit-deadcode{,2,3}.ps1`、`tmp/audit-deps.ps1`、`tmp/audit-orphan-mods.ps1`）与 1 个 panic 复现源文件 `tmp/paniccheck.rs`——**未触碰任何源码或配置**（`git status` 中 `desktop/src-tauri` 下的 M/D 条目全部是审计开始前就存在的 v2.145 工作树改动，与本次审计无关）。收尾时尝试删除这 5 个脚本**被沙箱拒绝**（`Access to the path … is denied`，本会话文件策略允许在 workspace 内创建但不允许删除），故它们**仍在 `tmp/` 下**；`tmp/` 属未跟踪的本地草稿目录（不入库），其中既有本批 6 个文件也有历批走查脚本与截图。`tmp/paniccheck.rs` 有意保留以坐实 P0 结论，其余 5 个可按需手动清理。

**结论摘要**：契约层**命令名双向零漂移**（85 = 85 = 85，见 H-01），但存在 **1 条可达 panic（P0，已机械复现）**、**3 条「包装壳活着、功能已死」的孤儿命令**、**1 个整模块退役残留**，以及成规模的重复小工具函数与若干热路径冗余读盘/克隆。

---

## 高价值（建议直接排期）

### B-01【真实缺陷 · P0 · 任意非 ASCII shell 命令触发 panic】`extract_windows_abs_paths` 按字节步进却做 `str` 切片
- 位置：`desktop/src-tauri/src/approval.rs:463-495`
- 原文：
  - `:470-473` `let bytes = cmd.as_bytes(); ... let mut i = 0usize; while i < cmd.len() {`
  - `:474` `let is_abs = rest_starts_unc(cmd, i)`（`||` 短路，**每轮必先求值**）
  - `:487` `i += 1;`（else 分支按**字节**步进）
  - `:493-495` `fn rest_starts_unc(cmd: &str, i: usize) -> bool { cmd[i..].starts_with("\\\\") }`
- 为什么是缺陷：`i` 走到多字节 UTF-8 字符的续字节位置时，`cmd[i..]` 不是 char boundary → `panic!("byte index N is not a char boundary")`。命令文本顺序扫描，一旦出现 CJK 就必然命中（3 字节字符的第 2 字节）。
- **可达性（已核）**：`state/mod.rs:313-319` 生产装配 `.with_modes(...).with_workspace(...).with_rules(...)` → `approval.rs:286-291` **每次** `resolve` 都计算 `external = command_touches_outside(...)`（`modes` 为 `Some` 时），而 `tool == "shell"` 时 `:449` 无条件调 `extract_windows_abs_paths(cmd)`。内核决议者对**含 Pure 的全量调用**都 consult，`shell` 更是 Irreversible → 每个含中文的 shell 调用都会进入该函数。
- **机械复现（rustc 1.98.1，逐字复制 `:463-495` 两函数后运行）**：

  | 输入 | 结果 |
  |---|---|
  | `node --version` | `OK -> []` |
  | `git status` | `OK -> []` |
  | `echo 你好` | **PANIC** `start byte index 6 is not a char boundary; it is inside '你'` |
  | `git commit -m "修复"` | **PANIC** `start byte index 16 ... inside '修'` |
  | `type 中文文件.txt` | **PANIC** `start byte index 6 ... inside '中'` |

  复现件：`tmp/paniccheck.rs`（`rustc -O -o %TEMP%\paniccheck.exe tmp\paniccheck.rs` 后运行）
- 为什么必报：这是本次审计**唯一的高危缺陷**，且本项目是中文语料项目（模型生成的中文命令、中文文件名、`git commit -m "中文"` 全是日常形态）；既有测试集**全部使用 ASCII 路径**（`tests_permission.rs:270` 是最接近的一条，仍为纯 ASCII），所以门禁全绿也抓不到。
- 置信度：**高**（机械复现，非推理）
- 建议动作：`while let Some(c) = ...` 改 `char_indices()` 步进；或最小改动——`rest_starts_unc` 改字节判定 `cmd.as_bytes()[i..].starts_with(b"\\\\")`，并把 `i += 1` 换成 `i += cmd[i..].chars().next().map_or(1, char::len_utf8)`。同时补一条含 CJK 的回归测试（`tests_permission.rs` 同文件即可）。
- 删除风险：无（纯 bug 修复，行为对 ASCII 输入逐字节不变）

### B-02【契约死代码】3 条命令「Rust 注册 + api.ts 包装都在，但包装从未被任何组件 import」
- 结论先给：**命令名层面的双向契约完全闭合**——`lib.rs:163-249` 的 `generate_handler!` 85 条 = `#[tauri::command]` 定义 85 条 = 前端 `invoke("…")` 字面量 85 条，**两个方向的差集均为空**。真正的漂移在**下一层**：`api.ts` 的 93 个导出函数中，有 **3 个全仓零 import**——包装壳活着，功能已死。
- 位置与原文：

  | # | api.ts 包装（零 import） | Rust 命令定义 | Rust 注册点 |
  |---|---|---|---|
  | a | `lib/api.ts:105-107` `export function assetGc(): Promise<{scanned;removed}> { return invoke("asset_gc"); }` | `commands/assets.rs:86-87` `pub async fn asset_gc(...)` | `lib.rs:172` |
  | b | `lib/api.ts:419-421` `export function llmListModels(): Promise<ProviderModels[]>` | `commands/llm.rs:62-63` `pub async fn llm_list_models(...)` | `lib.rs:221` |
  | c | `lib/api.ts:607-609` `export function llmProbeWindow(baseUrl, model): Promise<number>` | `commands.rs:366-367` `pub async fn llm_probe_window(...)` | `lib.rs:194` |

- 验证方法：把 `desktop/src` 全部 `.ts/.vue`（**排除 `api.ts` 自身**）做词边界计数，三者均为 **0**；再放宽到全仓（含 `docs/`、`plugins-src/`、`tmp/`）仍无调用点（`docs/design/ROADMAP.md:128`、`UI shell and patterns.md:990` 只是历史记录）。
- 为什么是死代码：
  - **c（`llm_probe_window`）**：唯一用途是「设置面板自动检测窗口」（`ROADMAP.md:128` 原记），v2.137 已把窗口改为 `settings::session_window` 四层链自动识别（模型行 ?? 目录 ?? 档位），设置页的「自动检测」按钮随之下线 → **这是明确的退役残留**，不是「暂时没接」。
  - **b（`llm_list_models`）**：v2.58 模型选择器的数据源，现由设置页/`ProviderSetupDialog` 走 `llm_probe_models`（`commands/llm.rs:166` 的 `list_models_swept`）与 `llm_lookup_model` 承担；`types.ts:1029` 的注释仍写着「llm_list_models 返回组」= 注释也在指向旧入口。
  - **a（`asset_gc`）**：`api.ts:104` 自述「走查/开发者」，但生产有自动 GC（`state/mod.rs:613-622`）承接；命令是手动触发器，无调用方。
- 连带结论（与 C 域一致，C-frontend.md C2 已独立发现，此处补 Rust 侧深度）：删 3 行包装后，Rust 侧这 3 条命令失去**唯一**入口；其中 c 连带整模块死（见 B-03）。
- **额外连带（本轮新发现）**：`ProviderModels` 接口也随 b 一起成为孤儿——定义 `desktop/src/types.ts:1030` `export interface ProviderModels {`，前端仅两处引用：`api.ts:40`（import）与 `api.ts:419`（唯一使用点，即死包装）。故 b 的退役是**命令 + DTO 投影 + 前端接口**三件一起清。
- 置信度：**高**
- 建议动作：① 删 `api.ts` 3 个导出函数；② `llm_probe_window` + `llm/probe.rs` 整套退役（B-03）；③ `llm_list_models` 退役（`llm/models.rs::list_models` 仍被 probe/sweep 使用，**只删命令与 DTO 投影**，勿动 `models.rs`）；④ `asset_gc` 若确要保留走查面，在命令 doc 注明「无 UI 入口，仅 CDP 走查用」，否则一并删（`assets/gc.rs` 仍被自动 GC 使用，**不可删模块**）。
- 删除风险：低。**风险点**：`asset_gc` 若外部走查脚本（不在仓库内）直接按名 invoke，删除会破坏该脚本——这也是它唯一的存续理由，需用户裁定。

### B-03【退役残留 · 整模块】`llm/probe.rs` 的唯一消费者是孤儿命令 → 全模块可退役
- 位置：`desktop/src-tauri/src/llm/probe.rs`（50 行）+ `llm/probe/tests.rs`（27 行）+ 声明 `llm/mod.rs:17` `pub mod probe;`
- 原文：`probe.rs:1-3` `//! Ollama /api/show 上下文窗口探测（S2，清单 §5 自动提取来源）：设置面板 "自动检测" 按钮的数据源。`
- 为什么是死代码：`probe_context_window` 全仓只有 1 个调用点 `commands.rs:371`（在孤儿命令 `llm_probe_window` 内）；`llm/probe/tests.rs` 只测本模块。而 `probe.rs:3` 自述的消费者（设置面板「自动检测」按钮）在 v2.137 已不存在 → **整模块 + 其测试 + 该命令 = 约 80 行可删子图**。
- 反例核查（防误删）：`llm/models.rs::list_models` 不在此簇内（被 `llm_probe_models` 经 `list_models_swept` 使用，`commands/llm.rs:166` → `models.rs:138`），四层链 `session_window`（`settings/llm.rs:365-398`）也不依赖 probe → 删 probe 不断任何活链路。
- 置信度：**高**
- 建议动作：删 `llm/probe.rs`、`llm/probe/tests.rs`、`llm/mod.rs:17` 声明、`commands.rs:366-375` 命令、`lib.rs:194` 注册行、`api.ts:605-609` 包装。同步清 `settings/embedding.rs:13`/`llm/models.rs:21` 的重复 Ollama 常量（B-07）。
- 删除风险：低。唯一不确定：是否有仓库外脚本按名 `invoke("llm_probe_window")`——但即使有，它走的也是已下线的按钮语义，删前问一句用户即可。

### B-04【陈旧抑制 + 错误注释】`#[allow(dead_code)]` 挂在**已被读取**的字段上
- 位置：`desktop/src-tauri/src/state/app_state.rs:22-26`
- 原文：
  ```rust
  pub struct AppState {
      /// 阶段八的窗口级命令（对话框、多窗口）从这里取句柄；当前
      /// 事件出口已是共享 sink，字段暂无读取方。
      #[allow(dead_code)]
      pub app: AppHandle,
  ```
- 为什么是垃圾：注释断言「字段暂无读取方」，但 **`commands.rs:110` 就是读取方**：`crate::title::spawn_auto_title(state.app.clone(), …)`（v2.143 新增「测试连接」/自动标题批引入）。即抑制属性与注释都已过期，属**代码腐烂**——它会掩盖将来真正的死字段，且误导读者以为该字段可删。
- 置信度：**高**（grep `state.app` 命中 `commands.rs:110`）
- 建议动作：删 `#[allow(dead_code)]` 与那句注释，改成「窗口级命令（对话框/多窗口）句柄；title 钩子等经此取 handle」。
- 删除风险：无（删抑制后编译期即可自证是否真无消费者——这正是期望效果）

---

## 中价值（重复实现与热路径优化）

### B-05【重复实现】`now_ms()` 定义 7 处、`now_secs()` 2 处，其中 5 处逐字节相同
- 位置与原文：

  | 文件:行 | 签名 | 体 |
  |---|---|---|
  | `media/mod.rs:289` | `fn now_ms() -> u128` | `.map(\|d\| d.as_millis()).unwrap_or(0)` |
  | `llm/catalog.rs:318` | `fn now_ms() -> u64` | `.map(\|d\| d.as_millis() as u64).unwrap_or(0)` |
  | `llm/diagnose.rs:206` | `fn now_ms() -> i64` | `.map(\|d\| d.as_millis() as i64).unwrap_or(0)` |
  | `organize/mod.rs:112` | `fn now_ms() -> i64` | 同上（逐字节相同） |
  | `runner.rs:231` | `fn now_ms() -> i64` | `unwrap_or_default().as_millis() as i64` |
  | `scheduler/store.rs:65` | **`pub(crate) fn now_ms() -> i64`** | 与 `organize/mod.rs` 相同 |
  | `organize/tree_tools.rs:280` | `fn now_ms() -> i64` | 与 `organize/mod.rs` 相同 |
  | `memory.rs:515` / `memory_reconcile.rs:242` | `fn now_secs() -> i64` | 两者逐字节相同 |
- 为什么是垃圾：`scheduler/store.rs:65` **已经是 `pub(crate)` 的共享版本**，其余 6 处却在各写私有副本；同形助手散落 9 处正是任务书点名的形态。时间戳口径（毫秒 vs 秒）本可一次性收敛，现分散在 9 个文件里，将来改口径必漏。
- 置信度：**高**（脚本枚举 + 逐条读体比对）
- 建议动作：新建 `src/timeutil.rs`，导出 `pub(crate) fn now_ms() -> i64` / `now_secs() -> i64`；各调用点 `use crate::timeutil::now_ms;`。`u128`/`u64` 两处按调用点窄化（`as u64`）或保留专用函数并注明理由。
- 删除风险：低（纯内部工具；唯一注意 `media/mod.rs` 的 `u128` 消费点需 `as u64` 收窄）

### B-06【重复实现】`clip()` 定义 6 处、4 种语义
- 位置与原文：
  - `export.rs:12-18` `fn clip(s: &str) -> String`（固定 `CLIP=400`；**截断后缀 `…（截断）` + 换行替空格**）
  - `browser/console.rs:46-48` `fn clip(s: &str) -> String { s.chars().take(TEXT_CLIP).collect() }`
  - `notifications.rs:80-82` `fn clip(s: &str, n: usize) -> String { s.chars().take(n).collect() }`（与上一处**语义完全相同**，仅元数不同）
  - `organize/search_titles.rs:235-241` `fn clip(s: &str, max: usize)`（超长追加 `"…"`）
  - `subagent_verify.rs:130-136` `fn clip(s: &str, n: usize)`（超长 `push('…')`——与上一处**结果等价**）
  - `scheduler/tools.rs:296-298` `fn clip(s: &str, max_chars: usize) { s.trim().chars().take(max_chars).collect() }`（**多一个 trim**）
- 为什么是垃圾：6 个同形助手、4 种语义（无省略号 / `…` / `…（截断）`+换行折叠 / trim+截断），且两组实为同一逻辑的复制。今后「转录里怎么截断」这类口径要改 6 处。
- 置信度：**高**
- 建议动作：收敛到 `src/text.rs` 两个函数：`clip_chars(s, n)`（无标记）与 `clip_ellipsis(s, n)`（超长追加 `…`）；`export.rs` 的特例（换行折叠 + 中文标记）保留为显式包装 `clip_transcript(s)`，因为它的**输出文案对用户可见**，合并会改变导出的 Markdown 文本。
- 删除风险：低。**注意** `export.rs` 的实现改动会影响导出的 Markdown 字节，属用户可见面，收敛时保持其输出不变（只去重其余 5 处）。

### B-07【重复实现 · 常量】Ollama 默认端点常量 3 份
- 位置：`llm/models.rs:21` `pub const DEFAULT_OLLAMA_BASE: &str = "http://localhost:11434";`、`llm/probe.rs:10` 同名同值、`settings/embedding.rs:13` `pub const OLLAMA_DEFAULT_BASE: &str = "http://localhost:11434";`
- 为什么是垃圾：同一端点三份字面量、两种命名；B-03 退役 `probe.rs` 后剩两份仍不同名。
- 置信度：**高**
- 建议动作：留 `llm/models.rs:21` 一份，`settings/embedding.rs` 改为复用它（或反向），删 `probe.rs:10`。
- 删除风险：无

### B-08【优化 · 热路径】每次审批决策重复解析会话上下文 ~4 次，每次含 1 次旁车读盘 + N 次 `canonicalize` 系统调用
- 位置：`approval.rs:140-143`（`workspace_of`）被 `resolve` 调用于 `:288`（经 `command_touches_outside`）、`:298`、`:340`、`:363`；下落 `workspace.rs:52-61`
- 原文：`workspace.rs:53` `let workspace = read_meta(&sessions_dir, session_id).and_then(|m| m.workspace);` / `:56-57` `let mut roots = vec![canon_or_raw(&data_root)]; roots.extend((fs_roots_of)().iter().map(|r| canon_or_raw(Path::new(r))));` / `:32-37` `canon_or_raw` → `p.canonicalize()`
- 为什么是优化点：`ctx(sid)` 每次调用 = **1 次 `<id>.meta.json` 同步读** + **1 次 `data_root` canonicalize** + **每个 fsRoot 一次 canonicalize**。一次 `resolve` 里最多 4 次 → 4 次读盘 + 4×(1+N) 次 `canonicalize` 系统调用，**对每个工具调用**（内核含 Pure 全量 consult）。`scheduler` 与 UI 侧另有调用。
- 置信度：**高**（调用点逐条数出）；影响量级**中**（单次成本低、频次高）
- 建议动作：在 `resolve` 开头解析一次并复用：`let ctx_now = self.ctx_of.as_ref().map(|c| root_sid.as_deref().map(|s| c(s)));`，`workspace_of` 改吃已解析值。**不要**做跨调用缓存——`workspace.rs:39-46` 明写「保存即生效，读当前值」是有意设计；`fsRoots` 的 canonicalize 结果可考虑加一代际缓存（设置变更时失效），但那需要新的失效通路，建议只做函数内 memo（零语义风险）。
- 删除风险：无（函数内 memo 不改变可观测语义）

### B-09【优化 · 热路径】同一次决策把同一份规则文件读盘 + JSON 解析两遍
- 位置：`approval.rs:299` `rules.denies(&ws, &req.tool, subject)` 与 `:341` `rules.allows(&ws, &req.tool, subject)`；实现 `permission_rules.rs:107-122`
- 原文：`permission_rules.rs:108-112` `let _g = self.io.lock()…; self.read(workspace).deny.iter().any(...)`；`:117-121` 同形 `allow`；`:87-95` `read()` 内 `std::fs::read_to_string(self.file_of(workspace))` + `serde_json::from_str`
- 为什么是优化点：deny 与 allow 是**同一个文件的两半**，现各自加锁 + 读盘 + 反序列化一次 → 每次非 Pure 审批决策 2 次读盘 + 2 次解析。AGENTS 明载「查询直读无缓存」是**有意设计**（人改 JSON 即生效兼撤销出口）——本报告不主张加缓存，只主张**一次读、两个判定**。
- 置信度：**高**
- 建议动作：新增 `fn verdicts(&self, ws, tool, subject) -> (bool /*deny*/, bool /*allow*/)`，内部一次 `read()`；`denies()`/`allows()` 保留为薄包装（供测试/其他调用方）。文件仍每次直读，撤销出口语义不变。
- 删除风险：无

### B-10【优化】`LlmConfig` 全量克隆（含各档明文 Key 串）每轮 2 次
- 位置：`commands.rs:83-87` `crate::settings::session_window(&state.settings.llm_config(), ov.as_ref(), Some(&state.catalog))`；`organize/mod.rs:355-358` 同形（在 `auto_collapse` 内，经 `after_turn` **每轮**执行）
- 原文：`settings.rs:210-212` `pub fn llm_config(&self) -> LlmConfig { self.inner.lock()…llm.clone() }`（`LlmConfig { active, providers: Vec<ProviderProfile>, … }`，`ProviderProfile` 含 `api_key: Option<String>`）
- 为什么是优化点：只读一个 `window_tokens` 却克隆整个档位表（`Vec<ProviderProfile>`，含每个档的 base_url/model/headers/api_key）。每轮 2 次，且克隆内容含明文凭据串（仅内存，但扩大敏感数据驻留面）。附带：`commands.rs:82/84/101` 连续三次独立取锁（`organize_threshold_tokens` / `llm_config` / `max_turn_steps`），可合并为一次锁。
- 置信度：**高**（代码路径确定）；影响量级**低-中**（档位表通常 <10 条）
- 建议动作：`session_window` 改吃 `&LlmConfig`；`SettingsStore` 增 `pub fn with_llm<R>(&self, f: impl FnOnce(&LlmConfig) -> R) -> R` 闭包访问器（不 clone）；`auto_collapse` 与 `agent_chat` 改用它。
- 删除风险：低（`llm_config()` 是 `pub`，先 grep 全部调用点再改签名；`commands/llm.rs` 的档位命令可能仍需 owned 值）

### B-11【优化】`turn_id` 每轮重扫全量日志 → 全会话 O(n²)
- 位置：`runner.rs:73-80`
- 原文：
  ```rust
  let turn_id = TurnId(
      session.events().iter()
          .filter(|e| matches!(e, SessionEvent::TurnStart { .. }))
          .count() as u64 + 1,
  );
  ```
- 为什么是优化点：每轮开场对**整段历史**做一次全遍历计数。第 k 轮的日志长度 ≈ O(k)，故整个会话累计 O(n²)。1000 轮的会话每轮扫上万条事件。
- 置信度：**高**
- 建议动作：`Session` 上维护 `turn_count` 计数（append `TurnStart` 时自增），或退一步取「最后一条 `TurnStart` 的 `turn_id` + 1」（`iter().rev().find_map`，仍是 O(n) 但通常在尾部——不彻底）。推荐前者。
- 删除风险：低-中（`Session` 属 `crates/session-store`，跨域改动；若不动内核，可用 `rev().find_map` 折衷，代价是回退/截断后语义要复核）

### B-12【优化】投影路径两处无条件全扫 + 三处可省 `format!` 分配
- 位置（全扫）：`runner.rs:278-284`
  ```rust
  let uncertain = session_store::scan_uncertain(events);
  let mut has_call_event: HashSet<String> = HashSet::new();
  for e in events { if let SessionEvent::ToolCall { call_id, .. } = e { has_call_event.insert(call_id.0.clone()); } }
  ```
- 为什么是优化点：`has_call_event` 是**无条件**构建的 `HashSet<String>`（含 N 次 `String` 克隆），而它只在 `uncertain.dangling` 非空时（`runner.rs:363/396`）才被查询——即**只有在崩溃恢复场景才有用，正常会话 100% 白做**。`session_history` 重放与 `project_new_events` 每次增量都走这里。
- 位置（分配）：`approval.rs:541` `t != r && !t.starts_with(&format!("{r}\\"))`（每个 root × 每个 token 一次 `String` 分配，可改 `t.strip_prefix(&r).is_some_and(|rest| rest.starts_with('\\'))`）；`permission_rules.rs:61` `subject.starts_with(&format!("{prefix} "))`（每条候选规则一次分配，可改 `subject.strip_prefix(prefix).is_some_and(|r| r.starts_with(' '))`）；`export.rs:13-17`（见 B-24）。
- 置信度：**高**
- 建议动作：`has_call_event` 改懒建（`if uncertain.dangling.is_empty() { skip }`，或改成只在命中时按需 `events.iter().any(...)`）；两处 `format!` 改 `strip_prefix`。
- 删除风险：无

### B-13【优化 · async 阻塞】`session_history` 在大锁内同步做 O(n) 克隆投影
- 位置：`commands.rs:167-183`
- 原文：`:181-182` `let session = lock.lock().await; Ok(runner::project_events(session.events(), &session_id))`
- 为什么是优化点：`project_events`（`runner.rs:265-501`，237 行）对整个事件日志做**深度克隆**（每条 `content.clone()`、`args.clone()`、`output.clone()`、`parts.clone()`）；此处它跑在 **per-session `tokio::sync::Mutex` 持有期内**、且在 **async 上下文里同步执行**。长会话（数千事件）切会话时：① 阻塞 tokio worker 线程；② 持有会话锁 → 并发的一轮 `agent_chat` 必须等这次重放结束。同类模式见 `session_search`（`commands/sessions.rs:179` → `session_log::search` → `search_sessions` 全目录扫描）、`session_export:228`、`asset_gc`、`permission_rules*`——均以 `std::fs` 同步 IO 跑在 `#[tauri::command] async fn` 里（本 crate 全仓 `std::fs::*` 命中 188 处，**含测试夹具**；上列生产路径——`commands/sessions.rs:228` 的 `std::fs::write`、`commands/assets.rs:41` 的 `std::fs::read`、`memory_reconcile.rs:83/134/166`、`permission_rules.rs:88/163/171`——已逐条目检确在 async 命令路径）。
- 置信度：**高**（代码结构确定）；影响量级**中**（取决于会话长度，长会话可见）
- 建议动作：① `session_history` 改成「锁内只 `events().to_vec()`（或取 `Arc<[SessionEvent]>` 快照），锁外投影」；② 重 IO 命令（search/export/gc/reconcile）改 `tauri::async_runtime::spawn_blocking` 包裹。
- 删除风险：低（快照语义不变——投影是纯函数；唯一注意 `to_vec()` 本身仍是一次克隆，但脱离了锁与 async 线程）

### B-14【优化 · IO 放大】`append_slice` 每个事件一次 open/write/close
- 位置：`session_log.rs:63-74`
- 原文：
  ```rust
  for e in &session.events()[from..] {
      if let Err(err) = append_event(&path, e) { … }
  }
  ```
- 为什么是优化点：一个 step 通常落 3–6 条事件（`ToolCall`/`ToolResult`/`AssistantMessage`/`LlmUsage`/`ContextUsage`…），每轮 N 步 → **每轮 3N–6N 次文件 open + close**。`append_slice` 被 `runner.rs:90/139/219/572` 四处调用（每 step 至少 1 次）。
- 置信度：**高**
- 建议动作：`append_slice` 内用单个 `OpenOptions::new().append(true).open(path)` 句柄 + 逐行写；或在 `session-store` 增批式 `append_events(path, &[SessionEvent])`（跨域，A 域已记同族问题则可合并处理）。
- 删除风险：低（原子性语义不变：仍是追加；注意现实现「失败即 return」的短路语义要保留）

---

## 结构 / 规模（超限文件的拆分建议）

> 口径：`scripts/check_length.sh:12` 建议值 400 行，超限走「三问」（`:3-7`）。以下给出**实测行数**（比 AGENTS 记档的旧数字有增长，按当前工作树为准）。

### B-15【结构 · 最严重】`state/mod.rs` 698 行 / 顶层仅 2 个函数——`init_state` 独占 **41–676 行 = 636 行**
- 位置：`desktop/src-tauri/src/state/mod.rs:41` `pub fn init_state(app: AppHandle) -> AppState {` … `:676` `}`；下一条 `:678` `fn open_plugin_service(`
- 症状（三问逐条）：①「只做一件事」= **否**——单函数内串起数据根解析、设置打开、keyring 迁移、资产库、记忆库、记忆文件、会话表恢复、插件服务、内核装配、中间件链、权限模式库、项目规则库、审批中心、调度引擎、托盘、通知、浏览器控制台、目录 store、EWS 记录器…；②「名字准确」= 否（`init_state` 是过程名不是职责名）；③「改动是否常牵动不相关部分」= **是**——任何子系统的构造参数变化都必须改这一个函数（如 v2.145 加 `permission_rules` 就在 `:313-319` 处插入）。
- 置信度：**高**（函数跨度已实测）
- 建议动作：按子系统抽出私有构造器，返回小结构体或 `impl` 字段组，`init_state` 退化为「读配置 → 依次调用 → 组装 `AppState`」的 ~80 行编排：`build_llm_and_catalog()`、`build_memory()`、`build_kernel_and_middleware()`、`build_approval(&permission_modes, &session_ctx, &permission_rules)`、`build_organizer_and_tree()`、`build_scheduler()`、`restore_sessions()`。**注意**：`AppState` 字段间存在构造顺序依赖（sink 先于 approval、catalog 先于 session_window），抽函数时要显式传参而非各自重建。
- 删除风险：中（纯重构，但装配顺序是隐性契约——必须保持 sink/catalog/permission_modes 的创建次序；建议重构后以「启动日志两行 + desktop 356 测试」为回归判据）

### B-16【结构】`organize/mod.rs` 621 行 / 19 个顶层函数，混装触发策略、投影 IO、泄压、通知、异步任务管线
- 位置与症状：`organize/mod.rs` — 触发判定 `plan_auto:242` / `plan_manual:310` / `anchor:205` / `record:216`；投影读写 `load_tree:195`；收束钩子 `after_turn:338`；泄压 `auto_collapse:349`；人读层同步 `sync_tree_md:387`；通知 `notify_done:152` / `notify_paused:182`；**异步任务管线 `spawn:418` → `run_task:426`(→452) → `pipeline:453-599`（147 行）**；构造 `new:125` / `build_organizer:600`。
  - 三问：① 否（至少 4 件事：阈值判定 / 树投影 IO / 人读层同步 / LLM 后台管线）；② 否（「organize」是域不是职责）；③ 是（改触发阈值要碰 `auto_collapse` 与 `plan_auto`，改管线重试要碰 `pipeline`，互不相关）。
- 置信度：**高**
- 建议动作：拆 `organize/trigger.rs`（`plan_auto`/`plan_manual`/`anchor`/`record`/`after_turn`）、`organize/collapse.rs`（`auto_collapse`/`sync_tree_md`）、`organize/task.rs`（`spawn`/`run_task`/`pipeline`/`notify_*`），`mod.rs` 留 `Organizer` 结构体定义 + 门面方法。目录已是模块化形态（`assemble/client/debug/edit/project_history/recovery/search_titles/summary/tree_tools/view`），再加三文件符合既有风格。
- 删除风险：低-中（`Organizer` 字段 `pub(crate)` 可见性需跨文件保持；`buffer_unordered` 等在 `pipeline` 内的并发语义不变）

### B-17【结构】`runner.rs` 592 行 / 9 函数——`project_events` 单函数 **265–501 行（237 行）**
- 位置与症状：`:265` `pub fn project_events(events: &[SessionEvent], session_id: &str) -> Vec<AgentEvent>` 起，`:501` 结束；内含 20 个 `SessionEvent` 变体的巨型 `match`。文件同时承载「轮次主循环」（`run_turn:53-197`）与「日志→UI 投影」（`project_new_events`/`project_events`/`record_stats`/`record_usage`/`reported_input_tokens`/`catalog_estimate`）两类职责。三问：② 否（文件名叫 runner，一半内容是投影）。
- 置信度：**高**
- 建议动作：拆 `runner/projection.rs`（`project_events` + `project_new_events` + 其专属测试），`runner.rs` 留 `run_turn`/`finish_turn`/`record_usage`/`record_stats`。注意 `project_events` 是 `pub` 且被 `commands.rs:182` 与 `session_history` 消费——移动后需 `pub use` 重导出或改调用路径。
- 删除风险：低

### B-18【结构】`settings/llm.rs` 682 行 / 17 函数，四段不同职责同文件
- 位置与分段：① 请求头校验与网络代理（`validate_headers:44`、`NetworkProxy:76-115`）；② 目录合成视图（`ProviderModel:116`、`PricingView:137`、`SourcesView:148`、`ProviderModelView:160`、`synthesize_view:182`、`synthesize_view_raw:197`）；③ 子代理钉档（`SubagentPin:279-306`）；④ 档位/配置与平面投影（`ProviderProfile:307-349`、`LlmConfig:350-357`、`session_window:365-398`、`impl LlmConfig:400-533`）；⑤ 投影与更新 DTO（`LlmSettings:534-564`、`ProviderView:565`、`LlmSettingsView:585`、`ProviderInput:604`、`LlmUpdate:625`、`new_profile_id:636`）。
  - 三问：① 否（校验 / 视图合成 / 持久态 / 更新 DTO 四件事）；③ 是（加一个目录字段要动 `synthesize_*` + `ProviderModelView` + 前端契约三处，与 `session_window` 无关）。
- 置信度：**高**
- 建议动作：拆 `settings/llm/view.rs`（① + ②，目录合成）与 `settings/llm/model.rs`（③④⑤，持久态与投影），`llm.rs` 保留 `LlmConfig` 与 `session_window` 门面。**注意** `settings.rs:31-35` 有 14 个 `pub use` 再导出，拆文件后保持这些再导出路径不变（前后端契约与 `settings/tests/llm.rs` 依赖它）。
- 删除风险：低-中（`settings/update.rs:63-67` 调 `next.llm.apply_update`，`state`、`commands/llm.rs` 大量消费再导出——以「不改 `pub use` 名单」为安全线）

### B-19【结构 · 测试文件】`organize/tests.rs` 867 行 / 31 测试函数，横跨 6 个主题
- 位置：`desktop/src-tauri/src/organize/tests.rs`
- 症状：单个测试文件同时覆盖 ① 触发布尔与熔断（`incremental_trigger_then_rebound_pauses_auto`、`batch_without_window_never_auto_fires`）、② 崩溃恢复（`crash_recovery_redispatches_pending_task`、`recovery_gives_up_when_session_file_deleted`）、③ 投影视图（`view_bundles_and_original_read_from_disk`、`project_view_flattens_across_sessions`）、④ 泄压（`auto_collapse_relieves_oldest_expanded`）、⑤ 章节编辑（`edit_chapter_*`、`delete_chapter_*`）、⑥ 检索与跨会话（`search_titles_*` ×2、`project_history_*` ×2）、⑦ 段落总结（`recorder_summary_stored_then_delivered_at_next_turn_opening`、`degraded_run_carries_no_summary_*`）。三问：① 否。
- 同类：`llm/tests.rs` **786 行 / 22 函数**（wire 形状 / SSE / 重试退避 / 端点归一 / dialect / 多模态投影六主题）、`tests_approval.rs` **640 行 / 20 函数**（旧的豁免/超时 + v2.145 新增的项目规则/子会话改道四主题混装）。
- 置信度：**高**（实测行数与函数主题）
- 建议动作：套用**仓库已有的先例**——`tests_session_log/`（`mod.rs` + `context_usage.rs`）与 `organize/assemble/tests.rs` 的目录式拆分：`organize/tests/{trigger,recovery,view,edit,search,summary}.rs` 由 `organize/tests/mod.rs` 声明；`llm/tests/{wire,sse,retry,multimodal}.rs`；`tests_approval/` 把 v2.145 四测独立成 `v145_rules.rs`。既有共享脚手架（`temp_root`/`settings_with`/`session`/`good_reply`）上移到 `mod.rs`。
- 删除风险：低（纯测试重组；注意 `use` 提升与 `#[cfg(test)] mod tests;` 声明同步）

### B-20【结构】其余超限文件实测（供排期参考，本轮不逐条给拆案）
- 实测 >400 行清单（`Get-Content | Measure-Object`，含测试）：`organize/tests.rs 867`、`llm/tests.rs 786`、`state/mod.rs 698`、`settings/llm.rs 682`、`tests_approval.rs 640`、`organize/mod.rs 621`、`memory.rs 606`、`runner.rs 592`、`llm/client.rs 575`、`approval.rs 543`、`settings/tests/llm.rs 533`、`commands/llm.rs 502`、`commands.rs 490`、`llm/wire.rs 487`
- 观察：`approval.rs 543`（v2.145 权限重设计的核心）与 `commands/llm.rs 502` 属「同职责面自然增长」，按三问可暂不拆；`memory.rs 606 / 28 函数`（memory 域 + reconcile + recall + repo_context 四主题）与 `llm/client.rs 575`（HTTP 客户端 + 重试 + 五分类测试 + 流式）有明确第二职责，建议排入下一批。
- 置信度：**高**（行数为实测）；拆分取舍**中**（属工程判断，留给用户裁定）

---

## 低价值 / 记录项

### B-21【垃圾 · 文案】面向模型的工具报错里写死 Windows 盘符示例
- 位置：`desktop/src-tauri/src/browser/tools.rs:119-123`
- 原文：`"path must be absolute (e.g. F:\\dir\\page.html)"`
- 为什么是垃圾：这是一条**回给模型的错误信息**（会进上下文、会被模型照抄），却把示例钉成 Windows 形态。macOS/Linux 构建下该示例是错的，会诱导模型生成非法路径；而 `Cargo.toml:80-85` 明确支持三平台（keyring 三 target）。同文件 `:186` 的测试也在断言 `F:\a\b.html`。
- 置信度：**高**（属跨平台文案缺陷）；影响**低**
- 建议动作：改平台中立的「e.g. `/dir/page.html` or `F:\\dir\\page.html`」，或按 `cfg!(windows)` 分支。
- 删除风险：无

### B-22【垃圾 · 仅为测试保活的再导出】`#[allow(unused_imports)] pub use`
- 位置：`desktop/src-tauri/src/plugins_root.rs:15-18`
- 原文：
  ```rust
  // lib 本体只用 find_artifact；dll_name 由 tests_plugins_root.rs 经此路径
  // 引用（保持测试里 `plugins_root::dll_name` 的既有写法）。
  #[allow(unused_imports)]
  pub use plugin_service::builtin::{dll_name, find_artifact};
  ```
- 为什么算垃圾（轻）：注释已诚实说明——`dll_name` 在生产代码中**零使用**，其唯一消费者是 `tests_plugins_root.rs:8/38/50/91`；为一个测试写法而给生产模块挂 `allow(unused_imports)`，是用抑制属性换测试便利。属「有意但可更好」。
- 置信度：**高**（grep 确认 `find_artifact` 是 lib 唯一真用者，`dll_name` 仅测试）
- 建议动作：改为 `pub use plugin_service::builtin::find_artifact;` + `#[cfg(test)] pub use plugin_service::builtin::dll_name;`，或让测试直接 `use plugin_service::builtin::dll_name`（更干净，去除生产面的无用导出）。
- 删除风险：无（测试同文件可同步改 import）

### B-23【占位/仅测试助手】2 处「为将来保留」的 pub 方法
- 位置 a：`desktop/src-tauri/src/settings/embedding.rs:40-49`
  ```rust
  /// 智能层是否就绪（…）。组装点用 `EmbedClient::from_settings` 直接裁决；
  /// 本 helper 留给设置侧语义判断（embedding/tests.rs 与未来的 UI 就绪提示）。
  #[cfg_attr(not(test), allow(dead_code))]
  pub fn is_ready(&self, provider: &str, llm_base: Option<&str>) -> bool {
  ```
  消费者仅 `settings/embedding/tests.rs:36/43/50/51`。注释自陈「未来的 UI 就绪提示」= **推测性 API**，生产零消费者。
- 位置 b：`desktop/src-tauri/src/memory.rs:269-273`
  ```rust
  /// 写入/覆盖一条记忆（store 基础 upsert；生产写路径 = memory-sqlite
  /// 插件的 memory_write 工具与管理页 update_value，本方法当前只服务测试播种）。
  #[cfg_attr(not(test), allow(dead_code))]
  pub fn write(
  ```
  注释自陈「只服务测试播种」——诚实且边界清晰，**但**生产写路径在插件侧而宿主保留重复写实现，属职责重叠；若插件侧契约稳定，可把 `write` 降为 `#[cfg(test)]`。
- 置信度：**高**（两者均 `cfg_attr(not(test), allow(dead_code))` + 调用点全在 tests）
- 建议动作：`is_ready` 若无近期 UI 计划 → 直接删（`from_settings` 已覆盖裁决）；`memory::write` → 改 `#[cfg(test)]`，让「生产写路径唯一」显式化。
- 删除风险：低（`is_ready` 删除只影响其自身测试；`memory::write` 降 cfg 前确认无 `#[cfg(not(test))]` 路径调用——grep 显示仅 `memory_files/tests.rs:149` 等测试）

### B-24【优化 · 微】`export.rs` 的 `clip` 对同一字符串做 3 遍 `chars()` 遍历
- 位置：`desktop/src-tauri/src/export.rs:12-18`
- 原文：`:13` `let mut out: String = s.chars().take(CLIP).collect();` `:14` `if s.chars().count() > CLIP {` `:17` `out.replace('\n', " ")`
- 为什么是优化点：① `chars().take(CLIP).collect()` 一遍；② `chars().count()` 又一遍全串（只为判是否超长）；③ `replace` 再分配一遍。可一次 `char_indices` 定位第 CLIP 个字符边界，判定与截取同源，省一次全串扫描 + 一次 `String` 分配。
- 置信度：**高**（语义确定）；影响**低**（导出是低频操作）
- 建议动作：若采纳 B-06 的统一 `clip_ellipsis(s, n)`，此处的 `count()` 冗余可随实现一并消除（该函数同样有 `s.chars().count() <= max` 前置判断，可合到 `take` 的 `next()` 探测）。**注意保持输出字节不变**（用户可见的 Markdown 导出）。
- 删除风险：无

### B-25【注释腐烂】退役模块名仍出现在注释里（纯可信度损耗）
- 位置与原文：
  - `desktop/src-tauri/src/state/app_state.rs:77` `/// ToolExposure 的活配置（⑫，同 compact_cfg 的换入模式）。` ——`compact_cfg` 随 v2.137 ctx-compact 桌面链退役，已无此物。
  - `desktop/src-tauri/src/scheduler/mod.rs:9` `//! 不受影响——与 usage / experiences 同纪律。` 与 `scheduler/store.rs:2` `//! experiences 三库同款"一关切一库"先例` ——`experiences` 模块已在 v2.136 整模块删除。
  - `desktop/src-tauri/src/approval.rs:435` `/// （v2.145：`sandbox_run` 随插件退役移出扫描面。）` ——这条是**正确的退役记档**（保留，见下「有意保留」）。
- 为什么算垃圾（轻）：前三条引用的是**已不存在的模块/配置**，读者无法验证，属注释腐烂；不影响运行。
- 置信度：**高**
- 建议动作：`app_state.rs:77` 改为描述当前事实；`scheduler` 两处把「experiences 先例」改成「一关切一库先例（见 memory.sqlite3）」。
- 删除风险：无

---

## 待验证 / 不确定（不凑数，附定论命令）

1. **`settings.rs:174-190` 的窗口迁移只检查活动档**：`if settings.llm.flat().window_tokens.is_none() && settings.context.window_tokens.is_some()` —— `flat()` 只投影**活动档**。若旧文件的 `context.windowTokens` 存在而活动档窗口已设、其他档未设，迁移被跳过，`context.windowTokens` 作为幻影键留在盘上（下次保存消失）。**是否算问题取决于「多档迁移」是否是需求**。
   - 定论命令：`cargo test --manifest-path desktop/src-tauri/Cargo.toml settings::tests::window` 后再人工构造「两档 + 旧 context.windowTokens」settings.json 跑 `settings_get` 看投影。
   - 当前判断：**无害**（幻影键有 serde 容错，`context.rs:9-10` 已明载「保留仅为读旧 settings.json」），故未计入发现。

2. **`capabilities/default.json:5-7` 只覆盖 `main`，`tauri.conf.json:14-23` 另建 `splash` 窗**：splash 零脚本（`lib.rs:98` 自述「静态 splash.html，零脚本」），故当前不需要 capability。**若将来给 splash 加任何脚本会静默失败**。
   - 定论命令：`rg -n "script" desktop/public/splash.html`（确认仍为零脚本）。
   - 当前判断：**健康**，已计入下方「已核查为健康」并留一句风险提示。

3. **`is_outside` 的「大小写不敏感」归一是否覆盖 UNC/verbatim 全形态**：`approval.rs:533-537` 去掉 `\\?\` 前缀后小写比较，但 `\\?\UNC\server\share` 形态未处理（`strip_prefix("\\\\?\\")` 后剩 `UNC\server\share`）。
   - 定论命令：在 `tests_permission.rs` 现有 `is_outside` 测试旁加一条 `is_outside(r"\\?\UNC\srv\share\f", &[PathBuf::from(r"F:\ws")])` 断言。
   - 当前判断：**宁漏勿错杀的有意边界**（`approval.rs:432-434` 明载只识别盘符与 UNC 两形态、漏报由档位兜底），故不计为缺陷，仅记录。

---

## 已核查为健康的部分（明确排除，避免后续重复劳动）

以下均已**逐条验证**，非目测：

1. **命令契约（名层）完全闭合**：`lib.rs:163-249` `generate_handler!` 85 条 = `#[tauri::command]` 定义 85 条 = 前端 `invoke("…")` 85 条；`Compare-Object` 双向差集**均为空**。真正的死面只在 `api.ts` 包装的 import 层（B-02 已单列）。
   - 复现：解析 `generate_handler![…]` 段取 `commands::([a-z0-9_]+)`；与 `desktop/src/**/*.{ts,vue}` 的 `invoke\(\s*["']([a-z0-9_]+)` 做 `Compare-Object`。
2. **事件名双向闭合**：Rust 侧 20 个 `agent://`/`plugin://` 常量（`events.rs:16-60`）+ 5 个 `lw://*`（`browser/tools.rs:17`、`notifications.rs:29`、`organize/mod.rs:170/190`、`title.rs:131`）与前端 `listen` 集合一一对应，无悬空、无拼写不符。
   - **有意不订阅/不 emit 的两条**（非垃圾）：`events.rs:31-34` `agent://user-message`（注释：「本通道**永不 emit**，前端也**有意不订阅**」）与 `events.rs:49-53` `agent://tool-uncertain`（注释：「只在会话重放产出…**永不 emit**（USER_MESSAGE 同款纪律）」）——前端注释 `api.ts:207-208` 与之逐字对齐。
3. **capability 权限项零冗余**：`capabilities/default.json:8-18` 的 9 项全部有消费者——`core:app:allow-version` → `Sidebar.vue:12,43` `getVersion()`；`window:allow-show/set-focus` → `main.ts:34-35`；`allow-minimize/toggle-maximize/close/is-maximized` → `WindowControls.vue:22/44/57/71`；`allow-start-dragging` → `data-tauri-drag-region` 9 处（`Topbar.vue` / `Sidebar.vue` / `DestinationShell.vue` / `SettingsNav.vue` / `SettingsPage.vue`）。
4. **零孤儿模块、零失效 `mod` 声明**：脚本对 `src/` 全部 `.rs` 做「是否被某个 `mod X;` 声明（或为目录入口 `mod.rs`）」双向核对——**孤儿 0 个、声明无文件 0 个**（105 个模块名全部有对应文件）。
5. **零 `TODO/FIXME/HACK/XXX/DEPRECATED`**：`desktop/src-tauri/src` 全量 grep 仅 1 条命中，且是测试内的跳过提示 `tests_plugins_root.rs:104` `eprintln!("skip: plugins/ 未暂存（先跑 scripts/build_plugins.sh）")`（测试夹具缺件时的诚实提示，非调试残留）。**零 `dbg!`/`todo!`/`unimplemented!`**。
6. **零注释掉的代码块**：抽查 + grep 未发现被注释的 `fn`/`let`/`impl` 块（退役记录以「已删 + 注释说明」形态存在，如 `settings/context.rs:3-10`，属健康的退役记档）。
7. **无写死的密钥/邮箱/端口/本地绝对路径（生产路径）**：全仓 `F:\`/`C:\` 命中**全部**落在测试夹具（`organize/tests.rs`、`memory_files/tests.rs`、`privacy/tests.rs`、`tests_permission.rs` …）或文档注释的走查轶事（`browser/preview.rs:54`）；无 `sk-…`、无邮箱、无硬编码端口（`localhost:11434` 是 Ollama 官方默认端点常量，非环境写死）。唯一生产面 Windows 专属字符串是 B-21 的报错示例。
8. **`#[allow(dead_code)]` / `#[allow(unused)]` 全部 5 处已逐条读**：`memory.rs:273`、`llm/keyring.rs:17`、`plugins_root.rs:17`、`state/app_state.rs:25`、`settings/embedding.rs:43`——其中 3 处是**正当的测试/平台差异抑制**并有注释说明（`keyring.rs:17` 因 `cfg(test)` 不接凭据库），2 处已在 B-04/B-23 单列。**没有借 `allow` 掩盖大块死代码的情况**。
9. **`Cargo.toml` 30 个依赖逐个取证，无未使用项**：对每个 crate 名做词边界引用计数，全部 ≥1（`tauri` 135、`kernel-agent` 85、`memory-tree` 111、`session-store` 97、`plugin-api` 76、`tokio` 148、`serde` 310、`serde_json` 155、`sha2` 13、`rusqlite` 18、`sys-locale` 4、`percent-encoding` 1、`screenshots` 1、`pdfium-render` 1、`toml` 7、`chrono` 2、`futures` 2 …）。低计数者（1–2 次）已单独目检：`percent-encoding` 用于 `browser/preview.rs` 段编码、`screenshots` 用于 `media/mod.rs` 截屏、`pdfium-render` 用于 `media/pdf.rs`、`chrono` 用于 `<environment>` Today 行、`futures` 用于 SSE `StreamExt`——**均为真实单点使用**。
10. **v2.145 `ShellSection` 退役是干净的**：`AppSettings`（`settings.rs:52-89`）**已无任何 `shell` 字段**，`settings/view.rs`/`update.rs` 亦无 shell 分区；`settings/shell.rs` 与 `settings/tests/shell.rs` 删除后**无残留引用**（全仓 grep `ShellSection` 仅命中 `docs/`）。
11. **幻影键与真垃圾的区分已核实**（严防误报）：以下键**有意保留为兼容幻影**，**不是垃圾**——`context.thresholdPct` / `context.strategy` / `context.stabilityRounds`（`settings/context.rs:3-10` 明载退役理由）、`memory.autoExtract` / `memory.handoffUpdate` / `memory.experiences` / `memory.defaultOrganizer`（`settings/memory.rs:60-61` 明载），且都有**回归测试锁住旧 JSON 可解析**：`settings/tests/memory.rs:355-368`（旧三键 + `defaultOrganizer` 共存解析不破）、`settings/tests/window.rs:24/45`（`thresholdPct`/`strategy`/`stabilityRounds`/`windowTokens` 幻影）。**反向核查**：`memory.collapseThresholdPct` 看着像退役键，实为**活配置**——`settings.rs:323-326` `collapse_config()` → `organize/mod.rs:350/366` 自动收起阈值，**误报即真 bug**，已在报告中明确标为健康。
12. **契约层无「前端调用但 Rust 未注册」的方向**（任务书要求标红的那类真 bug）：**0 条**。85 条前端 `invoke` 全部有同名注册命令。

---

## 复现命令（供 Lead 交叉复核）

```powershell
# ① 命令注册表 ↔ 前端 invoke 双向差集（预期两向皆空）
$rust = ((Get-Content desktop\src-tauri\src\lib.rs -Raw) -split 'generate_handler!\[')[1] -split '\]' | Select-Object -First 1
$a = [regex]::Matches($rust,'commands::([a-z0-9_]+)') | % { $_.Groups[1].Value } | Sort-Object -Unique
$b = Get-ChildItem desktop\src -Recurse -File -Include *.ts,*.vue |
     Select-String -Pattern 'invoke(?:<[^>]*>)?\(\s*[`"'']([a-z0-9_]+)[`"'']' -AllMatches |
     % { $_.Matches } | % { $_.Groups[1].Value } | Sort-Object -Unique
Compare-Object $a $b     # 无输出 = 闭合

# ② api.ts 导出函数的真实消费者（排除 api.ts 自身）——预期命中 assetGc / llmListModels / llmProbeWindow
$api = Get-Content desktop\src\lib\api.ts -Raw
$exp = [regex]::Matches($api,'(?m)^export (?:async )?function ([A-Za-z0-9_]+)') | % { $_.Groups[1].Value }
$src = Get-ChildItem desktop\src -Recurse -File -Include *.ts,*.vue | ? { $_.Name -ne 'api.ts' } | % { Get-Content $_.FullName -Raw }
$all = $src -join "`n"
$exp | ? { ([regex]::Matches($all,"(?<![A-Za-z0-9_])$_(?![A-Za-z0-9_])")).Count -eq 0 }

# ③ 孤儿模块双向核对（预期两向皆空）——逻辑见「已核查」§4；脚本留在 tmp\audit-orphan-mods.ps1（沙箱拒绝删除）

# ④ P0 panic 复现（保留件）
rustc -O -o "$env:TEMP\paniccheck.exe" tmp\paniccheck.rs ; & "$env:TEMP\paniccheck.exe"
#   预期输出：node --version / git status → OK；echo 你好 / git commit -m "修复" / type 中文文件.txt → PANIC
```
