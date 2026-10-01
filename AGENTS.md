# AGENTS.md

## 项目概览

- **项目名 / 仓库名 / 文件夹名**：`Locowrk`（用户指定，三者一致）
- **定位**：可热插拔、可回退、带桌面端（Tauri 2）的 Rust Agent 内核。"一切皆插件"——模型适配器、工具、会话存储、沙箱、Agent 循环本身都是插件，无特权模块
- **权威设计文档**：[docs/design/DESIGN.md](docs/design/DESIGN.md)。修改任何核心模块前先读对应章节，不要凭猜测实现；实现与文档冲突时先对齐文档。文档 §8 中的 `agent-kernel/` 指内核 Cargo 工作区，其 crate 命名（`plugin-api`、`cordis-core` 等）沿用文档，对外项目名是 Locowrk
- **当前进度**：**㉔·fix 设置页改分类板面切换已完成（v2.41）**：用户裁定设置页不许是长滚动页+左栏名义跳转，要拆到每个分类每个板面——UI shell 文档先改 v3.4（§3.32 板面切换模型：右栏一次只渲染当前分区组件、无滚动侦测/无锚点闪烁、⌘K 命中=直接切板面、当前分区放壳 store 重开保持；§3.32 三/七与 §3.15 随改）；SettingsPage 重写单板面渲染（shell.settingsSection 驱动 + Record 映射随迁，SettingsPanel 堆叠容器删除）；shell store settingsFocus{section,nonce} 载荷退役 → openSettings(section?) 单一入口（⌘K 与轨内点击共用，同分区连跳幂等）；lw-settings-flash 键帧删除。实机走查（tauri dev 截图）：轨点击切板面+页头换题+高亮迁移、⌘K「嵌入」直达设置分区板面、轨搜索「终端」命中切板面、返回应用状态完好（⌘K「记忆」命中记忆页面属页面组行为正确）。验证：vue-tsc+vite 构建过、check_length OK。此前：**㉔ 设置导航轨 + ⌘K 命令面板已完成（v2.40，UI 正式设计③）**：`lib/settings-index.ts` 数据驱动扁平索引唯一真源（11 实装分区 → 4 组 偏好/智能体/能力与安全/系统，`{id, group, labelKey, icon, searchKeys}`；导航轨/⌘K/SettingsPanel 三处同吃一份，Record 类型把"分区↔面板一一对应"变构建期保证；「对话」「关于与更新」未实装不发明占位随批次补）；设置导航轨 SettingsNav.vue（275px sidenav、顶部 46px 带 = 拖拽区 + 圆角全搜索框、项 28px 圆角 8 选中 wash-3、搜索命中分区名+行标题（索引驱动不爬 DOM）组标签收空隐藏、底部「‹ 返回应用」）+ 设置整页 SettingsPage.vue（56px 页头当前分区标题 + 760 正文列；滚动侦测驱动当前分区；SettingsPanel 按索引渲染全分区堆叠 + data-settings-section 锚；DestinationShell back prop 退役）+ ⌘K CommandPalette.vue（全局 Ctrl/⌘+K 开关 Esc/↑↓/Enter/mouseenter；居中 640px 弹层 52px 搜索行 Esc 键帽；四组 会话（relativeTime hint）/页面/设置（「设置 / 分区」hint）/命令（新会话、折叠展开侧栏）；过滤空态）+ ⌘K→设置跳转（shell store `settingsFocus{section,nonce}`：切目的地+滚到分区+`lw-settings-flash` 锚点一次性 wash-3 渐隐可重触发；watcher + onMounted 两路接）。实机走查（tauri dev 截图）：整页接管/轨点击滚动+页头换标题+侦测高亮/⌘K 四组/空态/命中跳转/轨搜索「终端」→命令终端/返回应用对话状态完好。验证：vue-tsc+vite 构建过、check_length OK（纯前端批次无 Rust 改动）。此前：**㉓ 壳骨架已完成（v2.39，UI 正式设计②）**：无边框窗口（decorations:false + 1280×820 首启 / 1024×640 最小 + capabilities 窗口控制权限）+ 46px chrome 带三段拼装（侧栏头 / 会话顶栏 lead 槽恒挂标题零跳变 / 窗口控制 120px 车道 z-1000）+ 侧栏（搜索+新会话+「会话」组+时间桶 近7天/近14天/更早；选中 wash-3；运行中脉动点+禁切置灰；底部 扩展/记忆/设置 目的地入口+版本号；会话↔工作区关联未落地，「项目」组随关联批次出现）+ 折叠/拖宽全套（§2.3：8px 热区/圆角条提示/全局锁光标/指针捕获+rAF/拖到最小值以下即折叠/键盘与双击复位/aria separator；宽度 localStorage 持久化）+ 目的地整页接管（扩展/记忆=侧栏右侧铺满；设置=整窗接管+过渡期「‹ 返回应用」；对话 display:none+inert 挂起不卸载）+ pill nav 与 SessionSwitcher 退役。新文件 stores/shell.ts、lib/icons.ts、lib/sidebar-preference.ts、components/shell/{Sidebar,Topbar,WindowControls,DestinationShell}.vue；实机自检（tauri dev 截图走查：亮色缺省/三段带/目的地接管与返回/折叠展开/窗口控制车道）。此前：**㉒ design tokens v2 与基线已完成（v2.38，UI 正式设计①）**：style.css 按 UI shell v3.3 §6 重写（13 档灰阶唯一颜色源 / wash 三层平面内无边框分层 / 表面层级九 token / ink 五级 alpha / 单色 accent 暗=白亮=近黑 / 状态五色 / 形状阶梯 10 档 / 量纲三轴同源 content-max 760 / 动效四档时长四缓动禁过冲）；tailwind 收缩到角色 token——实色走 --lw-rgb-* 三元组保 /16 修饰符、自带 alpha 的整值 var 引用（修饰符静默丢弃禁用），38 阶 hue 表删除、22 个组件 hue 类清零到语义色（徽章 §3.4 16% 底、副作用「纯」改中性、审批按钮去红）；缺省主题 dark→light（§8.3-1 裁定，theme.ts 回落同步 + 测试更新；CSS 产物 47→22KB）。修正原型残留：content-max 820→760、--motion-*→--dur-/--ease-*。文档基线：UI shell v3.3 定稿 + ui-prototype 原型已落库（775b14f）。此前：**㋑·fix 子代理收束汇报误报父史已修（v2.37，提交 2332d66）**：桌面端 smoke 探通报告“tools_used 出现未授权的 mcp_status”——**核查结论：白名单无恽**（FilterToolSource list/resolve 双侧过滤，resolve 成功才写 ToolCall 事件，越权即 NotAvailable → 子步 failed，既有测试覆盖）；真凶相是 `tools_used`/`tail_summary` 扫了整个子转录，把 fork 前缀里**父的历史调用**当成了子的使用记录。修法 = 自身段口径（fork+追加 prompt 后记 own_from，收束函数只扫 events()[own_from..]）+ 回归测试（父史预置 mcp_status → 断言 tools_used==[echo]）；如实记录脚点：tools:[] 空数组 = 继承全目录（与缺省同义，语义变更另行评估）。验证：kernel-agent 全测绿（库 56 + 集成 9，+1）、clippy 0、fmt、check_length OK。此前：**㋑ 工具调用卡默认折叠已完成（v2.36，提交 977853b）**：UI 正式设计阶段先行批（用户裁定：工具执行过程默认折叠，否则对话被 JSON 刷满、正文被挤走）；UI shell 文档 §3.7 先行成文再动码。模式库第二实例 `chat/ToolCallCard.vue`：折叠 = 头部单行（▸/▾ + 工具名 + 版本/副作用徽章 + 参数摘要（args 首个原始值 clip 48）+ 状态），整行 button 点击展开参数/结果明细（v-show 保 DOM）；折叠态组件本地不持久化，重放/切会话回折叠；失败自动展开一次（错误可见性优先，历史重放同规）；运行中不加动画（§六禁常驻）；ChatMessages 内联卡迁出（175→101 行）；clip 收敛进 lib/format.ts；§4.1 组件列改指 ToolCallCard。门禁：vue-tsc+vite 构建过、check_length OK（纯前端批次无 Rust 改动）。此前：**⑳ LLM 流式已完成（v2.35，提交 54a97ba）**：kernel-agent `LlmService::complete_streaming`（缺省回落 complete 整段回调，DemoLlm 零改动）+ `LlmStreamSink` delta 出口（on_delta 返回是否真消费；child_kernel scoped 换入 NoLlmStreamSink，子内 delta 黑盒不泄漏）+ `StepOutcome::Continue { streamed }`；genai `exec_chat_stream`（capture_content/tool_calls 聚合，End 取 captured_content）；`StreamChunkSink` 直推 `agent://chunk`、runner 按 streamed 抑制该步整段投影——**前端零改动正文不重复**，delta 不落盘；搭车修 api.ts CHANNELS 补 schedule-fired 死分支。此前：**⑲ desktop session 持久化已完成（v2.34，提交 a5347c3）**：磁盘 = 内存日志的镜像（`<数据根>/sessions/<id>.jsonl` 一行一事件）——session-store 新 `persist` 模块（append 打开-写-关 / load 逐行回放坏行跳过 / undo rewrite 前滚动备份 `.undo.bak` = 撤销物理留痕 / list mtime 降序 + 首条用户消息截断做标题；session id 白名单防穿越，测试 4 例）；desktop `session_log.rs` 胶水 + init_state 启动恢复（先于调度引擎，调度绑定旧会话跨重启存活）+ run_turn 增 log 参数增量镜像（手聊/调度两路，与投影共用同一游标）+ context_undo_compact 后同步重写；session_list / session_history 命令（历史 = `runner::project_events` 全量重放同一投影，不走事件通道）；前端 chat store sessions/switchSession（运行中禁切，重放恢复气泡/工具卡/系统通知）/newSession（首聊前不建后端状态）+ **行为变化：重启接最近会话继续**；SessionSwitcher.vue 最小切换面（UI shell 文档 §3.6 先行；会话管理类 UI 留给 UI 正式设计）。此前：**⑱ fs roots 工作区目录选择器已完成（v2.33，提交 dc2ad87）**：settings.json `tools` 区新 `fsRoots` 字段（缺省空 = 仅数据根，旧文件零迁移；normalize 校验 trim/去空/去重保序 + 目录存在性 fail-closed 同 fsutil 契约）；env 注入双时机 = 启动 + settings_set tools 区改动即重注入——fsutil `allowed_roots()` 每次工具调用读 env，**保存即生效免重启**，空表移除 env 回落缺省；`pick_directory` 命令（tauri-plugin-dialog 仅 Rust 侧，无 JS guest/无需 capability，前端零新增依赖）；DeveloperSection 工作区目录块（原生对话框添加 / ✕ 移除，增删即存即生效）；UI shell 文档 §3.2 分区表纠偏（memory/developer 占位标记→已实现、补 shell 行）+ §七规则 8（原生对话框封装模式）。此前：**⑰ UI 视觉抛光已完成（v2.32，提交 945e681）**：明暗主题 + 组件层语义颜色类 + 统一动效（壳批次 §六 预留三件兑现；批首交付 = 先改 UI shell 文档 §六/§3.2/§七 再动码）。settings.json 新 `appearance` 区（dark/light/system，缺省 dark 旧文件零迁移）：Rust `settings/appearance.rs` + update 校验 + 测试 2 例；前端 `lib/theme.ts` 唯一执行者（写 `<html data-theme>`，system 档 matchMedia 监听，**组件禁自读媒体查询**）+ AppearanceSection 三档分段控件。颜色三层：style.css 双主题变量块（暗色 = 原调色板值像素级不变、亮色逐 token 浅色族 + color-scheme）；token 双形态（hex + `--lw-rgb-*` 三元组，后者配 `rgb(var(--x)/<alpha-value>)` 保住 47 处透明度修饰符）；tailwind.config 角色色 surface-0..3/ink-1..5/line/accent + neutral 兜底 + 语义 hue 38 阶全量重映射；组件 neutral 灰阶清零 → 角色类（新颜色先扩 config 表再上组件，§七 第 6 条）。动效三类封闭清单：base 层统一 120ms 色变 + 弹窗 fade+scale / SystemNotice 淡入上滑 / 进度条宽度 200ms + prefers-reduced-motion 降 0。已知近似：原生标题栏跟随系统不随应用切换；亮色为逐 token 选配首版待实机反馈。此前：**⑯ tool-sandbox 受限执行沙箱已完成（v2.31，提交 dc822d9）**：批首方案定稿落 plugins.md §六（三候选取① Windows 受限令牌 + JobObject 限额；② WASI 只能跑 wasm 化命令弃、③ 容器可用性依赖用户环境记录为远期可选后端；能力矩阵如实分级：内存/进程数/CPU 占比硬限、剪贴板 UI 限制、无特权受限令牌不可 UAC 提升、整树治理 ✅，**fs/net 不隔离 ❌ 不做虚假安全声明**）。plugins-src/tool-sandbox 1.0.0：单工具 `sandbox_run`（Irreversible 一律审批）= JobObject 限额（mem_mb 512/max_procs 64/cpu_rate 可选硬顶，arg/env 双层缺省钳上限）+ 受限令牌 spawn（CREATE_SUSPENDED→挂 Job→Resume；降级 job-only 经输出 `isolation` 字段如实上报不静默）+ 与 tool-shell 同源三道闸且**高危无解锁开关**（fs 不隔离下解锁无增益，引导走 shell）；Unix 对称 pre_exec setrlimit + 进程组树治理；与 tool-shell **并存不替换**；测试 15 例含真跑受限 spawn 全链。此前：**⑮ agent-sub 子 Agent 调度已完成（v2.30，提交 d29cd69）**：kernel-agent `subagent/` 模块（mod/filter/run/tool 分文件）——`subagent_run(prompt, tools?, max_turns?, timeout_secs?)` 工具面（Irreversible 宁严勿松 + Always 暴露，仅 desktop 注册）；`SubagentGate` 快照表由内核每步刷新（fork 前缀含本轮 user 消息；残留至进程退出、128 条全清防御）；子会话 = `Session::fork(cursor())` 可见前缀克隆；`Kernel::child_kernel` 派生作用域子内核 + `FilterToolSource`（tools 缺省继承父全目录、非空严格子集；**单层硬剔除**白名单包含也无效）；轮数 8/32、超时 300s/1800s（0=不限）越限 **budget_exhausted/timeout 如实收束**摘要不虚构；`ToolCallReq` 增 `session_id`（12 处构造点补齐），子内 Irreversible 冒泡同一审批门、审批载荷带 `{parent}-sub-{n}`、ApprovalModal 子 Agent 徽章；派生/收束落父日志 + SystemNotice（§4.1 扩表先行），子内活动黑盒不投影；desktop init_state 重排（目录提供挪 kernel 后，工具持内核 Arc 有意成环）。此前：**⑭ 触发与推送已完成（v2.29，提交 7dc0139）**：tool-schedule 宿主内置工具面（desktop `scheduler/`：schedule.sqlite3 + `schedule_create/list/delete` 经 with_builtin 进目录——create=Always+Irreversible 审批即告知，list/delete=Pure 按会话隔离）+ 触发引擎（1s tick；目标解析=调度行会话条目存活→原地注入、行已不在→落最近聊过的活会话不落僵尸、无会话→顺延等待；忙 30s 顺延、一次性 2h 放弃并系统通知、周期让位跳槽不追帧；启动补跑=首次 tick 的 due() 天然覆盖）；`CallCtx.session_id` 首次落地（kernel-agent 新 `step_as`，runner 切换，普通 step 语义不变）；触发先推 `agent://schedule-fired`（SystemNotice）再注入轮，窗口不可见/失焦补系统通知兜底；plugin-api `HostVtable` **末尾追加 notify 槽**（append-only，ABI 版本不动；plugin-host `set_notify_hook` 组装点注入，headless=ERR_UNSUPPORTED 如实上报）；plugins-src/tool-notify 1.0.0（`notify` 工具按 channel 二选一：webhook 照搬 tool-fetch SSRF 防线且**非 2xx=未送达=报错**；desktop 走 vtable 槽；整体 Irreversible 宁严勿松）。拆分落点：kernel.rs→`kernel/request.rs`（决策环+请求管线）、state.rs→`state/app_state.rs`（结构体+方法）。此前：**⑬ 智能层已完成（v2.28，提交 42bcbf9）**：四项共享 embedding 基建（可降级总原则：embedding 缺席 = 智能层缺席，聊天零影响）：kernel-agent `tool_retrieval`（EmbedPort 契约 + 描述向量缓存）+ ToolExposure 接 0.5×similarity 检索信号（只在真要裁剪时才打 embedding）+ Memory-Inject 经历块（ExperienceSource async 契约）；desktop `llm/embed.rs`（OpenAI 兼容 /v1/embeddings + SharedEmbed 热换 + 60s 故障冷却）、settings 新 embedding/plugins 区 + memory.autoExtract、`experiences.rs`（experiences.sqlite3，维度不符行退役）、`extract.rs` 回合后提取（事实≤3 条强制 auto. 前缀永不覆盖显式记忆 + 经历摘要嵌入落库）、`autodisable.rs` 自动停用巡检（内置不参与、只停不启）、search_tools 语义优先降级关键词；前端 EmbeddingSection/PluginsSection 新分区 + MemorySection 开关 + MemoryPage 经历区块；commands 拆目录模块、settings/update.rs 拆出保 400 行。此前：**「禁用」统一改名「停用」**（用户裁定，内置与第三方同一动词）；内置插件可停用——`InstallSource::Builtin` 落 lockfile（开发态扫描装载即登记、保留 enabled 重启不复活、`dev_scan_skip_ids` 跳过停用/接管项、启用从内置根解析最新版本、uninstall/update 服务端拒绝、对账不删内置条目），停用即出工具候选池（resolve 只遍历 active_instances），InstalledTab 内置行加停用/启用按钮（不进批量）；门控一律按 source（lockfile）非 origin（manifest 自声明）；版本/工件解析下沉 plugin-service::builtin 共享。上下文窗口迁模型配置——settings `context.windowTokens` → `llm.windowTokens`（open 读旧值迁移一次幂等；**ContextSection 只去窗口下拉一条**，阈值/策略原地不动）；LlmSection 增窗口下拉 + Ollama `/api/show` 检测命令 llm_probe_window（probe.rs，reqwest json 特性）+ lib/model-profiles.ts 静态档案（前缀命中只在未设置时自动填充，手动优先）+ 手动兜底。数字呈现 compactNumber 接线（LlmSection 预设档位 / ContextSection 阈值预览 ≈51.2K / ContextProgressBar 比例段 ≈52K/256K / 压缩通知已节省）。约束最小化余项——tool-shell `LOCOWRK_SHELL_TIMEOUT`/`LOCOWRK_SHELL_MAX_BYTES` 环境默认（钳制+告警，arg 上限不变，产物 1.2.0）；宿主 Timeout 可配（kernel-agent `Timeout::hot` 共享锁 + settings tools 区 callTimeoutSecs 热换入）；版本策略定稿方案 B 免加 bundled 字段（packaging.md §9.4）。此前：S1 tool-shell 安全分级与进程治理已完成（v2.26，提交 f0fce64）：双工具分级——`shell`（审批面不变）+ 新 `shell_read`（Pure 只读豁免：ls/cat/grep + git status|log|diff|show 固定表；paths.rs 路径校验复用 fsutil allowed_roots 同源根，cwd/参数出根即拒并引导改用 shell，`-f/…` 贴值与 `--flag=..` 形态封堵）；三级分类硬锁/高危拆双清单（+17 项硬锁：xargs/env/find/nohup/time、nc/telnet/rsync、wscript/mshta/rundll32/osascript 等）；高危解锁开关 settings.json `shell.unlockRisky` → 插件加载前写 env（重启生效，解锁=进审批而非放行）+ ShellSection 设置分区；进程树治理（Windows Job Object KILL_ON_JOB_CLOSE / Unix process_group+killpg，正常退出也收残留——顺带修掉旧代码孙进程握管道的悬挂）；截断提示文案 + 启动期 allow∩denylist 冲突 warn；tool-shell 1.1.0 产物重建。此前：⑫ 打分截断 + search_tools 元工具已完成（v2.25，提交 da96c6f）：kernel-agent ToolExposure 监听器（agent/request 注册最前：ToolExposure → Memory-Inject → ctx-compact；会话热度从 payload 消息数 ToolResult 零新 trait，全局 EWMA 走 GlobalEwma trait 宿主实现；always 直通不占额度，Top-N 截断缺省 20 可配热换入，fail-open）+ usage.sqlite3 flush 时 EWMA 衰减积累（exp(-days/7)，⑩ 预留列开始写）+ search_tools 宿主内置元工具（always+Pure，关键词检索全目录含 hidden——RegistryToolSource 扩 list_all_tools；空结果回退 top-10 高频 ∩ 目录）+ 开发者分区真身（DeveloperSection：截断设置 + 最近暴露快照只读面板，打开时拉取不轮询，节省走 §3.5 档位）——⑨⑪⑫ 后设置面板占位清零。此前：⑪ 记忆注入 + 记忆管理 UI 已完成（v2.24，提交 d342551/7c581b7）：memory-sqlite 1.2.0（memory_write/read 命名对齐 + write/read/search 标 exposure=always；写路径本就同步落盘）；Memory-Inject 中间件（MemorySource trait 宿主实现，注入写 `RequestPayload::system` 新字段——记忆在消息列表之外，"系统提示永不压缩"由结构保证；注册次序 Memory-Inject → ctx-compact）；settings.json `memory` 区（enabled / topK 缺省 5，热换入）；记忆管理页（搜索/行内编辑/删除/清空 + SourceBadge 来源徽章 + lib/format.ts 相对时间）+ MemorySection 设置真分区。此前：⑩ 暴露声明 + 使用统计地基 + 闲置 UI 已完成（v2.20，提交 f6b1a44）：exposure 字段零破坏追加（缺省 on_demand，always>10 加载告警，hidden 不进目录）+ ToolCall 补 plugin_id（serde default 兼容旧日志）+ usage.sqlite3 宿主自有 5s 刷盘 + IdleLevel/IdleInfo 后端裁决（前端只渲染人类文案）+ plugin_list_with_usage / 批量停用/卸载命令 + InstalledTab 闲置徽章/两排 chips/批量栏（卸载二次确认列名）/提示条；**搮车修复（9a5d39c）：插件工具名去点号**（OpenAI 兼容接口拒收点号，22 个工具名改下划线 + 版本 bump + 产物重建，用户需重启桌面端生效）。此前：⑨ 上下文管理收口（P0）已完成（v2.19，提交 92f298b）：settings.json `context` 区（窗口预设+自定义，未知回落 32k 保守保护 / 阈值 10–90% / 策略三档）+ `ContextSection` 替换壳批次占位；compact.rs 配置驱动重写（CompactConfig 共享 RwLock 热换入、滑动窗口/分层/仅摘要、预算制切点压到阈值 50%、摘要失败降级滑动、suggested_threshold 分档建议）；`SessionEvent::ContextCompacted` 进会话日志（摘要存全文，derive/state 折叠留尾部 kept_messages，serde default 兼容旧日志）+ `context_undo_compact`（rewind_to+fork，其后轮次一并回退）；`agent://context-usage` 进度条接线 + 压缩 SystemNotice [查看详情][撤销] 真接线。此前：壳批次（UI 骨架）已完成（v2.18，提交 a2c82a5）：顶层 4 视图定稿（对话/插件/记忆/设置）、页面模式库（`patterns/ListPage` 列表页 / 设置分区一一对应 / `chat/SystemNotice` 会话内系统消息 / IdleBadge + ContextProgressBar）、事件→UI 流向图与弹层规则成文 `docs/design/UI shell and patterns.md`（功能批次照其 §七只扩表不改骨架）、design tokens `--lw-*`、PluginMarket 291→54 行、托盘菜单 i18n 搭车完成。更早：编号阶段①–⑧全部落地（⑧ WASM 加载器 v2.16：PluginLoader trait + WasmLoader wasmtime 34 沙箱、.wasm 扩展名分发、epoch 预算按次重置、宿主 alloc/free 缓冲协议；⑦ 第三批插件 tool-git / memory-sqlite / tool-mcp v2.15；⑥ 第二批插件 + 安全底线 v2.14；⑤ 托盘+快捷键+~/.Locowrk v2.13；④ 元数据 v2 v2.11/v2.12；③ 第一批工具+mw-audit v2.9；② 真实 LLM genai 三合一 v2.8；① i18n+最小设置面板 v2.7）；阶段一～七（内核骨架 / Effect+Fiber / C ABI 热插拔 / Waterfall / Tauri 脚手架 / 审批弹窗 / PluginService+市场）均已提交；落点细节见 DESIGN.md §14 实现备注。**后续排期：P2 四件已入正式排期（2026-09-30 用户确认），⑭⑮⑯⑰⑱ 已完成（v2.29–v2.33）——之后全部看需求——**编号批次全部收口，UI 正式设计进行中（㉑ 先行批 + ㉒ tokens v2 + ㉓ 壳骨架 + ㉔ 导航轨⌘K + ㉔·fix 板面切换已落，v2.37–v2.41；㉕ 对话主区 / ㉖ 审批决策卡+会话运营 / ㉗ 工作面板+通知 已排期见 ROADMAP）**（用户 2026-09-30 裁定进入该阶段；2026-10-01 UI shell v3.3 定稿 + ui-prototype 静态原型；2026-10-02 用户补充裁定：后端服务前端、缺的功能后期逐步补、前后端拆分可调、像素级+逐帧动画验收、参考截图 F:\Warehouse\AAA\333、PI-Desktop 源码对照附录 A）；tool-browser 用户裁定五件中排最末。其余收尾 backlog（看需求随批次搭车）：权限模式四档切换器 / 会话保留天数 / tool-http / mw-ratelimit / 中间件插件化 / desktop session 持久化 / LLM 流式输出 / 市场 wasm 分发 / kv_get 协议 / profile.toml 接线**；收尾 backlog 随批次搭车。**逐项进度（完成打钩）看 docs/design/ROADMAP.md**

## 目录结构

```
docs/design/DESIGN.md   权威设计文档（唯一需求来源）
docs/design/ROADMAP.md  进度看板（唯一进度表：完成打钩 / 未完排期，每批完成先勾它）
docs/design/LLM.md      阶段八②参考：三合一 LLM 适配器（方案一 = genai 统一库封装）
docs/design/plugins.md  阶段八③参考：必备插件清单与落地顺序（P0 四件套起步）
docs/design/Plug-in packaging.md  阶段八④参考：插件元数据与打包 v2（多语言/图标/manifest.json）
docs/design/Tool exposure strategy and usage statistics.md  ⑩⑫参考：工具暴露策略与使用统计（exposure 三层 / EWMA / 闲置分级 / 闲置 UI）
docs/design/Context and Memory.md  ⑨⑪参考：上下文与记忆管理（压缩策略三档 / ContextCompacted 撤销 / Memory-Inject / 记忆管理页）
docs/design/UI shell and patterns.md  壳批次落点：UI 导航模型 / 页面模式库 / 事件→UI 流向图 / 弹层规则 / design tokens（功能批次只扩表不改骨架）
crates/
  plugin-api/           冻结 ABI 契约（唯一，改动即破坏兼容；ffi/manifest/plugin_side/tool 四模块）
  cordis-core/          Context / Effect / Fiber / Scope / EventBus
  kernel-agent/         Agent 主循环 / Waterfall / 中间件链
  plugin-host/          dlopen Loader / 多版本 Registry / FFI→Tool 适配
  plugin-service/       插件安装/卸载/更新编排（CLI 与桌面端共用；阶段七落地）
  session-store/        会话事件日志 / 快照 / 回放 / 补偿
  kernel-bin/           CLI 可执行入口（组装一切）
plugins-src/            内置插件源码（workspace 成员；tool-echo 为阶段三验收插件，tool-shout 为阶段七市场验收插件）
desktop/                Tauri 2 + Vue 3 桌面端（src/ 前端 + src-tauri/ 内核组装；src-tauri 是
                        独立 Cargo 包、不进根 workspace，见根 Cargo.toml 的 exclude）
plugins/                开发态插件安装目录，布局 plugins/{id}/{version}/（产物已 gitignore）
market/                 本地目录市场（阶段七；bash scripts/build_market.sh 暂存，产物已 gitignore）
config/profile.toml     当前激活的插件组合（阶段八设置面板接入）
```

各 crate 的 `lib.rs` 顶部注释列有计划模块和对应 DESIGN.md 章节号。

## 常用命令

```bash
cargo check          # 快速类型检查
cargo build          # 构建
cargo test           # 全部测试（workspace）
cargo test -p cordis-core   # 只测某个 crate
cargo clippy --workspace --all-targets
cargo fmt
bash scripts/build_plugins.sh  # 插件产物暂存到 plugins/（tool-echo 双版本 + tool-fs / tool-fetch）
bash scripts/check_length.sh   # 单文件 400 行上限检查（pre-commit 自动执行）
cargo run -p kernel-bin -- demo  # 阶段三验收叙事（需先跑上一条）
```

Git hooks 在 `.githooks/`（随仓库提交）。本仓库已通过 `git config core.hooksPath .githooks` 启用 pre-commit 检查；重新克隆后需重新执行一次该命令。

验收叙事（前置 `bash scripts/build_plugins.sh`）：

```bash
cargo run -p kernel-bin -- demo    # 阶段三：加载 1.2.0 → 激活 1.3.0 → 回退 1.2.0，全程不重启
cargo run -p kernel-bin -- demo4   # 阶段四：审批插件经 agent/request 改写参数；两 Agent 工具视图隔离
```

桌面端（desktop/，Tauri 2 + Vue 3；前置 `pnpm install` 一次、插件产物已构建）：

```bash
cd desktop && pnpm tauri dev        # 开发模式（窗口 + 热重载）：聊天、观察 tool-echo 工具调用
cd desktop && pnpm build            # 仅前端：vue-tsc 类型检查 + vite 构建
cd desktop/src-tauri && cargo test  # 桌面端 Rust 单测（runner 投影 / demo_llm / plugins_root）
```

注意：`desktop/src-tauri` 被 exclude 在根 workspace 之外，`cargo check` 等要在
`desktop/src-tauri` 目录下单独执行；其 `generate_context!` 编译期要求 `desktop/dist/`
存在（先 `pnpm build` 一次）。

网络提示：crates.io 直连超时时，给 cargo 命令加
`--config 'source.crates-io.replace-with="rsproxy"' --config 'source.rsproxy.registry="sparse+https://rsproxy.cn/index/"'`
走镜像（命令行级覆盖，勿改全局 ~/.cargo/config.toml）。

## 架构边界（依赖严格单向，违反即设计错误）

```
kernel-bin        → kernel-agent + plugin-service + session-store   （组装点）
desktop/src-tauri → kernel-agent + plugin-service + session-store   （组装点）
kernel-agent      → cordis-core + plugin-api + session-store
plugin-service    → plugin-host + cordis-core
cordis-core       → plugin-api
plugin-host       → plugin-api
session-store     → plugin-api
```

（`X → Y` 表示 X 依赖 Y，与 DESIGN.md §8 一致。kernel-agent 与 plugin-host 互不依赖：
共用的 `Tool` / `CallCtx` 定义在 plugin-api 宿主侧、不进入 C ABI。内核经 `ToolSource`
服务 trait 解析工具；依赖图禁止 plugin-host 依赖 kernel-agent，故 plugin-host 只提供
同语义的固有 API（`RegistryToolSource::resolve`），由组装点（kernel-bin 的
`tool_source.rs`）桥接成该 trait 并注册进 Context）

- `plugin-api` 不依赖任何其他 crate；`cordis-core` 只依赖 `plugin-api`
- `kernel-bin`（CLI）和 `desktop`（GUI）是仅有的两个组装点，业务逻辑不写在这里
- **UI 永远不直接操作 `PluginRegistry`**，只通过 `PluginService`；Tauri command 是薄适配层，只做参数转换，不写业务逻辑
- 流式输出走事件推送（`agent://chunk` 等），不塞进 command 返回值

## 模块与文件规模纪律

- `lib.rs` / `main.rs` 只放模块声明、re-export 和文档注释，不写实现
- 实现必须落到 DESIGN.md 规划的模块文件（如 cordis-core 的
  `context.rs` / `effect.rs` / `fiber.rs` / `scope.rs` / `event_bus.rs` / `runtime.rs`）；
  文档未规划的新职责 = 先建新文件并加 `mod` 声明，禁止堆进已有文件
- **单文件软上限 400 行**，超限按职责拆成目录式子模块（`context/` + `context/mod.rs`）；
  `scripts/check_length.sh` 会机械检查，pre-commit 拒绝超限提交
- `#[cfg(test)]` 测试放独立文件（如 `effect/tests.rs`，模块内以 `#[cfg(test)] mod tests;` 声明）
- 内部类型一律 `pub(crate)`，只有跨 crate 需要的才 `pub`
- 动手写码前，先列出"将新建/修改哪些文件、各放什么"的清单，再按清单实现
- 超长文件的拆分是独立任务：按职责拆子模块，保持外部 API 与既有测试不变

## 设计纪律（写内核代码必须遵守；完整版见 DESIGN.md 附录 C）

1. 所有对 `Context` 的修改必须走 `effect()` 并携带逆操作（Disposer），撤销按 LIFO
2. 永不 dlclose：逻辑卸载 + 物理保留（graveyard），进程重启才真正释放
3. FFI 调用放 `spawn_blocking`；插件与宿主两侧都必须 `catch_unwind`
4. 先写 `ToolCall` 事件再执行工具——崩溃后 replay 能识别悬空调用
5. 会话事件只追加不删除：回退 = 移动游标 + replay，快照带 `schema_version` upcaster
6. `SideEffect::Irreversible` 工具调用前必须审批，审批结果也是一条事件
7. 补偿工具必须幂等，用 `call_id` 做去重键
8. 多版本并存：回退 = 切换 `active` 指针；更新失败自动回滚，旧版本永远保留

## 平台注意事项（Windows 开发机）

- Shell 是 Git Bash，路径写正斜杠
- 设计文档示例中的插件产物 `.so`，在 Windows 构建下是 `.dll`（见 plugins/README.md）
- dlopen 路径区分平台后缀，`plugin-host` 实现时注意 `lib`/`.dll`/`.so`/`.dylib` 前后缀差异

## 工作规则

- `plugin-api` 是冻结 ABI：对外发布后只允许把字段追加到结构体末尾，不改已有签名；`PLUGIN_ABI_VERSION` 变更是 breaking 事件
- 临时实验文件放 `tmp/`（已 gitignore）或系统临时目录，不要放仓库根目录
- 不要把 API Key、Token、密码、私钥写进任何项目文件
- 删除、覆盖、迁移、发布等高风险操作先向用户确认
- 完成任务后说明：改了哪些文件、解决了什么问题、是否运行了测试、遗留风险
- 每完成一批，先勾 docs/design/ROADMAP.md（⬜/◐ → ✅ 并补版本与提交哈希），再写 DESIGN.md 落点补记
