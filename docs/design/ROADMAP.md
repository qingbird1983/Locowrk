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

## 二、当前排期（v2.17，2026-09-29 确认；2026-09-30 增补《Setting and Constrained Optimization.md》逐项核对排期 = S1/S2 两批 + ⑪⑫搭车）

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

### ⑬ 智能层（后排，看需求）

> 四项共享 embedding 基建，出现真实需求一起排。

| 功能 | 状态 | 说明 |
|---|---|---|
| 工具向量检索 | ⏸ | embedding 索引，未就绪降级 BM25/关键词 |
| memory-vector 经历记忆 | ⏸ | 语义检索"聊过什么、做过什么决策" |
| 隐式记忆提取 | ⏸ | 对话后异步提取事实，易记垃圾故放最后 |
| 自动停用设置项 | ⏸ | 默认关闭 |

---

## 三、收尾 backlog（随批次搭车）

| 项 | 说明 | 计划搭车批次 |
|---|---|---|
| fs roots 工作区目录选择器 | 桌面端接线 LOCOWRK_FS_ROOTS | 壳批次或⑩ |
| desktop session 持久化 | 会话事件落盘 | 看需求（⑨ 压缩/撤销在内存会话上已可用；落盘顺带解决撤销 fork 的物理留痕） |
| LLM 流式输出 | genai 流式 → 事件推送 | 看需求 |
| 市场 wasm 打包分发 | ⑧⑧ 遗留 | 看需求 |
| kv_get 所有权协议 | | 看需求 |
| profile.toml 设置面板接线 | | 看需求 |
| 托盘菜单 i18n | ✅ 已随壳批次完成（a2c82a5：sys-locale 探测，语言改动即时重挂菜单） | — |
| UI 视觉抛光 | 壳批次 tokens 之后统一做 | ⑬ 后 |
| tool-http（POST/PUT 通用请求） | plugins.md P1 唯一未做的工具类；tool-fetch 仅 GET（其代码注释已声明 POST/PUT 归本插件） | 看需求 |
| mw-ratelimit 调用限速 | plugins.md P1.5 中间件：限制单位时间调用次数 | 看需求 |
| 中间件插件化 | mw-audit / 审批门 / ctx-compact 均为宿主侧形态；C ABI `PluginKind::Middleware` 仍是占位变体，无真插件使用 | 看需求 |
| 权限模式四档切换器 | 约束清单 §4（后期）：计划/变更前确认/自动编辑/完全访问，挂会话输入框旁随手切；三类硬底线（路径逃逸/SSRF/ABI 边界）任何档位生效——"完全访问≠无安全" | ⑬ 后看需求 |
| 会话保留天数可配 | 约束清单 §3 余项（session-store 保留期） | 看需求 |

---

## 四、plugins.md 必备插件清单完成度

> 对照《plugins.md》§一总览 19 项逐条核对（2026-09-29，以 plugins-src/ 与
> 提交记录为准）。**结论：还剩 tool-http / mw-ratelimit 两个未排期，P2 五件
> 明确后排；其余全部落地。**

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
| memory-vector | ⏸ | → ⑬ 智能层 |
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
| tool-browser | ⏸ | 文档自己建议晚做（fetch + MCP 已覆盖多数场景） |
| tool-sandbox | ⏸ | shell 容器/WASM 化——注意⑧的 WASM 加载器是插件沙箱，不是 shell 沙箱，两回事 |
| agent-sub | ⏸ | 子 Agent 调度 |
| tool-schedule | ⏸ | 定时任务 |
| tool-notify | ⏸ | 桌面通知 / Webhook 推送 |
