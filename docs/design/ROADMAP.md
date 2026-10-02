# Locowrk 落地路线表（ROADMAP）

> **这是唯一的进度看板。** 每完成一批：把 ⬜ 改 ✅（或 ◐ 改 ✅），补上版本号
> 和提交哈希。设计细节不看这里——每行的"详见"列指向 DESIGN.md 章节或配套
> 文档。
>
> 图例：✅ 已完成 · ◐ 部分完成（剩余项已拆进具体批次）· ⬜ 未开始 ·
> ⏸ 后排（看需求再动）

---

## 一、已完成

### 阶段一～七（内核与桌面端基座）

| 阶段 | 内容 | 提交 | 详见 |
|---|---|---|---|
| 骨架 | Cargo workspace + 各 crate 骨架，`cargo check` 通过 | 398a74c | §8 |
| 一 | cordis-core + session-store + kernel-agent 最小实现 | c48106c | §3/§4/§6 |
| 二 | Fiber 六态状态机 + Effect 撤销链 + runtime 反应式重载 | 7fa9066 | §2/§3 |
| 三 | C ABI 插件加载：plugin-api FFI + plugin-host + tool-echo + kernel-bin | d177cb6 | §5 |
| 四 | Waterfall 三决策点 + Scope 隔离 + 中间件套件 + 审批门 | bfc75e1 | §7 |
| 五 | Tauri 2 桌面端脚手架：agent_chat 跑通 + agent:// 事件流 | 27796e1 | §10/§13 |
| 六 | 桌面端审批弹窗 + i18n 基础（会话视图全 UI 化） | 690bf80 | §13.2/§13.5 |
| 七 | PluginService 编排 + 本地目录市场 + 插件管理 UI | 274f625 | §11/§13.3 |

### 阶段八①–⑧（全部完成，编号阶段收束）

| 批次 | 内容 | 提交 | 版本 |
|---|---|---|---|
| ① | i18n 切换入口 + 最小设置面板（API Key 不出 IPC） | 06e7508 | v2.7 |
| ② | 真实 LLM：genai 三合一适配器 + 工具目录随请求下发 | 492add0 | v2.8 |
| ③ | 第一批工具 tool-fs / tool-fetch + mw-audit 审计中间件 | bad9a65 | v2.9 |
| ④ | 插件元数据 v2：manifest 解析层 / IconLoader / 展示投影 / origin 分组 + Fallback 矩阵 + `locowrk new plugin` 脚手架 | 68250d6 + 4ddb5de | v2.11/v2.12 |
| ⑤ | 托盘 + 全局快捷键 + 数据目录改名 ~/.Locowrk | 5a41585 | v2.13 |
| ⑥ | 第二批插件（tool-fs 写三件套 / tool-shell / tool-edit）+ 安全底线（fsutil 允许根 / fetch SSRF 防线 / fs.restore 判 Irreversible / 补偿执行器）+ ctx-compact | 8a3e05a | v2.14 |
| ⑦ | 第三批插件 tool-git / memory-sqlite / tool-mcp | 1e53589 | v2.15 |
| ⑧ | WASM 加载器：PluginLoader trait + WasmLoader（wasmtime 沙箱） | 672f51b | v2.16 |

---

## 二、当前排期（v2.17，2026-09-29 确认；2026-09-30 增补《Setting and Constrained Optimization.md》逐项核对排期 = S1/S2 两批 + ⑪⑫搭车；同日二批增补：P2 四件入排期 = ⑭⑮⑯，tool-browser 用户裁定最后；同日三批增补：㉑ 工具卡折叠 = UI 正式设计阶段先行批）

### 壳批次 —— UI 骨架（✅ 已完成，提交 a2c82a5）

> 动因：功能越多 UI 越乱的根因是缺落位规则。壳只锁结构与规则，内容留给
> 功能批次；视觉抛光仍归最后。详见 **docs/design/UI shell and patterns.md**
> （本批落点文档：模式库 / 流向图 / 弹层规则 / 扩展规则）。

| 功能 | 状态 | 说明 |
|---|---|---|
| 导航模型定稿 | ✅ | 顶层 4 视图（对话/插件/记忆/设置）；>4 视图时 pill nav 切侧边栏（届时再评估 router）；开发者面板挂设置内 |
| 列表页模式 | ✅ | `patterns/ListPage.vue`：插件已安装页（origin chips）+ 记忆页骨架共用；搜索、筛选 chips、批量栏插槽、空态 |
| 设置分区模式 | ✅ | settings.json 分区 ↔ 面板分区一一对应；llm/sessions/language/shortcut 实装，context/memory/developer 占位（⑨⑪⑫ 同批替换） |
| 会话内系统消息模式 | ✅ | `chat/SystemNotice.vue` + chat store system 类别（存 i18n key）；压缩通知[查看详情][撤销]/"已记住"/审批结果统一形态 |
| 徽章体系 + 进度条模式 | ✅ | `IdleBadge.vue`（六级→灰/黄/红，后端只给结构化数据）+ `ContextProgressBar.vue`（绿/黄/红，无数据不渲染），挂载点就位数据随⑨⑩ |
| 事件→UI 流向图 | ✅ | UI shell and patterns.md §4：`agent://`→store→组件 + command→面板 映射表，功能批次只扩表 |
| 弹层规则 | ✅ | UI shell and patterns.md §5：modal（审批，全局至多一个）/ 行内展开 / 独立页面 / 默认不弹 判据 |
| design tokens | ✅ | style.css `--lw-*`（表面/文本/语义色/圆角/间距，唯一样式事实源；不做视觉抛光） |
| PluginMarket.vue 拆分 | ✅ | 291 → 54 行：DiscoverTab/InstalledTab 子组件 + lib/plugin-helpers；check_length.sh 纳入 .vue |

### ⑨ 上下文管理收口（P0）（✅ 已完成，提交 92f298b）

> 详见 DESIGN.md §14 v2.17 ⑨段 + 《Context and Memory.md》§3。

| 功能 | 状态 | 说明 |
|---|---|---|
| ctx-compact 中间件本体 | ✅ | 配置驱动重写：CompactConfig{threshold_tokens, strategy} 经共享 RwLock 注入，settings_set 成功即热换入 |
| 模型窗口来源 | ✅ | settings.json `context.windowTokens`（设置字段 + 预设 8k–1M + 自定义；genai 不暴露元数据）；窗口未知回落 32k 保守保护 |
| 压缩阈值设置面板 | ✅ | ContextSection：滑条 10–90%（10% 一级）+ 百分比/绝对 token 双显示 + <20% 警告 |
| 压缩策略三档 | ✅ | 滑动窗口（纯本地）/ 分层保留（默认）/ 仅摘要；目标 = 阈值 50%；摘要失败或缺 LlmService 就地降级滑动 |
| 动态建议值 | ✅ | suggested_threshold(window) 后端分档（30/35/25/15），投影带 suggested_pct；前端表单即时同表换算 |
| ContextCompacted 事件 + 撤销 | ✅ | 进 session-store（serde default 兼容旧日志；摘要存全文，折叠留尾部 kept_messages）；撤销 = rewind_to + fork 原地换入（会话 id 不变，其后轮次一并回退） |
| 会话内实时进度条 | ✅ | 每 step 推 agent://context-usage（与压缩同估算口径）→ ContextProgressBar；压缩推 agent://context-compacted → SystemNotice [查看详情][撤销] 真接线 |

### ⑩ 暴露声明 + 使用统计地基 + 闲置 UI（✅ 已完成，提交 f6b1a44；搮车：工具名去点号 9a5d39c + 产物重建与目录准入防线）

> 详见 DESIGN.md §14 v2.17 ⑩段 + 《Tool exposure strategy and usage
> statistics.md》§4/§3/§8。

| 功能 | 状态 | 说明 |
|---|---|---|
| ToolSpec `exposure` 字段 | ✅ | plugin-api `Exposure`（缺省 on_demand）；manifest 可选字段追加零破坏，`PLUGIN_ABI_VERSION` 不动；always>10 在 `RegistryToolSource::list_tools` 一次性告警（`merge_catalog` 纯函数 + 单测，被同名顶掉的不计数） |
| 暴露管线（直通态） | ✅ | list_tools 剔除 hidden（resolve 不过滤，⑫ 检索激活仍可达）；always 直通 + on_demand 全暴露，行为零变化（截断归⑫） |
| ToolCall 事件补 plugin_id | ✅ | `Tool` trait 加默认方法 `plugin_id()`（宿主侧非 C ABI，既有实现者零改动；SabiToolAdapter 经 `PluginInstance::id()`=manifest.id 带出）；事件 `#[serde(default)] Option<String>` 兼容旧日志（roundtrip 单测） |
| usage.sqlite3 统计库 | ✅ | desktop `usage.rs`：两表同 §3.1/§3.2（tool_usage 含 ewma_score 预留列）；runner 投影到 ToolResult 时 `record()` → mpsc → 5s 批量刷盘，全句柄 drop 收尾刷一次；库打不开降级丢弃；隐私只记工具名/插件 id/时间戳 |
| IdleLevel / IdleInfo | ✅ | desktop `idle.rs`：六级 + 9 档 unit 全按 §8.5–§8.7 表（后端裁决）；classify/filter_key 全表单测；builtin 开发态插件不评估闲置 |
| 闲置 UI | ✅ | `plugin_list_with_usage`（attach join 投影）+ IdleBadge 契约升级（per-unit 人类文案、never_used 黄档对齐 §9.4）+ 第二排闲置 chips（ListPage `subFilters`）+ 批量栏（全选/停用/启用/卸载，卸载内联二次确认列名）+ 顶部提示条（§9.3 点去看看切筛选）；批量命令 `plugin_batch_set_enabled`/`plugin_batch_uninstall`（BatchResult 逐条成败） |
| 停用落盘核对 | ✅ | 核对 service.rs `startup_reconcile` 确有 `.filter(|e| e.enabled)`，lockfile `set_enabled` 落盘已在（v2.12）——无需改动 |

### ⑪ 记忆注入 + 记忆管理 UI（✅ 已完成，提交 d342551）

> 详见 DESIGN.md §14 v2.17 ⑪段 + 《Context and Memory.md》§4/§5。

| 功能 | 状态 | 说明 |
|---|---|---|
| memory-sqlite 插件本体 | ✅ | 1.2.0（d342551）：memory_put/get 改名 **memory_write/memory_read**（§4.4 对齐）+ write/read/search 标 **exposure=always**（⑫ Top-N 截断不挤出）；list/delete/undo 维持 on_demand；写路径核对为同步落盘（连接按调用开、直写、无队列——"说了记住却没记住"不存在） |
| Memory-Inject 中间件 | ✅ | kernel-agent `middleware/memory_inject.rs`：`MemorySource` trait（内核定契约、组装点实现，与 ApprovalResolver 同模式；不经工具管道——每 step memory_list 会污染使用统计与审计）；注入写 `RequestPayload::system`（新字段，genai 映射 with_system）——**记忆在消息列表之外，"系统提示永不压缩"由结构保证**，会话日志不落注入内容；注册次序 Memory-Inject → ctx-compact（§3.6），ToolExposure（⑫）落地后排最前 |
| memory.put 同步落盘 | ✅ | 核对即满足（v2.15 起插件就是同步直写）；ROADMAP 旧行描述针对的是"计划中的异步队列"——该队列从未存在 |
| 注入条数可配（约束清单 §3 搭车） | ✅ | settings.json `memory` 区（enabled / topK，缺省开 / 5），共享 RwLock 注入、settings_set 即热换入；设置面板 MemorySection 真分区替换占位 |
| 记忆管理页 | ✅ | MemoryPage 填充：搜索（防抖）/ 列表（新→旧）/ 行内编辑（不开全局弹层，§5）/ 删除（行内二次确认）/ 清空（底部二次确认，§11.3 一键阻力最小）；每条来源徽章 `SourceBadge`（§3.4 来源族）+ 相对时间（lib/format.ts）；四命令 memory_list/update/delete/clear_all 薄适配 MemoryStore（宿主直读插件库，ALTER 补 source 列迁移旧库） |

### ⑫ 打分截断 + 元工具（✅ 已完成，提交 da96c6f）

> 详见 DESIGN.md §14 v2.17 ⑫段 + 《Tool exposure strategy and usage
> statistics.md》§5/§6。

| 功能 | 状态 | 说明 |
|---|---|---|
| EWMA 打分 | ✅ | flush 时读旧分衰减积累：`old × exp(-days/7) + 窗口调用数`（⑩ 预留列开始写入）；GlobalEwma trait = 内核定契约、组装点实现（desktop 读 usage.sqlite3，降级空表 = 退回会话内优先） |
| Top-N 截断 + 调试面板 | ✅ | ToolExposure 监听器（agent/request 注册最前，§7 链位）；settings.json `tools` 区 enabled/maxPerRequest（缺省 20，1–200）共享 RwLock 热换入；开发者分区（壳批次最后一个占位替换）= 截断设置 + 最近一次暴露快照（打开时拉取不轮询；节省走 §3.5 compactNumber 档位） |
| 会话内热度 | ✅ | 从 `RequestPayload.messages` 实时数 ToolResult（§3.5"不落盘"——投影日志即会话事实，零新 trait、无跨会话竞态）；会话 1.0 > 全局 0.7，always 直通不占额度；检索 0.5 归 ⑬ |
| search_tools 元工具 | ✅ | 宿主内置 always+Pure（目录+统计是宿主关切，插件看不到注册表）；关键词检索全目录（name 2 分/desc 1 分 + query 原串照顾中文）截 8 条；空结果回退 top-10 高频 ∩ 目录，统计也空则如实 no_match；**有意扩 API**：RegistryToolSource::list_all_tools 含 hidden（§4.1"显式检索才暴露"的唯一通路，resolve 本就不过滤） |

### S1 tool-shell 安全分级与进程治理（✅ 已完成，提交 f0fce64）

> 核对修正三条（排期按修正后事实走）：① 白名单"prog+子命令"两段匹配**机制已在**
> （policy.rs `decide`，`LOCOWRK_SHELL_ALLOW` 支持 `git log` 形态）——缺的只是默认
> 白名单内容与设置 UI 化；② 输出超限**已是截断 + `truncated` 标记**（exec.rs），
> 不是清单说的"直接拒绝"——只剩提示文案；③ allow∩denylist 冲突**调用时已明确报
> permanent denylist，非静默**——补的是启动期 warn。前提"fs_* 守卫修好"已满足
> （7ba8776）。denylist 现 64 项（清单写 ~50）。

| 项 | 状态 | 说明 |
|---|---|---|
| 只读命令免审批分级 | ✅ | **拆只读工具**形态：`shell_read`（Pure）只放 ls/cat/grep + git status\|log\|diff\|show（固定表，不吃 LOCOWRK_SHELL_ALLOW）；路径校验 paths.rs **复用 fsutil allowed_roots（与 fs_* 同源根）**：生效 cwd 出根整条拒、非 flag 候选与 `--flag=值` resolve 出根拒、`-f/…` 贴值形态带分隔符即拒、不存在候选视为模式串放行；拒绝文案引导改用 `shell`（降级审批 = 模型换工具，宿主审批门不变）；plugin-host e2e 新用例覆盖三态 |
| denylist 补齐 | ✅ | 双清单拆分：`DENY_HARD` 46 项（解释器/网络主力 + **新补包装 xargs/env/find/nice/timeout/nohup/time、网络 nc/ncat/netcat/telnet/rsync、exec 面 wscript/cscript/mshta/rundll32/osascript**）+ `DENY_RISKY` 35 项（高危可解锁）；§2.3 磁盘/系统类 mount/chroot/systemctl 随 §2.7 裁决归高危 |
| allow∩denylist 启动冲突日志 | ✅ | policy.rs `conflicting_allow`（两级清单求交）+ make_state 里经宿主 vtable 打 warn 点名被压制条目；解锁开关开启也打一条可见 warn（红警告原则）；denylist 永远赢不变 |
| 进程组 kill | ✅ | Windows：JobGuard（CreateJobObjectW + JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE，spawn 即 assign，Drop/超时 TerminateJobObject）——windows-sys 0.61 需补 Win32_Security feature（CreateJobObjectW 签名引 SECURITY_ATTRIBUTES）；Unix：`Command::process_group(0)` + killpg(SIGKILL)；**正常退出也收尾**（先清场再收管道）——残留孙进程握管道会让 rx 永等，旧代码本就有的悬挂一并修掉 |
| 截断提示文案 | ✅ | stdout/stderr 截断尾部附"[output truncated at N bytes — narrow the command scope … or raise max_bytes and retry]"（英文，模型面文案与工具 description 同语言） |
| 高危命令可解锁开关 | ✅ | settings.json 新 `shell` 区 {unlockRisky，缺省 false} → state.rs **插件加载前**写 `LOCOWRK_SHELL_UNLOCK_RISK` env（tool-shell init 读一次，**重启生效**——插件无运行期配置通道，kv_get 尚未开放）；解锁后高危放行，shell 工具 Irreversible，**放行即审批**；硬锁类不受开关影响；ShellSection.vue 设置分区（琥珀红警告 + 重启提示） |

### S2 设置归属、数字呈现与内置插件停用（清单 §1/§5/§6/§3 余项）

> **✅ 已完成（v2.27，提交 7448737）**——用户命名裁定：所有「禁用」统一改叫「停用」
> （内置与第三方同一动词，zh `disable`/`stateDisabled` 键值 + 各处注释同步）。
> ① lockfile 落 `InstallSource::Builtin` 条目（登记保留 enabled，重启不复活停用项）；
> ② 启用从内置根解析**最新**版本（独立升级，漂移同步回条目）；③ 市场安装同 id
> 条目转 Market（安装态接管，开发态扫描让位）；④ 窗口字段迁 llm 区（open 读旧
> 值迁移一次幂等）；⑤ Ollama /api/show 探测 + 静态档案前缀命中 + 手动兜底；
> ⑥ compactNumber 全接线（阈值预览 ≈51.2K 不再露 52,428.8）；⑦ tool-shell 产物
> 1.2.0。plugin-service 新 `builtin.rs`/`service/enable.rs`，desktop 新
> `llm/probe.rs`/`lib/model-profiles.ts`。

| 项 | 状态 | 说明 |
|---|---|---|
| 内置插件停用 | ✅ | **（用户命名：禁用→停用）**InstalledTab builtin 行加 停用/启用（不给卸载——随程序分发，uninstall/update 服务端拒绝；不进批量多选）；`plugins_root::load_all` 带 skip 集（lockfile 接管 + 已停用），装载成功即 `register_builtin` 落 `InstallSource::Builtin` 条目（版本同步、保留 enabled——重启不复活）；`set_enabled` Builtin 分支从内置根解析最新版本；停用即出工具候选池（`RegistryToolSource::resolve` 只遍历 `active_instances`，deactivate 即过滤）；操作门控按 **source**（lockfile 来源）而非 origin（manifest 自声明）；对账不删 Builtin 条目（内置家不在安装根）；插件端解析下沉 `plugin_service::builtin`（版本/工件发现共享，桌面不再自留副本） |
| 上下文窗口迁模型配置 | ✅ | settings key `context.windowTokens` → `llm.windowTokens`（open() 读旧值迁移一次，幂等，落盘随下次保存）；**ContextSection 只去掉窗口下拉这一条**（用户裁定：阈值滑条/策略/预览全部原地不动），LlmSection 增窗口下拉（provider/model/baseUrl/key 之后）；Ollama `/api/show` 探测命令 `llm_probe_window`（llm/probe.rs，reqwest json 只加无 TLS 特性——探测走 http，图内 TLS 由 genai 的 reqwest 统一；顶层 context_length 或 model_info `<arch>.context_length` 两代形态 + 5s 超时）；`lib/model-profiles.ts` 静态档案（deepseek/claude/gpt-4o/gemini/llama3.1/qwen2.5 前缀命中）**只在未设置时自动填充**，手动选择永远优先；读不到 UI 错误条提示手动兜底；agent_chat UsagePolicy / compact_config 改用 `effective_window`/`effective_threshold_tokens`（llm 优先、旧字段兜底、未知回落 32k） |
| 数字呈现落地 | ✅ | `lib/format.ts` `compactNumber`（上批已备好的唯一实现，1024 进位）接线：LlmSection 预设标签（256K 不写 "262,144 token"）、ContextSection 阈值预览（`≈51.2K`——1024 进位的真实值，UI shell doc §3.5 示例已同步改注）、ContextProgressBar 比例段 `≈52K / 256K`（chat store 透传 windowTokens，窗口未知只显百分比）+ 压缩 SystemNotice"已节省"档位化（rider）；组件不自带第二把尺 |
| shell 超时/输出限长默认可配 | ✅ | `LOCOWRK_SHELL_TIMEOUT`（1–120s）/ `LOCOWRK_SHELL_MAX_BYTES`（1024–256KB）环境默认，init 读一次（RunDefaults::from_env）：越上界钳制、低于下界/非数字回落内置缺省并经宿主 vtable warn（§2.4 不静默）；模型 arg 上限 120s/256KB 不变；tool-shell 产物 1.2.0 |
| 宿主 Timeout 中间件可配 | ✅ | kernel-agent `Timeout::hot(Arc<RwLock<Duration>>)`（`new` 保留给测试/CLI，DurationSource 两态）；settings.json tools 区 `callTimeoutSecs`（5–3600 缺省 30，state.rs 起共享锁 `timeout_cfg`）；settings_set 热换入即生效；DeveloperSection 增数字输入 |
| 内置插件版本策略定稿 | ✅ | **方案 B 独立升**（清单推荐，同意）；`bundled` 字段**不加**——source（lockfile 管理门控）× origin（manifest 展示分组）双字段已够；接管/回归/启用取最新等规则成文 packaging.md **§9.4** |

### ⑬ 智能层（✅ 已完成，v2.28，提交 42bcbf9）

> 四项共享 embedding 基建一起落地。**可降级是总原则**（§1）：embedding
> 未配置或端点不可用 = 智能层整体缺席，检索退关键词、经历不写不注入、
> 提取跳过经历——聊天主链路零影响。

| 功能 | 状态 | 说明 |
|---|---|---|
| 工具向量检索 | ✅ | kernel-agent 新 `tool_retrieval`：`EmbedPort` 契约（内核定契约、组装点实现，同 GlobalEwma 模式）+ 描述向量缓存（键含描述原文，插件换描述即失效）+ 余弦打分；ToolExposure 打分加 `+0.5 × similarity`（§5.3 连续补充信号），**只在目录 > 上限（真的要裁剪）时才打 embedding**；embedding 未配置/失败 = 空表退回两层打分；快照增 `retrieved` 归因（开发者面板同行渲染）；search_tools 元工具同升：语义优先（相似度 floor 0.25 取前 8，一次批量嵌查询+全目录）→ 关键词 → 高频回退链不变 |
| memory-vector 经历记忆 | ✅ | **宿主侧形态**（同 llm 插件先例：写入要过 embedding 模型，端点+密钥归宿主，插件无运行期配置通道；ROADMAP §四已注记）：desktop `experiences.rs`——experiences.sqlite3（content/embedding BLOB f32-LE/session_id/created_at，§4.3 表结构），余弦检索（全库限 2000 行、**维度不符行跳过 = 换 embedding 模型后旧行自然退役**）；kernel-agent `ExperienceSource` async 契约 → Memory-Inject 并列注入 `<session_experiences>` 块（查询 = 最近一条用户消息；同走 topK 上限）；记忆页增经历区块（可见可删可清空，§11.3 信任基础；无编辑语义——自动沉淀原文） |
| 隐式记忆提取 | ✅ | settings `memory.autoExtract`（缺省关——"易记垃圾故放最后"）；desktop `extract.rs`：轮次收敛（TurnDone）后 fire-and-forget——本轮消息转录（≤6000 字符）→ LlmService 提取 JSON → ① 事实 ≤3 条（key 清洗 + 强制 `auto.` 命名空间，**永不覆盖用户显式记忆**；source=extracted）写 memory 库（`MemoryStore::write` INSERT..ON CONFLICT）+ ② 经历摘要（≥10 字符，≤500）嵌入落 experiences；parse 剥围栏取 JSON、空 key/value 过滤；任何失败只 warn 不影响聊天；无 UI 通知——结果在记忆页可见（来源徽章"自动提取"） |
| 自动停用设置项 | ✅ | settings `plugins.autoDisableIdleDays`（0=关 缺省 / 7–365 校验）；desktop `autodisable.rs`：启动 + 设置保存即巡检——距最近使用（从未使用 = 安装时点，与 ⑩ IdleBadge 同口径）≥ N 天的**安装态第三方插件**逐条停用（内置不参与闲置评估，同 ⑩；**只停不启**——关闭不复活已停用项，§8.8 状态切换由用户显式触发）；EmbeddingSection / PluginsSection 新设置分区（§7 扩表）+ §4.2 命令表扩 `llm_probe_embedding`（检测显示向量维度）/ `experience_*` 三命令 |

### ⑭ 触发与推送：tool-schedule + tool-notify（✅ 已完成，v2.29，提交 7dc0139）

> plugins.md P2 四件入正式排期的第一批（tool-browser 仍后排——用户裁定
> 最后）。两件合批：调度与推送天然成对（触发时刻的"告知用户"通道就是
> notify）。生命周期归属先定调：定时器/触发在宿主主循环（tokio time 已
> 在依赖），插件无后台常驻能力——调度工具面走宿主内置（search_tools 先
> 例），通知的桌面通道经宿主 vtable 追加槽位。

| 功能 | 状态 | 说明 |
|---|---|---|
| tool-schedule 定时任务 | ✅ | **宿主内置工具面**：desktop `scheduler/`（store/engine/tools 分文件）——`schedule_create / schedule_list / schedule_delete` 经 `KernelToolSource::with_builtin` 进目录（create=Always+Irreversible 审批即告知，list/delete=Pure 按会话隔离）；独立 schedule.sqlite3（`MAX_SCHEDULES=100` 防失控）；一次性（at/delay_secs）+ 周期（interval_secs ≥10s）两型；绑定会话 = `CallCtx.session_id`（**kernel-agent 新 `step_as` 首次填充 §5.2 规划字段**，runner 聊天路径切换，普通 `step` 语义不变）；触发引擎 1s tick：目标解析 = 调度行会话条目仍存活 → 原地注入，行已不在（重启）→ 落最近聊过的活会话（agent_chat 记 last_chat，**不落僵尸会话**），无活会话 → 30s 顺延等待；会话忙（try_lock 失败）30s 顺延，一次性累计 2h 放弃并系统通知、周期让位 20 次跳槽不追帧；**启动补跑无单独路径**——错过期间的 run_at 落在过去，首次 tick 的 due() 天然覆盖（周期补跑一次后滚 now+interval）；触发先行推 `agent://schedule-fired`（SystemNotice），窗口不可见/失焦再补系统通知兜底 |
| tool-notify 桌面通知/Webhook | ✅ | plugins-src/tool-notify 1.0.0：一个 `notify` 工具按 `channel` 参数二选一——**webhook 通道** = 插件本体（POST JSON {title,body,source}，tool-fetch SSRF 防线照搬：目标解析全地址逐判、私网/环回/链路本地拒绝、redirects(0)、`LOCOWRK_NOTIFY_ALLOW_PRIVATE=1` 显式后门；**非 2xx = 未送达 = 报错**，不学 fetch 把 4xx 当数据）；**桌面通知通道** = plugin-api `HostVtable` **末尾追加 `notify` 槽**（append-only，`PLUGIN_ABI_VERSION` 不动；repr(C) 前缀兼容旧插件），plugin-host `set_notify_hook` 注入（desktop_notify.rs 注册，headless kernel-bin 返回 ERR_UNSUPPORTED 如实上报）；整体 Irreversible（宁严勿松：webhook 对外发数据，分级是工具级粒度） |

### ⑮ agent-sub 子 Agent 调度（✅ 已完成，v2.30，提交 d29cd69）

> 形态先定调：**宿主/内核侧**而非 C ABI 插件——子 Agent 要递归跑内核循环，
> 依赖方向禁止插件依赖 kernel-agent（同 llm / memory-vector 宿主侧先例，
> 理由不同：这次是循环本体，不是密钥）。v1 收敛为单层串行，不做多 Agent
> 协作协议。

| 功能 | 状态 | 说明 |
|---|---|---|
| 子 Agent 工具面 | ✅ v2.30 d29cd69 | kernel-agent `subagent/` 模块：`SubagentTool`（`subagent_run(prompt, tools?, max_turns?, timeout_secs?)`，Irreversible 宁严勿松 + Always 暴露）+ `SubagentGate` 快照表（内核每个带身份 step 刷新父转录前缀）+ `Kernel::child_kernel` 派生作用域子内核；组装点注册（desktop，同 search_tools 形态；kernel-bin 未接，需时搭车）；单层硬剔除（FilterToolSource 恒剔 subagent_run，白名单显式包含也无效） |
| 隔离与预算 | ✅ v2.30 d29cd69 | 子会话 = 父转录**可见前缀** fork（复用 ⑨ `Session::fork`/rewind 语义，`{parent}-sub-{n}` 内存投影）；`tools` 缺省 = 继承父全目录、非空 = 严格子集 ∩ 父目录（越权 resolve → NotAvailable → failed 如实闭合）；轮数缺省 8/上限 32、超时缺省 300s/上限 1800s（0 = 不限），越限 status = budget_exhausted / timeout，摘要取子转录尾部不虚构；收束 JSON `{status, summary, turns, tools_used, child_session_id[, error]}`，摘要 4000 字截断 |
| 审批与事件 | ✅ v2.30 d29cd69 | `ToolCallReq` 增 `session_id`（step_inner 与 CallCtx.session_id 同点填充，12 处构造点补齐）；子内 Irreversible 冒泡父内核同一 ApprovalHub，载荷带 `{parent}-sub-{n}`，ApprovalModal 紫"子 Agent 调用"徽章；派生/收束落父会话日志（subagent_run 的 ToolCall/ToolResult 事件即日志事实）+ 前端 SystemNotice（§4.1 扩表 2 行先行：tool-call → subagentStarted / tool-result → subagentDone + 卡片照常）；子内活动不投影（黑盒） |

### ⑯ tool-sandbox shell 沙箱（✅ 已完成，v2.31，提交 dc822d9）

> plugins.md 仅一行定义（"shell 在容器/WASM 里跑"），太薄，不足以直接动码；
> 且 Windows 开发机无容器保证。方案取舍是本批第一个交付物，落 plugins.md/
> DESIGN 补记后再写实现。

| 功能 | 状态 | 说明 |
|---|---|---|
| 方案定稿 | ✅ v2.31 dc822d9 | 三候选取**① Windows 受限令牌 + JobObject 限额**（② WASI 只能跑 wasm 化命令实用性窄，记录为远期可选内建命令集；③ 容器可用性依赖用户环境，记录为远期可选后端）；能力矩阵落 plugins.md §六：内存/进程数/CPU 占比硬限、剪贴板 UI 限制、无特权受限令牌（DISABLE_MAX_PRIVILEGE + deny Administrators 不可 UAC 提升）、整树治理 ✅；**文件系统与网络不隔离 ❌ 如实分级**（不做虚假安全声明——策略三道闸因此保留，高危无解锁开关引导走 shell） |
| tool-sandbox 插件本体 | ✅ v2.31 dc822d9 | 单工具 `sandbox_run`（Irreversible 一律审批）：JobObject 限额（mem_mb 缺省 512 / max_procs 64 / cpu_rate 可选硬顶，arg/env 双层缺省钳上限）+ 受限令牌 spawn（CREATE_SUSPENDED → 挂 Job → Resume；令牌不可用降级 job-only，输出 `isolation` 字段如实上报不静默）+ 同源三道闸（LOCOWRK_SANDBOX_ALLOW 扩展、denylist 永远赢）；Unix 对称 pre_exec setrlimit（AS/NPROC/CPU）+ 进程组树治理；与 tool-shell **并存不替换**（其白名单/denylist/进程治理纪律原样保留）；测试 15 例含真跑受限 spawn 全链（含全量限额参数） |

### ⑰ UI 视觉抛光（✅ 已完成，v2.32，提交 945e681）

> 壳批次 §六 明言"明暗主题切换、动效、精调间距归 ⑬ 后统一做"；本批兑现。
> 批首交付 = 先改 UI shell 文档（§六重写为主题/颜色类/动效规范 + §3.2/§七
> 扩表），再动码。

| 功能 | 状态 | 说明 |
|---|---|---|
| 明暗主题切换 | ✅ v2.32 945e681 | settings.json 新 `appearance` 区（dark / light / system，缺省 dark 旧文件零迁移）；Rust `settings/appearance.rs` + update 校验 + 2 测试；前端 `lib/theme.ts` 唯一执行者（写 `<html data-theme>`，system 档 matchMedia 监听，组件禁自读媒体查询）；AppearanceSection 三档分段控件挂 Language 之后；style.css 双主题变量块——暗色 = 原调色板值**像素级不变**，亮色逐 token 选配浅色族。已知近似：原生标题栏跟随系统不随应用切换 |
| 组件层语义颜色类 | ✅ v2.32 945e681 | tailwind.config 把颜色全部绑到 `--lw-rgb-*` 三元组（`rgb(var(--x) / <alpha-value>)` 形态是 47 处 `bg-x/60` 半透明修饰符能工作的前提）；组件 neutral 灰阶类清零 → 语义角色类 `surface-0..3 / ink-1..5 / line(-strong) / accent(-hover)`；语义 hue 38 个实用 (hue,shade) 全部入映射表（暗=原值，亮=浅色族）；token 双形态纪律（hex + 三元组同值同改）入 §六 |
| 统一动效 | ✅ v2.32 945e681 | §六三类：交互元素 @layer base 统一 transition 120ms；弹窗 fade+scale（`lw-modal`）、SystemNotice 淡入上滑（`lw-notice`）入场、进度条宽度过渡（200ms）；`prefers-reduced-motion` 一律降 0；禁止常驻动画/视差/弹跳入规。间距维持五档不批量改造（抽查无系统性问题） |

---

### ⑱ fs roots 工作区目录选择器（✅ 已完成，v2.33，提交 dc2ad87）

> backlog"桌面端接线 LOCOWRK_FS_ROOTS"兑现。fsutil `allowed_roots()` 每次
> 工具调用读 env（无缓存），因此选择器做成**保存即生效免重启**；目录选择
> 走 OS 原生对话框，Rust 侧封装（UI shell 文档 §七规则 8）。

| 功能 | 状态 | 说明 |
|---|---|---|
| settings `tools.fsRoots` | ✅ v2.33 dc2ad87 | 缺省空 = 仅数据根（与 fsutil 缺省语义一致，旧 settings.json 零迁移）；normalize 校验 trim/去空/去重保序 + 目录存在性 fail-closed（同 fsutil 使用时契约，保存时即报）；env 注入双时机 = 启动 + settings_set tools 区改动即重注入，空表移除 env |
| DeveloperSection 工作区目录块 | ✅ v2.33 dc2ad87 | `pick_directory` 命令（tauri-plugin-dialog 仅 Rust 侧，无 JS guest/capability）+ 列表增删即存即生效；§3.2 分区表纠偏（memory/developer 占位标记更新为已实现、补 shell 行）+ §七规则 8（原生对话框封装模式） |

---

### ⑲ desktop session 持久化（✅ 已完成，v2.34，提交 a5347c3）

> backlog"desktop session 持久化（落盘顺带解决撤销 fork 的物理留痕）"兑现。
> 磁盘 = 内存日志的**镜像**（`<数据根>/sessions/<id>.jsonl` 一行一事件）：
> 追加与投影共用同一游标；undo fork 截断整体重写 + `.undo.bak` 滚动备份。

| 功能 | 状态 | 说明 |
|---|---|---|
| session-store persist 模块 | ✅ v2.34 a5347c3 | JSONL 落盘/恢复/undo 重写三件套 + 列表（mtime 降序、首条用户消息截断做标题）；session id 白名单防穿越；坏行跳过计数（崩溃半行不致命）；测试 4 例 |
| desktop 接线 | ✅ v2.34 a5347c3 | 启动恢复先于调度引擎（调度行绑定的旧会话跨重启存活）；run_turn 增 log 参数增量镜像（手聊/调度两路）；context_undo_compact 后同步重写磁盘；session_list / session_history 命令（历史 = runner 同一投影函数 project_events 全量重放，不走事件通道） |
| 会话切换器（最小面） | ✅ v2.34 a5347c3 | UI shell 文档 §3.6 先行：chat 头部下拉（首条用户消息截断）+ 新会话；切换 = 全量投影重放恢复气泡/工具卡/系统通知；首聊前不建后端状态；列表运营类 UI（搜索/删除/改名）留给 UI 正式设计阶段 |

---

### ⑳ LLM 流式输出（✅ 已完成，v2.35，提交 54a97ba）

> backlog"genai 流式 → 事件推送"兑现。通道与前端追加逻辑零改动（壳批次
> 预留的 delta 语义），改点在内核 delta 出口契约与投影去重。

| 功能 | 状态 | 说明 |
|---|---|---|
| 内核流式契约 | ✅ v2.35 54a97ba | `LlmService::complete_streaming`（缺省回落 complete 整段一次回调）；`LlmStreamSink` delta 出口（返回"是否真消费"，黑盒替身 false）；`StepOutcome::Continue { streamed }` 仅 sink 真消费才置位；无 sink = 纯非流式；child_kernel scoped 换入 NoLlmStreamSink——子内 delta 不泄漏父 UI（⑮ 黑盒保持） |
| genai 流式 + 桌面接线 | ✅ v2.35 54a97ba | `complete_streaming_with` 走 `exec_chat_stream`（capture_content/capture_tool_calls 聚合正文与工具调用，End 取 captured_content；推理流不进正文）；`StreamChunkSink` 直推 `agent://chunk`；runner 按 streamed 抑制该步整段 chunk——同一正文不出现两遍 |
| 搭车 | ✅ v2.35 54a97ba | api.ts CHANNELS 补 `agent://schedule-fired`（⑭ 遗留死分支：store 有分支、通道缺席）；§4.1 chunk 行语义升级成文 |

---

### ㉑ 工具调用卡默认折叠（✅ 已完成，v2.36，提交 977853b）

> UI 正式设计阶段首个落点（用户 2026-09-30 指定先做）：对话密度治理——
> 不然一轮多工具调用的会话瞬间被参数/结果 JSON 刷满，正文被挤走。
> UI shell 文档 §3.7 先行成文（模式 + §4.1 组件列指向更新）。

| 功能 | 状态 | 说明 |
|---|---|---|
| ToolCallCard 模式组件 | ✅ v2.36 977853b | `chat/ToolCallCard.vue`（模式库第二实例）：默认折叠 = 头部单行（▸/▾ + 工具名 + 版本/副作用徽章 + 参数摘要（args 首个原始值 clip 48）+ 状态），整行 button 点击展开参数/结果明细（v-show 保 DOM）；折叠态是组件本地状态，不入 store 不持久化，重放/切会话回折叠 |
| 失败自动展开 | ✅ v2.36 977853b | `ok === false`（实时到达 pending 翻转 / 历史重放挂载即检）展开一次——错误可见性优先于密度，用户可手动再折叠；运行中不加脉动动画（§六禁常驻动画） |
| 迁出 + clip 收敛 | ✅ v2.36 977853b | ChatMessages 内联卡（~70 行）迁出换组件（175→101 行）；`clip` 从 chat store 私有助手收敛进 `lib/format.ts`，通知摘要与折叠行参数摘要共用 |

---

### ㉑·fix 子代理收束汇报误报父史（✅ 已完成，v2.37，提交 2332d66）

> smoke 实测跟进（用户转来探通报告："tools_used 出现未授权的 mcp_status，
> 与子的自述矛盾"）。核查结论：**白名单过滤无恙**——FilterToolSource
> list/resolve 双侧过滤，且 resolve 前置于 ToolCall 事件写入，越权调用
> NotAvailable 即整步 failed（既有测试 `explicit_whitelist_blocks_unlisted_resolve`
> 覆盖）；误报根因是 `tools_used`/`tail_summary` 扫了**整个**子转录，
> 把 fork 前缀里**父的历史调用**算成了子的使用。

| 修复 | 状态 | 说明 |
|---|---|---|
| 自身段口径 | ✅ v2.37 2332d66 | run_child 在 fork+追加任务 prompt 后记 `own_from = child.len()`，`tools_used` / `tail_summary`（含"最后动作"兜底）只扫自身段——父的调用史/旧正文不再混入子的收束汇报 |
| 回归测试 | ✅ v2.37 2332d66 | `tools_used_ignores_forked_parent_history`：父转录预置 mcp_status 调用，派生后断言 tools_used == ["echo"]、摘要取子自身段 |

### ㉒ UI 设计落地①：design tokens v2 与基线（✅ 已完成，v2.38）

> UI 正式设计阶段（UI shell 文档 v3.3 定稿 + ui-prototype 原型，提交
> 775b14f）第一个实现批次。批首先落文档（v3.3 已含全部规范），本批兑现
> §六 tokens 与基线层。用户裁定（2026-10-02）：后端服务前端，缺的功能
> 后期逐步补，前后端拆分可按需调整。

| 功能 | 状态 | 说明 |
|---|---|---|
| tokens v2 | ✅ v2.38 | style.css 按 §6 重写：13 档中性灰阶（唯一颜色源）+ wash 三层（平面内无边框分层）+ 表面层级（bg/side/dock/inset/secondary/tertiary/elevated/composer/raised）+ ink 五级 alpha + 单色 accent（暗=白/亮=近黑）+ 状态五色；形状阶梯 10 档；量纲（toolbar-h 46 / ctl 28 / field-h 38 / content-max 760 三轴同源 / wincontrols 120 / panel/main 地板）；修正原型两处历史残留（820→760、--motion-*→--dur-/--ease- 四档四缓动） |
| tailwind 角色色 | ✅ v2.38 | 配置只留 §6.1 角色 token：实色走 --lw-rgb-* 三元组（保 /16 修饰符），自带 alpha 的（wash/ink/line/表面亮色）整值 var 引用（修饰符会静默丢弃，禁止使用）；38 阶 hue 映射表收缩到语义色 + boxShadow 三 token（dialog/composer/raised）；过渡别名 surface-0..3 暂留（组件重写批次清除） |
| hue 类清零 | ✅ v2.38 | 全部 22 个组件的红/琥珀/黄/绿/靛/天/青/紫类清零 → 语义角色类（ok/warn/danger/info/purple/accent/wash）；徽章按 §3.4 配色（该色 16% 底 + 该色字；副作用"纯"徽章按规改中性不给绿）；审批弹窗按钮去红色（风险由染底承载，§3.17 纪律）；`accent-emerald` 原生控件色 → accent |
| 缺省主题 light | ✅ v2.38 | settings/appearance.rs 缺省 dark→light（§8.3-1 用户裁定：旧文件无 appearance 区随之变亮色属预期，显式存过的值不动）；theme.ts 未知值回落同步 light；测试更新（含"显式 dark 不被迁移"新断言） |

### ㉓ UI 设计落地②：壳骨架（✅ 已完成，v2.39）

> §2 区域地图与导航模型兑现：无边框窗口 + 46px chrome 带 + 侧栏 + 目的地
> 整页接管。实机自检（tauri dev 截图）：亮色缺省 / 三段带拼装 / 目的地
> 接管与返回 / 侧栏折叠展开（lead 槽标题零跳变）/ 窗口控制车道全部到位。

| 功能 | 状态 | 说明 |
|---|---|---|
| 无边框窗口 | ✅ v2.39 | tauri.conf `decorations:false` + 首启 1280×820 / 最小 1024×640（§3.27）；capabilities 显式加窗口控制与 start-dragging 权限 |
| 46px chrome 带 | ✅ v2.39 | 侧栏头（品牌+折叠）/ 会话顶栏（lead 槽恒挂+标题拖拽区+动作组）/ 窗口控制车道（120px 浮右端 z-1000，min/max/close 平字形按钮 12px 字形，close hover danger 实底）三段等高拼装；`data-tauri-drag-region` 嵌空白段，按钮区不重叠（§2.2） |
| 侧栏 | ✅ v2.39 | `shell/Sidebar.vue`：搜索（客户端标题过滤）+ 新会话 + 「会话」组（会话↔工作区关联 §8.2 未落地，「项目」组随关联批次出现）+ 时间桶（近 7 天/近 14 天/更早）；选中 wash-3、行 28px/圆角 8/缩进 28；运行中脉动点 + 运行中禁切（行置灰带提示）；底部目的地入口（扩展/记忆/设置，`aria-pressed`）+ 版本号；定时/通知入口随各自批次补位 |
| 侧栏折叠/拖宽 | ✅ v2.39 | 折叠 = 独立状态宽 0（150ms，展开 ease-out/收起 ease-in，体不卸载保滚动）；拖柄按 §2.3 全套（8px 热区、2×32 圆角条 20% ink hover/聚焦显形、`data-sidebar-resizing` 全局锁光标禁选、setPointerCapture+rAF 节流、拖到最小值以下即折叠、键盘 ←/→ 16px Home/End、双击复位 275、aria separator+valuetext）；宽度 localStorage 持久化（`lib/sidebar-preference.ts`，列宽才走 settings §2.7） |
| 目的地接管 | ✅ v2.39 | 扩展/记忆 = 侧栏右侧铺满（DestinationShell 56px 页头 20px 标题）；设置 = 整窗接管（§3.32，连侧栏一起）；对话 hidden+inert 挂起不卸载（恢复滚动与运行态）；设置页头过渡期「‹ 返回应用」（㉔ 导航轨落地后归位） |
| 旧导航退役 | ✅ v2.39 | pill nav 与 SessionSwitcher（⑲ 最小面）删除（§3.6 由侧栏取代）；i18n 增 sidebar/window/common 键、nav.plugins 改「扩展」 |
| shell store + 图标 | ✅ v2.39 | `stores/shell.ts`（目的地 + 侧栏宽/折叠）+ `lib/icons.ts`（原型 sprite 逐字采用的 30 个 glyph 统一组件） |

### ㉔ UI 设计落地③：设置导航轨 + ⌘K 命令面板（✅ 已完成，v2.40）

> §3.31/§3.32 设置整页组装与 §3.15 命令面板兑现。实机走查（tauri dev +
> 截图）：设置整页接管（导航轨四组 + 56px 页头 + 760 列）、轨内点击平滑
> 滚动 + 页头换标题 + 滚动侦测高亮、⌘K 四组与 Esc 键帽、过滤空态、
> ⌘K 命中设置分区 → 跳转 + 轨高亮 + 滚动 + 锚点闪烁、轨内搜索命中行
> 标题（「终端」→ 命令终端）+ 组标签收空隐藏、返回应用后对话视图状态
> 完好。

| 功能 | 状态 | 说明 |
|---|---|---|
| 设置索引唯一真源 | ✅ v2.40 | `lib/settings-index.ts`：11 个实装分区 → 4 组（偏好/智能体/能力与安全/系统）声明式索引 `{ id, group, labelKey, icon, searchKeys }`；导航轨、⌘K、SettingsPanel 三处同吃一份——"新增分区忘了加搜索/面板"结构性不可能（Record\<SettingsSectionId, Component\> 类型保证）；「对话」「关于与更新」未实装不发明占位，随各自批次补条目 |
| 设置导航轨 | ✅ v2.40 | `shell/SettingsNav.vue`：275px `--lw-sidenav-w` + bg-side + 右 1px line；顶部 46px 带 = 拖拽区 + 圆角全搜索框（§3.32 头 = 导航轨顶部）；组标签 11.5px ink-4、项 28px/圆角 8/缩进 12px/hover wash-2/选中 wash-3；搜索匹配分区名 + 分区内行标题（searchKeys 预声明，索引驱动不爬 DOM），组标签随命中收空隐藏 + 「没有匹配的设置」空态；底部固定「‹ 返回应用」 |
| 设置整页组装 | ✅ v2.40 | `settings/SettingsPage.vue`：导航轨 + 内容区（56px 页头显示当前分区标题 20px/600 + 760 正文列居中 + 分区间 gap 32px）；当前分区由滚动侦测驱动（标题越线换高亮）；SettingsPanel 改按索引渲染全分区堆叠 + data-settings-section 锚；过渡期页头「back」钮退役（DestinationShell back prop 删除） |
| ⌘K 命令面板 | ✅ v2.40 | `shell/CommandPalette.vue`：Ctrl/⌘+K 全局开关（Esc 关、↑↓ 移动、Enter 执行、mouseenter 跟随、scrollIntoView nearest）；居中弹层 640px/圆角 18/elevated/dialog 阴影/52px 搜索行 + Esc 键帽；四组 = 会话（相对时间 hint）/ 页面 / 设置（「设置 / 分区」hint）/ 命令（新会话、折叠/展开侧栏）；空 query 全量、过滤后高亮回零、组空隐藏、无结果空态 |
| ⌘K → 设置跳转 | ✅ v2.40 | shell store `settingsFocus {section, nonce}` 载荷：命中设置分区 → 切设置目的地 + SettingsPage 滚到该分区 + 锚点闪烁（`lw-settings-flash` 一次性 wash-3 渐隐，§3.15"闪一下"；重触发用 reflow 复位）；已在本页/跨目的地两路都接（watcher + onMounted 补偿） |
| i18n | ✅ v2.40 | zh/en 增 settings.nav/groups/navSearch/navEmpty + palette.* 五键组；icons 补 globe 字形（sprite 逐字） |

### ㉔·fix 设置页改分类板面切换（✅ 已完成，v2.41）

> 用户 2026-10-02 裁定：设置页不许是"一整条长滚动页 + 左栏名义跳转"，必须
> **拆分到每个分类每个板面**。UI shell 文档 §3.32 先行改版（v3.4 板面切换
> 模型）：右栏同一时刻只渲染当前分区组件，滚动侦测 / 锚点闪烁 / 分区间
> gap 随之退役；⌘K 命中 = 直接切板面。实机走查（tauri dev + 截图）：轨
> 点击切板面 + 页头换标题、⌘K「嵌入」直达设置分区板面、轨搜索「终端」
> 命中切板面、返回应用对话状态完好。

| 功能 | 状态 | 说明 |
|---|---|---|
| 板面切换模型 | ✅ v2.41 | SettingsPage 重写：右栏一次只渲染 `shell.settingsSection` 指向的分区组件（Record 映射随迁，SettingsPanel 堆叠容器删除）；当前分区放壳 store（重开设置保持上次板面），轨点击 / ⌘K / 轨搜索命中都改写同一份状态，组件纯响应式——滚动定位、scroll-spy、`settings-flash` 锚点闪烁、settingsFocus nonce 载荷全部退役 |
| ⌘K 入口收敛 | ✅ v2.41 | shell store `openSettings(section?)` 单一入口（CommandPalette 改调）；同分区连跳 = 幂等无操作（板面已在该处） |
| 文档 | ✅ v2.41 | UI shell v3.4：§3.32 头部增"板面切换模型"段、三/七两条随改、§3.15"闪一下"改板面切换 |

### ㉕ UI 设计落地④：对话主区（✅ 已完成，v2.42）

> §2.7 列宽三轴双柄 / §3.7–3.9 过程组·工具行·思考行 / §3.11 composer 坞 /
> §3.12 用量浮层 / §6.5 markdown 兑现；搭车落 §3.2 `ChatSection`（settings
> `chat` 区，§8.2 预留项）。实机走查（tauri dev + 截图 + settings.json 落盘
> 取证）：markdown 转录 / 过程组折叠展开 / 工具行中文动作与加工摘要 /
> 明细块 / 实时轮（用户气泡 + 流式 markdown + 用量环）/ 列宽柄（键盘
> ±16 / 拖拽实时缩放 + 双柄对称 + composer 同步 + 落盘 / 双击复位 760 /
> 热区下沿停在 composer 顶）/ 设置「对话」分区四行。走查中修掉两个真实
> 缺陷：① i18n 占位文案裸 `@` 触发 vue-i18n linked-format 编译崩溃（整页
> 白屏，`{'@'}` 转义修复）；② 列宽拖拽监听元素级在 capture 失配时随指针
> 出热区即死（改 window 级监听兜底）。

| 功能 | 状态 | 说明 |
|---|---|---|
| settings `chat` 区（后端） | ✅ v2.42 | settings/chat.rs：contentMaxWidth（钳 560–4000）/ expandToolProcess（缺省关）/ enterToSend（缺省开）/ usageReadout（used/remaining）；字段级缺省零迁移；update.rs 校验 + 命令接线 + 测试 3 例（全桌面 93 绿）；前端 types/api/store saveChat + ChatSection 分区（settings-index 新增「对话」条目 + chat 字形） |
| 列宽三轴双柄 §2.7 | ✅ v2.42 | ConversationWidthHandles：偏好值 settings 唯一真源（`chat.contentMaxWidth`），实战宽 = clamp(偏好, 560, 主区宽−48) 写到对话区根 `--lw-content-max`——转录列/composer/消息正文三处同吃一个变量；双柄中心对称、指针 1px ⇒ 2px；12px 隐形热区（容器 pointer-events:none，下沿 = composer 顶）；2×40 竖条 hover 淡入、拖拽 56px 加深（token conv-handle 暗 18%/亮 12%）；拖拽零延迟（data-conv-resizing 全局锁）+ rAF、程序化改宽走 conv-anim 滑动；键盘 ←/→ 16（Shift 32）方向按 data-side、Home 复位/End 最宽/Esc 取消不落盘/双击复位 760；pointerup 落盘 |
| 轮次过程组 §3.7 | ✅ v2.42 | TurnProcess.vue：头行 = sparkle + 处理中/已完成 + · 时长（实时轮每秒走字；历史重放无时间戳不虚构）+ · N 个工具（失败 ⓘ N 个问题）+ chevron 只换字形；hover 溢出 3px；展开体左缩进 + 1px 引导线，只含过程项（工具/思考）；连续过程项合成组（ChatTranscript 分组 walk），系统通知/助手正文/错误行是组外兄弟；折叠本地态、默认一律收起（含运行中）；展开/收起 = §6.6 ④ grid-rows 0fr→1fr + 内容 40ms 延后 + 结束摘 overflow |
| 工具行 §3.8 | ✅ v2.42 | ToolRow.vue 替代 ToolCallCard（卡降级为行，去边框 wash 分层）：折叠单行 = 类目字形 + **中文动作名**（§3.18 映射表 i18n 化，mcp_* 动态拼参；表外回落工具 id + 「插件」徽章）+ 加工参数摘要（lib/tool-summary.ts：命令类剥 cd/bash -lc 只留首命令 + … +N、文件类工作区相对路径、对象类取 prompt/query/url/key 键值不带括号引号、兜底 clip48）+ 写/编辑类变更统计（输出字段尽力提取，取不到不渲染不虚构）+ 状态点（ok 绿点/运行中脉动/失败 danger）；hover 复制调用按钮；展开态 bg-inset 圆角 10 参数/结果原样（失败 error danger 色）；失败自动展开一次（实时与重放同规）；「默认展开工具执行过程」设置只吃实时新发轮次（ts 存在），重放一律折叠 |
| 思考行 §3.9 | ✅ v2.42 | ThinkingRow.vue：sparkle + 「思考」+ 摘要单行截断（剥 markdown 记号与换行取首句）+ 展开原文 pre-wrap（wash-1 圆角 10、12.5px/1.55）；store 加 `thinking` 条目类型——**数据源（推理流事件）待后端（§8.2 已记），不发明占位事件，通道有了即为消费端** |
| composer 坞 §3.11 | ✅ v2.42 | Composer.vue 替代 PromptBar：浮起卡片（r-xl、bg-composer、shadow-composer、无边框、`min(100%, var(--lw-content-max))` 与转录列同源不写死）；自适应文本域 1 行起 max-h 180、占位「输入 / 使用命令 · {'@'} 引用文件」；工具条左 = 模型 chip（状态显示，§3.30 选择器前无菜单）；右 = 用量环（dasharray 按 used/window，点开 §3.12 浮层）+ 36px 发送键（空禁用灰/运行中换停止字形禁用——轮次中止后端 §8.2 待接，不假装可点）；Enter 发送（enterToSend 开关，关 = Ctrl+Enter；IME 组合中不发送）/ Shift+Enter 换行；附件 `+` / 权限 chip / 提示词增强随后端批次（§8.2 已记，不发明占位） |
| 用量浮层 §3.12 | ✅ v2.42 | ContextUsagePopover.vue：环上方右对齐 360px elevated + 1px line（弹层语法）；headline 口径跟 usageReadout（已用/剩余）；窗口行 used/window + %；分项（本轮合计/速度/模型用量/工具上下文）待后端载荷扩展（§8.2 既有行） |
| markdown §6.5 | ✅ v2.42 | lib/markdown.ts（marked + DOMPurify + 代码块增强：语言标签头行 + 复制钮，figure.code-block 形态）+ style.css 全局 .md 规则（段落/列表/行内代码/代码块 bg-inset 圆角 12 12.5px mono/表格满列宽 wash-1 表头 1px 行线无外框/引用）；助手正文整列宽不再是气泡；消息行动作区（复制 + 时间 HH:MM hover 显形，历史重放无时间戳不显示）；纯文本 pre-wrap 渲染缺口补齐 |
| 走查修复 | ✅ v2.42 | ① vue-i18n 占位文案裸 `@` = linked 语法 → 消息编译异常整页白屏（新增文案带 `@`/`|`/`{}` 必须转义——纪律入 §7 备忘）；② 列宽拖拽改 window 级 pointermove/up 监听（元素级在 capture 失配时失效）；③ Composer IME 组合中 Enter 不发送 |

### ㉕·fix 设置页行版式：左设置名、右设置参数/按钮（✅ 已完成，v2.43）

> 用户 2026-10-02 裁定：设置行 = 左行名、右控件。批首按纪律先改 UI shell 文档
> §3.32·七（v3.5 行版式规范，参考对象 `.settings-row`/`.settings-toggle` 逐值照搬）
> 再动码。实机走查（tauri dev + WebView2 CDP 截图，亮/暗双主题）：12 个分区全部
> 行式落位、开关翻转与 aria、行名 label 聚焦控件、暗色瓦片/开关 token 正确。

| 功能 | 状态 | 说明 |
|---|---|---|
| 行模式组件 | ✅ v2.43 | `SettingsRow.vue`（行 = 瓦片卡：wash-1 圆角 14、内边距 14/16、左行名 13px/500 + hint + #detail 动态取值、右控件列右对齐 max-w 240 min-h 38、controlId 连 label、wide 逃生口）+ `SettingsGroup.vue`（13px/600 小标题分块 + #action 块级动作 + 行间 gap 6px 代发丝线）+ `SettingsSaveRow.vue`（保存行左对齐 + 已保存闪烁；错误条收敛到板面级一处） |
| SettingsToggle 开关 | ✅ v2.43 | 32×20 全圆角轨道 + 16px 滑钮（on = translateX(12px)）；新 token `--lw-switch-track(-hover)`/`--lw-switch-knob`（暗 = 白 10%/18% + gray-300 钮，亮 = 近黑 10%/18% + 白钮；on 轨 = accent、on 钮 = accent-ink 不设独立 token）+ tailwind `switch` 角色组；`role="switch"` + aria-checked；`:enabled:hover` 禁用态 hover 不变色；**设置行内布尔项一律开关、复选框退场**（§3.32·七 成规） |
| 12 分区改行式 | ✅ v2.43 | 外观/语言（就地生效无保存钮）/对话（宽度数字 + 两开关 + 读数分段）/上下文（滑条 wide、当前值·建议·警告走 #detail；策略竖排 radio → 下拉）/快捷键/记忆/插件/命令终端（开关 + 行外警告条与重启提示）/模型/嵌入（文本输入 min-140 自适应、检测行 #detail 报维度或错误）/工具与暴露（三分块：截断行组、fsRoots 行内移除、快照整块）/会话与数据（只读路径整块）；卡内与页头重复的分区 h3 退役 |
| i18n 修正 | ✅ v2.43 | 新键 embeddingProbeTitle / fsRootsRemove（zh/en）；陈旧文案纠偏：sessionsMemory「会话保存在内存中重启清空」不符 ⑲ 事实 → 「逐事件落盘 sessions/ 重启恢复」；**走查修真实缺陷：shellHint 裸 `\|` 被 vue-i18n 当复数分隔符、面板只显示"log"**（`{'\|'}` 转义；§七规则 11 扩为 @/\|/{}/保留字符必须转义——㉕ `@` 白屏同源第二例） |

### ㉕·微调① 设置页目检：两色控件体系 + 启动白屏核查（✅ 已完成，v2.44）

> 用户 2026-10-03 目检 11 条裁定（同日起进入"一页完全调完再到下一页"节奏，设置页为
> 第一页）。批首先改 UI shell 文档 v3.6（§3.31 导航轨 / §3.32·七 两色控件体系 /
> §6.1 阴影例外 / §6.4 `--lw-settings-max` / §7 规则 12 字段类焦点环豁免）再动码。
> 模型（llm）分区按裁定豁免，待后期单独大改版。实机走查（WebView2 CDP）逐条取证：
> 行高 30/间隙 2px/搜索胶囊白底距顶 50/返回应用 84px fit-content、列宽 995→窄窗 615
> 无横溢、分段选中灰片、下拉白面板浅高亮、嵌入输入聚焦无环、明暗两主题自洽。

| 功能 | 状态 | 说明 |
|---|---|---|
| 导航轨（①②③④） | ✅ v2.44 | 项行高 28→**30**（+2px）+ **项间 2px 间隙**（高亮瓦片不粘连，组内 flex-col gap）；搜索胶囊**移出 46px 带**（带退化为纯拖拽区）自占一行、底 `--lw-bg` 白 + shadow-raised；「‹ 返回应用」宽度按字符内容收缩（`sidenav-item--fit` fit-content，不做整行瓦片） |
| 正文列（⑤） | ✅ v2.44 | 新量纲 token **`--lw-settings-max: 1024px`**（760 加宽 +34.8% ≥ 1/3）；`w-full max-w` 封顶居中，窗口变窄随窗收缩（CDP 实测 1280→995px、900→615px、无横向溢出） |
| 分段控件（⑥） | ✅ v2.44 | 新组件 `SettingsSegmented.vue`（radiogroup）：容器 = 白底台面（raised + shadow-raised）圆角 full + p-1；**选中片 = `--lw-bg-side`（左轨的灰）**；外观主题三档 / 对话用量读数二档同批换装 |
| 下拉（⑦） | ✅ v2.44 | 新组件 `SettingsSelect.vue` 替代原生 select（语言 / 压缩策略；模型页豁免）：触发器白底台面无点击环；弹出面板 teleport 到 body——白底（raised）+ 1px line + 圆角 14 + shadow-raised；选项 hover wash-2、**选中 wash-3（与导航轨统一，吃掉系统蓝高亮）**+ 勾标；多类别组间 1px 发丝线 + 11px 组名；↑↓/Home/End/Enter/Esc 键盘全套 + `aria-activedescendant`；语言 中文↔English 实测互切 |
| 快捷键可自定义（⑧） | ✅ v2.44 | 新组件 `SettingsKeyRecorder.vue` 录制式改键：点键位钮进录制态 → `e.code` 归一化（免 Shift 符号干扰）→ `Ctrl/Alt/Shift/Meta+主键` 串走既有 `saveShortcut` 即时重注册；纯修饰键等待、Esc 取消、无修饰键且非 F 键拒收；组头右侧「**恢复默认**」（回落 Ctrl+Shift+L）；CDP 真键序录制 Ctrl+Shift+K **settings.json 落盘取证** + 恢复默认回 Ctrl+Shift+L |
| 控件统一（⑩） | ✅ v2.44 | `.lw-field` / `.lw-btn` 两个全局公共类（style.css）：raised 底 + shadow-raised 微悬浮 + 无边框 + hover `--lw-ctl-hover`（新 token 双形双主题：亮 #f3f3f3 / 暗 #303030）；**字段类聚焦不出环**（§7 规则 12 用户裁定豁免；按钮键盘 :focus-visible 环保留）；对话宽度数字框、记忆/插件/开发者数值框、嵌入三输入、检测/刷新/添加/清除 Key/保存按钮全量换装 |
| 说明一行封顶（⑨） | ✅ v2.44 | 组说明与行说明超一行文案全量精简（zh/en）：chatWidth / shell（顺带褪掉 `{'\|'}` 转义）/ memoryAutoExtract / embedding / autoDisableDays / shortcut 六处；新键 shortcutRowHint / shortcutRecording / shortcutRestore，删 shortcutPlaceholder |
| 启动白屏核查（用户追加） | ✅ v2.44 | **根因：窗口默认 `visible:true`，Tauri 在 setup 前即建窗显示——WebView2 初始化 + 前端加载全程露白**（后端 init_state 实测仅 ~40ms 不在华点）；修复 = 窗口 `visible:false` + `backgroundColor` 起步，前端 mounted+2×rAF 首帧后 `show()`+`setFocus()`（capabilities +allow-show/allow-set-focus），Rust 侧 8s 兜底显窗（前端崩溃也不吞窗口）；lib.rs 加 boot 计时日志（builder→setup 668ms→init_state 40ms），前端热启动首帧实测 432ms |

### ㉖ UI 设计落地⑤：审批决策卡 + 会话运营（⬜ §3.17 三键卡（后端：决策枚举/风险档/超时）/ §3.6 重命名·删除·导出·搜索（后端命令）/ §8.2 会话↔工作区关联）

### ㉗ UI 设计落地⑥：工作面板 + 通知（⬜ §2.5 面板与插件贡献槽位 / §3.16 子 Agent 转录页签（后端：子会话落盘）/ §3.28 diff 视图 / §3.13 通知浮层（后端：应用内通知）/ 目的地页整版式 §3.29/§3.31）

---

## 三、收尾 backlog（随批次搭车）

| 项 | 说明 | 计划搭车批次 |
|---|---|---|
| fs roots 工作区目录选择器 | ✅ 已完成（⑱，v2.33，dc2ad87：tools.fsRoots + 保存即生效 env 重注入 + 原生对话框选择器） | — |
| desktop session 持久化 | ✅ 已完成（⑲，v2.34，a5347c3：sessions/*.jsonl 镜像 + 启动恢复 + undo 重写备份 + 会话切换器，详见下方 ⑲ 块） | — |
| LLM 流式输出 | ✅ 已完成（⑳，v2.35，54a97ba：genai 流式 → delta 直推 + streamed 投影去重 + 子内黑盒压制，详见下方 ⑳ 块） | — |
| 市场 wasm 打包分发 | ⑧⑧ 遗留 | 看需求 |
| kv_get 所有权协议 | | 看需求 |
| profile.toml 设置面板接线 | | 看需求 |
| 托盘菜单 i18n | ✅ 已随壳批次完成（a2c82a5：sys-locale 探测，语言改动即时重挂菜单） | — |
| UI 视觉抛光 | ✅ 已完成（⑰，v2.32，945e681：明暗主题 + 语义颜色类 + 统一动效；规范落 UI shell 文档 §六） | — |
| tool-http（POST/PUT 通用请求） | plugins.md P1 唯一未做的工具类；tool-fetch 仅 GET（其代码注释已声明 POST/PUT 归本插件） | 看需求 |
| mw-ratelimit 调用限速 | plugins.md P1.5 中间件：限制单位时间调用次数 | 看需求 |
| 中间件插件化 | mw-audit / 审批门 / ctx-compact 均为宿主侧形态；C ABI `PluginKind::Middleware` 仍是占位变体，无真插件使用 | 看需求 |
| 权限模式四档切换器 | 约束清单 §4（后期）：计划/变更前确认/自动编辑/完全访问，挂会话输入框旁随手切；三类硬底线（路径逃逸/SSRF/ABI 边界）任何档位生效——"完全访问≠无安全" | ⑬ 后看需求 |
| 会话保留天数可配 | 约束清单 §3 余项（session-store 保留期） | 看需求 |

---

## 四、plugins.md 必备插件清单完成度

> 对照《plugins.md》§一总览 19 项逐条核对（2026-09-29，以 plugins-src/ 与
> 提交记录为准）。**结论：还剩 tool-http / mw-ratelimit 两个未排期；P2 五件
> 于 2026-09-30 用户确认排期 = ⑭（schedule+notify）⑮（agent-sub）⑯
> （sandbox），tool-browser 用户裁定最后；其余全部落地。**

### P0 —— 全部 ✅

| 插件 | 状态 | 落点 |
|---|---|---|
| llm-openai / llm-ollama | ✅ | v2.8：宿主侧 llm 模块（genai 三合一），非插件形态 |
| tool-fs | ✅ | v2.9 读三件套 + v2.14 写三件套（write=Compensable{fs.restore} / delete=Irreversible / restore=Irreversible） |
| tool-shell | ✅ | v2.14（白名单 + denylist 永远赢 + 元字符拒绝，一律 Irreversible） |
| tool-fetch | ✅ | v2.9（GET-only；SSRF 防线 v2.14 补） |

### P1 —— 差一个 tool-http

| 插件 | 状态 | 落点 |
|---|---|---|
| tool-edit | ✅ | v2.14（replace/lines/严格 diff，跨插件 undo） |
| tool-git | ✅ | v2.15（status/diff/log/commit/push + undo_commit） |
| tool-http | ⬜ 未排期 | POST/PUT 通用请求；已入收尾 backlog |
| memory-sqlite | ✅ | v2.15（跨会话记忆；plugins.md 所述"会话持久化"归 session-store——desktop 落盘在 backlog） |
| memory-vector | ✅ | v2.28 **宿主侧形态**（42bcbf9，同 llm 插件宿主侧先例：embedding 端点+密钥归宿主）——experiences.sqlite3 + ExperienceSource 契约，详见 ⑬ 行 |
| ctx-compact | ✅ | v2.19 配置驱动收口（三策略/阈值 50%/事件+撤销，92f298b） |

### P1.5 —— 中间件类：宿主侧形态已齐，插件化未做

| 插件 | 状态 | 落点 |
|---|---|---|
| mw-approval | ✅ | 阶段四审批门 + 阶段六桌面审批弹窗（宿主侧） |
| mw-audit | ✅ | v2.9（宿主侧 JSONL 审计） |
| mw-ratelimit | ⬜ 未排期 | 已入收尾 backlog |
| （中间件插件化本身） | ⏸ | `PluginKind::Middleware` 仍是占位变体；已入 backlog |

### P2 —— 只做了 tool-mcp，其余后排

| 插件 | 状态 | 说明 |
|---|---|---|
| tool-mcp | ✅ | v2.15（mcp.json 加载期发现冻结 + stdio JSON-RPC + side_effect 缺省 Irreversible） |
| tool-browser | ⏸ 最后 | 文档自己建议晚做（fetch + MCP 已覆盖多数场景）；**用户裁定 2026-09-30：五件中排最末** |
| tool-sandbox | ✅ | ⑯（v2.31，dc822d9）：受限令牌 + JobObject 限额受限档（能力矩阵如实分级，fs/net 不隔离由策略三道闸补偿），定稿见 plugins.md §六 |
| agent-sub | ✅ | ⑮（v2.30，d29cd69）：kernel-agent `subagent/` 单层调度 + fork 前缀 + 白名单视图 + 审批冒泡，详见 ⑮ 块 |
| tool-schedule | ✅ | ⑭（v2.29，7dc0139）：宿主内置工具面 + schedule.sqlite3 + 触发引擎，详见 ⑭ 块 |
| tool-notify | ✅ | ⑭（v2.29，7dc0139）：webhook 插件（SSRF 防线照搬）+ 桌面通知走 vtable 追加槽 |
