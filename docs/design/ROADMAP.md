# Locowrk 落地路线表（ROADMAP）

> **这是唯一的进度看板。** 每完成一批：把 ⬜ 改 ✅（或 ◐ 改 ✅），补上版本号 和提交哈希。设计细节不看这里——每行的"详见"列指向 [DESIGN.md](http://DESIGN.md) 章节或配套 文档。
>
> 图例：✅ 已完成 · ◐ 部分完成（剩余项已拆进具体批次）· ⬜ 未开始 · ⏸ 后排（看需求再动）

---

## 一、已完成

### 阶段一～七（内核与桌面端基座）

| 阶段 | 内容 | 提交 | 详见 |
|---|---|---|---|
| 骨架 | Cargo workspace + 各 crate 骨架，cargo check 通过 | 398a74c | §8 |
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
| ④ | 插件元数据 v2：manifest 解析层 / IconLoader / 展示投影 / origin 分组 + Fallback 矩阵 + locowrk new plugin 脚手架 | 68250d6 + 4ddb5de | v2.11/v2.12 |
| ⑤ | 托盘 + 全局快捷键 + 数据目录改名 \~/.Locowrk | 5a41585 | v2.13 |
| ⑥ | 第二批插件（tool-fs 写三件套 / tool-shell / tool-edit）+ 安全底线（fsutil 允许根 / fetch SSRF 防线 / fs.restore 判 Irreversible / 补偿执行器）+ ctx-compact | 8a3e05a | v2.14 |
| ⑦ | 第三批插件 tool-git / memory-sqlite / tool-mcp | 1e53589 | v2.15 |
| ⑧ | WASM 加载器：PluginLoader trait + WasmLoader（wasmtime 沙箱） | 672f51b | v2.16 |

---

## 二、当前排期（v2.17，2026-09-29 确认；2026-09-30 增补《Setting and Constrained [Optimization.md](http://Optimization.md)》逐项核对排期 = S1/S2 两批 + ⑪⑫搭车；同日二批增补：P2 四件入排期 = ⑭⑮⑯，tool-browser 用户裁定最后；同日三批增补：㉑ 工具卡折叠 = UI 正式设计阶段先行批）

### 壳批次 —— UI 骨架（✅ 已完成，提交 a2c82a5）

> 动因：功能越多 UI 越乱的根因是缺落位规则。壳只锁结构与规则，内容留给 功能批次；视觉抛光仍归最后。详见 **docs/design/UI shell and [patterns.md](http://patterns.md)** （本批落点文档：模式库 / 流向图 / 弹层规则 / 扩展规则）。

| 功能 | 状态 | 说明 |
|---|---|---|
| 导航模型定稿 | ✅ | 顶层 4 视图（对话/插件/记忆/设置）；>4 视图时 pill nav 切侧边栏（届时再评估 router）；开发者面板挂设置内 |
| 列表页模式 | ✅ | patterns/ListPage.vue：插件已安装页（origin chips）+ 记忆页骨架共用；搜索、筛选 chips、批量栏插槽、空态 |
| 设置分区模式 | ✅ | settings.json 分区 ↔ 面板分区一一对应；llm/sessions/language/shortcut 实装，context/memory/developer 占位（⑨⑪⑫ 同批替换） |
| 会话内系统消息模式 | ✅ | chat/SystemNotice.vue + chat store system 类别（存 i18n key）；压缩通知\[查看详情\]\[撤销\]/"已记住"/审批结果统一形态 |
| 徽章体系 + 进度条模式 | ✅ | IdleBadge.vue（六级→灰/黄/红，后端只给结构化数据）+ ContextProgressBar.vue（绿/黄/红，无数据不渲染），挂载点就位数据随⑨⑩ |
| 事件→UI 流向图 | ✅ | UI shell and patterns.md §4：agent://→store→组件 + command→面板 映射表，功能批次只扩表 |
| 弹层规则 | ✅ | UI shell and patterns.md §5：modal（审批，全局至多一个）/ 行内展开 / 独立页面 / 默认不弹 判据 |
| design tokens | ✅ | style.css --lw-\*（表面/文本/语义色/圆角/间距，唯一样式事实源；不做视觉抛光） |
| PluginMarket.vue 拆分 | ✅ | 291 → 54 行：DiscoverTab/InstalledTab 子组件 + lib/plugin-helpers；check_length.sh 纳入 .vue |

### ⑨ 上下文管理收口（P0）（✅ 已完成，提交 92f298b）

> 详见 [DESIGN.md](http://DESIGN.md) §14 v2.17 ⑨段 + 《Context and [Memory.md](http://Memory.md)》§3。

| 功能 | 状态 | 说明 |
|---|---|---|
| ctx-compact 中间件本体 | ✅ | 配置驱动重写：CompactConfig{threshold_tokens, strategy} 经共享 RwLock 注入，settings_set 成功即热换入 |
| 模型窗口来源 | ✅ | settings.json context.windowTokens（设置字段 + 预设 8k–1M + 自定义；genai 不暴露元数据）；窗口未知回落 32k 保守保护 |
| 压缩阈值设置面板 | ✅ | ContextSection：滑条 10–90%（10% 一级）+ 百分比/绝对 token 双显示 + <20% 警告 |
| 压缩策略三档 | ✅ | 滑动窗口（纯本地）/ 分层保留（默认）/ 仅摘要；目标 = 阈值 50%；摘要失败或缺 LlmService 就地降级滑动 |
| 动态建议值 | ✅ | suggested_threshold(window) 后端分档（30/35/25/15），投影带 suggested_pct；前端表单即时同表换算 |
| ContextCompacted 事件 + 撤销 | ✅ | 进 session-store（serde default 兼容旧日志；摘要存全文，折叠留尾部 kept_messages）；撤销 = rewind_to + fork 原地换入（会话 id 不变，其后轮次一并回退） |
| 会话内实时进度条 | ✅ | 每 step 推 agent://context-usage（与压缩同估算口径）→ ContextProgressBar；压缩推 agent://context-compacted → SystemNotice \[查看详情\]\[撤销\] 真接线 |

### ⑩ 暴露声明 + 使用统计地基 + 闲置 UI（✅ 已完成，提交 f6b1a44；搮车：工具名去点号 9a5d39c + 产物重建与目录准入防线）

> 详见 [DESIGN.md](http://DESIGN.md) §14 v2.17 ⑩段 + 《Tool exposure strategy and usage [statistics.md](http://statistics.md)》§4/§3/§8。

| 功能 | 状态 | 说明 |
|---|---|---|
| ToolSpec exposure 字段 | ✅ | plugin-api Exposure（缺省 on_demand）；manifest 可选字段追加零破坏，PLUGIN_ABI_VERSION 不动；always>10 在 RegistryToolSource::list_tools 一次性告警（merge_catalog 纯函数 + 单测，被同名顶掉的不计数） |
| 暴露管线（直通态） | ✅ | list_tools 剔除 hidden（resolve 不过滤，⑫ 检索激活仍可达）；always 直通 + on_demand 全暴露，行为零变化（截断归⑫） |
| ToolCall 事件补 plugin_id | ✅ | Tool trait 加默认方法 plugin_id()（宿主侧非 C ABI，既有实现者零改动；SabiToolAdapter 经 PluginInstance::id()=manifest.id 带出）；事件 #\[serde(default)\] Option<String> 兼容旧日志（roundtrip 单测） |
| usage.sqlite3 统计库 | ✅ | desktop usage.rs：两表同 §3.1/§3.2（tool_usage 含 ewma_score 预留列）；runner 投影到 ToolResult 时 record() → mpsc → 5s 批量刷盘，全句柄 drop 收尾刷一次；库打不开降级丢弃；隐私只记工具名/插件 id/时间戳 |
| IdleLevel / IdleInfo | ✅ | desktop idle.rs：六级 + 9 档 unit 全按 §8.5–§8.7 表（后端裁决）；classify/filter_key 全表单测；builtin 开发态插件不评估闲置 |
| 闲置 UI | ✅ | plugin_list_with_usage（attach join 投影）+ IdleBadge 契约升级（per-unit 人类文案、never_used 黄档对齐 §9.4）+ 第二排闲置 chips（ListPage subFilters）+ 批量栏（全选/停用/启用/卸载，卸载内联二次确认列名）+ 顶部提示条（§9.3 点去看看切筛选）；批量命令 plugin_batch_set_enabled/plugin_batch_uninstall（BatchResult 逐条成败） |
| 停用落盘核对 | ✅ | 核对 service.rs startup_reconcile 确有 \`.filter( |

### ⑪ 记忆注入 + 记忆管理 UI（✅ 已完成，提交 d342551）

> 详见 [DESIGN.md](http://DESIGN.md) §14 v2.17 ⑪段 + 《Context and [Memory.md](http://Memory.md)》§4/§5。

| 功能 | 状态 | 说明 |
|---|---|---|
| memory-sqlite 插件本体 | ✅ | 1.2.0（d342551）：memory_put/get 改名 memory_write/memory_read（§4.4 对齐）+ write/read/search 标 exposure=always（⑫ Top-N 截断不挤出）；list/delete/undo 维持 on_demand；写路径核对为同步落盘（连接按调用开、直写、无队列——"说了记住却没记住"不存在） |
| Memory-Inject 中间件 | ✅ | kernel-agent middleware/memory_inject.rs：MemorySource trait（内核定契约、组装点实现，与 ApprovalResolver 同模式；不经工具管道——每 step memory_list 会污染使用统计与审计）；注入写 RequestPayload::system（新字段，genai 映射 with_system）——记忆在消息列表之外，"系统提示永不压缩"由结构保证，会话日志不落注入内容；注册次序 Memory-Inject → ctx-compact（§3.6），ToolExposure（⑫）落地后排最前 |
| memory.put 同步落盘 | ✅ | 核对即满足（v2.15 起插件就是同步直写）；ROADMAP 旧行描述针对的是"计划中的异步队列"——该队列从未存在 |
| 注入条数可配（约束清单 §3 搭车） | ✅ | settings.json memory 区（enabled / topK，缺省开 / 5），共享 RwLock 注入、settings_set 即热换入；设置面板 MemorySection 真分区替换占位 |
| 记忆管理页 | ✅ | MemoryPage 填充：搜索（防抖）/ 列表（新→旧）/ 行内编辑（不开全局弹层，§5）/ 删除（行内二次确认）/ 清空（底部二次确认，§11.3 一键阻力最小）；每条来源徽章 SourceBadge（§3.4 来源族）+ 相对时间（lib/format.ts）；四命令 memory_list/update/delete/clear_all 薄适配 MemoryStore（宿主直读插件库，ALTER 补 source 列迁移旧库） |

### ⑫ 打分截断 + 元工具（✅ 已完成，提交 da96c6f）

> 详见 [DESIGN.md](http://DESIGN.md) §14 v2.17 ⑫段 + 《Tool exposure strategy and usage [statistics.md](http://statistics.md)》§5/§6。

| 功能 | 状态 | 说明 |
|---|---|---|
| EWMA 打分 | ✅ | flush 时读旧分衰减积累：old × exp(-days/7) + 窗口调用数（⑩ 预留列开始写入）；GlobalEwma trait = 内核定契约、组装点实现（desktop 读 usage.sqlite3，降级空表 = 退回会话内优先） |
| Top-N 截断 + 调试面板 | ✅ | ToolExposure 监听器（agent/request 注册最前，§7 链位）；settings.json tools 区 enabled/maxPerRequest（缺省 20，1–200）共享 RwLock 热换入；开发者分区（壳批次最后一个占位替换）= 截断设置 + 最近一次暴露快照（打开时拉取不轮询；节省走 §3.5 compactNumber 档位） |
| 会话内热度 | ✅ | 从 RequestPayload.messages 实时数 ToolResult（§3.5"不落盘"——投影日志即会话事实，零新 trait、无跨会话竞态）；会话 1.0 > 全局 0.7，always 直通不占额度；检索 0.5 归 ⑬ |
| search_tools 元工具 | ✅ | 宿主内置 always+Pure（目录+统计是宿主关切，插件看不到注册表）；关键词检索全目录（name 2 分/desc 1 分 + query 原串照顾中文）截 8 条；空结果回退 top-10 高频 ∩ 目录，统计也空则如实 no_match；有意扩 API：RegistryToolSource::list_all_tools 含 hidden（§4.1"显式检索才暴露"的唯一通路，resolve 本就不过滤） |

### S1 tool-shell 安全分级与进程治理（✅ 已完成，提交 f0fce64）

> 核对修正三条（排期按修正后事实走）：① 白名单"prog+子命令"两段匹配**机制已在** （[policy.rs](http://policy.rs) `decide`，`LOCOWRK_SHELL_ALLOW` 支持 `git log` 形态）——缺的只是默认 白名单内容与设置 UI 化；② 输出超限**已是截断 + `truncated` 标记**（[exec.rs](http://exec.rs)）， 不是清单说的"直接拒绝"——只剩提示文案；③ allow∩denylist 冲突**调用时已明确报 permanent denylist，非静默**——补的是启动期 warn。前提"fs\_\* 守卫修好"已满足 （7ba8776）。denylist 现 64 项（清单写 \~50）。

| 项 | 状态 | 说明 |
|---|---|---|
| 只读命令免审批分级 | ✅ | 拆只读工具形态：shell_read（Pure）只放 ls/cat/grep + git status\|log\|diff\|show（固定表，不吃 LOCOWRK_SHELL_ALLOW）；路径校验 paths.rs 复用 fsutil allowed_roots（与 fs\_ 同源根）\*：生效 cwd 出根整条拒、非 flag 候选与 --flag=值 resolve 出根拒、-f/… 贴值形态带分隔符即拒、不存在候选视为模式串放行；拒绝文案引导改用 shell（降级审批 = 模型换工具，宿主审批门不变）；plugin-host e2e 新用例覆盖三态 |
| denylist 补齐 | ✅ | 双清单拆分：DENY_HARD 46 项（解释器/网络主力 + 新补包装 xargs/env/find/nice/timeout/nohup/time、网络 nc/ncat/netcat/telnet/rsync、exec 面 wscript/cscript/mshta/rundll32/osascript）+ DENY_RISKY 35 项（高危可解锁）；§2.3 磁盘/系统类 mount/chroot/systemctl 随 §2.7 裁决归高危 |
| allow∩denylist 启动冲突日志 | ✅ | policy.rs conflicting_allow（两级清单求交）+ make_state 里经宿主 vtable 打 warn 点名被压制条目；解锁开关开启也打一条可见 warn（红警告原则）；denylist 永远赢不变 |
| 进程组 kill | ✅ | Windows：JobGuard（CreateJobObjectW + JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE，spawn 即 assign，Drop/超时 TerminateJobObject）——windows-sys 0.61 需补 Win32_Security feature（CreateJobObjectW 签名引 SECURITY_ATTRIBUTES）；Unix：Command::process_group(0) + killpg(SIGKILL)；正常退出也收尾（先清场再收管道）——残留孙进程握管道会让 rx 永等，旧代码本就有的悬挂一并修掉 |
| 截断提示文案 | ✅ | stdout/stderr 截断尾部附"\[output truncated at N bytes — narrow the command scope … or raise max_bytes and retry\]"（英文，模型面文案与工具 description 同语言） |
| 高危命令可解锁开关 | ✅ | settings.json 新 shell 区 {unlockRisky，缺省 false} → state.rs 插件加载前写 LOCOWRK_SHELL_UNLOCK_RISK env（tool-shell init 读一次，重启生效——插件无运行期配置通道，kv_get 尚未开放）；解锁后高危放行，shell 工具 Irreversible，放行即审批；硬锁类不受开关影响；ShellSection.vue 设置分区（琥珀红警告 + 重启提示） |

### S2 设置归属、数字呈现与内置插件停用（清单 §1/§5/§6/§3 余项）

> **✅ 已完成（v2.27，提交 7448737）**——用户命名裁定：所有「禁用」统一改叫「停用」 （内置与第三方同一动词，zh `disable`/`stateDisabled` 键值 + 各处注释同步）。 ① lockfile 落 `InstallSource::Builtin` 条目（登记保留 enabled，重启不复活停用项）； ② 启用从内置根解析**最新**版本（独立升级，漂移同步回条目）；③ 市场安装同 id 条目转 Market（安装态接管，开发态扫描让位）；④ 窗口字段迁 llm 区（open 读旧 值迁移一次幂等）；⑤ Ollama /api/show 探测 + 静态档案前缀命中 + 手动兜底； ⑥ compactNumber 全接线（阈值预览 ≈51.2K 不再露 52,428.8）；⑦ tool-shell 产物 1.2.0。plugin-service 新 `builtin.rs`/`service/enable.rs`，desktop 新 `llm/probe.rs`/`lib/model-profiles.ts`。

| 项 | 状态 | 说明 |
|---|---|---|
| 内置插件停用 | ✅ | \*\*（用户命名：禁用→停用）\*\*InstalledTab builtin 行加 停用/启用（不给卸载——随程序分发，uninstall/update 服务端拒绝；不进批量多选）；plugins_root::load_all 带 skip 集（lockfile 接管 + 已停用），装载成功即 register_builtin 落 InstallSource::Builtin 条目（版本同步、保留 enabled——重启不复活）；set_enabled Builtin 分支从内置根解析最新版本；停用即出工具候选池（RegistryToolSource::resolve 只遍历 active_instances，deactivate 即过滤）；操作门控按 source（lockfile 来源）而非 origin（manifest 自声明）；对账不删 Builtin 条目（内置家不在安装根）；插件端解析下沉 plugin_service::builtin（版本/工件发现共享，桌面不再自留副本） |
| 上下文窗口迁模型配置 | ✅ | settings key context.windowTokens → llm.windowTokens（open() 读旧值迁移一次，幂等，落盘随下次保存）；ContextSection 只去掉窗口下拉这一条（用户裁定：阈值滑条/策略/预览全部原地不动），LlmSection 增窗口下拉（provider/model/baseUrl/key 之后）；Ollama /api/show 探测命令 llm_probe_window（llm/probe.rs，reqwest json 只加无 TLS 特性——探测走 http，图内 TLS 由 genai 的 reqwest 统一；顶层 context_length 或 model_info <arch>.context_length 两代形态 + 5s 超时）；lib/model-profiles.ts 静态档案（deepseek/claude/gpt-4o/gemini/llama3.1/qwen2.5 前缀命中）只在未设置时自动填充，手动选择永远优先；读不到 UI 错误条提示手动兜底；agent_chat UsagePolicy / compact_config 改用 effective_window/effective_threshold_tokens（llm 优先、旧字段兜底、未知回落 32k） |
| 数字呈现落地 | ✅ | lib/format.ts compactNumber（上批已备好的唯一实现，1024 进位）接线：LlmSection 预设标签（256K 不写 "262,144 token"）、ContextSection 阈值预览（≈51.2K——1024 进位的真实值，UI shell doc §3.5 示例已同步改注）、ContextProgressBar 比例段 ≈52K / 256K（chat store 透传 windowTokens，窗口未知只显百分比）+ 压缩 SystemNotice"已节省"档位化（rider）；组件不自带第二把尺 |
| shell 超时/输出限长默认可配 | ✅ | LOCOWRK_SHELL_TIMEOUT（1–120s）/ LOCOWRK_SHELL_MAX_BYTES（1024–256KB）环境默认，init 读一次（RunDefaults::from_env）：越上界钳制、低于下界/非数字回落内置缺省并经宿主 vtable warn（§2.4 不静默）；模型 arg 上限 120s/256KB 不变；tool-shell 产物 1.2.0 |
| 宿主 Timeout 中间件可配 | ✅ | kernel-agent Timeout::hot(Arc<RwLock<Duration>>)（new 保留给测试/CLI，DurationSource 两态）；settings.json tools 区 callTimeoutSecs（5–3600 缺省 30，state.rs 起共享锁 timeout_cfg）；settings_set 热换入即生效；DeveloperSection 增数字输入 |
| 内置插件版本策略定稿 | ✅ | 方案 B 独立升（清单推荐，同意）；bundled 字段不加——source（lockfile 管理门控）× origin（manifest 展示分组）双字段已够；接管/回归/启用取最新等规则成文 packaging.md §9.4 |

### ⑬ 智能层（✅ 已完成，v2.28，提交 42bcbf9）

> 四项共享 embedding 基建一起落地。**可降级是总原则**（§1）：embedding 未配置或端点不可用 = 智能层整体缺席，检索退关键词、经历不写不注入、 提取跳过经历——聊天主链路零影响。

| 功能 | 状态 | 说明 |
|---|---|---|
| 工具向量检索 | ✅ | kernel-agent 新 tool_retrieval：EmbedPort 契约（内核定契约、组装点实现，同 GlobalEwma 模式）+ 描述向量缓存（键含描述原文，插件换描述即失效）+ 余弦打分；ToolExposure 打分加 +0.5 × similarity（§5.3 连续补充信号），只在目录 > 上限（真的要裁剪）时才打 embedding；embedding 未配置/失败 = 空表退回两层打分；快照增 retrieved 归因（开发者面板同行渲染）；search_tools 元工具同升：语义优先（相似度 floor 0.25 取前 8，一次批量嵌查询+全目录）→ 关键词 → 高频回退链不变 |
| memory-vector 经历记忆 | ✅ | 宿主侧形态（同 llm 插件先例：写入要过 embedding 模型，端点+密钥归宿主，插件无运行期配置通道；ROADMAP §四已注记）：desktop experiences.rs——experiences.sqlite3（content/embedding BLOB f32-LE/session_id/created_at，§4.3 表结构），余弦检索（全库限 2000 行、维度不符行跳过 = 换 embedding 模型后旧行自然退役）；kernel-agent ExperienceSource async 契约 → Memory-Inject 并列注入 <session_experiences> 块（查询 = 最近一条用户消息；同走 topK 上限）；记忆页增经历区块（可见可删可清空，§11.3 信任基础；无编辑语义——自动沉淀原文） |
| 隐式记忆提取 | ✅ | settings memory.autoExtract（缺省关——"易记垃圾故放最后"）；desktop extract.rs：轮次收敛（TurnDone）后 fire-and-forget——本轮消息转录（≤6000 字符）→ LlmService 提取 JSON → ① 事实 ≤3 条（key 清洗 + 强制 auto. 命名空间，永不覆盖用户显式记忆；source=extracted）写 memory 库（MemoryStore::write INSERT..ON CONFLICT）+ ② 经历摘要（≥10 字符，≤500）嵌入落 experiences；parse 剥围栏取 JSON、空 key/value 过滤；任何失败只 warn 不影响聊天；无 UI 通知——结果在记忆页可见（来源徽章"自动提取"） |
| 自动停用设置项 | ✅ | settings plugins.autoDisableIdleDays（0=关 缺省 / 7–365 校验）；desktop autodisable.rs：启动 + 设置保存即巡检——距最近使用（从未使用 = 安装时点，与 ⑩ IdleBadge 同口径）≥ N 天的安装态第三方插件逐条停用（内置不参与闲置评估，同 ⑩；只停不启——关闭不复活已停用项，§8.8 状态切换由用户显式触发）；EmbeddingSection / PluginsSection 新设置分区（§7 扩表）+ §4.2 命令表扩 llm_probe_embedding（检测显示向量维度）/ experience\_\* 三命令 |

### ⑭ 触发与推送：tool-schedule + tool-notify（✅ 已完成，v2.29，提交 7dc0139）

> [plugins.md](http://plugins.md) P2 四件入正式排期的第一批（tool-browser 仍后排——用户裁定 最后）。两件合批：调度与推送天然成对（触发时刻的"告知用户"通道就是 notify）。生命周期归属先定调：定时器/触发在宿主主循环（tokio time 已 在依赖），插件无后台常驻能力——调度工具面走宿主内置（search_tools 先 例），通知的桌面通道经宿主 vtable 追加槽位。

| 功能 | 状态 | 说明 |
|---|---|---|
| tool-schedule 定时任务 | ✅ | 宿主内置工具面：desktop scheduler/（store/engine/tools 分文件）——schedule_create / schedule_list / schedule_delete 经 KernelToolSource::with_builtin 进目录（create=Always+Irreversible 审批即告知，list/delete=Pure 按会话隔离）；独立 schedule.sqlite3（MAX_SCHEDULES=100 防失控）；一次性（at/delay_secs）+ 周期（interval_secs ≥10s）两型；绑定会话 = CallCtx.session_id（kernel-agent 新 step_as 首次填充 §5.2 规划字段，runner 聊天路径切换，普通 step 语义不变）；触发引擎 1s tick：目标解析 = 调度行会话条目仍存活 → 原地注入，行已不在（重启）→ 落最近聊过的活会话（agent_chat 记 last_chat，不落僵尸会话），无活会话 → 30s 顺延等待；会话忙（try_lock 失败）30s 顺延，一次性累计 2h 放弃并系统通知、周期让位 20 次跳槽不追帧；启动补跑无单独路径——错过期间的 run_at 落在过去，首次 tick 的 due() 天然覆盖（周期补跑一次后滚 now+interval）；触发先行推 agent://schedule-fired（SystemNotice），窗口不可见/失焦再补系统通知兜底 |
| tool-notify 桌面通知/Webhook | ✅ | plugins-src/tool-notify 1.0.0：一个 notify 工具按 channel 参数二选一——webhook 通道 = 插件本体（POST JSON {title,body,source}，tool-fetch SSRF 防线照搬：目标解析全地址逐判、私网/环回/链路本地拒绝、redirects(0)、LOCOWRK_NOTIFY_ALLOW_PRIVATE=1 显式后门；非 2xx = 未送达 = 报错，不学 fetch 把 4xx 当数据）；桌面通知通道 = plugin-api HostVtable 末尾追加 notify 槽（append-only，PLUGIN_ABI_VERSION 不动；repr(C) 前缀兼容旧插件），plugin-host set_notify_hook 注入（desktop_notify.rs 注册，headless kernel-bin 返回 ERR_UNSUPPORTED 如实上报）；整体 Irreversible（宁严勿松：webhook 对外发数据，分级是工具级粒度） |

### ⑮ agent-sub 子 Agent 调度（✅ 已完成，v2.30，提交 d29cd69）

> 形态先定调：**宿主/内核侧**而非 C ABI 插件——子 Agent 要递归跑内核循环， 依赖方向禁止插件依赖 kernel-agent（同 llm / memory-vector 宿主侧先例， 理由不同：这次是循环本体，不是密钥）。v1 收敛为单层串行，不做多 Agent 协作协议。

| 功能 | 状态 | 说明 |
|---|---|---|
| 子 Agent 工具面 | ✅ v2.30 d29cd69 | kernel-agent subagent/ 模块：SubagentTool（subagent_run(prompt, tools?, max_turns?, timeout_secs?)，Irreversible 宁严勿松 + Always 暴露）+ SubagentGate 快照表（内核每个带身份 step 刷新父转录前缀）+ Kernel::child_kernel 派生作用域子内核；组装点注册（desktop，同 search_tools 形态；kernel-bin 未接，需时搭车）；单层硬剔除（FilterToolSource 恒剔 subagent_run，白名单显式包含也无效） |
| 隔离与预算 | ✅ v2.30 d29cd69 | 子会话 = 父转录可见前缀 fork（复用 ⑨ Session::fork/rewind 语义，{parent}-sub-{n} 内存投影）；tools 缺省 = 继承父全目录、非空 = 严格子集 ∩ 父目录（越权 resolve → NotAvailable → failed 如实闭合）；轮数缺省 8/上限 32、超时缺省 300s/上限 1800s（0 = 不限），越限 status = budget_exhausted / timeout，摘要取子转录尾部不虚构；收束 JSON {status, summary, turns, tools_used, child_session_id\[, error\]}，摘要 4000 字截断 |
| 审批与事件 | ✅ v2.30 d29cd69 | ToolCallReq 增 session_id（step_inner 与 CallCtx.session_id 同点填充，12 处构造点补齐）；子内 Irreversible 冒泡父内核同一 ApprovalHub，载荷带 {parent}-sub-{n}，ApprovalModal 紫"子 Agent 调用"徽章；派生/收束落父会话日志（subagent_run 的 ToolCall/ToolResult 事件即日志事实）+ 前端 SystemNotice（§4.1 扩表 2 行先行：tool-call → subagentStarted / tool-result → subagentDone + 卡片照常）；子内活动不投影（黑盒） |

### ⑯ tool-sandbox shell 沙箱（✅ 已完成，v2.31，提交 dc822d9）

> [plugins.md](http://plugins.md) 仅一行定义（"shell 在容器/WASM 里跑"），太薄，不足以直接动码； 且 Windows 开发机无容器保证。方案取舍是本批第一个交付物，落 [plugins.md/](http://plugins.md/) DESIGN 补记后再写实现。

| 功能 | 状态 | 说明 |
|---|---|---|
| 方案定稿 | ✅ v2.31 dc822d9 | 三候选取① Windows 受限令牌 + JobObject 限额（② WASI 只能跑 wasm 化命令实用性窄，记录为远期可选内建命令集；③ 容器可用性依赖用户环境，记录为远期可选后端）；能力矩阵落 plugins.md §六：内存/进程数/CPU 占比硬限、剪贴板 UI 限制、无特权受限令牌（DISABLE_MAX_PRIVILEGE + deny Administrators 不可 UAC 提升）、整树治理 ✅；文件系统与网络不隔离 ❌ 如实分级（不做虚假安全声明——策略三道闸因此保留，高危无解锁开关引导走 shell） |
| tool-sandbox 插件本体 | ✅ v2.31 dc822d9 | 单工具 sandbox_run（Irreversible 一律审批）：JobObject 限额（mem_mb 缺省 512 / max_procs 64 / cpu_rate 可选硬顶，arg/env 双层缺省钳上限）+ 受限令牌 spawn（CREATE_SUSPENDED → 挂 Job → Resume；令牌不可用降级 job-only，输出 isolation 字段如实上报不静默）+ 同源三道闸（LOCOWRK_SANDBOX_ALLOW 扩展、denylist 永远赢）；Unix 对称 pre_exec setrlimit（AS/NPROC/CPU）+ 进程组树治理；与 tool-shell 并存不替换（其白名单/denylist/进程治理纪律原样保留）；测试 15 例含真跑受限 spawn 全链（含全量限额参数） |

### ⑰ UI 视觉抛光（✅ 已完成，v2.32，提交 945e681）

> 壳批次 §六 明言"明暗主题切换、动效、精调间距归 ⑬ 后统一做"；本批兑现。 批首交付 = 先改 UI shell 文档（§六重写为主题/颜色类/动效规范 + §3.2/§七 扩表），再动码。

| 功能 | 状态 | 说明 |
|---|---|---|
| 明暗主题切换 | ✅ v2.32 945e681 | settings.json 新 appearance 区（dark / light / system，缺省 dark 旧文件零迁移）；Rust settings/appearance.rs + update 校验 + 2 测试；前端 lib/theme.ts 唯一执行者（写 <html data-theme>，system 档 matchMedia 监听，组件禁自读媒体查询）；AppearanceSection 三档分段控件挂 Language 之后；style.css 双主题变量块——暗色 = 原调色板值像素级不变，亮色逐 token 选配浅色族。已知近似：原生标题栏跟随系统不随应用切换 |
| 组件层语义颜色类 | ✅ v2.32 945e681 | tailwind.config 把颜色全部绑到 --lw-rgb-\* 三元组（rgb(var(--x) / <alpha-value>) 形态是 47 处 bg-x/60 半透明修饰符能工作的前提）；组件 neutral 灰阶类清零 → 语义角色类 surface-0..3 / ink-1..5 / line(-strong) / accent(-hover)；语义 hue 38 个实用 (hue,shade) 全部入映射表（暗=原值，亮=浅色族）；token 双形态纪律（hex + 三元组同值同改）入 §六 |
| 统一动效 | ✅ v2.32 945e681 | §六三类：交互元素 @layer base 统一 transition 120ms；弹窗 fade+scale（lw-modal）、SystemNotice 淡入上滑（lw-notice）入场、进度条宽度过渡（200ms）；prefers-reduced-motion 一律降 0；禁止常驻动画/视差/弹跳入规。间距维持五档不批量改造（抽查无系统性问题） |

---

### ⑱ fs roots 工作区目录选择器（✅ 已完成，v2.33，提交 dc2ad87）

> backlog"桌面端接线 LOCOWRK_FS_ROOTS"兑现。fsutil `allowed_roots()` 每次 工具调用读 env（无缓存），因此选择器做成**保存即生效免重启**；目录选择 走 OS 原生对话框，Rust 侧封装（UI shell 文档 §七规则 8）。

| 功能 | 状态 | 说明 |
|---|---|---|
| settings tools.fsRoots | ✅ v2.33 dc2ad87 | 缺省空 = 仅数据根（与 fsutil 缺省语义一致，旧 settings.json 零迁移）；normalize 校验 trim/去空/去重保序 + 目录存在性 fail-closed（同 fsutil 使用时契约，保存时即报）；env 注入双时机 = 启动 + settings_set tools 区改动即重注入，空表移除 env |
| DeveloperSection 工作区目录块 | ✅ v2.33 dc2ad87 | pick_directory 命令（tauri-plugin-dialog 仅 Rust 侧，无 JS guest/capability）+ 列表增删即存即生效；§3.2 分区表纠偏（memory/developer 占位标记更新为已实现、补 shell 行）+ §七规则 8（原生对话框封装模式） |

---

### ⑲ desktop session 持久化（✅ 已完成，v2.34，提交 a5347c3）

> backlog"desktop session 持久化（落盘顺带解决撤销 fork 的物理留痕）"兑现。 磁盘 = 内存日志的**镜像**（`<数据根>/sessions/<id>.jsonl` 一行一事件）： 追加与投影共用同一游标；undo fork 截断整体重写 + `.undo.bak` 滚动备份。

| 功能 | 状态 | 说明 |
|---|---|---|
| session-store persist 模块 | ✅ v2.34 a5347c3 | JSONL 落盘/恢复/undo 重写三件套 + 列表（mtime 降序、首条用户消息截断做标题）；session id 白名单防穿越；坏行跳过计数（崩溃半行不致命）；测试 4 例 |
| desktop 接线 | ✅ v2.34 a5347c3 | 启动恢复先于调度引擎（调度行绑定的旧会话跨重启存活）；run_turn 增 log 参数增量镜像（手聊/调度两路）；context_undo_compact 后同步重写磁盘；session_list / session_history 命令（历史 = runner 同一投影函数 project_events 全量重放，不走事件通道） |
| 会话切换器（最小面） | ✅ v2.34 a5347c3 | UI shell 文档 §3.6 先行：chat 头部下拉（首条用户消息截断）+ 新会话；切换 = 全量投影重放恢复气泡/工具卡/系统通知；首聊前不建后端状态；列表运营类 UI（搜索/删除/改名）留给 UI 正式设计阶段 |

---

### ⑳ LLM 流式输出（✅ 已完成，v2.35，提交 54a97ba）

> backlog"genai 流式 → 事件推送"兑现。通道与前端追加逻辑零改动（壳批次 预留的 delta 语义），改点在内核 delta 出口契约与投影去重。

| 功能 | 状态 | 说明 |
|---|---|---|
| 内核流式契约 | ✅ v2.35 54a97ba | LlmService::complete_streaming（缺省回落 complete 整段一次回调）；LlmStreamSink delta 出口（返回"是否真消费"，黑盒替身 false）；StepOutcome::Continue { streamed } 仅 sink 真消费才置位；无 sink = 纯非流式；child_kernel scoped 换入 NoLlmStreamSink——子内 delta 不泄漏父 UI（⑮ 黑盒保持） |
| genai 流式 + 桌面接线 | ✅ v2.35 54a97ba | complete_streaming_with 走 exec_chat_stream（capture_content/capture_tool_calls 聚合正文与工具调用，End 取 captured_content；推理流不进正文）；StreamChunkSink 直推 agent://chunk；runner 按 streamed 抑制该步整段 chunk——同一正文不出现两遍 |
| 搭车 | ✅ v2.35 54a97ba | api.ts CHANNELS 补 agent://schedule-fired（⑭ 遗留死分支：store 有分支、通道缺席）；§4.1 chunk 行语义升级成文 |

---

### ㉑ 工具调用卡默认折叠（✅ 已完成，v2.36，提交 977853b）

> UI 正式设计阶段首个落点（用户 2026-09-30 指定先做）：对话密度治理—— 不然一轮多工具调用的会话瞬间被参数/结果 JSON 刷满，正文被挤走。 UI shell 文档 §3.7 先行成文（模式 + §4.1 组件列指向更新）。

| 功能 | 状态 | 说明 |
|---|---|---|
| ToolCallCard 模式组件 | ✅ v2.36 977853b | chat/ToolCallCard.vue（模式库第二实例）：默认折叠 = 头部单行（▸/▾ + 工具名 + 版本/副作用徽章 + 参数摘要（args 首个原始值 clip 48）+ 状态），整行 button 点击展开参数/结果明细（v-show 保 DOM）；折叠态是组件本地状态，不入 store 不持久化，重放/切会话回折叠 |
| 失败自动展开 | ✅ v2.36 977853b | ok === false（实时到达 pending 翻转 / 历史重放挂载即检）展开一次——错误可见性优先于密度，用户可手动再折叠；运行中不加脉动动画（§六禁常驻动画） |
| 迁出 + clip 收敛 | ✅ v2.36 977853b | ChatMessages 内联卡（\~70 行）迁出换组件（175→101 行）；clip 从 chat store 私有助手收敛进 lib/format.ts，通知摘要与折叠行参数摘要共用 |

---

### ㉑·fix 子代理收束汇报误报父史（✅ 已完成，v2.37，提交 2332d66）

> smoke 实测跟进（用户转来探通报告："tools_used 出现未授权的 mcp_status， 与子的自述矛盾"）。核查结论：**白名单过滤无恙**——FilterToolSource list/resolve 双侧过滤，且 resolve 前置于 ToolCall 事件写入，越权调用 NotAvailable 即整步 failed（既有测试 `explicit_whitelist_blocks_unlisted_resolve` 覆盖）；误报根因是 `tools_used`/`tail_summary` 扫了**整个**子转录， 把 fork 前缀里**父的历史调用**算成了子的使用。

| 修复 | 状态 | 说明 |
|---|---|---|
| 自身段口径 | ✅ v2.37 2332d66 | run_child 在 fork+追加任务 prompt 后记 own_from = child.len()，tools_used / tail_summary（含"最后动作"兜底）只扫自身段——父的调用史/旧正文不再混入子的收束汇报 |
| 回归测试 | ✅ v2.37 2332d66 | tools_used_ignores_forked_parent_history：父转录预置 mcp_status 调用，派生后断言 tools_used == \["echo"\]、摘要取子自身段 |

### ㉒ UI 设计落地①：design tokens v2 与基线（✅ 已完成，v2.38）

> UI 正式设计阶段（UI shell 文档 v3.3 定稿 + ui-prototype 原型，提交 775b14f）第一个实现批次。批首先落文档（v3.3 已含全部规范），本批兑现 §六 tokens 与基线层。用户裁定（2026-10-02）：后端服务前端，缺的功能 后期逐步补，前后端拆分可按需调整。

| 功能 | 状态 | 说明 |
|---|---|---|
| tokens v2 | ✅ v2.38 | style.css 按 §6 重写：13 档中性灰阶（唯一颜色源）+ wash 三层（平面内无边框分层）+ 表面层级（bg/side/dock/inset/secondary/tertiary/elevated/composer/raised）+ ink 五级 alpha + 单色 accent（暗=白/亮=近黑）+ 状态五色；形状阶梯 10 档；量纲（toolbar-h 46 / ctl 28 / field-h 38 / content-max 760 三轴同源 / wincontrols 120 / panel/main 地板）；修正原型两处历史残留（820→760、--motion-\*→--dur-/--ease- 四档四缓动） |
| tailwind 角色色 | ✅ v2.38 | 配置只留 §6.1 角色 token：实色走 --lw-rgb-\* 三元组（保 /16 修饰符），自带 alpha 的（wash/ink/line/表面亮色）整值 var 引用（修饰符会静默丢弃，禁止使用）；38 阶 hue 映射表收缩到语义色 + boxShadow 三 token（dialog/composer/raised）；过渡别名 surface-0..3 暂留（组件重写批次清除） |
| hue 类清零 | ✅ v2.38 | 全部 22 个组件的红/琥珀/黄/绿/靛/天/青/紫类清零 → 语义角色类（ok/warn/danger/info/purple/accent/wash）；徽章按 §3.4 配色（该色 16% 底 + 该色字；副作用"纯"徽章按规改中性不给绿）；审批弹窗按钮去红色（风险由染底承载，§3.17 纪律）；accent-emerald 原生控件色 → accent |
| 缺省主题 light | ✅ v2.38 | settings/appearance.rs 缺省 dark→light（§8.3-1 用户裁定：旧文件无 appearance 区随之变亮色属预期，显式存过的值不动）；theme.ts 未知值回落同步 light；测试更新（含"显式 dark 不被迁移"新断言） |

### ㉓ UI 设计落地②：壳骨架（✅ 已完成，v2.39）

> §2 区域地图与导航模型兑现：无边框窗口 + 46px chrome 带 + 侧栏 + 目的地 整页接管。实机自检（tauri dev 截图）：亮色缺省 / 三段带拼装 / 目的地 接管与返回 / 侧栏折叠展开（lead 槽标题零跳变）/ 窗口控制车道全部到位。

| 功能 | 状态 | 说明 |
|---|---|---|
| 无边框窗口 | ✅ v2.39 | tauri.conf decorations:false + 首启 1280×820 / 最小 1024×640（§3.27）；capabilities 显式加窗口控制与 start-dragging 权限 |
| 46px chrome 带 | ✅ v2.39 | 侧栏头（品牌+折叠）/ 会话顶栏（lead 槽恒挂+标题拖拽区+动作组）/ 窗口控制车道（120px 浮右端 z-1000，min/max/close 平字形按钮 12px 字形，close hover danger 实底）三段等高拼装；data-tauri-drag-region 嵌空白段，按钮区不重叠（§2.2） |
| 侧栏 | ✅ v2.39 | shell/Sidebar.vue：搜索（客户端标题过滤）+ 新会话 + 「会话」组（会话↔工作区关联 §8.2 未落地，「项目」组随关联批次出现）+ 时间桶（近 7 天/近 14 天/更早）；选中 wash-3、行 28px/圆角 8/缩进 28；运行中脉动点 + 运行中禁切（行置灰带提示）；底部目的地入口（扩展/记忆/设置，aria-pressed）+ 版本号；定时/通知入口随各自批次补位 |
| 侧栏折叠/拖宽 | ✅ v2.39 | 折叠 = 独立状态宽 0（150ms，展开 ease-out/收起 ease-in，体不卸载保滚动）；拖柄按 §2.3 全套（8px 热区、2×32 圆角条 20% ink hover/聚焦显形、data-sidebar-resizing 全局锁光标禁选、setPointerCapture+rAF 节流、拖到最小值以下即折叠、键盘 ←/→ 16px Home/End、双击复位 275、aria separator+valuetext）；宽度 localStorage 持久化（lib/sidebar-preference.ts，列宽才走 settings §2.7） |
| 目的地接管 | ✅ v2.39 | 扩展/记忆 = 侧栏右侧铺满（DestinationShell 56px 页头 20px 标题）；设置 = 整窗接管（§3.32，连侧栏一起）；对话 hidden+inert 挂起不卸载（恢复滚动与运行态）；设置页头过渡期「‹ 返回应用」（㉔ 导航轨落地后归位） |
| 旧导航退役 | ✅ v2.39 | pill nav 与 SessionSwitcher（⑲ 最小面）删除（§3.6 由侧栏取代）；i18n 增 sidebar/window/common 键、nav.plugins 改「扩展」 |
| shell store + 图标 | ✅ v2.39 | stores/shell.ts（目的地 + 侧栏宽/折叠）+ lib/icons.ts（原型 sprite 逐字采用的 30 个 glyph 统一组件） |

### ㉔ UI 设计落地③：设置导航轨 + ⌘K 命令面板（✅ 已完成，v2.40）

> §3.31/§3.32 设置整页组装与 §3.15 命令面板兑现。实机走查（tauri dev + 截图）：设置整页接管（导航轨四组 + 56px 页头 + 760 列）、轨内点击平滑 滚动 + 页头换标题 + 滚动侦测高亮、⌘K 四组与 Esc 键帽、过滤空态、 ⌘K 命中设置分区 → 跳转 + 轨高亮 + 滚动 + 锚点闪烁、轨内搜索命中行 标题（「终端」→ 命令终端）+ 组标签收空隐藏、返回应用后对话视图状态 完好。

| 功能 | 状态 | 说明 |
|---|---|---|
| 设置索引唯一真源 | ✅ v2.40 | lib/settings-index.ts：11 个实装分区 → 4 组（偏好/智能体/能力与安全/系统）声明式索引 { id, group, labelKey, icon, searchKeys }；导航轨、⌘K、SettingsPanel 三处同吃一份——"新增分区忘了加搜索/面板"结构性不可能（Record<SettingsSectionId, Component> 类型保证）；「对话」「关于与更新」未实装不发明占位，随各自批次补条目 |
| 设置导航轨 | ✅ v2.40 | shell/SettingsNav.vue：275px --lw-sidenav-w + bg-side + 右 1px line；顶部 46px 带 = 拖拽区 + 圆角全搜索框（§3.32 头 = 导航轨顶部）；组标签 11.5px ink-4、项 28px/圆角 8/缩进 12px/hover wash-2/选中 wash-3；搜索匹配分区名 + 分区内行标题（searchKeys 预声明，索引驱动不爬 DOM），组标签随命中收空隐藏 + 「没有匹配的设置」空态；底部固定「‹ 返回应用」 |
| 设置整页组装 | ✅ v2.40 | settings/SettingsPage.vue：导航轨 + 内容区（56px 页头显示当前分区标题 20px/600 + 760 正文列居中 + 分区间 gap 32px）；当前分区由滚动侦测驱动（标题越线换高亮）；SettingsPanel 改按索引渲染全分区堆叠 + data-settings-section 锚；过渡期页头「back」钮退役（DestinationShell back prop 删除） |
| ⌘K 命令面板 | ✅ v2.40 | shell/CommandPalette.vue：Ctrl/⌘+K 全局开关（Esc 关、↑↓ 移动、Enter 执行、mouseenter 跟随、scrollIntoView nearest）；居中弹层 640px/圆角 18/elevated/dialog 阴影/52px 搜索行 + Esc 键帽；四组 = 会话（相对时间 hint）/ 页面 / 设置（「设置 / 分区」hint）/ 命令（新会话、折叠/展开侧栏）；空 query 全量、过滤后高亮回零、组空隐藏、无结果空态 |
| ⌘K → 设置跳转 | ✅ v2.40 | shell store settingsFocus {section, nonce} 载荷：命中设置分区 → 切设置目的地 + SettingsPage 滚到该分区 + 锚点闪烁（lw-settings-flash 一次性 wash-3 渐隐，§3.15"闪一下"；重触发用 reflow 复位）；已在本页/跨目的地两路都接（watcher + onMounted 补偿） |
| i18n | ✅ v2.40 | zh/en 增 settings.nav/groups/navSearch/navEmpty + palette.\* 五键组；icons 补 globe 字形（sprite 逐字） |

### ㉔·fix 设置页改分类板面切换（✅ 已完成，v2.41）

> 用户 2026-10-02 裁定：设置页不许是"一整条长滚动页 + 左栏名义跳转"，必须 **拆分到每个分类每个板面**。UI shell 文档 §3.32 先行改版（v3.4 板面切换 模型）：右栏同一时刻只渲染当前分区组件，滚动侦测 / 锚点闪烁 / 分区间 gap 随之退役；⌘K 命中 = 直接切板面。实机走查（tauri dev + 截图）：轨 点击切板面 + 页头换标题、⌘K「嵌入」直达设置分区板面、轨搜索「终端」 命中切板面、返回应用对话状态完好。

| 功能 | 状态 | 说明 |
|---|---|---|
| 板面切换模型 | ✅ v2.41 | SettingsPage 重写：右栏一次只渲染 shell.settingsSection 指向的分区组件（Record 映射随迁，SettingsPanel 堆叠容器删除）；当前分区放壳 store（重开设置保持上次板面），轨点击 / ⌘K / 轨搜索命中都改写同一份状态，组件纯响应式——滚动定位、scroll-spy、settings-flash 锚点闪烁、settingsFocus nonce 载荷全部退役 |
| ⌘K 入口收敛 | ✅ v2.41 | shell store openSettings(section?) 单一入口（CommandPalette 改调）；同分区连跳 = 幂等无操作（板面已在该处） |
| 文档 | ✅ v2.41 | UI shell v3.4：§3.32 头部增"板面切换模型"段、三/七两条随改、§3.15"闪一下"改板面切换 |

### ㉕ UI 设计落地④：对话主区（✅ 已完成，v2.42）

> §2.7 列宽三轴双柄 / §3.7–3.9 过程组·工具行·思考行 / §3.11 composer 坞 / §3.12 用量浮层 / §6.5 markdown 兑现；搭车落 §3.2 `ChatSection`（settings `chat` 区，§8.2 预留项）。实机走查（tauri dev + 截图 + settings.json 落盘 取证）：markdown 转录 / 过程组折叠展开 / 工具行中文动作与加工摘要 / 明细块 / 实时轮（用户气泡 + 流式 markdown + 用量环）/ 列宽柄（键盘 ±16 / 拖拽实时缩放 + 双柄对称 + composer 同步 + 落盘 / 双击复位 760 / 热区下沿停在 composer 顶）/ 设置「对话」分区四行。走查中修掉两个真实 缺陷：① i18n 占位文案裸 `@` 触发 vue-i18n linked-format 编译崩溃（整页 白屏，`{'@'}` 转义修复）；② 列宽拖拽监听元素级在 capture 失配时随指针 出热区即死（改 window 级监听兜底）。

| 功能 | 状态 | 说明 |
|---|---|---|
| settings chat 区（后端） | ✅ v2.42 | settings/chat.rs：contentMaxWidth（钳 560–4000）/ expandToolProcess（缺省关）/ enterToSend（缺省开）/ usageReadout（used/remaining）；字段级缺省零迁移；update.rs 校验 + 命令接线 + 测试 3 例（全桌面 93 绿）；前端 types/api/store saveChat + ChatSection 分区（settings-index 新增「对话」条目 + chat 字形） |
| 列宽三轴双柄 §2.7 | ✅ v2.42 | ConversationWidthHandles：偏好值 settings 唯一真源（chat.contentMaxWidth），实战宽 = clamp(偏好, 560, 主区宽−48) 写到对话区根 --lw-content-max——转录列/composer/消息正文三处同吃一个变量；双柄中心对称、指针 1px ⇒ 2px；12px 隐形热区（容器 pointer-events:none，下沿 = composer 顶）；2×40 竖条 hover 淡入、拖拽 56px 加深（token conv-handle 暗 18%/亮 12%）；拖拽零延迟（data-conv-resizing 全局锁）+ rAF、程序化改宽走 conv-anim 滑动；键盘 ←/→ 16（Shift 32）方向按 data-side、Home 复位/End 最宽/Esc 取消不落盘/双击复位 760；pointerup 落盘 |
| 轮次过程组 §3.7 | ✅ v2.42 | TurnProcess.vue：头行 = sparkle + 处理中/已完成 + · 时长（实时轮每秒走字；历史重放无时间戳不虚构）+ · N 个工具（失败 ⓘ N 个问题）+ chevron 只换字形；hover 溢出 3px；展开体左缩进 + 1px 引导线，只含过程项（工具/思考）；连续过程项合成组（ChatTranscript 分组 walk），系统通知/助手正文/错误行是组外兄弟；折叠本地态、默认一律收起（含运行中）；展开/收起 = §6.6 ④ grid-rows 0fr→1fr + 内容 40ms 延后 + 结束摘 overflow |
| 工具行 §3.8 | ✅ v2.42 | ToolRow.vue 替代 ToolCallCard（卡降级为行，去边框 wash 分层）：折叠单行 = 类目字形 + 中文动作名（§3.18 映射表 i18n 化，mcp\_\* 动态拼参；表外回落工具 id + 「插件」徽章）+ 加工参数摘要（lib/tool-summary.ts：命令类剥 cd/bash -lc 只留首命令 + … +N、文件类工作区相对路径、对象类取 prompt/query/url/key 键值不带括号引号、兜底 clip48）+ 写/编辑类变更统计（输出字段尽力提取，取不到不渲染不虚构）+ 状态点（ok 绿点/运行中脉动/失败 danger）；hover 复制调用按钮；展开态 bg-inset 圆角 10 参数/结果原样（失败 error danger 色）；失败自动展开一次（实时与重放同规）；「默认展开工具执行过程」设置只吃实时新发轮次（ts 存在），重放一律折叠 |
| 思考行 §3.9 | ✅ v2.42 | ThinkingRow.vue：sparkle + 「思考」+ 摘要单行截断（剥 markdown 记号与换行取首句）+ 展开原文 pre-wrap（wash-1 圆角 10、12.5px/1.55）；store 加 thinking 条目类型——数据源（推理流事件）待后端（§8.2 已记），不发明占位事件，通道有了即为消费端 |
| composer 坞 §3.11 | ✅ v2.42 | Composer.vue 替代 PromptBar：浮起卡片（r-xl、bg-composer、shadow-composer、无边框、min(100%, var(--lw-content-max)) 与转录列同源不写死）；自适应文本域 1 行起 max-h 180、占位「输入 / 使用命令 · {'@'} 引用文件」；工具条左 = 模型 chip（状态显示，§3.30 选择器前无菜单）；右 = 用量环（dasharray 按 used/window，点开 §3.12 浮层）+ 36px 发送键（空禁用灰/运行中换停止字形禁用——轮次中止后端 §8.2 待接，不假装可点）；Enter 发送（enterToSend 开关，关 = Ctrl+Enter；IME 组合中不发送）/ Shift+Enter 换行；附件 + / 权限 chip / 提示词增强随后端批次（§8.2 已记，不发明占位） |
| 用量浮层 §3.12 | ✅ v2.42 | ContextUsagePopover.vue：环上方右对齐 360px elevated + 1px line（弹层语法）；headline 口径跟 usageReadout（已用/剩余）；窗口行 used/window + %；分项（本轮合计/速度/模型用量/工具上下文）待后端载荷扩展（§8.2 既有行） |
| markdown §6.5 | ✅ v2.42 | lib/markdown.ts（marked + DOMPurify + 代码块增强：语言标签头行 + 复制钮，figure.code-block 形态）+ style.css 全局 .md 规则（段落/列表/行内代码/代码块 bg-inset 圆角 12 12.5px mono/表格满列宽 wash-1 表头 1px 行线无外框/引用）；助手正文整列宽不再是气泡；消息行动作区（复制 + 时间 HH:MM hover 显形，历史重放无时间戳不显示）；纯文本 pre-wrap 渲染缺口补齐 |
| 走查修复 | ✅ v2.42 | ① vue-i18n 占位文案裸 @ = linked 语法 → 消息编译异常整页白屏（新增文案带 @/\` |

### ㉕·fix 设置页行版式：左设置名、右设置参数/按钮（✅ 已完成，v2.43）

> 用户 2026-10-02 裁定：设置行 = 左行名、右控件。批首按纪律先改 UI shell 文档 §3.32·七（v3.5 行版式规范，参考对象 `.settings-row`/`.settings-toggle` 逐值照搬） 再动码。实机走查（tauri dev + WebView2 CDP 截图，亮/暗双主题）：12 个分区全部 行式落位、开关翻转与 aria、行名 label 聚焦控件、暗色瓦片/开关 token 正确。

| 功能 | 状态 | 说明 |
|---|---|---|
| 行模式组件 | ✅ v2.43 | SettingsRow.vue（行 = 瓦片卡：wash-1 圆角 14、内边距 14/16、左行名 13px/500 + hint + #detail 动态取值、右控件列右对齐 max-w 240 min-h 38、controlId 连 label、wide 逃生口）+ SettingsGroup.vue（13px/600 小标题分块 + #action 块级动作 + 行间 gap 6px 代发丝线）+ SettingsSaveRow.vue（保存行左对齐 + 已保存闪烁；错误条收敛到板面级一处） |
| SettingsToggle 开关 | ✅ v2.43 | 32×20 全圆角轨道 + 16px 滑钮（on = translateX(12px)）；新 token --lw-switch-track(-hover)/--lw-switch-knob（暗 = 白 10%/18% + gray-300 钮，亮 = 近黑 10%/18% + 白钮；on 轨 = accent、on 钮 = accent-ink 不设独立 token）+ tailwind switch 角色组；role="switch" + aria-checked；:enabled:hover 禁用态 hover 不变色；设置行内布尔项一律开关、复选框退场（§3.32·七 成规） |
| 12 分区改行式 | ✅ v2.43 | 外观/语言（就地生效无保存钮）/对话（宽度数字 + 两开关 + 读数分段）/上下文（滑条 wide、当前值·建议·警告走 #detail；策略竖排 radio → 下拉）/快捷键/记忆/插件/命令终端（开关 + 行外警告条与重启提示）/模型/嵌入（文本输入 min-140 自适应、检测行 #detail 报维度或错误）/工具与暴露（三分块：截断行组、fsRoots 行内移除、快照整块）/会话与数据（只读路径整块）；卡内与页头重复的分区 h3 退役 |
| i18n 修正 | ✅ v2.43 | 新键 embeddingProbeTitle / fsRootsRemove（zh/en）；陈旧文案纠偏：sessionsMemory「会话保存在内存中重启清空」不符 ⑲ 事实 → 「逐事件落盘 sessions/ 重启恢复」；走查修真实缺陷：shellHint 裸 \| 被 vue-i18n 当复数分隔符、面板只显示"log"（{'\|'} 转义；§七规则 11 扩为 @/\|/{}/保留字符必须转义——㉕ @ 白屏同源第二例） |

### ㉕·微调① 设置页目检：两色控件体系 + 启动白屏核查（✅ 已完成，v2.44）

> 用户 2026-10-03 目检 11 条裁定（同日起进入"一页完全调完再到下一页"节奏，设置页为 第一页）。批首先改 UI shell 文档 v3.6（§3.31 导航轨 / §3.32·七 两色控件体系 / §6.1 阴影例外 / §6.4 `--lw-settings-max` / §7 规则 12 字段类焦点环豁免）再动码。 模型（llm）分区按裁定豁免，待后期单独大改版。实机走查（WebView2 CDP）逐条取证： 行高 30/间隙 2px/搜索胶囊白底距顶 50/返回应用 84px fit-content、列宽 995→窄窗 615 无横溢、分段选中灰片、下拉白面板浅高亮、嵌入输入聚焦无环、明暗两主题自洽。

| 功能 | 状态 | 说明 |
|---|---|---|
| 导航轨（①②③④） | ✅ v2.44 | 项行高 28→30（+2px）+ 项间 2px 间隙（高亮瓦片不粘连，组内 flex-col gap）；搜索胶囊移出 46px 带（带退化为纯拖拽区）自占一行、底 --lw-bg 白 + shadow-raised；「‹ 返回应用」宽度按字符内容收缩（sidenav-item--fit fit-content，不做整行瓦片） |
| 正文列（⑤） | ✅ v2.44 | 新量纲 token --lw-settings-max: 1024px（760 加宽 +34.8% ≥ 1/3）；w-full max-w 封顶居中，窗口变窄随窗收缩（CDP 实测 1280→995px、900→615px、无横向溢出） |
| 分段控件（⑥） | ✅ v2.44 | 新组件 SettingsSegmented.vue（radiogroup）：容器 = 白底台面（raised + shadow-raised）圆角 full + p-1；选中片 = --lw-bg-side（左轨的灰）；外观主题三档 / 对话用量读数二档同批换装 |
| 下拉（⑦） | ✅ v2.44 | 新组件 SettingsSelect.vue 替代原生 select（语言 / 压缩策略；模型页豁免）：触发器白底台面无点击环；弹出面板 teleport 到 body——白底（raised）+ 1px line + 圆角 14 + shadow-raised；选项 hover wash-2、选中 wash-3（与导航轨统一，吃掉系统蓝高亮）+ 勾标；多类别组间 1px 发丝线 + 11px 组名；↑↓/Home/End/Enter/Esc 键盘全套 + aria-activedescendant；语言 中文↔English 实测互切 |
| 快捷键可自定义（⑧） | ✅ v2.44 | 新组件 SettingsKeyRecorder.vue 录制式改键：点键位钮进录制态 → e.code 归一化（免 Shift 符号干扰）→ Ctrl/Alt/Shift/Meta+主键 串走既有 saveShortcut 即时重注册；纯修饰键等待、Esc 取消、无修饰键且非 F 键拒收；组头右侧「恢复默认」（回落 Ctrl+Shift+L）；CDP 真键序录制 Ctrl+Shift+K settings.json 落盘取证 + 恢复默认回 Ctrl+Shift+L |
| 控件统一（⑩） | ✅ v2.44 | .lw-field / .lw-btn 两个全局公共类（style.css）：raised 底 + shadow-raised 微悬浮 + 无边框 + hover --lw-ctl-hover（新 token 双形双主题：亮 #f3f3f3 / 暗 #303030）；字段类聚焦不出环（§7 规则 12 用户裁定豁免；按钮键盘 :focus-visible 环保留）；对话宽度数字框、记忆/插件/开发者数值框、嵌入三输入、检测/刷新/添加/清除 Key/保存按钮全量换装 |
| 说明一行封顶（⑨） | ✅ v2.44 | 组说明与行说明超一行文案全量精简（zh/en）：chatWidth / shell（顺带褪掉 {'\|'} 转义）/ memoryAutoExtract / embedding / autoDisableDays / shortcut 六处；新键 shortcutRowHint / shortcutRecording / shortcutRestore，删 shortcutPlaceholder |
| 启动白屏核查（用户追加） | ✅ v2.44 | 根因：窗口默认 visible:true，Tauri 在 setup 前即建窗显示——WebView2 初始化 + 前端加载全程露白（后端 init_state 实测仅 \~40ms 不在华点）；修复 = 窗口 visible:false + backgroundColor 起步，前端 mounted+2×rAF 首帧后 show()+setFocus()（capabilities +allow-show/allow-set-focus），Rust 侧 8s 兜底显窗（前端崩溃也不吞窗口）；lib.rs 加 boot 计时日志（builder→setup 668ms→init_state 40ms），前端热启动首帧实测 432ms |

### ㉕·微调② 设置页目检第 2 轮 + 冷启动根因与闪屏（✅ 已完成，v2.45）

> 用户 2026-10-03 目检第 2 轮 8 条 + 追问 3 条 + 弹出面板 2 条。冷启动二次核查 （用户追问"是不是加载插件"）：**插件无辜**——init_state 含 10 插件装载实测 37–51ms；真凶 = 本机 IPv4 回环每个 TCP 连接被过滤驱动拖 \~2s（curl 实测 127.0.0.1 = 2.02s vs ::1 = 0.02s；机器常驻 clash-verge/mihomo 疑 TUN 钩子） × vite dev 136 模块请求（resource timing：main.ts 单请求卡 25s、71 请求
>
> > 200ms）→ FCP 31.4–35.5s。批首按纪律先改 UI shell 文档 v3.7 再动码。

| 功能 | 状态 | 说明 |
|---|---|---|
| ① 参照字体档案 | ✅ v2.45 | ZCode 源码实测（F:\\Warehouse\\Github\\ZCode，Explore 代理）：无 webfont、Tailwind v4 缺省 sans 栈（Win = Segoe UI / YaHei）、基准 14px 可调 12–20、导航行 14px/400 高 32 圆角 12、组标签 12px/500 前景 40%、图标 lucide 16px stroke-2 全前景；档案入 UI shell 文档 §3.32·七，是否对齐待用户裁定（Locowrk 现状 13px/30px/无图标） |
| ② 分段滑动游标 | ✅ v2.45 | SettingsSegmented 选中片改绝对定位 --lw-bg-side 游标（offsetLeft/Width 挂载/换值/换文案/字体就绪四时机校准，首测显形前一帧防开场滑入）+ .lw-seg-thumb 150ms ease-out；主题档「跟随系统」→「系统」；动效开关 = Win11 设置 > 辅助功能 > 视觉效果 > 动画效果（WebView2 读此项，不读老"性能选项"对话框；用户开启后实机确认动效正常） |
| ③ 数字输入单参 | ✅ v2.45 | .lw-field-num（style.css 一处定义 宽 140 + 高 28，用户裁定"统一 = 一个参数值"）——对话宽度/每轮注入条数/Top-N/调用超时/闲置天数五处接线，CDP 实测双点 140×28 一致 |
| ④⑤ 快捷键定宽 | ✅ v2.45 | 录制钮 160px 内容居中（键位长短不一按钮恒定）；「恢复默认」组头 #action 与块说明同一行 + undo 图标 + mr-3 补齐组头 px-1 → 右缘与录制钮对齐（实测同行、rightDelta=0、160/160） |
| ⑥ 滑块减淡 | ✅ v2.45 | 新 token --lw-slider-accent（亮 #a3a3a3 / 暗 #b0b0b0，双形态）替代 accent 直用（accent-color 实测 rgb(163,163,163)） |
| ⑦ 下拉说明行 | ✅ v2.45 | SelectFlat.desc：触发器只显主名，括号说明进面板 11px ink-3；带说明面板 minWidth 224 + 右缘对齐触发器停靠（实测 rightDelta=0，左缘视口钳制）+ 打开不预置高亮（activeIndex=-1，hover wash-2 / 键盘首按落选中项才亮）——弹出面板两条用户裁定 |
| ⑧ 警示气泡 | ✅ v2.45 | 新 HoverTip.vue（warn ⚠ 13px + hover 白底气泡 raised+shadow-raised+max-w 280 承载解锁后果与重启提示，纯 CSS hover 显隐实测 visible→hidden）；行下常驻警告条退役（§3.32·七 v3.7 成规） |
| 组头动作钮 | ✅ v2.45 | 添加目录…/刷新从 11px 小钮提到标准 28px 高（px-3 py-1.5 text-xs，与行内控件同规格；用户裁定"按钮怎么这么小"） |
| 闪屏 + 兜底 | ✅ v2.45 | 新 public/splash.html 静态闪屏窗（零脚本内联样式，配置窗口序第一先建先画）+ lib.rs listen("lw:frontend-ready")（前端首帧 show 后 emit）关闪屏 + 兜底 8s→40s（实测健康冷启动 31.4s + 余量，主窗 hidden 直至首帧）；闪屏截图 + body 文本取证，关闭走事件路径（无兜底警告） |
| 冷启动根因修复 | ✅ v2.45 | vite.config.ts server.host: "::1" 只绑 IPv6 回环整体绕开被过滤的 IPv4——稳态 FCP 31,400–35,500ms → 108–140ms（120 请求零慢请求、最慢单请求 26ms）；代价 = vite 重启后首回依赖预构建 \~18s 一次性（闪屏兜住）；生产构建无 vite 不受影响 |

验证：vue-tsc+vite 构建过、cargo test/fmt/clippy 绿（[lib.rs](http://lib.rs) 闪屏接线后全量重编）、 check_length OK；CDP 走查逐项数值取证（见各行）+ 亮暗两主题截图 + 闪屏截图。

> **设置页第 1 页目检收口**（用户 2026-10-03 确认"基本都 OK"）。唯一遗留 = 模型 （llm）分区豁免——后期单独大改版，**含后端逻辑**（用户裁定），随未来批次另立。

### ㉕·微调③ 单轮步数上限可配置 + 面板更名「工具与分发」（✅ 已完成，v2.46）

> 冒烟测试第 6 轮复盘引出（用户 2026-10-02 追加）：桌面 runner 单轮 step 兜底 原硬编码 MAX_STEPS=8（超限报错"超过单轮步数上限（8 步），强制收束"）， 上限收进设置面板可自调；落点用户裁定**工具与分发面板**（settings `tools` 区，不进对话分区）。UI shell 文档 §3.2 面板表先行补行再动码。

| 功能 | 状态 | 说明 |
|---|---|---|
| settings 字段 | ✅ v2.46 | tools.maxTurnSteps（u32 缺省 8 = 原 MAX_STEPS，旧文件字段级缺省零迁移）；区间 1–100（0 等于拒绝任何工具轮、上限防失控烧 token；子代理 max_turns 上限 32 是另一套旋钮互不影响），出界拒绝且不污染内存态 |
| runner 参数化 | ✅ v2.46 | run_turn 增 max_steps 参数（MAX_STEPS→DEFAULT_MAX_STEPS 只作缺省与测试值）；两消费端 agent_chat + 调度引擎 fire 每轮开跑各取一次——保存即对下一轮生效（含调度触发轮）；报错文案带配置值并指引「设置 → 工具与分发」；循环 1..=max_steps.max(1) 防 0 值空转 |
| 面板行 | ✅ v2.46 | DeveloperSection「单轮步数上限」数字行（.lw-field-num 140×28 同源，min1/max100）+ zh/en 文案 + ⌘K/轨搜索键 maxTurnSteps；与 Top-N/调用超时同一保存行 |
| 搭车：面板更名与文案 | ✅ v2.46 | 用户裁定「暴露」难听——设置页展示名「工具与暴露」→「工具与分发」（导航/hint/「最近一次分发分布」/「本轮分发 N / M 个工具」/「全量分发」随改；机制名 exposure 与 en 文案不变）；「节省上下文：约 ≈2.9K token」去重留「≈」（模板去掉「约」——compactNumber 小数档自带 ≈、整数档精确显示，§3.5 共享实现不动） |
| 测试 | ✅ v2.46 | +1：step_cap_force_closes_with_configured_limit（上限 1 未收敛即 Err 带"1 步"、失败约定不发 TurnDone）；settings 测试扩缺省 8 / 更新 24 往返 / 0 与 101 拒绝 |

验证：vue-tsc+vite 构建过、cargo test 94 绿（+1）、clippy 0、fmt、check_length OK。

### ㉕·fix 历史会话用户消息重放丢失（✅ 已完成，v2.47）

> 用户实机报障（2026-10-03）：软件重启后历史会话只剩模型回复，用户发的 每一轮都不见。根因 = ⑲ 起 `project_events` 漏投影 `UserMessage`（落 `\\\_` 通配被丢）——**落盘一直有**（jsonl 镜像含用户消息，恢复逐条一致 测试早已锁住），实时路径也一直正常（前端发送即本地回显）；丢的只是 `session_history` 全量投影这一层。UI shell 文档 §3.6/§4.1 先行补记再动码。

| 功能 | 状态 | 说明 |
|---|---|---|
| 投影补 UserMessage | ✅ v2.47 | project_events 新分支 → AgentEvent::UserMessage{session_id,content}；实时轮不受影响（run_turn 投影起点在 UserMessage 追加之后，该段只落盘不上通道）——双气泡即回归 |
| 通道定位 | ✅ v2.47 | agent://user-message 通道永不 emit、前端有意不订阅（api.ts CHANNELS 不含；variant 只为 session_history 返回形状服务，channel() 穷尽性所需） |
| 前端重放 | ✅ v2.47 | types.ts UserMessageEvent + chat store onEvent user_message 分支（重放不记时间戳，同 §3.7 时序纪律） |
| 测试 | ✅ v2.47 | +1：history_projection_includes_user_messages（落盘→启动恢复→全量投影首条=用户消息原样；实时 Collector 不含 UserMessage）；settings 测试补重开证据（drop 后重开同一数据根，maxTurnSteps=24 等逐字段读回——步数上限永久保存的硬证明） |

验证：vue-tsc+vite 构建过、cargo test 95 绿（+1）、clippy 0、fmt、check_length OK。

### ㉕·对话区① 首屏空态 + §3.17 审批决策卡（✅ 已完成，v2.48）

> 用户裁定（2026-10-02）：对话区按原型 HTML + UI shell 文档**先都完成再细节微调**， 从首屏开始；原型/文档不精确 1:1，明显不合理处自行权衡（不一根筋照搬）； **后端为前端服务——冲突时改后端适配前端**（硬边界，重申）。设置页抛光确立的 细节规范**全局适用**（§七 规则 15 落档）。UI shell 文档 §3.6/§3.17/§七 先行补记再动码。

| 功能 | 状态 | 说明 |
|---|---|---|
| 首屏空态 S1 | ✅ v2.48 | 启动落首页（用户裁定，⑲"重启接最近会话"在 UI 层退役）：chat.init() 不再自动 switchSession，列表照常拉；空转录 = §3.14 首页——单线云朵吉祥物 72px + 26px/400/-0.02em 问句垂直居中，composer 原位，顶栏标题空；⌘K/侧栏点开即进会话 |
| §3.17 内联决策卡 | ✅ v2.48 | chat/PermissionCard.vue 六段卡紧跟宿主工具行渲染（ChatTranscript 按 request_id==call_id 注入；无宿主行兜底挂转录尾）：头行（需要权限+排队数+子 Agent 标识+风险胶囊）/ 提问行（§3.18 中文动作名——原型裸工具 id 与纪律冲突，以文档为准）/ 理由行（ToolSpec.description 进载荷）/ 参数区（顶层键值行、bg-inset、max-h 240、复制）/ 环境行（fsRoots\[0\] 或 临时会话）/ 动作行（秒数倒计显示 + 三键，主键=允许一次）；风险染底 side_effect 映射缺省按高；决策后焦点回 composer；ApprovalModal 退役删除 |
| 后端三件（改后端适配前端） | ✅ v2.48 | ①「允许本次对话」= ApprovalHub 进程内豁免集 (session_id, tool) 命中即 resolve 直接放行不再问（Kernel 照写 ApprovalDecided 逐条留审计）；②超时守夜：请求置位即 spawn 计时（缺省 100s，测试可注入），抢到移除权 = 未响应 → fail-closed 拒绝（§8.3-5 阈值待裁定）；③载荷 +description；agent_approve +scope |
| 决策记录条投影 | ✅ v2.48 | ApprovalDecided 此前同样落 \_ 通配——project_events 新分支 → agent://approval-resolved（tool 从同切片 ToolCall 按 call_id 找回），实时增量与历史重放同源，"谁批过什么"重启后还在；chat store 消费推 SystemNotice（已允许/已拒绝 · 动作名），approvals store 同通道摘待决卡（超时路径也有兜底）；被拒调用照旧以失败 ToolResult 闭合（失败工具行自动展开） |
| 搭车 | ✅ v2.48 | ToolRow 状态词「等待审批」（审批未决 ≠ 执行中）；侧栏当前会话行待决 danger 点（§3.17 卡不在视野的兜底）；§七 规则 15（设置页细节规范全局适用：2px 行间隙/字号阶梯/明暗双形态/浮层间距，同一特性全局一套实现） |
| 测试 | ✅ v2.48 | +2：session_scope_exempts_same_tool_next_call（scope=session 放行 → 同会话同工具不再推请求、其他会话照问、豁免放行逐条落 ApprovalDecided）、timeout_guard_denies_when_unanswered（短超时守夜 → fail-closed 拒绝落日志 + 失败 ToolResult 闭合）；denied 路径投影断言更新（tool_call → approval_resolved → tool_result） |

验证：vue-tsc+vite 构建过、cargo test 97 绿（+2）、clippy 0、fmt、check_length OK。 实机走查（tauri dev + CDP 截图）：首屏空态（吉祥物+问句+composer 原位+顶栏空标题）； 真实轮审批链全通——拒绝 →「已拒绝 · 派生」记录条+子 Agent 收束通知+模型如实报告； 100s 守夜超时拒绝**实机真实发生**（fail-closed 理由落转录）；切会话再切回重放完整 恢复用户气泡+记录条（㉕·fix 直接受益证据）；豁免按会话隔离（parent 豁免 subagent_run、 子 session 的 shell 照问）。缓办：§3.30 模型选择器（settings 只存单 provider+model， 无模型清单数据源——`llm_list_models` 后端立起前不发明空壳选型器）。

### ㉖·对话区② 会话运营 + 开发者选项地基（✅ 已完成，v2.49）

> 用户三条新裁定（2026-10-03）落档：①\*\*[DESIGN.md](http://DESIGN.md) 不再堆补记\*\*——逐批落点补记迁至 [CHANGELOG.md](CHANGELOG.md)（本批起生效，本条即其首条）；②**可配置原则**——不确定 数值一律进设置页可调项；界面上写死的禁止项都要有开关（含最危险动作），统一收默认 隐藏的「开发者」分区（安卓式连点版本号 7 次解锁）→ UI shell 文档 §七 规则 16 / §3.2 先行成文再动码。自规划批次 = ㉖ 会话运营（§3.6 行尾菜单四件，后端命令）+ 开发者选项 地基；搭车 = 审批超时可配（§8.3-5 就此裁定闭账）。

| 功能 | 状态 | 说明 |
|---|---|---|
| 重命名 | ✅ v2.49 | 行内编辑（Enter/失焦提交、Esc 取消、空串不动）；标题存 <id>.meta.json 旁车（session-store persist 新 write_title/read_meta，列表覆盖自动标题；文件留 workspace 字段给 §2.3 关联批次）；session-store 测试 +2 |
| 删除 | ✅ v2.49 | 菜单内两步确认（首点变「确认删除？再点一次执行」，keepOpen 菜单不收）；后端 session_delete = 日志+旁车+undo 备份一并移除（幂等）+ 忙锁守卫（try_lock 失败 = 运行中默认拒绝，developer.allowDeleteRunningSession 后端直读设置放行强制）+ 内存条目与调度 last_chat 指针剔除（防僵尸注入轮） |
| 导出 | ✅ v2.49 | pick_save_path（原生存储对话框，§七 规则 8 同族）+ session_export = Markdown 转录（export.rs：用户/助手标题段 + 工具行/审批记录/压缩引用行，参数与长输出截 400 字标注）；数据 = runner::project_events 同一投影（内存优先缺则回读磁盘）；渲染测试 +3 |
| 搜索 | ✅ v2.49 | 侧栏搜索框 = 标题前端过滤照旧 + 「正文命中」第二段：session_search 大小写不敏感子串扫 UserMessage/AssistantMessage（逐字符折叠记回原字节位，to_lowercase 变长也不切片错位），摘录 ±24 字符 + 命中数，mtime 降序限量 20（后端缺省 50 上限 200）；防抖 250ms 序号守卫竞态 |
| 审批超时可配 | ✅ v2.49 | settings tools.approvalTimeoutSecs（缺省 100，0 = 不限，1–86400 校验出界拒绝）；ApprovalHub 超时改 AtomicU64 ms 热换入（set_timeout_secs，settings_set 即生效），0 不起守夜任务；载荷 +timeout_secs → 决策卡倒计时读载荷（不限档不渲染行）；测试 +2（settings 三档往返+出界 / 不限档 300ms 不超时+手动收束+载荷断言） |
| 开发者选项地基 | ✅ v2.49 | settings.json 新 developer 区（unlocked + 两危险开关，缺省全关零迁移）；默认隐藏分区：索引 devOnly 标记，导航轨/⌘K/面板三处同过滤（面板持久化落在隐藏分区时回落外观）；解锁 = 侧栏版本号连点 7 次（2s 重计，第 3 次起 toast 进度同 key 原位替换），分区内「隐藏开发者选项」锁回；禁止项开关：允许删除/切走运行中会话（前端守卫解锁，后端忙锁仍兜底）；⑫ 暴露调试面板自工具与分发迁入；DeveloperSection 组件名归位（原 DeveloperSection 更名 ToolsSection——分区 id 本就是 tools） |
| 新基础组件 | ✅ v2.49 | shell/RowMenu.vue（§3.20 形态：184px/圆角 14/danger 项/先测量再显示/视口翻转/↑↓Enter/Esc 还焦点/keepOpen 两步确认）+ shell/ToastStack.vue + stores/toast.ts（§3.22：右下栈同屏 3 条排队、状态点不染底、4s hover 暂停、Esc 关栈顶、key 原位替换）+ shell/SessionRow.vue（行+状态点+菜单+行内重命名，自 Sidebar 迁出保 400 行） |
| 测试 | ✅ v2.49 | 桌面 103 绿（+3：unlimited_timeout_keeps_waiting_for_decision / approval_timeout_configurable / developer_defaults_off_roundtrip_and_persist；既有 32 处 update() 调用补 developer 参数）；session-store 22 绿（+3：meta 覆盖与重置 / delete 幂等全清理 / search 命中摘录限量） |

验证：vue-tsc+vite 构建过、cargo test 103+22 绿、clippy 0（desktop+session-store）、fmt、check_length OK。 实机走查（tauri dev + CDP 截图 26-1…26-11）：行尾菜单三件与分隔线、行内改名落 meta.json （`{"title": "走查改名会话", "workspace": null}` 落盘取证）、正文命中 3 条带摘录、两步 删除（确认文案 → toast「会话已删除」→ jsonl 磁盘消失）、版本号 7 连点（进度 toast「再点 1 次…」→ 成功 toast）→ 导航轨出现「开发者」（危险开关缺省关 + 调试面板迁入）→ 审批 等待时长行改 30 落盘取证 → 锁回后轨内消失 + settings.json `unlocked:false`。走查期间 改动的用户设置已还原（approvalTimeoutSecs=100）。缓办：⌘K 会话组接正文搜索（命令面板 侧消费端，后端命令已就位）；§8.2 会话↔工作区关联（meta 文件已留 `workspace` 字段）。

### ㉖·收尾 会话↔工作区关联（✅ 已完成，v2.50）

> 自规划收尾批：㉖ 排期只剩 §8.2 关联（v2.49 meta 旁车已留 `workspace` 字段）。 UI shell 文档 §2.3/§3.6/§8.2 先行成文再动码。

| 功能 | 状态 | 说明 |
|---|---|---|
| 后端关联 | ✅ v2.50 | session-store persist：write_workspace（通用 write_meta 合并写入——两字段全空才删旁车文件；修 v2.49 真实缺陷：write_title 空标题删整个旁车文件会连带抹掉已关联 workspace，改为只清字段，回归测试锁住）；SessionMeta +workspace 随 session_list 透出；session_associate 命令后端直读设置校验目标必须是当前 fsRoots 之一（不信前端转述），移出 = null；测试 +2（关联往返+清除 / 改名清空保关联） |
| 侧栏两级树 | ✅ v2.50 | lib/session-groups.ts 纯函数（工作区→时间桶两级组装：组名 = 存储路径末段（\\// 通吃；根目录移除后组照常显示不静默丢）、工作区组按组内最近活动降序、runningId 传入算组内运行）+ SessionGroups.vue（顶层「会话」组 + 「项目」组；工作区头 28px = 12px chevron（展开转 90°）+ 14px 文件夹 + 13px 名 + 可选 6px 运行点，hover wash-2；头行可折叠、折叠键 = workspace 存储路径、持久化进侧栏偏好 collapsedGroups（读改写合并，与 shell store 宽/折叠互不覆盖）；嵌套行缩进 56px、桶标签随迁）——Sidebar 分组逻辑迁出保 400 行 |
| 行尾菜单关联项 | ✅ v2.50 | SessionRow … 菜单增「移入工作区」（候选 = 设置 fsRoots，当前归属项置灰 ✓ 前缀）与「移出工作区」（有归属才出现）；toast 反馈 + 失败条；chat.associateSession → sessionAssociate API |
| 测试 | ✅ v2.50 | session-store 24 绿（+2）；桌面 103 绿（命令层薄适配无新增，由 session-store 测试与走查覆盖） |

验证：cargo test 103+24 绿、clippy 0（desktop+session-store）、fmt、check_length OK、pnpm build 过。 实机走查（tauri dev + CDP 截图 26b-1…26b-7 + 落盘取证）：真实轮（deepseek + 8 工具步）建会话 → `…` 菜单四项（重命名/导出/移入「Locowrk」/删除）→ 移入 → 「项目」组 + 工作区头 + 嵌套行 + toast「已移入工作区」，meta.json 落盘 `{"title": null, "workspace": "F:\\\\\\\\…\\\\\\\\Locowrk"}` → 头行折叠（嵌套行消失）→ **Page.reload 后折叠态保持**（localStorage 偏好）→ 移出 → 回「会话」组、 「项目」组消失。搭车：en 补缺的 `settings.nav.developer` 键（zh 有 en 缺，回落键名）。走查 预置的 fsRoots 已还原（fsRoots=\[\]），走查会话经 UI 两步删除；上批遗留的走查会话残留一并清理。

### ㉗·A 通知浮层 + 应用内通知记录（✅ 已完成，v2.51）

> 自规划拆批：㉗ 体量大，拆 A（§3.13 通知）/ B（§2.5 面板 + §3.16 子转录）/ C（§3.28 diff）。 UI shell 文档 §3.13/§4.1/§8.2 先行成文再动码。

| 功能 | 状态 | 说明 |
|---|---|---|
| 后端记录 | ✅ v2.51 | desktop 新 notifications.rs：<数据根>/notifications.json 自持（cap 100 FIFO，坏文件当空表，变更全量重写）；NotificationStore（add/list/mark_all_read/mark_read/clear，list 新→旧，未读由前端计数）；入账收拢在 TauriSink::emit 一处——draft_from_event 纯函数 + call_id→tool 配对表（cap 64）识别五类「错过的事」：ScheduleFired（label=调度名）/ subagent_run 收束成败（ToolCall↔ToolResult 配对）/ ContextCompacted（label=省下 token）/ 审批超时拒绝（ApprovalResolved 未通过且 reason 命中 approval::TIMEOUT_REASON_PREFIX 共享常量，手动拒绝不入账）；ApprovalHub 与调度引擎零改动；入账后 emit lw://notification |
| 命令 | ✅ v2.51 | notification_list / read_all / read / clear 四条薄命令 + manage(Arc<NotificationStore>)（TauriSink 经 try_state 惰性取用，AppState 同持一份） |
| 浮层 | ✅ v2.51 | shell/NotificationPopover.vue：teleport body + fixed 锚侧栏底 bell 上方（380px 可溢出侧栏，§五 popover 家族 elevated+line+shadow）；头 = 通知 + 全部已读 ✓ + 清空 🗑（空态禁用）；轻量两键分段 全部/未读（未读带计数）；行 = 未读 6px accent 点 + 类别标题（i18n 模板，记录只存 kind+参数）+ 正文（数据参数：调度名/会话标题/省下 token/错误摘要）+ 相对时间（§3.5 共享实现）；行点击 = 置读 + 有会话归属则跳转；点外部/Esc 关 |
| 侧栏入口 | ✅ v2.51 | bell 第 5 图标（§2.4，popover 非页面）+ 未读 >0 右上 6px accent 点；开合/锚定抽 use-notification-entry.ts、版本连点解锁抽 use-dev-unlock.ts（Sidebar 保 400 行）；App.vue notifications.init()（load 全量 + 订阅推送） |
| 测试 | ✅ v2.51 | 桌面 106 绿（+3：add/持久 roundtrip/FIFO cap / mark 语义与 clear / 五类提取与配对含负例） |

验证：cargo test 106 绿（+3）、clippy 0、fmt、check_length OK、pnpm build 过。 实机走查（tauri dev + CDP 截图 27a-1…27a-8 + notifications.json 取证）：空态（「没有通知」+ 全读/清空禁用 + 点外部关）→ 预置 `approvalTimeoutSecs=5` 真实轮：shell 审批不响应 → **5s 守夜 fail-closed 拒绝实机发生**（记录条「已拒绝 · 运行」+ 模型如实报告）→ **bell 徽标 点亮** → 浮层行「审批超时，已自动拒绝 / 请用 shell 工具运行…（会话标题回落）/ 3 分钟前 / 未读点 / 未读 · 1」→ 盘上 `{"kind":"approval_timeout","sessionId":"desktop-…","read":false}` → 全部已读（徽标/未读点消）→ 未读分段空态 → 清空（盘上 `\\\[\\\]`）。走查设置已还原 （approvalTimeoutSecs=100），走查会话 UI 两步删除，通知清零。

### ㉗·B 工作面板骨架 + 子 Agent 转录页签（✅ 已完成，v2.52）

> 自规划拆批第二子批。UI shell 文档 §2.5/§3.16/§8.2/§4.2 先行成文再动码。

| 功能 | 状态 | 说明 |
|---|---|---|
| 后端：子会话落盘 | ✅ v2.52 | kernel-agent 新 SubTranscriptSink trait（close(child_id, own_from, &child)，SubagentTool::with_transcript_sink 构造期可选注入——headless 不注入维持纯黑盒，⑮ 语义零改动）；run_child 收束时把子自身段交给宿主（own_from 前移到任务 prompt 之前——prompt 是子的自有内容，落盘/页签以任务开头；对 tools_used/摘要无影响，⑮ 既有用例全绿佐证）；入参 8 个收拢 ChildRunParams（clippy too_many_arguments）；desktop sub_transcript.rs 写 <数据根>/sub-transcripts/<child_id>.jsonl（id 走 session_file 白名单，写前删旧防残留）；不进内存会话表/侧栏列表（restore_all 只扫 sessions/）；子事件零进父日志 |
| 后端：读取 | ✅ v2.52 | subagent_transcript(child_id) 命令 = 盘上 load + 与 session_history 同一 project_events 全量投影（前端数据形状与主转录一致；文件缺席如实 Err）；child_session_id ⑮ 起就在收束 output 里（v2.37 加的），前端入口现成 |
| 前端：面板骨架 | ✅ v2.52 | stores/work-panel.ts（开合/宽度/最大化/页签 = 跨会话壳状态；开合+宽度持久化 work-panel.v1，页签进程内不跨重启）+ WorkPanel.vue（三栏预算 clamp(244, 容器−侧栏−450)，拖宽顶到预算先折叠侧栏（地板硬）；拖柄全规格：setPointerCapture + 全局锁 + 2×32 圆角条 + 键盘 16px/Home/End + 双击复位 360 + aria separator（valuemax=预算动态）；头部 = 页签条可关闭 + + 回启动器 + 最大化/还原 + 关闭；空态 = 启动器清单（§3.14：不是空态文案——当前会话可看的子转录 + 指引行；只列已实装条目，插件贡献槽位随插件批次））+ App.vue 挂主区右缘（只对对话目的地）+ Topbar panel 开关（独立成组，aria-pressed） |
| 前端：转录页签 | ✅ v2.52 | lib/sub-transcript-items.ts（wire 事件 → chat store 同款 ChatItem\[\] 投影）+ SubagentTranscriptTab.vue（复用 MessageRow/ToolRow/SystemNotice 同一套行组件——§3.7–3.9 同一心智模型；近似如实记录：平铺过程行，过程组合成组后续批接入）；页签名 = 子 Agent + prompt 截断（⑮ 无 name 参数不发明）；底部固定只读说明（composer 不渲染，自证只读）；入口 = §3.10 工具行头行「看转录」小钮（subagent_run 收束且带 child_session_id）+ 启动器清单 |
| 修缺陷 | ✅ v2.52 | 走查实机抓到：子任务失败收束（output.status=failed）被通知记成「已完成」——分类只看 ToolResult.ok 没看 output.status；修 draft_from_event 按 status == "completed" 如实分类 + 回归测试 |
| 测试 | ✅ v2.52 | kernel-agent +1（transcript_sink_receives_own_segment：收束恰好回调一次、自身段 = prompt+回复）；桌面 +2（store 写自身段/覆盖重写/非法 id 跳过）+ 通知分类回归；桌面 108 绿、kernel-agent 全绿、clippy 0 双侧、fmt、check_length、pnpm build |

验证：cargo test 108+kernel-agent 全绿（v2.52 行内曾误记 110，v2.53 更正）、clippy 0、fmt、check_length（state/mod.rs 压回 400 整）、pnpm build 过。 实机走查（tauri dev + CDP 截图 27b-1…27b-8 + 落盘取证）：顶栏开关开面板（缺省 360 + 启动器空态

* 点外部关）→ 真实轮派生子 Agent（审批「允许一次」）→ **子会话落盘实机发生** （`sub-transcripts/desktop-…-sub-1.jsonl` 7 事件，首条 = 任务 prompt，父前缀不带；本轮子内 单层硬剔除生效 → 子如实 failed 收束，失败也落盘）→ 启动器出列 → 点开页签（bot 字形 + 截断 标题 + 任务气泡 + 工具行复用 + 只读说明 + 无 composer）→ 键盘调宽 360→392（ArrowLeft 变宽 方向正确）+ 双击复位 → 关页签回启动器 → 关面板/重开（localStorage 持久开合态）→ 通知 `subagent_failed` 分类缺陷当场抓到当场修。走查设置已还原、走查会话/孤儿转录已清、通知清零。

### ㉗·C diff 审阅 + 目的地整版式（✅ 已完成，v2.53）

> 自规划拆批第三子批（收口批）。UI shell 文档 §3.28/§3.29/§3.31/§8.2 先行成文再动码。 本批纯前端（数据源 = `edit_diff` 的 args.patch，零后端改动）。

| 功能 | 状态 | 说明 |
|---|---|---|
| §3.28 审阅页签 | ✅ v2.53 | lib/diff-parse.ts（unified diff 解析：---/+++ 头 + @@ hunk + +/-/空 行，容错优先不抛错；+N −M 统计与 §3.8 同源；collapseContext 折叠上下文 >3 行成中缝）+ ReviewTab.vue（工作面板 rev:<callId> 页签：文件名头 wash-1（左路径 mono 省略 title 全路径 / 右统计）+ 整行染底 ok/danger 8% + 行首 1px 实色条 + 行号 32px 右对齐 + … 展开 N 行 只增不收回 + 只读无 composer）；数据 = edit_diff args.patch 快照进页签（同 callId 重开覆盖为最新态）；入口 = 工具行头行「看 diff」小钮 + 启动器「审阅」组 |
| §3.29 市场卡 | ✅ v2.53 | PluginCard 对齐规范逐值：wash-1 圆角 14、34px 图标块（圆角 8）、标题 14px/500、元信息 11px ink-4（作者 · 版本）、描述 12px 截 3 行（line-clamp）、副作用 chips（复用 §3.4 徽章真实语义）、底部行左 license/origin 右动作区；网格 repeat(auto-fill, minmax(280px,1fr)) gap 12px；数据源差异如实记录：本地市场无安装量/更新时间（不发明），分类 chips 无维度（随分发批次） |
| §3.31 页头核对 | ✅ v2.53 | 导航轨细则（30px 行/2px 间隙/搜索胶囊/fit-content 返回）㉕·微调① 已兑现零改动；页头 56px/actions 插槽已达标——主按钮与 … 菜单留插槽按目的地实装（强加空菜单 = 发明）；计数页签兑现：扩展两页签改「发现 N / 已安装 N」（计数 11px ink-4，选中 wash-3 圆角块） |
| 测试 | ✅ v2.53 | 纯前端批无 Rust 改动；桌面 108 绿、clippy 0、fmt、check_length、pnpm build 过；更正 v2.52 行的桌面测试数笔误（110→108） |

验证：桌面 108 绿、clippy 0、fmt、check_length OK、pnpm build 过。 实机走查（tauri dev + CDP 截图 27c-1…27c-4）：真实轮调 `edit_diff`（可补偿工具直接执行不经审批） → 工具行「看 diff」钮 → 审阅页签（文件名头 +1 −0 / 行号双列 / 上下文原样 / 新增行绿染底+ 行首绿条全对；走查脚本 fsRoots 转义致 diff 实际未落盘——模型如实报告，与产品无关，审阅 数据来自 args.patch 渲染不受影响）→ 扩展目的地：计数页签「发现 1 / 已安装 11」+ 规范卡片 网格（280px 自适应/图标块/line-clamp-3）。走查设置已还原（fsRoots=\[\]）、走查会话与目标 文件已清。

### ㉗·C·fix 全壳走查核对与补欠（✅ 已完成，v2.54）

> 用户裁定「原型不可能一比一，实施要比原型更细」——对照原型 HTML + UI shell 文档逐条核对 实现后的修正批，纯前端。UI shell 文档先行 v3.8（§3.11 勘误 + §8.2 三行缓办）。

| 项 | 状态 | 说明 |
|---|---|---|
| §2.2-5 车道交接 | ✅ v2.54 | WindowControls 透明浮层（去 bg-page/pl-2/红 hover）+ Topbar padding-right 随面板开合让位 + 面板头预留 120px（修双车道空洞与按钮被窗控遮挡） |
| 面板对齐 §6.1/原型 | ✅ v2.54 | bg-side→bg-dock、删平面内边框×2、页签逐值（圆角 10/wash-1 常态/12.5px/200/close 20×20/bot 仅子 Agent 页签）、拖柄 aria vertical、启动器行 hint；WorkPanel 430 行拆出 WorkPanelLauncher.vue（§8.4 规划件） |
| §3.11 v3.8 勘误 | ✅ v2.54 | 模型 chip 移右组（用量环→模型→发送；文档原文与原型冲突以原型为准），左组固定/右组可缩、模型 chip 唯一收缩项；占位符 12.5px ink-5 |
| §3.10 子 Agent 卡 | ✅ v2.54 | SubagentCard.vue（头行+节点图+查看转录；子内步骤数/模型黑盒不发明、重放无时长不虚构）挂 TurnProcess 组尾；formatDuration 入 lib/format |
| §3.4/§3.16 副作用徽章 | ✅ v2.54 | lib/side-effect.ts 映射表自插件源码逐一对出；表外不渲染（宁缺勿假） |
| §3.25 快捷键 | ✅ v2.54 | use-global-shortcuts.ts（B/J/N/,/\[/\]）+ Composer ↑ 召回 / Esc 浮层栈语义 + ⌘K 命令 5 条带键帽（切主题 system 档先解实效档） |
| §3.23 加载态 | ✅ v2.54 | 冷切换 2px 不定段进度轨（reduced-motion 不显示）+ 重放期间 composer 锁 |
| §3.26 选择纪律 | ✅ v2.54 | 全局 user-select:none + 白名单（正文/明细/diff/pre/输入类），白名单内 button 重归不可选 |
| §五/z-index/杂项 | ✅ v2.54 | 批量卸载升级 ConfirmDialog Modal（原型 S12 逐值；顺手修 common.cancel 裸键名）；⌘K 60→40、SettingsSelect 80→60、HoverTip 50→60；遮罩入场 fast；会话行 2px 间隙（§7-15）；ws-head chevron 旋转退役（§6.6 禁令）；侧栏头/页脚 0 8px + gap 2px；闪屏 8s 静态恢复提示 |
| §8.2 三行缓办 | 📝 落档 | 消息编辑重发（rewind 方案，落地前不渲染编辑钮）/ 恢复面动作化 / 定时任务目的地页 |

验证：vue-tsc+vite 过、check_length OK（WorkPanel 拆件后 356 行）；纯前端批无 Rust 改动。 实机走查（CDP）：车道交接三态（面板开/关）、面板底色与无边框、composer 右组、⌘K 键帽、 会话行间隙。

### ㉗ UI 设计落地⑥：工作面板 + 通知（~~§3.13 通知浮层~~ ✅v2.51 / ~~§2.5 骨架 + §3.16 子转录页签~~ ✅v2.52 / ~~§3.28 diff + §3.29 网格 + §3.31 整版式~~ ✅v2.53 / ~~走查核对与补欠~~ ✅v2.54 / ⬜ §2.5 插件贡献槽位——**随插件批次（plugin-api 末尾追加），出 UI 排期；UI shell 文档内设计项就此全部落地**）

---

## 二·五、㉘ 项目级隔离（2026-10-04 用户指令：「分项目 = 项目级上下文、项目级记忆，不能相互串、相互污染」+「借鉴 PI-Desktop 的项目级实现」+「顺带权限审批一起借鉴」）

> 背景：用户指出侧栏「组织工作区」不可见（v2.50 关联功能休眠——fsRoots 未配置时整段消失）， 且归属必须**有语义**：记忆与上下文按项目隔离。UI shell 文档 §2.3 **零改动**（本批全是 "实现向规范对齐" + 用户指令的新后端能力，规格落 Context and [Memory.md](http://Memory.md) §4.6）。

### ㉘·A 项目记忆隔离 + 侧栏项目段常驻（✅ 已完成，v2.55）

| 项 | 状态 | 说明 |
|---|---|---|
| 侧栏项目段常驻 | ✅ v2.55 | groupSessions(+fsRoots)：组集合 = fsRoots ∪ 已关联会话（§2.3 原文「工作区 = fsRoots 里的一个根目录」）；空组只渲染头行、有会话组按最近活动降序在前；fsRoots 全空且无关联时段落隐藏（§2.3 零改动，纯实现对表） |
| FFI 调用期上下文 | ✅ v2.55 | plugin-api HostVtable 末尾追加 session_context 槽（append-only 不破 ABI，⑭ notify 先例；插件提供缓冲、宿主写入 JSON、ERR_BUF_TOO_SMALL/ERR_UNSUPPORTED 语义）+ CallContext 载荷；plugin-host 线程本地调用期上下文（adapter 在 blocking 线程预置、RAII 清理）+ set_session_workspace_resolver 组装点注入（headless 不注入 = 全局语义回落，⑮/headless 零变化） |
| 记忆按项目隔离 | ✅ v2.55 | memory-sqlite 1.3.0：主键 (workspace, key)（同名 key 跨项目并存不覆盖）、全工具面（write/read/list/search/delete/undo）按键位过滤、undo 快照带 workspace 忠实还原原键位、旧库整表迁移归未归属（插件/宿主双侧等价幂等迁移）；kernel MemorySource/ExperienceSource 契约 + RequestPayload.session_id（内核只透传 id，workspace 是宿主概念）；desktop workspace.rs 解析器单一真源（旁车 meta）三处消费（vtable resolver / 注入键位 / 经历键位）+ experiences workspace 列 + extract 打标；管理页透出归属徽章（人的管理面看全量，§4.6） |
| 隔离规格 | ✅ v2.55 | Context and Memory.md §4.6 成文：严格对称无全局层、键位即主键、注入即隔离、工具面同隔离（"模型看不见的数据才是真隔离"）、管理页豁免、PI-Desktop 借鉴映射 |
| 测试 | ✅ v2.55 | kernel-agent 56 绿（payload/契约随改）、memory-sqlite 13 绿（+3：跨项目同名并存/按键位删+undo 原键位还原/旧库迁移）、desktop 112 绿（+4：解析器/键位归一/记忆键位隔离/经历键位隔离）、clippy 0 双侧、fmt、check_length、pnpm build 过、插件产物重建（memory-sqlite 1.3.0） |

验证：实机走查（CDP 28a-1…7 + sqlite 落盘取证）：预置 fsRoots → 侧栏「项目」段空组 `Locowrk` 头行渲染 → 行菜单移入成组 + meta 落盘 → **归属会话真实轮 memory_write**：sqlite 行 `workspace=F:\\\\…\\\\Locowrk` 打标、旧冒烟行迁入 `''` 键位 → **未归属新会话 memory_read 同 key = `{"found":false}`**（模型如实报告"可能在别的项目下写的"）→ 归属会话读回 `found:true` → 记忆页归属徽章「项目 Locowrk」/未归属行无徽章。走查会话两步删除、冒烟会话移出、设置还原（fsRoots=\[\]）、memory.sqlite3 还原迁移前备份（迁移在用户真实数据上留待自然发生）。

### ㉘·B 权限审批借鉴 + 工具围栏（✅ 已完成，v2.56——PI-Desktop 映射落地；插件按项目过滤可见性随插件批次）

| 项 | 状态 | 说明 |
|---|---|---|
| 权限模式四档切换器 | ✅ v2.56 | 会话级四档（计划/询问/自动编辑/完全）挂 composer 左组（UI shell §3.33 新增）；档位写会话旁车 permission_mode（重启保持）+ PermissionModeStore 缓存即时生效（运行中可切）；设置 tools.defaultPermissionMode 只定新会话缺省（工具与分发分区 SettingsSelect）；决策表权威落 DESIGN §7.2 |
| 决策表序 | ✅ v2.56 | 计划硬拒（纵深防御）> 外部路径门 > 模式放行（完全=全放 / 自动编辑=仅 Compensable，reason mode:\* 前端不渲染记录条）> 会话豁免（既有语义）> 决策卡；kernel consult 扩到非 Pure（询问 档 = 写类也要问），无决议者时 Compensable 照旧放行（headless 零变化） |
| 工具围栏跟会话 | ✅ v2.56 | CallContext +fs_roots（归属会话 = \[数据根, 项目工作区\]；未归属不下发回落 env 全局）；plugin-api fsutil::effective_roots 唯一取根入口；tool-fs / tool-shell(路径类) / tool-git 全部改调用期根；desktop ToolSource 会话感知（计划模式 = 目录级围栏：非 Pure 不出目录、resolve 即拒） |
| 外部路径即审批 | ✅ v2.56 | 审批决策表加路径门：shell / sandbox_run 命令文本扫出围栏外 Windows 绝对路径（盘符/UNC、分量边界、大小写归一）→ 强制弹卡且只许一次（连完全档与豁免都不能跳过），载荷 external:true 前端只渲染两键 |
| 审批卡环境行如实 | ✅ v2.56 | 载荷 +workspace（会话真实归属，旁车真源）；§3.17 环境行三级回落：归属 → fsRoots\[0\] → 临时会话；子 Agent 冒泡调用无旁车 = 未归属如实显示（文档边界） |
| 插件版本基准重置 | ✅ v2.56 | 全部插件重置 0.1.0 起步（packaging.md §9.4·补，2026-10-04 用户裁定）：小迭代 +0.0.1、大改版 +0.1.0 经用户确认；tool-echo 双档 0.1.0/0.2.0（demo 升级回退叙事），kernel-bin demo/e2e 夹具随改；锁文件旧 1.x 版本由 resolve_latest 取内置根可用目录天然回退 |
| 测试 | ✅ v2.56 | workspace 36 套件绿（kernel-agent +2：consult 扩围/resolve 失败闭合；subagent 2 例更新到新契约）；桌面 122 绿（+10：permission_mode 存储 3 / 决策表 5 / 外部路径扫描 1 / e2e 随改）；clippy 0 双侧、fmt、check_length、pnpm build 过、插件产物重建（10 插件 0.1.0） |

验证：实机走查（CDP 28b-1…7 + 旁车落盘取证）：切换器四档渲染与当前档勾选 → 档位写旁车 `permissionMode:"ask"` → **计划模式实机**（fs_write 被目录围栏拒收、模型如实报告、search_tools 纯工具照常）→ 环境行如实（归属会话 = 「项目 F:\\…\\Locowrk」；未归属 = 「临时会话」）→ 询问档审批卡三键/中风险胶囊/理由行/倒计时 → **外部路径门实机**（完全档下围栏外命令强制弹卡、只有「拒绝/允许一次」两键、子 Agent 徽章 + 未归属 env 如实）→ 设置页「新会话权限档」下拉行。**走查实抓缺陷已修**：resolve 失败走 `?` 留悬空 tool_calls，下一轮被供应商拒收（"insufficient tool messages"）——内核改为解析失败也以失败 ToolResult 逐调用闭合（纪律 5 另一半），+回归测试；⑮ 两例收束语义随新契约更新（围栏不变：越权工具零执行）。走查会话已删、设置零改动（切换器只写会话旁车）。

### ㉘·B·fix 围栏拒绝自描述 + tool-edit 纳入会话围栏（✅ 已完成，v2.66——用户实机报障「选了工作区 Agent 仍报零个工作区」）

| 项 | 说明 |
|---|---|
| 围栏拒绝自描述 | ✅ v2.66：fsutil ensure_inside（tool-fs / tool-git / tool-edit 共用拒点）与 tool-shell paths.rs cwd 拒绝的报错改为列出本次调用实际生效的围栏根（新 fsutil::display_roots，去 Windows verbatim \\\\?\\ 展示形），并注明「围栏跟会话归属工作区，LOCOWRK_FS_ROOTS 只是未归属回落」——旧文案只教人去设 env，归属会话下是死胡同（诊断取证：audit.jsonl 时间戳证明 15:05 探测发生时会话已归属 F:\\Warehouse\\AAA，围栏 \[数据根, AAA\] 全程正确生效，被拒的 F:\\ / C:\\Users / F:\\projects / 进程 cwd 全在围栏外；「零个工作区」是内嵌 Agent 无法发现围栏所致的误报） |
| tool-edit 补口 | ✅ v2.66：㉘·B 漏网——edit_replace / edit_lines / edit_diff（写入面）此前仍用 env-only allowed_roots()，不随会话围栏收缩（归属会话下可编辑 fsRoots 全局根内任意文件，比会话围栏宽）；三工具改 effective_roots(host)（lib.rs 分发传 self.host），与 tool-fs / tool-git 同语义 |
| 插件版本 | ✅ v2.66：tool-{fs, shell, git, edit} 0.1.0 → 0.1.1（小迭代 +0.0.1，packaging.md §9.4·补）+ build_plugins.sh 重建暂存；其余插件行为不变不动 |
| 测试 | ✅ v2.66：plugin-api 22 + tool-fs 16 + tool-shell 9 + tool-git 10 + tool-edit 5 绿（直呼函数的单测传空 host = context_roots None 回落 env，与改造前语义一致）；plugin-host 集成 37 绿；workspace 361 绿、桌面 156 绿、clippy 0 双侧、fmt、check_length OK |

验证：静态取证（audit.jsonl / 会话旁车 / settings / DLL 构建时间 vs ㉘·B 提交时间）+ 全量测试；实机复验待应用重启载入 0.1.1 产物后重试——此后第一次越界拒绝即自带围栏根清单，内嵌 Agent 据此可直接列出归属工作区。

### ㉘·C 权限档一等公民：围栏让路 + 隐私门（✅ 已完成，v2.67——用户裁定「权限选择是一等公民，其它阻挡全部让路；完全访问基本零弹窗，仅最危险的隐私/本地隐私有拦截」，参照 ZCode 规则设计）

| 项 | 说明 |
|---|---|
| 围栏让路 | ✅ v2.67：CallContext.fs_roots 统一下发全局声明根 \[数据根\] ∪ settings.fsRoots（resolver 增 fsRoots 供给闭包，保存即生效），所有权限档位一致、不再按归属收窄为 \[数据根, 项目工作区\]——会话工作区归属不再构成权限墙（跨项目浏览在声明根内畅通，出声明根仍由插件 fail-closed 拒绝 + 文案自描述列根指引设置加根）。直接消除 v2.66 报障场景：已选 AAA 工作区问 Locowrk 文件夹不再被拒 |
| consult 全量 | ✅ v2.67：kernel-agent 决策环 if self.approval.is_some() ‖ 非 Pure ——决议者在场 = 每次调用都过决策表（含 Pure），隐私门才能拦住 shell_read 读 .ssh；只读直通由决策表放行（reason mode:pure / mode:plan-readonly，前端不渲染记录条）；headless 无决议者语义零变化（Pure/Compensable 静默放行、Irreversible fail-safe 拒） |
| 决策表重排 | ✅ v2.67（DESIGN §7.2 重写）：①计划档非 Pure 硬拒 → ②硬门（隐私 + 越界，强制两键卡、不进豁免集、完全档也跳不过）→ ③模式放行（完全=全放基本零弹窗 / 自动编辑=Compensable / 询问=非 Pure 卡 / Pure 直通）→ ④会话豁免 → ⑤决策卡。越界门围栏改读 CallContext.fs_roots 与插件围栏同源（声明根内命令不再弹卡） |
| 隐私门 v1 | ✅ v2.67：新 desktop privacy.rs 纯函数扫描（宁漏勿错杀——漏报由档位兜底）：凭据目录 .ssh/.aws/.gnupg/.kube（组件边界）、.git-credentials/.netrc、.docker/config.json、浏览器凭据库（厂商目录限定）、系统凭据库/转储命令（vaultcmd/mimikatz/sekurlsa/lsass dump）、毁灭性命令（format x:/mkfs/diskpart/cipher /w/vssadmin delete/bcdedit/dd of=/dev//fork bomb/盘根递归强删）；.env 不拦（开发日常，ZCode 同款）；文件体字段（content/patch）跳过防误伤。命中载荷 sensitive:<类别> |
| ZCode 借鉴记录 | ✅ v2.67：侦察其源码（core/src/permission/）——无路径围栏（path-policy 自述有意不硬拦）、yolo 之上仅 alwaysAsk 工具钉 + 交互类必问、决策序「阻断类全在放行类前」；采纳其骨架（模式一等公民 + 最宽档只留 always-ask 硬钉），补其自认缺口（凭据/毁灭命令清单为桌面端最小集） |
| 前端 | ✅ v2.67：PermissionCard 硬门提示行（越界 warn 行 + 隐私类别行 触及隐私或高危目标：{kind}，approval.sensitive.\* 10 类别 zh/en）、两键判据 external ‖ sensitive；四档菜单描述（完全 = 「全部放行，基本不再弹窗；仅越界/隐私拦截」）+ 设置行 hint 同步；types.ts 载荷镜像 +sensitive |
| 插件版本 | ✅ v2.67：tool-{fs, shell, git, edit} 0.1.1 → 0.1.2（fsutil 拒绝文案改「全局声明根 + 指引设置加根」，v2.66 的「围栏跟会话归属」措辞过时；fsutil 为四件静态链接依赖，同批重建） |
| 测试 | ✅ v2.67：kernel-agent 70 绿（+2：Pure 决议者在场被 consult 可拒 / headless Pure 静默零审批事件）；桌面 166 绿（+10：隐私门 6 + 隐私卡/Pure 直通/越界门含声明根内放行 3 + 围栏保存即生效 1）；workspace 全绿、clippy 0 双侧、fmt、pnpm build 过、check_length 报告 4 文件超建议值（三问自检保留：kernel/tests.rs 测试单主题、PermissionCard 卡单组件、approval.rs 决策表单职责、state/mod.rs 组装点） |

### ㉘·A·fix 双层记忆模型 + 归属先行 + 工作区可见注入（✅ 已完成，v2.68——用户实机报障「选了 Glance-main 它却分析了 Locowrk，工作区没起到作用吗？」并裁定「跨会话记忆要关闭——全局记忆记用户喜好偏好全局起作用，项目记忆不能跨项目读到」）

| 项 | 说明 |
|---|---|
| 取证 | ✅ 磁盘取证（会话 jsonl 170 事件 + meta 旁车 + memory.sqlite3）：归属本身全程正确（Glance-main 旁车、围栏拒绝 F:\\ 根、声明根全可读、mode:pure 记录 = v2.67 内核在跑）；真凶两个——①归属时序缝：首聊挂起归属在轮次收束后才 sessionAssociate（chat.ts finally 块），首轮 Memory-Inject 落在未归属键位（''），旧 smoke-test 记忆被注入且被 M1 会话级预取整场缓存；②工作区对模型不可见：工作区只是权限边界，模型思考原话「I don't know the path」，"看下这个项目"只能靠旧记忆猜，猜了记忆足迹最大的 Locowrk |
| 双层记忆模型（用户裁定） | ✅ Context 文档 §4.6 重写：全局层（''）= 用户喜好偏好（如"永远用 pnpm"），对一切会话可见；项目层 (workspace,key) = 项目记录，绝不跨项目可读。召回 = 本项目 ∪ 全局层，条目带层标（scope: "project"\|"global"，注入渲染 - \[global\] key = …）。未归属会话缺省项目写拒绝（没有项目的"项目记忆"不许落地污染全局层，报错指引先归属或 scope="global"）；memory_write 显式 scope="global" 升格；全局行聊天工具禁删（memory_delete 只作用本项目键位，删全局 = 管理页专属——人的操作）；headless（无会话身份）保持 '' 单一宇宙旧行为（plugin-host 测试装配/kernel-bin 直调不受扰——判据 = 无 session_id 即 headless 等价，v2.64 ToolExposure 同款，WsCtx::from_parts 纯函数 + 测试）；经历不设全局层维持严格本项目 |
| 插件 memory-sqlite 0.1.1 | ✅（小迭代 +0.0.1）：workspace() 二值回落 → WsCtx 三态（Headless / Unattributed / Project——adapter 无解析器时预置的全缺省 CallContext 曾被误判 Unattributed，实抓于 plugin-host 端到端测试）；write +scope；read 项目优先未命中回落全局（带层标）；list/search = 项目 ∪ 全局逐行带层标；delete 全局禁删 + headless 例外；undo 忠实还原不受禁删约束（补偿是审批后的机械还原）；manifest 描述/schema 全部同步双层语义；18 测试绿（+5：双层写路径/未归属拒绝/读回落与项目优先/全局禁删/三态纯函数） |
| 归属先行 | ✅ agent_chat +workspace 参数：首聊挂起归属（chat.pendingWorkspace）随请求带入，后端 ensure_associatable（与 session_associate 共用一条 fsRoots 防线）+ 建会话即写旁车——先于首轮任何注入/围栏/提取；前端 chat() 第三参 + send 流程提前消费 pendingWorkspace（finally 块的收束后 sessionAssociate 退役）；搭车：未归属会话隐式提取跳过（autoExtract 的项目事实不许污染全局层） |
| 工作区可见注入 | ✅ MemorySource 新 context_line（缺省 None）：归属会话注入块头部 <workspace_context>Current project workspace: {根} — "this project" 指这里，先列/读它，不要按旧记忆猜路径</workspace_context>；零记忆也注入（此前零记忆 = 完全不注入，归属会话对自身项目一无所知）；MemorySource 新 scope_id（缺省空串）：M1 预取缓存键 = (session_id, scope_id)——会话中途改归属 = 换键自动重算，键位不变逐字节复用（M1 前缀纪律不破） |
| 内核 kernel-agent | ✅ MemoryEntry +scope 字段（空串不标层，旧装配零负担）；memory_inject 中间件：缓存键并键位 + 上下文行进块（与记忆/经历同缓存同失效）；74 测试绿（+4：scope 换键重算/上下文行零记忆注入/上下文行随块冻结/层标渲染） |
| 管理页 | ✅ 记忆行归属徽章补 '' 态：「全局偏好」（i18n memory.globalTier，zh/en）——'' 不再是无标注的悬空行；经历区维持原样（无全局层） |
| 数据清理 | ✅ memory.sqlite3 四条走查残留（test.smoke.2025.host-plugin / test.coexist.2025.{r1,findings,r3}，v2.62 前后冒烟探针）备份后删除（memory.sqlite3.bak-20261004）——全局层清零起步，不再有毒记忆可漏 |
| 测试 | ✅ v2.68：kernel-agent 74 绿（+4）；memory-sqlite 18 绿（+5）；plugin-host 全绿（端到端走真 vtable，WsCtx 三态判据在此实抓）；桌面 167 绿（+1：scope_id/context_line 随归属 + 召回双层断言重写）；workspace 39 套件全绿；clippy 0 双侧、fmt、pnpm build 过、插件产物重建；check_length 报告 5 文件超建议值（三问自检保留：两测试文件单主题、PermissionCard 卡单组件、approval.rs 决策表单职责、commands.rs 薄适配入口） |

验证：静态取证（jsonl 逐事件复盘 + meta 旁车 + memory.sqlite3 键位盘点 + LLM 深思原文）+ 全量测试；实机复验待应用重启（载 0.1.1 记忆插件 + 新内核）后重试原实验：Glance-main 项目下新会话问「看下这个项目」——应直接列 Glance-main（上下文行在场），旧记忆零引用（全局层已清空）；模型写"永远用 pnpm"应落全局层（scope 标 global），其它项目可读；未归属会话 memory_write 应被拒并指引归属。LLM 流中断报错（error decoding response body）= DeepSeek 侧流式传输中断（网络/代理层瞬时故障），内核如实上浮、日志收束干净（170 事件无半截写入），重发即可、非缺陷。

### ㉙ 会话运营补全（✅ 已完成，v2.57——功能层规划第二批）

| 项 | 说明 |
|---|---|
| thinking 数据源 | ✅ v2.57：SessionEvent::Thinking（DESIGN §4.1 新事件，derive_messages 跳过——不回传供应商，压缩/用量不受影响）；LlmStreamSink +默认 on_reasoning、LlmService +默认 complete_streaming_reasoning（缺省回落，DemoLlm/headless 零改动）；genai ReasoningChunk + capture_reasoning_content；agent://thinking 通道实时 delta 与重放整段同形（StepOutcome.reasoned 抑制同 ⑳ streamed），内核闭包累积全文落日志（sink 故障也有整段可投）；v2.42 思考行壳点亮；服务契约拆 kernel/services.rs（单文件上限） |
| 消息编辑重发 | ✅ v2.57（UI shell §3.34 新节）：投影 UserMessage 带事件位 pos；session_truncate_after(pos?) 命令（None = 最后一条用户消息；忙锁 try_lock 守卫；校验截断位是 UserMessage；rewind_to + fork + persist rewrite .undo.bak 留痕同 ⑨⑲）——核心提 session_log::truncate_after 纯函数；前端 MessageRow 编辑态（Enter 提交/Shift+Enter 换行/Esc 取消）→ 本地截断 → 复用既有 send 通道重发（审批/用量/提取零新链路）；编辑钮出现条件 = 重放消息或最后一条（宁缺勿假） |
| LLM 自动会话标题 | ✅ v2.57：chat.autoTitle 缺省开（可配置原则 = 开关）；首轮收束后旁车无 title 字段（手动重命名过永不覆盖）时一次 LLM 调用生成 ≤25 字标题（净化：去引号/取首行/截断）；agent_chat 与调度引擎两轮收束点同挂；写旁车 + lw://session-title 推送（侧栏/顶栏即时换名）；失败静默 |
| 定时任务目的地页 | ✅ v2.57（UI shell §3.35 新节）：§2.4 侧栏底第 3 图标（clock）兑现；schedule_list/schedule_delete 命令（store +list_all/+created_ms）；页面 = 徽章（一次性/每 N）+ 标题 + prompt 两行 + 目标会话名 + 下次触发（逾期 = 「等待会话空闲」如实态）+ 顺延次数 + 两步确认删除；空态引导（创建入口是模型工具，页面不做编辑器——宁缺勿假） |
| 测试 | ✅ v2.57 |

验证：实机走查（CDP 29-1…7 + 落盘取证，真实轮，deepseek-flash 实际流式吐推理）：**思考行实机点亮**（实时推理英文原文进 §3.9 行、单行截断可展开；jsonl 落 `thinking.text` 全文）→ **自动标题实机生成**（两个新会话 meta.title = 「天空为何蓝及一句诗」「瑞利散射与晴空一鹤诗」，顶栏/侧栏同步；编辑重发后的第二轮不覆盖）→ **编辑重发实机**（行内改问句 → 旧气泡与旧轮消失、新问句重发新回复流式；主日志只剩新轮 3 事件、`.undo.bak` 留截断前全文）→ **重放一致**（切走再切回，思考行/编辑后气泡从 session_history 恢复）→ **定时页全链路**（模型 schedule_create 经审批卡允许一次 → 定时目的地行：一次性徽章/标题/prompt/目标会话名/下次触发 13:11 → 两步删除 → 空态；sqlite 归零）→ 设置页「自动会话标题」开关行在场。走查会话已删（行菜单两步）、调度库空、settings 与走查前逐字段一致。

---

### ㉞ 欢迎页改版 + 侧栏项目接管（✅ 已完成，v2.59——docs/issue 14 条用户问题批 + 走查中追加裁定）

来源：`docs/issue/空白对话欢迎页问题统计.md`（14 条 + 走查中用户追加 3 条：弹层实底、暗影尺寸、任务行不缩进）。UI shell 文档先行 v3.9（§2.1/§2.3/§2.4/§3.6/§3.11/§3.14/§3.15/§五/§6.6），规范成文后动码。

| 项 | 说明 |
|---|---|
| ① 拖宽自由调宽 | ✅ v2.59：拖拽/键盘只在 240–520 内自由调宽，不再触发折叠（旧"拖到最小值即折叠"废弃，折叠只经按钮/Ctrl+B）；走查实抓真根因：--lw-sidebar-w 从未有 JS 写入 DOM（宽度只存 store/localStorage，视觉从未变过）——Sidebar aside 内联写 --lw-sidebar-w: {width}px 修复，"不能自由拉大拉小"的本源即此 |
| ② 右栏滑动动画 | ✅ v2.59：WorkPanel 挂 <Transition name="wp">，与侧栏统一（开 width 0→预算 + opacity --dur-base/--ease-out、收 --dur-fast/--ease-in；transition 只挂 enter/leave-active，拖宽不动画；§6.6 ② 成文） |
| ③⑤ 顶栏减钮 + 动作区 | ✅ v2.59：顶栏新建/搜索钮移除（侧栏已有，重复）；侧栏顶部动作区三行 = 新建任务（Ctrl+N）/ 搜索（Ctrl+K 开命令面板——搜全部可操作项）/ 插件市场（原「扩展」上移改名） |
| ④⑥⑦⑧⑨ 项目接管 | ✅ v2.59：侧栏只保留项目工作区（用户裁定严格口径：顶层「会话」组、时间桶、「未分组」全部退役；无归属会话不进树，⌘K 照列全部可移入归位）；「项目」组头 = 下三角（点击折叠/展开全部项目）+ hover 显形「筛选（全部/只看进行中，前端过滤）/ 新建项目（pick_directory → fsRoots）」；项目行 = 文件夹双态（闭/开对应折叠/展开，无前置 chevron）+ hover 显形「⋯（菜单：新建任务/在文件夹中显示 reveal_in_explorer/移除项目）/ ＋（该项目下新建任务）」；任务行 = 名与项目文字同一左缘不缩进（用户裁定）+ 名前 1px 竖向发丝线（挂文件夹字形下方）+ 状态点；chat.pendingWorkspace：项目下新建任务首聊后自动 session_associate（后端校验不变） |
| ⑩⑬⑭ 欢迎页 | ✅ v2.59：独立形态 chat/WelcomeHome.vue——吉祥物（浮动 4s + 周期眨眼，reduced-motion 全停）+ 四态问候（上午好 05–12/下午好 12–18/晚上好 18–23/夜深了，名字 = 当前索引项目：当前会话归属 → 最近活动会话归属 → 第一个 fsRoot → "Locowrk"）+ composer（Teleport 进槽位，元素引用不走选择器——初挂载整树未入 document，选择器必扑空；草稿跨形态保留）；定宽 720、列宽柄不渲染；双层卡影（扁平暗影三边 \~1px + 下 16px 露出，尺寸经用户两轮裁定）；首页空态下顶栏面板开关隐藏 + Ctrl+J/⌘K 命令守卫（空白页不可展开右栏） |
| ⑪⑫ 页脚重排 | ✅ v2.59：设置（真齿轮图标，原 sun 形齿标换 lucide settings）/ 记忆 / 定时 / 通知 + 版本号（暂留，后期入设置页）；新增 lucide folder-open / filter（缩放 g 变换 24→16 viewBox，描边视觉重量与原生表一致） |
| 弹层实底（追加裁定） | ✅ v2.59：--lw-bg-elevated 两主题改不透明（亮 #ffffff / 暗 #212121），CommandPalette/ConfirmDialog 去 backdrop-filter；分类处淡发丝线分隔（--lw-line-subtle，两端不到头 inset 12px）——⌘K 组间、模型选择器 provider 组间（行菜单分隔线原本已合规）；成文 §五 弹层质感 |
| 走查追加·第二轮（弹层与 composer 细节） | ✅ v2.59：权限菜单照抄参考（名称 计划模式/变更前确认/自动编辑/完全访问 + 描述 + 💡✋🛡✓🛡! 四图标 + 行间发丝线 + 选中态只挂勾不做整行高亮 + 「完全访问」chip 橘黄警示色 --lw-warn + 首页向下弹/会话向上弹）+ 权限 chip 首页空态显示（选择=写新会话缺省档）；模型 chip 底色不常显（hover/展开才显）+ 首页向下弹；工作区 chip（WorkspacePicker，卡片外底行坐在暗影层上，最近/选择新项目/不需要项目，会话=关联、首页=挂起归属）；附件 ＋ v1（添加文件或图片 → pick_files 原生多选 → @路径 引用写入输入框，图片上传通路属 ㉛）；发送键（箭头 16px/禁用态底加深箭头变浅）；弹层滑动过渡（lw-pop-down/up + 挂载动画孪生，五处弹层接线）；暗影改挂卡片 box-shadow（0 模糊实底双层：三边 1px + 下 32px，与卡外底行解耦不被撑高）；弹层面板坏变量修复（--lw-surface-1/2、--shadow-raised 不存在致透明/无影）；主题实效档解析下沉 lib/theme.ts |
| 收尾 | ✅ v2.59：docs/issue 加入 .gitignore；5 条「你好」测试会话按用户指令清除 |
| 测试 | ✅ v2.59 |

验证：实机走查（CDP，tauri dev + vite 热重载）：动作区三行/项目头三图标 hover 显形/文件夹双态/项目菜单三件/任务行发丝线+对齐/**项目下新建任务真实轮自动归属**/折叠全部↔展开/只看进行中/⌘K 实底+组间发丝线+正文命中组/Ctrl+J 面板开合动画/键盘拖宽 240 钳制不折叠/reveal_in_explorer 实机打开资源管理器（后端 tauri dev 热重建）。走查会话已删（行菜单两步）、fsRoots 已还原空、settings 其余零改动。

---

### ㉟ 走查修复批：欢迎页/顶栏/工作面板/composer 十三条（✅ 已完成，v2.60——用户 2026-10-04 逐条裁定，纯前端批）

| 项 | 落地 |
|---|---|
| ① 无项目问候不带名 | ✅ v2.60：无任何已选项目 = greetingBare 变体（"晚上好，今晚想把什么收尾？"），旧 "Locowrk" 兜底退役（lib/greeting.ts homeProjectName 返回 null → chat.greetingBare.\*，zh/en 四态全配） |
| ② 新建任务即时跟切 | ✅ v2.60：homeProjectName 挂起归属最优先——项目行 ＋ / 工作区 chip 选中的项目立刻反映进欢迎页问候（此前只读旁车/会话归属，问候滞留旧项目） |
| ③ 首屏收右栏 | ✅ v2.60：App.vue WorkPanel v-if 增 !isHome——会话态开面板切回首屏即收起，回会话再恢复（空态锁右栏补全） |
| ④ 发送键浅箭头 | ✅ v2.60：禁用态 = 浅色箭头（白 92%，两主题一致）+ 退灰底（ink 18%）——旧 ink 混色暗主题浅底浅箭不可读 |
| ⑤ 项目 chip 移顶栏 | ✅ v2.60：会话态 WorkspacePicker 以 iconOnly 变体挂顶栏标题前（纯文件夹图标，点击下弹菜单）；composer 底行只剩首页形态（Topbar + WorkspacePicker iconOnly prop） |
| ⑥ 标题固定宽度 | ✅ v2.60：顶栏标题 max-width 360px + truncate，长标题尾部省略（面板开合截断宽度稳定不跳） |
| ⑦ 弹层全部上滑 | ✅ v2.60：会话态附件/权限/模型实测上滑（既有语义核实）；修回 v2.59 UsageRing 拆分时截丢的 ContextUsagePopover 整块 <style scoped>（用量浮层失去定位/实底在工具条里平铺——本批走查实抓）；搭车修实抓缺陷：模型选择器是五弹层中唯一无外点/Esc 收口的 → Composer 锚点 pointerdown + 选择器 Esc 补齐（实机验证外点/Esc/双击 chip 三路收口） |
| ⑧ 面板开关裸钮不移位 | ✅ v2.60：action-group wash 底容器去除（裸 .ctl）；顶栏恒预留 --lw-wincontrols 车道（原面板打开时 padding-right 归零使开关右跳 120px）——开合前后开关相对主区右缘恒 120px（实测） |
| ⑨ 面板头去关闭钮 | ✅ v2.60：WorkPanel 头部「关闭面板」钮退役（开合统一走顶栏开关，i18n panel.closePanel 删）；最大化图标换斜对角双箭头（lucide maximize-2 缩 16 格，新图标 maxDiag） |
| ⑩ 关闭钮红底 | ✅ v2.60：窗口关闭钮 hover #c42b1c 白字形 / active 加深一档（Win11 标准配置；实测 hover bg rgb(196,43,28)） |
| ⑪ 最大化盖对话区 | ✅ v2.60：toggleMaximized 吃 fullBudget = 视口 − 侧栏宽（主区地板 450 最大化态不保）——侧栏展开实测 1040=1280−240、折叠实测 1280 满窗口；面板 z-20 压过 composer 坞 z-12/列宽柄 z-11（覆盖态浮层残影不透出）；aria-valuemax 随最大化切换 |
| ⑫ 列宽柄留白 | ✅ v2.60：柄位公式 −6px → −16px（柄 12px 全落列外 + 4px 空隙），提示条与正文实测 9px 间距 |
| ⑬ 正文对齐 | ✅ v2.60：转录滚动区 scrollbar-gutter: stable both-edges 对称预留——转录列与 composer 列同以主区中线居中（旧右缘单侧留槽使正文整体左偏 ≈5px；实测两列 left/right 逐像素相等） |
| 测试 | ✅ v2.60 |
| ⑭ 问候语同源（v2.61 复测单①） | ✅ v2.61：用户复测「还是不灵敏」——v2.60 索引链的「最近活动会话/第一个 fsRoot」回落与 chip 不同源（挂起归属清空后 chip=「选择文件夹」问候仍挂旧项目）。修 = chat store 新 getter currentWorkspace（会话态旁车 / 首页挂起归属），chip 与 WelcomeHome 同吃一个真源，homeProjectName 回落链退役；实测选 A→同变 A、新建任务→chip 空 + 问候不带名、选 B→同变 B |
| ⑮ 面板开关钉窗（v2.61 复测单②） | ✅ v2.61：用户复测「展开右栏还是被挤跑」——v2.60 恒预留车道只对消 120px 车道，主区自身随面板收缩文档流内必被挤动。修 = 开关 position:fixed; top:9px; right:120px; z-30 零位移，Topbar/面板头 padding-right = 车道+40px 留槽（原「关闭面板」钮位置，⑨ 裁定闭环）；实测开合前后 left=1132/top=9 逐像素不变、与 \[+\]\[⤢\] 无重叠 |
| ⑯ 钉窗开关层叠修正（v2.62 复测单③） | ✅ v2.62：用户复测「右栏一展开开关就不见了、没有按钮收起」——开关虽 fixed 仍在顶栏 header（relative z-10）层叠上下文内，整份只有 z-10，被 z-20 面板整体盖住（此前 JS .click() 验证绕过命中测试没抓到）。修 = Topbar 改双根、开关作为组件第二根挂 data-chat-root 直属（z-auto），自身 z-30 真压过面板；验证升级为真实命中测试（elementFromPoint + CDP 真实鼠标点击收起，皆 PASS） |
| ⑰ 上下文统计持久化（v2.62 复测单④） | ✅ v2.62：用户复测「新会话首条消息有用量显示，退出再进就没了」——用量快照只推实时通道不落日志，重放面无此事件。修 = 照 LlmUsage 先例：session-store 新 SessionEvent::ContextUsage（serde(default) 向后兼容；derive_messages 跳过），run_turn 轮次前 + 每 step 后各落一条（record_usage 自带落盘与实时单发，下轮窗口越过不重发；收敛判据读 last() 在追加前），project_events 重放同形——前端重放期恢复用量环/进度条；回归测试 context_usage_persists_and_replays（+1，桌面 142 绿） |

验证：实机走查（CDP 260-1…12-13 + 截图，tauri dev）：无项目问候 nameless/带项目问候/项目 ＋ 即时跟切（greeting+chip 双证）/会话态开面板→新建任务回首屏面板收起/发送键禁用态白箭 rgba(255,255,255,.92)/顶栏项目图标+下弹菜单（wsrow 退场）/长标题 360px 省略/三弹层上滑 + ctx-popover 规则在场/开关无底色 + 120px 恒定偏移/面板头两钮（启动器+斜对角最大化）无关闭/关闭钮 hover 红底/最大化 1040 与 1280 双态/柄距正文 9px/两列 left=283 right=1237 零偏差。v2.61 复测（CDP）：场景 A 同源三步（选 A/新建任务/选 B 两边逐次同变）+ 场景 B 钉窗零位移；走查会话已清（含一个 LLM 流式挂起轮次随进程终止后盘上删除）、fsRoots 还原空、settings 其余零改动。

### ㊱ 消息流 v4.11 十一条：停止/排队/坞卡/折叠（✅ 已完成，v2.74，提交 25275ce——用户 2026-10-05 逐条裁定，功能逻辑参照 F:\\Warehouse\\Github\\ZCode；UI shell 文档先行 v4.11：§3.7/§3.9/§3.11/§3.17 重写 + §3.39 新节 + §3.25 Esc 链 + §4.1/§4.2 扩表）

| 项 | 落地 |
|---|---|
| ① 决策/失败通知去卡片底 | ✅：chat store approval_resolved 投影改 decision 条目（无卡片底），ChatTranscript 过程组内平铺 12px 行（ok=check --lw-ok / denied=warn --lw-danger）；错误行同准入组；纯决策/错误（无工具/思考）不套组壳直接平铺；SystemNotice 审批气泡退役 |
| ② 过程组默认全折叠 | ✅：expandToolProcess 开关覆盖重放（旧「实时才吃设置」退役）；「已停止」新状态词入组头（仅最新组，stopRequested 驱动） |
| ③ 会话移入其它项目退役 | ✅：SessionRow 菜单 ws: 段删除（归属只由首聊挂起/项目下新建产生；session_associate 命令后端保留） |
| ④ 权限卡坞化 | ✅：PermissionCard 重写紧凑三段坞卡（头行 动作名+风险胶囊+徽章+右置动作〔拒绝 Esc 键帽｜允许一次｜chevron 上弹「允许本次对话」，硬门两键〕/ 参数单行 mono 预览 / 元信息行）渲染进 Composer 坞槽（决策后即隐、对话流零气泡；orphan 兜底退役）；守夜倒计时=卡底 2px 细条 |
| ⑤ 回到底部 | ✅：ChatTranscript 32px 圆钮（坞上方 12px，lw-pop-up） |
| ⑥ 发送键=停止 | ✅：后端停止链——stop.rs StopFlags 注册表（RAII 守卫进 run_turn）+ LlmStreamSink::should_stop（回调签名回 bool，genai 流循环命中即带部分正文收束）+ runner 步间边界 TurnDone + agent_stop 命令三路同收（标记置位/审批 fail-closed 拒绝/ask 跳过）；前端 stopRequested→「已停止」头行 |
| ⑦ ask_user 选择卡 | ✅：desktop 新 ask.rs（AskHub oneshot+守夜+ask_snooze 摘除 + AskUserTool Pure+always 常驻）+ agent://ask-user/ask-resolved 通道 + AskCard（字母瓦片选项/单选即提交/多选+自由输入/首次交互续期）+ tools.askTimeoutSecs（0=不限，缺省 300，设置行+热换+校验） |
| ⑧ 排队卡 | ✅：前端 UI 态队列（不入会话日志）——QueueCard（两行预览/立即发送=抢占〔移出→停止→重发〕/删除/暂停续跑）+ turn_done 自动派发一条一轮 + 停止/出错转暂停 |
| ⑨ 跟随滚动 | ✅：默认贴底 48px 窗口，watch(chat.items) 隐式深（流式 delta 也跟）；用户上滚让位 |
| ⑩ 嵌套内容换行 | ✅：markdown 代码块 pre-wrap+anywhere（旧横滚）；决策/错误行 span anywhere；tool 明细/思考体既有 pre-wrap 核实 |
| ⑪ 一级消息左对齐 | ✅：TurnProcess 展开体去缩进/引导线（gap 1px 扁平直排）；ThinkingRow「思考 · 持续了 N 秒」（实时记账 ts/doneTs，重放首句回落） |
| Esc 链 | ✅：§3.39⑤——跳过 ask → 拒绝审批 → 停止 → 交出焦点（模型浮层开着让位） |

测试：desktop 210（+5：stop 3 + ask DTO 2）绿、workspace 全套件绿、clippy 0 双侧、fmt、pnpm build 过、check_length 16 超（14 历批已裁定 + Composer 447/PermissionCard 444/ChatTranscript 511/commands.rs 462/state 515/approval 452 三问皆单职责不拆）。**实机冒烟走查（CDP 真实轮 deepseek-flash）**：发送键停止方块态 + 运行中占位「AI 正在执行，输入将排队…」实拍（w2/w3）→ 排队消息 turn_done 自动派发实证（模型收到「第二条」为新轮）→ schedule_create 审批 100s 守夜 fail-closed 自动拒绝（模型自述证实；无调度残留）→ 过程组头「已完成 · Ns · N 个工具 ⓘ 1 个问题」+ 扁平左对齐实拍 → 自动标题实发。**走查中止说明**：用户正实机使用 dev 实例，为不干扰操作 CDP 驱动中途收手——已停止头行/QueueCard/AskCard 视觉、决策记录行展开态留待用户目检（组件与 store 逻辑已被构建与路由覆盖）；走查会话 2 条留侧栏待删（无调度/记忆/设置残留）。**v2.74·fix（用户目检）**：dock-cards 缺宽度规则使三卡吃坞全宽（933 vs 724 实测）→ 补 conv-w 宽度 + 子项 min-width:0；cv-head 标签 4px 右移（padding 7px→3px）+ label nowrap；markdown 列表缩进摊平（padding-left 22px→0 + inside）——三卡/头行/列表与列同宽同缘实测复核。**v2.74·fix②（目检追加 + 参考图重做）**：`.turn__grid` 隐式 auto 列轨道按 max-content 取尺寸致展开体横向溢出（1693 vs 560 实测）→ 显式 `minmax(0,1fr)`（用户重启后确认生效；**scoped-style HMR 不可靠记档**——样式修复验证以重启后为准）；三卡按用户三张 ZCode 参考截图重做=贴合柱面（dock-cards 零间隙同底色连成一根、有卡时输入卡顶角方角化）+ 权限卡 ✋ 头行与 `允许一次|⌄` 分体主钮（风险胶囊退役、预览裸行 mono、顺手修 overflow:hidden 裁上弹单项）+ 任务卡问题即卡头（跳过移右下）+ 排队卡 `N 条排队中 ⌄`+垃圾桶清空+圆形 ↑ 抢占（单行预览悬停删）——UI shell 文档 §3.17/§3.39 同步落「v4.11·重做」，pnpm build 过，三卡视觉待用户重启 dev 目检（其实例未开 CDP 无法自查截图）。

---

### ㊲ LLM 兼容层重构：genai 退役 + 自建 OpenAI 兼容客户端 + 传输韧性（✅ 已完成，v2.75——用户 2026-10-05 裁定：「llm 兼容层全删、参照 PI-Desktop 重构、不打补丁式修」+「重试不设次数上限，半次失败都不可接受」；[LLM.md](http://LLM.md) 文档先行 v2.x→v3.0）

| 项 | 落地 |
|---|---|
| ① genai 全删 | ✅：llm/genai_backend.rs 删除 + genai = "0.6" 依赖退役（desktop/src-tauri 唯一依赖点；内核 crate 零 genai）；TLS/stream 特性显式补进 reqwest（原靠 genai 依赖图带入，不补则 HTTPS 全断）；AppLlm/AppLlmRecorder 换 CompatLlm |
| ② 一条线协议 | ✅：线上只有 OpenAI Chat Completions（{base}/chat/completions + SSE）——kind 只决定鉴权头（openai=Bearer / anthropic=x-api-key+anthropic-version 双带 / ollama=无）与端点归一（join_version 沿用：智谱 /v4、方舟 /v3 原样，ollama 落 /v1 官方兼容端点）；22 预设全兼容；非流式 complete = 同一管道聚合（无第二形状）；不发 max_tokens 派生限额（PI 同款教训：兼容网关拒收该类字段） |
| ③ 传输韧性重试 | ✅：错误分类（401/403 鉴权 fatal / 429 限流 / 408·5xx·网络·断流瞬时 / 400·413+上下文超限语义 fatal 带指引 / 其余 4xx fatal 带报文片段）+ 退避（限流 2s×2 指数+25% 抖动封顶 30s；瞬时 1/2/4/8s 封顶 8s；服务器 retry-after-ms/Retry-After 优先同封顶）+ 重试不设次数上限（用户裁定：瞬时故障一直重试到成功或用户停止；单次等待封顶不存在无限等）+ 180s 零事件看门狗（断流可重试，长输出不限总时长）+ 重试等待 200ms 步进轮询停止标记 |
| ④ 流式重置 | ✅：已推 delta 的重试先发 agent://llm-retry（attempt/delay_ms，组装闭包 emit——AppHandle 不进 llm 模块，v2.66 同款）→ 前端 resetAssistant 清本步流式缓冲 + notice.llmRetried 平铺状态行；重试轮 delta 从零重建同一气泡；重放不受影响（投影只认返回值）；纯等待型重试零通知（用户不可见） |
| ⑤ 步数到顶优雅收束 | ✅：runner 不再 Err 硬掐——TurnDone 正常收束 + 新 agent://turn-capped 如实告知（notice.turnCapped：「发『继续』可接着跑；设置 → 工具与分发可调高」）；已完成的工具成果全保留；契约测试随新语义更新 |
| ⑥ 线上件 | ✅：llm/ 新四模块——wire.rs（请求装配/SSE chunk 解析/工具调用按 index 分片累积/空 call_id 补号/usage 映射含 OpenAI cached_tokens·DeepSeek prompt_cache_hit_tokens·reasoning_tokens，全 0=None 宁缺勿假）+ sse.rs（行缓冲分帧/注释跳过/\[DONE\] 立即派发/EOF 冲刷）+ retry.rs（分类+退避纯函数）+ client.rs（CompatLlm 重试循环/停止部分正文收束/stream_options 400 修复性去字段重发一次） |

验证：desktop 213 绿（+llm 全新 13 测试：payload 形状/分片累积/补号/推理分流/usage 三形态/SSE 跨块分帧/分类表/退避曲线/端点归一/演示回落/缺 Key 快速失败/会话覆盖；步数到顶契约测试随新语义更新）、workspace 398 绿、clippy 0 双侧、fmt、pnpm build 过、check_length 12 文件超建议值（llm/tests.rs 425 新增入列——单一测试职责不拆，其余历批已裁定）。genai 源/依赖/锁文件零残留（grep 全仓 0 命中，Cargo.lock 无 genai）。已知边界（记档）：①Anthropic 走官方 OpenAI 兼容层（原生 /v1/messages 不再用）——兼容层不暴露的扩展字段（如 thinking 签名）宁缺勿假；②Ollama 走 /v1 兼容端点（原生 /api/chat NDJSON 不再用）；③HTTP-date 形态的 Retry-After 不解析（回落退避曲线，极少见）；④重试等待期间停止=空收束（前端「已停止」已示，不虚构结尾）。**实机走查待做**（真实轮 deepseek：正常轮/构造 429 重试/断流恢复/停止语义/步数到顶新文案）。

### ㊳ 术语统一 + 三栏响应式（2026-10-06 用户指令：「把项目里所有捆/bundle 术语统一替换」+「实现三栏响应式布局，遵循非对称级联折叠规范」；同日 V2 规格微调：「限制不能碰能不能用，只能调怎么用」，规格文档 `docs/design/three-column responsive layout V2.md`）

| 批次 | 状态 | 说明 |
|---|---|---|
| 术语统一（Bundle→Chapter / 捆→章节 / 记录员→记忆管理器） | ✅ v2.108 | memory-tree 模块与 desktop organize 管线全量改名；落盘契约不动（serde alias 双读、b{n} id、recorder/rule 数据词汇、设置键、tree.md 指针格式）；「捆稻草」比喻只在设计哲学语境保留；详见《Memory Tree Design v2》术语注 |
| 三栏响应式（非对称级联折叠） | ✅ v2.109 | 断点 1400/1100/800 级联折叠；左栏 >1100 自动展开（手动折叠优先）、右栏永不自动展开；<800 窗口过窄整窗提示；UI shell §3.40 |
| ㊳·fix 目检修正（单钮 + 右栏意图消费） | ✅ v2.110 | 顶栏钉窗收拢单钮（左栏开关退役——规格「右上角切换按钮」只指右栏；车道 +72→+40px）；右栏开意图被断点折叠消费（<1400 即落 false 持久化）+ work-panel setOpen 视口钳制（程序化入口不落悬空 open）——回宽永不自动展开，修 v2.109「回宽恢复」违规；断点常量下沉 lib/breakpoints.ts；§3.40 ②·4 违规条款重写 |
| ㊳·fix② V2 规格（按钮永不冻结 + push/overlay 双模式） | ✅ v2.111（部分被 v2.112 口述定稿覆盖：级联回归、警告层退役） | 断点只决定展开模式、永不决定按钮可用性（用户限制评审尺度落档 §3.40 头）：v2.109 级联折叠退役（<1400 右栏、<1100 左栏不再自动折叠），两切换钮任何 ≥400px 宽度可见可点；断点改 1100/800/400——800–1399 push 双栏可共存、400–799 overlay 浮层（fixed 出流 + 最大宽 2/3 视口 + 左右互斥 + 遮罩点击收起；互斥方向不对称：开右栏挤左栏 = 瞬态旗标回宽自恢复，开左栏挤右栏 = 意图消费不回弹——右栏意图消费点随级联退役移到 800 界，非对称裁定原样保留）；过窄界 800→400 + 应用 minWidth 1024→400（窄窗使用是用户自由，600px 真实可用）；viewportLeftOk/viewportRightOk 旗标退役、overlayMode/tooNarrow/overlaySidebarFold 新三态；互斥收口在 toggleSidebar()/setOpen() 两个唯一写入口（Ctrl+B/J/钮/⌘K/程序化入口零旁路）；§3.40 全节重写 |
| ㊳·fix③ 口述定稿（收缩级联 + overlay 展开带 + 警告层退役） | ✅ v2.112 | 用户口述「拉窄右栏先收、继续拉小左栏收、到 800 两栏全收只剩主会话区、主区最小限位 400、到 400 取消警告——窗口已经限制了还要警告什么、400\~800 展开是悬浮式」：级联沿触发挥复（跨 1400 右栏收+意图消费不回弹 / 跨 1100 左栏瞬态 sidebarAutoFold 回升自展开 / 跨 800 进带全收），带内手动展开不被回弹（沿触发否则按钮可用是假的）；narrow-veil/tooNarrow 旗标/narrow.\* 文案键整体退役；overlaySidebarOpen 浮层展开态 + sidebarCollapsed 分档 getter + toggleSidebar 分档（级联折叠时展开 = 推翻级联）；用户报「主会话区放到下面、侧栏流在上面」根修 = 级联回归后 800 以下侧栏永不可 Dock；§3.40 v2.112 重写（口述原文落档节首）；CDP 22/22 + 450/600 截图 |

### ㊴ 工具层真实测试复测批 + 思考语言设置（✅ 已完成，v2.116——用户 2026-10-06 两份文档：《BUG/Tool Layer Issues Todo List》（整轮工具复测产物）+《NEW/language of thought.md》规格）

| 项 | 落地 |
|---|---|
| P0-1 fetch SideEffect 标错 | ✅ Pure → Irreversible（GET 也对外发起网络请求：服务端留痕、URL 可能带敏感 token；描述补「不是只读查询」；plugins.md §二表同步；tool-fetch 0.1.1） |
| P0-2 git_undo_commit 根提交 | ✅ 三选一裁定取方向①显式拒绝（直面「隐性失败让模型反复尝试」）：`rev-list --parents -n 1` 判父，报错明说 ROOT commit 系设计限制 + 给 `git update-ref -d HEAD` via shell 出路（需用户同意）；git_commit/git_undo_commit 描述写明例外（tool-git 0.1.3；方向②③记档不取） |
| P1-1 git 报错截断 | ✅ require_ok 保留 stderr 末 6 条非空行（原只取末行——push 无远端的 fatal 主因在前两行被截掉，模型只见 "and the repository exists."；共享 helper 全 git 工具受益） |
| P2-1 schedule 多模式 | ✅ 互斥校验落地：描述承诺 exactly one of、旧实现静默优先级 interval > at > delay——多模式同给显式拒绝列字段；start_at 双锚点歧义路徠除（interval 首跑恒 now+N） |
| P2-2 memory_undo 描述失实 | ✅ 删「usually invoked by the compensation layer」劝退措辞（v2.115 fs_restore 同款裁定）+ 审批说法改「跟随会话权限档」（决策表实况：完全档 Irreversible 放行不出卡）；git_undo_commit 同款措辞一并修（memory-sqlite 0.1.4） |
| 环境事实 2：shell 是 Unix coreutils | ✅ 根因探明 = tool-shell 无 shell 直 spawn + PATH 解析（非「跑在 Git Bash 里」）；shell/shell_read 描述写明「用 Unix 风格命令，cmd 内建（dir/del/copy、`rmdir /s /q`）不存在」（tool-shell 0.1.3）；跨平台策略维持现状、单独排期记档 |
| 环境事实 1：出站网络非全通 | ✅ 长期文档 docs/design/BUG/环境已知限制.md（LLM/fetch/未来市场三方影响 + 排查口诀「先怀疑网络别怪工具」+ 国内外二分对照；代理配置不做记档） |
| 思考语言设置（NEW 规格全项落地） | ✅ 设置项非插件非中间件：settings.json 新 `thinking` 区（follow_ui 缺省 / zh-CN / en-US / none；serde default 零迁移；白名单校验）+ 模型配置「默认项」组 SettingsSelect 四档行 + 保存 toast「已保存。新设置将在下个会话生效。」（同值重选不提示）；宿主内置 waterfall 监听器 ThinkingLanguageInject 注册在 TreeDirectoryInject 之后 = **system 真尾部**；**会话内锁死**（前缀缓存红线）：session_id 首请求解析并 FIFO 128 缓存（M1 同款），中途改设置当前会话不变、新会话用新配置；跟随界面 = settings.locale + `system` 档 sys-locale 解析 OS 语言；模板 prompts/thinking_language/{zh-CN,en-US,none}.md 编译期 include_str!（memory-tree 同款；适配记录：文档的 llm.toml/运行时读文件改随本项目 settings.json/编译期内嵌惯例） |
| 搭车：organize auto_collapse flaky 根修 | ✅ 必现化后定位 = 测试自身竞速（after_turn 触发的整理任务经 tauri::async_runtime 独立 runtime 与测试读盘竞速，降级重建章节抹 collapsed_at）；夹具 thresholdPct 90 抬整理线（收起/整理两阈值独立）；全集连跑两遍稳定，已记档 4 例 flaky 之一闭账 |
| 测试与产物 | ✅ workspace 全套件绿、desktop 224 绿（+5 thinking_language）×2 稳定复跑、tool-git 11 绿（+1 根提交拒绝）、scheduler 9 绿（互斥拒绝）、clippy 0 双侧、fmt、check_length 零新增超限、pnpm build、四插件产物重建（build_plugins.sh 注释记档）。实机走查 ✅ CDP 第二实例：新行渲染/四档下拉/保存 toast/落盘往返 + 注入 trace 实证（hint_len=33 had_system=true）；**边界记档**：deepseek-flash CoT 语言不服从软指示（thinking 落盘恒英文）且模型自述不可信——效果评估只看 thinking 落盘 |


### ㊵ 工具执行过程展示重构（✅ 已完成，v2.117——用户 2026-10-07《NEW/Tool Execution Process Display.md》规划；纯前端批）

| 项 | 落地 |
|---|---|
| 五列栅格行（红线：列必须对齐） | ✅ 新 ProcessRow.vue 共享列轨（CSS Grid 20/64/1fr/80/24px = 图标/类型/描述/状态/三角），思考行 + 工具行同吃；类型词 tool.type.* 十六档 + 图标分派（icons 新增 brain/gitBranch）；状态克制（脉动+弱化词 / 时长或弱对勾 / 失败 danger 唯一高亮）；三角 hover 淡入恒占位不位移；CDP 测量实证：同组各行五列左缘逐像素一致 |
| 明细人话渲染（红线：不展示原始 JSON） | ✅ 新 lib/tool-detail.ts（按工具类型定制视图模型）+ ToolDetailView.vue：fs_read=代码块组件复用（.md renderMarkdown 围栏注入）/ fs_list=字形列表 / fs_grep=匹配列表 / shell=命令+退出码+mono 输出 / fetch=状态码+摘要 / edit 系=编辑处数+匹配方式枚举翻译 / memory·schedule=标签值行 / subagent=标签值行 / git 与表外=键值兜底；字段名中文标签、枚举中文（budget_exhausted→步数到顶）、字节带单位、时间相对化、路径相对化（剥 Windows 扩展前缀——走查实抓）；「原始数据 ▸」默认收起（+复制调用文本钮）——摘要人话→明细人话→原始数据兜底三层 |
| 子代理紧凑行（红线：不要卡片） | ✅ SubagentCard.vue 大卡片退役删除；subagent_run = 五列栅格一行，展开 = 任务/状态/轮次/工具/摘要标签值缩进行（竖线层级）+「查看完整对话」文本钮 → §3.16 转录页签（⑮ 黑盒边界不变，父侧拿不到不渲染）；行上悬浮钮与副作用徽章退役进明细；组外错误行去卡片底 |
| 折叠逻辑与状态机 | ✅ 零改动（§3.7 相位机 / 默认一律收起含失败态 v2.98 / 默认展开开关） |
| 文档 | ✅ UI shell v4.13（§3.7 过程行形态 / §3.8 重写 / §3.9 / §3.10 紧凑行 / §3.18 三层呈现 / §四流向表 / 组件清单）+ CHANGELOG v2.117 |
| 跟进：层级间距与颜色规格（v4.14，同日口头规格） | ✅ v2.118：三层间距统一 8px（flex+gap，垂直 margin 全撤，旧 8/8/12 退役）；颜色两档明度——层 1 主色 ink-1、层 2+3 统一次要色 ink-2（文字图标同色）；三层背景透明；语义色与计时行 chrome 例外；CDP 实测五处 gap=8px + 两档颜色实测命中（截图 tmp/w117-shots/v414-layers.png） |
| 测试与走查 | ✅ vue-tsc 0、pnpm build 过、check_length OK（纯前端批）；实机走查 ✅ CDP 第二实例真实轮 ×3 + 重放：改造前截图 ×5 vs 改造后 ×8（含子代理「步数到顶」枚举翻译、实时脉动+流式思考、原始数据展开）；栅格对齐 CDP 测量逐像素一致；自检口径记档：结构面零裸 JSON，内容载体（代码块/匹配行/命令输出）例外系 §四渲染表明令 |

---
### v2.120 · Typography 排版标准（ZCode 体系采纳，纯前端批）

| 项 | 落点 |
|---|---|
| 调查结论 | ✅ ZCode 桌面端无定制字体/无打包字体文件（拉丁 Segoe UI、中文雅黑）——观感全部来自排版纪律；CSS 逐字符 fallback 天然实现「中英不同字体」 |
| UI shell 文档先行 | ✅ v4.16：§6.1 ink 语义重写（正文非极值原则）+ §6.5 排版节全节重写（标准主体）+ §3.2·微调 ZCode 档案处补 2026-10-07 采纳注 |
| ① mono 栈补 CJK | ✅ `--font-mono` generic monospace 前接 YaHei UI/YaHei/PingFang SC/Noto Sans CJK SC（ZCode 同款；根治代码块中文注释掉宋体） |
| ② 正文非极值 | ✅ ink-1 暗 #ffffff→#ededed（gray-100）、亮 #1a1c1f→#414141（gray-750）= ZCode neutral-200/700 同档；accent 保持极值拉开一档；ink-2..5 五级不动 |
| ③ 字重收敛三档 | ✅ 400/500/600（520/560 伪精确值退役，渲染零变化）；th 520→500 表头去伪粗；ConfirmDialog/CommandPalette 随收 |
| ④ shiki 语法高亮 | ✅ 新 lib/highlight.ts（惰性单例+25 语言预载+按需动态加载+LRU 300）；github-light/dark 双主题 defaultColor:false CSS 变量模式（主题切换零重渲染）；底色保持 bg-inset 灰阶纪律；增强阶段接线不过 DOMPurify；main.ts 预热 |
| 度量对齐 | ✅ md 标题层级表定稿（此前浏览器缺省裸奔：h1 18/600、h2 16/600、h3-h6 同字号只靠字重 600/500/400）；行高 1.625；中文断行 pretty+auto-phrase；表格 14、行内 code 12、代码块 14 |
| 验证 | ✅ headless Edge + vite 独立验证页 CDP 实测：亮暗双主题 19 断言全过（含 shiki 双主题 token 色、无语言降级、mono CJK fallback）；截图 tmp/w416-dark.png / w416-light.png；门禁 vue-tsc 0、pnpm build 过、lint:style 0、check_length OK |
| 已知边界 | 记档：语言未预载首帧降级自愈 / --font-scale 缩放为规划描述实现固定 px / dist 含语言 chunk 按需加载 |

---
### v2.126 · 对话设置「折叠工具执行过程」（改名+语义反转+明细解耦，纯设置批）

| 项 | 落点 |
|---|---|
| 设置行 | ✅ 设置 → 对话「默认展开工具执行过程」改名「**折叠工具执行过程**」+ 补一行两态说明 hint（zh/en）；`chat.expandToolProcess` 键退役 = 幻影键（字段级缺省零迁移），新 `chat.collapseToolProcess` **缺省关** |
| 语义（用户口述裁定） | ✅ 勾选 = **完整折叠三层**（容器收束收缩为计时行 + 过程组折叠为一行摘要——即旧缺省行为）；关闭（缺省）= **只折叠最里面那层**（轮次保持展开：实时轮 / 收束后 / 历史重放同口径，过程行直接可见，唯一折叠的是工具明细） |
| 明细解耦 | ✅ 工具明细（层 3）恒默认收起、不再接设置开关——`ToolRow` defaultExpanded 入参退役（v2.98「明细默认收起」恢复为无条件铁则） |
| 相位机 + 容器 | ✅ `TurnProcess` 规格层随开关（live 相位不变执行中组体常开；相位回落关 = 保持展开、开 = 折为组头；v2.97 全映射自愈语义不变）；**第 0 层容器同接开关**（缺省开合 = 执行中恒开 / 收束后按开关）——实机走查补线：初版容器不接开关，两档下对话都是一排计时行＝「开关没起作用」（用户实测抓到） |
| 即时生效 | ✅ 组规格层改 computed + 手动覆盖（相位/开关变化丢弃覆盖重落规格层）；开关保存即时作用于已挂载组与容器，无需重启/重开会话 |
| 测试 | ✅ settings 测试随迁 + **旧键幻影测试**（存量 `"chat": {"expandToolProcess": true}` 读得进、新键走缺省关）；desktop 239 绿、clippy --all-targets 0、fmt、vue-tsc 0、pnpm build 过、lint:style 0 |
| 实机走查 | ✅ CDP 独立数据根 + 构造单轮 jsonl（思考×2+工具×2+汇报）**17/17 PASS**：OFF 五断言（行可见/组全开/组头在场/无明细/容器展开）→ 真点开关+保存切 ON 不重启即收（容器收为计时行）→ 重开容器组折叠一行摘要→组头可展开 → 切回 OFF 即时回扩；settings.json 往返盘证；数据根已清，脚本/截图留 tmp/v126-walk/ |
| 文档 | ✅ UI shell v4.20（§3.2 行 / §3.7 语义段重写 / §3.8 铁则 / 设置表 / 版本条）+ CHANGELOG v2.126 |

### v2.162 · 审计修复批 F-12（F15 走 A 案 = 开发态暂存面口径改字面，「回退 = 切回旧版本指针」）

**依据**：《AUDIT-FIX-PLAN-v1》**F15**（v2.155 批 F-5 期间字节级新立）+ 用户 2026-10-11 裁定 **§4 Q9 = A**。本轮只动「构建脚本 + 文档措辞 + 验收叙事」这一类变量：内核 API 契约零破坏、插件行为零改动（改的只有注释），测试数不变。

| 面 | 落点 |
|---|---|
| ① 先核实事实（不照抄审计结论） | **`tool-echo` 与 `tool-shout` 的双版本有真实行为差异**：两个夹具都按内嵌版本串做**运行时**判定（`plugins-src/tool-echo/src/lib.rs` `reverse_supported(v)=v>=REVERSE_SINCE`、tool-shout `repeat_supported()`），所以"回退后 reverse 报错"是可观察的 ⇒ A 案说的"历史版本目录仅保留 tool-echo 这类**机制夹具**"有代码依据，不是文字游戏。反面 = `build_plugins.sh` 里其余 20 个 stage 调用都是**当前 HEAD 重编 + 只换版本串**（审计实测只差 21 字节），且集成测试自己往 `target/locowrk-test/` 暂存、**不吃脚本产物**（`plugins-src/*/manifest.json` 一律还写 0.1.0，版本号真源只在脚本里） |
| ② 脚本面 `scripts/build_plugins.sh` | stage 清单**数据化**为 `PLAN`（10 项 = 9 个 id 各当前版本 + `tool-echo` 双份）+ **开机预校验**：`FIXTURE_IDS` 之外的 id 在清单里出现第二次直接 `exit 1`，报文把出路也写出来（"确需多版本行为差异 = 做成机制夹具：插件侧按内嵌版本串做运行时判定"）；收尾按清单 **prune** 不在册的版本目录。实跑：**清理陈旧 30 份**，终态 `plugins/` = `memory-sqlite{0.1.5} tool-echo{0.1.0,0.2.0} tool-edit{0.2.2} tool-fetch{0.1.2} tool-fs{0.1.6} tool-git{0.1.4} tool-mcp{0.1.0} tool-notify{0.1.0} tool-shell{0.1.8}` |
| ③ 判据（A 侧，Q9 原文要的那条） | 「同 id 不同版本目录的 dll 只差版本串」的共存 = **0 处**（脚本判据：每个 id 目录数必须为 1，`tool-echo` 必须为 2）；脚本幂等 = 第二次运行 prune 计数 0 |
| ④ 措辞面全改字面 | **AGENTS.md**：纪律 8（"旧版本永远保留"限定为注册表内 + 安装态，开发态不成立）+ 常用命令行（版本真源 = PLAN 清单）+ 验收叙事行；**DESIGN.md**：§11.5 之后新立 ⚠ 段（安装态 vs 开发态字节来源）+ 附录 C 纪律 8；**Plug-in packaging.md**：§三 目录布局后加 ⚠（哪个面会真共存）+ §9.4 内置插件段整句重写（原句"运行期回退依赖 registry 内多版本并存"在开发态是假的）；**代码注释五处**：`kernel-bin/src/demo.rs` 头注（这条叙事证明的是版本切换机制 + 插件自维版本闸门，不是"旧二进制被保存"）/ `echo_llm.rs` 脚本注释 / `plugin-host/src/registry.rs` 纪律行 / `desktop/src-tauri/src/commands/plugins.rs` 命令注释 / `plugins-src/tool-echo/src/lib.rs` 夹具自述 |
| ⑤ 用户可见副作用（如实登记） | 开发态内置插件的「回退」入口不再出现：`plugin-helpers.ts` 的 `rollbackTargets` 读 registry 的 `loaded_versions`，此前它列出的正是那些假历史目录 ⇒ **一项 UI 谎报随本批消失**（不是新增缺陷）。装机/市场通路不受影响（`plugin-service` 安装态仍保留历史字节） |
| ⑥ 变异验证（2 条 + 还原） | M1 清单里加第二条 `tool-fs` → `RC=1` 且报文点名，**`==> build` 行数 = 0**（拦在第一个构建之前，不是构建完才抱怨）；M2 摘掉 `FIXTURE_IDS` 豁免 → 同一判据立刻拦 `tool-echo`，证明白名单不是摆设。两条均在**副本**上做（`tmp/`），仓库文件零污染；还原后 grep 残留 0 |
| ⑦ demo 回归（产物重建后复跑） | `cargo run -p kernel-bin -- demo` **RC=0**：phase A `active=0.1.0` plain 回显 → phase B `active=0.2.0（升级；0.1.0 仍在注册表内，回退 = 换回指针）` reverse=`olleH` → phase C `rollback -> 0.1.0` 报 `tool-echo 0.1.0 does not support mode=\`reverse\``；收尾 `tool-echo active=Some("0.1.0") loaded=["0.1.0","0.2.0"]`，事件日志 10 条含三条 `plugin_version` 字段可查 |
| 测试账 | 根 **438** / desktop **369** 持平（本批只改注释，用例数不可能变——门禁仍全量复跑） |
| 门禁（最后一次编辑后全量复跑） | `tmp/v162-gates.sh` 逐步骤单独记 RC：**16 动作全 0 = `GATE_ALL_GREEN`**（根 fmt/clippy **0** + 39 套件 **438**、desktop fmt/clippy **0** + 3 套件 **369**、五道结构闸 `33==33` / `zh=en=891` / `85·85·85` / `10·10·0` / `22 模块 S0…S4 全 0`），含本批新立的四项独有判据 `stage_idempotent=0` / `fifteen_a_zero_fake_history=0` / `mutation_residue=0` / `tmp_copy_leftover=0`。**门禁自己也写反过两处并当场改正**：残留判据把 `docs/` 一起扫 ⇒ 报"残留 2"的正是台账里如实记录的变异串（文档要记录变异内容，不能拿它当残留）；桌面三件初稿写进子 shell ⇒ `FAIL` 传不回父 shell = **桌面全红也会绿**，改走 `--manifest-path` |
| 边界 | ①**B 案（真快照）未被否决，只是未被选**：若今后要"回退 = 拿回旧行为"，必须做到安装态（或按 commit 归档产物），开发态暂存面不承接这件事；②开发态由此**失去"在装机前演练多版本"的能力**——需要多版本时走 `tool-echo`/`tool-shout` 夹具或临时手工复制目录（脚本重跑会把它 prune 掉，这是有意的）；③`PLAN` 里的版本号仍是"叙事版本"（源 `manifest.json` 一律 0.1.0，靠 sed 落成），把它变成源 manifest 真源属另一类变量，本批不动；④批 G 的 `target/` 清理按 Q8 推迟 |

### v2.161（提交 28922cf）· 审计修复批 F-11（第五道结构闸入库 = 拆分等价性守卫 `check-split-equivalence.mjs`）

**依据**：用户 2026-10-11 三项裁定（《AUDIT-FIX-PLAN-v1》§4 Q8/Q9/Q10）：**第五道闸入库** + **F15 选 A 口径改字面** + **批 G 推迟到全批收口后**。本批只做"闸"这一类变量（生产码一行未触、内核契约零破坏、前端零改动）。

| 面 | 落点 |
|---|---|
| ① 为什么要入库 | 批 F-6…F-10 五刀的等价证明全在 `tmp/`，跑完就没人再跑。批 F-10 的驱动自纠 (e) 就是这条边界的注脚：**驱动把整个 `tree.rs` 压成一行，判据照样"红"，红得毫无意义**。证据链不入库 ⇒ 下一次没人会再跑它，也没人会发现它坏过 |
| ② 闸的两种模式 | `scripts/check-split-equivalence.mjs`（**默认 = 形状不变量**，对活代码长期有效，亚秒级）：S0 解析面可靠性自检 / S1 `mod` 声明↔兄弟文件**双向闭合** / S2 测试目录 `mod.rs` 不混住用例 / S3 **私有项被兄弟簇引用**（= `E0425` 的成因，编译器之前点名）/ S4 测试簇名撞被测父模块子模块**且真产生歧义**（`use super::x` 或正文 `super::x::`）。`--split --ledger <json>`（**一次性拆分等价证明**）：七段判据从 `git show <ref>:<原文件>` 取原文重放，含编译器注册表段 |
| ③ 入库的三张登记表 | `scripts/split-ledger/{organize-tests,llm-tests,kernel-tests}.json` = 批 F-8 / F-9 / F-10 三刀的归属表 + 头注模板 + 断言数 + 有意新增行登记表。三份 `--split` 全绿：**19/19·109** / **18/18·108**（含 1 条登记内新增 `use super::wire;`）/ **12/12·44**，段4 未登记与登记未出现两侧均 0；**段7 三份都带 cargo 本地复跑 = `running 19 / 18 / 12`，编译器里查无均 0**。CI 的 hygiene 作业逐个重放（`--no-cargo`，段7 由 cargo 作业覆盖），`checkout` 因此改 `fetch-depth: 0`——浅克隆取不到历史 blob，**这道闸在 CI 上能不能复验，取决于它有没有材料** |
| ④ 接线 | `.githooks/pre-commit` 第 ⑥ 项（暂存面含 `.rs` 即跑，0.28s）+ `ci.yml` hygiene 两步 + AGENTS 命令清单改「**五道结构闸**」 |
| ⑤ 判据自纠六例（本批最值钱，全部落进脚本注释） | (a) **正则剥字符串漏了 raw string**：`r#"{"choices":[{"delta":…}"#` 里的花括号被普通串规则吞掉 ⇒ 批 F-9 原文的 **18 条用例数成 15 条**、断言 108 数成 89 —— 而当时我以为"历史登记表对不上"。⇒ 换**逐字符状态机**（行注释 / **可嵌套**块注释 / 普通串转义 / `r#"…"#` / 字节串 / 字符字面量，且生命周期 `&'a` 不当定界符）。教训 = **用正则逼近一门语言的文法，漏多少种形态就报多少假信号**。(b) **我先把可靠性自检的不变式写反**：写成"剥离前后花括号净额相等"，而剥离的目的正是改变净额（注释里本来就有不配对的花括号）⇒ 一把好尺子被判成 9 个文件"不可靠"。正确不变式 = **剥离后净额必须为 0**（合法 Rust 必配平）。(c) **`mod approval;` 的单元名是裸 `approval`**（批 F-10 教训 (b) 的同族）⇒ 单元模型带 `kw` 字段。(d) **登记表必须双向**：`无归属单元 = 0` 之外补 **`登记表里原文查无的键 = 0`**，于是抓到 v2.158 老 MAP 里的 `MemoryManagerClient`（当年按"仅 trait 名"登记，新口径按完整签名）⇒ 用 `mapExtra`/`mapDrop` 显式适配，**不为了让表过而把闸退回旧口径**。(e) **驱动的残留检查扫 `crates desktop`** ⇒ 走进 `src-tauri/target`（41.7 GB）一跑就是 5 分钟不出结果；这不是"红"也不是"绿"，是**工具自伤的等待** ⇒ 扫面收窄到本批真正被碰过的两个目录。(f) **同一轮里两把尺子指向相反结论时，先怀疑尺子**：段1 报"丢失 3"我第一反应是"历史证据有假"，实际是新闸自己读不懂文件——好在**硬闸「条目数 == 去重单元名数」**（批 F-8 立）第三次生效，它把"解析面不完整"这件事变成了可发现的信号 |
| ⑥ 变异验证账（**6 条**，逐条红在指定计数器） | V0 扫描根为空 → `扫到 0 个目录式模块`（自我保护代码也必须被证明会跑）；V1 摘 `mod prefix;` → S1 = 1；V2 声明不存在的簇文件 → S1 = 1（反向）；V3 往 `llm/tests/mod.rs` 塞一条用例 → S2 = 1；V4 把私有夹具 `Spy` 从共享区搬进单簇 → **S3 ≥ 1**（= 批 F-10 的 M9 形态，**现在不需要编译器参与**）；V5 建一个与被测子模块同名的测试簇 + `use super::wire` → S4 = 1（`E0255` 的成因）。每条 per-case 即时还原 + sha 逐字节，跑完基准回 `SPLIT_SHAPE_OK`，变异串 grep 0 |
| 测试账 | 根 **438** / desktop **369** 持平（本批零代码改动，测试数不可能变——但门禁仍全量复跑证明没有被连带破坏） |
| 门禁（最后一次编辑后全量复跑） | `tmp/v161-gates.sh` 逐步骤记 RC：**14 动作全 0 = `GATE_ALL_GREEN`**；其中新闸两项 = `gate_split_shape=0` 与 `ledger_replay_fail=0` |
| 边界 | ①S3 只在测试目录生效（生产目录里 `impl`/字段名/宏名会让 `\bname\b` 误报，宁可窄而准）；②CI 上 `--split` 跳段7（编译由 kernel/desktop 作业覆盖），本地已带 cargo 复跑；③登记表是**那一刀的快照**，后人若改动产物归属必须同步更新登记（与 `check_length` 的 `BASELINE` 同一机制：闸允许历史形状，但不许悄悄变坏）；④批 F-6/F-7 两刀（生产文件拆分）没有 ledger —— 它们的形状由 S1/S3/S4 长期守护，但"当次等价证明"没有入库；⑤F15 的 A 案执行排批 F-12，批 G 清理按 Q8 推迟 |

---

### v2.160（提交 d4b641e）· 审计修复批 F-10（巨型测试文件目录化第三件 `kernel-agent/src/kernel/tests.rs` 783 行）

**依据**：《AUDIT-FIX-PLAN-v1》§1 F5 余项里的「巨型测试文件目录化」第三件（前两件 = `organize/tests.rs` 随批 F-8、`llm/tests.rs` 随批 F-9）。纯测试面批：**行为零改动**（生产码一行未触、内核契约与前端零改动），**12 条用例与 44 行断言逐条守恒**。

| 面 | 落点 |
|---|---|
| ① 为什么拆 | 783 行里住着 12 条用例横跨三个被测面（审批门 `consult` 范围 / M1 相邻轮前缀一致 / M4 记忆树切流）+ 一堆假件，原文件靠 4 条 `// ----------` 段注释分区。三问自检有两条为「否」（一件事？否；改名即准？否——文件名叫 `tests.rs` 谁都能塞）⇒ 拆 |
| ② 形状与数字 | `tests/mod.rs` **147**（12 条 `use` + 七件跨簇假件连 `impl` 块）+ `approval.rs` **208**(5) / `prefix.rs` **117**(1) / `tree.rs` **350**(6) = **822 行**（净 **+39** 全是簇头注），四文件全守 400 线内 ⇒ `check_length` **34 → 33**。骨架第三次复用（拆分器只换 `MAP`/`HEAD`/`GAP_TO` 三张表）|
| ③ **本批的结构性差别 = 夹具不再全留共享区** | F-8/F-9 的假件都被两用例以上引用 ⇒ 天然留 `mod.rs`；本文件有三簇私有假件（`RecordingResolver`/`ScriptedLlm`/`turn_tools`、`NoEwma`/`TurnOne`、`FakeTrees`/`TextLlm`/`tail_plan`/`echo_kernel`/`OutputShape`）⇒ 按 `tmp/v160-refs.mjs` 的**夹具 × 用例引用矩阵**下沉。代价 = 判据段3 口径重写为四条子判据（集合双向相等 / 归属 = 声明表 / `mod.rs` 无用例 / **跨簇可见性独立复算**），3d 在判据侧另写一遍不复用拆分器 U2 ⇒ **拆分器预闸 + 判据复算 + 编译器 `E0425` 三方对齐**。这条闸拆前就真拦住一次：首版把 `ProbeTool` 判给 `approval`，而共享区的 `specs()` 要构造它 ⇒ 报"夹具被簇外引用"，**把 `ProbeTool` 上收共享区即可，可见性改动仍是 0** |
| ④ 等价判据七段全绿（`tmp/v160-verify.mjs`） | 测试名 **12=12** 丢失/多出/归属错 0；**断言两级守恒 44=44** 且逐条；夹具 **41=41** 丢失/多出/归属错/跨簇不可见全 0；双向逐行字面 = 原文 **716** 非空行未命中 0，产物新增 **24 行全部结构性可指回**、**新增代码行 0 = 登记 0**（本批无任何有意新增代码行——正文 `super::`/`self::` 出现 **0 次** ⇒ 无相对路径漂移面，与 F-9 补的那一行 `use super::wire;` 正好对照）；次序倒挂 0；`mod` 声明闭合 **3=3**；**编译器注册表** `running 12 / 查无 0 / 12 passed` |
| ⑤ 判据与驱动自纠六例（本批最值钱） | (a) MAP 解析器不认 `SHARED` 变量值 ⇒ 只解析 30/53 条目、三段同时空转——抓它的还是批 F-8 那条「解析条目 == 去重单元名」硬闸（**第二次真正生效**）；(b) `mod approval;` 的单元名是裸 `approval` ⇒ 用 `/^mod\s/` 判结构性永不成立，段3 打出"原 41 / 产物 44 / 零多出"的口径矛盾，修 = 单元模型带 `kw` 字段（**这条能被发现靠的是 F-9 教训"显示层不许藏口径差"变成了习惯**）；(c) 断言期望写成实测常量 `origAssert === 44`，防"判据自己那份数一起错而照绿"；(d) `use` 按规则整体归共享区 + `useUnits.length === 12` 硬闸；(e) **驱动把产物写坏**——`join(blk.eol)` 而 `blockOf()` 不返回 `eol` ⇒ `join(undefined)` 按逗号把整个 `tree.rs` 压成一行（M11"红错段"才暴露）：**变异驱动改完文件必须先证明产物还是合法源码，判据"红"不等于"红得有意义"**；(f) `V160_ONLY=M1` 用 `startsWith` 把 M10/M11/M12 一起拖跑 ⇒ 一条驱动 bug 被误读成三条变异同时坏，改精确匹配 + 空匹配 `throw` |
| ⑥ 变异验证账（**12 条**逐条红在指定段） | M1 删断言 → 逐条不一致 1；M2 测试改名 → 丢失 1；M3 摘 `#[tokio::test]` → 产物 11；M4 复制到别簇 → 归属错 1；M5 改期望值 → 未原样命中 1；M6 共享夹具改名 → 丢失 1；M7 塞未登记代码行 → 未登记 1；M8 摘 `mod tree;` → 声明 2 / 文件 3；**M9 把 `Spy` 整块下沉进 `tree`** → **`夹具跨簇不可见引用 = 1`**（新判据，编译器未参与即红）；M10 正文改等价写法 → 未原样命中 1；**M11 同簇内两条测试换序** → **只有段5 红**（其余六段全绿 = 次序判据不是摆设）；**M12 声明不存在的簇文件** → 段6 反向。per-case 即时还原 + sha + 基准回 `VERIFY_OK`；变异串 grep 0 |
| ⑦ 就地订正两处失实自报 | §1 批 F 标题「九个子批已落」按实测标签数 = **11**（F-1 / F-2a / F-2b / F-3…F-10）；批 F-9 的 ROADMAP 文档行自称"标题已改『十个』"而磁盘当时仍是"九个"——**同一条自报两处不一致**。与 F-6 抓的「14 条 `pub use`」同病：自报数没人复算就当真 |
| 测试账 | 根 **438 持平** / desktop **369 持平**；本批在根 workspace 侧 ⇒ 验收跑根全量（39 套件），`kernel::tests` 12 条由编译器名单点名 |
| 门禁（最后一次编辑后全量复跑） | **13 个动作逐个记 RC 全 0 = `GATE_ALL_GREEN`**（`tmp/v160-gates.sh`）：根 fmt CLEAN / clippy `-- -D warnings` **0** / 39 套件 **438 passed 0 failed**；desktop fmt CLEAN / clippy **0** / **369 passed 0 failed**；四道结构闸 = `check_length` **33 == 基线 33**、`check-i18n` 891=891、`check-commands` 85·85·85、`check-middleware-wiring` 10·10·0；判据复跑 `VERIFY_OK`；变异残留 0 / `.mutbak` 0；零 `.ts/.vue` 故未跑前端两件 |
| 边界 | ①纯测试面 ⇒ 未做走查、未跑前端两件；②**巨型测试文件目录化余 6 件**（681 / 638 / 557 / 492 / 421 / 418）；③F5 余两处 = `runner.rs::project_events` **237** / `session-store/src/session.rs` **410**；④四件脚本仍在 `tmp/`（第五道闸待点头）——本批 (e)(f) 两条驱动 bug 正是这条边界的注脚：**证据链不入库，下次没人会再跑，也没人会发现它把文件写坏过**；⑤`prefix.rs` 只 1 条用例（117 行）= 按被测面命名的代价，若该面不再增长可与 `tree` 合并；⑥F15 待裁定 |
| 文档 | 《AUDIT-FIX-PLAN-v1》§1 批 F 标题按实测改「十一批已落」+ **F5 标第六件已落**（余项改列两处 + 巨型测试余 6 件按实测逐个列行数）+ §5 批 F-10 证据段；ROADMAP 本节 + 回补 v2.159 提交哈希 `3d77164`；CHANGELOG v2.160；AGENTS 三条轮转（预算硬闸）；`scripts/check_length.sh` 基线 **33** + 批 F-10 注释（顺手删掉一条重复了三遍的批 F-7 注释） |

---

### v2.159（提交 3d77164）· 审计修复批 F-9（巨型测试文件目录化第二件 `llm/tests.rs` 785 行）

**依据**：《AUDIT-FIX-PLAN-v1》§1 F5 余项里的「巨型测试文件目录化」第二件（榜首 `organize/tests.rs` 已随批 F-8 消失）。纯测试面批：**行为零改动**（生产码一行未触、内核与前端零改动），**18 条用例与 108 行断言逐条守恒**。

| 面 | 落点 |
|---|---|
| ① 为什么拆 | 785 行里住着 4 个共享夹具（`temp_root` / `user_session` / `asset_output` / `tool_msg`）+ **18 条用例横跨四个被测面**（线上协议装配 / SSE 分帧与错误退避 / 端点与设置分发 / 工具结果图片投影），原文件靠 8 条 `// ----------` 段注释分区 = 分区意图已经在注释里、但没有文件边界。三问自检三条全否 ⇒ 拆 |
| ② 形状与数字 | `tests/mod.rs` **86**（夹具 + 12 条 `use` + `mod` 声明）+ `payload.rs` **307**(7) / `transport.rs` **88**(3) / `dispatch.rs` **181**(4) / `assets.rs` **163**(4) = **825 行**（净 **+40** 全是簇头注），五文件全守 400 线内 ⇒ `check_length` **35 → 34** 基线显式收紧。先例同批 F-8（夹具留父模块、子簇 `use super::*;`）⇒ **可见性零改动** |
| ③ **本批新学两条硬约束（F-8 没有的，已落成机器闸）** | **(a) 簇名不得与被测父模块已有的子模块同名**：首跑把该簇叫 `wire`，与 `llm::wire` 撞 ⇒ 子簇正文的 `super::wire::Delta` 解析到测试簇自己，`E0255` + `E0425` 共 12 条。修 = 按**职责**命名（`payload`）+ 拆分器开机对照 `llm/mod.rs` 的 `mod` 名单，撞名直接 `throw`（约束前移到拆之前，不靠 `cargo check` 兜）。**(b) `super::` 是相对路径，模块加深一层语义就变**：`use super::wire::{…}` **不绑定模块名**，用例进子簇后 `super` 从 `llm` 变成 `tests` ⇒ 正文 11 处 `super::wire::…` 全断。两条出路选了代价小的：**`tests/mod.rs` 补一行 `use super::wire;`**，用例本体一字未改（另一条是改 11 处正文，可核对性差得多） |
| ④ 等价判据七段全绿（`tmp/v159-split.mjs` + `tmp/v159-verify.mjs`） | 测试名 **18=18** 丢失/多出/归属错 0；**断言两级守恒** 总数 **108=108** 且逐条；夹具 16→17（**差 1 条由登记表解释**）；双向逐行字面 = 原文 730 非空行未命中 0，新增 25 行 = 结构性 24 + **登记内代码行 1**，未登记 0 / 登记未出现 0；次序倒挂 0；`mod` 声明双向闭合 4=4；**编译器注册表** `running 18 / 查无 0 / 18 passed` |
| ⑤ 判据自纠五例（本批最值钱，逐条落进脚本注释） | (a) **同一份数据在两个脚本里用两种口径**：拆分器提取 `use` 单元名去了行尾分号、`MAP` 键写的是带分号原行，拆分器自己归一化了所以能跑，**判据没归一化** ⇒ 报「无归属单元 = 1」。抓它的正是批 F-8 立的「解析条目 == 去重单元名数 + 无归属单元 = 0」硬闸——**上批立的闸第一次真正生效**。(b) **登记表必须双向**：F-8 只判"未登记 = 0"，本批补"登记了却没出现 = 红"（M9 一摘即红），否则登记表过期就变免检通行证。(c) **显示层不许藏口径差**：段3 打印「原 16 / mod.rs 17 / 多出 0」自相矛盾，改成显式"差 1 条由登记表解释"。(d) 簇名撞名发生在编译期、判据看不见 ⇒ 约束前移到拆分器硬闸。(e) `--write` 只能对同一份原文跑一次：返工前必须 `git checkout HEAD -- <原文件>` + `rm -rf <产物目录>`，基准保护正是为此而立 |
| ⑥ 变异验证账（10 条逐条红在指定段） | M1 删断言 → 逐条不一致 1；M2 测试改名 → 丢失 1；**M3 摘 `#[test]` 属性**（函数还在、编译还过、只是不再是测试）→ 产物 17；M4 复制到别簇 → 归属错 1；M5 改期望值 → 未原样命中 1；M6 夹具改名 → 丢失 1 + 编译红；M7 塞未登记代码行 → 未登记 1；**M8 摘 `mod assets;`** → 声明 3 / 文件 4；**M9 摘本批唯一登记的 `use super::wire;`** → 登记未出现 1 + 编译红；**M10 正文 `super::wire` 改成 `crate::llm::wire`** → 未原样命中（把"用例本体一字未改"钉成机器判据）。per-case 即时还原 + sha + 跑完基准回 `VERIFY_OK`；变异串 grep 0 |
| 测试账 | 根 **438 持平** / desktop **369 持平**；`llm::tests` 子树 18 条由编译器名单点名（段7），不靠脚本数文本 |
| 门禁（最后一次编辑后全量复跑） | **14 个动作逐个记 RC 全 0 = `GATE_ALL_GREEN`**（`tmp/v159-gates.sh`）：根 fmt CLEAN / clippy `-- -D warnings` **0** / 39 套件 **438 passed 0 failed**；desktop fmt CLEAN / clippy **0** / **369 passed 0 failed**；四道结构闸 = `check_length` **34 == 基线 34**、`check-i18n` 891=891、`check-commands` 85·85·85、`check-middleware-wiring` 10·10·0；判据复跑 `VERIFY_OK`；零 `.ts/.vue` 故未跑前端两件 |
| 边界 | ①纯测试面 ⇒ 未做走查、未跑前端三件；②**巨型测试文件目录化余 7 件**（783 / 681 / 638 / 557 / 492 / 421 / 418）；③F5 余两处 = `runner.rs::project_events` **237** / `session-store/src/session.rs` **410**；④三件脚本仍在 `tmp/`（本批 (a) 那条"两脚本两种口径"正说明：判据不入库，下次没人会再跑它）；⑤F15 待裁定 |
| 文档 | 《AUDIT-FIX-PLAN-v1》§1 批 F 标题「十个子批已落」+ 巨型测试文件余 7 件（逐个按实测列行数）+ §5 批 F-9 证据段（含两条新硬约束）；ROADMAP 本节 + 回补 v2.158 提交哈希 `45a636d`；CHANGELOG v2.159；AGENTS 三条轮转（预算硬闸）；`scripts/check_length.sh` 基线 **34** + 批 F-9 注释（含簇名撞名的教训） |

---
---
### v2.158（提交 45a636d）· 审计修复批 F-8（F5 第四项 `organize/tests.rs` 865 行目录化拆分）

**依据**：《AUDIT-FIX-PLAN-v1》§1 **F5 余项里的"巨型测试文件目录化"**（榜首那件 = `organize/tests.rs` 865 行，拆前全仓最长文件）。纯测试面批：**行为零改动**（生产码一行未触、内核与前端零改动），**用例数与断言数一条未变**。

| 面 | 落点 |
|---|---|
| ① 为什么拆 | 865 行里同时住着两类东西：**11 个共享夹具**（假记忆管理器 / 临时数据根 / settings 直写 / 会话与材料构造 / 投影读取）与 **19 条用例**（管道与触发、崩溃重投、窗口溢出与自动收起、观测与编辑、两把检索工具、轮级总结）。三问自检：①不止一件事（夹具与六种被测面混住）②`tests.rs` 概括不了 19 条不同能力的用例 ③改一处观测面要滚过熔断与收起的用例 ⇒ 拆 |
| ② 形状 = 照既有先例，不是新发明 | `tests/mod.rs` **172**（只留共享夹具 + 7 条 `use` + `mod` 声明）/ `pipeline.rs` **104**（4 条）/ `recovery.rs` **45**（2 条）/ `overflow.rs` **157**（3 条）/ `views.rs` **153**（4 条）/ `tools.rs` **177**（4 条）/ `summary.rs` **98**（2 条）= **906 行**（净 **+40**，增量全是簇头注），七文件全守 400 线内 ⇒ `check_length` **36 → 35** 基线显式收紧。**先例 = `settings/tests/`**（v2.58 起就是"夹具放父模块、子簇 `use super::*;`"）⇒ **本批可见性零改动**，与批 F-7 相反：那里搬的是 `impl` 成员、必须提升 5 处；这里搬的是顶层项归属、夹具仍在各簇的父模块里（父模块私有项对后代可见） |
| ③ 本刀的等价判据（`tmp/v158-split.mjs` + `tmp/v158-verify.mjs` 七段） | 段1 测试名集合 **19=19**、丢失/多出/**归属错** 全 0；段2 **断言守恒两级** = 总数 **109 = 109** 且**逐条**（19 条各自的断言行数全等 ⇒ 删断言、并用例、把断言挪进别条都会露出来）；段3 夹具/类型/`use` **20=20** 且 `mod.rs` 内测试 0（夹具与用例没混住）；段4 **双向逐行字面** = 原文 801 非空行未命中 0，产物新增 28 行**全部**落在三类允许来源之外为零；段5 簇内次序倒挂 0；段6 `mod` 声明与文件集合双向闭合 6=6；段7 **编译器视角的测试注册表** = `running 19 / 名单 19 / 查无 0 / 19 passed 0 failed`（**不用 `--exact`**，前缀精确匹配必空集 = 会把"一条没跑"报成"全等"） |
| ④ 判据自纠五例（全部落进脚本注释，逐条可查） | **(a) 判据输入解析只命中 7/39，而下游三段同时打印"零偏差"**：`MAP` 键大多不带引号（`FakeClient: 'mod',`），解析正则只认带引号形 ⇒ `claimed` 近乎空，段1 归属 / 段5 次序 / 段7 名单**同时空转**。修 = 正则兼容两形 + **硬闸「解析条目 == 去重单元名数，不等即宣布三段作废」**；这次是硬闸自己叫的，不是人眼。(b) **硬闸自己的期望值算错 ⇒ 假红**：`struct FakeClient` 与 `impl FakeClient` 同名 ⇒ 键必然去重成 38；正确口径 = 去重单元名数，另设**「无归属单元 = 0」**才是防丢代码的真判据。(c) **调试输出把真因过滤掉了**：失败用例只打印含 `=` 的行，M4 首跑 `exit=-1` 线索为空——真因是复制块起点取错（属性行没带、产物没变）；改成"判据没产出报告就原样打全"。(d) `--exact` 同族第二次咬人（见 ③）。(e) **bash 双引号转义又咬人**：往脚本写 `/\r/g` 被 shell 吃成裸 CR 与 `/^M/g` 两种错形 ⇒ 本轮起**改文件内容一律走 Edit**，bash 只做只读统计 |
| ⑤ 变异验证账（8 条逐条红在指定段） | M1 删一条断言 → `逐条断言数不一致 = 1`；M2 测试改名 → `丢失 1`；**M3 摘掉 `#[tokio::test]` 属性**（函数还在、编译还过、只是不再是测试）→ `丢失 1`；M4 复制到别的簇 → `归属错 1`；M5 体内改期望值数字 → `未原样命中 1`；M6 夹具改名 → `丢失 1` + 编译红；M7 塞一行模板外代码 → `无法指回模板的 1`；**M8 摘 `mod summary;`**（文件在、不参与编译 = 两条测试静默消失）→ `声明 5 / 文件 6`。per-case 即时还原 + sha 逐字节 + 跑完基准回 `VERIFY_OK`；五串变异注入全仓 grep 0 命中、`.mutbak` 0 |
| 测试账 | 根 **438 持平** / desktop **369 持平**；organize 子树 `cargo test --lib organize` = 31 条全绿（含本批 19 条）。**用例数不变本身就是拆分成立的证据**——测试一旦掉出 `mod` 树数量必变，而数量由编译器名单（段7）数出来，不靠脚本文本计数 |
| 门禁（最后一次编辑后全量复跑） | **14 个动作逐个记 RC 全 0 = `GATE_ALL_GREEN`**（`tmp/v158-gates.sh`，比批 F-7 多两项：变异注入串残留、`.mutbak` 锁文件残留）：根 fmt CLEAN / clippy `-- -D warnings` **0** / 39 套件 **438 passed 0 failed**；desktop fmt CLEAN / clippy **0** / **369 passed 0 failed**；四道结构闸 = `check_length` **35 == 基线 35 零新增**、`check-i18n` 891=891、`check-commands` 85·85·85、`check-middleware-wiring` 10·10·0；判据随门禁复跑 `VERIFY_OK`；零 `.ts/.vue` 故未跑 `pnpm build`/`lint:style` |
| 边界 | ①纯测试面 ⇒ 未做走查、未跑前端三件（不产生任何用户可见形态）；②**巨型测试文件目录化余 8 件**（785 / 783 / 681 / 638 / 557 / 492 / 421 / 418），本批只做榜首一件，一轮一类变量；③F5 余两处 = `runner.rs::project_events` **237** / `session-store/src/session.rs` **410**；④拆分器与判据仍在 `tmp/` 未入库（(a) 那种"下游三段同时零偏差"正是判据不入库的风险面，第五道闸待点头）；⑤F15 待裁定 |
| 文档 | 《AUDIT-FIX-PLAN-v1》§1 批 F 标题改「九个子批已落」+ **F5 标第四项已落**（余项改列两处 + 巨型测试文件余 8 件按实测逐个列行数）+ §5 批 F-8 证据段；ROADMAP 本节 + 回补 v2.157 提交哈希 `dd83f5a`；CHANGELOG v2.158；AGENTS 三条轮转（预算硬闸）；`scripts/check_length.sh` 基线 **35** + 批 F-8 注释 |

---
### v2.157（提交 dd83f5a）· 审计修复批 F-7（F5 第三项 `organize/mod.rs` 614 行目录化拆分）

**依据**：《AUDIT-FIX-PLAN-v1》§1 **F5 余项的第一条**。纯结构性批：**行为零改动**（内核、LLM 轨、前端全部未触），桌面单测用例数不变。

| 面 | 落点 |
|---|---|
| ① 为什么拆 | `organize/mod.rs` 614 行里同时住着四件事：**同步触发决策**（含反弹熔断与窗口溢出整理）、**异步执行管道**（超时重试 + 降级规则切分）、**空间急救**（自动收起）、**对模型与界面的告知**（两条系统消息 + 尾部碎片置位）。三问自检：①只做一件事 ✗ ②名字概括不了 ✗ ③改告知要滚过管道 ✗（`notify_done` 与 `pipeline` 在同一文件同一个 `impl` 块）⇒ 拆 |
| ② 拆成的形状 | `mod.rs` **229**（编排器状态与构造 + 投影/台账小工具 + 三个门面入口 + 组装点构造）/ `events.rs` **89**（告知面）/ `trigger.rs` **122**（决策面）/ `collapse.rs` **50**（急救面）/ `task.rs` **196**（管道面）= **686 行**（净 **+72**，增量是各簇头注），五文件全守 400 线内 ⇒ `check_length` **37 → 36**，基线随批显式收紧 |
| ③ 本刀的技术难点 = 拆 `impl` 块 | 与批 F-6 的差别：`impl Organizer` 一个块里 **16 个方法要分到 4 个文件**。拆分器的块模型因此是两层（顶层块 + impl 成员块），成员搬出时包一层 `impl super::Organizer { … }`；Rust 允许同一类型的多个 impl 块分散在子模块，且父模块的私有项对后代可见 ⇒ 留在 `mod.rs` 的 `load_tree`/`anchor`/`record`/`sync_tree_md` 被各簇直接调用，无需改可见性 |
| ④ 唯一一处字面改动 = 五处可见性提升，且逐个证明必要性 | `notify_done`/`notify_paused`（被 trigger + task 调）、`auto_collapse`（被 `mod` 的 `after_turn` 调）、`spawn`（被 `mod` 调）、`run_task`（被 `tests.rs` 调 18 处）由私有 → **`pub(super)`**。论证：私有方法原可见面 = 「定义模块 organize 及其后代」，移到子模块后写 `pub(super)` 的可见面**恰好也是 organize 及其后代** ⇒ 对外一字未增。`pipeline` 只被同簇的 `run_task` 调用 ⇒ **保持私有**（判据实测产物里 `pub(super) fn` 恰好 5 个，多一个即红） |
| ⑤ 等价判据（`tmp/v157-split.cjs` + `tmp/v157-verify.cjs`，七段） | 单元集合 **24 = 24**（顶层 8 + 成员 16）且**归属簇 = 映射表**；**双向对账分两个口径**——正文按**逐行字面**（orig 每一非空行在产物原样命中，丢失 0；产物没被解释的余下每一行必须能指回脚本模板或 `impl` 包裹行 = 余量法，不用"总量差 N 行"那种归不了因的算法），头部按**语句**（`cargo fmt` 会重排 `use {}` 的换行，逐行比会把折行报成来路不明）；簇内**次序不倒挂**；外部 `crate::organize::X` 引用**全部可达**；未透出的搬出项（`OrganizeDoneEvent`/`OrganizePausedEvent`）**全仓零引用**；五处提升**各有簇外调用点**（2/1/1/2/18 处） |
| ⑥ 顺手摘掉两处空转 re-export | 两个事件载荷 struct 全 crate 只有 `events.rs` 自己构造（`grep` 实测外部引用 0，唯一带外引的是 `OrganizeNotifier` = `state/background.rs:90`）⇒ `mod.rs` 只透出 `OrganizeNotifier`。判据第 5、7 段把这个口径变成机器检查：既查"该透出的没透出"，也查"透出了但没人用" |
| ⑦ 判据本次自纠七例（全部落到脚本注释，逐条可查） | **(a) 块模型漏识别**：版正则不认 `async fn` ⇒ `run_task`/`pipeline`（含最大的 139 行）整段不进块集合，**拆完不报错、只静默丢代码**；靠"先只读勘察再动手"抓到（勘察 23 单元 → 修正 24）。**(b) 注释吸附不认缩进**：`ATTACH` 写死行首 ⇒ impl 成员的文档注释与 `#[allow]` 属性没跟着块走，悬空注释残留 `mod.rs`，`rustc` 直接 `expected item after doc comment`（编译器先抓住，判据本来也会报重复行）；本批因此**回滚重做一次**。**(c) 判据里写废的分支**：反向对账第一版是"consumed 计数 + `bad` 数组"，而 `bad` 永远是空数组 ⇒ 看着在执行、其实一条都没判。**(d) 空转假绿（本次最危险）**：头部对账正则写成 `(?:pub…use\|mod)`，**裸 `use` 没有分支** ⇒ 四个簇一条语句都匹配不上、`headBad` 恒 0，却照样打印"头部语句全部可解释 = **true**"；是 **N6 变异不红**把它暴露的——若不跑变异，这条断言会长期装作有效。**(e) 变异必须单变量且必须红在指定段**：原 N3「交换两个方法的声明行」把签名也换了 ⇒ 红在"单元丢失"而不是"次序倒挂" = 没测到目标断言，改成整块移动；`where` 最初抓输出里第一个含关键词的行 ⇒ 八条全显示"单元丢失 = 0"，改成抓计数非零那行。**(f) 变异锚点过期**：`notify_paused` 提升后 N7 还按 `    fn notify_paused(` 找 ⇒ 假变异；另一处正则把空格写进可选组**内**，导致 `pub(crate) fn` 一律不匹配。**(g) 基准保护**：第二次 `--write` 会把上一次的**产物**当原文备份回去，判据随即拿产物跟自己对账 = 恒绿 ⇒ 脚本遇到已存在的备份直接拒绝，并加 `--template-only`（第一次跑就撞过：模板导出写在 `!WRITE` 早退之后，该模式永远走不到） |
| ⑧ 变异验证账（8 条逐条红在指定段） | N1 删正文一行 → `正向未命中 = 1`；N2 摘 `OrganizeNotifier` 透出 → `外部引用不可达 = 1`；N3 `plan_manual` 整块前移 → `簇内次序倒挂 = 1`；N4 给 `pipeline` 多加一个提升 → `提升无理由: task pipeline`（总数 6）；N5 把 `run_task` 降回私有 → 总数 4；N6 头部塞模板外的 `use` → `头部来路不明的语句 @ collapse`；N7 正文伪造一行 → `无法解释的新增行 = 1 ['let _injected_v157 = 1;']`；N8 整块删 `notify_paused` → `单元丢失 = 1`。每条 per-case 即时还原 + `sha` 逐字节核对，八条跑完基准回绿；注入串与临时 use 全仓 grep 0 命中 |
| 测试账 | 根 **438 持平** / desktop **369 持平**（纯结构批不新增用例；`organize/tests.rs` 865 行一条未改——它 `use super::Organizer` 后直接驱动 `run_task`/`plan_manual`/`plan_auto`/`after_turn`，跨簇可达性由 ④ 的提升与 ⑥ 的透出保证） |
| 门禁（最后一次编辑后全量复跑） | 12 个动作逐个记 RC 全 0 = `GATE_ALL_GREEN`（脚本 `tmp/v157-gates.sh`）：根 fmt `--check` CLEAN / clippy `--workspace --all-targets -- -D warnings` **0** / `cargo test --workspace` 39 套件 **438 passed 0 failed**；desktop fmt CLEAN / clippy `--all-targets -- -D warnings` **0** / **369 passed 0 failed**；四道结构闸全绿 = `check_length` **36 == 基线 36 零新增**、`check-i18n` zh=891 en=891 无静态引用键 0、`check-commands` 85·85·85 预留 0、`check-middleware-wiring` 10·10·0 白名单 0；等价判据随门禁复跑 = `VERIFY_OK`；零 `.ts/.vue` 改动故未跑 `pnpm build`/`lint:style`（口径同 v2.155/v2.156） |
| 边界 | ①**未做真机走查**：本批不产生任何新形态，IPC JSON 形状一行未动（`OrganizeDoneEvent`/`OrganizePausedEvent` 的 `#[serde(rename_all = "camelCase")]` 属性行在块体内逐字命中 ⇒ 由判据证），事件面证据 = 单测里的哑 notifier 断言；②`organize/tests.rs` **865 行**（F5 余项里的"巨型测试文件目录化"）未动，一轮一类变量；③F5 余两项 = `runner.rs::project_events` **237 行**、`crates/session-store/src/session.rs` **410 行**；④判据与变异脚本仍在 `tmp/` 未入库（「第五道闸」待点头）——⑦(d) 那种空转假绿正是"判据不入库就没人再跑"的风险面；⑤F15 待裁定 |
| 文档 | `AUDIT-FIX-PLAN-v1.md`：§1 批 F 标题改「八个子批已落」+ **F5 标第三项已落**（余两项按实测行数）+ §5 批 F-7 证据段（含「文档轮转」一行）；ROADMAP 本节 + 回补 v2.156 提交哈希 `3cdeb20`；CHANGELOG v2.157；AGENTS 三条轮转 = 当前 **3440** + 此前 **1352** + 指针 **1204** = **5996 ≤6000**（`tmp/v157-rot.mjs` 预算硬闸：超限 exit 2 不写盘，本批连压三轮才落；v2.156 全文搬入归档，**条数改双口径并报** = 行首 `* ` 39 条 / 非空条目行 40，因复核实测上批标题「38 条」漏计一条历史无前缀段 ㉚ v2.58）；`scripts/check_length.sh` 基线 36 + 批 F-7 注释 |

---
### v2.156（提交 3cdeb20）· 审计修复批 F-6（F5 第二项 `settings/llm.rs` 目录化拆分 + F12b 最后一支）

**依据**：《AUDIT-FIX-PLAN-v1》§1 **F5 余项的第二条**（682 行的 `settings/llm.rs`）与 **F12b**（v2.155 复核时新立的第四支报错教学）。拆分是**结构性搬用**——外部 API 一字未改、内核契约零改动；F12b 是**零逻辑改动**的一句报文。

| 面 | 落点 |
|---|---|
| ① 为什么拆（三问自检三条全命中） | 原 682 行一个文件做四件互不相干的事：**落盘结构** / **校验规则** / **目录合成** / **IPC 形状**（外加磁盘旧格式迁移）⇒ ①不做一件事 ②名字概括不了 ③改展示合成要滚过落盘结构 |
| ② 拆成的形状 | 目录式子模块 `settings/llm/`：`mod.rs` **38**（只声明 + re-export + 路径下沉说明）/ `policy.rs` **105**（PROVIDERS·保留头表·代理档·`validate_headers`）/ `persist.rs` **337**（落盘态：`LlmConfig`·`ProviderProfile`·`ProviderModel`·`SubagentPin`·`LlmSettings`·`session_window`·`apply_update`）/ `synthesize.rs` **153**（钉死值 ∪ models.dev 目录的合成面）/ `ipc.rs` **86**（四型 IPC 投影与入参）/ `migrate.rs` **43**（旧平面一次幂等）。682 → **762**（净 +80，全是各簇头注的次序与职责契约），**六文件全守 400 线内** |
| ③ 等价判据（本刀唯一的真证据） | 桌面单测对本刀零证明力（顺序与形状错位编译器不看见），判据 = `tmp/v156-split.mjs`（dry/write 两态）+ `tmp/v156-verify.mjs`：**顶层块 31 = 31**、声明行**丢失 0 / 新增 0**（唯一一处已登记改动见 ④）、**块体 634 个非空行在新文件里原样命中 0 丢失**（derive/serde 属性行也在块体内 ⇒ 属性漂移可被逐字证明）、**簇内相对次序倒挂 0**、`settings.rs` 的 **17 条 `pub use`** 全部在 mod.rs 的 re-export 语句里、五个下沉项**全仓零引用** |
| ④ 唯一一处有意改动 = 可见性提升，并把它写成断言 | `migrate_flat_llm` 原为 `pub(super) fn`；子模块的 `pub(super)` 项被父模块 `pub use` 会撞 **E0364** ⇒ 改 `pub fn`，可见面由 `mod.rs` 的 **`pub(super) use migrate::migrate_flat_llm;`** 收口（super 仍是 `settings` ⇒ **对外可见面不变**）。判据不"放宽"而是把这条登记成白名单 + **三条硬断言**：migrate.rs 声明字面必须等于新版、mod.rs 必须是 `pub(super) use`、全簇只有这一处该形态；任何第三处差异照旧判红 |
| ⑤ 五个空转 re-export 摘掉（不挂 `#[allow]`） | `RESERVED_HEADERS` / `MAX_CUSTOM_HEADERS` / `PROXY_MODES` / `PricingView` / `SourcesView` 原本 `pub`，但 `settings.rs` 从没带它们、`mod llm` 又是私有子模块 ⇒ **无人可达**；保留 re-export 会让 rustc 报 `unused_imports`，而挂 `#[allow]` 续命正是批 A 在清的形态 ⇒ 保持 `pub` 留在簇内、路径下沉一级（`llm::X` → `llm::policy::X`），实测引用点 0 ⇒ 使用面一字未改 |
| ⑥ F12b（报错教学最后一支） | `tool-shell/src/exec.rs::cwd_of` 改前只宣布 `shell \`cwd\` \`{dir}\` is not a directory`。现**分病因**（那是个文件 vs 那里什么都没有）**+ 两条出路**（省略 `cwd` / 先 `fs_list` 上级）。措辞按 `run()` 实况订正：`cwd` 为 None 就是**不设 `current_dir`** ⇒ 写 "the host process's working directory"，**不写与代码不符的 "workspace root"**（《AUDIT-FIX-PLAN-v1》§1 F12b 预写的修法文本里就有这个说法，落时改真）；新增测试 `exec::tests::cwd_denial_names_the_cause_and_points_a_way_out` |
| ⑦ 新立 IPC 键名守卫（编译器不证的那一面改成机器证） | `settings/tests/llm.rs::llm_ipc_json_shape_is_camel_case_end_to_end`：`rename_all = "camelCase"` 是 derive 属性，既有断言读的都是 Rust 字段，**没有一条证明前端吃到的键名**。新测试吃一份完整 camelCase 盘面 JSON，**按层各查各的键集**（顶层 / ProviderView / ProviderModelView / SourcesView / NetworkProxy / SubagentPin，正向缺键 + 反向漏下划线双向）。第一版递归收成全集时摘 `LlmSettingsView` 的 `base_url` 被 `ProviderView` 的 `baseUrl` 遮蔽 ⇒ 三条变异全停在同一个无关键上，**红是红了但报不出是谁掉的**——分层后才有定位能力 |
| ⑧ 变异验证（两条新守卫，9 个动作） | **等价判据 6 条**：M1 删字段行 / M2 摘 re-export 名 / M3 块整体挪前（次序倒挂）/ M4 收口改 `pub use` → 四条见红；M5 抹掉头注里的五个下沉名 → **仍 VERIFY_OK**（证明判据不靠注释喂绿）；M6 簇外加一行 `llm::PROXY_MODES` 真引用 → refs 见红（证明零引用查证有牙齿）。**IPC 键名 6 条**：五个结构体各摘 `rename_all` **且各红在对应层**；NetworkProxy 摘后**不红 = 已确认边界**（`mode`/`url` 两形同拼，键名守卫对它零信息量，摘了也确实无契约后果）。**F12b 3 条**：V1 报文整段退回 `git HEAD` 原文 / V2 病因合并（`is_file` 短路）/ V3 删掉「省略 cwd」出路 → 三条全红。**全部逐字节 sha 还原一致 + grep 零残留**，复绿必须看到 `Compiling`（v2.154 教训）与 `ran=1`（见 ⑩） |
| ⑨ 版本随批 | stage 清单加 tool-shell **0.1.8**（F12b 报文进产物）；全量重跑暂存 = **22 份**（21 + 1），幂等 |
| ⑩ 判据自己又骗人三次（同族教训，逐条记档） | **(a) 假绿**：第 6/7 段用 `\bNAME\b` 在 mod.rs 全文找 re-export，而那五个下沉名写在头注里 ⇒ "未透出 = 0" 是注释喂出来的，改成**只解析 `pub use` 语句**；改完立刻露出真实差集 5 项，再被 IPC 测试里的字符串字面量 `"SourcesView"` 判成引用命中 ⇒ 引用查证改成**剥掉字符串与行尾注释后的代码面**匹配（不剥就会逼后人为了过闸改文案）。**(b) 假红**：变异脚本用 `cargo test <短名> -- --exact`，短名配 `--exact` 过滤成**空集**，"0 passed" 把三条变异全伪装成"守卫没抓到" ⇒ 脚本改为硬断言 `ran≥1`，并新增 `NOT-RAN` 状态。**(c) 污染**：多 case 共用一个文件时把还原全押在外层 `finally`，I2 改过的 `ipc.rs` 带着变异去编后面四条的测试二进制 ⇒ 改成 **per-case 即时还原** + 进入下一条时校验 sha 必须干净。附带一条纪律：**变异必须单变量**——第一版删整行 `#[serde(rename_all = "camelCase", default)]` 把 `default` 一起摘了，炸点跑到反序列化（真红，但不是键名契约的红） |
| 测试账 | 根 workspace 437 → **438**（+1 = F12b 报文测试）；desktop 368 → **369**（+1 = IPC 键名守卫）；断言只增不减，旧用例一条未动 |
| 门禁 | 根 fmt `--check` CLEAN / clippy `--workspace --all-targets -- -D warnings` **0** / `cargo test --workspace` **438 passed 0 failed**；desktop fmt CLEAN / clippy `--all-targets -- -D warnings` **0** / **369 passed 0 failed**；四道结构闸全绿 = `check_length` **37 == 基线 37 零新增**（本批净降一项 ⇒ **基线 38 → 37 显式收紧**，脚本注释同步记账）、`check-i18n` zh=891 en=891 无静态引用键 0、`check-commands` 85·85·85 预留 0、`check-middleware-wiring` 10·10·0 白名单 0；`build_plugins.sh` 重跑一次全绿（22 份，`plugins/tool-shell/0.1.8/manifest.json` 版本串已核）；零 `.ts/.vue` 改动故未跑 `pnpm build`/`lint:style`（口径同 v2.155） |
| 边界 | ①**未做设置页真机 CDP 走查**——本批的 IPC 面证据停在「derive 属性行属块体内逐字命中 + 既有往返测试吃原始 JSON 字面串 + 新分层键名守卫 + 369 绿」，运行期渲染一态未截图（拆分不产生新形态，走查的增量证明有限，但这是取舍不是无证）；②`PricingView` 的键名未进守卫（目录未命中时 `pricing = null`，测它要先注入目录快照，成本高 ⇒ 登记不扩）；③`NetworkProxy` 键名守卫无信息量（⑧ 里 B1 已确认）；④F5 余三项未动 = `organize/mod.rs` **614**、`runner.rs::project_events` **237**、`session.rs`（另巨型测试文件目录化）；⑤判据与变异脚本仍在 `tmp/` 未入库，「第五道闸」仍待点头；⑥F15（开发态历史版本目录不是历史快照）待裁定 |
| 文档 | `AUDIT-FIX-PLAN-v1.md`：**F5 第二项标已落**（余三项按实测行数改写）+ **F12b 标 ✅**（含「workspace root 措辞落时改真」的订正）+ §5 批 F-6 证据段 + **失实自报「14 条 `pub use`」就地订正为实测 17**；`scripts/check_length.sh` 基线 38 → 37 并顺手改正批 F-3 注释的「六簇 / 350 行」为实测「八簇 / 220 行」；`BUG/bug-report.md` 第四支随批转 ✅；ROADMAP/CHANGELOG/AGENTS v2.156 + 本节回补 v2.155 哈希 `a2534a1` |

---
### v2.155（提交 a2534a1）· 审计修复批 F-5（F12 报错教学三处 + F13 market manifest 真源并轨）

**依据**：《AUDIT-FIX-PLAN-v1》§1 批 F 的 **F12** 与 **F13**（两条都是批 E 登记的面）。三处报文都是**零逻辑改动**，F13 是**一条通路换真源**；合计净 **+127/−50**（7 个代码/测试文件 + 2 个构建脚本 + 1 个新源文件）。

| 面 | 落点 |
|---|---|
| ① F12-a 病根比登记更靠上 | 审计按三个工具记（`fs_read`/`fs_grep`/`fs_list`），实测它们共吃**共享基座** `plugin-api/src/fsutil.rs::resolve_existing` 的 NotFound 分支（tool-edit 三处、tool-git 一处也走同一段）⇒ 一处补出路，四个插件同受益。改前真报文（变异复跑时抓到）= ``` `…\no_such_file.txt`: 系统找不到指定的文件。 (os error 2) ```；改后补「Run `fs_list` on the parent directory…（记忆/上一轮里的路径是待验证线索，不是保证）」 |
| ② 措辞的硬约束（写进源码注释） | 新文本**不得含** `"outside the allowed roots"` 子串——那是守卫拒绝的识别码，`fsutil/tests.rs` 靠它区分「越界」与「真找不到」（2026-09 冒烟误读教训）。落点用 "nothing exists at this path under the allowed roots"，注释里写破原因，防后人"统一措辞"把两条报文合并 |
| ③ F12-b 元字符报文 | `tool-shell/src/policy.rs` 的 `METACHARS` 分支从"只宣布禁令"改为**把替身说出来**：搜内容 → `fs_grep`、列目录 → `fs_list`、字面量 → 单引号包住；并讲清病因 = 宿主不经 shell 直 spawn ⇒ 管道不是被禁、是根本不会执行。元字符清单与 `policy.rs:47` 常量逐一对应（**没**顺手写 `( )`，那对面归 proof.rs） |
| ④ F12-c 引号不闭合 | `proof.rs` 从通用 `structural()` 模板（"not a single simple command"，会把模型推向改命令结构）改为自出报文并**点名是哪把引号**（`"` vs `'`，字符取自己跟踪的 `quote` ⇒ 零新增状态）。既有测试把这两条也断言成通用模板短语，随批拆成独立循环 + **反向锁死**「不该再吃通用模板」 |
| ⑤ 变异验证 | 三处报文精确退回改前原文 ⇒ 三条测试各见红（`fsutil::tests::missing_path_…` 报「要指『先 fs_list 上级』」、`policy::tests::structural_denies_remain` 报「报文要指搜索替身」、`proof::tests::dangerous_regression_structural_forms` 在引号循环红）。还原 sha 逐字节 == 变异前，且**这次显式 `writeFileSync` 回写刷 mtime**（v2.154 的 Windows CopyFile 陷阱当场兑现），出现 `Compiling plugin-api` / `Compiling tool-shell` 后才认复绿 |
| ⑥ F13 真源并轨 | `scripts/build_market.sh` 里 36 行 heredoc 整份硬编码 → 改为与 `build_plugins.sh` 同一条通路：新 `plugins-src/tool-shout/manifest.json` 为真源，脚本只 sed 版本号。**tool-shout 此前是十个内置 id 里唯一没有源 manifest.json 的那个**。验收 = 两份暂存 manifest 改前/改后逐行 diff：**只有 description 的 zh/en 两行变化**，version 行与其余字段（含空行与结尾换行）逐字节相同。原先 per-version 的 EN 描述收进真源一句（`repeat` 由产物内 `repeat_supported(version)` 门控，两版本共用更准确） |
| ⑦ 版本随批 | stage 清单加 tool-fs **0.1.6** / tool-shell **0.1.7** / tool-edit **0.2.2** / tool-git **0.1.4**（后两条自身零改动，只因共享基座文本进入产物）；全量重跑暂存 = 21 份，幂等 |
| ⑧ 新立 F15（本批自己拆了自己的台） | 字节级抽查发现：`tool-fs/0.1.5/tool_fs.dll` 与 `0.1.6/tool_fs.dll` **长度相同、逐字节仅 21 处不同**（= 内嵌版本串），且**新报文在两个目录里都在**（旧报文两处都查不到）⇒ `build_plugins.sh` 每次用当前 HEAD 重编所有版本目录，**开发态的"历史版本目录"不是历史行为快照**。连带：demo 叙事证明的是**版本切换机制**不是**行为回滚能力**；「旧版本永远保留」只在安装态成立；**本批四条 bump 在开发态只是叙事标签**（此话已写进 §1 F15 自陈）。修法候选 A（口径改字面 + 开发态只 stage 当前版本）/ B（按 commit 归档真快照，体积复杂度双涨）**待你裁定**，判据也一并登记 |
| 测试账 | 断言全部落在既有用例内（用例数不变）；plugin-host 侧新增两条**端到端**断言 = 经真实 dlopen 的 FFI 面证明模型实际读到的那句（`fs_read` 缺路径含 `fs_list`；`shell` 元字符含 `fs_grep`/`fs_list`/`without a shell`）。unit 证明函数，集成证明句子真的到了模型眼前 |
| 门禁 | 12 个动作逐个记 RC 全 0 = `GATE_ALL_GREEN`（脚本 `tmp/v155-gates.sh` 自身按 v2.154 的教训写：clippy 一律 `-- -D warnings`，`set -o pipefail` + 每步单独记 RC，不再用 `| tail` 把 clippy 的退出码盖掉）。根 fmt `--check` CLEAN / clippy `--workspace --all-targets -- -D warnings` **0** / `cargo test --workspace` 39 套件 **437 passed 0 failed**（与 v2.154 持平——本批断言全部加固在既有用例内，用例数不变）；desktop fmt CLEAN / clippy `--all-targets -- -D warnings` **0** / **368 passed 0 failed**（零改动仍全量复跑）；四道结构闸全绿 = `check_length` **38 == 基线 38、零新增**、`check-i18n` zh=891 en=891 无静态引用键 0、`check-commands` 85·85·85 预留 0、`check-middleware-wiring` 10·10·0 白名单 0；`build_market.sh` 与 `build_plugins.sh` 各重跑一次全绿（market 7 份文件 / plugins 21 份，幂等）；零 `.ts/.vue` 改动故未跑 `pnpm build`/`lint:style`。 |
| 边界 | ①F12 按登记的三支收口，**复核发现同族第四支未覆盖** = `shell_read` 的 `cwd` 非目录报文（`exec.rs:100` "…is not a directory"）也不指路 ⇒ **新立 F12b**（工单写「仍开放的四支」而正文只列三条**不是笔误**，漏写的正是这支——差点误判成数目自相矛盾，回查「对照实测」表才没误判）；同族另 `fs_write` 盲覆盖拒绝、`fs_delete` 目标消失、`fs_grep` 空 query 等报文未逐条审读（按登记面收口，不扩大）；②F15 未修（要动的是口径/存储策略，属裁定不属手艺）；③F13 只并轨**市场**通路，`plugin-api/src/lib.rs` 的 `DOC_EXAMPLE` 那份 tool-shout manifest 是 packaging.md §2.1 规范示例（含被忽略字段与 ja-JP），性质不同未并；④新报文一律英文（工具错误面自 v2.6 统一英文、模型侧读取，不受 i18n 闸约束）；⑤未新增门禁（第五道闸仍待点头） |
| 文档 | `AUDIT-FIX-PLAN-v1.md` §1 批 F 标题改「六个子批已落」+ **F12/F13 标 ✅**（含实测与措辞约束）+ **新立 F15** + §5 批 F-5 证据段；`BUG/bug-report.md` Bug 3 状态段随批改真（三支 ✅ + 第四支转 F12b + 「对照实测」表加"上报当时"限定，防人把历史实测记录读成现状）；ROADMAP/CHANGELOG/AGENTS v2.155；`scripts/build_market.sh` 头注与 `build_plugins.sh` stage 注释同步真源口径 |

---
### v2.154（提交 28f80dc）· 审计修复批 F-4（F11c：同一测试二进制内的暂存路径竞争修掉）

**依据**：《AUDIT-FIX-PLAN-v1》§1 **F11c**（v2.153 批 F-3 期间机制确证后新立，本批 ✅）。纯测试面——
`crates/plugin-host/src/` 一行未动、生产码零改动、desktop 与前端零改动；plugin-host 用例
**37 → 38**（唯一新增 = 一条确定性守卫）。标题里的提交哈希按本仓现行做法由下一批回补
（本批已把 v2.146–v2.153 八节的哈希回补进标题）。

| 面 | 落点 |
|---|---|
| ① 病与登记时的差别 | 登记时把 native 侧写成「理论风险面」，**逐条统计调用表后推翻**：同一测试二进制内已有 5 组共享目标（`fs_fetch_plugins` 的 tool-fetch@1.0.0 **×4**、`echo_plugin` 的 tool-echo@0.2.0 ×2、`tools_batch2` 的 tool-fs@0.1.5 ×2 + tool-shell@1.0.0 ×2）⇒ **18 次 `stage()` 里 12 次落在会被别人 truncate 的路径上**。`{pid}` 只隔进程，不隔同一二进制内的并行用例；`fs::copy`（truncate + 写）在 `build_lock` 内、`registry.load()` 的读在锁外 |
| ② 修法 | `stage()` 落点 → **`{run_root}/{id}/{version}/stage-{n}/{artifact}`**（`n` = 进程内 `AtomicU64`）。不把 nonce 拼进版本名：`{version}` 目录名要保持可辨认（与 packaging.md §3.1 布局同形），多一层子目录对 registry 透明（`plugin_dir()` 取父目录，`wasm_plugins` 那条断言原样通过）。**为什么不能走「原子 rename 覆盖」/「内容相同就跳过」**：纪律 2「永不 dlclose」——已装载 dll 一直被映射，第二方无论 truncate 还是 rename-over 都撞 os error 32，只有「一人一份」成立 |
| ③ 顺带（同类变量并刀） | `run_root()` 从 `{pid}` 补成 **`{pid}-{启动纳秒}`**：批 F-2a 在 desktop 侧立过的同一味药（Windows 回收复用 pid ⇒ 读到上一轮残留），根侧 v2.152 漏了。其余 crate 无同类面（`kernel-agent/tests/common` 全内存 mock，`grep target/` 零命中） |
| ④ 必红复现（一次性探针） | 临时 `tests/race_probe.rs`（8 线程 × 200 轮 × 64 KiB）：**改前 脏读 1322 / copy 报错 61 / 1600 → FAILED**（copy 报错原文 = `Os { code: 32, ... 另一个程序正在使用此文件 }` = `wasm_plugins.rs` 历史注释那个症状，两种病形同源一次拿到）；**改后同一探针 0 / 0 → ok**。探针随批删除（单跑造 ~100 MB 副本，与「测试不制造负载」冲突），证据留《AUDIT-FIX-PLAN-v1》§5 批 F-4 段 |
| ⑤ 入库守卫（确定性，不靠运气） | 新 `wasm_plugins.rs::staging_two_calls_never_share_a_target_path`：同一 `(id, version)` 两次 `stage()` 路径必不相等 + 两份内容各自完整。放测试文件而非 `common/mod.rs`（后者的 `#[test]` 会被五个二进制各跑一遍）；用确定性断言而非并发断言（并发就是本题的偶发形态，守卫不能自己也偶发） |
| ⑥ 变异验证 + 一条新陷阱 | 摘掉 `stage-{n}` ⇒ 守卫 **FAILED**（`assert_ne!` 两侧相等，exit 101）= 变异真生效。还原后 sha 与改前逐字节一致却**仍然红**——根因 = **Windows 上 `fs.copyFileSync` 走 `CopyFile` 会连备份的旧 mtime 一起复制 ⇒ cargo 的 mtime 指纹认为源码未变 ⇒ 跑的是变异体编出的过期二进制**。改「读字节 + 同字节 `writeFileSync`」刷 mtime，出现 `Compiling plugin-host` 后复绿。**口径：sha 相等只证明内容，`Compiling` 行才证明运行态**（与批 B「变异要在运行时真生效」同族，这次撞到的是它的还原侧） |
| ⑦ 老红对上号 | 批 F-2b 记的「`fs_fetch_plugins` 4 failed / 1 passed、机制未确证」——该二进制正是 tool-fetch@1.0.0 被四个用例共用的那个，一方 truncate 时其余三方在锁外读 ⇒ **四个一起红**。**未复现**，故只作解释、不作证明；§1 F11 段的「当时未确证」与 §5 批 F-2b 段均已就地订正指向 F11c |
| 测试账 | plugin-host **37 → 38**（lib 21 + 集成 17：2/5/4/3/3）；根 workspace **436 → 437**；desktop **368 持平**（零改动仍全量复跑取证）——以门禁实跑为准 |
| 门禁 | 根 fmt `--check` CLEAN / clippy `--workspace --all-targets -- -D warnings` **0** / `cargo test --workspace` 39 套件 **437 passed 0 failed**（436 → 437 = ⑤ 新增的那条守卫）；desktop fmt CLEAN / clippy `--all-targets -- -D warnings` **0** / **368 passed 0 failed**（本批零改动，仍全量复跑取证）；四道结构闸全绿 = `check_length` **38 == 基线 38、零新增**（改后 `common/mod.rs` 231 行、`wasm_plugins.rs` 134 行都在 400 线内）、`check-i18n` zh=891 en=891 无静态引用键 0、`check-commands` 85·85·85 预留 0、`check-middleware-wiring` 10·10·0 白名单 0；零 `.ts/.vue` 改动故未跑 `pnpm build`/`lint:style`。**同批抓到并修掉门禁脚本自己的一条假绿**：`cargo clippy … -D warnings` 少写 `--` 分隔 ⇒ cargo 报 `error: unexpected argument '-D'` 根本没跑 clippy，而脚本把它接进 `| tail` ⇒ 退出码被 tail 盖住、汇总照常打出 `GATE_FAIL=0`（正撞在本仓既有纪律「跑闸必须 && 串联、中间不套管道」上，这次是它的还原/汇总侧变体）。两侧 clippy 已按正确形式复跑 = 0，**AGENTS.md 门禁命令块同批改对并加成因注释**（原文两处都少了 `--`，照它抄就会静默不跑）。 |
| 边界 | ①**跨进程那半边仍未修、机制仍未确证**（`target/debug/` 产物共享 + `LOCOWRK_PLUGIN_VERSION` 编译期注入的理论窗口）：本批探针不并发跑 cargo，治不到它；不凭猜测造跨进程锁（fs-lock 新依赖或自研锁文件 + 陈旧回收 = 单独口径），三要素报文保留。②`stage-{n}` ⇒ 每次调用留一份完整副本，`target/locowrk-test/` 增长比改前快：本机实测**改前 578 MB → 本批跑完 1.2 GB**（273 个进程根 / 3512 个快照目录），口径仍与批 F-2a「治脏读不治累积」一致，`cargo clean` 一并清（批 G 射程，等你点头）。③守卫锁「路径唯一」而非「并发无竞争」：若今后改成共享路径 + 进程内互斥，守卫会红而实际安全，届时要按具体情况改写断言（这条刻意取保守方向）。④纯测试面批，无界面走查。⑤未新增门禁（与批 F-1/F-2a/F-2b/F-3 同一件待你点头的事） |
| 文档 | `AUDIT-FIX-PLAN-v1.md` §1 批 F 标题改「五个子批已落」+ **F11c 标 ✅**（含落法/实测/陷阱八条）+ §1 F11 段残留项与「未确证」措辞就地订正 + §5 新立批 F-4 证据段；ROADMAP/CHANGELOG/AGENTS v2.154；五个测试文件头注随代码改真（不留领先注释） |

---
### v2.153（提交 584fbf5）· 审计修复批 F-3（组装点 `init_state` 结构性拆分：636 行函数拆成八簇 + 一条「语句零重排」机器判据）

**依据**：《AUDIT-FIX-PLAN-v1》§1 批 F 的 **F5 第一项**（审计 M-4 超限榜首：`desktop/src-tauri/src/state/mod.rs` 698 行、其中 `init_state` 一个函数 636 行）。
生产码语义零改动：**497 条原语句逐条保序、零丢失、零新增行为**（机器判据，见 ③）；desktop 与根测试数**全部持平**。
`state/mod.rs` 699 → **220 行**、`init_state` 636 → **195 行**（纯编排 + `AppState` 字面量）；拆出 **8 个簇文件共 1174 行**，全批净 **+696 行**——**这是一次有意的行数增加**，理由见 ②。

| 面 | 落点 |
|---|---|
| ① 拆分形态 | 新 `state/boot.rs`（启动基座：Context/注册表/事件出口/通知 vtable 钩子/数据根/调度库/统计库/embedding/会话表与启动回放/SettingsStore 含降级/env 注入）、`state/llm_stack.rs`、`state/memory_stack.rs`、`state/waterfall.rs`（七监听器注册序本体）、`state/kernel_stack.rs`（洋葱链 + 审批 + AskHub + Gate + Kernel）、`state/tools.rs`（宿主内置工具 + 两棵目录树 + ToolSource 注册）、`state/plugins.rs`（服务开启 + 对账 + 开发态装载 + 广播 + 停用巡检）、`state/background.rs`（Organizer + 调度引擎 + 在途重投 + 首启导出 + 资产 GC）。**每簇 = `Init`（入参逐字段显名）+ `Stack`（出参字段名 = 原 `init_state` 局部变量名）+ `fn build`**：调用点按名解构，所以下游语句和 `AppState` 字面量**一行都不用改写** |
| ② 为什么值得（不是为行数拆） | `init_state` 是**两个组装点之一**、也是历批提交里被反复"顺手加一段"的地方（v2.131 媒体三件 / v2.142 树工具四件 / v2.144 浏览器两件 / v2.135 G5 一件…）。三问自检：①它只做一件事吗——不，它做了八件事；②名字准确吗——`init_state` 掩盖了八个子系统；③改它是否牵连不相关部分——**是，而且是本刀的全部动机**：加工具要滚到 335 行处、改 GC 要滚到 610 行处，一次误触就是整窗不起。拆完 `tools.rs` 自带 18 参 `Init`，**新增内置工具的改动面从"636 行函数中段"缩到"一个 242 行文件"** |
| ③ 等价判据（本刀最值钱的一项） | 新 `tmp/v153-equiv2.mjs`：**A = `git show HEAD:` 的 init_state 有序语句（497 条）**，**B = 改后 init_state 体把八个调用点就地展开回各簇 `fn build` 体**。判三条：改前有改后无（丢语句）= **0**、保序子序列失败（顺序错乱）= **0**、B 多出的行不属于机械形状 = **0**（多出 16 行全部是 `Stack` 字面量字段名行与两个簇返回值行）。**为什么必须有它**：`state/mod.rs` 原注释明写「桌面单测不引用 state」（为了不让 tao/WebView2 符号进测试二进制）⇒ **368 个单测全绿也证明不了我把句柄接对了**，"顺序契约"这一类缺陷编译器根本看不见 |
| ④ 判据脚本自己的两个 bug（如实记） | ⑴首版只认 `::build(`，漏了 `build_and_provide` → tools 簇整簇没展开，误报「281 条顺序错乱 + 90 条丢语句」；⑵调用块收尾只认 `trim() === "});"`，而 13 字段的 boot 簇收尾是 `} = boot::build(boot::Init { app: app.clone() });`（同行）→ 扫过头吞到 GC 块。**两处都是判据自己错、却把绿的说成红**——若我当时选择"信脚本"就会误判整个拆分失败。教训 = **判据脚本自己也要验**，且窗口法（往后 N 行找匹配）不可靠，改为只认两种块起点（同行调用 / `::Stack {` 头）|
| ⑤ 实机走查（state 无单测的替代证据，两子批各一次共 **20/20 PASS**） | `tmp/v153-walk/walk.mjs`：隔离数据根 + 隔离 WebView2 目录 + CDP 9233 起 debug 实例。判据 **W1 九插件装载**（`plugins loaded` 逐 id 列出）·**W2 `boot: init_state done`**（991/3029/3723ms 三次实测）·**W3 零 panic/零服务不可用/零 ENTRYPOINT_NOT_FOUND**·**W4 窗口真起来**（composer 渲染）·**W5b 真轮第一跳 `search_tools` 返回目录带回 screenshot/browser_open/subagent_run/ask_user**（= `with_builtin` 链在位）·**W5c/W5d 插件工具 `fs_list` 被调到、`plugin_id=tool-fs`、结果里读到夹具文件名**（= 一条调用同时穿过插件装载簇与目录簇，端到端）·**W6 `tool_call` 先于 `tool_result` 落盘**（纪律 4 的事件面）。**走查环境两条教训**：⑴首版把 CDP 的 `about:blank` 当主窗 → DOM 全空、`HTMLTextAreaElement` 取属性描述符抛 `Illegal invocation`；⑵**debug 构建的 `devUrl = http://localhost:5173`**（`tauri.conf.json`），直接跑 exe 时前端没人服务就是空白页——脚本改为先起 vite（`netstat` 核实 5173 空闲，未与用户 dev 实例抢端口）、结束时同 kill；`evaljs` 改为返回 null 而非抛错，否则半途退出漏 `proc.kill()` 把 exe 锁住（历批门禁红的同款坑）|
| ⑥ 顺带 | `llm/mod.rs` 的 embed re-export 行加 `CooledEmbed`（1 行；`tools.rs` 的 `Init` 字段要写 `Arc<CooledEmbed>`，而 `mod embed` 是 llm 的私有子模块——与既有 `pub use embed::{EmbedClient, …}` 同形状，不新开模式）；`cargo fix --lib` 清搬空后变孤儿的 import（两轮：F-3a 六条、F-3b 若干）；`check_length.sh` **BASELINE 39 → 38 并在脚本内记因**（`state/mod.rs` 脱离超限区 = M-4 榜首消失）；四处新簇头注踩了 clippy `doc_lazy_continuation`（编号列表后无空行 = rustdoc 会把下一段当成列表续行），补 `//!` 分隔后归零 |
| ⑦ 本批新抓（根门禁红 → 机制**确证**，未修 = 新登记 **F11c**） | 满跑抓到 `wasm_plugin_staged_and_served_via_registry` 红：报文 `compile: failed to parse WebAssembly module: unexpected end-of-file (at offset 0x0)`，而路径是 `target\locowrk-test\28344\wasm-ping\0.1.0\wasm_ping.wasm`——**pid 分层已在位（F-2b 那层挡的是跨进程）却仍然红**，机制就此钉死为另一层：同二进制的两个测试都往 **同一个** `run_root()/wasm-ping/0.1.0/wasm_ping.wasm` 做 `fs::copy`（truncate + 写），而 `registry.load()` 的读发生在 `build_lock` **之外** → 并发时读到 0 字节/半截文件。`offset 0x0` 正是 truncate 后的形状。**这把批 F-2b 记的「机制未确证」升级为「已确证」**，也推翻了我当时猜的理论窗口（跨进程 cargo 产物竞争）。单跑 2/2 绿、随后 6 轮 `-p plugin-host` 全绿 = 窗口小。**修法 = `stage()` 目标带 nonce**（每次调用独占暂存路径，id/version 注册语义不变），但那是 plugin-host 测试面的一刀，与「desktop 组装点拆分」不是一类变量 ⇒ 登记 **F11c** 未在本批动 |
| 测试账 | desktop 3 套件 **368 持平**（本刀未碰任何被测函数，`state` 本就无单测）；根 workspace 39 套件 **436 持平**（根侧零改动，仍全量复跑取证）|
| 门禁（最后一次编辑后全量复跑） | 根 fmt CLEAN、clippy `--workspace --all-targets -- -D warnings` 0、39 套件 436 passed 0 failed；desktop fmt CLEAN、clippy `--all-targets` 0、368 passed 0 failed；四道结构闸全绿（**`check_length` 38 == 收紧后的基线**、`check-i18n` 891=891、`check-commands` 85·85·85、`check-middleware-wiring` 10·10·0·白名单 0）；等价判据 EXIT=0；走查 10/10（两子批各一轮）。零 `.ts/.vue` 改动故未跑 `pnpm build`/`lint:style`（前端与 LLM 轨零改动）|
| 边界 | ①**净增 +696 行**：八簇各带「次序契约」头注 + `Init`/`Stack` 结构体，这是把隐式知识显式化的代价——不认这笔账就会退回"一个巨型函数最省行"的反面；②F5 其余项未动：`organize/mod.rs` 614 行、`runner::project_events` 237 行、`settings/llm.rs` 682 行、`session.rs` 410 行、巨型测试文件目录式拆分；③`init_state` 仍有 195 行（`AppState` 字面量 40 个字段占大头），进一步压缩要动 `AppState` 形状 = 另一类变量；④等价判据是 **tmp/ 下的一次性脚本**（未入库、未成门禁），它证明的是"这一刀"，不会防住下一刀重排——与「第五道闸」是同一件待你点头的事；⑤簇 `Init` 参数面宽（tools 18 字段、background 21 字段）是**忠实反映原本就存在的数据流**，不是新增耦合；真正收敛它要给启动期句柄做分层容器，本批按「一轮只动一类变量」未做 |
| 文档 | `AUDIT-FIX-PLAN-v1.md` §1 **F5 标 ◐**（本刀 = F5 第一项落，余项排期写明）+ **新增 F11c 条目（机制确证 + 修法 + 未修理由）** + F11 段内把「偶发红未确证」订正为「已确证 = 同进程 stage/load 共享路径」+ §5 批 F-3 证据段；`DESIGN.md` §12.3 加实现备注（原文是 v0.x 伪代码，本批只注明真实模块划分与文件指路，不重写规范正文）；ROADMAP/CHANGELOG/AGENTS v2.153 |

---

### v2.152（提交 fccbd05）· 审计修复批 F-2b（plugin-host 测试夹具单点化 + 暂存目录 pid 分层）

**依据**：《AUDIT-FIX-PLAN-v1》§1 批 F 的 **F8**（五个集成测试各抄一遍样板）与 **F11 的根侧部分 F11b**。
纯测试面：**生产码零改动**（`crates/plugin-host/src/` 一行未动）。已跟踪测试文件 **+44 / −428 = 净 −384 行**，
新夹具模块 `tests/common/mod.rs` 187 行（13 个 `pub fn`）= 全批净减 **197 行**。

| 面 | 落点 |
|---|---|
| ① F8 夹具单点化 | 新 **`crates/plugin-host/tests/common/mod.rs`**，照 `kernel-agent/tests/common/mod.rs` 先例（`mod common;` + 整模块 `#![allow(dead_code)]`，因为每个测试二进制只用到一部分）。**夹具函数定义副本 37 → 0**：`build_lock` ×5 / `workspace_dir` ×5 / `artifact_name` ×5 / `build_and_stage` ×5 / `call` ×5 / `load_activate` ×4 / `env_lock` ×2 / `temp_root` ×2 / `err_msg` ×2 / `echo_error` ×1 / `stage` ×1。`tools_batch2.rs` 与 `tools_batch3.rs` 的夹具区**除一个前缀字符串外逐字相同** = 本项最直白的实物证据 |
| ② 形态合并的判据 | 两处不是简单复制：**`load_activate` 两种签名**（batch 系列的「往调用方 registry 上挂」与 fs_fetch 的「自建 registry 返回元组」）保留为 `load_activate(id, ver, &registry)` + `stage_and_load(id, ver) -> (Arc<Registry>, ToolSource)`，后者内部调前者；**`echo_error(&ToolError)` 与 `err_msg(ToolError)`** 是借用/取值两种形态，合并成 `err_msg_ref(&e)` + `err_msg(e) = err_msg_ref(&e)`（行为逐字不变）。断言消息取信息更全的那份（`cargo build -p {id} ({version}) failed`，batch 版原来不带 version） |
| ③ F11b 暂存目录 | `stage()` 落点从 `target/locowrk-test/{plugin}/{version}/` 改成 **`target/locowrk-test/{pid}/{plugin}/{version}/`**（`run_root()`），并行进程不再写同一条路径（`create_dir_all` + `copy` 非原子，原形状下另一半可能读到写了一半的 dll；Windows 还会报「文件正在使用」——`wasm_plugins.rs` 那条 tmp 注释记的就是这个症状）。**诚实边界**：逐条查过当前调用表，两个二进制请求同一 `(plugin, version)` 的情况**并不存在**（tool-echo 被 `echo_plugin` 用 0.1.0/0.2.0、被 `wasm_plugins` 用 1.0.0；tool-fs 被 `fs_fetch` 用 1.2.1、被 `tools_batch2` 用 0.1.5……），所以这一层是**结构性隔离防今后新增重叠组合**，不是"修好了历批记的偶发红" |
| ④ 偶发红压测与未确证 | `cargo test -p plugin-host` 连续 **8 轮全绿**；再把 `fs_fetch_plugins` 与 `tools_batch2` 两个测试二进制**定向并行 4 轮**（8 个结果）全绿。批内进行过一轮完整跑时抓到一次红（`fs_fetch_plugins` 4 failed / 1 passed，**0.80s 内失败 = 不像 5s 网络超时**），此后再未复现 → **机制未确证**。理论窗口是「`cargo build` 产物固定在共享 `target/debug/`，而 `LOCOWRK_PLUGIN_VERSION` 编译期注入 → 并发构建同一插件不同版本时，先构建方的「build 返回 → copy」窗口可能复制到手」；但**不凭猜测造跨进程锁**（那要单独定口径：fs-lock 类新依赖，或自研锁文件 + 陈旧回收）。替代动作 = 把 `load_activate` 的失败报文从光秃秃的 `"load"` 补成 **`(plugin_id, version, 暂存路径)` 三要素探针**，下次再红能直接定性 |
| ⑤ 新夹具自身的红 | `run_root()` 首版**漏了 `create_dir_all`**，`stage_ping_guest` 先 `fs::write` 再 `stage()` → `NotFound` panic，并连锁把 `build_lock` 打中毒（第二个测试 `PoisonError`）→ **`wasm_plugins` 两个测试当场红**。这条正是「测试是真判据」的实证：夹具层改动没有生产码可看，全靠测试跑出来。修法 = `run_root()` 改为访问即确保存在并写进 doc |
| ⑥ 顺带 | `cargo fix --tests -p plugin-host` 清掉合并后变孤儿的 import（4 个文件自动修，`echo_plugin.rs` 的 `use plugin_api::Tool` 手工删）；五个文件头注随批订正——原注释写「暂存到 `target/locowrk-test/`」和「所有 cargo 调用串行执行（同一把构建锁）」，后者在合并后**只对同一二进制内成立**，注释领先代码是批 E 在罚的形态，一并改为如实措辞并指向 `mod common`；另抓到并修好**上一批留下的数字失实**——归档文件标题写「共 34 条非空条目」而文件里实有 **33** 条（v2.151 批自报数字靠推算 = 多算 1），本批搬入 v2.151 全文后用脚本数出实数 **34** 并按实测登记，AGENTS 指针条同步改 34，并把「今后这类自报数字由脚本数出来再写、不靠推算」写进 AGENTS 写入纪律 |
| 测试账 | 根 workspace 39 套件 **436 passed 0 failed**（`plugin-host` 侧 21 + 2 + 5 + 4 + 3 + 2 = 37 个用例数量不变，夹具换实现）；desktop **368 持平**（本刀未碰 desktop） |
| 门禁 | 根 fmt CLEAN、clippy `--workspace --all-targets -- -D warnings` **0**、436 全绿；四道结构闸全等基线（`check_length` 39==基线零新增——新模块 187 行守 400 内、`check-i18n` 891=891、`check-commands` 85·85·85、`check-middleware-wiring` 10·10·0·白名单 0）；零 `.ts/.vue` 与 desktop Rust 改动故未跑 `pnpm build`/`lint:style` |
| 边界 | ①**偶发红机制未确证**（见 ④），跨进程互斥「构建+复制」整段是待裁定的独立一刀，登记在本批与 `common/mod.rs` 头注 ⚠；②`target/locowrk-test/{pid}/` 会随运行次数累积（`cargo clean` 一并清，属批 G 射程），与批 F-2a 边界①「不治累积」同一口径；③同二进制内并行用例仍共享 `build_lock`，一旦某用例持锁 panic，`lock().unwrap()` 会毒化其余用例——收敛前也是这个形状，本批未扩大改动；④未新增门禁（与批 F-1 ⑤、批 F-2a ④ 同一件待点头的事） |
| 文档 | `AUDIT-FIX-PLAN-v1.md` §1 F8 标 ✅ + F11 标 ✅（desktop F-2a + 根侧 F-2b 两刀合完，残留项如实写）+ §5 批 F-2b 证据段；ROADMAP/CHANGELOG/AGENTS v2.152 |

---

### v2.151（提交 59abd0d）· 审计修复批 F-2a（测试临时目录单点化：把"满跑红单跑绿"的地雷拆掉）

**依据**：《AUDIT-FIX-PLAN-v1》F11（批 B 自己撞出来、**审计完全没覆盖**的一类——它查死代码与重复实现，"测试脚手架脏"只有真跑门禁才现形）。
本子批口径 = **只做 desktop 侧**（F11 的根侧 `wasm_plugins` 共享目录 = F11b、F8 plugin-host 测试样板都未动 → **下一刀 v2.152 批 F-2b 已把这两条收口**，本节保留为当时快照）。
纯测试面改动：**生产码零改动、零行为变化**，判据 = 全量测试在改前改后都绿 + 机器证明没有附带删除。

| 面 | 落点 |
|---|---|
| ① 新入口 | **`desktop/src-tauri/src/testutil.rs`**（`#[cfg(test)] mod testutil;`）：唯一名 = `locowrk-{tag}-{pid}-{纳秒}-{进程内 seq}` 三重保险，`dir_name(tag, nanos, seq)` 抽成**纯函数**（否则"同纳秒靠 seq 分家"这一维根本测不到——快速循环在 Windows 100ns 粒度下完全可能撞，撞了就是两个并行测试互删对方落盘产物，表现为满跑红单跑绿）。两入口按调用方需求分：`work_dir`（已建好、保证空）/ `unique_dir`（**故意不创建**，给「目录缺席 / fail-closed / 缺文件出默认值」那一类）。头注写清「纳秒属于发号不属读时间，故不引 `crate::timeutil`」 |
| ② 收敛面 | 改前 HEAD：**测试面 44 处 `std::env::temp_dir()` 散在 36 个文件**，形态三档——`pid` 且**不先清** 17 处（**地雷本尊**：Windows 回收复用 pid → 本次读到上次残留）/ `pid` + 先清 19 处（不脏读，但同进程同 tag 互踩）/ 带 nonce 8 处（不脏读，目录名每次不同）。改后 **44 → 0**（`grep -rc env::temp_dir()` 测试面只剩 0，唯一实现点在 `testutil.rs`）。同批消掉 4 份**逐字同构**的 `fn temp_dir(tag)`（`instruction_files` / `memory` / `memory_reconcile` / `repo_context` 各抄一遍）与 `tests_session_log` 里那段"为什么要 nonce"的**重复解释**（真源改指 testutil 头注，批 E 立的「同一信息单点化」同款） |
| ③ 明确不动 | `data_root.rs:56` 与 `state/mod.rs:89/494` 的 `temp_dir()` 是**生产码的降级兜底数据根**（`locowrk-data` / `locowrk-degraded-data`，主目录创建失败时聊天不受影响），语义与测试夹具无关，且 `testutil` 是 `#[cfg(test)]` 模块——生产码根本不该引它。脚本层直接按「非测试文件不碰 + 非 `let NAME = temp_dir()` 开头不碰」双闸挡住 |
| ④ 新守卫的变异验证 | 三例全「弄坏 → 见红 → 逐字节还原（sha==baseline）」：①目录名丢 seq 维 → `same_nanosecond_is_split_by_seq` 红 ②`prepared` 不先清 → `a_taken_name_is_cleared_before_it_is_handed_back` 红 ③`unique_dir` 也建目录 → `unique_dir_never_touches_the_filesystem` 红。**第一版测试锁不住 ①**（只测"200 次调用全不同"，去掉 seq 也未必红）——把命名抽成纯函数才让这一维可判，这条写进了 `dir_name` 的 doc |
| ⑤ 事故与检测器 | 批量脚本首版 `L.splice(i, j - i + 1, line)` 有 **off-by-one**：多吃一行，把 `let path = dir.join(..)`、函数返回行 `dir` / `(dir.clone(), Arc::new(..))` 一起删了。**是人眼审 diff 抓到的**，回滚重放时又因「回滚保护名单」把两个已损坏文件排除在外，导致坏状态被带进第二轮——两处编译错误（`implicitly returns ()`）当场暴露。修法 = 把这类核对**机器化**：新 `tmp/v151-nocollateral.mjs` 逐行判「被删的 198 行是否都属于取临时目录这件事」，命中 7 行可疑 → 逐条判读全部合法（3 处返回行合并进单表达式 + 4 行注释迁移到 testutil 头注），其余 191 行零可疑 |
| 测试账 | desktop 363 → **368**（+5 `testutil::tests`，删 0）；根 workspace **436 持平**（本批未碰 `crates/`，仍全量复跑取证）。**关键判据 = 44 处目录取得方式全换、368 个测试仍然绿**：这些测试全部依赖临时目录，换错任何一处语义（该建的没建 / 不该建的建了 / 名字撞车）都会当场反映为红 |
| 门禁 | desktop fmt CLEAN、clippy `--all-targets -- -D warnings` **0**、368 passed 0 failed；根 fmt CLEAN、clippy 0、39 套件 436 passed 0 failed；四道结构闸全等基线（`check_length` 39==39 零新增、`check-i18n` 891=891·无引用键 0、`check-commands` 85·85·85、`check-middleware-wiring` 10·10·0·白名单 0）；零 `.ts/.vue` 改动故未跑 `pnpm build`/`lint:style`。净变化 **desktop +48 / −198 = −150 行**（新模块 154 行 = 实现 71 + 测试 83） |
| 边界 | ①**本入口治"脏读"不治"累积"**：带 nonce 的目录跑完仍留在 `%TEMP%`，要收这条得给每个调用点挂 Drop 守卫（44 处路径会被 move 进 store/闭包，逐点判生命周期），与 F11 的病不是一回事——已写进 `testutil.rs` 头注并登记待裁定，不在本批扩大改动面；②`wasm_plugins` 用 `target/locowrk-test/` 共享目录（历批记的"已知偶发红"即它）= **F11b 未动**，属根侧；③F8 plugin-host 测试样板未动；④**未新增门禁**防「今后又有人手写一份 temp_dir」——现防线只有「本批改完的 grep 对账（测试面 44 → 0）」这一条，它不是持续闸；要长期钉死需第五道闸，与批 F-1 边界⑤同一件待裁定事项；⑤脚本判「同一变量的 remove/create 行」用前缀匹配，`fs::create_dir_all`（文件里 `use std::fs` 的写法）首版漏吸，靠 dry-run 预览发现——批量改造的 dry 必须**带上下文预览**才审得动 |
| 文档 | `AUDIT-FIX-PLAN-v1.md` §1 F11 标 ◐（desktop 侧已落 / 根侧 F11b 待做）+ §5 批 F-2a 证据段 + ROADMAP/CHANGELOG/AGENTS v2.151 |

---

### v2.150（提交 b947038）· 审计修复批 F-1（同源重复实现收敛：一把时钟尺 · 四种截断语义 · 一份缺省端点）

**依据**：《AUDIT-FIX-PLAN-v1》§5 批 F 段（F1 / F2 / F3 / F14 四项，审计 M-1 同族）。子批口径 = **只收敛「同一件事在多处各写一遍」**，
不动性能（F4/F10）、不动结构（F5/F9）、不动跨 crate 基座（F7）——那些各自是独立一刀。纯 desktop + kernel-bin/plugin-host 代码面，
**内核 API 契约零破坏、ABI 零重排、前端零改动、LLM 轨零行为变化**。

| 面 | 落点 |
|---|---|
| ① 时间尺（F1） | 新 **`desktop/src-tauri/src/timeutil.rs`**：`now_ms()` / `now_secs()` / `to_epoch_ms(SystemTime)` 三入口（一把尺两个单位 + mtime 读数同一把尺）。收敛前 desktop 生产码有 **9 份本地定义**（`llm/catalog` u64 / `llm/diagnose` / `media` u128 / `memory` / `memory_reconcile` / `organize/mod` / `organize/tree_tools` / `runner`（唯一用 `unwrap_or_default` 的那份）/ `scheduler/store` `pub(crate)`）**+ 8 处把同一段链内联写开**（`browser/console` / `notifications`×2 / `commands/sessions` / `commands/plugins` / `autodisable` / `usage` / `memory_files` mtime），彼此只差单位与返回类型——实测 `grep -c duration_since` **17 → 0**。`scheduler::store::now_ms` 的三个消费者（`engine.rs` / `tests.rs` / `tools.rs`）改直连 timeutil，**不留转发 re-export**（批 D 刚删过同类冗余 re-export）。`llm/catalog.rs` 的 `loaded_ms: u64` 是既有序列化面，**尺统一不等于换字段类型**：调用点显式 `as u64` ×2。**明确不收的两类**写进头注：纳秒造号（`settings/llm.rs::new_profile_id` + 13 处测试夹具 nonce）语义是"发号"不是"读时间"；`llm/retry.rs::jitter_random` 取 `subsec_nanos` 当**熵源** = 第三种语义 |
| ② 截断尺（F2） | 新 **`desktop/src-tauri/src/text.rs`**：`clip_plain` / `clip_ellipsis` / `clip_trimmed` / `clip_markdown` 四语义。审计抓的形态在这里格外真：**6 份同名 `clip` 实为 4 种互不相同的行为**（加不加 `…` / trim 在哪层 / 换行折不折 / 标记文本长短），同名不同物 = 「顺手误用」的产地。16 个调用点按各自旧行为逐一对位改指（`console` 1 / `notifications` 2 / `export` 3 / `search_titles` 1 / `scheduler/tools` 3 / `subagent_verify` 6），`export.rs` 的 `clip_json` 保留（不同名不同物）。实测 `grep`：**本地 `fn clip(` 定义 6 → 0**，**裸 `clip(` 引用 22 → 0** |
| ③ 缺省端点（F3） | `llm/models.rs` 三个 `DEFAULT_*_BASE` 常量补**唯一真源**声明并写清四处消费者；`llm/probe.rs` 的私有 `DEFAULT_OLLAMA_BASE` 与 `settings/embedding.rs` 的 `pub OLLAMA_DEFAULT_BASE` 删除改吃真源——`embedding.rs` 原注释**自称「与 llm/probe.rs 的探测兜底同源」而磁盘上是两份独立字面量**（批 D 的 D7「注释自称同源而实际已分叉」同款，只是这批还没漂）。搭车抓到同文件另一对：`PROBE_TIMEOUT` 在 `models.rs`（pub）与 `probe.rs`（私有）各写一份 5s，一并收。实测 `grep localhost:11434` **3 → 1**、`const PROBE_TIMEOUT` **2 → 1** |
| ④ 版本号命名（F14） | `kernel-bin/demo.rs` 的 `V12/V13` → `V_OLD/V_NEW`（值早在 v2.56 基准重置时换成 0.1.0/0.2.0，名字还讲 1.2/1.3 = 读代码的人把 "V13" 当真实版本）；`demo4.rs` `V13` → `V_ACTIVE` + 注；`plugin-host/tests/echo_plugin.rs` 的 `v12/v13/p12/p13` → `v_old/v_new/p_old/p_new`（24 处标识符）。`registry/tests.rs` 的 `v12` **不动**——它的值真是 `1.2.0`，名副其实。实测 crates 侧这类标识符 **37 → 2**（余 2 = 新注释里解释旧名的字面量）|
| ⑤ 变异验证 | 两个新模块的等值测试各三例变异，全部「弄坏 → 见红 → 逐字节还原 → sha 校验无残留」：`text.rs`（ellipsis 无条件加 `…` / trimmed 丢 trim / markdown 不折平换行 → 各击中 `every_width_boundary…` 等测试）与 `timeutil.rs`（epoch 前改 `unwrap()` 由回落 0 变 panic / 毫秒尺换微秒 / 秒换算多给 1 秒 → 各击中形状锁与回落锁）。**判据强度**：参考实现是**被删的旧代码原文照抄**（出处文件行号标在注释里），不是手写期望值——所以「换了一条取整路径」「悄悄不加省略号」这类行为漂当场红 |
| 测试账 | desktop 356 → **363**（+4 `text::tests` / +3 `timeutil::tests`，删 0）；根 workspace **436 持平**（本批不改内核契约，`echo_plugin` 重命名后 2/2 仍绿）|
| 门禁 | 根 fmt CLEAN、clippy `--workspace --all-targets -- -D warnings` **0**、39 套件 **436 passed 0 failed**；desktop fmt CLEAN（v2.140 起的每批硬闸）、clippy --all-targets **0**、**363 passed 0 failed**；四道结构闸全等基线 = `check_length` **39 == 基线**（新增四文件全在 400 内：51/164/57/107）、`check-i18n` 891=891·无引用键 0、`check-commands` 85·85·85、`check-middleware-wiring` 10·10·0·白名单 0；本批零 `.ts/.vue` 改动故未跑 `pnpm build`/`lint:style`。已跟踪文件 **+114 / −201 = 净 −87 行**，新模块 +379 行（其中测试 271 行、实现 108 行）|
| 边界 | ①**行尾风格陷阱复现**：`organize/search_titles.rs` 等文件是 **CRLF**，首轮批量脚本用 `lines[i] === "}"` 判块尾找不到（行实为 `}\r`）且 `cat -A` 在本机工具输出里**不显示 `^M`**（`\r` 被输出通道吞掉）——判换行风格只能靠 node 读原始字节；脚本改为 split(`/\r?\n/`) + 按原 eol 写回，改后 `git diff --numstat` 每文件都是小数字（若被整文件翻成 LF 会是数百行）②**脚本半途抛错留半状态**：首版六文件批处理在第 4 个 throw 时前 3 个已写盘（三文件 diff 事后逐行核过内容正确、只是集合不齐），F1 的脚本据此改成「全部在内存变换 + 断言全过才统一写盘」并先跑 dry-run 预览 19 处锚点命中数 ③**变异脚本自身的还原缺陷**：首版用 `replace(反向)` 还原，锚点非唯一时把 `trim` 错插进另一个函数（当场被 sha 校验抓到，手工逐字节复原后改「写回整份备份」重跑三例全 PASS）——登记教训：**还原永远走备份，不走反向替换** ④本批只动 desktop + kernel-bin/plugin-host 测试，F4–F13 各自独立成刀未动 ⑤未新增第五道闸防「字面量再分叉」（加闸属批 B 门禁体系范畴，需你点头；现防线 = `grep` 对账 + 等值测试锁形状）|
| 文档 | `AUDIT-FIX-PLAN-v1.md` §5 批 F-1 段（F1/F2/F3/F14 逐条证据 + 上述三条边界）+ §1 批 F-1 标 ◐（余 F4–F13）+ ROADMAP/CHANGELOG/AGENTS v2.150 |

---

### v2.149（提交 67bd6dc）· 审计修复批 E（权威文档真实化：让下一个 agent 不再被文档骗）

**依据**：《AUDIT-FIX-PLAN-v1》§5 批 E 段（逐条「改前 → 改后」计数与取证都在那儿）。本批**零功能代码改动**，
射程是文档层——判据不是写了多少字，而是**「文档里还有多少句在描述已不存在的机制」**。审计未覆盖而本批
核对时新抓的一类：**同一信息两份表互相分叉**（`ROADMAP §四` 的插件完成度表与 `plugins.md §一`），已单点化。

| 面 | 落点 |
|---|---|
| ① DESIGN.md 归真（E1） | §7.1 洋葱链改真实（原写「实装 Tracing → Timeout → **Retry** → CircuitBreaker」）+ 新增 **waterfall 七监听器注册序**段；§7.2 决策表补 ③项目 deny / ⑤项目 allow 重排七序、row2 删 `sandbox_run`、逐序按 `approval.rs::resolve` 抄实；§8 目录树删 `config/` 块；§11.3 开发态/安装态重写为 dev_scan + lockfile；§3.5 表加「现网接线」列。`grep -c v2.145 DESIGN.md` = 改前 **0** → 改后 **6** |
| ② 插件门面归真（E2/E7） | `plugins/README.md` 整表重写：件数「八件」→ **9 个 id**（`tool-shout` 只进 market、`tool-sandbox` 已退役）、工具名点号 → 下划线、1.x → 最新暂存版本、`fetch` Pure → **Irreversible**、`shell`「白名单 + denylist」→ v2.145 两态、`Compensable` 写成实际形状 `{kind, undo_tool}`，并新列「宿主内置工具不在本表」；`plugins-src/*/Cargo.toml` 九条写死的版本号（1.0.0 / 1.2.0 / 1.3.0 / 1.1.0）全改指 **`build_plugins.sh` 的 stage 清单**（唯一真源），`grep` 计数 **9 → 0** |
| ③ AGENTS.md 瘦身（E4） | 32 条历史流水（195,039 字节；搬出后按新纪律把 v2.147 条从本文件移除，归档里本来就有它，不重复搬入）**逐字原样**搬入 `docs/design/archive/AGENTS-progress-archive.md`（脚本按 `git show HEAD:AGENTS.md` 行区间切，**逐行校验 mismatch = 0**）；`wc -c AGENTS.md` = **206,707 → 22,080 字节（−89.3%）**，从此整份文件在会话注入的 65,536 字节预算内、现行纪律全部可读；写死新**写入纪律**（只留当前 + 前一版 + 归档指针三条，合计 ≤6,000 字节（本批实测 3,572 + 1,191 + 915），被挤出的整条搬走不留残段）。同批订正五处失真：**「定位」行**（原把「模型适配器、沙箱」都算插件——LLM 按 v2.75 是宿主内置服务、沙箱 v2.145 已退役）、权威文档行加「动手前先查 INDEX.md」、目录结构块补 `memory-tree` crate / `INDEX.md` / `archive/` / `scripts/`、常用命令块补批 B 立的四道结构闸与 desktop/前端三件（demo 叙事版本号 1.2.0/1.3.0 → 0.1.0/0.2.0）、**设计纪律 ④⑤⑥ 三条加实况注**（桌面是**每 step 批量落盘**不是 per-event / 磁盘快照从未接线 = RESERVED / `Irreversible` 由「必须审批」改「**默认**审批」并列明五条免审路径）|
| ④ 文档体检入口（E6） | 新 **`docs/design/INDEX.md`**：七节分类（权威规范 / 已兑现设计稿 / 待研究 / 对照留档 / 存档·未采纳 / 工单与台账 / 目录约定）+ 六个状态词汇 + 「新建或退役 md 同批改本表」纪律；`Sandbox Execution Design.md`（**761 行**，审计记 575 = 数字失实已按实测登记）`git mv` 进 `archive/` + 首行三行定性（从未采纳从未实施 + 主题已退役 + 现役安全面指路）；`icon replacement plan.md` 查实**未采纳**（`grep -ri tabler desktop/src` = 0）就地标注 |
| ⑤ 撞号与状态行（E5/E8） | `CHANGELOG.md` 两条互不相关的批共用 `v2.75` → LLM 兼容层条**号沿用**（承载全仓 40+ 处按号引用），前端统一标准条改占空号 **v2.76** 并按日期移到正确位序（改号前逐条判读 47 处 `v2.75` 引用：除被改条外全部指 LLM 批、且该批在别处一律按日期引用 = **改号零牵动**，diff 只 3/3 行）；`README.md` 从「当前状态 = 阶段一」25 行重写为 84 行现役面貌；`market/README.md` 的 manifest 形状从 v1 三键订正为 packaging v2 全量 + 版本基准偏差登记；`BUG/` 两份清单状态行与自家处理记录对账（「待处理」→「已处理 v2.116，唯余 ⏸ 一项」；Bug 3「待上报」→「◐ 部分处理」并写清哪支已修哪两支开放 → 登记 **F12**）|
| ⑥ 退役件就地注（E3/E8） | `UI shell and patterns.md` 四处现状表**只加注不改正文**（幻影四键 / `stabilityRounds` / `session_set_organizer` 已删，判据 = `grep -rn session_set_organizer desktop/` **0** 与 `settings/tests/memory.rs:351-369` 幻影锁）；`plugins.md` **§一 加「现役状态」整列**（19 行逐项 ✅/🔧/◐/❌/⬜ 带依据）+ §二/§三/§四/§六 四处就地注（含 §四 表里「挂载点 = `serial` on `tool/call`」这条**实际不存在的通路**，与批 D 的 §3.5 RESERVED 同源）；`Reliability:755` 与 `Model Capability:154` 的「硬锁/unlockRisky 全保留」加 v2.145 订正注（**CRLF 保持**：改前改后 `crlf=213/213`，注插在 `\r` 之前）|
| ⑦ 新抓：两份表分叉单点化（审计未列） | `ROADMAP §四`「plugins.md 完成度」逐条表与 `plugins.md §一` 是同一信息两份——核对时它已分叉到 **6 处 ✅ 描述已退役/已换形态的东西**（genai 三合一 / shell 白名单+denylist / ctx-compact v2.19 / memory-vector 的 experiences / **tool-sandbox 标 ✅ 而目录已删** / tool-fetch 措辞）。处置 = §四 删掉逐条表，改为「结论（未做的只剩 tool-http、mw-ratelimit、中间件插件化、Playwright 级网页自动化、shell 跨平台策略）+ 六处失效清单 + **今后只改 plugins.md §一**」；`上下文与记忆工作流程报告.md` §1.2/§1.3 按 `state/mod.rs:215-300` 重写为**七监听器现役序**（原表把已退役的 `CtxCompact` 列为现役第 ⑤ 步、漏列 v2.139 三件），注里的行号指路改锚名指路 |
| 门禁 | 根 fmt CLEAN、clippy `--workspace --all-targets -- -D warnings` 0、**39 套件 436 passed 0 failed**（与 v2.148 持平，零回归；`wasm_plugins` 满跑一次偶发红 = **F11 已知并行互踩**，单跑 2/2 绿，与本批零代码改动无关）；desktop fmt CLEAN、clippy --all-targets -- -D warnings 0、3 套件 **356 passed 0 failed**（与 v2.148 持平，零回归）；四道结构闸全绿 = `check_length` 39==基线、`check-i18n` 891=891·缺键 0·无引用键 0、`check-commands` 85·85·85、`check-middleware-wiring` 10·10·0·白名单 0；**本批新增一项自检** = 全仓 markdown 本地链接自检（`tmp/v149-linkcheck.mjs`，73 条本地链接 broken 0；剩下两条命中是审计报告里的正则代码片段被误当链接）|
| 边界 | ①**E4b 未采纳**：`CHANGELOG.md`（186 行 / 418 KB）不拆成目录——它不进会话注入源，拆分无 E4 那种收益，而 150 个文件 + 几十处引用改写是纯成本，检索靠 `grep "^\*文档版本：v2.14"` 已够；仍要拆请单独点头；②`market/tool-shout` 仍 1.0.0/1.1.0 与 v2.56「全部插件 0.1.0 起步」**已知偏差**（夹具保留 1.x 恰好持续覆盖旧 lockfile 条目回退路径），未擅自动产物，待裁定；③INDEX.md 的「已兑现」批次号来自 AGENTS/ROADMAP 自述，未逐批重跑验证；④纯文档批，无 `.ts/.vue` 改动故未跑 `pnpm build`/`lint:style`，也未做界面走查 |
| 文档 | `AUDIT-FIX-PLAN-v1.md` §5 批 E 段 + §1 批 E 标 ✅ + 批 F 段新增 **F12/F13/F14** + ROADMAP §四 重写 + **INDEX.md 新立** + README/plugins·market 两份 README/AGENTS/CHANGELOG/DESIGN/plugins.md/UI shell/两份 NEW 设计稿/工作流程报告/BUG 两份 全随批 v2.149 |

---

### v2.148（提交 1935164）· 审计修复批 D（死配置与哑 ABI：删的删、预留的标注、分叉规则单点化）

**依据**：《AUDIT-FIX-PLAN-v1》§5 批 D 段（逐条证据 + grep 对账在那儿）。**D2 改判转批 H**（查证发现取消信号过不去 C ABI 边界，只接宿主侧 = 造半接线，正是本轮审计在罚的形态；用户对该口径尚未表态，先按最低风险路径诚实化 + 立批）。

| 面 | 落点 |
|---|---|
| ①死配置（D1/Q4） | `config/profile.toml` **删文件** + README/AGENTS 目录条目 + DESIGN §8 目录树与 §11.3 开发态段按真实机制重写 + DESIGN 末 backlog 与 ROADMAP backlog 行撤销 + `kernel-bin` 头注改完成时；改前 `grep profile.toml` = 9 处（README 1/DESIGN 4/ROADMAP 1/AGENTS 2 + 代码注释 1）→ 改后 **0**（历史批次日志与审计/计划文档除外） |
| ②谎报能力（D5） | `host_kv_put` 不再写进读不回来的 `KV`、如实回 `ERR_UNSUPPORTED`（`static KV` + `LazyLock/HashMap` 随删；测试改名并断言 UNSUPPORTED，null 参仍 BAD_ARGS）——审计抓的「回 OK 但数据永不可读」= 谎报成功；`ERR_UNKNOWN_TOOL = -2` 标 **RESERVED**（现网两端都不返回它，未知工具实走 `-3` + JSON 信封；冻结 ABI 数值槽不删） |
| ③分叉规则单点化（D7） | 新 `plugin_api::naming::valid_session_id`（唯一真源，`SESSION_ID_MAX=64`），`session-store/persist.rs` 与 `memory-tree/storage.rs` 两份本地实现删除（memory-tree 的 plugin-api 由 dev 依赖升为正式依赖）；`TriggerInput.anchor_source` 幻影字段删（`AnchorSource` 枚举留 = 桌面观测面板在读）。**变异验证**：规则改回不认点号 → plugin-api / session-store / memory-tree **三处回归锁齐红** → 逐字节还原复绿 |
| ④测试专属 pub 面（D6） | `runtime::deps_of` 删（零消费者）、`lockfile::clear_pending_delete` 删（与 `startup_reconcile` 同语义两份实现）、`registry::graveyard_len` 降 `#[cfg(test)]`（只降 `pub(crate)` 会在无 cfg(test) 的 lib 构建触发 `dead_code`，实测）、`kernel::with_retry_budget` 保留 + 注明「活机制唯一注入口」、cordis-core 四个运行态观测方法保留 + RESERVED（§11.6 明写的 UI 投影面） |
| ⑤未接线簇诚实化（D3/D4/D8） | 磁盘快照 + upcaster 簇标 RESERVED（§9.5/纪律 5 明写、实现与测试在位、磁盘面从未接、恢复真源 = JSONL 逐行回放，并写明接上那天要做的三件事 + ⚠ 勿与内存态 `Session.snapshots` 混）；`parallel`/`serial` 两簇标 RESERVED + **DESIGN §3.5 表加「现网接线」列**（emit/waterfall ✅、parallel/serial 零生产调用点，审计链实际落 waterfall）；`cordis-core → plugin-api` 依赖不动但在 `Cargo.toml` 依赖行旁钉住理由（DESIGN §8 要求 + CI `cargo machete` advisory，防顺手删） |
| 测试账 | 根 workspace 432→**436**（+2 naming / +1 session-store / +1 memory-tree，删 0）；desktop 356 持平；`kv`/graveyard/lockfile 三处旧断言按新语义重写，未丢覆盖面 |
| 门禁 | 根 fmt CLEAN、clippy `--workspace --all-targets -D warnings` 0、39 套件 436 绿；desktop fmt CLEAN、clippy 0、356 绿；`check_length` 39 == 基线；`check-middleware-wiring` 10·10·0·白名单 0；`check-i18n` 891=891、`check-commands` 85·85·85（本批零前端改动，故未跑 `pnpm build`/`lint:style`） |
| 边界 | ①D2 真接线 = 新立**批 H**（内核活动调用登记表 + 停止逐个 cancel + vtable 末尾追加 `call_cancelled` 槽 + 长任务工具轮询 + §5.2 文档同批），一动就是「行为 + ABI」两面，需设计稿先行与实机走查；②`kv_put` 如实回 UNSUPPORTED 后，插件真要用 kv 仍须先定缓冲区所有权协议（§15.1 末尾追加 `kv_free` 槽路径不变）；③`memory-tree::fnv1a` 与 `memlayout::fnv1a64` 两份实现 = 登记批 F 收敛，本轮不动 |
| 文档 | `AUDIT-FIX-PLAN-v1.md` 批 D 段 + §1 新增**批 H** 表 + 批 C/批 D 标题标 ✅ + ROADMAP/CHANGELOG/AGENTS v2.148 |

---
### v2.147（提交 78e3dd9）· 审计修复批 C（两个从未接线的内核中间件摘本体 + 134 条死文案清零）

**依据**：`docs/reports/AUDIT-FIX-PLAN-v1.md` §5 批 C 段（逐条证据在那儿，本节只记结论）。用户裁定 = Q2「`CtxCompact`/`Retry` 删本体」+ Q3「批 C 只删孤儿 i18n 键，`sandbox_run` 残留与三包装保留加注释」。

| 面 | 落点 |
|---|---|
| ①摘本体 | `middleware/retry.rs` 整删（52 行）+ `compact.rs` 332→56 行 + 其测试 `compact/tests/{mod,stability}.rs` 509 行随葬；`lib.rs`/`middleware/mod.rs` 再导出与洋葱序注释**与实际对齐**（审计 H2 抓的「注释宣称有、实现里没有」就此闭合）；死码面净摘 **893 行** |
| ②留活面 | `CompactionOutcome`（旧日志 `ContextCompacted` 重放链在读）+ `estimate_tokens`（全仓唯一粗估尺）留任，新 `compact/tests.rs` 四测锁形状；`WaterfallDecision::Retry`/`RetryBudget`/`with_retry_budget` **保留** = 有意偏离审计 M-22（它们是 `agent/request-error` 重试环的活机制，与被删的洋葱件 `Retry` 同名不同物） |
| ③接线闸收口 | `check-middleware-wiring.mjs` 的 `KNOWN_UNWIRED` **清空**（原登这两条）→ 现跑「实现 10 · 已接线 10 · 未接线 0 · 白名单 0」，此后任何新写忘接线的件当场判红；**变异验证**用临时 `MutateProbe` 探针实抓判红 → 删探针复绿 → 零残留 |
| ④死文案 | 62 个孤儿键 ×zh/en = **134 条**删除（`subagent`/`error` 两命名空间整体收敛）；与台账对账 = 闸为台账超集，多出仅 `chat.newSession`/`turn.thinkingFor` 两个后批新孤儿，9 条「叶名歧义」逐条复核无误删；动态前缀面逐条手查确认**无一个被删键的前缀**存在动态消费；删后 `check-i18n` = zh=891 en=891 · 缺键 0 · 无引用键 0 |
| ⑤按裁定保留 | `sandbox_run` 前端 6 处「历史渲染面」注释在位（批 A+B 已落）；`api.ts` 三包装 + `types.ts` `ProviderModels` 本批补「**预留·零消费者**」注释——并纠正 v2.146 ⑤ 的过度声明（当时只落了 tool-summary/tool-detail 两处，`api.ts` 面并未落）。零消费者为 grep 实证：三包装 `desktop/src` 命中各 1（= 自身定义行）；`llm/probe.rs` 唯一消费点 = `commands.rs:371` 命令本身 |
| 测试账 | 根 workspace 443→**432** passed（删 12 compact + 2 Retry + 1 `tree_session_skips_compaction_over_threshold`，增 4 = 净 −11，逐数对上）；desktop 356 持平零回归 |
| 门禁 | 最后一次编辑后全量：根 fmt CLEAN / clippy `-D warnings` 0 / 39 套件 432 绿；desktop fmt CLEAN / clippy 0 / 356 绿；`pnpm build`(vue-tsc 0)+`lint:style` 0+`check-style` 91 文件干净；`check_length` **39 == 基线 39**（本批净降 1 = 旧 compact 测试 405 行超限文件随葬，基线随批收紧 40→39）；`check-commands` 85·85·85 |
| 边界 | ①本批纯代码/门禁/死文案面，**未做实机 CDP 走查**（判据 = 静态引用三路闭合 + vue-tsc + 全量测试）；裸键名回潮的眼验列入批 V；②删除器首轮用行级状态机被 `en.ts` 折行值里的裸撇号（`provider's`）骗过 → 26 键假「文件里没有」，改与门禁同一套字符级扫描器并先校 953 叶键一致才落盘——**删除工具的解析器必须与门禁同源**，已写进计划稿；③`docs/上下文与记忆工作流程报告.md` 仍把 `CtxCompact` 列为流程第 ⑤ 步，随批 E 文档真实化处理 |
| 文档 | `AUDIT-FIX-PLAN-v1.md` 批 C 段（含对账与变异验证全记录）+ ROADMAP/CHANGELOG/AGENTS v2.147 |

---
### v2.146（提交 b904e9d）· 审计修复批 A+B（用户可见缺陷 + 门禁体系四道新防线）

**依据**：`docs/reports/CODE-AUDIT-v1.md`（2026-10-10 全仓审计，4 子域 + Lead 机械取证）；执行计划与逐条证据 = `docs/reports/AUDIT-FIX-PLAN-v1.md`。**用户 2026-10-10 裁定**：本轮先做批 A+B；H2 `CtxCompact`/`Retry` 删本体（批 C）；批 C 删除范围只勾「59 孤儿 i18n 键」（`sandbox_run` 前端残留与三个死 API 包装保留 + 加注释）；`config/profile.toml` 删文件 + 移除文档描述（批 D）。**P0 复核 = 已修**（`approval.rs` 字节判定 + 整字符步进 + 三条 CJK 回归在位，逐行核过）。

| 面 | 落点 |
|---|---|
| ①裸键名修复（H3b，用户可见） | 补 4 键 ×中英（`common.close` / `settings.proxySavedToast` / `developer.modeMemoryManager` / `developer.modeRule`）+ `settings-index.ts:136` 搜索键改指 `developer.panelTitle`；zh/en 949→**953** 对称，引用-存在性双向零缺键 |
| ②UI 谎报修复（H8，用户可见） | `tool-fetch` manifest `fetch.side_effect` Pure→**Irreversible**（代码 v0.1.1 就改了、manifest 漏改 → 插件卡显示「纯」而每次调用都弹卡）；`tool-shell` manifest 简介与两工具描述同步 v2.145 语义（删「白名单/denylist 硬锁/解锁开关」幽灵，补「无命令级禁令 + EXACT_ARGV 免审面」）；版本随批 tool-fetch **0.1.2** / tool-shell **0.1.6** |
| ③报错教学（M-10） | `policy.rs` `split_command` 返回 `Result<_, SplitError::{UnclosedQuote, Empty}>`——引号不闭合不再冒充「命令为空」（模型原本会去查空串而不是补右引号）；desktop `browser/tools.rs` 绝对路径示例按 `cfg!(windows)` 分支（三平台 crate 不再给非 Windows 模型喂 `F:\…`） |
| ④小垃圾（H3 部分/H6/M-3/C1/C7/M-17/M-25） | 删孤儿整文件 `lib/side-effect.ts`（57 行，副作用分级真源是后端字段）；删 `tool-summary.ts` 的 `fs_edit` 幽灵映射；删两条零使用依赖（`kernel-bin/plugin-service`、`memory-tree` dev `tokio`）；删 `session.rs` 冗余 re-export + `AppState.app` 陈旧 `#[allow(dead_code)]`（字段确有消费者）；订正 6 处腐烂注释（experiences 先例 / compact_cfg / profile.toml / 整理模式透出 / 「三个」监听器实为 7 模块 9 处） |
| ⑤退役残留按裁定保留 | `sandbox_run` 前端 6 处 + `assetGc`/`llmListModels`/`llmProbeWindow` 三包装 = **保留并加「历史渲染面 / 预留」注释**（用户多选只勾 i18n 孤儿键一项） |
| ⑥门禁 CI 补齐（H1） | `ci.yml` 三作业：kernel（根 workspace）/ **desktop**（`windows-latest`：前端 build 供 `generate_context!` 吃 dist + lint:style + desktop fmt/clippy/test，此前**桌面端与前端在 CI 上完全不存在**）/ hygiene（四道脚本闸）；`cargo machete` 以 advisory 进 kernel 作业 |
| ⑦四道新防线（新脚本） | `check_length.sh` 恒 exit 0 的**空转门禁**改基线比较（基线 32→**40**，覆盖面同时补 `plugins-src/` +2 与 `desktop/src/**/*.ts` +6——locales/chat.ts/types.ts/api.ts/tool-detail.ts 长期在盲区；逐文件 `wc` 改 `xargs` 批算，整闸 15s→2.3s）；新 `check-i18n.mjs`（token 级状态机解析，不再犯「正则把 prettier 折行误判成缺键 18 条」；键集双向相等 + 反向缺键 + 动态前缀豁免的无引用报告）；新 `check-commands.mjs`（`#[tauri::command]` ↔ `generate_handler!` ↔ `invoke` 三向，实测 85=85=85）；新 `check-middleware-wiring.mjs`（未接线必须进 `KNOWN_UNWIRED`，接上了不删条目反向判红——**独立复现出审计 H2 的 CtxCompact + Retry**）；`.githooks/pre-commit` 按暂存面触发这四道 |
| ⑧构建脚本（M-5/M-12） | `build_plugins.sh` 暂存幂等化（`cp -R assets` 前先清，修掉每跑一次叠一层 `assets/assets/` 的错位；连跑两次当前 stage 全集嵌套数 = **0**）；收尾文案改由 stage 清单生成（旧硬编码版本行已写出自相矛盾） |
| ⑨附带修的测试地雷（审计未列） | `tests_session_log/mod.rs::tmp_dir` 目录名只带 pid、不先清 → Windows 复用 pid 时读到上次运行残留（实测「满跑红单跑绿」，`.undo.bak` 读出 6 条 vs 期望 4）；按 `memory_files` 先例加纳秒 + 先清。同形态全仓还有 ~20 处 → 登记为计划批 F11（本批不扩大改动面） |
| 变异验证 | 新闸全部按「弄坏→见红→精确还原→查残留」验过：i18n 两例（单边加键 / 引用不存在键）、命令闭合一例（注册表塞幽灵命令 → 同时报两类）、接线闸两例（摘白名单 / 假登白名单）、行数一例（塞 401 行文件）；tool-shell 报文分离断言亦做变异（改回合并形态当场红）。每次还原后 sha 逐字节比对 + grep 零残留 |
| 门禁 | 最后一次编辑后全量复跑（见 CHANGELOG v2.146 行数字）；desktop `fmt/clippy/test` 三件绿、356 测与 v2.145 持平零回归 |
| 边界 | ①CI 三作业的**实跑结果本机无法验证**（Actions 要 push 后才知，desktop 在 Windows runner 上的编译时长未知，退路 = 改 `workflow_dispatch`/定时）；②无引用键报告口径比审计严（我这边 133 条 vs 审计 59 条确证），批 C 删键前须与 `_parts/1-i18n-orphans.md` 取交集而非直接照我的清单删；③本批**未做** 4 个裸键名的中/英界面截图（机制层证据 = i18n 闸 + `pnpm build`）；④`cargo machete` 首版标 advisory，跑绿一次才转硬门；⑤`proof.rs` 的 `unterminated quote` 报文仍走通用模板（出路提示偏弱，同族但轻，留批 F） |
| 文档 | `AUDIT-FIX-PLAN-v1.md`（新立，含裁定登记 + 逐条证据）+ ROADMAP/CHANGELOG/AGENTS v2.146 |

---
### v2.145 · 权限模型重设计（对齐 ZCode：无命令级禁令 + 粘性授权 + 全自动档真全自动）

**用户 2026-10-10 口述裁定**：「我们按照 ZCODE 的学来吧……这个不能用 那个不能开 就连发起子代理 还要申请来授权 权限限制的太死板了。真实做项目的话就没办法使用了。除非用户一直坐电脑边上看着，zcode 的机制就很完美的」+ 问卷两条：sandbox_run **退役**、Auto 档 **ZCode yolo 同款字面全放行**（两道既有硬门保留）+ 追加口谕「设置页终端的分类删掉」。

| 面 | 落点 |
|---|---|
| ①shell 政策层 | `decide()` 收敛两态——Deny 仅剩结构性拒绝（元字符/空命令/引号未闭合），**无命令级禁令**，其余一切命令 Allow 进审批卡（工具 Irreversible 机制不变）；DENY_HARD（约 50 项）/DENY_RISKY+unlockRisky/BASE_ALLOW/LOCOWRK_SHELL_ALLOW 全退役（env 幻影化设置即 warn）；proof.rs 免审面新增 **EXACT_ARGV 精确表**（pwd / git --version / node -v/--version / python[3] --version，ZCode 同款逐 token 全等），git 全局选项守卫零例外；敏感路径/写旗标拒绝保留；版本 0.1.5 |
| ②sandbox 退役 | `plugins-src/tool-sandbox/` 整删 + stage 行删 + 审批越界扫描面移除（ZCode 沙箱拆除同款先例——不隔离 fs/网络即非安全边界，第二入口只多撞锁面）；已存会话钉死集回映射 miss 自然消失 |
| ③决策表 | 新序 ①Plan 硬拒 → ②硬门（隐私+越界，含 Auto 强制卡只许一次）→ **③项目拒绝规则 deny（先于模式放行 = 连 Auto 也拦，安全向偏离 ZCode 记档）** → ④模式放行 → **⑤项目允许规则 allow** → ⑥会话豁免 → ⑦卡 |
| ④粘性规则 | 新 `permission_rules.rs`：数据根 `permissions/<fnv1a-hex8>.json`（ZCode PermissionRuleset 同形；查询直读无缓存 = 人改即生效即撤销出口）；匹配 = 工具级 / `prefix:*` token 边界 / 精确（ZCode 同款）；UI 落「本项目始终允许」（shell=精确命令，未归属跳过持久化）；不产 deny 规则；设置页管理块（计数 + 清除 + 打开文件夹，三新命令） |
| ⑤子代理 | `subagent_run` Irreversible→**Pure**（派生不动用户世界，ZCode Agent 同口径；副产品 = Plan 档只读研究子代理）；**子会话审批改道** `{parent}-sub-{n}`→父根——权限档继承 + 豁免跨派生 + 卡片工作区取父，一修三得（两处既有缺陷：子 id 全新豁免永不命中 / 子档回落设置缺省）；kernel 契约零改动 |
| ⑥前端 | PermissionCard 上弹组扩「本项目始终允许」（真实归属才渲染）；设置页「命令终端」分区整删（ShellSection + unlockRisky 幻影化）+ SettingsPage 删区回落外观兜底 |
| 实机走查 | CDP 隔离数据根 + CM 共享零明文 + deepseek 真轮：W1 `node --version` shell_read 免审直跑（mode:pure，v24.14.0 实回）零卡；W2 python 弹卡 →「本项目始终允许」→ 精确命令规则落盘（逐字段盘证）→ 二轮 **rule:project-allow 免卡直行**（jsonl 铁证 + 截图「无需再次确认」）；W3 派生零卡 + 子内卡挂父会话带徽章（截图）→ 允许本次对话 → 二次派生子内零卡（改道豁免生效）；W4 Auto 档 rm 真跑零弹窗；W5 Auto 档绝对路径 .ssh → 隐私门强制卡「SSH 凭据目录」两键硬门 + 允许一次执行；W6/W7 sandbox 不在加载清单 + 「命令终端」分区消失 + 项目权限规则块在场（截图）。**校准记档**：.env 从不在隐私门清单（v2.26 宁漏勿错杀，Auto 直跑 = 新旧一致）；隐私门 path 类只认反斜杠归一绝对路径（相对漏报设计内）。**环境教训**：种子 settings 需 `keyStorage:"keyring"` 标记 + active 指真实档 id；侧栏树 fsRoots 与旁车 workspace 字符串需一致 + 组默认折叠 |
| 门禁 | fmt 双侧 CLEAN、clippy 根 0 + desktop --all-targets 0、根 workspace 39 套件全绿、desktop 356 绿（+10 新测，政策面旧断言按新语义重写——「permanently locked」时代结束）、vue-tsc 0、pnpm build 过、lint:style 0、check_length **32 < 基线 33**（ShellSection 删除净降一；approval.rs/tests_approval.rs/PermissionCard.vue 既有超限同职责面增长不拆） |
| 边界 | ①粘性 v1 精确优先无智能前缀（无 CLI 注册表；matcher 支持 `:*` 可手改 JSON）；②项目拒绝规则先于 Auto（安全向偏离记档）；③Auto 字面全放行 = 用户显式自担面，两道硬门是唯一拦截；④shell 保持 Irreversible——Ask 档未粘住命令每次弹卡（粘性规则是承接面）；⑤已存会话看不到 sandbox_run；⑥NO_BLANKET_PREFIX 集随批退役，将来前缀建议批回归 |
| 文档 | ✅ 《Permission Model Redesign (ZCode Alignment) v1》（新立+落地标注）+ Setting and Constrained Optimization.md（§2.3/§2.7/§3 修订注——致命级收敛为隐私门/越界门/元字符三项）+ plugins.md（shell 策略修订注 + sandbox 退役注）+ UI shell v4.30（§3.17 上弹组 + 硬门扫描面 + 版本条）+ CHANGELOG/AGENTS v2.145 |

---
### v2.144 · 子代理超时契约修复 + 工作面板浏览器页签

**用户 2026-10-10 口述两条**：①TITY 测试项目实抓 subagent_run 30s 即掐——修；②浏览器参考 ZCode 在右栏加入浏览器入口。

| 面 | 落点 |
|---|---|
| ①超时契约修复 | 病根 = Timeout 中间件（`tools.callTimeoutSecs` 缺省 30s）包裹一切工具调用，`subagent_run` 的 timeout_secs 契约（300 缺省/1800 上限/0 不限，v2.132 批 F）永不可达；修 = `Timeout` 新增**自管时长名单**（内置缺省即 `subagent_run`——工具内部已有预算+步数上限+诚实部分结果三重纪律，全局预算让位；`with_self_managed` 供宿主追加）+ middleware tests +3 |
| ②浏览器页签 | 渲染 = 前端 iframe + 新 `lwpreview://` 协议（fsRoots 围栏 canonicalize 比对 + 逐段编码保相对引用 + 20MB 钳 + MIME 表；CSP 扩 frame-src）；`browser/bridge.rs` text/html 注入 console 桥（error/warn/unhandledrejection → postMessage）；`ConsoleStore` 每签环形 200 条（`browser_report_errors` 增量推入）；双工具 **`browser_open`**（path 渲染型扩展名门 / url http(s)，emit `lw://browser-open`）+ **`browser_console`**（读账本缺省最近签）双 Pure+OnDemand |
| 前端 | WorkPanelTab +browser 种类（工具签 `br-*` / 手动签固定 id 复用）+ `BrowserTab.vue`（自管历史 + URL 栏 + 刷新 + 系统浏览器 + 控制台抽屉带徽章；sandbox 无 top-navigation/popups）+ 浏览器签**常驻 v-show 保活**（ZCode 同款）+ 启动器常备条目 + browser store（环形 + 500ms 节流推后端）|
| 走查实抓 | `preview_url` 吃 canonicalize verbatim 前缀（`\?\`）→ URL 首段 `%3F` 协议侧 400，工具签全错误页——修 = URL 面剥前缀 + 回归锁 |
| 实机走查 | CDP 隔离数据根 + CM 共享零明文 + deepseek 真轮：breakout.html 面板 iframe 完整渲染；切签保活 DOM 节点不换；工具签 URL 干净页面真渲染；console 回环 E2E 三条报文全链通模型原样引用 boom-marker；**子代理 5 文件顺序读 48s 完成**（修复前 30s 必死）零 timeout 落日志 |
| 门禁 | fmt 双侧 CLEAN、clippy 根 0 + desktop 0、根 workspace 458 绿（wasm_plugins 已知偶发复跑即绿）、desktop 347（+25）、vue-tsc 0、build 过、lint:style 0、check_length 33 持平 |
| 文档 | ✅ UI shell v4.29（§2.5 浏览器页签节 + 版本条）+ CHANGELOG v2.144 |

---
### v2.143 · 对话面四件修复（整理发丝线 + 斜杠面板 + 选择器双修 + 测试连接正名）

**用户 2026-10-10 口述四条**：①会话整理从开始到完成的两个卡片式显示改**一根发丝线、整理状态居中盖在线上**（参考「上下文已压缩」分隔线）；②斜杠命令参考 ZCode 命令面板——对话框上方弹出面板，选择/点击**直接触发**；③模型选择弹窗搜索胶囊右边出界 + 选中行对勾把内容挤移位（欢迎页同款一并修）；④设置模型编辑页「测试连接」点了跟「获取列表」一样，应真实测连接。

| 面 | 落点 |
|---|---|
| ①发丝线 | SystemNotice 新 `hairline` 形态（贯穿列宽发丝线两段 + 状态文字居中盖线，[查看] 随文字同组）；chat store 新 `pushOrganizeNotice`——`/整理` 落「正在整理对话上下文…」在途线（tag="organize"），organize-done/paused **就地改写**同一条线，自动整理直接落收束线；旧「已发起整理（影子运行…）」双卡片与 📦 退役。**实抓真缺陷**：发起失败（空会话「没有可整理的尾巴」）在途线永久悬死 → `dropOrganizeRunning` 失败收线 + 错误走原文条 |
| ②斜杠面板 | 新 `lib/slash-commands.ts` 注册表（唯一真源）+ `SlashPanel.vue`：键入 `/` 弹出（「命令」头 + `/名称`+描述 + 底部提示行），键入过滤（name/别名，空态如实）、↑↓ 游标 hover 同步、Enter/Tab 或点击直接触发（不回填文本），Esc 先关面板再走既有 Esc 链；触发走既有 `chat.submit` 拦截，/整理 语义零改动 |
| ③选择器 | ModelSelector 单实例双态同修：搜索胶囊包 `ms-search` padding 容器（病根 = 子组件 `width:100%` scoped 特异性 + `mx` 外边距在 280px 弹层恒右溢 10px）；对勾槽位恒在（非当前行 `visibility:hidden`——病根 = `v-if` 只渲染当前行 ✓，勾出现挤移该行内容）；换选零位移 |
| ④测试连接 | 新 Rust 命令 `llm_test_custom(kind, base_url, api_key, profile_id, model)`：吃**表单当前值**（未保存也可测；Key 留空回落已存 Key 凭据库双读、headers 取已存档位 = 探测同口径）走 `test_model` 最小 ping 五分类；结果独立成行「✓ 连接正常 · {ms}ms」/ 报文红，与清单状态行分离；表单再编辑即清；模型 = 已选清单首项（保存后即档位默认），清空回落已存默认 |
| 走查 | walk1 **20/20**（双态选择器：胶囊 gap=11px + 4 行 4 勾 + 换选行宽逐位相等 94.1→94.1/83.1→83.1；斜杠面板全流程；发丝线在途→就地改写「已整理 1 轮为 1 个章节 [查看]」唯一一根线；失败路径收线实证；deepseek 真轮）+ walk2 **8/8**（「连接正常 · 1694ms」与「已连接，发现 2 个模型」并存分离；错误 Key → 鉴权失败 HTTP 401 真实报文；取消不保存）。隔离数据根 + CM 共享零明文（CM 零写入）；**环境教训**：用户 dev 实例占 5173 → 走查实例 5174 + 独立 `WEBVIEW2_USER_DATA_FOLDER`（共享 WebView2 用户数据目录 = INVALID_STATE 0x8007139F 启动即崩）；tauri.conf.json 临时端口改动已还原 |
| 门禁 | fmt 双侧 CLEAN、clippy 根 0 + desktop --all-targets 0、根 workspace 40 套件全绿、desktop 322 绿、vue-tsc 0、pnpm build 过、lint:style 0、check_length 33 与基线持平（SlashPanel ~150 / slash-commands ~40 / SystemNotice ~100 守线内；Composer/ProviderSetupDialog/commands/llm 系既有超限同职责面增长） |
| 边界 | ①整理通知 live-only 不落日志（切会话/重放消失，与既有口径一致，树文件是持久记录）；②自动整理无在途线，收束线直接出现；③面板现单命令，注册表加行即扩；④测试连接 headers 取已存档位（自定义头草稿不参与，与探测同口径）；⑤与模型行「测试」钮分工：对话框测表单形态、行钮测已存档位 |
| 文档 | UI shell v4.28（§3.3/§3.30/§3.36/§3.37③ + 版本条）+ CHANGELOG v2.143 + AGENTS v2.143 |

---
### v2.142 · 跨会话取回闭环（时间线 = 第一答案面）

**用户 2026-10-10 口述裁定**：①批 1（工具面闭环）照做，批 2（常驻进展行注入）**暂缓**——后期实际验证确实需要时再考虑；②**时间线优先**：最先查到的应该是时间线里的**总结 + 该总结所包含的标题**，这就够了，需要某标题的详细内容再深入追寻。病根 = 树投影按 `(workspace, session)` 隔离存储（有意设计），新会话问「之前做了什么」四处断裂：目录只投影当前会话 / search_titles 只对短标题打分 / **expand 只认当前会话章节**（v2.136 已知边界④）/ 项目总结链只在 UI 模型面零暴露。数据都在盘上，断的全是取回链路。

| 面 | 落点 |
|---|---|
| 下钻 | `memory_expand` 跨会话化：schema 加可选 `session_id`（缺省 = 当前会话，旧调用逐字节兼容），(会话, 章节) 复合寻址；workspace 含入由当前会话解析（目标树必须在本项目 scope 下，跨项目天然拒）；L2 原文按**目标会话**日志 event_range 切片；**`mark_expanded` 只在目标 = 当前会话时写**（expanded_at 是「本会话上下文有展开内容」的记账，跨会话展开 = 纯读，写目标会话的树会让它的自动收起误判）；版本 1.1.0 |
| 搜索 | `memory_search_titles` 索引扩容：打分文本 = 标题 + L1 摘要 + **所在轮总结正文**（章节区间 ⊆ RunSummary [from,to] 即归属；宽查询的语义主体是总结叙事），关键词降级同扩；hits 增 `session_title`；构造函数增 sessions_dir 参；版本 1.1.0 |
| 时间线 | 新工具 `memory_project_history`（Pure + OnDemand）：项目跨会话**时间正序** ×（旁车标题 + 各轮 RunSummary 正文 + 该轮所含章节 `{id,title}` + 无总结覆盖章节落 other_chapters）；标题不带 L1 正文（裁定：总结+标题够了，L1 走 expand level=1）；**钳最近 20 会话** + truncated 如实指路 search_titles；数据源与 `tree_project` 同源但自带聚合（组装点在 Organizer 构建之前） |
| 钩子 | repo_context 静态提醒段加历史查询钩子一句（项目过往 → memory_project_history 时间线先行 → search_titles/expand 下钻；on demand not proactively，v2.139 按需制纪律） |
| 测试 | desktop 322（+4：跨会话 expand 读目标且两会话树零记账污染 / 缺省仍当前会话 / search_titles 只在总结正文的查询词命中 / project_history 分组时间正序 + 20 会话钳制 + 无总结落 other_chapters）、根 workspace 40 套件全绿 |
| 门禁 | fmt 双侧 CLEAN、clippy 根 0 + desktop --all-targets 0、vue-tsc 0、pnpm build 过、lint:style 0、check_length 零新增超限（project_history.rs 165 / tree_tools.rs 288 / search_titles.rs 241 守线内；organize/tests.rs 722→864 系既有集成测试文件内增长，三问自检不拆） |
| 边界 | ①search_titles 同轮总结文本在多章打分文本重复，一轮多章可能挤占 top-5（接受）；②时间线 20 会话钳制——更早主题靠 search_titles（候选池无钳制）；③会话排序按 session_id 字典序（桌面 id 含毫秒 = 时间序）；④**批 2 挂账**：常驻进展行注入，待实测验证；⑤大一统树目录否决（别会话整理改写本会话 system 前缀 = 全项目缓存互断，分层方案零缓存代价） |
| 文档 | **《Cross-Session Recall Design v1》新立** + CHANGELOG v2.142 + AGENTS v2.142 |

---
### v2.141 · 整理段落总结（断层交接 + 项目跨会话锚点）

**用户 2026-10-10 口述裁定四条**：①整理在章节标题之外增加「可追溯对话流的浓缩总结」；②总结只在整理触发的下一轮拼进提示词，起断层交接作用，不每轮专门携带；③层级上总结在该轮 N 份章节标题的上层（这批章节的浓缩父节点）；④总结兼作项目跨会话的进展记忆锚点。另核实：手动 `/整理` 斜杠命令自 v2.65 在位（不经阈值、不受熔断限制、随时可触发），与本批管线天然协同零改动。

| 面 | 落点 |
|---|---|
| 层级 | 四层多分辨率：**RunSummary 轮级总结（常驻·历史消息面）→ L0 标题（常驻·system 目录）→ L1/L2（按需 expand）**；去噪只在常驻层（L1/L2 无损保留 = 有安全网的降采样）；持久性偏好居所仍是 memory_write（v2.136 模型） |
| 交付 | **真实历史事件落在断层处**：新 `SessionEvent::ContextSummary`，下一轮开场（agent_chat / 调度引擎持锁、先于 run_turn）从树投影排空落日志——旁路管道不持锁不得直写日志（内存/磁盘索引分叉防线）；位置恒在断层缝合处，随历史每轮在场直到被下一次整理**滚动收编**（近详远略 + 2400 字符硬钳） |
| 缓存 | 总结走消息历史追加面，system 与目录一字不改——prompt_layering 锁的前缀纪律原样成立；不重蹈 v2.136 handoff 教训（总结 = 封段一次成文的封闭期记录，非活文档） |
| 管道 | 记录员输出 = JSON 数组 + `===SUMMARY===` 尾随标记段（数组契约不动，先切标记再取数组）；rule 降级无总结宁缺勿假；四份家族提示词补「段落总结」节（叙事/去噪/引号原文/章节编号指代/衔接上一份/只用材料内信息/500 字上限）；材料新 `MaterialKind::Summary`（转录标签「总结」、逐字、非 User 不作切点） |
| 观察面 | 转录淡色可展开总结块（`agent://context-summary` 实时与重放同形）+ 记忆页会话档章节组上方总结链 + 项目联合视图升 `ProjectTreeView`（跨会话总结链时间正序 = 进展叙事锚点）+ tree.md 每轮总结引用块在该轮章节组正上方 |
| 测试 | memory-tree 45（+4）、session-store 46（+1）、desktop 318（+3：落树→下一轮排空全链含磁盘镜像对齐与幂等 / 降级无总结+越界跳过 / tree.md 字面上层渲染）、根 workspace 全绿 |
| 门禁 | fmt 双侧 CLEAN、clippy 根 0 + desktop --all-targets 0、vue-tsc 0、pnpm build 过、lint:style 0、check_length 零新增超限 |
| 边界 | ①整理在途时抢先发言 = 总结晚到一轮；②fork 截断已交付事件后重投（与树投影 rewind 语义一致）；③记录员编造风险靠提示词纪律 + expand 下钻验证缓解；④search_titles 不搜总结正文（记档）；⑤无设置开关（自然降级已覆盖，幻影旋钮不做） |
| 文档 | **《Organize Summary Design v1》新立** + CHANGELOG v2.141 + Memory Tree Design v2 §3.1 注记 + Context and Memory.md §3.2 注记 |

---
### v2.140 · desktop fmt 漂移归一（v2.138 挂账兑现 + desktop 纳入门禁）

**用户 2026-10-10 口述裁定「那45多正好修复掉吧」**（v2.138 挂账项「desktop 树内 fmt --check 全 crate 飘红是否独立归一批」的兑现）。实测漂移 = **39 文件 / 121 diff 块**（挂账时的「~45」为目测约数；中位 10 行/文件、总 1369 diff 行），全部为 v2.127–v2.132 时代旧文件（assets/、llm/import/、media/、settings/tests/、subagent_verify、wire.rs 等），与 v2.137–139 触碰集零交集。根因不变：desktop edition 2021、代码系 edition-2024 风格（style_edition 随 crate edition 缺省），根 fmt 因 workspace exclude 从不覆盖 desktop。落点 = `cargo fmt` 一次归一：+418/−223 **纯格式重排**（抽查 = 函数调用参数竖排等 style_edition 差异），零语义改动、零意外文件（39 与漂移清单 1:1）。门禁（最后一次编辑后复跑）：**desktop `cargo fmt --check` 历史首次 CLEAN** + 根 fmt CLEAN + clippy --all-targets 0 + desktop 315 绿（与 v2.139 持平，格式不改语义）；根工作区零触碰（位等同 v2.139 提交态）。**自此 desktop fmt --check 纳入每批门禁**（基线已净，此后见红即本批之责）。已知边界（记档）：无 rustfmt.toml——desktop 按 crate edition 缺省 style_edition 2021 归一；将来若升 edition 2024 需重归一轮。文档：CHANGELOG/ROADMAP/AGENTS v2.140。

---
### v2.139 · 提示词分层拼装（《Prompt Assembly Layering Design v1》四批全落——治「开场就查记忆问项目」）

**用户 2026-10-09 口述「那开始实施吧」**（设计稿同日评审通过）。病根：聊天请求的 system 无身份段与行为总纲，行为全由三个「主动去做」的注入块驱动（读 handoff / 维护记忆 / list 项目文件），模型开场就查记忆、读项目、盘问阶段。四层落地（ZCode 分层架构同构，M1 字节前缀纪律全保留）：

| 层 | 落点 | 版本注 |
|---|---|---|
| 静态不可变层 | kernel-agent 新 `BaseSystemInject`：`prompts/base/` 六节 `include_str!`（引言/身份总纲/工具策略/安全边界/输出纪律/reminder 合法性）≈1.25K t，排 system 字节前缀——跨会话同前缀 | 「背景不是议程」优先级金字塔 + 开场禁令 = 治本一句 |
| waterfall 重排 | ToolExposure → Base → Memory → 思考语言 → 树目录（殿后）→ 指令文件 → 碎片；`tests/prompt_layering.rs` 键静态段前缀 + 目录增长前缀逐字节不变 | 修 v2.137 前遗留：思考语言排在只增目录之后，每整理断一次缓存 |
| 措辞手术 §5 | repo_context / memory_guidance / environment 三块：write 主动保留，read 全改按需（「never as an opening ritual」「never as a way to start a conversation」显式禁令） | 「do not wait to be asked」只保留在 write 语义 |
| 动态环境层 | `<workspace_context>` 升级 `<environment>`：+ Today/Platform 两行（日期随记忆块预取 = 会话锁死；chrono 直依赖零新增编译成本）；diagnose SYSTEM_MARKERS 随改 | git 快照/项目树不做（模型自查，ZCode 亦无） |
| 指令文件层 | kernel `InstructionFilesInject` + desktop `instruction_files.rs`：全局 `<数据根>/AGENTS.md` + 项目根 `AGENTS.md`（回落 `.Locowrk/`）→ 合成首条 user 消息（`# agentsMd` + OVERRIDE）+ system-reminder 包裹；会话首查快照锁字节；100KB 截断；settings `memory.instructionFiles` + 设置行 | I 子目录级裁掉记档；全局文件落数据根（设计稿 `~/.Locowrk` 的偏差记档） |
| 会话态碎片 | kernel `FragmentInject`（wrap_system_reminder 集中转义）+ desktop `FragmentPending`：整理完成 / Plan 模式 / 轮次到顶三碎片，轮开场（最后一条消息是 User）取件拼接一次即清 | 落点裁决：kernel waterfall messages 可变（ctx-compact 同款通路），headless 同享 |
| 描述微调 | memory_read / memory_search 尾补 on-demand 句 | memory-sqlite 0.1.5 随批 |

门禁（最后一次编辑后全量复跑）：fmt --check 双侧 CLEAN、clippy 根 0 + desktop 0、根 workspace 全绿、desktop 315 绿（+6）、vue-tsc 0、pnpm build 过、lint:style 0、check_length 33 持平。已知边界：碎片 pending 内存态崩溃即失（无害）；碎片清标记同轮前缀分叉 ≈ 一条 user 消息编码；日期跨天不提醒；「背景不是议程」措辞强度实机双场景验收通过（用户 2026-10-10 目检「真机验证比之前好太多了」）。文档：设计稿标注 v2.139 + CHANGELOG v2.139。（进度条/用量环接入 llm_usage 真实事件——压力实测口径分裂修复）

**用户 2026-10-09 压力实测抓的口径分裂 + 裁定**（「进度条能不能接入到这个 llm_usage 事件真实的事件上，这样不是准确吗？统一改成真实锚点吧」）：上下文 UI 显「已用 ≈110K」且阈值进度条 112% 全红，100K 整理阈值却不触发——触发锚点吃供应商真实 `prompt_tokens`（最后一条 `llm_usage.input` = **95,483 < 100,000**），用量环/进度条吃字符估算 `context_usage.used` = **112,606（+18%）**，两把尺并存（trigger.rs 挂账「M4 组装器批统一」一直未兑现）。落点（纯 runner/organize 面，前端零改动）：runner.rs 新 `reported_input_tokens` 共享锚点；`record_usage` 的 `used` 改真实优先（无上报回落原估算链：树组装口径 → 全量字符估算）；`Organizer::anchor` 收敛调用共享函数（触发/自动收起/观测面板与进度条同尺——**「看到的 = 触发用的」**，进度条到线必然伴随整理）；`ContextUsage`/`trigger.rs` 注释随改。回归：`context_usage_prefers_reported_input`（预置实测值 95,483 → 每条快照全等，估算回落不得发生）。门禁全绿（desktop 304）。已知边界：真实锚点滞后一步（最后一条 assistant 回复不计入，下次请求真算）；供应商不报用量 = 永久估算回落；旧日志历史事件不重算；浮层「尾巴 %」「工具上下文 ≈N」仍估算（无供应商计数）。详见 CHANGELOG v2.138。

### v2.137 · 上下文面重构（整理阈值绝对 K + 窗口自动识别 + 压缩/legacy 整理面退役 + 缓存单价退役）

**用户 2026-10-09 口述裁定**（用量环要自动识别模型最大上下文；整理触发改「达到多少 K 就触发」不按百分比；压缩阈值改名整理阈值 + 100K–800K 滑块带气泡；压缩策略没用了就删；用量进度条改名阈值用量进度条、分母 = 阈值；缓存命中/未命中单价要用户手填 = 删；缺省整理方式随标准整理退役）。落点：`session_window` 四层链（会话覆盖旁车 → 模型行 → 目录 → 档位）喂 UsagePolicy 与自动收起；`TriggerInput.threshold_tokens` 绝对阈值（窗口未知照常触发）；ctx-compact 桌面链退役 + organizer_mode 删库（**一切会话皆树**，旧旁车/旧 settings 键全幻影）；ContextSection 重写（滑条 + 拖动气泡）；缓存单价两键 + 节省估算退役。门禁全绿（desktop 303）。已知边界：阈值需低于模型窗口；窗口全未知 = 环按未设置呈现、自动收起跳过。详见 CHANGELOG v2.137。

### v2.136 · 记忆模型 v2（autoExtract / experiences / handoff 后台改写三退役 + 注入面收敛项目侧全提醒制）

**用户 2026-10-09 口述裁定**（真正的记忆 = 短期会话上下文 + 长期记忆树 + 项目记忆两处 + 全局记忆（进流水线保持简短）；项目记忆由主模型在工作中自己管理记录修改——记忆管理器只管当前上下文打包整理，软件只做节点提醒钩子）：**①autoExtract 退役**（extract.rs 整模块 + agent_chat 收束钩子 + app_state extract_deps/llm 字段）；**②experiences 退役**（experiences.rs 整模块 + `<session_experiences>` 注入段 + 内核 ExperienceSource 契约 + 管理页经历区块；向量基建保留服务 search_titles/工具打分/search_tools）；**③注入面收敛**：persistent_memory 只注入全局层（项目层事实改按需 memory_read/search），repo_context 改**静态提醒段**（节点钩子：开场先读 requirements/handoff、状态实质变化时主模型自己更新），scope_id 去 mtime 指纹（人改 .Locowrk/ 不再触发记忆块重算）；**④handoff 后台自动改写退役**（agent_chat/调度引擎两收束点钩子拔除，交接文件主模型按提醒自维护）。GUIDANCE_BLOCK 按新模型改写；settings 三键幻影化（存量 settings.json 照常解析）。门禁：fmt、clippy 0、根 workspace 39 套件 + desktop 307 绿、vue-tsc 0、build、lint:style、check_length 零新增超限。已知边界：历史 auto.*/experiences 数据不迁移；主模型不读提醒则交接面退化（观察）。详见 CHANGELOG v2.136。

### v2.135 · 批 G4 只读证明（候补批 G 第四件·P1，可靠性稿 §6 兑现——G 轨全表收口）

**shell_read 从「白名单匹配」升级为「证明只读」**（纯 tool-shell 插件层，LLM 轨与内核零改动）：新 `proof.rs` 三段分析——**词法**引号感知（未加引号/双引号内的 `| & ; ( ) < > ` $` 与换行 = 结构面；单引号内全字面——引号内元字符是数据，`grep "a|b"` 不再误拒）；**语法** = 单条简单命令（宿主无 shell，复合形态不可执行，「进审批」= 拒绝指路 shell 审批承载简单形态，偏离记档）；**语义** = 白名单表 → git 全局选项拒（`-c` 可借配置触发外部程序）→ 写/外部旗标拒（`--output`/`--ext-diff`/`--textconv`）→ 敏感凭据路径拒（`.env`/`.ssh`/credentials/密钥材料；与桌面 privacy 门经济学相反非重复建设——豁免面宁严勿松）。三态 Safe/Unsafe/Unknown，**Unknown 按 Unsafe 处理**，报文全带出路。`shell` 管线/路径围栏/unlockRisky/外部路径门全保留；`decide_readonly` 退役并入 prove；shell_read 描述写明六条禁令；manifest.json 随批补齐 shell_read 条目（陈年脱节欠账）。插件版本 tool-shell 0.1.4（小迭代 +0.0.1）。**危险命令回归集实抓 ×2**：git 全局选项检查被白名单盖序；`\n` 满足 is_whitespace 被当切词空白（结构分支必须先行）。**走查**（CDP 隔离数据根 + CM 共享零明文 + deepseek 真轮 7 轮 + 询问档）：引号竖线 grep 免审批成功；`ls $(rm -rf /)` 模型认知层自拒（描述升级防线前移）；`ls $(pwd)` 结构拒 → **模型读报文自发改跑纯 ls（教学闭环端到端）**；`cat .env` 敏感拒（认知自拒一次 + 强制调用实拒一次）→ 指路 shell → **询问档真审批卡 → 允许读到夹具**；`git -c core.pager` 拒 + 干净 git status 照常；`git diff --output` 写旗标拒逐字转述。门禁：fmt、clippy 0、根 workspace 39 套件 + desktop 322 绿、vue-tsc 0、pnpm build、lint:style、check_length 零新增超限。已知边界：敏感路径子串启发式宁严勿松；`\` 按字面。详见 CHANGELOG v2.135。
### v2.134 · 批 G2 Unknown Effect（候补批 G 第二件·P1，可靠性稿 §5 兑现）

**三件全落，不新增双轨事件（复用 ToolCall/ToolResult 配对）**：**①扫描** `session_store::scan_uncertain`（声明位 − ToolResult 事件 = 悬空，两形态都认：ToolCall 事件已落 / 死在两次落盘间只剩声明位）+ **derive_messages 悬空诚实闭合**（设计稿未言及的落地补充——不给合成则下一轮请求带悬空 tool_calls 被供应商整单拒收；未裁决 = `status:"unknown"` 效果未知，有裁决按裁决闭合；合成在投影层，日志不回填）。**②uncertain UI**：组外决策卡（折进过程组会被默认折叠藏住）+ 工具行 warn「结果未知」态；三裁决 = 视为已完成 / 视为未执行 / 检查后记录——设计稿四选的 **Rerun 裁掉**（「未执行」后模型下一轮自然重做，走全部既有审批/守卫门）。**③用户决定事件** `ToolExecutionUncertain`（serde default，只写一次重放不再询问）+ `session_resolve_uncertain` 命令（忙锁守卫 + inspected 必带记录）实时回推与重放同形（**走查实抓真缺陷**：重放缺合成闭合 → 已裁决行恒 pending，修投影后重放=实时同序列「建卡→裁决收束→行闭合」）+ 启动 restore_all 观测日志。**走查**（CDP 隔离数据根 + CM 共享零明文 + deepseek 真轮 4 轮）：真 kill + 构造 mid-flush 悬空尾——重放卡/行态；**未裁决发轮模型如实引用 `status:"unknown"` 拒绝猜测**（原话「我不知道删没删……绝不凭猜测宣称已完成」——§5「崩溃恢复时不猜」模型层实证）；裁决落 jsonl（inspected 带 note）卡收记录行；再重启不重复询问 + 裁决后真轮完成（hello.txt 落盘）。门禁：clippy 0、fmt、根 workspace 431（39 套件）+ desktop 322 绿、vue-tsc 0、build、lint:style、check_length 零新增超限。详见 CHANGELOG v2.134。

### v2.133 · 批 G3 Revision Guard（候补批 G 第三件·P0，可靠性稿 §3 兑现）

**共享基座 `plugin_api::fsutil`**（纯插件层，LLM 轨与内核零改动）：`content_revision`（sha256 over LF 归一全量字节，\r\n 折叠与 edit_replace 容错同口径；全量哈希换无死角检测）+ `file_revision` + `check_revision`（StaleObservation 报文带双方 revision + 指路 fs_read 重读）+ `arg_expected_revision`；plugin-api 增 sha2（与 desktop 同锁）。**五写类工具守卫**：fs_read 回全量 revision（与截断窗口解耦）；fs_write expected_revision 缺省/null = 新建语义（已存在即拒盲覆盖）；fs_delete 同守卫（防误删外部已改文件）；edit_replace/lines/diff 必传（比对先于任何备份/写盘）；**成功输出带新 revision 供链式写**（守卫不断链）；fs_restore 输出带恢复后 revision；undo 契约与审批面零改动。read_image/pdf_to_images 的 asset 引用不带 revision（无 edit 消费方，评估后裁掉记档）。**实抓 ×2**：tool-fs read/write 测试模块私有 ENV_LOCK 互斥失效（env 串场）→ 锁上收 crate::testutil 共锁；lines.rs 守卫绑定遮蔽 args 的 content（clippy 抓到，防住 new_lines 误用文件内容的真 bug）。搭车：desktop catalog 测试同 pid 临时目录并行互删分家。**走查**（CDP 隔离数据根 + deepseek 真轮 3 轮）：W1 §10.2 打地鼠全链——读→（驱动外部注入行）→edit_replace stale 拒（报文带双方 revision + fs_read 指路）→重读→编辑成功且**注入行幸存（无静默覆盖）**，模型收束报告逐字写明「工具因观测过期拒绝写入……这是安全机制要求的」；W2 链式 revision（fs_write 输出 = 下一次 edit_replace 的 expected_revision 逐字节相同，零失败）；W3 删除守卫首跑实锤（陈旧 fs_delete → stale 拒 → 重读 → 删成功），复跑轮模型自觉先重读并**主动报告外部改动异常**（认知层生效）。驱动教训记档：轮内 watcher 只认成功读（失败重试读会提前点燃注入）。门禁：fmt、clippy 0、根 workspace 全绿（plugin-host FFI 端到端含盲覆盖/陈旧拒绝断言）+ desktop 321、vue-tsc 0、pnpm build、lint:style；check_length 零新增超限。插件版本 tool-fs 0.1.5 / tool-edit 0.2.1（小迭代 +0.0.1）。详见 CHANGELOG v2.133。

### v2.132 · 批 F 生态（LLM 能力轨第六批·收官，G5 随批）

**① 子代理模型档 + 终端错误回退链**：`ProviderModel.available_for_subagents`（用户钉死标记，None = 不参与）+ `LlmConfig.subagent` 钉档（缺省跟随会话）；路由走 `params["route"]` sanctioned 通道（内核零改动、route 不进请求体）；`SubagentLlm` 每子 run 一实例——钉档打头 + 标记池，凡上抛 Err（= Fatal）推进下一候选同请求重试，窗口装不下已携上下文的候选记因跳过（PI 案）；死供应商不触发回退（v2.75 不设上限纪律，偏离 PI 记档）；`SubagentOutcome` 增 model/model_failures；内核 `child_model.rs` 契约 + child_kernel 作用域换入。**② 从其他工具导入**：claude-code/codex/opencode/cc-switch 四解析器（纯读、密钥不进 IPC、cc-switch = SQLite 容器分派前三解析器）+ `ImportDialog` 预览确认写入档位。**③ 推荐三选**：`llm/recommend.rs`（toolCall 门 + 价格升序 + 族分散 ≤3）驱动探测命令 `recommended`，前端自动预选改吃它——**实抓修复：自动预选自 v2.58 起因 onPickService 误置 touched 从未生效**。**④ G5 subagent_verify**：宿主内置 Pure 工具，读子转录 + 验收 LLM 调用（辅助降档）逐准则裁决；claimed/verified 分离走既有工具结果通道（偏离设计稿 §7.6 新事件记档）；FilterToolSource 剔除面扩 `subagent_` 前缀。**走查**（CDP 隔离数据根 + deepseek 真轮）：推荐三选 IPC+UI 实证 / 导入四源全命中 + cc-switch 落档 / 钉模型真轮 outcome.model = v4-pro（父在 flash）/ 回退真轮（伪模型 400 记因 → 推进 v4-pro 跑完）/ verify 真轮如实判 failed 带证据。门禁：clippy 0、fmt、workspace 39 套件 + desktop 321 绿、vue-tsc 0、pnpm build、lint:style。详见 CHANGELOG v2.132。

### v2.131 · 批 E 多模态进阶（LLM 能力轨第五批，D4 按推荐落档 = 投影）

| 项 | 落点 |
|---|---|
| 工具结果投影（D4） | ✅ wire.rs `project_tool_assets` 纯函数（`AppLlm::prepare` 调用点 = image_allowed 在场 + 钳制估算吃投影后形状）：tool 消息剥 asset + `image_note`（跨请求字节稳定）+ 尾部合成 user 图片消息；历史轮不重发；不支持 = 引用号留原地 + 未发送 note 降级不报错。设计稿「无下一条 user 保持占位」偏离记档（占位 = 模型永远看不到图） |
| 多模态工具 | ✅ `media/` 三件宿主内置 Pure：`screenshot`（Always，screenshots crate，maxDimension 缩边）+ `read_image`（OnDemand，env 围栏 + 先验后写）+ `pdf_to_images`（OnDemand，**pdfium.dll 7.5MB 随包**、缺库不进目录、进程级绑定单例、页数缺省 8 顶 20、目标宽按 cap×纵横比 floor+余量）；`asset_save_as` 另存命令 |
| 图片查看器 | ✅ `ImageViewer.vue` + `viewer.ts`：左右切换（chat.viewerImages 转录序）/缩放（滚轮/＋−/双击/0）/拖移/Esc+滚动锁/另存为/reveal；MessageRow 图片缩略点击改开查看器 |
| 工具行产物预览 | ✅ output.asset/assets 图片 56px 缩略 + 查看大图/另存为（§8.3 三行形态）；动作名三键 + camera/download/chevronLeft 图标 |
| 降级链挂捆边界 | ✅ 最小形态：memory-tree 材料 UserMessage 折 `[图片: name]` 占位（捆边界降级不归零）；「视觉描述替代原图」记档 P2 待裁定；**detail 旋钮评估 = 裁掉**（记档维持） |
| 实机走查 | ✅ echo 18900 抓包 10/10（投影两形态载荷实锤）+ deepseek-flash 真轮（截屏描述屏幕/read_image 红绿图/pdf 双页逐页描述/查看器全交互/捆边界 L1 占位 + 命中率 4864→0→3712 回落恢复）；**实抓修复 ×4**（截屏缩边/PDF 页高 1px 超限/pdfium 绑定单例/投影数组翻面页序颠倒）；走查教训记档（echo SSE 必须 HTTP/1.1）。数据根已删、进程已停，脚本/截图留 tmp/v131-walk/ |
| 门禁 | ✅ clippy --workspace --all-targets 0、fmt、根 workspace 418 绿 + desktop 301（292→+9：投影 4/media 5）、vue-tsc 0、pnpm build 过、lint:style 0；check_length 新增超 2 文件三问不拆（wire.rs 515 / llm/tests.rs 784） |
| 文档 | ✅ 阶段计划勾批 E + D4 落档 + LLM.md v3.5（多模态进阶一节）+ UI shell v4.25（§3.8 产物预览 + 查看器 + 版本条）+ CHANGELOG v2.131 |

---

### v2.130 · 批 D 多模态地基（LLM 能力轨第四批·最大批（内核穿透），D2 随批落档 = 单线纪律维持）

| 项 | 落点 |
|---|---|
| 消息结构 | ✅ session-store 新 `content.rs`：`ContentPart` 枚举 text/image/file + `AssetRef{id,mime,name}`；`Message::User +parts` + `UserMessage +parts`（serde 字段级缺省 + skip_serializing_if = 纯文本消息磁盘字节逐位不变，v2.47 双气泡回归绿）；快照 schema 1→2 + `V1ToV2Upcaster`——实抓 snapshot.rs 缺 `mod tests` 声明存量测试从未跑过，补后 upcaster 未注册被抓 |
| 穿透面 | ✅ fork/rewind+undo/树偏移投影/derive_messages/ctx-compact（parts 不进摘要；est_tokens Image 1024/File 1000 进钳制估算，1024 = DeepSeek 官方单图锚点）/转录折叠 `[图片: name]`（export/extract/title 同吃）全绿；~30 文件构造点随迁 |
| AssetStore | ✅ 新 `assets/` 三文件：内容寻址 `assets/{images,files}/<sha256>.<ext>`（同文件天然去重）+ data-URL LRU(16) + magic 嗅探 + 超尺寸拒收不转码 + 五命令（put/put_path/read/reveal/gc）；GC 三扫描面（sessions jsonl + snapshot + sub-transcripts）+ 宽限 gcThresholdDays 缺省 7（0=即净）+ 启动 + 24h 循环——实抓 setup 无 tokio runtime `tokio::spawn` 即 panic，改 `tauri::async_runtime::spawn` |
| wire 编码 | ✅ 空 parts = 纯字符串 content（M1 前缀缓存零扰动）；非空 = content 数组 text 首位稳定前缀 + `image_url` data URL / `file{filename,file_data}`；`check_modalities` 后端发送门（目录未知放行宁缺勿假）；D2 = 多模态输入侧走 OpenAI 兼容层随批落档 |
| 双道门控 | ✅ 前端上传时（supportsImage 三态 + 目录 modalities，门卡「仅发送文字/打开模型选择器」）+ 后端发送时各一道 |
| settings | ✅ `multimodal` 区（maxDimension 2048/maxSizeMb 5/formats/autoGc/gcThresholdDays 7）字段级缺省零迁移 |
| 前端 | ✅ composer 预览条（56px 缩略图 + 粘贴/拖拽/AttachButton 路由）+ MessageRow 96px 缩略图与文件 chip（点击 reveal）+ 重放恢复 + editResend 保留 parts + i18n mm.*；实抓产品缺陷：发送失败清空附件条（门控拒绝后无法原样重试）→ 改成功才清 |
| 实机走查 | ✅ CDP 独立数据根 + keyring 标记真 Key + deepseek-flash 真轮：walk1 13/13 PASS（老格式会话重放零变化/双气泡；贴图真轮模型如实描述红底绿圆报色值 E8232A/00CC44；jsonl parts 落盘 asset id 一致；第二轮 cache_read=3840>0 前缀缓存未塌；重放缩略图恢复；同文件去重）；walk2 门控两道（前端门卡+仅发送文字 / 后端直发被拒如实报错）；GC 配对验证（days=7 scanned=2 removed=0 双存 / days=0 removed=1 孤儿删被引用存，jsonl 引用扫描实证；W6a 驱动伪失配记档）。数据根已清，脚本/截图留 tmp/v130-walk/ |
| 门禁 | ✅ clippy --all-targets 0、fmt、workspace 39 套件（desktop 292 = +9：assets 3/validate 3/gc 3）、vue-tsc 0、pnpm build 过、lint:style 0；check_length 新增超限 2 文件三问自检不拆（MessageRow 475 / settings tests tools 403），Composer 既超同职责面扩张不拆 |
| 文档 | ✅ LLM.md v3.4（多模态输入侧一节）+ UI shell v4.24（§3.11 附件预览条 + 门控卡）+ 阶段计划勾批 D + D2 落档 + CHANGELOG v2.130 |

---
### v2.129 · 批 C 连接与安全（LLM 能力轨第三批，D6/D7 按推荐落档）

| 项 | 落点 |
|---|---|
| keyring（D7） | ✅ 新 `llm/keyring.rs`（三平台 feature，service="Locowrk"/account=provider_id）：`key_storage` 标记 + 双读回落（明文优先）+ settings open 惰性迁移（**有变更立即落盘**）+ 写失败回落文件；测试构建不碰真实凭据库；IPC `api_key_set` 两态点亮；`flat`/`flat_profile` 双入口全消费面接线 |
| 自定义请求头 | ✅ `ProviderProfile +headers`：`validate_headers`（≤32 条/token 键/14 保留键拒收）+ `build_headers` HeaderMap 鉴权先入、用户头**最后写入者覆盖**；探测/清单/扫描同吃；UI 折叠行编辑器（chips 预设 + JSON 导入/复制） |
| 端点扫描 | ✅ `sweep_candidates`（现值剔除 + ≤3 候选 × 3s；**裸域名补 scheme 分形态**：本地→http、其余→https，走查实抓修正）+ `ProbeOutcome.effectiveBaseUrl` 表单未再编辑自动采纳 + 提示行；选择器路径不扫描（谎报记档） |
| 逐模型测试 | ✅ `llm_test_model`（最小 ping 15s）五分类 + unknown 带报文；ModelPanes 行「测试」钮 + 结果徽章常驻（title 带报文） |
| 网络代理（D6） | ✅ `llm.networkProxy{mode,url}`（system/direct/custom）+ `apply_proxy` 四处同吃（客户端逐请求新建 = 保存即热换）+ `llm_set_proxy` 命令 + 「网络代理」组（custom 草稿档门控地址行） |
| 实机走查 | ✅ CDP 独立数据根 + 复制真 Key + 本地 echo/scenario 服务器：明文 Key 启动即迁移（settings 只剩标记 + Credential Manager 双条取证）→ 重启真轮读回 → 种子/UI 双头随请求注入 → 裸域名扫描自动补全（唯一 host）→ 五分类+network 七徽章逐例命中 → custom 死代理真轮无限重试不收束（如实）+ 停止空收束退路 → system 恢复真轮完成。走查脚本教训记档 ×2（轮次排队交错断言纠偏 / test_model 走已存档位）。数据根已清 + CM 条目删净，脚本/截图留 tmp/v129-walk/ |
| 门禁 | ✅ clippy --all-targets 0、fmt、workspace 39 套件（desktop 283 = +13）、vue-tsc 0、pnpm build 过、lint:style 0；check_length 新增 5 文件三问自检不拆 |
| 文档 | ✅ LLM.md v3.3（连接与安全一节）+ UI shell v4.23（§3.30 五处 + 版本条）+ 阶段计划勾批 C + D6/D7 落档 + CHANGELOG v2.129 |

---

### v2.128 · 批 B 推理档位 + 输出钳制 + G1 闸（LLM 能力轨第二批 + 候补 G1 随批，D3/D5 按推荐落档）

| 项 | 落点 |
|---|---|
| 方言即数据 | ✅ 新 `llm/dialect.rs`：6 方言 × 语义档 → body patch 纯函数（单测锁形状）；DeepSeek 官方归一表；映射不了不发；别名表三元组（host, 供应商键, 方言）——deepseek/glm/qwen/openai_effort/anthropic_effort 五家带方言 |
| 会话档位 | ✅ 旁车 `thinking_level` + `ModelOverrideStore` 双字段缓存 + 新命令 `session_set_thinking`（八语义档校验）；`SessionMeta.thinkingLevel` 透出 |
| 解析链 | ✅ `AppLlm::prepare`：档位 → 方言 clamp（就近降档/剥除）→ `params["thinking_level"]`（内核零改动）；**辅助降档** = 四调用点 `params["aux"]=true` → 方言最低档 + 不发 max_tokens（记录员直呼点就地解析） |
| 输出钳制 | ✅ D5 窄修订：`WireCaps`（钉死值 + 生效窗口）→ 预检钳制 `min(钉死, ctx−est−1000)` 下限 1024；未钉死不发送；400 点名 max_tokens 剥字段重发一次 |
| G1 length 闸 | ✅（候补 G1 随批）`finish_reason==length` → Fatal 如实报错、工具不执行、截断消息不入史 |
| UI | ✅ composer 档位循环钮（[默认→...梯子] 循环，会话级，首页/无梯子隐藏）+ 两模型菜单行 ✨/👁 徽章；ModelPanes 梯子改方言可发送口径（§5.1） |
| §5.5 复测 | ✅ 走查第一项：deepseek-flash 真轮 8 次工具调用不回传 reasoning **零 400**——义务未被供应商执行，**维持现状不实现回传**（记档复核结论 + PI 回退方案记档） |
| 实机走查 | ✅ deepseek 真实轮 6 轮（隔离数据根 + 复制真 Key + fsRoots 换 scratch）：循环钮真点默认→disabled→low→high；按轮归属 thinking 4+2→**0**→1→1；autoTitle 辅助降档两标题生成；G1 两轮实锤（钉死 1024 → UI 截断文案 + failed + 不入史）；旁车档位落盘。走查脚本教训记档：settings 手改只认 camelCase / 直改文件须重启 / jsonl 标签小写。数据根已清，脚本/截图留 tmp/v128-walk/ |
| 门禁 | ✅ clippy --all-targets 0、fmt、desktop 270 绿（+12：dialect 9/wire 2/override 1）、vue-tsc 0、pnpm build 过、lint:style 0 |
| 文档 | ✅ LLM.md v3.2（请求装配批 B 两节 + 流式 G1 + §5.5 结论）+ UI shell v4.22 + 阶段计划勾批 B + D3/D5 裁定落档 + CHANGELOG v2.128 |

---
### v2.127 · 批 A 模型能力目录（LLM 能力轨第一批：《Model Capability and Multimodal Phase Plan》A 批，D1 按推荐落档 = models.dev 快照）

| 项 | 落点 |
|---|---|
| 快照资源 | ✅ `resources/models.dev/api.json` 5.34MB 随包（tauri bundle.resources）+ `llm/catalog.rs` CatalogStore 惰性加载（缺文件/坏 JSON = 空目录如实不 panic）+ 在线刷新只进内存永不落盘 |
| 匹配 | ✅ 新 `llm/catalog_match.rs`：预设 host 别名表 +100 / 快照 api host 精确·后缀 +80 / 归一化 id 两遍扫（先原样后归一）；**借池不做**（无定位 = None）；别名表 × 快照守卫测试（实抓 together→togetherai） |
| 合成投影 | ✅ `ProviderModelView` = 钉死面 ∪ 目录（sources 逐字段 user/catalog/None）；`ProviderModel` +max_output/max_output_source/supports_image（serde 字段级缺省 + 旧文件幻影测试）；**目录数据永不落 settings**（前端 save 剥 sources=catalog 派生值 + 后端 serde 边界丢弃双保险）；五出口统一 enrich（settings_get/set + 档位三命令） |
| 命令 | ✅ `llm_catalog_status` / `llm_catalog_refresh` / `llm_lookup_model`（自定义模型添加时就地升级）+ `open_external_url`（https 白名单壳，零新依赖） |
| 梯子归一化 | ✅ toggle+effort 并存 enabled 冗余不出现（deepseek = disabled/low/high/max）；toggle 独占补 enabled（GLM）；none→disabled；未知值保序追加——批 A 只投影展示，批 B 接方言 |
| UI | ✅ 模型目录组转正（状态行 + 刷新钮）；编辑模态右栏视觉徽章（新 eye 图标）+ ctx·max 徽章 + 高级区扩容（最大输出钉死/图像输入三态/梯子只读 chips/价格行 +「编辑即钉死」info 悬浮）；sources=null 行自动合并（未命中落定全未知防回环）；价格三处（composer 选择器/默认浮层/详情行，USD 原值如实）；预设 22→27 家（Google/百川/零一/阶跃/LM Studio）+ docsUrl/local + 「获取 API Key ↗」 |
| 实机走查 | ✅ CDP 独立数据根 **18/18 PASS**（种子 DeepSeek 档无 Key，目录面离线全验）：状态行/刷新 30s/徽章/梯子/价格/悬停钉死提示/32K 钉死落盘（目录派生零落盘）/重开并存/自定义模型就地升级（v4-pro 真值 $0.66/$1.98）；**实抓修复 ×3**（刷新超时 10s→30s：5.3MB 载荷代理链路 10.2s 撞死；addCustom 预填标 user 挡合并：预填只写值不标来源；归一化多变体裁决吃 HashMap 随机序：三级确定性裁决 5 连跑验证）；数据根已清，脚本/截图留 tmp/v127-walk/ |
| 测试 | ✅ desktop 258 绿（+19：catalog 8 / catalog_match 9 / settings 钉死往返+合成 2）；clippy --all-targets 0、fmt、vue-tsc 0、pnpm build 过、lint:style 0 |
| 文档 | ✅ LLM.md v3.1（§6 模型能力目录）+ UI shell v4.21（§3.30 四处 + 版本条）+ 阶段计划勾批 A + CHANGELOG v2.127 |

---

## 三、收尾 backlog（随批次搭车）

| 项 | 说明 | 计划搭车批次 |
|---|---|---|
| fs roots 工作区目录选择器 | ✅ 已完成（⑱，v2.33，dc2ad87：tools.fsRoots + 保存即生效 env 重注入 + 原生对话框选择器） | — |
| desktop session 持久化 | ✅ 已完成（⑲，v2.34，a5347c3：sessions/\*.jsonl 镜像 + 启动恢复 + undo 重写备份 + 会话切换器，详见下方 ⑲ 块） | — |
| LLM 流式输出 | ✅ 已完成（⑳，v2.35，54a97ba：genai 流式 → delta 直推 + streamed 投影去重 + 子内黑盒压制，详见下方 ⑳ 块） | — |
| 市场 wasm 打包分发 | ⑧⑧ 遗留 | 看需求 |
| kv_get 所有权协议 |  | 看需求 |
| profile.toml 设置面板接线 | ❌ 已撤销（批 D v2.148，审计 H4：加载器从未实现、文件七行全注释零生效键；文件与 README/DESIGN §8·§11.3/AGENTS 目录描述一并移除，开发态装载归 dev_scan + lockfile） | — |
| 工具级取消与 deadline 真接线（审计 H5 → 计划批 H） | ⬜ 设计稿先行：`CallCtx.cancel/deadline` 现网两端未接（deadline 有消费者无生产者、cancel 无人触发），且取消信号过不去 C ABI（`HostVtable` 无取消槽）。最小完整形态 = 内核活动调用登记表 + 停止时逐个 cancel + vtable 末尾追加 `call_cancelled` + `subagent_run`/`ask` 真轮询 + §5.2 同批。v2.148 批 D 已在代码头注标 RESERVED 并写清事实 | 需设计 + 你定口径 |
| 托盘菜单 i18n | ✅ 已随壳批次完成（a2c82a5：sys-locale 探测，语言改动即时重挂菜单） | — |
| UI 视觉抛光 | ✅ 已完成（⑰，v2.32，945e681：明暗主题 + 语义颜色类 + 统一动效；规范落 UI shell 文档 §六） | — |
| tool-http（POST/PUT 通用请求） | plugins.md P1 唯一未做的工具类；tool-fetch 仅 GET（其代码注释已声明 POST/PUT 归本插件） | 看需求 |
| mw-ratelimit 调用限速 | plugins.md P1.5 中间件：限制单位时间调用次数 | 看需求 |
| 中间件插件化 | mw-audit / 审批门 / ctx-compact 均为宿主侧形态；C ABI PluginKind::Middleware 仍是占位变体，无真插件使用 | 看需求 |
| 权限模式四档切换器 | 约束清单 §4（后期）：计划/变更前确认/自动编辑/完全访问，挂会话输入框旁随手切；三类硬底线（路径逃逸/SSRF/ABI 边界）任何档位生效——"完全访问≠无安全" | ⑬ 后看需求 |
| 会话保留天数可配 | 约束清单 §3 余项（session-store 保留期） | 看需求 |

---

## 四、[plugins.md](plugins.md) 必备插件清单完成度

> **状态列唯一真源 = [plugins.md §一「现役状态」列](plugins.md)**（v2.149 批 E 立的）。本节此前是一张
> 2026-09-29 的逐项 ✅/⬜ 快照，而 plugins.md 同时也被人读——**同一信息两份表，改一处必漏另一处**，
> 到批 E 核对时两份已经分叉到互相矛盾的程度（本节 6 处 ✅ 描述的是**已退役或已换形态**的东西）。
> 处置 = 本节删掉逐条表，只留结论与分叉清单；今后**新增/退役能力只改 plugins.md §一**，本节不复制表格。

**结论（v2.149 核）**

* 清单 19 项里：**动态库插件 9 个 id 全部现役**（`plugins-src/`，见 [plugins/README.md](../../plugins/README.md) 版本表）；
  另有 6 项能力以**宿主内置**形态交付（模型接入 / 审批 / 审计 / 子 Agent / 定时任务 / 向量检索基建）——
  「不是插件」不等于「没做」，也不等于「该补成插件」。
* **仍未做**：`tool-http`（POST/PUT 通用请求）、`mw-ratelimit`（工具调用限流；现存的只有 LLM 传输层 429 退避）、
  `PluginKind::Middleware` 中间件插件化（契约位仍是占位变体）、Playwright 级网页自动化（`tool-browser` 的原始定义；
  v2.144 交付的是宿主内置预览 + 控制台回读，不是同一件事）、shell 跨平台长期策略（`BUG/` 清单里 ⏸ 的那项）。
* 用户裁定记录：`tool-browser` 在五件 P2 中排最末（2026-09-30）。

**本节原表里已失效的 6 处 ✅（批 E 逐条订正，写在这里是为了让「按老表干活」不再发生）**

| 项 | 原表陈述 | 现况 |
|---|---|---|
| `llm-openai` / `llm-ollama` | ✅ v2.8 宿主侧 llm 模块（**genai 三合一**） | genai 依赖随 **v2.75 全删**，现役 = 自建 OpenAI 兼容客户端一条线协议；kind 只决定鉴权头与端点归一 |
| `tool-shell` | ✅ v2.14（**白名单 + denylist 永远赢** + 元字符拒绝） | **v2.145 命令级禁令整批退役**：Deny 仅剩结构性拒绝，其余进审批卡；另有 `shell_read` 免审面（证明层 v2.135 + EXACT_ARGV v2.145） |
| `tool-fetch` | ✅ v2.9（GET-only） | ✅ 仍在，但副作用自 v2.113 起 = **Irreversible**（原标 Pure 作废），且展示元数据自 v2.146 起与代码同步 |
| `ctx-compact` | ✅ v2.19 配置驱动收口（三策略/阈值 50%/事件+撤销） | **已退役**：桌面压缩链 v2.137 整面退役（整理接管），本体 v2.147 批 C 摘除；只剩 `CompactionOutcome` 重放面 + `estimate_tokens` |
| `memory-vector` | ✅ v2.28（experiences.sqlite3 + ExperienceSource 契约） | **experiences 已随 v2.136 删除**（含内核契约）；向量基建以宿主共享件 `llm/embed.rs` 存活，服务 `search_titles` / ToolExposure 第三路 / `search_tools` |
| `tool-sandbox` | ✅ ⑯ v2.31 受限令牌 + JobObject | **整目录退役（v2.145）**：不隔离 fs/网络即非安全边界，第二入口只多撞锁面；已存会话的 `sandbox_run` 按钉死集回映射 miss 自然消失（前端仅保留历史日志重放映射） |

另两处旧措辞随批 E 一并纠正：`tool-fs` 的 `Compensable{fs.restore}` = 点号旧名，现役 `fs_restore`（v2.25 起因 OpenAI 兼容接口拒收点号全表改下划线）；`tool-browser` 由「⏸ 最后」细化为「◐ 预览面已交付、自动化面未做」。

## 五、功能层排期（2026-10-04 规划：UI shell 文档内设计项已全部落地（㉗·C），转入功能面——PI-Desktop 全功能面借鉴盘点 + 既有欠账合并）

> 盘点基准：PI-Desktop（F:\\Warehouse\\Github\\PI-Desktop）源码与 spec 共 54 处。 取舍原则照旧：后端服务前端、不发明占位、已有等价实现不重复借、tool-browser 仍压轴（2026-09-30 用户裁定）。批首照例文档先行（新增设计项成文，不改既有规格）。

### 已有等价实现（不重复借）

⑲ 会话持久化 / ⑨ 压缩三策略 / ⑳ 流式 / ㉗·A 通知 / ㉗·B 工作面板 + 子转录页签 / ㉗·C diff 审阅页签（数据源弱于 PI 的"工具执行期留底"，加固见 ㉝ 搭车）/ v2.49 会话 搜索 + 导出（PI 反而没有导出）/ ⑫ search_tools ≈ PI ToolSearch 按需检索 / ⑭ 定时触发引擎（缺目的地页，见 ㉙）/ ⑮ 子 Agent 单层同步（升级见 ㉜）。

### 建议批次（顺序待用户拍板）

| 批 | 内容 | 规模 |
|---|---|---|
| ㉘·B（先动，已排期） | 权限与围栏：PI 决策表序（合同/计划硬拒 > 外部路径门 > 低险直通 > 模式放行 > 会话豁免 > 决策卡）+ 四档切换器挂输入框旁（文档先行补 §3.x 新行）+ 工具围栏读调用期根（session_context 槽已就位：tool-fs / tool-shell 路径类 / tool-sandbox，回落 env 全局）+ 外部路径低险也审 + 审批卡环境行如实（§3.17 占位 fsRoots\[0\] 纠偏，载荷 +workspace） | 中大型 |
| ㉙ 会话运营补全（✅ v2.57） | 消息编辑重发 + 定时任务目的地页 + LLM 自动会话标题 + thinking 数据源——四件全落（见上方 ㉙ 块） | 中 |
| ㉚ 模型体系（✅ v2.58，见 CHANGELOG） | llm_list_models 后端（Ollama /api/tags + OpenAI 兼容 /v1/models 探测）→ §3.30 模型选择器（composer 模型 chip 变选择器 + 会话级覆盖，settings 缺省）+ provider 预设档（多 provider 配置管理——模型分区大改版本就"含后端逻辑"，在此一并收）+ 搭车用量明细（PI message-usage：六类 token / cache 率 / tokens per sec 进 ContextUsagePopover） | 大 |
| ㉛ Composer 三件 | 附件（图片起步；后端消息分段通路，§8.2 新行）+ @ 文件提及（fsRoots 界内枚举）+ 提示词增强（一次性改写 + 超时 + 开关，PI prompt-enhancement）+ 权限 chip（与 ㉘·B 模式切换器同位联动） | 中 |
| ㉜ 子 Agent 升级 + 队列 | 后台子 Agent（PI Task/TaskWait/TaskList/TaskStop 语义；⑮ 现为同步单层）+ 用户自定义子 Agent 预设（模型/工具面/思考档钉死）+ 运行中队列与插话（PI turn_queue：提示词排队持久、可取消、可插话 steering——热插话先做，持久队列看需求） | 大 |
| ㉝ 插件生态批 | plugin-api 贡献槽位（§2.5，工具已通；命令/主题随批定）+ tool-http（P1 唯一欠账）+ mw-ratelimit + Skills 技能包（PI user_skills：SKILL.md + Skill 工具 + 斜杠调用；宿主侧/插件形态批内定）+ 搭车审阅快照加固（PI review.rs：写类工具执行期于工作区外留底图 + 哈希守卫回滚，增强现 edit_diff args 单点） | 大 |
| 杂项随批搭车 | 会话保留天数可配 / 长任务 keep-awake / ask 结构化提问工具（PI asktool 问卡，模型主动向用户发问）/ 启动恢复面动作化 / profile.toml 接线 / kv_get 协议 / 市场 wasm 分发 | 小 |

### 明确不借（阶段理由）

语音输入与实时语音 / 多端加密同步（WebDAV）/ 远程 host（SSH 隧道）/ MCP 市场 + OAuth （tool-mcp 本地 mcp.json 已覆盖起步场景）/ 浏览器预览面板（随 tool-browser 压轴批再议）/ 嵌入式终端 / 编辑器（VS Code）集成 / 遥测（PI 也没有，同取"无"）/ Worker 全会话编排器 （㉜ 的远期形态，先不做）。