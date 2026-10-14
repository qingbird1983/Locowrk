# Locowrk 落地路线表（ROADMAP）

> **这是唯一的进度看板。** 每完成一批：把 ⬜ 改 ✅（或 ◐ 改 ✅），补上版本号 和提交哈希。设计细节不看这里——每行的"详见"列指向 [DESIGN.md](http://DESIGN.md) 章节或配套 文档。
>
> 图例：✅ 已完成 · ◐ 部分完成（剩余项已拆进具体批次）· ⬜ 未开始 · ⏸ 后排（看需求再动）
>
> **批次叙述的在役窗口（v2.195 文档预算批立）**：本文件的批次叙述区只住**最近 8 批**；第 9 批进来时必须把最老一批**逐字原样**追加进 [archive/ROADMAP-batches.md](archive/ROADMAP-batches.md) 正文最前（倒序），本文件只留一条指针。排期表（§一 / §二 / §二·五 / §三 / §五）不受窗口影响 —— 它才是「唯一进度真源」。判据 = 第七道结构闸 scripts/check-doc-budget.mjs：它数**块数**而不只比字节，因为单批靠压措辞能混过字节闸，混不过「块数不超过窗口」；「最老那一批到底有没有真搬走」必须由机器盯。为什么立这条：本文件在 v2.195 之前长到 694.6 KB，其中约 550 KB 是 61 批的历史叙述，每批收口都要把它们全量读一遍；同族先例 = 批 E 把 AGENTS 的历史流水搬出（当年 94% 字节是流水）。

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






> **更早批次流水已搬出归档**：v2.120–v2.193 累计 68 批 / 归档现 520292 字节，**逐字原样搬出、内容未改写**（终态校验 = 行数守恒 / 行多重集守恒 / 每条与搬出前的行区间逐字节相同）→ [archive/ROADMAP-batches.md](archive/ROADMAP-batches.md)（倒序，最新在上）。在役只留最近 8 批 + 这条指针；本次搬出  1 块 / 24463 字节（v2.193）（脚本 = `tmp/doc-trim2.py`，口径见 AGENTS「工作规则」与队列 §7）。

### v2.201（提交 `PENDING`；⚠ 本条哈希由同批回补提交落下（照 `c052318` / `4755e8b` / `86bb4df` / `18fca76` / `2f97c55` / `3083990` / `27bab62` / `af6a77d` / `741b751` / `b97dd3c` / `c9fdcd4` / `87322e1` / `9d3b977` / `deac6ee` / `a27c54a` 先例，不留「下一批回补」的悬空账））· **队列 Q10 落地 = 四信号 breakdown 落观测面 + 管理页只读解释区块：可解释的正解是「发生那一次存回执」，不是「读的时候再算一遍」**

| 项 | 内容 |
| --- | --- |
| 依据 | 队列 §4 阶段三第二刀 **Q10** = ML 稿《Memory Learning Mechanism (SME Alignment) v1》§3.1 原则 4（"每个信号都要可解释"）+ §5.1 的 L4 档（Q9 已落 L2/L3，稿把 L4 单独排一批）。顺带 MR 稿 §8 的 **M3 遗迹清理**：本批**只取证不动盘**（涉用户数据目录，见「已知边界」行） |
| ① 形状：`rank` 返回 `Scored` 而不是 `Candidate` | 分数在排序那一层**算出来就必须带得出去**。反面是"读的时候再算一遍"：再算要的是同一个 `now` 与同一份词表，跨会话重放根本拿不到（词表由最近三条用户消息切出），于是给出的解释与真发生过的那次不一致——而"为什么排在这"这句话一旦有一次是编的，面板整体就不可信了。`Scored { cand, scores }` 与 Q1 的 `keys` 列同一条纪律：回执记的是**那一次** |
| ② 落盘：`memory_inject_log.breakdown` 一列，**三档 NULL 必须可区分** | ①检索档（`note_receipt`）根本没有加权这一步 ⇒ NULL；②注入档但学习档关着（走现役 SQL 本体）⇒ NULL；③注入档且开着 ⇒ 那一次的 JSON。列形 `breakdown TEXT` **不给** `NOT NULL DEFAULT ''`——空串会把①②读成"有回执但回执为空"= 假数据（与本模块 `store_empty_ratio` 用 `None` 不用 `0.0` 同一条口径）。判据 S13 钉"检索档这一列恒 NULL"，防的就是后来某批"顺手补个分数" |
| ③ 迁移：存量库补列（`CREATE TABLE IF NOT EXISTS` 不补列） | 形状逐字照 `memory.rs::ensure_learning_schema`（探针 → `BEGIN IMMEDIATE` → 等锁后二次探针 → 补列 → COMMIT/ROLLBACK，幂等可自愈）= `recall_log/schema.rs::ensure_breakdown_column`，第六次用同一把尺，不另发明一份。**本机实测**（只读 `mode=ro`）：`usage.sqlite3` 的 `memory_inject_log` 现为 9 列 / 43 行（inject 5 + 检索档 38），`breakdown` 列**不存在** ⇒ 新码首次启动会真迁一次；`ADD COLUMN` 只增列、不改任何既有字节与行 |
| ④ 读端：`inject_view.rs` = 这张表的**第一个生产读取点** | `stats()` / `recent()` 的 `allow(dead_code)` 因此摘掉（第六道闸立的那条：接线了却不摘标注 = 判红）。两条口径写进模块头：**找"最近一次真算过分的注入"而不是"最近一次注入"**（关档会话照常埋点但列是 NULL，把它当"没什么可说"就挡掉了用户想看的那条）；**解析失败不上抛**，给 `unreadable = true` 让面板如实说"读不出来"——整条通路报错 = 观测面判死自己的数据源，连"注入了几次、空块率"都看不见。空交付 `{"rows":[]}` 是**有效回执**（Q1 空块率判的行），不是坏字节档 —— 这一条是本批自己写错一次后抓回来的（见判据自纠 131 的第二面） |
| ⑤ 界面：记忆页第三区块 `InjectSection.vue`（只读） | 先例 = 同页 `TreeSection`：自持状态 + 打开拉一次 + 手动刷新，**不推送不轮询**；区块头 12/600、空态 12/次字色、错误态 danger 底 —— 三态（加载中 / 没有可解释的行 / 有回执但读不出来）文案各一条，不共用。**占比用 mono 数字而不是条形**：四段占比条是《Front-end Design Guidelines》清单外形态（§零.0 自发明判错 + §零.5 特例申请制），本批没有申请，就不造那个条；数字同样回答"哪一档把这条抬上来的"。IPC 载荷 snake_case 与 `MemoryRow` 同形；命令 `memory_inject_view` 走 `check-commands` 三向闭合（86·86·86） |
| ⑥ 拆簇两件 + 测试目录一件，全部逐字节搬移 | `memory_learn.rs` 420 行（本批把 explain 面加进去后越过 400 建议线）⇒ `git mv` 目录化 = `mod.rs` 140（SQL 面：候选 / 计数 / 写回）+ `signals.rs` 155（四信号算式）+ `order.rs` 85（排序 + E5）+ `explain.rs` 95（占比 + JSON）；`recall_log.rs` 同理 = `mod.rs` + `schema.rs`（库形状：DDL / `MODE_INJECT` / 迁移）。拆簇脚本 `tmp/q102-split.py` 自带四条自检（块起点必须是 item 首行 / 行区间不重不漏 / 每块在原文逐字节存在 / mod.rs ≤400），**唯一一处字面改动** = `PERMILLE` 从父模块私有升 `pub(crate)`（子簇 `tests.rs` 不是 `signals` 的孩子，祖先私有项对它不可见；抄第二份刻度常数 = 同一读数两个出处），**唯一一句注释改动** = `rank` 头那句"判据全在这一层"改成"全在纯函数层（signals + order）"——拆簇把一个不存在的事实写进了注释，比不搬更坏。测试面 `memory_learn/tests.rs`（315 行 + 本批 6 条会越线）⇒ `tests/{mod,formulas,ordering,receipt}.rs` 三簇，分簇口径 = "这条判错了你会去改哪个文件" |
| ⑦ `retention`（日志多久清一次）**决定不做**，登记为挂账 | 做了它要新增删除行为 + 一个设置键 + 校验 + 界面四件套，而收益现在不可见：本机 43 行 / 三个半月，`breakdown` 每行约 0.3–0.6 KiB，注入档还按会话去重（每会话每键位一行）。⇒ 挂账触发条件写死在队列 Q10 行尾：**当 `memory_inject_log` 的行数或字节数成为可观察的负担（面板读盘明显变慢、或表过十万行级）再立批**，届时先量的也是"读一次多少毫秒"而不是先写删除代码 |
| 门禁 | 见 CHANGELOG 本版行（17 项动作全 rc=0 + 形状判据 21 条 FAILED=0 + 变异 6 靶全见红） |
| 判据自纠 **10**（本批第 131–140 条；累计 **140**，起点 130 = 上一批） | 见 CHANGELOG 本版行同段（两处文字由脚本从同一份数据生成，不手抄两遍 = 历批第 116 条立的规矩）。最值两条：**131** 手搓 JSON 转义只挡 `"` 与 `\`，键里一个裸控制字符就产出**非法 JSON**——比"没有解释"更坏，因为它让那一行的观测整体失明，而 `serde_json` 一直躺在依赖里；**132** 一次 `git mv` 之后代码注释里 **13 处**路径指向已不存在的文件（8 处 `recall_log.rs` + 3 处 `memory_learn.rs` + 2 处 `memory_learn/tests.rs`）⇒ 目录化那一刀的**同一批**必须把 `grep -rn "旧名\.rs"` 打到 0，而不是留给收口批 |
| 已知边界与真机走查 | ①**真机走查未做** ⇒ 判读清单交用户动手：记忆页出现第三区块 / 关掉学习档时该区块说"还没有可解释的注入"而**不是**报错 / 开档跑一轮后区块列出交付的每条与四档占比、四档相加 = 1000 / 首启时 `usage.sqlite3` 被补上 `breakdown` 列且旧 43 行读数不变（`PRAGMA table_info` 可自证）。②面板显示的是"最近一次真算过分的那次注入"，**不是当前会话**——按会话预取缓存的形状，同会话内块字节钉死（M1 纪律），所以改完权重要看**新会话**那一行。③占比数字与系统块里的顺序同源，但 `share` 是**加权后**的占比：权重给 0 的那一档占比必为 0，即使它的信号值在场（用例钉住）。④遗迹清理只取证：`~/.Locowrk/memory/memory.sqlite3` 0 字节 0 表（现役代码零创建点，`grep` + `git log -S` 双向空，来历不猜）、`~/.Locowrk/experiences.sqlite3` 16 KiB / `experiences` 表 0 行（v2.136 提交 `e28a3c9` 退役 experiences.rs 后无读写点）。两件一律**移入 `~/.Locowrk/backups/legacy-<ts>/` 而不是删除**，且**等用户点头才动盘**（工作规则：删除/覆盖/迁移先确认）。⑤`memory_learn` 的 `SCAN_LIMIT = 200` 仍是候选上限，本批没动它，也没给它加索引 |
| 文档 | 本批登记：ROADMAP 本节 · CHANGELOG 本版行 · 队列 §4 Q10 行 ⬜→✅（含 retention 挂账触发条件）· ML 稿状态"部分兑现"→**§3.1 原则 4 / §5.1 L4 兑现注** · INDEX 该稿状态行 · AGENTS 轮转（v2.200 整条降「此前进度」不作压缩；被挤出的那条逐字搬进 `archive/AGENTS-progress-archive.md` 正文最前）· ROADMAP／CHANGELOG 两份在役文档的 8 批窗口各自滚出最早那一批，逐字搬进 `archive/` 对应批次归档正文最前，在役文件只留一条指针（判据 = 第七道闸 `check-doc-budget`） |

### v2.200（提交 `a27c54a`；⚠ 本条哈希由同批回补提交落下（照 `c052318` / `4755e8b` / `86bb4df` / `18fca76` / `2f97c55` / `3083990` / `27bab62` / `af6a77d` / `741b751` / `b97dd3c` / `c9fdcd4` / `87322e1` / `9d3b977` / `deac6ee` 先例，不留「下一批回补」的悬空账））· **队列 Q9 落地 = 记忆注入的四信号加权排序 + 交付即命中（ML 稿 L2+L3 合一批）**

| 项 | 内容 |
| --- | --- |
| 依据 | 队列 §4 阶段三第一刀 **Q9** = ML 稿《Memory Learning Mechanism (SME Alignment) v1》§3.3 排序 + §3.4 强化回路（稿内 L2/L3 合一批，§5.1 明写可合并：同一函数同一锁）。前置 Q5（学习两列）与 Q2（相关性要会话消息流）都已在场；库里有东西（本机 `memories` 全局层 6 行）也是 Q9 的前置，已实测。用户指令「**Post-Audit Roadmap v1.md 按照这个路线走**」= 把 v2.199 那句「下一刀转主线」具体化为**严格按 §4 排队顺序**，本批起走 Q9。 |
| ① 契约形状：`RecallQuery` 捆住「词表 + 权重」而不是两个参数 | `MemorySource` 加**带缺省实现的** `recall_relevant(session_id, limit, query)`：缺省支忽略 `query` 回落 `recall()` ⇒ 五个测试 fake 与 headless 装配零改动，而 `LearnWeights::default().enabled = false` 让这个缺省支**就是现役行为**（等价性由类型给出，不靠测试近似）。词表与权重捆成一个载荷的理由是**「关掉学习档」只许有一个表达方式**：分两个参数就能传成「词表非空 + 档位关」的半开态，宿主就得再判一次"到底算不算开"。`LearnWeights.enabled` 的契约缺省（关）与 `MemoryInjectConfig` 的产品缺省（开）**语义相反是设计**，不是两处出处。 |
| ② 落点：打分与写回全部住在新文件 `memory_learn.rs`，宿主只做装配 | 四信号（`relevance` / `strength` / `freshness` / `recency`）现算、tie-break、E5 保留槽、候选 SQL、写回 SQL 全在 `memory_learn.rs`（289 行，纯函数面不碰 sqlite ⇒ 判据能在无库情况下钉死）；`memory.rs` 的 `recall_relevant` 覆写只负责连接、观测与降级。为什么新开文件而不是堆进 `memory.rs`（本批后 902 行、在 BASELINE 名单内）：按工作规则「文档未规划的新职责 = 先建新文件」，且这是**新职责**不是既有职责的延长。 |
| ③ 对稿的 **8** 条实测订正（逐条在代码注释有落点） | **一** `freshness` 的 `?? created_at` 落空：`memories` 表从未有 `created_at` 列（Q5 只加两列），旧行也回填不了 ⇒ 兜底取 `updated_at`，于是**未命中期它与 `recency` 共线**（同一年龄、不同半衰期）；这条良性共线由用例 `freshness_falls_back_to_updated_at_when_never_hit` 钉成"写下来的形状"，防将来被当 bug 顺手改。**二** 稿写 float 权重，实现换**千分比整数**：配置面靠 `PartialEq` 判变更、浮点既不进 `Eq` 也不闭合。**三** 相关性匹配面**截到注入上限**（复用内核 `VALUE_INJECT_MAX_CHARS`，宿主侧不许再写 500）——不截就是给「模型根本看不见的第 8000 字」让路。**四** 候选集**有界 200**，不是稿 §5.3 那句"全量取"（value 单条可达 256KiB，无界读取没有上界保护；E1「打分必须发生在比交付更宽的候选集上」仍照做）。**五** 写回粒度 = **每会话一次**而不是稿 §3.4 的"每次 recall 调用"：桌面链的会话预取缓存决定一个会话只有首查真走到这里 ⇒ `hit_count` 的单位是会话不是轮次（方向上是把稿 §6.1 的污染从 ×轮 压到 ×会话，但 A/B 读数必须按这个口径解释）。**六** 稿 §5.4 的「权重和为 1」从硬约束降为**缺省值恰好和为 1000**：排序只取决于 Σ 权重×信号（`total` 不做归一除法），硬约束会逼出「改一个滑条就把余量摊给另外三个」的联动机制；全 0 因此是合法档（= 退回现役序）。**七** E5「恒保 ≥1 条最近更新」在 `top_k = 1` 时**如实不成立**：保留槽花的是一个相关性名额，单条档的花费是 100%，唯一名额给"最新"会让"问部署却看到今天天气"，所以 `limit >= 2` 才做，用例 `reserve_yields_to_the_score_when_top_k_is_one` 把这个取舍钉住。**八** 稿 §3.1 原则 4「每个信号都要可解释」本批**不兑现**——breakdown 属 L4 = 队列 Q10，注入形状 `MemoryEntry` 因此不许带学习字段（判据 S10 钉；把 `hit_count` 写进 system 既花 token 又把机器状态摆给模型看）。 |
| ④ 等价性的两端各有一条用例 | **生产端**：关掉学习档走的是 `Self::recall` **本体**，不是另写一份等价老 SQL ⇒ "逐字节回到现役"由**调用图**给出（判据 S4，窄靶 M3 证明它抓的就是这条纪律本身：改成就地重写一份等价映射后 cargo 全绿、只有 S4 红）。**算术端**：`zero_weights_degrade_to_current_ordering` 钉"权重全 0 ⇒ 排序完全由 tie-break 决定，而 tie-break 逐字等于现役 `updated_at DESC, key`"。观测面也同端：`log_inject` 拆成键位版 `log_inject_keys` 而不是改签名，**现役 `recall_quiet` 的调用点一字未动**。 |
| ⑤ 设置面：五键 + 界面 | `memory.learnEnabled`（缺省**开**）+ 四档权重（0–1000 千分比，出界 = `settings_set` 拒绝并**点名是哪一档**）。装配点唯一 = `settings.rs::memory_inject_config()`；缺省数字唯一真源 = 内核 `LEARN_WEIGHT_DEFAULT` 那一个数组（设置面逐档取 `LearnWeights::default()`，判据 S6 钉）。界面 = 记忆区一条开关 + 四条数字输入（`v-if` 展开，照 `factCardMax` 既有形态，不自发明滑条）。⚠ **改权重的生效时机 = 下一个会话**：会话预取缓存只认 `(session_id, scope_id)` + `top_k`，块字节会话内钉死是 M1 纪律；买一次"立刻重算"的代价是打断本轮缓存前缀而换不到任何正确性（归属变了必须重算，偏好变了不必）。这个形状的承重 = 内核用例 `memory_inject/tests/learn.rs::cjk_query_produces_terms_and_hits_prefetch_once` 断言同会话跑两轮 `seen.len() == 1`（`PrefetchEntry` 的字段只有 `block`/`top_k`）——谁把权重塞进失效判据，第二轮就重查，它当场见红；真机走查按此判读，别当 bug。 |
| ⑥ 用例 **29** 条新增 | 内核 `memory_inject/tests/learn.rs` 7 条（中文 2 字滑窗切得出词、词表随会话预取只下传一次、关掉时词表与开关一起归零、取数窗口只数用户消息、去重与钳制、关掉时块字节逐字相同、块内不带学习字段、缺省 trait 支回落 `recall`）；宿主 `memory_learn/tests.rs` 14 条（四信号各自值域与饱和、共线兜底、匹配面截断、全 0 退化、相关性 vs 新鲜度、强化史抬序、E5 顶替与让位、整体缩放不改序、脏值不溢出）；宿主端到端 `memory/tests/recall_learn.rs` 4 条（真库真 SQL：加权排序 + E5 顶替 + 落选不计数 + 写回不刷新 `updated_at`、关档与现役逐字段相同且一次写回都不发生、全 0 档仍写回、库打不开时同档降级）；`settings/tests/learn.rs` 4 条（缺省与桥接、部分更新、投影五键、校验放行任意和/拒绝出界）。 |
| 拆簇（新增超限件的正解，不抬基线） | 本批一度把两件推过 400 行闸：`memory_inject.rs` 580（HEAD 382）与 `settings/tests/memory.rs` 551（HEAD 386）。处置 = 拆而不是改基线、压注释或在文件间转字段：①`memory_inject.rs` 目录化（`git mv` 保历史）成 `mod.rs` 329 + `contract.rs`（契约面：`MemoryEntry` / `MemorySource` / 配置与权重类型）+ `terms.rs`（切词三函数），**正文逐字节搬移**，根侧测试数拆前拆后都是 505 = 零行为变化的读数；②`settings/tests/memory.rs` 的学习档断言全搬进新簇 `settings/tests/learn.rs`，既有文件回到 396 行，两笔 `MemoryUpdate` 字面量只补 5 个 `None`（**显式列字段**是有意保持的性质：新增设置项必须被显式考虑过）。搬移中 `set_memory` 这颗 helper 被我在两簇各写一份 ⇒ 第五道闸报双向「私有项被兄弟簇引用」⇒ 按判据建议**上收 `settings/tests/mod.rs`** 而不是改名把重复藏起来。终态超限 **22 == 基线 22**。 |
| 门禁 | **根 505 passed / 桌面 540 passed**（对账 = 桌面 +22 正是本批宿主新增数，根 +7 正是内核新增数 ⇒ 没有用例被静默丢弃）；两侧 fmt + clippy `-D warnings` 全 0；前端三件全 0（`pnpm build` 含 vue-tsc、stylelint、check-style 91 文件干净）。**七道结构闸全 rc=0**：`check_length` 超限 **22 == 基线 22（零新增）**、i18n **zh=923 en=923**（+10 = 本批界面十串）、commands **85·85·85**、wiring **10·10·0**、形状闸 **SPLIT_SHAPE_OK**（S0–S6 全 0，目录式模块 36 个）、预留对账 **517·3·5·0**、第七道 **在役块数 8 · md 断行 0**。本批形状判据 **11 条 FAILED=0**。变异 **6 靶全见红**（宽靶 2：加权不产生区分度 / 宿主不再覆写 ⇒ 断言面红；窄靶 4：SET 多写一列 `updated_at = updated_at`、关档改为就地重写等价映射、缺省权重改手抄字面量、匹配面把内核常量换成 500——**四条都行为等价、cargo 全绿，各自只让 `S1/S4/S6/S7` 一条红**，证明这四档独有），逐条 `restored=True`、还原后基线复绿。插件无需重编（trait 扩展带缺省实现，插件只链 `plugin-api`）。 |
| 判据自纠 **14**（本批第 117–130 条；累计 **130**，起点 116 = 上一批） | 1.**117 = 测试夹具的时间基准必须与生产同源**：夹具写死 `NOW = 1.7e9`、`rank` 用 `now_secs()` ⇒ 真实当前时间与夹具差 3360 天，两个时间信号一起塌到近 0，`strength` 反而赢了排序——第一次实跑读数 `["mid-hot","new-noise"]` 看着像"算法不对"，实际判的是夹具与生产不同源。写死常量在这里**更像可控**，恰恰把被测关系测没了。2.**118 = 第一条测试把实现按崩**：`strength_permille(i64::MAX)` 在 `1000 * h` 处 multiply-with-overflow（debug panic、release wrap 成**负信号**——把"被用过很多次"压到所有候选之下，比不钳更糟）。⇒ 算式改恒等变形并钳位。3.**119 = 判据的允许集不能按记忆写**：S2 第一版预设"宿主 + 插件两个文件"，实测只有 1 个（Q5 给插件侧建了列却从没给它读取点）⇒ 按实测重写。历批那句「禁词串须先在 HEAD 里验存在」的同族。4.**120 = 同一个字面量不许被两条判据按两种语义各数一遍**：S6 数到 `SCAN_LIMIT = 200` 判红，而 S8 正是专门管这颗 200 的 ⇒ 先红的那条会被当成"实现有问题"去改，实际是判据口径打架；S6 口径改成 400/250/150。5.**121 = 判据里的文件路径也是口径**：拆簇后 `memory_inject.rs` 变成 `mod.rs` + `contract.rs`，S3/S6/S10 三处的读数来源必须跟着改，否则要么假红要么**静默空转**。6.**122 = 新增 helper 前先数出处**：见上表拆簇行（`set_memory` 双份 ⇒ 双向报红 ⇒ 上收）。7.**123 = 插入式编辑第三次吃掉锚邻近文字，这次长在代码上**：本批两次 Edit 删掉了不该删的相邻行（`scope_id` 的函数体、`TailStrategy` 的 `#[derive]` 行），一次插入留下**重复的 format 具名参数** `n = NOW, n = base`——三者都会被编译抓到，但都发生在"改完没立刻验"的窗口里。规矩 = **结构性插入之后马上 `cargo check`，不攒到最后**；卸载脚本这类按行号操作必须**删完立刻测括号平衡**（本批就靠这一步抓到少删的一行 `}`）。8.**124 = 按想象中的 API 写测试**：Rust 里「no two closures have the same type」⇒ 闭包数组编译不过；`SettingsStore::update` 是 **13** 个位置参数而我按 14 写了一版；`set_memory` 的返回类型我猜成 `Result<(), String>` 而真身是 `Result<SettingsView, String>`。三处都是"我以为的签名"，与记忆里那条「夹具字段必须照生产代码抄」同族。9.**125 = 环境事实**：本机 rustc 1.98.1 上 `i64::div_ceil` 仍报 **E0658 unstable `int_roundings`** ⇒ 向上取整一律写算术形式，不为一档饱和曲线开 feature gate。10.**126 = 已登记的坑本批又踩两次**：①`cargo clippy … \| tail` 把退出码盖成 0（历批 F-4 立过，这次靠输出内容而非 rc 判读）；②Python `subprocess` 按 cp936 解 stdout ⇒ `UnicodeDecodeError` 把三条窄靶的判读冲成"没抓到"，修成 `encoding="utf-8"` + 结果只从 `tmp/q101-mutate.out.txt` 取，中文不再过终端。**这两条的害处在于它会给出可信外观的假判**——第一次跑出的 MISSED=3 完全是测量工具坏了，不是守卫失效。11.**127 ⚠ 文档里写了一条根本不存在的判据名**：`prefetch_entry_carries_no_weights` 同时落在 `settings.rs` 头注与本节 ⑤ 行，收口时 grep 才发现它的命中面**只有我自己写的这两句**——起草时它是我「认为本该有的守卫」，从未写进 `tmp/q101-shape.mjs`。处置 = 把措辞改成指向**真实在场**的承重（内核用例 `memory_inject/tests/learn.rs::cjk_query_produces_terms_and_hits_prefetch_once` 的 `seen.len() == 1` 断言 + `PrefetchEntry` 字段只有 `block`/`top_k`），**而不是**为了圆文档再造一条判据（那只会多一条没人要的守卫加一次全量重跑）。同族的第二面：本行计数句与任何判据/用例名一律**从磁盘现取**，先 grep 命中 ≥1 再落笔（历批 Q8c「注释指向不存在的测试」的第二次兑现）。。12.**128 ⚠ 门禁脚本里的「`cd` + 相对路径重定向」会让命令根本没跑，而台账行一起丢**：`run()` 写成 `"$@" > tmp/x.txt`，脚本中途 `cd desktop/src-tauri` ⇒ 重定向目标指向**不存在的** `desktop/src-tauri/tmp/`，bash 打不开文件就**不执行命令**（rc=1 是重定向给的，不是构建给的），同目录的 `echo ACTION … >> tmp/gates.out.txt` 也因同一条理由失败 ⇒ 17 个动作只落下 12 行台账，末行却仍打 `ACTIONS_TOTAL=17 ACTIONS_FAILED=5`（计数器是 shell 变量，活在另一个目录里）。**只判 FAILED 数分不清「红了」和「没跑」** ⇒ 处置 = 台账与逐项输出一律绝对路径，脚本末尾自证 `ACTION_ROWS=n` 且 `n == ACTIONS_TOTAL`，不等就 `exit 8`（本批第二次跑才拿到 17 行全 rc=0）。13.**129 ⚠ 历批脚本里「按行号定位文档那一行」会被本批自己的窗口搬出作废**：登记第 128 条的第一版沿用 `rl[716]`，而本批 `tmp/doc-trim2.py --apply` 刚把 v2.192（19 行）搬出 ROADMAP ⇒ 716 现在是**门禁行**，锚命中 0 直接 BLOCK 在写盘前（磁盘未动）。历批立过「跨提交复跑的参照要按内容定位」，这是它的**文件内部行定位**那一面 ⇒ 规矩 = 文档行一律按 `startswith(本批特征串)` 定位并断言命中恰好 1（本批三处文档同改，各用各的特征串）。14.**130 ⚠ 一句叙述里混掉了两件不同的「搬出」，落盘时自己没看出来**：本批「文档」行原写「AGENTS 轮转（v2.199 降「此前进度」，被挤出的 v2.192 逐字搬进**两份批次归档**）」——可 AGENTS 归档被挤出的那条是 **v2.198**，v2.192 属于 ROADMAP／CHANGELOG 那一窗口的两份**批次**归档。两个不同的搬出共用一个动词，就写成了一句假话，而第七道闸只判块数与表格断行，**看不见语义错乱**。⇒ 规矩 = 凡「搬出／归档」类叙述必须写明**哪一份文件、哪一个窗口、哪一批**，一份文件一句话；同一句里出现两个以上「搬出」对象时拆开写。同族第二面（**同一脚本、同一次收口 ⇒ 并进本条不另立条数**）：`tmp/q101-verify.mjs` 的 E0 主提交定位式只按「subject 含 v2.200 + 排除 `docs(回补`」，于是**补登那一笔落下后**命中 2 笔、MAIN 取到最新那一笔、它的父就成了「回补提交自己」 ⇒ E1a/E1c 连锁假红（历批第 32 条「态要枚举到底」的第四面）。处置 = 主提交一律按 **type 前缀 + 版本号** 定位（feat／fix／refactor 三选一），并把本批笔数当读数打出来（终态 `BATCH_COMMITS=3`、17 条断言 FAILED=0） |
| 已知边界与真机走查 | ①无基准数（本刀不谈性能；候选 200 条 × 8 词的内存匹配是亚毫秒档，但未实测）。②§6.1 的"检索即自我强化"仍在，只是单位从轮变成会话；`strength` 权重压在 0.25 是稿给的初值，收紧要等 Q10 的读数。③项目层不参与（v2.136 裁定原样，端到端用例里 `project-only` 那条含 "docker" 但恒不进注入、恒不被计数）。④`memory_reconcile` 的 md 回写路径不碰两列学习量（Q5 已有防回归判据，本批未新增写路径）。⑤界面判读清单交用户动手：a) 记忆区出现「按有用程度给记忆排序」+ 展开后四条权重；b) 改权重保存后**当前会话不变、新会话起效**；c) 关掉开关后注入内容回到"最近更新前 N 条"；d) 连续两个新会话问同一件事，`memories` 表里被注入那几条的 `hit_count` 各 +1（单位 = 会话）。另附 Q7d 遗留三条：设置页规则行 / 清除真的删文件 / 换写法打开同一项目不再重问授权。 |
| 文档 | 本批登记：ROADMAP 本节 · CHANGELOG 本版行 · 队列 §4 Q9 行 ⬜→✅ · ML 稿状态「待研究」→**部分兑现**（§2/§3.2 已落、§3.3/§3.4 本批落并就地标 8 条订正、L4 = Q10 未做）· INDEX 该行状态 · AGENTS 轮转（v2.199 整条降「此前进度」不作压缩；被挤出的 **v2.198** 逐字搬进 `archive/AGENTS-progress-archive.md` 正文最前）· ROADMAP／CHANGELOG 两份在役文档的 8 批窗口各自滚出 **v2.192** 进 `archive/{ROADMAP,CHANGELOG}-batches.md`，在役只留一条指针且指针自指读数重算。`tmp/q101-*` 不入库（台账 **T8** 同族）。 |
### v2.199（提交 `deac6ee`；⚠ 本条哈希由同批回补提交落下（照 `c052318` / `4755e8b` / `86bb4df` / `18fca76` / `2f97c55` / `3083990` / `27bab62` / `af6a77d` / `741b751` / `b97dd3c` / `c9fdcd4` / `87322e1` / `9d3b977` 先例，不留「下一批回补」的悬空账））

| 项 | 内容 |
| --- | --- |
| 依据 | 队列 Q7d 主体（第五个键位面收口）。用户 2026-10-14 裁定「先把 Q7d 收尾再转」= 批准取证稿《Approval Keyside Alignment and Rules Store Migration v1》**甲案**（一次性迁移，代价 = 1 个文件改名），并要在此刀收完后转主线。取证稿 §5 六条迁移动作 + §6 决策一按甲案执行；决策二按默认 = 旧件不删、移进时间戳备份的 `.legacy/` 留一版周期。 |
| ⚠ 本批第一条交付物是**订正上一批的判据设想** | 取证稿 §7 写的是「形状：E1/E2 不得再出现 resolver 直取 `.workspace`」。照它实现做不到：那两处里有一处同时喂决策卡的环境行，改吃视图产出就得**再解析一次会话上下文**，而 M-18a（批 F-24a 立的、`tests_approval/ctx_parse.rs::assert_one_parse` 五支各钉 `count == 1`）规定一次决策只解析一次。⇒ 正解不是改代码凑判据，是把判据订正成「**取用点仍 2 处 + 审批面不出现归一 + 归一唯一发生在 store**」（形状判据 K3/K3b/K4/K1），并在取证稿正文就地登记这条订正。这是历批那句「照稿开工 = 造重复面 / 落点已漂 / 清单不能用」的第六次长出，也是**第一次错在判据本身**。 |
| ① 落点 = `permission_rules.rs::keyed()` 一处 | `file_of()` 退役，换 `keyed(workspace) -> (PathBuf, String)`：文件内**唯一**一处 `plugin_api::wskey::normalize` 调用，产出「文件名 + 归一键」两半；`read_at`/`write_at` 只吃路径，`add_allow` 一次 `keyed` 拿齐两者并把 `workspace` 字段写成**归一键**。`verdicts`/`add_allow`/`clear` 三个入口自动同形，`list_all` 走目录遍历与键位无关 ⇒ 一字未动（形状判据 K5 钉住「没被顺手改」）。 |
| ② 为什么不在调用方补参数 | 入参有**三处**来自生产装配（`approval` 两处经 `ctx_of`、`commands.rs` 的删除入口经界面回传）再加一条人手改文件的路：调用方各补一次 = 下一刀新增调用点又漏一次。这一条与 v2.197 那句「16 处逐个补必然漏」同源，收口点从"视图"变成"规则库自己"，因为规则库是 `permissions/` 目录的唯一持有者。 |
| ③ 用例（1 条端到端）+ 既有判据的正当改写（3 处） | 新簇 `tests_approval/keyside.rs`：同一份规则库、Ask 档，只换 ctx 下发的写法 ⇒ 第一次弹卡按「本项目始终允许」落盘 → **② 磁盘只有一个文件且名字 = 归一键的 fnv**（反面断言：原样串那颗键位不再被生产出来，Windows 支）→ ① 换写法打开同一项目免卡直行（`rule:project-allow`）→ ③ 别的项目不许命中且不许多出一个文件。既有改写三处：`roundtrip_add_count_clear` 的 `workspace` 字段期望改按 `scope_key` **现算**（钉字面串会得到一条 Linux 绿、Windows 红的假判据）；`project_rules.rs` 与 `ctx_parse.rs` 里两处「手改 JSON 注入 deny」原本自己 `format!("{:08x}.json", fnv1a64("ws-A"))` = 把改前的键位形状钉进判据 ⇒ 上收成 `tests_approval/mod.rs::rules_file_of()` 一处。 |
| ④ 目标名不自己拍：四条磁盘已知答案反验全中 | `tmp/q100-repro.mjs` 只读。**R1** 现文件名 = `fnv1a64(json 里的原样串)`；**R2** 词法归一形**逐字等于** `memory.sqlite3` 的 `memories.workspace` 列值（`f:\warehouse\tity`，那 6 行是 Q2/Q3 由 Rust `scope_key` 亲手写进去的 ⇒ 权威答案不是我算的）；**R2b** 归一串的完整 fnv = 目标文件名 `9242b25e6d61a032`；**R3** 它的高 32 位 `9242b25e` 正是磁盘上 `memory-tree/f__warehouse_tity-9242b25e` 与 `memory/projects/tity-9242b25e` 两个归一形目录的短哈希 ⇒ **跨面同源**。R2 是本批在取证稿 R1–R3 之外新加的一档，也是唯一一条「与生产代码同源的键位」证据。 |
| ⑤ 迁移（甲案）实况 | `tmp/q100-migrate.mjs`：停机自检（`tasklist` 无 locowrk 进程）→ 时间戳备份 `~/.Locowrk/backups/permissions-<ts>/`（原件副本 + 逐文件 sha256 的 `MANIFEST.json`，备份 sha 不符 ⇒ 什么都不做）→ 阶段 A 在备份区生成新内容并逐条对账 → 阶段 B 才进正式目录、校验落盘 sha、旧件 **rename 进 `.legacy/`（不删除）**，任一步失败即回退。字段替换用**精确锚** `"workspace": "<原样串>"`，命中 ≠ 1 直接 BLOCK。对账五条全 OK：**M1** allow 5／deny 0 条数守恒 · **M2** 逐条 `tool`+`content` 字节相同（授权内容一字未改）· **M3** 字段 = Rust 亲写键位且**字节数不变**（1,087 B，`F:\\Warehouse\\TITY` 与 `f:\\warehouse\\tity` 同长）· **M4** 目录终态仅归一名在场 · **M5** 旧件 sha 与备份一致可回退。 |
| 改动面（件数现读，不手抄） | 入库 6 件代码：`permission_rules.rs`（键位收口 + 模块头「键位」条）、`permission_rules/tests.rs`（341 行 = 337 + 5 − 1，只动 roundtrip 那一处期望，余行逐字节不动）、`tests_approval/keyside.rs`（新簇）、`tests_approval/mod.rs`（`rules_file_of` + `mod keyside;`）、`tests_approval/project_rules.rs` 与 `ctx_parse.rs`（各 1 处上收 + 随之退役的 `fnv1a64` import）。文档 6 件 = ROADMAP 本节 · CHANGELOG 本版行 · 队列 §4 Q7d 行 ◐→✅ · 取证稿状态改「已兑现」+ §7 判据订正注 · INDEX 该行状态 · AGENTS 轮转（v2.198 整条降「此前进度」，被挤出的 v2.191 逐字搬进两份批次归档）。`tmp/q100-*` 不入库（台账 **T8** 同族）。 |
| 门禁 | **17 项全 rc=0（`ACTIONS_TOTAL=17 ACTIONS_FAILED=0`）**：根 **498 passed**（39 套件）／桌面 **518 passed**（3 套件）／fmt+clippy 全 0／前端三件全 0／七道结构闸全 0（`check_length` 超限 **22 == 基线 22**、i18n **zh=913 en=913**、commands **85·85·85**、wiring **10·10·0**、形状闸 **S0=0…S6=0**、预留对账 **510·3·5·0**、第七道 **在役块数 8 · md 61 份断行 0**）／本批形状判据 **11 条 FAILED=0**／变异 **4 靶全见红在断言面**（含窄靶 M4：行为等价、cargo 全绿、只有 K4 红）、逐条 `restored=True`、还原后基线复绿／迁移 **M1–M5 全 OK**（allow 5 内容一字未改，旧件在 `.legacy` 可回退） |
| 判据自纠 **6**（本批第 111–116 条；累计 **116**，起点 110 = 上一批三笔收口） | 1.**111 = 上一批自己立的判据设想做不到**（见上表「订正」行）：判据与既有承重不变式（M-18a 一次决策只解析一次）冲突时，改的应是判据并就地登记，不是把代码拧成判据的形状。2.**112 = 新用例的落点会自己抬门禁**：我最初把它写进 `permission_rules/tests.rs`（336 → 440 行）⇒ `check_length` 超限数 22 → 23 判红，而这闸判的是「不许新增超限文件」，不判该不该拆。规矩 = **新增用例前先算它落在哪个文件、会不会把超限推高**；本批改放 `tests_approval/keyside.rs`（走生产链路 = 证明力更强、零基线代价），顺带省掉一次「测试文件目录化 + 逐字节等价自证」的大动作。3.**113 = 形状判据两处口径写错**（都是假红，不是代码红）：第一版数 `self.ctx_of` 把 `with_workspace` 里的**赋值行**也算成解析点（得 3，要求 2）⇒ 口径要写"取用"而不是"提及"；K7 扫**全文**，把我自己写进模块头注释的取证举例（真实项目路径）判成「把本机目录名写进代码」⇒ 口径 = 代码面，同时把注释里的举例换成中性名（少泄露本机信息）。4.**114 = 跨脚本传读数不能按 `=` 切**：反验脚本一行打印两个读数（`MIGRATE_ALLOW=5 MIGRATE_DENY=0 …`），接收侧 `split("=")[1]` 把尾巴一起吞进值 = `NaN`，第一次迁移实跑因此在 **M1** 上 BLOCK（磁盘未动 —— BLOCK 点全部在阶段 A 之前，这是脚本形状给的，不是运气）。规矩 = **一行一键**，或取值时先 slice 到行尾再按空格切第一段。5.**115 = 窄靶必须存在**：变异 **M4**（审批侧多做一次归一）**行为完全等价**，cargo 全绿 ⇒ 它证明 K4 抓的是「归一收在一处」这条纪律本身；历批那句「宽变异证整体承重、窄变异证这一档独有」在这里的形态 = 光有功能测试，这条纪律就没有机器兜底 6.**116 = 历批第 73 条第三次长出，这次长在自证脚本自己身上**：verify 的 A2b 判「tests.rs 相对参照只动一处」用的是按索引逐行比（`a[i] !== b[i]`）——插入 4 行注释后断点以下整体错位，**一次正确的改动被读成「整片重写」** = 假红；同一支脚本的 A6 又把 git 对含空格路径加的双引号算进改动面 ⇒ 又一个假红。两次都错在**判据的读数来源**而不是代码。规矩 = 局部性一律「前段全等 + 后段全等」，取 git 面必先去引号（与 `--porcelain` 的重命名箭头同族）。同族的第四例在**本批回补时**又长一次：同一颗哈希在队列 Q7d 那一行里被写了两遍（状态格 + note 各一处），回补脚本「每文件占位恰好 1 次」的判据因此 BLOCK —— **判据没错，是写 note 时不该再抄一遍**；顺手把回补脚本改成幂等 SKIP（已回补的文件重放时判「哈希在场」再跳过，历批第 100 条同源）。 |
| 已知边界 | ①本批磁盘读数只覆盖本机（1 个文件 / allow 5 / deny 0），实现里没有一处写死这些形状（K7 钉）。②迁移只动键位与文件名，**授权内容一字未改**；旧件留在 `backups/permissions-<ts>/.legacy/`，等下一版周期再谈清理（决策二默认档，照现役 `.bak-20261004` 先例）。③决策卡环境行那串人读 workspace **仍下发原样串**（围栏面与人读面不归一，v2.197 同条纪律）⇒ 界面上看到的项目路径大小写可能与记忆层不同形，这是设计而非缺陷。④无基准数（本刀不谈性能）。⑤真机走查未做 ⇒ 收口时交付一份判读清单（设置页规则行 + 换写法打开同一项目不再重问授权）。 |
| 文档 | 本批登记：ROADMAP 本节 · CHANGELOG 本版行 · 队列 §4 Q7d 行 ◐→✅（含"判据设想被推翻"那句）· 取证稿《Approval Keyside Alignment and Rules Store Migration v1》状态 ◻→**已兑现** + §7 就地订正注 · INDEX 该行状态同步 · AGENTS 轮转 · 两份批次归档正文最前滚入 v2.191。 |

### v2.198（提交 `9d3b977`；⚠ 本条哈希由同批回补提交落下（照 `c052318` / `4755e8b` / `86bb4df` / `18fca76` / `2f97c55` / `3083990` / `27bab62` / `af6a77d` / `741b751` / `b97dd3c` / `c9fdcd4` / `87322e1` 先例，不留「下一批回补」的悬空账））

| 项 | 内容 |
| --- | --- |
| 依据 | 队列 Q7d = v2.197 补形状判据 B2 时掉出来的**第五个键位面**（登记在队列 §4 的 Q7d 行）。本批是**取证批**：零代码改动、零磁盘动作，交一份实测 + 方案 + 待裁定决策点。新立《Approval Keyside Alignment and Rules Store Migration v1》（Q7d 取证稿）。 |
| ⚠ 第一条交付物又是取证订正 | 上一批我登记 Q7d 时写的是「规则库迁移改错就是**权限面失效**」——本批实测把这句**订正一半**：归一发生在**读写两侧同时**，所以不会把 allow 变成 deny、也不会让 deny 失配；真实代价只有「已记住的授权要重答一次」（乙案）或「一次可回退的文件改名」（甲案）。原话不擦，订正写进稿子 §0 与本批第③条（判据自纠第 100 条）。 |
| ① 磁盘实测（会随时间漂，开工前必须重取） | 数据根 `~/.Locowrk/permissions/` = **1 个文件** `9467667791757632.json`（1,087 B · mtime 10-13 23:39），内 `workspace` 字段 = `"F:\Warehouse\TITY"`（**原样串**）、allow **5 条**（全是 `shell` 精确命令，内容看着是压测遗留）、deny 0 条。文件名形状 = `{:08x}` of `fnv1a64(串)`（`{:08x}` 不截断长哈希 ⇒ 16 位十六进制，本机这串恰好全是数字）。 |
| ② 三条反验全中（不许拿想象当文件名） | **R1** `fnv1a64(原样串)` = `9467667791757632` == 磁盘文件名 ⇒ 现状确证；**R2** 归一形的 fnv **高 32 位** = `9242b25e` = 记忆层已归一目录 `tity-9242b25e` 的短哈希 ⇒ 两条独立编码吃的是同一个输入串，迁移不会造出第三个身份；**R3** 归一后应落成的 `9242b25e6d61a032.json` 在场数 = 0 ⇒ 可安全改名。算式一律走 `tmp/q7d-fnv.mjs`（串从文件里读）。 |
| ③ 键位面清点（四处，全部现读） | E1 `approval/mod.rs:155` = 决策序 ③deny/⑤allow 的规则查与写（经 `ctx_of` = `SessionContextResolver` **本体**，不经视图）；E2 `:307` 审批卡载荷那条人读串；**E3 `commands.rs:228 permission_rules_delete` ← `ToolsSection.vue:109` 把 `list_all()` 返回的 `workspace` 字段原样回传**；E4 = 手改 JSON 的出口（文档明写"可作撤销授权的手工出口"）。`list_all()` 本身走目录遍历 ⇒ 与 fnv 输入无关，**不动它**（改它 = 造第二条通路）。 |
| ④ 方案 = 归一收在 store 层一处，不是三处补参数 | 落点选 `permission_rules.rs` 里 `file_of()` 之前那一处（`verdicts`/`add_allow`/`clear` 三个入口共用），理由是这里入参有**三处**加一条人写的路：调用方各补一次 = 下一刀新增调用点又漏一次（v2.197 第②条那句"16 处逐个补必然漏"在这里直接复用）。**审批语义一行不动**：`rule_matches`、③⑤判定顺序、模式放行优先级全不变 —— 本刀只让"同一个项目的几种写法落到同一个文件"。**否掉丙案（兼容读）**并写明代价落在哪一侧：双读要把 `clear()` 从"整文件删除"改成"跨文件删除 + 两次空集判断"，复杂度全部长在**权限判定路径**上，换来的只是"5 条压测规则不用重答"。 |
| ⑤ 迁移清单（甲案才需要，六条 · 逐条可回退） | 备份整目录（时间戳 + 逐文件 sha256 + MANIFEST，全成才动）→ 两阶段改名（临时名里**只精确替换** `"workspace": "<原样串>"` 一处，替换数 ≠ 1 直接 BLOCK；替换前后用锚证明字段之外逐字节不动；全成功才转正）→ 删旧文件前再与备份对 sha → 对账 M1 条数相同（5/0）/ M2 逐条 `tool`+`content` 字节相同 / M3 字段 = 归一形 / M4 旧文件名在场数 = 0。sqlite 与记忆层四个面一个字节不动。⚠ **不需要停机窗口**（本批第 103 条订正：这条直觉是从 Q7c 稿抄来的，实测规则文件只在审批那一刻读写，桌面不在跑就没有并发 ⇒ 换成"动盘前后各扫一次目录"）。 |
| ⑥ 两个待裁定决策点（他回之前不动盘、不动代码） | **决策一 · 存量去留**：甲 一次性迁移（荐，代价 = 1 个文件改名）／乙 不迁移（新授权另起文件，5 条规则失效 + 旧文件成孤儿；⚠ **乙案必须连带处理 E3** —— UI 撤销按钮回传的是旧字段原样串，指向一个再也写不到的名字 ⇒ 选乙就要在 `clear()` 里同时过一次归一，否则设置页会出现"点了清除但规则还在"，这个副作用是在列方案时才发现的，不在最初的三个选项里，本稿把它钉在决策一旁边）／丙 兼容读（不荐，见④）。**决策二 · 那 5 条 allow 的内容本身要不要清**：本刀默认不动内容只换名字；要清 = 撤销授权 = 另一次高风险动作，单独确认。 |
| ⑦ 判据设计（开工那批落，现在先立清楚） | 形状：E1/E2 不得再出现「resolver 直取 `.workspace`」⇒ B2 白名单从 {视图, approval} 收成 {视图}，且 `permission_rules.rs` 内归一调用点**必须恰好一处**（入口各归一次 = 第二真源）；行为：四种写法经归一命中同一规则文件（Windows 两支合并、POSIX 不许合，期望现算不钉字面串）；反面：`fnv1a64(原样串) ≠ fnv1a64(归一形)`（本机 `9467…` vs `9242…`）；**不变式：同一 `tool`+`subject` 的 `verdicts()` 结果迁移前后必须相同**（这条最要命——改名字可以，改判定不可以）；变异四靶各证一档独有（摘归一／入口再归／删反面／顺手"优化" `rule_matches`）。 |
| 改动面（件数由 `tmp/q97-face.py` 现算） | 入库 4 件 = 新立取证稿 1 份 + INDEX 1 行 + 队列 §4 Q7d 行状态 ◻→◐ + ROADMAP 本节与 CHANGELOG 本版行（外加 AGENTS 轮转与两份批次归档滚出）。`tmp/q7d-fnv.mjs` 不入库（台账 **T8** 同族）。**零代码、零磁盘动作**。 |
| 门禁 | 本批零代码改动 ⇒ 未重跑 cargo/前端门禁；**最后一次文档写盘之后复跑七道结构闸，全 rc=0**（读数由 `tmp/q99-gate-line.py` 从 `tmp/q99-gate-*.txt` 反解，文档不手抄）= `check_length` 超限 **22 == 基线 22（零新增）** · `check-i18n` **zh=913 en=913** · `check-commands` 85·85·85·预留 0 · `check-middleware-wiring` 10·10·未接线 0（白名单 0）· 第五道 S0…S3 = 0/0/0/0、S4…S6 见输出尾行（测试目录 18 个）· `check-reserved` 扫描 509 个 .rs·标注 3·清单 5·越界 0 · `check-doc-budget` **DOC_BUDGET_FAILED=0**（在役各 **8** 块 · 指针各 1 · md **61** 份 · 断行 0）；形状判据 `tmp/q97-shape.mjs` 复跑 = **17 条 FAILED=0**、**CALLSITES=16**（B2 仍在白名单档，等 Q7d 开工那批清零）。⚠ 本句初稿把 md 份数写成 60 = 抄的上一批读数，见判据自纠第 107 条。 |
| 判据自纠 **11**（本批第 100–110 条；累计 **110**，起点 99 = 上一批三刀收口） | 1.**100 = 上一批那句「改错就是权限面失效」写重了**（读写两侧同时变 ⇒ allow 不会变 deny；真实代价只有"已记住的授权重答一次"），登记批自己就要被自己反验一遍；2.**103 = 照上一批方案的条件抄**：我最初把"停机窗口"写进迁移动作，那是从 Q7c 稿（边写边换名会造出第三种状态）抄来的直觉，实测这一面只在审批那一刻读写 ⇒ 不需要停机；3.**101 = `node -e` 的转义层第三次吃掉反斜杠**（历批第 91 条同族）⇒ 第一次反验算出的 `5e1aa127…` 与磁盘 `9467…` 不中，错的是工具不是磁盘，靠"串从文件里读"修掉；4.**102 = 三个决策点列漏了一条副作用**（乙案下 UI 撤销按钮回传旧字段 ⇒ 静默指错文件），是写方案时才抓到的 ⇒ 决策点必须连带列出"选它会留下什么"，否则他选乙 = 我埋一个静默失效；5.**104 = 登记脚本自己踩了上一批立的第 99 条**：ROADMAP／CHANGELOG 两步是「在某锚前整块插入」，锚本身合规（不含被插入串），但缺**「新节头在场数 = 0」的前置断言** ⇒ 队列那步 BLOCK 后我改完锚重跑，已成功写盘的两步又执行一遍 = 同一节、同一行各插两遍。补的规矩（同时写进接回脚本头注）= **插入式登记必须先断言目标标题在场 0 次**，且登记/接回脚本要能从「已接回态」重放（heads==1 判 DONE 而不是 BLOCK）。附一条同源教训：接回脚本第一版的自证写成 `kept == lines[:b] + lines[c:]` 那种**构造式复述 = 恒真**，连自己下标算错都抓不住（CHANGELOG 那步崩才暴露，历批第 86 条同族）⇒ 换成三条有判据力的：被删块逐字节 == 保留块 / 行数守恒 / 接回处验邻行；6.**105 = 上一条脚本的这次复发本身就是证据**：它给 ROADMAP 源做判据句替换时锚选的是**整句**（`否则他选乙 = 我埋一个静默失效。 \|`）而替换串以「；」开头 ⇒ 锚前半句连着整条结论被吃掉，第 102 条那句"否则他选乙 = 我埋一个静默失效"在源里当场消失（本脚本第①步就是把它补回来）。⇒ 规矩的正确形态：追加式替换要么锚到句尾标点级的小锚，要么替换串**逐字重现锚原文**。同一支脚本在 CHANGELOG 源上犯了**一模一样的错**（⑪ 段那句「并在稿子里写破"不走 `node -e`"」整段消失）⇒ 两份源各中一次、都已补回，所以这条不是手抖，是**锚选成整句这个写法本身留不住锚前文字**；7.**106 = 同一支脚本写的新计数句漏了一对 `**`（源一度写作 `累计 **104，起点`）= 行内加粗未闭合**。第七道闸数的是表格断行与块数，**管不到行内加粗的闭合** ⇒ 这种破损既不红也不显眼，只有把「判据行里 `**` 计数为偶数 + 「累计 **N**」形状完整匹配」写成下限才会被抓（本批终态断言已加，并要求 verify 的 C 组同判）；8.**107 = 登记稿里的闸读数是我在跑闸之前抄的上一批值**（那句写的是 `MD_SCANNED=60`，而本批新立取证稿让 md 变成 61）：它既不红也不报错，只有真跑一次才发现 ⇒ 处置 = **文档里从此不手抄任何闸读数**，门禁句由 `tmp/q99-gate-line.py` 从 `tmp/q99-gate-*.txt` 反解后写入。历批第 ㉘／96 条在这里长出第三面：前两条讲"同一读数抄两遍必漂"，这条讲"还没测就先写上了"）。；9.**108 = 第 105 条在同批内又复发两次，这次吃掉的是尾部标记而不是前半句**：①ROADMAP 源判据行替换后，行尾的表格闭合「竖线」随锚一起消失（表格会断，第七道闸抓得到，但**是我自己写坏的**）；②CHANGELOG 源 ⑪ 段替换把整支 ⑫ 段标记「⑫**改动面**」吃掉 ⇒ 真写盘就少一整段。两处都落在写盘之前，被自家断言拦下（`endswith(「。 \|」)` 与 `index(MARK12)` 抛 ValueError），磁盘 CHANGELOG 行至今没被写坏 ⇒ 规矩补全 = **追加式替换之后立刻重测锚两端的哨兵**（句首句柄 + 行尾闭合 / 下一段标记），并把它写进 verify 的必要条件。；10.**109 = 本批 verify 在态②当场三条假红**（A2 说面上该有取证稿、E3/E4 参照版取错），根因是**判据只按"主提交未落下"这一种态写死**（历批第 76 条「判据按提交拓扑枚举态」在我自己批里复发）⇒ 处置 = 参照 ref 改成态表（态① HEAD／态② head~1／态③ head~2，一律 `~N` 不用 `^`，历批第 26 条 cmd.exe），并新立 E0「参照版拓扑成立」下限：AG_REF 那一版的当前条必须是上一批 v2.197、此前条必须是 v2.196 —— 取错版本时 E3/E4 会静默假绿，比假红更危险。；11.**110 = 上一条只修了三条判据就交卷，态③复跑时 A2 又当场红**：它照「工作树面」取改动面，而回补提交之后工作树是**干净**的 ⇒ 面为空 = 一支在终态永远红的判据（要求"提交完了还留着未提交改动"本身自相矛盾）。处置 = **面也按态取**：态③改取本批两笔提交的累计面 `head~2..head`（态①② 仍取工作树面 ∪ 未跟踪件）。教训说破 = 枚举态要枚举到"读数从哪里取"这一层，只改判据正文而不改取数来源 = 把假红挪了个位置。 |
| 已知边界 | ①本稿的磁盘读数只覆盖本机，实现里**不许把 `9242b25e6d61a032` 这类目录名/文件名写进代码**（照 Q7c 同条纪律）。②E2 审批卡上那行人读串归一后会变成小写形 —— 这是键位统一的直接结果不是显示 bug，他若要卡片继续显示原始写法是另一个决策（那就等于在权限面上再养一种串形）。③规则文件名 = 哈希对人不可读，靠文件里 `workspace` 人读字段核对；归一后该字段也变，人手工核对要按 §3 复现算。④无基准数；真机走查在开工那批给。 |
| 文档 | 本批登记：ROADMAP 本节 · CHANGELOG 本版行 · 队列 §4 Q7d 状态 ◻→◐（取证已落，等裁定）· 新立取证稿 · INDEX 该行 · AGENTS 轮转（v2.197 降「此前进度」，v2.196 逐字搬进归档）。 |

### v2.197（提交 `cfadccd`；⚠ 本条哈希由同批回补提交落下（照 `c052318` / `4755e8b` / `86bb4df` / `18fca76` / `2f97c55` / `3083990` / `27bab62` / `af6a77d` / `741b751` / `b97dd3c` / `c9fdcd4` 先例，不留「下一批回补」的悬空账））

| 项 | 内容 |
| --- | --- |
| 依据 | 队列 Q7c 主体 = 取证稿《Key Unification and Legacy Scope Merge v1》§4 的方案 + §5 的存量清单；**用户 2026-10-14 原话裁定**「那些记忆数据无所谓的，本来就是些测试数据，没啥用的，继续下一步吧」⇒ §6 两个决策点当场有解：合并形态取**甲**（不留兼容读分支），旧名去留从"改名 `.legacy` 保留"**降级为备份后删除**（备份仍做 = 他不在乎数据，不等于我该放弃可回退）。 |
| ⚠ 本刀第一条交付物还是取证订正 | 上一批取证稿 §2 写的是「organize 里七处喂点」，**实测 16 处**（`(...workspace_of)(...)` 全量调用点：organize 内 14 + memory.rs 2）。少算的原因是取证时 `grep | head -40` 把输出截断了——**截断的读数当成全量读数写进文档**，与历批「管道会盖退出码」同族。⇒ 这直接改了方案：逐个调用点补 `scope_key` 要改 16 处（且必然漏），所以归一**收在视图构造点那一处**。 |
| ① 键位合一落点（一处，不是十六处） | `desktop/src-tauri/src/workspace.rs::workspace_view` 的产出由"旁车原样串"改成**键位**（`.map(|ws| scope_key(Some(ws)))`）。它是 `WorkspaceResolver` 的**唯一构造点**（形状判据 C2 钉住），所以 `organize/{assemble,backfill,collapse,debug,pressure,project_history,recovery,search_titles,summary,tree_tools,trigger,task}` 与 `memory.rs` 的 16 处消费一次同形；未归属仍是 `None`（消费方 `?` 与 `unwrap_or_default` 两种写法语义不变），`Some("")` 仍归一成 `""`。 |
| ② 刻意没做的两件事（各有判据钉住） | **A 不碰围栏面**：`session_context_resolver` 里 `CallContext.workspace` 与 `fs_roots` 仍是 canonicalize 后的原样串——`wskey.rs` 头注早写明「围栏比较吃 `starts_with`，把下发串折叠成小写会让插件侧越界判定整体失配」，判据 A4/A5 判的就是"这一层没被顺手改"，反向证据是 `resolver_reads_sidecar_and_fails_closed` 那条既有断言**一字未改仍绿**。**B 不把两套目录编码并成一个**：`memory-tree/src/storage.rs::scope_dir`（整串前缀，机器投影）与 `plugin-api/src/memlayout/mod.rs::project_dir_name`（末段前缀，人读目录）各自仍是独立实现，判据 D1/D2 取**函数体**验互不转发——要合一的是喂进去的键，并函数会让人读目录全量换名（扩大风险面而不解决问题）。 |
| ③ 迁移面有两半这一条兑现成用例 | `crates/memory-tree/src/storage.rs` 新用例 `scope_dir_of_normalized_variants_is_one_directory`：尾分隔符漂移经归一必须合成同一个目录名（跨平台都成立的一档），斜杠方向漂移按 `cfg(windows)` / `cfg(not(windows))` **两支各写期望**（POSIX 上反斜杠是合法文件名字符，那里合并才是错），并留一条反面断言 `assert_ne!(scope_dir(raw), scope_dir(raw + "/"))` = "不归一就会长出第二个目录"，把磁盘上看到的缺陷形状钉成判据。桌面侧对应面 = `workspace/tests.rs::view_collides_sidecar_variants_into_one_keyside`（三种旁车写法经视图出同一个键位）。 |
| ④ 既有判据被正当改写（一处） | `workspace_view_projects_workspace_only` 的期望从字面串 `Some("F:/ws/a")` 改成按 `scope_key` 现算。这不是"为了让测试变绿"：本刀改的正是这条用例所断言的行为，处置照**历批 Q3 的同族先例**（「期望一律按 `scope_key` 现算，不许钉字面串」），且字面串仍留在下发面那条用例里当反向证据（判据 E4 专门验它没被顺手改掉）。 |
| ⑤ 存量迁移（用户授权免除的是停机窗口与 `.legacy` 保留，**不是**"保住树游标"） | `tmp/q97-migrate.mjs`。**取向改变**：原计划"备份后删除"，落笔前发现删除会让那 3 份投影的会话游标（`chaptered_until` = 434／270／144）归零 ⇒ 下次整理把整段历史重切一遍；而改名 + 精确改写 json 里的 `workspace` 字段是零成本保住它的路径。**迁移面有两半** = 目录名 + 投影 json 的 `workspace` 字段（`storage.rs:163 scan_all` 是按 `scope_dir(summary.workspace)` **重算路径**去取 mtime，只改名不改字段 ⇒ 清单 mtime 归 0，等于把游标保住了却把章节列表弄丢）。步骤：R1/R2 先拿磁盘现存目录名**反验**编码复现（不中就不许拿它决定迁移面）→ 候选面 = 由原样串编码、与归一形不同的目录（实测 2 个 = `memory-tree/F__Warehouse_TITY-94676677` 3 份投影、`memory/projects/TITY-9242b25e` 的 `tree.md` 侧）→ **R3 判「待移文件与目标目录现有文件零同名」**（有同名就不自动合，交人看；第一版写成"目标须为空目录"是错的——`tity-9242b25e` 本就合法住着 `MEMORY.md` + `facts/`）→ `--apply` 时先整树复制到 `~/.Locowrk/backups/q7c-<时间戳>/` 并逐文件 sha256 复核、写 `SHA256MANIFEST.txt`，全成才动磁盘 → 投影目录 `renameSync` 改名为归一形 + 每份 json 只精确替换 `"workspace": "<原样串>"` **一处**（替换数不为 1 直接 BLOCK）→ `tree.md` 并入归一的人读目录 → 对账 M1（`session_id`/`chaptered_until`/章节数逐项相同）、M2（字段等于归一形）、M3（`tree.md` sha 不变）、M4（旧名目录在场数 = 0）。sqlite 与已归一目录**一个字节不动**。 |
| ⑥ 收尾拆簇 = `check_length` 报本文件超限 427 行，处置照历批那句「**拆簇不抬基线**」 | `storage.rs` 的内联 `mod tests`（178 行 · 8 条用例 · 30 处 assert）**整块逐字节摘出**到 `crates/memory-tree/src/storage/tests.rs`，宿主留 `#[cfg(test)] mod tests;`。⚠ **「宿主 .rs + 同名子目录」正是第五道形状闸不管辖的那个空白档**（历批 Q8c 第⑤条实测：声明被摘掉时 cargo 不报错、闸也不红，那 8 条用例**静默不跑**）⇒ 本批补两条把它纳管：判据 **E5**（宿主声明在位 + 子文件存在 + 顶层测试属性数 ≥ 8，且 E3 的用例面跟着搬家）与变异 **M8**（摘掉声明 ⇒ 只有 E5 红，cargo 与第五道全绿）。摘出对账 = 用例名序列逐字相同、assert 计数 30→30、`cargo test -p memory-tree storage` **8 passed**。 |
| 改动面（**入库 4 件**，件数由 `tmp/q97-face.py` 现算，文档不另数一遍）：生产件 1 = `desktop/src-tauri/src/workspace.rs`（视图收口 + 语义注释）；测试件 3 = `desktop/src-tauri/src/workspace/tests.rs`（改写 1 条 + 新增 1 条）、`crates/memory-tree/src/storage.rs`（摘出内联测试 + 留声明，251 行）、**新文件** `crates/memory-tree/src/storage/tests.rs`（186 行 · 8 条用例）；形状判据 `tmp/q97-shape.mjs` 与变异驱动 `tmp/q97-mutate.py`、迁移脚本 `tmp/q97-migrate.mjs` 均不入库（台账 **T8** 同族）。零 ABI、零内核契约、零插件改动 ⇒ 未重编插件。 |
| 门禁（最后一次编辑之后全量复跑 `tmp/q97-gates.sh`，16 项动作各自单独取 rc） | 最后一次代码编辑之后全量复跑 `tmp/q97-gates.sh`（17 项动作各自单独取 rc，末行读数 = **ACTIONS_TOTAL=17 ACTIONS_FAILED=0**）：根 **498 passed（39 套件）** · 桌面 **517 passed** · 两侧 `fmt` + `clippy -- -D warnings` 全 0 · 前端三件（`pnpm build`／`lint:style`／`check-style`）全 0 · **七道结构闸全 0** = `check_length` 超限 **22 == 基线 22（零新增、没抬基线）**、`check-i18n` **zh=913 en=913** 持平、`check-commands` 85·85·85·0、`check-middleware-wiring` 10·10·0·0、形状闸 `S0…S6 全 0`（目录式模块 35 · 测试目录 17）、`check-reserved` OK（扫描 509 个 .rs · 预留标注 3 处 · 清单 5 件 · 越界 0）、`check-doc-budget` **DOC_BUDGET_FAILED=0** · 本批形状 **Q97_SHAPE_ASSERTIONS=17 FAILED=0** 且 **CALLSITES=16**；变异 **MUTATE_TOTAL=8 MUTATE_BAD=0**、逐条 `restored=True`。⚠ 门禁**两轮读数的分家**（判据自纠第 92 条）：上一轮我在 `de_fmt` 红之后主动停掉，`TaskStop` 只杀 wrapper bash，**它拉起的 cargo/node 子进程继续往同一个输出文件追加**（`>` 只在打开时截断一次），于是文件里同时留着 `ACTIONS_TOTAL=17 ACTIONS_FAILED=0`（本轮终态，前面正好 17 条 ACTION 行）与 `…FAILED=2`（旧代码态，缺前六项、从 de_test 起）——我第二次提取时两条一起打出来，几乎把旧态那条当本批结论写进提交说明。处置 = 读数前先核 **ACTION 条数 == ACTIONS_TOTAL**、门禁输出文件一律带本轮时间戳、`GATES_RC=` 这类 wrapper 行不属于工具。文档登记（纯 md，不可能改变 cargo 读数）之后再复跑一次**七道结构闸 + 本批形状判据**作为提交前确认，pre-commit 那六道按暂存面再跑一遍 = 历批「零代码改动 ⇒ 不重跑 cargo，结构闸照跑」的同一口径。 |
| 判据自纠 **22**（本批第 78–99 条；累计 **99**，起点 77 = 上一批取证） | 1.**⚠ 本批最大的面不是红出来的，是补判据时掉出来的** = B1 扫 `read_meta(...).workspace`、C1 扫 `(self|this).workspace_of)(`，两种形状都没覆盖「直接调 resolver 取 `.workspace`」这一路 ⇒ 补判据时才发现 `approval/mod.rs` 两处（③deny/⑤allow 规则查写 + 卡载荷）不经视图，且前者落到 `permission_rules.rs::file_of` = `{:08x}.json`（fnv1a64 吃**原样串**）= **第五个按 workspace 编码的磁盘落点**，本刀只合一并迁移了 B／C／D ⇒ 新增取证型判据 B2（命中文件集合必须恰好 = {视图自身, approval}，0 命中 = 形状瞎、3 命中 = 有人开旁路），第五面登记为队列 **Q7d** 不在本刀顺手改；如实写脆性 = B2 认这一种写法、换局部变量名能绕 ⇒ 下限闸不是防弹闸，故不配代码变异靶（凑假靶只会污染变异台账）。2.json 字段搜索串按**语义串**拼正则 ⇒ 命中 0（磁盘字节是 JSON 转义形，一个反斜杠在文件里是两个），apply 在改写前 fail-closed ⇒ 处置 = 一律 `JSON.stringify(key).slice(1,-1)` 取字节形，且**预检与改写共用同一个构造**。3.**单阶段搬运留过一份"目录已归一、字段未归一"的孤儿副本**（第一份复制成功、改写抛错，旧目录还在 ⇒ 同项目两份投影并存，比没动更坏）⇒ 三层处置：两阶段（A 全落 `.q7cmig`、B 才转正删旧）、**undo 栈声明在 try 之外**（在 try 里时 catch 拿到 ReferenceError = 回滚代码看不见已做动作，等于没有回滚）、undo 按 kind 分形（投影的临时件是副本 ⇒ 删正式名就够；人读层搬的是**原件本身** ⇒ 必须反向 rename，"只删不回搬"的回滚会把用户数据删掉）。4.**上一批那份「七处喂点」错在 `grep | head -40` 的截断输出**（队列 §4 同一处另写「五处」= 同一读数两个出处各错一次），实测 16 处 ⇒ 处数从此只由判据 C1 现算并打成 `CALLSITES=` 行。5.B2 第一版**逐行**匹配 ⇒ 假红（视图体里 `resolver(session_id)` 与 `.workspace` 本就跨两行）⇒ 先剥注释行再按整文件匹配；4 与 5 是「形状种类漏了」与「形状所在行漏了」的两面。其余 13 条（79 pathspec 漏根一级 · 80/81 全文子串当函数体判 · 83 R3「目标须空」误判人读层合法形状 · 86 拿原样锚在新串找 = 恒抛错的死闸 · 87 并发检查写在入列之前 ⇒ 对人读层空转 · 88 读数行 id 撞名）全文见 CHANGELOG v2.197 第⑫段；91 = 历批第 ㉕ 条换载体复发（Python 层把 JS 正则里的换行转义落成真换行 ⇒ 判据**崩而不红**，靠末行读数行在场这条必要条件兜住）；**92 = 同一份门禁输出文件里留着两轮读数**（主动 kill 只杀掉 wrapper，子进程继续追加同一文件）⇒ 读数前先核 ACTION 条数 == ACTIONS_TOTAL、输出文件带本轮时间戳；历批第 ㉔ 条的第二面；93–99 = 七条**历批同族复发与自家判据假红**（93 = 历批第 74 条，`.mjs` 里用 require ⇒ 崩而不红；94 = 历批 ㉘，文档面白名单手抄漏两份批次归档；95 = 新文件不在 `diff --name-only` 面上 ⇒ 工作树面必须取并集；96 = 计数句按字样取数，取到历史批次的「34 条」与另一句借用的数字；97 = 历批第⑧条 GBK 第九次咬人，崩在回补的**逐字节自证**上、此时替换已写盘 = 磁盘处于态②，处置含「自证循环按实测含哈希的集合驱动」；98 = F3 把回补提交面钉死成「四份文档」而本批实际六份 ⇒ 与 A1 同一个面两套口径，改判 every(isDoc) 同源；99 = 本段自己就是证据：**追加式替换的串里含着被替换的锚** ⇒ 脚本重放时锚仍命中 1 次，同一句被插了两遍，靠「删掉一份并按索引核对两份逐字相同」接回）与 `tmp/q97-selffix.txt`（未入库 = 台账 **T8**）。另**流程事实**不占条数：桌面 `cargo fmt --check` 抓出我新用例两条长 assert——我此前只跑 `cargo test` 就启动门禁；那一轮在 de_fmt 处主动停掉（格式修正后必然全量重跑，中途编辑它正读的文件更危险），改完按 mutate → gates 串跑重放。 |
| 已知边界（如实记档） | ①归一发生在**读侧视图**，旁车字节与 `CallContext` 下发串不变 ⇒ 旁车与键位永久差一个归一映射，这是设计（取证稿 §8 已写破，免得下批当 bug 修）。②**迁移保住了树游标**（改名 + 字段改写，`chaptered_until` = 434／270／144 迁移前后逐项相同 = 对账 M1），代价如实 = 目录换名 ⇒ 该三份会话下一轮整理的前缀缓存断一次（历批 §6 第 12 条口径下的既有代价，不是新增限制）。③sqlite `memories` 表已是归一形，未动。④迁移只在这台机器上按扫出的实际目录做过一次（脚本不含任何写死的目录名；换一台机器 R1/R2 反验不过就直接 BLOCK 而不是猜）。⑤**旁车里的原样串没被改写**（写侧保持不动，只有读侧视图归一）⇒ 磁盘上旁车与投影 json 的 `workspace` 字段从此是两个形态，这是 ① 的同一件事在磁盘上的样子。⑥无基准数；真机走查见下。 |
| 真机走查判读清单（交用户动手，逐项可对） | **A** 现有会话直接跑一轮整理 ⇒ 数据根里只应出现归一形 `memory-tree/f__warehouse_tity-9242b25e/`，旧名 `F__Warehouse_TITY-94676677` 已不存在且**不再被写**。**B** 记忆页会话切换器条目数不翻倍（同一项目只剩一份清单）。**C** 打开那三个会话的记忆树 ⇒ 章节仍在且游标读数不为 0（迁移前 434／270／144；若为 0 就是 M1 对账没真做到，回来找我）。**D** `memory/projects/` 只剩 `tity-9242b25e`，其下 `tree.md` 与 `MEMORY.md`、`facts/` 并存。 |
| 文档 | 本批登记：ROADMAP 本节 · CHANGELOG 本版行 · 队列 §4 Q7c 状态 ◐→✅ · 取证稿《Key Unification and Legacy Scope Merge v1》状态由「待研究」改「已兑现」（含 §2 那处"七处"的订正注）· INDEX 该行同步 · AGENTS 轮转（v2.196 降「此前进度」，v2.195 逐字搬进归档）。 |

### v2.196（提交 `b4a7629`；⚠ 本条哈希由同批回补提交落下（照 `c052318` / `4755e8b` / `86bb4df` / `18fca76` / `2f97c55` / `3083990` / `27bab62` / `af6a77d` / `741b751` / `b97dd3c` 先例，不留「下一批回补」的悬空账））

| 项 | 内容 |
| --- | --- |
| 依据 | 队列 Q7c 剩下的两面（键位三套形状合一 + 存量数据目录合并，**涉用户磁盘**）+ MR 稿 §8 的 M3 + `plugin-api/src/wskey.rs` 头部那句明写的「不动**记忆树**的 scope 编码……涉用户数据目录、要单独确认，不在本刀顺手做」+ 用户 2026-10-14「继续下一步吧」。**本批只做取证与方案，磁盘一条没动。** |
| ⚠ 第一条交付物是磁盘事实，不是代码 | 同项目的记忆**此刻正躺在两个目录里**（2026-10-14 本机 `C:/Users/42527/.Locowrk` 实测，非历史遗照）：主键侧 sqlite `memories` 的 workspace 分组 = `f:\warehouse\tity` **一组 6 行**（Q2 归一已兑现 ⇒ 零迁移红利仍在）；磁盘侧 `memory-tree/F__Warehouse_TITY-94676677/`（3 份投影 json，章节 5／7／2、游标 434／270／144，mtime 2026-10-13 仍在写）与 `memory/projects/TITY-94676677/tree.md`（19,649 B）走的是**未归一形**，而 `memory/projects/tity-9242b25e/`（`MEMORY.md` + `facts/` 6 个 md）走的是归一形。旁车侧 8 份文件全是原样串 `F:\Warehouse\TITY`，其中一份 `.jsonl` 里**两种写法并存**（某条事件参数已是归一形）⇒ 旁车不许改（追加不删除纪律），归一只能发生在"把串喂给键位的那一刻"。 |
| ① 三套形状逐个定位（代码点带证据行） | **A 主键**（宿主 `workspace::scope_key` + 插件 `WsCtx` 共用 `wskey::normalize`）= 已归一。**B 树投影目录** `memory-tree/src/storage.rs:31 scope_dir`（整串安全化取前 40 + fnv1a 高 32 位 hex8）= **未归一**。**C 人读 md 目录** `plugin-api/src/memlayout/mod.rs:63 project_dir_name`（**路径末段**安全化 + 同一 fnv 短哈希）= tree.md 侧未归一、facts 侧归一。喂点全部现读：`organize/{assemble×2,backfill,collapse,debug,task×2}.rs` 七处引用清一色 `(self.workspace_of)(…).unwrap_or_default()`（旁车原样串），同一条链上**只有** `organize/facts.rs:50` 与 `:67` 过了 `scope_key` ⇒ 同一条链上事实卡落归一目录、树投影与 tree.md 落原样目录，这就是"劈成两份"的成因。 |
| ② 复现前置判据（不许拿想象当目录名） | `tmp/q7c-hash.mjs` 先拿磁盘上**已有的三个答案**反验两套编码：`scope_dir(F:\Warehouse\TITY)` = `F__Warehouse_TITY-94676677` ✅、`project_dir_name(同串)` = `TITY-94676677` ✅、`project_dir_name(f:\warehouse\tity)` = `tity-9242b25e` ✅（`HASH_REPRO_FAILED=0`）⇒ 同一工具算出的目标名才可用作合并依据：归一后 `scope_dir` = **`f__warehouse_tity-9242b25e`**（磁盘上不存在 ⇒ 纯改名无同名冲突）、`project_dir_name` = **`tity-9242b25e`**（存在但**其下没有 tree.md** ⇒ 移入不覆盖）。顺带量到：同一项目的反斜杠／正斜杠／带尾分隔符三种写法算出**三个不同哈希**（`94676677`／`bb23643f`／`a1298b2c`）= `wskey.rs` 立项理由的磁盘版证据。 |
| ③ 合一方案的边界（本批最重要的一条判断） | 合一 = 五个喂点的输出过 `scope_key`，**不是把 `scope_dir` 与 `project_dir_name` 并成一个函数**——前者是机器投影目录（整串前缀，能看出盘与父目录），后者是人读目录（末段前缀，要好看）；并成一个会让人读目录**全量换名**，扩大风险面而不解决问题。**迁移面有两半 = 目录名 + json 里的 `workspace` 字段**：`storage.rs:163 scan_all` 的第二遍按 `scope_dir(summary.workspace)` 重算路径取 mtime ⇒ 只换目录名不改字段 = 路径与实际目录不符 = 清单 mtime 归 0、排序垫底（这一条最容易漏，写进稿 §4）。 |
| ④ 存量合并清单（取证稿 §5，六条逐条可回退） | 1 备份三处（`memory-tree/` · `memory/projects/` · `memory.sqlite3`）进 `backups/q7c-<时间戳>/` + sha 清单 → 2 `F__Warehouse_TITY-94676677/` 改名 `f__warehouse_tity-9242b25e/` → 3 三份 json 的 `workspace` 字段改写为归一形 → 4 `TITY-94676677/tree.md` 并入 `tity-9242b25e/` → 5 空壳改名 `TITY-94676677.legacy-<时间戳>` 保留 → 6 sqlite **不动**（实测已归一）。执行时机 = 停机窗口内（整理调度与副车写入都在跑，边写边换名会造出第三种状态）。**六条在用户逐条确认前一条都不动。** |
| ⑤ 两个决策点交用户（本批不收口，因为裁定没到） | **决策一 · 合并形态**：甲 = 一次性显式迁移（推荐：代码不留兼容分支、清单立刻单份、一次可回退；代价 = 停机一次 + 动用户磁盘）／乙 = 惰性兼容读（不停机；代价 = 兼容分支永久住在代码里，而 `list_scopes` 是按目录遍历的 ⇒ 旧目录在搬空前**会继续出现在记忆页会话清单里**，双份来源不消除，搬完还留空壳要二次清理）。**决策二 · 旧名去留**：删／`.legacy-<时间戳>` 保留一个版本周期（推荐，现役先例 = `memory.sqlite3.bak-20261004`）／原地不碰。 |
| ⑥ 交付物一半是判据（动工那批要照 §7 交） | 形状判据（五个喂点必须调 `scope_key`；时机型纪律只能由源码证明 = §6 第 13 条）· 行为用例（同一项目四种写法必须落进同一个 `scope_dir` 与同一个 `project_dir_name`）· **复现判据**（三条反验全中才允许脚本给出目标名）· 迁移对账（文件数守恒 · 每份 json 的 `chaptered_until` 与章节数不变 · 移入的 `tree.md` 逐字节相同 · 备份 sha 逐项一致，任一不符 = 回退而不是继续）· 变异靶（摘掉某个喂点的 `scope_key` ⇒ 形状红；迁移只改名不改字段 ⇒ 对账红；跳过备份 ⇒ 迁移拒绝执行）。 |
| 改动面（现算，文档不数第二遍） | **零代码改动、零磁盘动作**。入库 6 件 = 新设计稿 1（`docs/design/NEW/Key Unification and Legacy Scope Merge v1.md`）+ 登记 5（INDEX §三 一行 · 队列 §4 新增 **Q7c 行**（状态 ◐ 取证已落）· ROADMAP 本节 · CHANGELOG 本版行 · AGENTS 轮转）。取证工具 `tmp/q7c-{scan,hash}.mjs` 与 `tmp/q96-*.py` 不入库（台账 T8 同族）。 |
| 门禁（最后一次编辑之后全量复跑） | 七道闸全 0：`check_length` 超限 22 == 基线 22（零新增，本批零 .rs）、i18n zh=913 en=913（零键变动）、commands 85·85·85、wiring 10·10·0、形状闸 S0…S6 全 0、预留对账 OK、**第七道文档预算闸含表格完整性档 = 扫描 60 份 md · 断行 0**（新稿与两条插入行都必须闭合，这一档是上一刀刚立的常驻判据，本批第一次被新文档触发）。零代码 ⇒ 未重跑 cargo/前端门禁。 |
| 判据自纠 **5 条**（本批；累计 **77 条**，续 v2.195 线的 72 条） | **73 ⚠ 插行之后拿「按索引逐行比」当局部性判据** = `src[i] != dst[i]` 在插入点以下整体错位 ⇒ 一次完全正确的插入被读成"整片重写"（INDEX 80–84 行全红），**v2.195 第 40 条在同一条链上第三次长出来**，而且这次犯在刚把那条判据写完的脚本里；处置 = 局部性一律写成**前段全等 + 后段全等**（`dst[:at] == src[:at] and dst[at+1:] == src[at:]`），不写索引比较。**74 取证脚本写成 `.mjs` 却用 `require`** ⇒ `ReferenceError` 在第一条读数之前崩掉，"崩而不红"在本仓第五次兑现；处置 = 落盘后缀与模块制式一起看（`.mjs` = ESM 强制），跨制式一律 `import`。**75 `assert a == b if cond else True` 的三元被吞进断言表达式** ⇒ 另一支退化成 `assert True` **恒真**（第 66 条「恒真比恒假更险」的第三种形态：不是判据写错，是判据**根本没在判**）；处置 = 期望值先按键查进变量，断言里不写条件式。 **76 收口自证把「每件都能在 HEAD 里查到」写死** = 新立的稿在**主提交之前**必然不在 HEAD ⇒ `git show` 报 `fatal: path … exists on disk, but not in 'HEAD'`，于是「提交面也是 LF」这一档把一次完全正确的收口判红，而磁盘没有任何错（历批第 29 条「凡跨提交复跑不许写死任何一态」第六次兑现，且犯在同一批刚把那条纪律写进文档之后）；处置 = 提交面只查**已在 HEAD 里的件**，并把「可查 n/N 件」这个比值打出来而不是默默少查。**77 `git log --pretty=format:%h %s` 的空格未加引号** ⇒ cmd.exe 把 `%s` 当成位置参数（`fatal: ambiguous argument '%s'`），自证脚本**崩在第一条断言之前** = 「崩而不红」本仓第六次，也是命令行 quoting 家族（历批 `HEAD^` 被吃、`git status` 给含空格路径加引号而 `diff --name-only` 不加）的又一次当场兑现；处置 = 一切带空格的 git 参数整体加双引号，并一律把「末行读数行在场」当结论的必要条件（缺行 = 崩了，不是绿）。 |
| 已知边界（如实记档） | ①本批**没有任何生产改动** ⇒ 磁盘双份仍在，下一次整理仍会往 `F__Warehouse_TITY-94676677` 写（这是当前事实，不是遗漏）。②读数只覆盖本机：一个项目（TITY）· 8 份旁车 · 3 份投影 · 6 行 `memories`；迁移实现必须按**扫出来的实际目录**算，不许把本稿的目录名写进代码。③两个决策点未裁定 ⇒ Q7c 主体不开工。④跨机器／跨平台不统一（`wskey.rs` 已裁定 POSIX 不折叠大小写）。⑤无基准数；未真机走查（清单见下行）。 |
| 真机走查判读清单（交用户动手，逐项可对） | **A** 记忆页会话切换器现在的条目数（迁移前应 = 3 条会话，全部来自同一项目）。**B** `C:\Users\42527\.Locowrk\memory-tree` 目录数 = 1、`memory\projects` 目录数 = **2**（后者就是待合并的实据）。**C** 决策一／二 各选一项并回话，我才动 §5 那六条。**D** 若要我出迁移命令，先决条件 = 应用停机（否则换名窗口里会有写入）。 |
### v2.195（提交 `f428858`；⚠ 本条哈希由同批回补提交落下（照 `c052318` / `4755e8b` / `86bb4df` / `18fca76` / `2f97c55` / `3083990` / `27bab62` / `af6a77d` / `741b751` 先例，不留「下一批回补」的悬空账））

| 项 | 内容 |
| --- | --- |
| 依据 | 用户 2026-10-14 口头裁定「在进入 Q7c 之前，把文档规范优化一下——`CHANGELOG.md` / `ROADMAP.md` 已经膨胀得太大」+ AskUserQuestion 三项选定：**刀法 = B+C（搬出归档 + 分工单源 + 加闸）**、**在役窗口 = 最近 8 批**、**归档形态 = 两份单文件、倒序追加**。先例 = 批 E 对 `AGENTS.md` 做过的同一件事（当年 94% 字节是历史流水，搬出后 45 批一直在预算内）。 |
| ⚠ 立项前提先实测再动刀 | 搬出前两份主力 = 711,317 + 822,956 = **1,534,273 字节 = `docs/design/` 全部 2.99 MiB 的 49%**（`tmp/docsize.mjs` 现算）。ROADMAP 49 个叙述块 ≈ 550 KB、块外头部排期表 + 图例 = 141.7 KB 才是"进度看板"本体；CHANGELOG 134 块 ≈ 804 KB。**同批两份的 3-gram Jaccard 平均只有 13.8%**（`tmp/docsize2.mjs`）⇒ 它们**不是复制粘贴**，光做窗口化只省涨速、省不到存量，而"两份各写各的、还会再长回来"这件事必须靠 C 档（分工单源）治 ⇒ 本批 B+C 一起做而不是只做 B。 |
| ① 搬出（D2 = `tmp/doc-trim.py` → `tmp/doc-trim2.py`） | ROADMAP 搬出 **61 批 / 373.8 KB**（v2.120–v2.186）→ `archive/ROADMAP-batches.md`；CHANGELOG 搬出 **122 批 / 722.1 KB**（v2.27–v2.186，含 4 条 UI shell 文档版本行**随其所属批次同行**）→ `archive/CHANGELOG-batches.md`。逐字原样、内容未改写，四条硬自证全在写盘之前跑：行数守恒 / **行多重集守恒**（在役保留 ∪ 搬出 == 原文，一条不多不少）/ 每个搬出块在归档正文里**逐字节可见** / 不制造新的不闭合表格行。在役降到 ROADMAP 323.0 KB、CHANGELOG 82.6 KB （**v2.195 主收口时刻** 329.5／92.2 KB；登记本批叙述前 323.0／82.6 = 第 61 条。⚠ ⑥／⑬ 续批登记又加了一次正文 ⇒ 该读数跟着过期 = 同一把尺在本批第二次兑现，处置不是再补一个「终态」而是**体积类终态从此不在文档里预填**，要现值跑闸即可）。 |
| ② 第七道结构闸（D3 = `scripts/check-doc-budget.mjs`，新入库） | 判据打在**块数**上而不是字节上：一批的收口叙述靠压措辞能混过字节闸，但窗口纪律是「只许最近 `DOC_WINDOW = 8` 批住在在役文件里」——**数块数才能证明最老那一批真被搬走了**。在役档三条 = 块数 ≤ 窗口 / 指针行在场 / **指针恰好 1 条**；归档档 = 含「逐字原样」声明 + 非空。闸自带两道"闸自己也得是活的"下限：块数读数 = 0 直接判红（否则「块数 ≤ 8」退化成恒真空转，第一版就是这个形状——`new RegExp(re.source,'g')` 把 `m` 丢了），名单条目数 ≠ 实际扫描数判红。pre-commit 加第 ⑧ 段，触发面 = 暂存里含 `^docs/` 或 `AGENTS.md`。 |
| ③ 变异自证（D4 = `tmp/doc-mutate.py`） | 8 条靶全红在断言面：`MUTATE_TOTAL=8 MUTATE_BAD=0`，逐条 `restored=True`、零编译失败。M0 = **宽变异**把窗口常数 8→9（证闸整体承重），M1 在役多留一块 / M2 摘掉指针行 / M3 改归档「逐字原样」声明 / M4 破坏块标题形状 / M5 名单里的文件缺失 / M6 = **窄靶**：把闸的块正则 `m` flag 摘掉（证"下限"那一档独有——只有它红说明下限真在盯闸自己）。M7 = **窄靶**把指针行复制成两条（证「指针恰好 1 条」这一档独有 = 第 44 条换来的判据，处置上线的同一批就把靶配上）。 |
| ④ 分工单源（D5 = `tmp/doc-disc.py`，四份规范文本） | 治的是"两份各写一遍必漂"：**AGENTS「工作规则」末条**加在役窗口口径（含"每批收口时把被挤出窗口的最老一批逐字追加到归档正文最前"这句动作指令，`check-doc-budget` 就是它的闸）；**AGENTS 目录结构行**把 `check-doc-budget` 列进 scripts 清单；**ROADMAP 图例后**新增纪律段（窗口 8 批 / 第七道闸 / 本批实测体积读数 / 批 E 先例）；**INDEX** 的 ROADMAP·CHANGELOG 两行描述补窗口口径、§五 表新增两条归档条目、§七 `archive/` 行改成「已退役 / 未采纳 + 在役文档的历史流水（三份）」；**队列 §7** 加「在役文档的批次窗口维护（v2.195 立）」一条。 |
| ⑤ 窗口维护从一次性动作变成**可重放动作**（D6 第一刀） | 本批自己就是第一次实战：写完 v2.195 那两条后窗口变 9 批 ⇒ 必须重跑搬出脚本把 **v2.187** 滚进归档。第二版为此补三件：重放前**先剥掉已有指针行**（否则在役出现两条读数互相矛盾的指针，而旧闸只判「指针 ≥ 1」看不见它）；指针句的批数/区间/字节改成**归档累计现算**（「本次搬出」单列），顺手纠正 v1 把区间上界渲染成 `v.186` / `v.2` 这种**不存在的批次号**（磁盘上原本就是这两句）；搬出 0 批不再崩。新增自证⑤指针恰好 1 条、⑥重放幂等（旧归档正文逐字保留在末尾）。 |
| ⑥ 续批（Q7c 第一刀 = 把本批边界②那笔挂账当场结清） | 批 Q8c 收口时按队列 §6 第 15 条挂账的 **4 处存量断行**（`INDEX.md` 47 与 74、`Model Capability and Multimodal Phase Plan.md` 154、`Reliability Reinforcement Design.md` 755）在这一刀全部接回。工具 = `tmp/q7c-splice.py`：**引号感知切列**（代码格里的竖线不是分隔符）+ **两支分派**（补尾闭合 ／ 移动闭合符，各带自己的守恒式）+ **三条机器守恒**（竖线总数守恒 · 抹掉竖线与空白后正文逐字相同 · 结果必须闭合）+ **循环到收敛**（一轮接回会暴露同一句里的下一处），终态读数 = 全仓 **59 份 md 断行 0**。同刀把这条判据**接进第七道闸**而不是留在临时脚本里：新增 `DOC_TABLE_BREAKS` 读数行 + `MD_MIN` 地板 30 + 四个 md 目录各须命中的遍历自检 + 窄靶 M8/M9 ⇒ 文档表格完整性从此是**常驻闸**，不再依赖"下一批记得扫一遍"。另根治行尾：`.gitattributes` 钉 `*.md text eol=lf`（第 42/60 条那句「CR 被 eol 规范化掩盖在 diff 之外」从此在**源头**关掉，而不是每批手工归一）。顺手结清一处存量登记缺口：队列 §4 的 **Q8c 行状态仍写 ⬜** 而 v2.194 早已交付 ⇒ 改 ✅（一句状态读数错不改代码，但它是下批"照队列开工"的读数源）。零代码改动；入库 **9 件** = 被接回的 md 3 + `scripts/check-doc-budget.mjs` + `.gitattributes` + 登记文档 4（AGENTS 只做等字符数的条数替换 ⇒ 轮转三条读数不变）。⚠ 同一把尺在本批第二次兑现第 61 条：这次登记**又**让本批 ① 行那句「在役降到 …」过期 ⇒ 处置不是再补一个终态数，而是从此**不在文档里预填体积终态**（要现值就跑闸，它每次打印六份 KB）。 |
| 改动面（现算，文档不数第二遍） | **零代码改动**：生产件 0 · 前端件 0 · 测试面 0。入库件 = 文档 5（AGENTS / ROADMAP / CHANGELOG / INDEX / 队列）+ 新归档 2 + 既有归档追加 1（`archive/AGENTS-progress-archive.md` = 轮转搬入 v2.193）+ 新闸 1 + `pre-commit` 1 = **10 件**；不入库 = `tmp/doc-*`（台账 **T8** 同族）。件数由 `tmp/doc-verify.mjs` 现算并打印 `FACE_TOTAL=`，本句不写死第二套读数。 |
| 门禁（最后一次编辑之后全量复跑） | 七道闸全 0：`check_length` 超限 22 == 基线 22（零新增）（本批零 .rs 改动 ⇒ 与 `c35a0d5` 同读数）、i18n zh=913 en=913 持平、commands 85·85·85、wiring 10·10·0、形状闸 S0…S6 全 0、预留标注对账 OK、**新增的第七道 `DOC_BUDGET_FAILED=0`**（扫描 6 份 · 在役块数 8 · 8（ROADMAP · CHANGELOG））。未重跑 cargo / `pnpm build`：本批一行代码没动，重跑只会得到与 `093e518` 相同的读数——如实登记为边界②而不是冒充"全量绿"。 |
| 判据自纠 **34 条**（本批；累计 **72 条**，续 Q8c 线的 38 条） | **39 校验口径与写盘口径不同源** = `doc-disc` 用「全文子串」校验、用「整行替换」写盘 ⇒ 一次正确的替换把该行其余部分吃掉，BLOCK 救回磁盘。**40 局部性判据照"一文件一处修订"写** = 本批一个文件里有四处修订 ⇒ 一次完全正确的动作被判红；同族 = 插一行后用「按索引逐行比」当局部性判据，断点之后整体错位读成"整片重写"。**41 ⚠ 三段拼接公式在 new 为单行时把内容写两遍** = `head + new + tail` 里 new 本身含整行、head/tail 又各带一份 ⇒ AGENTS 那条 1,200 字符被截成三段且内容重复，而**旧局部性判据全放行**（恒真比恒假更险）；处置 = 断言 `count(new)==1` + 行增量必须等于预期。**42 ⚠ 行尾 CR 逐批累加** = HEAD 里这些文档全是 LF，工作区却被历批「检测原行尾再 join」的写法写到每行 4~5 个 CR，`core.autocrlf` + `.gitattributes` 把它**掩盖在 diff 之外**⇒ 只有 Read/渲染层看得见；处置 = 一律读入归一 LF、写出 LF。**43 damage 探针用「前 40 字包含」判重复** ⇒ 对 `>` 续行与缩进注释**恒误报**（"像不像 ≠ 是不是"同族）；本批用它扫出的 4 处"重复"实测全是合法续行。**44 重放会留下两条互相矛盾的指针**（旧指针不在任何块里 ⇒ 原样保留 + 又插一条新的），第七道闸当时只判「指针 ≥ 1」看不见 ⇒ 补自证⑤ + 闸补一条 `ptrs > 1` 判红，并在同一批给它补上窄靶 M7。**45 搬出 0 批时 `min()` 打在空列表上直接崩** = "窗口未变所以不搬"是一次完全正常的收口态被当事故（历批「凡跨提交复跑不许写死任何一态」第四次犯）；同族残骸 = 改造脚本时把旧表达式的碎片留在原地（`len(stripped) and 0 + ...` 调一个没定义的函数），跑之前 NameError 而不是判据红。**46 指针句渲染手搓第二套拼接** ⇒ 区间上界打成 `v.186` / `v.2`，而且**这份错读数已经落盘一次**；处置 = 版本号→字符串只走 `vstr()` 一处。**47 重放脚本硬编码归档 header** = 我把 header 里的 `tmp/doc-trim.py` 改成 `doc-trim2.py`（脚本自己的措辞），下一次重放就 BLOCK 在「归档头部被人改过」上——**改的是工具，红的却是产物**；处置 = 已存在归档的 header 从磁盘读（单源在产物里），硬编码只服务首次创建。**48 归档累计扫描不带 `MULTILINE`** ⇒ `^` 只认字符串开头，读数打成「1 批 v2.186–v2.186」；⚠ **同一个丢 `m` 的坑在第七道闸身上刚修完（M6 窄靶），换一个脚本第二次长出来** ⇒ 处置 = 跨行首扫描一律显式 `re.M` + 加一条下限断言（既有 61 批，数出个位数就是瞎而不是真没了）。**49 累计口径与开窗口径不同源** = 开窗那侧按项目归属分组（UI shell 行跟随所属批次），累计那侧直接数块首 ⇒ 指针打成「累计 126 批 v2.27–**v4.2**」：批数虚高 4、区间上界是项目里不存在的批次号（历批 Q8b 第⑮条「同一读数两套算法」第三次兑现）；处置 = 归集函数只写一次、两侧共用。**50 占位清零判据写成「全文不含 `{{`」** ⇒ 被历批一句**合法叙述**判红（ROADMAP 第 852 行 / CHANGELOG 第 20 行写着「真口径收窄成"只查脚本模板占位 `{{大写}}`"」，那句讲的正是怎么把这类判据写窄）= 恒假红，还是自己打自己那种；处置 = 只查本批定义的占位名，且清单由替换表现算而不是手写第二遍。 **51 插入脚本把三种前缀形状当成同一件事** = ROADMAP 块自带 `### `、CHANGELOG 行自带 `*文档版本：`、AGENTS 条目要外部加 `* ` ⇒ v2.195 条目落盘成了普通段落，而写盘时的三条自证（行数／局部性／表格闭合）**全放行**；抓到它的是**下一步轮转脚本**的窗口形状断言 ⇒ 判据覆盖面要沿下游动作反推，不是照"我改了什么"来写。 **52 复算脚本把「当前条目」写死成 `* **摘要补写落地（队列 Q8c`**（= v2.194 那批的措辞）⇒ 轮转后 BLOCK「窗口三条只取到 prev/ptr」；上一批 Q8c 第㉚条刚点过同族错（"全文前三个 `* ` 行"取到概览段），当时的处置只是**换了一种写死法** ⇒ 处置 = 三条各按结构标记取 + 断言无标签进度条目恰好 1 条（两条 = 轮转没做完，新旧当前同时在场）。 **53 归档标题的下界用「最后一条 `* ` 行的版本号」现算** = 末条是 v2.55 到 v2.9x 那段，`v2\.\d{3}` 匹配不到 ⇒ `re.search().group(0)` 当场 None 崩；最早批次本轮不可能变化 ⇒ 正解是照抄原标题的下界（**只改必须改的**，而不是"全部重算一遍看起来更严谨"）。 **54 f-string 里给正则多写一道反斜杠**（`r'v2\\.\\d{3}'`）⇒ 模式变成"匹配字面反斜杠"、恒空匹配 = 转义家族的新变体（历批是 `\n` 被写成真换行，这次反过来是 `\.` 被写成 `\\.`）；处置 = 正则不进 f-string，读数先算成变量再插进句子（与第 22 条"正文搬出脚本"同一条处置）。 **55 轮转脚本照终态写定位与切片** = 以为窗口是「1 无标签 + 1 带标签 + 1 指针」，实测收口时刻是**四条**（新当前／待降级／待搬出／指针）；同一脚本又把新指针行**加**在窗口里而没替换原指针行 ⇒ 一次 BLOCK 抓定位、一次 BLOCK 抓行数（少 1 行 vs 差 0 行）——**两条断言各挡一半**，缺一条就把 AGENTS 头部写坏。 **56 ⚠ Edit 工具吃掉紧邻下一行（本仓第三次兑现，记忆条目同名）** = 在本批 doc-fill 脚本里删一个空行，结果 `by_file = {}` 与下一行 `for path, old, new in PAIRS:` 粘成一行 = SyntaxError；暴露它的不是判据而是**下一次 Edit 报「0 occurrences」后我按纪律读回文件** ⇒ 处置 = 每次 Edit 之后读回受影响行，并把 `python -m py_compile` 加进收尾流程（与历批「每个 `.mjs` 先过 `node --check`」同一条纪律）。 **57 自证脚本在 DRY-RUN 态去读磁盘做终态判据** = 本脚本最后那段"三处计数同数"把 AGENTS / ROADMAP / CHANGELOG 从文件读回来比，而 DRY-RUN 时新文本还在内存里 ⇒ 一次**完全正确的 dry-run 恒 BLOCK**（报的是"ROADMAP 本批条数不是 19"，真相是它读到了改写前的 11 条）。历批 Q6 ⑫⑬、Q7 ⑬、Q8c ㉙ 已把「凡跨提交/跨阶段复跑不许写死任何一态」写死，本批第五次犯在**新写的脚本**里 ⇒ 处置 = 判据按阶段取源：DRY-RUN 判内存 out，--apply 之后另读磁盘复核（同一个函数两处调用而不是两套判据）。 **58 "过期读数不许在场"判据打在全文，被别的批次合法撞上** = 这条检查要抓"本批的 11/49 还留在场"，而 ROADMAP 在役窗口里**历批自己的判据自纠行**（"判据自纠 11 条"是某一批的真实条数）恒把它判红 = 一个正确收口被自己的覆盖面误伤；处置 = 过期读数只在**本批那一句**上判（per-file 先定位本批行、断言恰好 1 行、再判该行含新数且不含 stale），并由此得到一条更普布的口径：**跨批数字判据必须带批次上下文**，光数数字会把历史当残留（历批 Q8c ㉚「计数正则不认粗体」是同族，方向相反）。⚠ 这条在本脚本内第二次兑现才算想清楚：我把「判据自纠 11 条」当**例子**写进第 58 条正文，结果被自己的 stale 检查判红 ⇒ 正解 = 判据只作用在**计数句那一段**（从「判据自纠」到「续 Q8c 线的 N」），不是整行更不是全文。**59 收口自证脚本（`tmp/doc-verify.mjs`）四条判据照想象而不是照磁盘** = **(a)** 改动面白名单**漏了轮转搬入的 `archive/AGENTS-progress-archive.md`** ⇒ 一次正确的改动被判"越界"（白名单少一档在本仓第三次出现：批 Q8b 第②条调用点白名单、批 Q8c 第㉘条 DOCS 档 ⇒ 处置 = 清单按**本批动作枚举**而不是照"我以为会动的文件"写）；**(b)** "三处同数"判成**整句逐字等式** ⇒ AGENTS 那句天生少「本批；」三个字，恒红（第 34 条同族），正解 = 提 (本批条数, 累计, 基线) 三元组再比；**(c)** 重放下限写成「`→ True` 计数 ≥ 12」，而第⑤条的打印形状是「在役指针行 = 1（必须恰好 1）」**不带** `→ True` ⇒ 一次完全正确的重放被判红，正解 = 判关系与语义标记（六标签 ×2 + 无 False + 无 Traceback + 无 BLOCK）而不是判计数值（批 Q8b 第⑰条同处置）；**(d)** 这一句自己：追加段的「另 50–56 七条」是 NEW 列表还只有 7 项时**手写的字面串**，列表长到 9 项后文档说的条数与磁盘上的条数脱节 ⇒ 同一读数两处定义（我在脚本注释里写着"条数单源现算"，却把范围与条数又手写了一遍）。**60 ⚠ `core.autocrlf` 的"回写效应"让 CR=0 只是一次当下态** = 本批把九份文件归一成 LF 并在写盘后逐份复验 `CR 残留 0`，而 `git add` 时那十行 warning 说明**下次 git touch 工作区**（checkout / clone / 部分 diff）就把它们写回 CRLF ⇒ 这条判据**不是不变量**，跨批复跑 B 组会在一次完全正确的收口上假红。处置 = 行尾判据按阶段取源：写盘后判工作区字节，跨批复跑判 `git show <本批提交>:<文件>` 那一面；登记在回补批而不是留给下一批踩。 **61 同一份文档里混住两个时刻的读数而不写时刻** = 「在役降到 323.0／82.6 KB」是**搬出那一刻**的实测，本批随后又把 v2.195 的批次叙述登记回在役文件 ⇒ 收口终态更高。两个数都没错，错在句子没说是哪个时刻的，读者拿搬出值当现状（历批「报告新旧要靠 mtime 而不是记忆」同族，对象从工具输出换成文档句子）。处置 = 体积类读数一律带时刻限定语，终态那句由脚本现算并**迭代到不动点**——因为登记这条正文本身又会改变字节，预填数字就是再造一次同样的错。⚠ 同族错在本脚本里**当场再犯一次并被自己的断言抓住**：「占位清零」的断言又写成通用双花括号形状，于是被第 50 条正文里那句合法的模板占位引用判红——第 50 条讲的正是这件事（处置 = 待清占位名由 SIZE 模板现算提取，见本脚本的 PLACEHOLDERS；按批 Q8b「同族错并进原条而不抬条数」的做法登记进本条）。**62 收口自证脚本在主提交落下那一刻把自己判瞎了（「跨提交复跑必须先枚举态」第六次兑现）** = 主提交定位那句写成 `^\w+\([^)]*\):?.*v2\.195`，而 `[^)]*` 把 scope 里**唯一**那个 v2.195 消耗掉、subject 正文里没有第二个版本号 ⇒ 读数退化成 `FACE_MODE=工作树（主提交未落下）`、`FACE_TOTAL=3`，连带 A3「允许清单都在场」、I3「AGENTS 带哈希」、H4「文档件数 == 实测面」三条一起假红。四个态（写盘未提交 / 主提交已落 / 回补已写盘未提交 / 回补已提交）里它只覆盖了第一个。处置 = 定位式改成"scope 内认版本号 + 排除同批回补提交"，并补一条下限：**工作树已经没有哈希占位 ⇒ 主提交必然已落下，取不到就判红**，不许静默退回工作树面（退回分支是给上一个态留的，不能变成万能兜底 = 恒真比恒假更险）。同轮另两处并条：**(a)** I4 那条「回补提交自身的哈希不进 AGENTS」在 `HEAD == 主提交` 时**没有可判的对象**却被写成兜底表达式 ⇒ 一次正确的中间态被判红，处置 = 态不成立时如实报 n/a 而不是让判据装作有力；**(b)** 中文数词表只到「十」⇒ 追加段变成十二条时 `CN['十二']` KeyError、跨度判据把一次正确的登记判红 = **表覆盖不足也是照想象写**。 **63 分隔符算术不引号感知** = `q7c-splice` 第一版把「代码格里的竖线」和转义竖线也数成分隔符 ⇒ 列数算错，一次本来正确的接回被**自己的守恒断言**判成"动了正文"。处置 = 切列、数分隔符、定位末位分隔符三个动作共用同一个引号感知实现，不许一处感知一处裸 `count`。**64 ⚠ 反引号嵌在正文里 ⇒ 配对状态机从那一处起整体翻转** = `Reliability Reinforcement Design.md:755` 的内容本身含着半只反引号（讲代码时的那一只），从它开始"是否在代码区间内"这个状态与真相相反 ⇒ **判据在别人的文本上失去前提，而且失去的方式是看起来仍在工作**。处置 = 该行走 `--manual` 人工裁定支，但人工裁定不等于免检：三条机器守恒照验（竖线总数守恒 · 抹掉竖线与空白后正文逐字相同 · 结果必须以闭合竖线结尾），且"下一行也是断行"（一行被切成两截的形状）时**拒绝接回**交人看。**65 移动闭合符支误用补闭合支的算术** = 把「分隔符数 = 列数 + 1」这条只属于补闭合支的等式拿去验移动支 ⇒ 一次正确的移动被判红；处置 = 每一支带自己的守恒式（补闭合验列数、移动验竖线总数）。**66 行数守恒判据自比恒真** = 「写出后拆行的条数 == 拆出来的那个数组的长度」两侧同源（拼完再拆当然同数）⇒ 恒真。处置 = 与**读入快照**比。历批「恒真比恒假更险」在这把尺上第二次兑现：红的那条会被追查，不红的没人看。**67 接回被写成一次动作而不是一个循环** = 把清单当一次性快照用，而接回一处之后同一句里被切开的下一处才进入扫描视野 ⇒ 清单在写盘后就不新鲜。处置 = 仍有断行就继续接、上限 8 轮、超限 BLOCK 而不是接着猜。**68 闸的扫描面下限拍成当前实测** = `MD_MIN` 第一版写 60 而实测 59 ⇒ 一次**合法的删文档**会把正确的闸判红（历批「凡跨提交复跑不许写死任何一态」第五次犯，且犯在新写的闸身上）。处置 = 下限只当**地板**（30），另加一条逐目录命中下限：`design` / `NEW` / `archive` / `reports` 四个目录各须 ≥1，命中 0 = 遍历瞎了而不是文档没了——**覆盖面自检与读数下限是两条判据，不是一条**。**69 收口自证的白名单漏了本批自己要改的闸脚本** ⇒ `A2 无越界` 把一次正确的改动判红（v2.195 第 59(a) 条同族：白名单按本批**动作**枚举，不按记忆）。**70 ⚠ 局部性判据只排除当前循环行** = `D2 其余行逐字相同` 写的是「除 `ln-1` 之外全等」，而本批在同一文件里有两处落点 ⇒ 两处互相被判成"顺手改动"，一次完全正确的收口报 2 条红（v2.195 第 40 条**第二次**兑现，而且这次是我上一批刚立的那条）。处置 = 排除集合按「本批在该文件的**全部**落点」算，与循环变量无关。**71 取证文件的 mtime 会被变异驱动器刷新** = 还原是逐字节写回，内容一字未动而 mtime 变了 ⇒ 新立的「取证晚于最后一次写入」假红。处置 = **不放宽判据**，改取证的**顺序**（mutate → gate → gates → verify）并把顺序写进注释：这一档判据的成立条件里本来就含着顺序。**72 命令行里的 pathspec 没加引号** = `git show HEAD:docs/design/NEW/Model Capability…` 被 git 当两个路径，`fatal: path '…/Model' does not exist` 把自证脚本崩在第一条断言之前 = 「崩而不红」在本仓第四次；同族第二处 = 读数行提取写的是 `split('\n').pop()`，拿到的恒是**空串**，把一次成功的跑打印成"文件空"（第 30 条「判据照措辞不照磁盘」在打印语句上重演）。处置 = 拼进命令行的路径一律加双引号；取末行一律取末条**非空**行。 |
| 已知边界（如实记档） | ①`archive/AGENTS-progress-archive.md` 现在 **314.0 KB**（= 本批轮转搬入 v2.193 之后；搬出前 311.6）且**每批只增不减** —— 本批只把它纳入第七道闸的**存在 + 声明**视野，没给它预算口径；下一批要么给它分片要么给它字节基线。②本批**未重跑 cargo / 前端门禁**（零代码改动，见门禁行）。③在役头部排期表（ROADMAP 的 ⬜/◐/✅ 那一大张）仍占 141.7 KB，本批只动"批次叙述"这一维；它要不要按季度分片 = 未决。④`check-doc-budget` 的字节维度只对 `live-loose` 档报告不判红（队列/AGENTS 与批次块不同构），AGENTS 三条 ≤6,000 由收口脚本判而不是闸。⑤`tmp/doc-*` 工具不入库 = 台账 T8 同族。⑥**存量断行还有 4 处未接回**（`INDEX.md:47` / `INDEX.md:74` / `NEW/Model Capability and Multimodal Phase Plan.md:154` / `NEW/Reliability Reinforcement Design.md:755`）：本批的自证④只保证「搬出动作不制造新断行」，不管修旧的 ⇒ 挂 **Q7c 的第一件事**。 |
| 真机走查 | 不适用（纯文档批，无界面变化）。验收方式 = 跑 `node scripts/check-doc-budget.mjs --dbg` 看六份读数 + 跑 `tmp/doc-trim2.py` 的六条自证 + `tmp/doc-mutate.py` 的 7 条靶。 |
| 文档 | 本批即文档批：AGENTS（工作规则末条 + 目录结构行 + 轮转）· ROADMAP（纪律段 + 本节）· CHANGELOG（本节 + 指针句重写）· INDEX（两行描述 + §五 两条 + §七 一行）· 队列 §7（窗口维护一条）· 新归档两份各带首部定性注。 |

### v2.194（提交 `093e518`；⚠ 本条哈希由同批回补提交落下（照 `c052318` / `4755e8b` / `86bb4df` / `18fca76` / `2f97c55` / `3083990` / `27bab62` / `57210e1` / `af6a77d` 先例，不留「下一批回补」的悬空账）——**v2.194 自己的哈希只能由随后那次回补提交落下**，不是「下一批」 · **队列 Q8c 落地 = 规则切分章节的摘要补写（只换正文不破目录字节 ⇒ 前缀缓存不断）**）

| 项 | 内容 |
| --- | --- |
| 依据 | 队列 Q8c = 批 Q8b 收口时**自己登记的那条边界**（ROADMAP v2.193「已知边界」第①条：「机械切落的章节标题/摘要来自规则切分，补写需要一条保留 `chapter_id` 与 title、只换正文的替换通路——目录行的字节形是 `- [id]: [title]`，只换正文才不破前缀缓存，**这条边界必须先写成判据再动手**」）+ 用户 2026-10-13 裁定「等一次机械切，**摘要后台补**」的后半句。前置 = Q8b（v2.193）与 Q8（v2.192）。本批同时兑现 Q8b 边界⑤：补 `Incremental` 档到水位也切的行为用例（当时登记为「Q8c 的第一件事」）。 |
| ⚠ 第一条交付物是对稿改口 | Q8b 那句边界写的是「只换 `summary`/`keywords`/`token_count`」三样。实测**只换 `summary` 一样**：`keywords` 与 `token_count` 全仓只有写点、没有读点（`grep` 覆盖 `crates`/`desktop`/前端三面，读点为 0），记录员通路从落到今天都没填过它们 ⇒ 为「将来观测面可能要」去接一个当前无人消费的形状，正是批 Q7 抓的那类件（那次是「注释声称的使用者从未存在」）。裁定 = 不产出无人读的字段，`ChapterSummary` 只有 `chapter_id` + `summary` 两个字段，落笔也只写 `summary`；这条裁定写在 `summary.rs` 头注第 2 条并由形状判据 M2a/M2b 钉。 |
| ① 债只有一处置位点，覆盖两个来源 | `MemoryChapter` 追加 `summary_pending: bool` + `summary_attempts: u32`（都走 serde default，零迁移），置位点唯一 = `from_draft` 里 `summary_pending: source == "rule"`——轮内泄压（Q8b）与记录员降级（§5.4）都经 `build_chapters(drafts, "rule", now)` 这一条出口，所以一处判定天然覆盖两个来源（形状 M1a 判「恰好一次」，M1e 判生产面零处 `= true` 的第二写法）。⚠ 边界照实写：`from_draft` 之外没有第二处把债置真，**因此 Q8c 之前写在磁盘上的 rule 章节不追认这笔债**（键缺失 = `false`）。要让旧债也追回来，需要能区分「键缺失」与「显式 false」的反序列化哨兵（`Option<bool>` 或手写 `Deserialize`），代价是打破本仓「字段演进一律走 serde default」的零迁移纪律；处置 = 不打破，登记为边界（核验 I1 判这句话在文档里）。 |
| ② 本批最值钱的一条 = 目录字节不变（前缀缓存不断） | 请求组装的目录行是 `out.push_str(&format!("\n- {}: {}", b.chapter_id.0, b.title))`，而注入的 system 前缀按字节命中 ⇒ 补写若动 `title` 就打断缓存。落笔面结构上碰不到它（`apply_summaries` 只有一句 `chapter.summary = item.summary.clone()`，身份字段与 `keywords`/`token_count` 全在禁词表里）；端到端再钉一层：`backfill_keeps_the_injected_directory_byte_identical` 用**同一个 `TreeAssembler` 实例**比对补写前后 `plan().directory` 的字节，并断言补写前目录里确有 `- b1:` 那一行（⚠ 这条第一次跑红过：我原先把首次组装排在泄压之前，那时目录是空串，「前后相同」成了空话——判据必须打在「真有内容」的状态上）。字节不变靠两个事实叠加：目录行只含 id/title（本批）+ 项目级进展行会话内钉死（Q2 的 `ProgressPin`，它读投影 mtime，补写落盘会把 mtime 推新，靠钉死才不会把「last organize」改写）。 |
| ③ 失败落账与两个 0 档 | `summary_attempts` 跨轮生效（落在投影里），到 `organize_summary_max_attempts` 就退出候选池——不落账的后果是「模型一直给不出可用摘要 ⇒ 每轮重挑同一章重烧」，与 `organize_max_rerun` 同族语义。成功那条不涨计数（`apply_summaries` 清债即可）。两个新键 `organizeSummaryChapters`（0–20 缺省 6）与 `organizeSummaryAttempts`（0–5 缺省 2）**任一为 0 即关掉通路**，关掉时判定发生在任何 IO 之前（`backfill_summaries` 第一句返回，连会话日志都不重读）= 队列 §6 第 12 条第三次兑现；用例 `backfill_off_writes_nothing_at_all` 的形状 = 投影序列化串前后完全相同 + 空脚本夹具不被消费。 |
| ④ 一把尺三处复用，不新增并发面 | **在途锁复用** `Organizer::inflight`（补写抢不到就退出，与整理同一条链，用户「不新增并发面」口径继续兑现）；**超时/重试复用** `memory_manager_timeout_secs`/`_max_retries`（不为补写新增第三把尺，developer.rs 里没有 `summary*timeout` 这样的键）；**模型对收成一处** `manager_model_pair`（`pipeline` 里原先把「配了 memory_manager_model 才下发覆盖」写了两遍，补写是第三个调用方 ⇒ 抽单源，`manager.rs` 里同一段 `filter` 从此只有一处，形状 M6a/M6b 钉；这是批 Q7 把 `tail_tokens` 救活成唯一出处的同族处置）。**不打扰用户**：不发 `notify_done`、不记 `OrganizeRecord`（补写不是整理：不动游标、不改章节数，塞进台账会让「整理次数」虚增）——禁词判据 M5e 把本簇 `notify_done`/`push_history`/`apply_organized`/`insert_placeholder` 的调用点判成 0，变异面有对应靶。 |
| ⑤ 解析层两处**故意与 `parse_drafts` 不同**，各有一条理由 | 允许少给（补写缺那章下次再来，磁盘仍是机械摘要、什么都不丢；而记录员缺一个区间 = 目录有洞、那段历史永久不再被整理）；未知 id / 空摘要 / 重复 id **逐条跳过**而不是整批判废（补写是增益操作，模型编一个编号不该让另外五条能用的陪葬；落笔侧还有 `apply_summaries` 的 id 对账兜底）。全部被跳过时返回空表并按失败记账 ⇒ 最多烧到上限自动收手。错误面不扩（不新增 `MemoryManagerError` 变体，`Coverage` 不出现在 `parse_summaries` 里 = 形状 M4a）。提示词新立 `prompts/summarizer/default.md`：签名照 `manager_prompt` 同形但**只有一份通用版**（没测出家族差异就不为它写三张方言稿），用例把「四家族取值逐字节相同」钉住——将来要么补三份 md，要么承认它不分家族，不许默默漂。 |
| ⑥ 时机与次序是判据对象（§6 第 13 条第二次兑现） | `after_turn` 里补写排在 `plan_auto` 的 **else 支**（真有整理要跑时让位给那条链）；`run_task` 里排在 `rerun_while_marked` 之后、`inflight` 释放之前（锁在手 = 与下一轮 `after_turn` 天然互斥）。两句都是**源码次序**判据（M5a/M5b），行为等价的交换（变异 M8 用三步换把 rerun 与 backfill 互换）cargo 全绿而形状判据当场红——这就是这条口径存在的理由。0 档判定必须早于 spawn、早于任何读盘（M5c/M5d）。 |
| ⑥ 收尾期两刀拆簇 = 门禁先红的是磁盘事实，而它拆出来一个**闸空白档** | `gate_length` 连红两次（超限 23 > 基线 22），OVER 名单里的两个新面孔都是本批造出来的：`manager/tests.rs` 401 行（追加 5 条摘要解析用例）与 `organize/tests/summary_backfill.rs` 416 行（端到端 7 条）。⚠ 我第一时间的候选解里有「把基线抬到 23」这一项，是历批 Q8 那句「拆簇不抬基线」把它拉回的 ⇒ 结论写死：**基线不是可选项，除非三问自检答『是』并在提交说明里给理由**。两刀分别答『否』——第①问（同一个宿主文件既管记录员草稿解析与规则切分、又管补写解析）与第③问（改「谁欠债」牵动这一半、改「通路怎么收手」牵动那一半）⇒ 拆成 `manager/tests.rs` + `manager/tests/summaries.rs`（宿主 5 条搬出，**相对 HEAD 删除 0 行** = 搬移而不是重写，由 M10b 钉）、`summary_backfill.rs`（通路 4 条）+ `summary_debt.rs`（债 3 条），三件共享夹具（`persisted_turn` / `release_once` / `GOOD_SUMMARY_REPLY`）上收 `organize/tests/mod.rs`（形状闸 S3 的处置口径：兄弟簇互相看不见私有项）。**拆出来的发现**：`check-split-equivalence` 的目录枚举是`if (existsSync(join(d, "mod.rs"))) dirs.push(d)` ⇒ 「宿主 .rs + 同名子目录」这一档形状**根本不在五道闸的管辖面内**（本仓既有先例 `manager/facts.rs` + `facts/tests.rs` 从 v2.186 起就一直没人判过它）：把 `mod summaries;` 摘掉之后闸一道不红、cargo 也全绿，只是静默少跑 5 条用例。处置 = 补 M10（6 条）+ M11（6 条）判据与两条窄靶（M13 摘声明、M14 把一条用例**复制**进另一簇而不删原件——Rust 里两个模块各有同名 `fn` 合法 ⇒ cargo 全绿而只有 M11b 红，证「拆簇必须是搬移」这条纪律承重）；形状判据由 70 抬到 **88**。⚠ M11a 的「夹具被复制」形状**造不出靶**：本地副本会遮蔽 glob 进来的宿主件、用例当场 panic ⇒ cargo 也红 = 归属不纯，所以它只能作为静态对账存在，这条精度边界写在这里而不是假装证过。 |
| 改动面（现算，文档不数第二遍） | 代码面 25 件（由 `git status` 现算、`tmp/q8c-verify.mjs` A 组双向闭合）：memory-tree 生产 6 = `chapter.rs`（两字段 + `from_draft` 置位）· `summary.rs`（新簇：候选/落笔/计数）· `prompts/summarizer/default.md`（新提示词）· `manager.rs`（`parse_summaries` + `summarizer_prompt`）· `tree.rs`（占位支显式不欠债）· `lib.rs`（导出）；memory-tree 测试 4 = `summary/tests.rs`（新簇）· `manager/tests/summaries.rs`（新簇：摘要解析 5 条，v2.194 自 `manager/tests.rs` 拆出）· `chapter.rs`/`manager/tests.rs`/`tree/tests.rs` 内联与既有用例；desktop 生产 6 = `organize/backfill.rs`（新簇）· `organize/mod.rs`（`mod backfill` + `manager_model_pair` 单源 + `after_turn` 接线）· `organize/task.rs`（收束末尾接线 + 模型对改调单源）· `organize/pressure.rs`（边界文案改口）· `settings/developer.rs`（两键 + 两个 check 函数）· `settings/view.rs`；desktop 测试 5 = `organize/tests/summary_backfill.rs`（通路行为 4 条）· `organize/tests/summary_debt.rs`（债的归属 3 条，v2.194 拆簇）· `organize/tests/pressure.rs`（+1 条增量档）· `organize/tests/mod.rs`（mod 声明与上收的三件共享夹具）· `settings/tests/developer.rs`（同一条既有用例内扩面）；前端 4 = `types/settings.ts` · `components/settings/DeveloperSection.vue` · `locales/zh-CN/developer.ts` · `locales/en/developer.ts`。**零插件、零 ABI、零内核契约、零 `material.rs` 改动**（`tail_tokens` 单源在批 Q7 已收，本批不动）。 |
| 门禁（最后一次编辑之后全量复跑 `tmp/q8c-gates.sh`，16 项动作 rc 全 0 / `ACTIVITIES_FAILED=0` / `GATE_ALL_GREEN`） | 根侧 fmt/clippy `-D warnings` 全 0 + **497 passed（39 套件）**；桌面侧 fmt/clippy 全 0 + **516 passed**；前端三件全 0；六道闸全 0 = `check_length` **超限 22 == 基线 22**（零新增超限，`developer.rs` 加完两键仍在本批线内、`summary.rs`/`backfill.rs` 皆 <400）、i18n **zh=913 en=913**（新增四键：补写章数 + 尝试上限各带标题与 hint，双语对称由闸与变异 M11 双重钉）、commands 定义=85 · 注册=85 · 前端引用=85、wiring 监听器实现=10 · 已接线=10 · 未接线=0、形状闸 S0…S6 全 0（目录式模块与测试目录读数见 JSON）、预留标注对账 OK；本批特有形状判据 **Q8C_SHAPE_ASSERTIONS=88 FAILED=0**；变异 **14 条全红在断言面**（`MUTATE_BAD=0`、逐条 `restored=True`、零 COMPILE-FAILURE；三张面 test/shape/gate = 6/6/2）。 |
| 判据自纠 **38 条**（全是判据/取证工具自己的错，红因先怀疑判据） | 1.两条 memory-tree 用例的期望照想象写 = 排序里根本没有「试过几次」这一档，而我以为有：`summary_candidates(9, 3)` 我期望 `[b3, b2]`（以为试过的排后），实测 `[b2, b3]`——计数只决定**在不在池里**，不参与排序；同一次里 `mark_summary_attempts` 三条打点我数成 4（实际 3）。两次都是**磁盘行为正确、我的期望错**，处置 = 改期望并把「次数闸不改排序」写进注释。（历批「测试期望照措辞、行为照代码」同族） 2.宿主用例里把章节编号写死成 `b1` = 生产通路 `pipeline` 先插占位（占位吃掉一个编号）再落真章节，所以记录员/降级支的产出是 `b2` 起步 ⇒ `tree.chapters["b1"]` 直接 panic（no entry found for key）。处置 = 改成「取第一条非占位章节」再按它的 id 拼脚本回复，编号规则将来再变也不会假红；泄压支不占号所以那边 `b1` 仍然对——**两个来源的编号起点不同**这件事，是这条红教出来的。 3.反证用了错的口径 = 增量档那条用例（Q8b 边界⑤的欠账）我先想「同一份输入在批量档不该命中」，于是拿 `org.trigger_input` 产出的 `TriggerInput` 去判——而它的 `pacing` 就来自本用例那份 incremental 设置，等于拿增量档的输入证批量档不命中，什么都没证。处置 = 手改 `inp.pacing` 做正反两判（Batch 不命中 + Incremental 命中），把「这一档独有」钉住。 4.我给自己造了一个零价值夹具：测试簇里写了 `fn dev_settings(root, json) { settings_with(json, root) }`——只是把既有夹具的参数顺序换了个方向。队列 Q7 刚因为「零调用还带假出处的转发别名」删掉过 `count_user_messages`，本批手写工具时又长出一个同族件。处置 = 删包装、六处调用点改直调既有夹具（参数顺序交换用脚本做，逐处断言靶串唯一）。 5.变异驱动的靶单自检把 M8 BLOCK 在跑之前 = 两条编辑（A→B、B→A）的第二条在中间态里出现 2 次 ⇒「靶串要恰好 1 次」这条纪律拦下了一次**其实没换位**的空转。处置 = 改成三步交换（先占位、再落第一位、再把占位落成第二位），中间态不落盘所以不影响编译。⚠ 这条 BLOCK 本身就是防线在起作用，处置不是放宽自检。 6.取证命令的过滤串打空，把两条**正确的靶冤枉成无效靶** = M5/M6 首跑都报 `期望红 -> 未红 [判据不承重]`，真因既不在靶也不在代码，而在驱动器那句 `cargo test -p memory-tree --lib summary`：目标用例叫 `parse_summaries_*`（summar**ies**），子串 "summary" 一条都选不上 ⇒ cargo 打印 `running 0 tests` 却 rc=0 = **假绿**。处置 = memory-tree 面改跑全量（67 条，秒级），并给驱动器加一条下限自检：每条命令必须从输出里数出 `running N tests` 且 N ≥ 声明下限，达不到直接判红而不是判绿。批 Q8b 第⑭条（过滤串用子串导致两张面重叠）的同族，但这次方向更险——不是重叠，是空跑；而且它伪装成的结论是「我的判据不承重」，会把人推向「去改代码或删靶」，而磁盘上的代码与判据都是对的。⚠ 修它的时候我又犯了一次同族错（登记进本条而不另计条数）：给驱动器补下限自检时顺手留了一条 `needle 必须出现在输出里` 的检查，而 needle 用的是 `Compiling memory-tree v0.1.0` 那行——增量编译后那一行根本不再打印 ⇒ 基线被自己的新自检判成不绿、整轮 BLOCK 在跑之前。处置 = 去掉这条**基于措辞**的判据，「有没有选上目标用例」由 `running N tests` 的下限独家负责（历批「判据照措辞不照磁盘形状」第五次兑现，对象是工具自己的输出）。 7.靶串含换行的形状在 CRLF 工作区里必然不匹配 = 开机自检那条「锚必须单行」把一条 `old` 带 `\n` 的靶在跑之前 BLOCK（历批 M7 那条教训在这里第二次兑现：同仓 `runner/mod.rs` 是 LF 而 `.vue`/多数 `.rs` 是 CRLF，锚按 `\n` 写就是 0 次命中）。处置 = 全部靶串改单行，需要跨行的语义用多次单行编辑表达。 8.形状判据把 rustfmt 的折行当成代码改写 = `M3c` 判整句 `a.created_at.cmp(&b.created_at)`，而磁盘上 fmt 后是`a.created_at\n    .cmp(&b.created_at)` ⇒ 一次正确的实现被判红。处置 = 拆成三个片段各判（链式调用的**存在**而不是它的排版）。 9.三条期望常数照记忆写 = `parse_summaries` 在 `lib.rs` 里我以为出现两次（其实只有导出行一次）、Vue 里 `form.<key>` 我以为四角各一次（初值行 `key: 6,` 不带 `form.` 前缀，实测三次）、校验函数名我拼成 `check_organize_summary_chapters`（磁盘上叫 `check_summary_chapters`，去掉了 organize_ 前缀）⇒ 三条恒假判据。处置 = 一律改成磁盘实测值并写进 detail；`refs === 3` 这类数字若将来再加一角会红，那时按新增的**角色**补判而不是把数抬走。 10.禁词判据没剥注释，被我自己写的头注骗了一次 = `M9a` 全文反查「本批没做」，而 `pressure.rs` 的头注里**如实引用着**当年那句「后半句『摘要后台补』本批没做」（改口时把它留在引号里做史料）⇒ 一次正确的改口被判红。处置 = 改成正面断言（头注必须指向`organize/backfill.rs`、旧的悬空账串 `补写见 Q8c` 必须不在），批 Q7 第⑤条第四次兑现。 11.两次 `Edit` 因锚照措辞而不是照磁盘而 0 命中 = 前端 en hint 那行我照想象中的英文原文写 `old_string`（连错两次，第二次仍错），以及形状脚本里那行带 `\n` 的正则串在 Edit 里被当成真换行。处置 = 先 Read/grep 取磁盘字节再改，正则串一律走「按行内容定位 + 整行替换」的脚本，不靠记忆复述。 12.取证工具自己踩了两次转义坑 = `node -e` 内联串里 `\\(` 被 shell 与 JS 双层吞掉 ⇒ 正则未配对直接崩（崩而不红）；`python - <<EOF` 里我把 `\n` 写进普通串 ⇒ 变真换行，替换 0 命中（记忆条目「Heredoc escape trap」第九次）。处置 = 诊断与改写一律落成 `.mjs` / `.py` 文件再执行，并按历批纪律用行索引而不是正则改含反斜杠的行。 13.注释指向不存在的测试 = 在改 `developer.rs` 时读到上一批留下的「由测试 `release_count_zero_keeps_the_pre_batch_shape` 钉」，全仓 grep 该名字 **0 个用例**（真名 `release_cap_zero_writes_nothing_at_all`）。这不是本批写的，但它与本批正要动的同一个结构体相邻 ⇒ 顺手订正并把教训写进注释：指向不存在的测试比没有注释更坏，它让下一个人以为判据在位。 14.门禁红的第一反应把「抬基线」当候选解 = `gate_length` 报超限 23 > 基线 22，OVER 名单里两个新面孔都是本批造出来的（`manager/tests.rs` 401 行 = 追加 5 条摘要解析用例；`organize/tests/summary_backfill.rs` 416 行 = 端到端 7 条）。我先把「22→23」写进候选，是历批 Q8 那句「拆簇不抬基线」把它拉回的 ⇒ 把结论写死在这里：**基线不是可选项，除非三问自检答『是』并在提交说明里给理由**。两刀分别答『否』——第①问（同一个宿主文件既管记录员草稿解析与规则切分、又管补写解析）与第③问（改「谁欠债」牵动这一半、改「通路怎么收手」牵动那一半）⇒ 拆成 `manager/tests.rs` + `manager/tests/summaries.rs`、`summary_backfill.rs` + `summary_debt.rs`，三件共享夹具上收 `organize/tests/mod.rs`（形状闸 S3 的处置口径）。宿主相对 HEAD = 插入若干行、**删除 0 行**（M10b 钉住，证明拆簇是搬移而不是重写）。 15.拆簇工具照「函数名」这个词的形状去切 ⇒ 分派集合零命中、整轮在写盘前 BLOCK = `tmp/q8c-split2.py` 用 `l.split(" ")[2].rstrip("(")` 取 `async fn name() {` 的名字，磁盘上的形状是 `name()`，`rstrip("(")` 去不掉右括号 ⇒ 三条债用例一条也没匹配上，断言 `(0, 7)` 当场崩（三份文件一个没动，也没留半成品）。处置 = `.split("(")[0]`；教训还是历批那句「判据照磁盘形状而不是照措辞」，这次的对象是**我自己写的切片工具**而不是断言。 16.形状闸对「宿主 .rs + 同名子目录」这一档**完全不设防**（本批拆簇拆出来的闸空白面）：`check-split-equivalence` 的目录枚举是 `if (existsSync(join(d, "mod.rs"))) dirs.push(d)`，于是 `manager/tests.rs` + `manager/tests/summaries.rs` 这种形状（本仓既有先例 `manager/facts.rs` + `facts/tests.rs` 同形，从 v2.186 起一直没人判过）不在 S1…S6 任何一档管辖内——把 `mod summaries;` 摘掉之后**五道结构闸一道不红、cargo 也全绿**（只是静默少跑 5 条用例）。处置 = 补 M10 组六条判据 + 窄靶 M13（FACE=shape、cargo=True、pnpm=True、红在 M10a）。它与批 Q8b 第⑬条互为镜像：那条说「靶要落在闸真正管辖的面」，这条说**判据覆盖面必须按闸的枚举口径核对**，而不是按「目录里有文件」想当然——我写 M8 那组「测试面只增不减」时以为测试簇都在管辖内，就是这次红出来的。 17.改判据跟着拆簇走时有两处必须同批改，我在第三处又犯了历批第⑨条的镜像 = M8a 原判「一簇 ≥7」若不同改 ⇒ 恒假判据、`cargo_bf` 的 `running N tests` 下限 7 若不同改 ⇒ 过滤串数量不足判红；这两处同批改了（各写下限 4 / 3），但用 Edit 替换 `cargo_bf` 那段时把**紧邻其后的 `def cargo_press()` 一起吞掉**，而 `CARGO` 里对它的引用还在 ⇒ 驱动器会 NameError 崩（历批「删判据必须连带删它的引用」+ 记忆条目「Edit swallows newline」在本仓第三次兑现）。靠「改完立刻 grep 定义↔引用是否成对」抓到，没让它跑到红。⚠ 这条纪律的形态：凡是删/挪一段定义，必须把引用它的名字全量 grep 一遍，而不是看 diff 顺眼。 18.驱动器自己被 GBK 崩在**最后一次 print** 上 = 证据缺一整行读数（历批 Q8b 第⑧条同族，位置从「读子进程」换到「写自己的报告」）：新增那条靶的 expect 串里我写了 `↔`（GBK 编不出）⇒ `UnicodeEncodeError` 在第 13 条的结果行抛出，`finally` 里的逐字节还原照常执行（事后 `restored=True` + `git diff --numstat` 复核磁盘干净），但 `MUTATE_TOTAL/BAD` 末行一行没打出来 ⇒ **一份前 12 条全绿的报告缺一整行读数**，比整轮崩掉更险（崩了没人当结论，缺行很容易被读成「13 条都在」）。处置三条：①`sys.stdout.reconfigure(encoding="utf-8", errors="replace")`；②ASCII 台账行的每个字段过 `ascii_safe()`（历批「活数字只从 ASCII 行取」，这次补的是「这一行本身也必须编得出」）；③**全量复跑**而不是只补跑崩的那一条（历批第⑧条同口径）。 19.**幂等锚照批次号而不是照落点串** = doc2 那条「INDEX 里出现 `Q8c` 就判重复运行」被历批行「下一刀 = Q8c」触发 ⇒ 一次正确的首次运行 BLOCK 在写盘之前。处置 = 幂等锚必须是**本批将要写入的那句**（`已随 v2.194 兑现`），批次号不是落点。20.**轮转脚本三条自检全照想象中的文件形状写** = ①`BASE.startswith("## ")`（磁盘上 AGENTS.md 首行是 `# [AGENTS.md](…)`）、②降级只剥 `* `（该剥 `* **`，否则拼出 `**此前进度**：****轮内…` 的四星）、③搬出读数那句正则漏了粗体闭合的 `**` ⇒ 0 命中。⚠ 这三条只要有一条没挡住，破坏的都是归档头部那句「逐字原样搬出，内容未改写」——而它是归档可信度的全部依据（历批「判据照磁盘形状而不是照措辞」第七次兑现，对象仍是自己的工具）。21.**压字节时删了一个 `%s` 却没删它的参数** ⇒ `TypeError: not all arguments converted`；与历批「删判据必须连带删它的引用」同族，这次发生在格式化占位上。处置 = 占位数与参数数在脚本里显式判，条目正文搬进 `tmp/q8c-entry.txt`（活文本单源，压字节只动文本不动逻辑）。22.**条目正文写死在轮转脚本里，迭代压缩时要改 28 行** ⇒ 一次三层转义（`\"` 在 Python 源里提前闭合字符串）就把脚本写坏；改成从 `.txt` 读之后，预算迭代不再碰逻辑。23.⚠ **本批收口链最险的一条：把「后台任务通知 completed / exit code 0」当成工具跑完**，在一条读数都没读到的情况下就写出「13 条全红 / 16 项全 rc=0」= **编造验证结果**。真相 = 那条通知对应的是上一条命令的重复投递，而变异驱动当时仍在跑（stdout 块缓冲 ⇒ 输出文件 0 字节）、门禁报告还是拆簇前那份。处置 = ①收口结论**只能**从输出文件里的读数行取（`MUTATE_TOTAL/BAD`、`ACTIVITIES_FAILED`、`GATE_ALL_GREEN`）；②wrapper shell 的 rc 不属于工具（重定向 + `echo` 恒 0，与历批「管道会盖退出码」同族）；③等外部进程用**检查命令**（`tasklist` 找 PID 直到消失）而不是猜时间。24.**两份报告的新旧要靠 mtime 而不是靠记忆** = 我中途一次 grep 拿到的是上一轮 gates.out（超限 23 > 22、形状 76 条），差点把拆簇前的读数当成本批结论——本批真实收口态是 22 == 22 与 88 条；处置 = 读数与 `ls -l --time-style` 的 mtime 同批看。25.⚠ **文档脚本里写单反斜杠 `\n` ⇒ 落盘成真换行 ⇒ 把 ROADMAP 的表格行从中间切断**（ SELF_FIX 第 11 条原文要写「那行带 `\n` 的正则串」，那个 `\n` 在 Python 源里就是换行）。**没有任何一道闸会红**：`check_length` 只数行、i18n / commands / wiring / 形状闸 / 预留闸都不看文档，暴露它的是我这次「判据自纠那一行必须以 ` |` 结尾」的断言（本批唯一一次由判据抓到文档结构损坏，而那条断言本来是为别的判据写的顺手加的）。处置 = ①按字面把断开的两行接回去（还原原意而不是删内容）；②脚本源里凡要落 `\n` 字面一律写 `\\n`；③新增一条落盘后自检：表格每一行必须以 ` |` 结尾且行数与插入前一致（`tmp/q8c-splice.py` 的 `merged.endswith` 断言就是它的雏形）。记忆条目「Heredoc escape trap」第十一次兑现——前几次都栽在 shell/heredoc，这次栽在**写成文件的 Python 源**里，说明这条纪律的对象是「任何要落盘的文本」而不只是命令行。 26.**`HEAD^` 在 `execSync` 走 cmd.exe 时被当转义字符吃掉** ⇒ 提交面恒空、A2/A3 连带双红（历批「管道会盖退出码」同族，这次盖的是 shell 方言）。⚠ 这条错在历批脚本里就存在（`git rev-parse --short HEAD^`），本批第一次真跑到它才暴露 ⇒ 处置 = 全部改 `HEAD~1`，并把「凡跨提交取面」的命令列进收口必查项。27.**git 的 status 面给含空格的路径加引号，而 `diff --name-only` 不加** ⇒ 同一个「面」在两个来源里形状不同：A1 把 `"docs/design/Post-Audit Roadmap v1.md"` 判成越界件、F11 判工作树非空。处置 = 取面一律 `unq()` 去引号（历批「测量工具自己废判据」第八次的同族，这次废的是改动面判据）。28.**DOCS 档少一件** = 本批新立的《In-turn Pressure Release Design》没进核验的文档清单 ⇒ A1 假越界 + A3 代码面多算一件（26 而 25）。批 Q8b 第②条「白名单少一档 = 判据看不见它本该认识的那个档」在同一条链上第二次兑现，而且第二次仍发生在**收口链**而不是代码面。29.⚠ **三处把「某一态」写死，一次完全正确的回补被判 5 条红** = A 组没有「回补已写盘、尚未提交」这一支（HEAD 仍是主提交时提交面被取成空集）、F4/F4b 从 HEAD 取 AGENTS 而那一刻 HEAD 里当然还是占位串、F11 判「工作树必须只剩 tmp/」而回补的 4 份文档正当未提交态。历批 Q6 的 ⑫⑬ 与 Q7 第⑬条早就把这条口径写死了：**凡跨提交复跑的判据不许写死任何一态**——本批是在**自己新写的脚本**里第三次犯，说明兑现方式不能靠「记得」，只能靠「写判据前先枚举态」。处置 = 三处补第三支（占位实存数决定看工作树还是看 HEAD），并把态的枚举写进 A 组注释而不是只写在文档里。30.**四条判据照措辞而不是照磁盘形状** = ①窗口三条用「全文前三个 `* ` 行」⇒ 取到 AGENTS 概览段的 `* **项目名…**`（F3/F4 双红）；②计数正则 `/判据自纠 (\d+) 条/` 不认 ROADMAP 里的粗体夹字（`判据自纠 **25 条**`）⇒ G1 报 [0,8,2]、G2 报 [null,25,25]；③G7 从 CHANGELOG 全文第一行开始数 `v2.` ⇒ 把头部说明当正文（历批 Q7 第⑩条同族，本批第二次犯）；④B4 判「enum 之后不许出现 Summary 字样」⇒ 恒假（`pub struct RawSummary` 就合法地住在 enum 之后），改判**枚举体与基线哈希逐字相同**（基线取 `af6a77d` 这个具体历史提交，而不是 `HEAD~1` —— 后者在回补态指的是本批主提交，又是写死一态）。 31.⚠ **形状判据里把「主提交尚未落下」这一态写死了** = M10b 用 `git diff --numstat HEAD -- 宿主` 判「拆簇对宿主零删除」，而主提交落下之后工作树对该文件**没有 diff** ⇒ numstat 取到空串、判据当场红；M10c 用 `git show HEAD:宿主` 取「拆前基线」，同一时刻 HEAD 已经**包含拆后文本** ⇒ 那条会从「对账」退化成自我循环（恒真比恒假更险：红的那条会被追查，不红的没人看）。历批第⑫条口径在这里第三次兑现，而且这次的载体是本批**新写**的 shapes 脚本——说明「先枚举态」必须发生在写判据之前而不是收口之后。处置 = 两条一律改绑**固定参照提交** `af6a77d`（本批主提交的父），三种态（回补前 / 回补后 / 下一批）复跑读数相同；并给 `tmp/q8c-backfill.mjs` 加 `--recount` 模式（占位已换成真哈希之后再动 AGENTS 文本时，重算指针条自指字节到不动点，而不是把「必须还有占位」写死成唯一路径）。 32.**改动面与核验脚本把「本批的态」按「HEAD 是谁」枚举** = face 的 S0 只认「HEAD = 主提交」或「HEAD = 回补提交」，verify 的提交面按「HEAD~2 到 HEAD」写死；而本批在回补之后**还有一刀结构修复**（接回队列文档两处存量断行）⇒ 那一刀落下后 S0 直接 BLOCK、verify 的 committed 退化成空集（A2/A3 会假红）。红因照例先怀疑判据：磁盘上 29 件改动一件没少。处置 = 态不靠 HEAD 是谁，靠 Conventional Commits 的 **type 前缀 + 版本号**定位主提交（feat / fix / refactor 三选一且 subject 含 v2.194），面 = 主提交的父到 HEAD ∪ 工作树⇒ 一次覆盖任意多个续提交。批 Q3b F14「不许写死任何一态」第四次兑现，这次的人质是**回补之后的续提交**（历批枚举到「回补完成态」就停了）。 33.**接回脚本的自证判据拿自己跟自己比** = `tmp/q8c-splice3.py` 第一版写盘之后拿「磁盘重读」与「写盘前刚改过的那份内存列表」比，两者当然逐字相同 ⇒ 「变动行号 == 目标行号」恒成立为 0 行，assert 反倒把一次**正确**的接回崩掉（RC=1，而文件已经写对了）。恒等式判据比恒假更隐蔽：它永远通过，所以从没证过任何事。处置 = 自证一律与**输入快照**比；并补 `--from-head` 幂等重放（`git show` 给 LF 而工作区是 CRLF ⇒ 先按工作区风格归一再比，实测重放产物与工作区逐字节相同）。历批 H1「空转门禁」同族，对象换成脚本自己的自证段。 34.**「每一处变动都要可解释」做成逐字等式，第一次跑就三条假红** = 历批回补在**同一行里干三件事**（占位换成哈希 + 改「判据自纠 N 条」的活数字 + 改指针条的自指字节数），拿「旧行把占位替换成哈希 等于 新行」判 ⇒ 一次正确的回补被判红。 35.退一步做的**掩码形状配对**（占位与哈希归一成一个符号、数字归一成一个井号）仍四条假红 = ① 续条登记会把同一行**加长**（本批一次追加六条），掩码管不住长度变化；② 接回是「旧两行变成新一行」，新旧根本不同形 ⇒ 两侧全判成无配对。结论写死：文档面的可解释性只能按**「本批允许被动的那几行」锚清单**判（行首串照磁盘取证取值，取值来自 `tmp/q8c-anchors.mjs`），不能按行内容等式判；数量配额只作兜底。 36.**F9e 把「像接回」当成「是接回」** = 第一版拿「新侧变动行以空格加竖线结尾」当接回标志，而文档里本来就大量表格行以竖线闭合 ⇒ 三条假红（批 Q7 第③条「像不像 不等于 是不是那一种形态」第三次兑现）。 37.改成等式之后又红一次 = 「新行 等于 旧断行本体加闭合」只覆盖**一种**接回形状。磁盘上有两种：本批队列文档两处只丢了行尾闭合、正文完整，而回补期 ROADMAP 那处是整条后半截掉到下一行；且 ROADMAP 那处**与续条登记同行发生** ⇒ 没有任何形状等式能解释它。处置 = 两形取并集、ROADMAP 的纯接回数如实填 0（它的存在由「锚行豁免 + 行数与 numstat 自洽 + 工作区已无断行」三条共同承重）。**等式判据只许用在「这一次动作只干了一件事」的对象上**，同一对象上叠了两个动作就得换判据维度，而不是把等式放宽。 38.**登记脚本自己的锚又一次照措辞而不是照磁盘** = CHANGELOG 那处我照记忆把插入锚写成「⑨ 文档」，实测磁盘上 ⑨ 是边界、⑩ 才是文档 ⇒ 命中 0 次、BLOCK 在写盘之前（四份文档一个字节没改）。批 Q5 第⑨条「判据必须照磁盘形状而不是照措辞」在本批正文里被引用过多次，这次犯在它的使用者身上：本批明明已经有探针脚本（`tmp/q8c-selffix5-probe.mjs` / `probe2.mjs`），我却没先探 CHANGELOG 的段落标记就动笔。处置 = 锚一律从探针输出里抄（probe2 已加打带圈数字的逐个位置），不从记忆里取。 另记一条**存量发现**（不是判据的错，不计入条数）= F9d「不许存在以竖线开头却不以竖线结尾的表格行」第一次跑就抓到队列文档第 176、184 行两处**历史断行**（分别由 Q5·v2.189 与 Q2·v2.185 写入，非本批造成）⇒ 证明第 25 条那个坑在判据出现之前已经犯过两次。口径立进队列 §6 **第 15 条**：新判据上线的当批就把覆盖面**全量**跑一遍，抓到的存量当场接回或挂账，不许无声略过。 |
| 已知边界（如实记档） | ①**旧投影的 rule 章节不追认债**（见 ① 的理由与代价）。②**补写不发通知**：用户在界面上看到的仍是「摘要从简」，过了若干轮才变成真摘要——这是刻意的（不打扰），但意味着「泄压完立刻去记忆页看摘要」仍然会看到机械说明。③**每轮 `after_turn` 多一次投影读**（无债时早退，成本 = 一次本地 JSON 读，与 `auto_collapse` 每轮那次同数量级）。④**磁盘日志最多落后内存半轮**：补写材料走 `session_of` 重读，章节区间若压在末尾那半轮上会取到较短一段（与批 Q6 的补跑同一诚实边界）。⑤**补写不重排章节**：模型若试图改写标题或重新切分，`parse_summaries` 只认 id + summary，其它字段无从落笔——这是结构性的，不是提示词恳求。⑥**无基准数**（收益 = 目录里机械摘要的比例下降，没有可测的对照数字）。⑦未做真机走查（清单见下）。⑧`tmp/q8c-*.{mjs,py,out}` 不入库 = 台账 **T8** 同族。 |
| 真机走查判读清单（交用户动手，逐项可对） | **A** 长会话跑到水位之后出现泄压通知 ⇒ 等它跑完一两轮，去记忆页树面板看那一章的摘要：应已从「（规则切分）本章节涵盖事件 …」变成一段真摘要，而**标题不变**。**B** 同一会话继续聊：不应因为补写而重新回答或重复内容（补写不发通知、不动游标）。**C** 设置 → 开发者 → 摘要补写每次章数 调 0：机械摘要就留着，且不应有任何额外请求（关掉即回到改前）。**D** 把单章尝试上限调 0：同 C。调 1 且断网/模型不可用跑几轮：同一章不该每轮重烧（观测面板台账条数不变）。**E** 前缀缓存：补写发生后的下一次请求，用量浮层的 cache 命中不应明显掉（掉 = 目录字节被动过，本批判据的现场版）。**F** `python tmp/q8c-mutate.py` 末行 `MUTATE_TOTAL=14 MUTATE_BAD=0` 且逐条 `restored=True`。 |
| 文档 | 队列 §4 **新增 Q8c 行并转 ✅** + §6 口径增补（第 12 条第三次兑现、第 13 条第二次兑现）· INDEX 的 Q8c 落点补记 · 《In-turn Pressure Release Design》§5 那条「本批不做的第二件事」就地标注**已随 v2.194 兑现** · ROADMAP v2.194 节 · CHANGELOG v2.194 · AGENTS「当前进度」完整轮转（v2.193 降「此前」并就地补「已随 v2.194 兑现」，v2.192 条目逐字搬进归档正文最前）。**下一刀 = Q7c**（键位三套合一 + 存量数据目录合并，涉用户磁盘：必须先时间戳备份、逐条列冲突、可回退）；其后的 Q9（记忆学习面）与 Q8b 遗留的发丝线噪声、TOC 跳号标注一并排。 |

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