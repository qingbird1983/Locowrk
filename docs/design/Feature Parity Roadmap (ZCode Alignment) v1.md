# 功能面对齐路线（ZCode 对齐）v1

> **状态：待研究。本文是差距盘点 + 逐域落法设计 + 候选批次清单，不是已实施设计，别照它写码。**
> 立稿依据：用户 2026-10-11 裁定——「调研 ZCode 的功能层面，对照 Locowrk 缺失的做对齐设计，作为逐步迭代补齐的路线；谁先谁后由用户届时安排；ZCode 自家登录账号体系不需要」。文中**不做任何批次时序承诺**，§7 只给候选批次与依赖关系。
> 调研方法：对 `F:\Warehouse\Github\ZCode` 做 5 路并行勘察（命令/CLI 面、扩展系统、桌面工作台、远控与自动化、外加 Locowrk 现状盘点），证据以 ZCode 仓库的 docs spec 与源码 file:line 为准（附录 A）；Locowrk 现状锚点（附录 B）全部实码核对。账号 / OAuth / 支付 / 套餐 / ARMS 遥测**不在调研范围**（§6）。
> 关联文档：《Memory Redesign (ZCode Alignment) v1》（记忆域不重复展开）、《Conversation Flow Redesign (ZCode Alignment) v1》（对话流 UI 不重复展开）、《UI Design System》（视觉规范不重复展开）。本稿只管**功能域**。

---

## §0 总判断

ZCode 是三层运行时产品（Renderer / Host / ZCode CLI + 手机 Web 远控 + Bots 渠道），功能面约 20 个域。Locowrk 的**内核域**（审批、多版本插件、会话存储、记忆、子代理、定时任务、多模型）普遍不落后，部分更细（七序审批表 vs ZCode 模式制；冻结 ABI 插件 vs ZCode 声明式资源包——两者本质不同，见 §3 备注）。真正的缺口集中在四类：

1. **扩展生态域**（用户点名）：斜杠命令全集与自定义命令、Skills 技能、Hooks 钩子、MCP 强化——ZCode 全部就绪且有 16 家竞品一键导入；Locowrk 斜杠命令仅 1 条、无 skills、无 hooks。
2. **工作台域**（用户点名）：内置终端（真 PTY）、后台 bash、文件阅读器（Markdown/PDF/Office）、Git 审查面板强化、文件级 checkpoint/rewind、分屏、会话内查找——Locowrk 有 WorkPanel 三页签雏形（subagent / review / browser），无终端无阅读器。
3. **触达域**（用户点名远控）：移动端 Web 远控、Bots 渠道（Telegram/微信/飞书）、会话分享、远程工作区（SSH/WSL）、headless CLI——Locowrk 全缺。
4. **平台治理域**：自动更新、反馈日志导出、存储管理、本地用量统计、快捷键设置页、deep link、轨迹查看器——单点都不大，ZCode 全有。

---

## §1 ZCode 功能面全景（勘察结论）

### 1.1 命令与 CLI

- **内置斜杠命令 21 条**（`packages/shared/src/zcode-slash-command-help.ts`）：`/help /login /logout /compact /init /expert /effort /dwf /fork /locale /mcp /plugins /mode /model /new /resume /rewind /skill /goal /workflow` + App 独有 `/plan`。全部有保留名保护，自定义命令不得占用。
- **自定义命令**：`.md` 文件即命令（frontmatter `description/allowed-tools/argument-hint/model/skills`），目录层级 = 命名空间（`review/code.md` → `/review/code`），`$ARGUMENTS` + `$1..$n` 展开；发现目录：用户级 `~/.zcode/commands`、`~/.agents/commands`，项目级 `<ws>/.zcode/commands`、`<ws>/.agents/commands`，向上扫描到 git 根合并。
- **CLI 子命令**：无参全屏 TUI、`-p` 非交互单 prompt（headless，支持 `--attach`、`--output-format stream-json`、`--target` 设 goal）、`-c/--resume` 恢复会话、`doctor`、`plugins` 全生命周期、`skills list`、`hooks` 信任管理、`commands list`、`app-server`（stdio NDJSON 协议服务，桌面 Host 的运行形态）。
- **计划模式**：与权限模式独立的布尔；`ExitPlanMode` 恒需用户批准，批准边界三态（approve / deny / deny+反馈意见转为真实 user message 防自循环）；计划文件批准后原子写入 `<ws>/.zcode/plans/plan-<sessionId>.md`，compact 后注入全文 reminder 保 resume 连续。
- **输入编排**：CommandInbox FIFO 串行准入、运行中插话 steer、队列编辑/重排/暂停、`/compact` 作为维护意图入队不打断；Prompt Enhance（发送前润色回填）；附件（图片/PDF/视频，40MiB 预算，模型不支持时投影为占位文本）；Task Search（Cmd+K 命令中心，跨任务搜索，SQLite `searchable_text`）。

### 1.2 扩展系统

- **Skills**：含 `SKILL.md`（frontmatter `name/description[/version]`）的目录；会话启动只注入 name+description 元数据，命中后按需读正文（渐进披露）；来源三级 Workspace > User > Extension（插件内置），同名覆盖；触发 = `$` mention 建议 / 模型语义自配 / 设置页手动；无在线技能市场（明确 non-goal），分发走插件商店。
- **Plugins（与 Locowrk 插件不同物）**：声明式**资源包**——一个 manifest 目录可打包 skills / commands / MCP servers / hooks / subagents / UI 交互页面（MCP Apps）/ userConfig 默认值；市场 = 官方内置分片 + CDN 分片 + 默认注册 anthropics 官方 + 个人源（github/git/URL/本地路径/拖放）；安装到本地 cache，插件级启停；兼容 Claude Code marketplace（分阶段投影）。
- **Hooks**：7 个生命周期事件（SessionStart / UserPromptSubmit / PreToolUse / PermissionRequest / PostToolUse / PostToolUseFailure / Stop），process（argv）或 command（shell）两种类型；输出效果 = exit 2 阻断、`updatedInput` 改参（重过 schema）、`additionalContext` 注入、Stop block 续跑上限 3 次；**信任模型**：workspace 仓库自带 hook 默认不信任，Trust = workspaceIdentity + hook 声明 digest 持久化，声明变更即失效重审，fail-closed。
- **MCP 客户端**：stdio / http(Streamable) / sse 三 transport + OAuth（authorization_code+PKCE、client_credentials）；配置按 User/Workspace scope 存 `mcp.servers`；插件贡献的 MCP 按 pluginId 命名空间合并防覆盖；设置页 Form/JSON 双模式 + 状态诊断。
- **子智能体**：`.zcode/agents/*.md` frontmatter profile（内置 `general-purpose`/`Explore` 为保留名，用户/项目/插件同名覆盖）；前台/后台两态（`run_in_background` 返回 async_launched，后台完成以 `<task-notification>` synthetic user input 唤醒父 agent）；`SendMessage`（父→子按 agentId 续聊）/ `RespondToCoordinator`（子→父）；工作区右侧 side tabs 每个 child session 一个只读查看 tab；设置页可给各 profile 配模型与推理档位。
- **外部导入**：设置页四类资源（skills/commands/plugins/MCP）一键从 16 家竞品目录导入（Claude Code、Codex、OpenCode、Qwen Code、Windsurf、Trae、Kiro、Roo 等），统一「Global/Project 目标 × 软链或复制」、同名跳过；首启迁移向导。

### 1.3 桌面工作台

- **内置终端**：真 PTY（node-pty）+ XTerm.js；底部 Terminal 面板（单例）+ 右侧 side pane 多 tab（每 tab 独立 PTY 实例，跨 workspace 切换保活，关闭才杀进程）。
- **后台 bash**：前台 Bash 默认等 120s（上限 600s），超时且可后台时子进程不杀、自动移交后台任务（返回 taskId + 输出文件）；输出面板每秒轮询 8KiB 尾窗、上滚冻结跟随；5GiB 软上限杀进程树。
- **CodeViewer 与 Git 审查**：CodeViewer（语法高亮/行号/主题）承载文件查看与 Markdown 代码块只读展示；Git 面板 = **轻量改动审阅**（改动提示、diff 右栏、提交/推送/建分支菜单），明确不做 stage/unstage/rebase 全套；Git Graph 只读提交图（泳道 SVG，分页）；**checkpoint 服务**（createCheckpoint / diffCheckpoints / restoreBetweenCheckpoints，不可变快照不依赖 worktree）；**文件级安全 rewind**：撤销本轮文件修改——确认弹窗、按文件标注可/否安全撤销、bash 改动不参与、多 checkpoint 倒序回放、all-or-nothing。消息级 cascade rewind 已下线，替代 = `editUserQuery`（改历史重跑）/ `retryTurn` / `applyFileRewind`。
- **Markdown 与媒体**：阅读级字阶（h1~h6/表格/代码块全 token 化）；表格独立横向滚动开关 + 长表 sticky 滚动条；流式渲染无逐词动画（增量来自 projection 更新）；图片全屏预览（缩放/翻页/下载）。
- **Office 预览**：PDF（分页缩放）、DOCX/DOC、XLSX 只读预览 pane；PPTX 与原生编辑为规划；Agent 文档生产走官方 documents/pdf/presentations/spreadsheets 四个内容插件。
- **内嵌浏览器**：手动浏览 tab（`<webview>` 地址栏/前进后退/DevTools/element-picker）+ agent browser-use（Node REPL → CDP-on-guest）；tab 管理（list/get 绑定激活、32 tab LRU 硬上限、复用打开）；轮次结束自动截 active tab 作轮尾图片；HTML 卡片「在浏览器中打开」走 shell。**Computer Use**（macOS/Windows beta）：独立 Helper 进程 + permission broker，fail-closed。
- **工作台布局**：split pane（VS Code editor groups 模型，二叉分割树、叶子 ≤4 pane、pane 绑定 workspace+session、跨 tab 常驻）；会话分组（拖拽会话入对话区建可恢复 group）；会话内查找（Cmd+F 对话范围、大小写不敏感、数据级索引）；**白板**（右侧 pane 画板，按 workspace 隔离，导出 PNG 入对话附件）；**模型轨迹查看器**（model-io JSONL 可视化：角色/token/耗时/finishReason/工具出入参，保留最近 3 会话）。
- **上下文与任务**：上下文用量条（事实源 = snapshot.usage，UI 不本地累加）；缓存命中率（token 加权，生产阈值 78% 以下不显示）；任务看板（SQLite 索引、pinned 按 workspace scope、刷新语义分类学）；会话上下文工具（`#sess_*` 读取其他持久会话，relevant/handoff 两策略）；session 空闲回收（10 分钟 TTL + LRU 高低水位，运行/排队/等交互/被订阅不回收）。

### 1.4 触达与远程

- **移动端 Web 远控**：手机浏览器 + Android 原生壳（扫码/deep link/前台服务，不建 runtime）；架构 = 桌面以 device 角色、手机以 terminal 角色连外部 relay（只鉴权配对+心跳+透传，不解析业务）；手机 attach 桌面 Host 的 workspace scope，与桌面共享同一 task runtime；能做 = 浏览任务/打开会话/发输入/新建任务/处理审批与提问/置顶归档/触发重连；断线恢复 = replayable profile（重连 → resume/snapshot → gap 一次 resync → 从 projection 恢复权限/队列/pending command）；快照预算 200KB/10 条消息，超限裁剪 + bodyRefs 补拉；非 owner 端过滤流式 chunk（「另一台设备正在发送…」）；跨端统一 CommandInbox 队列。
- **远程工作区**：SSH（ssh2 池化）/ WSL（wsl.exe 免认证）/ Docker / Server（连已运行实例）四种；Agent runtime、文件、git、terminal 全在远端执行（远端部署 zcode-server）；连接池 window-scoped，最小化/托盘不销毁；状态权威在远端（Provider 配置单向镜像）。
- **会话分享**：按 productTurnId 选已完成 turn 上传，Web 落地页只读预览 + `zcode://share/import` deep link 唤起桌面导入为本地新任务；Highspeed 加速卡为客户端本地生成的营销卡。
- **Bots 渠道**：Telegram（长轮询）/ 微信 iLink（长轮询）/ 飞书（WebSocket streaming card）三渠道；每个 bot = 完整访问实例（provider/凭据/workspace scope/命令策略）；私聊绑定任务后 IM/桌面/手机任一端输出回传绑定用户；飞书群 = 一群一默认会话 + 每话题独立会话；审批以 v4 resolveInteraction 为提交点。

### 1.5 自动化与平台杂项

- **定时自动化**：workspace 主视图级管理页（标题/指令/项目/权限/模型/推理档）；cron + 「自定义重复」（interval 1–200 + anchor 锚点推进，scheduleRule 为权威、cron 为兼容载体）；全局上限 20 条（SQLite 串行检查）；`automation_runs` 运行台账 + 立即运行 + single-flight；消息气泡带「由定时任务发送」来源标识。
- **闲时任务**：一次性后台任务，提交取号排队、服务端按算力空闲准入、免费池专用 provider、3h 票据分段续期 resume；砍掉余额/ETA/专用审批状态机（明确的减法先例）。
- **平台**：托盘（close-to-tray 默认开、菜单含新建任务/检查更新/导出日志）；快捷键设置页（应用内命令绑定，明确不做系统全局键）；自动更新（服务端 manifest、自动下载安装、release notes）；帮助菜单更新日志；反馈上传（单文件 8MiB/包 32MiB、强脱敏、默认勾选可取消）；本地崩溃 minidump 留档；数据目录可配置（bootstrap 指针固定 homedir）；深链接（`zcode://workspace/open`、Finder/资源管理器右键打开）；存储管理页（按磁盘卷展示 `.zcode` 占用、分类下钻、轨迹可清理）；用量统计页（本地 App Usage：累计/峰值 token、连续天数、日历热力图——与套餐额度拆分）；窗口尺寸持久化；系统缩放快捷键。

---

## §2 Locowrk 现状锚点（实码盘点）

- **插件 10 个**（plugins-src/，9 现役 + tool-shout 市场夹具）：memory-sqlite（6 工具）、tool-echo、tool-edit（3）、tool-fetch（SSRF 守卫）、tool-fs（6）、tool-git（7：status/diff/log/commit/push/undo_commit）、tool-mcp（mcp_status/mcp_reload + 动态注册远端 MCP 工具）、tool-notify、tool-shell（shell/shell_read 唯一命令入口）、tool-shout。
- **宿主内置工具**：ask_user、search_tools、browser_open/browser_console + lwpreview:// 预览协议（fsRoots 限定、20MB 钳）、memory_expand/collapse/search_titles/project_history、schedule_create/list/delete、subagent_verify、subagent_run（kernel-agent，prompt+工具白名单+max_turns）、pdf_to_images、screenshot/read_image、send_email（演示）。
- **桌面端**：会话管理（新建/删除/重命名/Ctrl[ ]切换）；设置 12 分区（appearance…developer/sessions，开发者分区连点 7 次解锁）；审批/询问/Uncertain/Queue 四卡；ToolRow 可展开 + diff 摘要；WorkPanel 三页签（subagent/review/browser，`stores/work-panel.ts:20`）；无终端；ImageViewer；markdown 渲染 + 代码块复制；双语；dark/light/system；上下文用量组件四件套；压缩撤销 undo 动作；5 个硬编码快捷键；CommandPalette；SlashPanel 仅 1 条「整理」（`lib/slash-commands.ts:18`）；托盘已有。
- **会话存储**：事件日志 + 游标回退（`rewind_to/rewind_before`，`session.rs:366-371`）、fork 原语（`:382`，无 UI）、快照+upcaster（**未接线**，v2.148 RESERVED）、补偿、uncertain 崩溃恢复、markdown 导出、truncate_after（后端有前端未接）。
- **kernel-bin CLI**：仅 demo / demo4 / new plugin 三子命令，无 headless 交互。
- **plugin-service**：本地目录市场 LocalDirMarket、安装/启停/回滚/更新、桌面 PluginMarket UI；CLI 侧无 plugin 子命令。
- **调度**：1 秒 tick 引擎，一次性 run_at + 周期 interval_secs（最小 10s），**无 cron 表达式**；SchedulePage 有列表无创建 UI（创建走 LLM 工具）。
- **LLM 轨**：多 provider 管理 + 模型档位制（主会话档/记录员钉档/子代理钉档）+ models.dev 目录快照 + 本地导入 + 代理设置 + 推理档位。
- **缺口速览**：无 web 搜索（仅 fetch）、无终端、无 cron、无远程市场源、无 headless、无消息级 rewind UI、slash 仅 1 条、fork 无 UI、无远控/Bots/分享/远程工作区。

---

## §3 差距总矩阵

定级：**≈** 已有（形态可不同）｜**◐** 部分有（需补齐或强化）｜**✗** 缺失。归宿列见 §4 原则。

| # | 功能域 | ZCode 形态 | Locowrk 现状 | 定级 | 归宿建议 |
|---|---|---|---|---|---|
| 1 | 内置斜杠命令 | 21 条内置 + 保留名 | SlashPanel 1 条 | ✗ | 前端注册表 + 组装点命令 |
| 2 | 自定义命令（md） | 目录发现 + $ARGUMENTS | 无 | ✗ | 宿主薄层发现 + 前端展开 |
| 3 | Skills 技能 | SKILL.md 渐进披露三级覆盖 | 无 | ✗ | 注入中间件 + 指令文件家族 |
| 4 | Hooks 钩子 | 7 事件 + 信任模型 | 无（有洋葱中间件地基） | ✗ | kernel-agent 中间件配置化 |
| 5 | MCP 客户端 | 3 transport + OAuth + scope | tool-mcp 插件（动态注册） | ◐ | 插件强化 + 设置页列表 |
| 6 | 资源包式插件 | manifest 打包 skills/commands/MCP/hooks/UI | ABI 动态库插件（工具运行时） | ≈/异构 | 不对齐形态，见 §3 备注 |
| 7 | 外部导入 | 16 家竞品四类资源 | 无 | ✗ | 设置页导入器 |
| 8 | 子智能体 profile | md profile + 后台唤醒 + side tabs | subagent_run prompt 式 | ◐ | profile 文件 + 注入 + 唤醒 |
| 9 | 内置终端 | node-pty + xterm 多 tab 保活 | 无 | ✗ | src-tauri PTY + 前端 xterm |
| 10 | 后台 bash | 120s 自动移交 + 输出面板 | shell 同步等待 | ✗ | tool-shell 扩展 |
| 11 | Git 审查面板 | 轻量审阅 + graph + checkpoint | WorkPanel review tab 雏形 | ◐ | 前端强化 + tool-git 补工具 |
| 12 | 文件级 checkpoint/rewind | checkpoint 服务 + safe rewind | 会话游标回退（无文件快照） | ✗ | 快照层（git 优先） |
| 13 | 文件阅读器 | Markdown/PDF/DOCX/XLSX 只读 | lwpreview（HTML 预览）+ ImageViewer | ◐ | lwpreview 扩展 md/pdf |
| 14 | 分屏工作台 | 二叉分割树 ≤4 pane | 单会话单窗 | ✗ | 前端 pane tree（大） |
| 15 | 会话内查找 | Cmd+F 数据级索引 | 无 | ✗ | 前端 |
| 16 | 白板 | 右侧 pane 画板 + 导出附件 | 无 | ✗ | 前端（低优先，可不做） |
| 17 | 轨迹查看器 | model-io JSONL 可视化 | audit.jsonl 已落盘（无查看器） | ✗ | 前端查看器（读现成日志） |
| 18 | 上下文用量/缓存命中 | snapshot 事实源 + 阈值显隐 | 四件套组件已有 | ≈ | 维持 |
| 19 | 任务看板/跨会话搜索 | SQLite 索引 + Cmd+K | CommandPalette + session_search | ◐ | 前端接入既有搜索 |
| 20 | 会话上下文工具 | #sess_* 读取其他会话 | 无 | ✗ | 内置工具（ReadSessionContext 对位） |
| 21 | 会话空闲回收 | TTL + LRU | 无 | ✗ | 挂账（单进程桌面收益存疑） |
| 22 | 移动端远控 | relay + replayable + Android 壳 | 无 | ✗ | 局域网 ws 直连先行 |
| 23 | Bots 渠道 | Telegram/微信/飞书 | 无 | ✗ | 宿主渠道 runtime |
| 24 | 会话分享 | 上传 + Web 落地页 + import | markdown 导出 | ◐ | 独立 HTML 导出（无服务端变体） |
| 25 | 远程工作区 | SSH/WSL/Docker/Server | 无 | ✗ | 远期独立立项（§5.12） |
| 26 | headless CLI | -p 非交互 + resume + stream-json | 无（demo 叙事） | ✗ | kernel-bin run 子命令 |
| 27 | 定时自动化 | cron + 自定义重复 + 台账 + 上限 | run_at + interval | ◐ | engine 加 cron + 台账 UI |
| 28 | 闲时任务 | 服务端空闲池取号 | 无 | ✗ | 不对齐云形态，见 §6 |
| 29 | goal 目标延续 | /goal + 一等公民 + 互斥 plan | 无 | ✗ | kernel-agent 会话字段 + 注入 |
| 30 | 输入 steer/队列编排 | CommandInbox + steer + 编辑 | QueueCard 有、无 steer | ◐ | 与对话流设计稿 C1 合流 |
| 31 | 附件（图/视/PDF） | 40MiB 管线 + 能力投影 | 无（多模态轨已有） | ✗ | 前端 + 请求组装 |
| 32 | prompt enhance | 润色回填 | 无 | ✗ | LLM 轨小功能 |
| 33 | Web/图片搜索 | websearch + 域名 allowlist + image-search | 无（仅 fetch） | ✗ | 新插件 tool-search |
| 34 | Office 文档生产 | 四内容插件 + visual-judge | 无 | ✗ | 未来内容插件（挂账） |
| 35 | 计划模式 | 独立布尔 + 批准三态 + 计划文件 | Pure 工具目录围栏（tool_source.rs:84） | ◐ | 补计划文件持久化 + /plan 命令 |
| 36 | 自动更新/托盘/快捷键设置/deep link/存储管理/用量统计/反馈导出 | 全有 | 托盘✓ 其余✗ | ✗ | §5.15 平台杂项束 |
| 37 | i18n / 主题 / 上下文用量 UI | 有 | 有 | ≈ | 维持 |

> **#6 备注（重要，防误读）**：ZCode 的「插件」= 声明式资源包（skills/commands/MCP/hooks/UI 打包分发）；Locowrk 的「插件」= 冻结 ABI 的动态库工具运行时（dlopen、多版本、永不 dlclose）。两者不在同一层：Locowrk 缺的不是 ABI 插件的替代，而是上面缺一个**资源包发现层**（skills/commands/agents 的目录约定）。本稿不为「资源包」再造插件系统，而是让资源目录约定直接挂在数据根与工作区（§5.1–§5.3、§5.8），市场只继续管 ABI 插件。若未来要「一键装一包」，届时再评估在市场 manifest 里允许捆绑资源目录（挂账，不设计）。

---

## §4 归宿原则（Locowrk 架构图式落法）

每个缺口先问「放哪一层」，遵守既有裁定：

1. **高危执行面 → 宿主内置**：PTY 终端、hooks 子进程、Bots 渠道长连接、远控 ws 监听、checkpoint 写入——都不进插件（v2.145 裁定：不隔离 fs/网络即非安全边界；这些面权限太大，塞进无特权插件等于谎报威胁模型）。
2. **纯工具面 → 插件**：web 搜索、MCP 客户端强化、后台 shell 语义——继续走 ABI 插件 + manifest `side_effect` 如实标注（七序决策表管辖）。
3. **纯发现与注入 → 注入中间件家族**：skills 列表注入、AGENTS.md/命令帮助——挂 waterfall 现有顺序（ToolExposure → BaseSystemInject → MemoryInject → … → InstructionFilesInject → Fragment），新增件不改变相对次序，字节钉死保前缀缓存。
4. **纯前端 → desktop/src**：斜杠命令面板、查找、分屏、轨迹查看器、存储/用量页——Tauri command 薄适配，不写业务。
5. **一切资源目录约定统一走「双层作用域」**：用户级 `<data_root>/`（默认 `~/.Locowrk`，env `LOCOWRK_DATA_DIR` 可改）+ 项目级 `<repo>/.Locowrk/`——与记忆双层（Memory Redesign §3）、repo 交接文件（L-R 层）同构，**不引入第三套目录习惯**（不抄 `.zcode/`/`.agents/` 双目录合并那套复杂度）。

---

## §5 逐域设计

每域给：现状 → 落法 → 依赖/规模（S=小批 M=中批 L=大工程）。规模只描述工作量级，**不排期**。

### 5.1 内置斜杠命令 + 自定义命令（#1 #2）

- **落法**：三层命令源。①内置命令注册表放前端 `lib/slash-commands.ts`（现成架子扩展），每条 = `{name, aliases, description, kind}`，kind 决定执行路径：`command`（映射既有 Tauri command：session_new/session_set_model/truncate_after/export_markdown…）、`prompt`（展开为改写后 prompt 发给模型，如 /init 生成 AGENTS.md）、`panel`（打开本地 picker：mode/model/rewind）。首批内置候选（全部对应已有后端能力，纯接线）：`/help /new /model /mode /compact /export /schedule /memory /rewind /init /plugins /mcp /locale`。②自定义命令：`<repo>/.Locowrk/commands/**/*.md` + `<data_root>/commands/**/*.md`，frontmatter 收 `description/allowed-tools/argument-hint`，相对路径 = 命名空间，`$ARGUMENTS`/`$1..$n` 展开；发现逻辑放 src-tauri 一个薄模块（fs 扫描 + frontmatter 解析），Tauri command `commands_list` 供 SlashPanel 补全。③保留名 = 全部内置名 + 别名，自定义同名被过滤。
- **不做**：`` !`cmd` `` shell 动态展开（ZCode 也明确不支持）；插件贡献命令。
- **依赖**：无（全部已有能力接线）。**规模 S–M**。

### 5.2 Skills 技能（#3）

- **落法**：`<repo>/.Locowrk/skills/<name>/SKILL.md` + `<data_root>/skills/<name>/SKILL.md`（项目覆盖用户同名）。frontmatter 只收 `name/description[/version]`。注入 = 新中间件 `SkillInject`：会话首请求算毕、只注 name+description 一行索引表（字节钉死、钳 ~2KB），挂 InstructionFilesInject 邻位。命中激活 = prompt 前置指令「先读 `<skill 目录>/SKILL.md` 全文并遵循」（模型用既有 fs_read 即可，**零新增执行面**）。用户触发 = `/skill <name> <task>` 命令（§5.1）改写下一条 prompt；模型语义自配由索引表 + description 完成。
- **不做**：技能市场（分发 = 目录 + git）；激活前用户确认点（首批）；`~/.agents/skills` 双目录兼容。
- **依赖**：§5.1（/skill 命令）。**规模 S–M**。

### 5.3 Hooks 钩子（#4）

- **落法**：事件对位 kernel-agent 洋葱中间件——`BeforeToolCall=PreToolUse`、`AfterToolCall=PostToolUse`、`TurnEnd=Stop`、`SessionStart`、`UserPromptSubmit`。hook 声明存 `<repo>/.Locowrk/hooks.json` + `<data_root>/hooks.json`（user 先跑必执行；workspace 需信任）。执行器 = src-tauri 宿主内置 hook runner（spawn argv，stdin 单行 JSON，超时杀进程，永不走 shell 字符串拼接）。输出契约对齐 ZCode：exit 2 = 阻断工具调用、stdout JSON `updatedInput`（重过工具 schema）/`additionalContext`（注入 Fragment 位）/Stop block 续跑上限 3 次。
- **安全（关键）**：hooks 能执行任意本地命令 = 最高危面。①workspace hooks 默认**不信任**：Trust = `workspaceIdentity + hooks.json 内容 digest` 持久化，digest 变更即失效重审（对齐 ZCode fail-closed）；②信任审查 UI 进设置页新分区；③hook 触发的子进程不走审批卡（用户已信任），但 hook **改参**（updatedInput）必须走既有审批表重判——七序决策表对改参后的调用重新过门。
- **依赖**：kernel-agent 中间件 trait 稳定（已在）。**规模 M–L**。

### 5.4 内置终端 + 后台 bash（#9 #10）

- **终端**：src-tauri 加 `portable-pty` 依赖起真 PTY，前端 xterm.js 新依赖；承载 = WorkPanel 新增 terminal 页签（多实例 tab，跨会话保活 = PTY 会话表存 src-tauri 全局态，关闭 tab 才杀）。resize/主题跟随 --lw-* 令牌。终端是**用户手操面**，不经审批、不进工具池；agent 执行命令仍收敛 tool-shell 单一入口（v2.145 裁定不破）。
- **后台 bash**：tool-shell 扩展——前台等待超阈值（默认 120s，开发者分区可调）且命令可后台时，子进程不杀、移交后台任务表（返回 background_id + 输出文件路径），模型后续用 `shell_read(background_id)` 轮询尾窗（8KiB 钳、5GiB 软上限杀树）；输出文件落 `<data_root>/shell-logs/<id>.log`；`side_effect` 语义不变（进程已审批放行，后台化只是等待方式变化，**不构成绕过审批的新入口**——后台移交只发生在已放行的调用上）。
- **依赖**：终端无；后台 bash 无。**规模：终端 M，后台 M**。

### 5.5 Git 审查强化 + 文件级 checkpoint/rewind（#11 #12）

- **审查面板**：WorkPanel review tab 现有 diff 审阅强化为「本轮/本会话改动文件清单（+A −D 统计）→ 点开 diff → 单文件 restore 按钮（走 tool-git 新增 `git_restore_file` 或 fs_delete+fs_restore 既有补偿）」；git graph 只读弹窗挂账。
- **checkpoint**：Locowrk 已有会话游标回退但无文件快照。落法 = 快照层薄件：每 turn 收束时，若本 turn 有文件改动（ToolCall 事件里 fs_write/edit/git_commit 的目标文件），对这些文件做 checkpoint——**git 仓库内优先 `git stash create`/临时 commit 语义**（tool-git 扩展 `git_checkpoint_create/restore`），非 git 目录退化为 shadow copy 到 `<data_root>/checkpoints/<session_id>/<seq>/`。UI = 改动清单面板「撤销本轮文件改动」：确认弹窗 + 逐文件可/否安全撤销标注（bash 改动不可标）+ **all-or-nothing 预检**（有不安全项即整批禁用，对齐 ZCode 语义）。
- **不做**：消息级 cascade rewind（ZCode 自己已下线；Locowrk 用 editUserQuery 对位物挂账）。
- **依赖**：checkpoint 依赖 §5.11 的 FileChanges 投影（对话流设计稿 C1）知道「本 turn 改了哪些文件」。**规模：审查 S，checkpoint M**。

### 5.6 文件阅读器（Markdown/PDF/Office）（#13）

- **落法**：lwpreview:// 协议已限定 fsRoots + 20MB 钳（安全模型现成）。扩展三步：①WorkPanel 加「文件」页签 + 产物卡片「在右栏打开」；②Markdown 渲染复用前端 `lib/markdown.ts`（只读、受《UI Design System》字阶约束）；③PDF 用 pdf_to_images 现成内核能力出页图。DOCX/XLSX 只读 = 远期（引 mammoth/sheetjs 类前端依赖，单独批）。目录树浏览挂账。
- **依赖**：无。**规模：md+pdf S–M，office M**。

### 5.7 MCP 强化（#5）

- **落法**：tool-mcp 插件已能动态注册远端工具；补①http/SSE transport（现在若只有 stdio）；②设置页 MCP 列表（scope/连接状态/工具数/失败摘要，对齐 ZCode 诊断形态）；③工具名命名空间化防覆盖（`mcp__<server>__<tool>`）。OAuth 挂账（需要本地 callback server，M 级）。
- **依赖**：无。**规模 M**。

### 5.8 子代理 profile 化（#8）

- **落法**：`.Locowrk/agents/*.md` + `<data_root>/agents/*.md` frontmatter（`name/description/tools/model/thinking`），项目覆盖用户；内置保留名 `general-purpose`（缺省）+ `explore`。subagent_run 加 `agent_type` 参数按 profile 解析（现 prompt/白名单/max_turns 参数保留为 inline 覆盖）。目录注入 = agent_listing 一行索引进 SkillInject 同件（钳预算）。**后台唤醒**：后台完成注入 synthetic user input 触发一轮 model-only turn——kernel-agent turn 循环要支持非用户来源输入，中风险，独立小批做；首批先做「后台运行 + 完成通知横幅」。
- **不做**：child 会话 side tabs 查看（依赖会话投影 C1，挂账）；SendMessage 双向通信（现有一次性 run 语义保留）。
- **依赖**：profile 解析无依赖；唤醒依赖 turn 循环改造。**规模：profile S，后台唤醒 M**。

### 5.9 headless CLI（#26）

- **落法**：kernel-bin 加 `run -p "<prompt>" [--cwd <dir>] [--resume <id>] [--mode] [--output-format text|json]`：非交互单 prompt → 组装最小内核（kernel-agent + plugin-service + session-store + 内置工具子集）→ 跑完落 session（复用 session-store 回放支持 `-c/--resume`）。`--output-format stream-json`（逐事件 JSON 行）评测外围可用。会话存储与桌面共用 data_root，**同一份会话两端可见**（这是对 ZCode app-server 形态的单进程替代）。
- **诚实边界**：kernel-bin 组装点与 desktop/src-tauri 组装点是两套（desktop 被 exclude 在 workspace 外），headless 首批只保证核心工具面（fs/shell/edit/fetch/memory），desktop 特有内置（ask_user 卡片、浏览器、审批卡）在 headless 退化为自动拒绝/计划档硬拒——记档不隐瞒。
- **依赖**：无硬依赖。**规模 M**。

### 5.10 自动化升级（#27）

- **落法**：①scheduler engine 增加 cron 字段（Rust `cron` crate 解析 + 本地时区），与既有 run_at/interval 三态并存，`scheduleRule` 权威；②全局上限默认 20 条（可配置原则，进开发者分区）；③运行台账 `schedule_runs` 表 + SchedulePage 展示历史（触发时间/状态/耗时）+ 「立即运行」；④会话消息来源徽标「由定时任务发送」（session 事件加 origin 字段）。闲时任务不对齐云形态（§6），可选变体「低峰窗口」挂账。
- **依赖**：无。**规模 M**。

### 5.11 触达：远控 / Bots / 分享（#22 #23 #24）

- **远控（分两程）**：第一程 = **局域网直连**——桌面 src-tauri 起 ws server（默认监听局域网接口，随机端口 + 访问 token），设置页「远程控制」分区生成二维码（`http://<ip>:<port>/remote?token=…`，静态页由桌面端内嵌 serve）；手机浏览器直开。能力对齐 replayable 精简版：看任务列表/打开会话/发消息/审批与提问交互/停止；投影语义**直接复用对话流设计稿 C1 的 TurnHeader/ToolStatus 行投影**（这是硬依赖——没有行级投影就没有远控）。快照预算 200KB。第二程 = 外网中继（需要自建 relay 服务）+ Android 壳：**不做**（无服务端设施，挂账待用户立项）。
- **Bots**：宿主内置渠道 runtime（src-tauri 后台任务）：首批 Telegram 长轮询（最简、无回调地址要求）；私聊绑定会话 + `/new /status /model /mode` 命令 + 审批以消息按钮回传（映射 resolveInteraction 语义）。微信/飞书挂账（飞书要 streaming card 协议，微信 iLink 协议不公开）。凭据存本地设置（加密可选），**绝不写进仓库文件**（AGENTS 纪律）。
- **分享**：无服务端变体——「导出为独立 HTML」：会话 jsonl → 只读静态页（内联样式、受《UI Design System》约束、零外部依赖），存 `<data_root>/exports/` 并可打开；markdown 导出已有保留。上传式分享与 import deep link 挂账（同远控第二程）。
- **依赖**：远控硬依赖对话流 C1；Bots 依赖审批交互的投影。**规模：远控一程 L，Bots-Telegram M，HTML 导出 S–M**。

### 5.12 远程工作区（#25）

- **定级：远期独立立项，本稿不展开设计。** 诚实理由：远程工作区 = 第二执行 host，Locowrk 的 ToolCtx/插件装载/fsRoots 全部绑定本机进程，「把 runtime 搬到 SSH 远端」是内核级改造（对位 ZCode 远端部署 zcode-server 的整层），不是功能补丁。若立项，起点是把 §5.9 headless run 跑通后经 ssh 远程执行（粗粒度变体），细粒度（远端 PTY/远端 fsRoots）再议。

### 5.13 对话输入编排：goal / steer / 附件 / enhance（#29–#32）

- **goal**：session 加 goal 字段（session-store 事件）+ 注入 reminder（MemoryInject 邻位）+ `/goal` 命令 + 不完整 goal 阻断会话自动收尾。规模 S–M。
- **steer（运行中插话）**：kernel-agent turn 循环 mid-turn input 检查点；UI 复用 QueueCard 语义加「插话」态。与对话流设计稿 C1 同批合流（其 §2 已声明不做 inputStreaming，本域是它的姊妹域）。规模 M–L。
- **附件**：composer 支持图片/PDF 附加 → 多模态轨（LLM.md 已有）+ pdf_to_images 已有内核能力，主要是前端上传/预览 + 请求组装 + 不支持模型时的占位投影。规模 M。
- **prompt enhance**：LLM 轨小功能（草稿交当前 provider 润色回填，可关）。规模 S。
- **会话上下文工具**（#20）：内置只读工具 `session_context(session_id, query)` 读其他持久会话（session-store 已有回放），relevant/handoff 两策略，`#sess_*` UI 语法进 composer。规模 S–M。

### 5.14 工作台布局：分屏 / 查找 / 轨迹 / 看板（#14 #15 #17 #19）

- **分屏**：前端 pane tree（二叉分割 ≤4 pane、pane 绑定 workspace+session、跨会话 tab 常驻）。前端大工程，独立立项；依赖对话流 C1 的会话投影独立性。
- **会话内查找**：Ctrl+F 对话范围（user/assistant 文本、大小写不敏感、数据级索引兼容虚拟滚动、命中高亮）。规模 S–M。
- **轨迹查看器**：audit.jsonl 已在落盘（982KB 实证）——只读查看器（按 turn 分组：模型请求 token/耗时/finishReason、工具出入参、错误），WorkPanel 新 tab。**零新落盘**，纯读。规模 S–M。
- **任务看板/跨会话搜索**：session_search 后端已有 → 接入 CommandPalette 升级为命令中心（命令 + 会话 + 文件三源）。规模 S–M。
- **白板**：挂账（低优先，用户未点名；ZCode 也是边缘功能）。
- **会话空闲回收**：挂账（单进程桌面内存压力场景与 ZCode 多 session 服务形态不同，收益存疑；若做只回收非活动会话的内存投影）。

### 5.15 平台杂项束（#36）

单点都小，可按需拆并：①自动更新（tauri updater 标准）;②快捷键设置页（现 5 个硬编码键进设置）；③存储管理页（data_root + tests 产物占用统计与清理——v2.154 实测 `target/locowrk-test/` 1.2GB 增长，此页有真实用处）；④本地用量统计（audit.jsonl/session 事件聚合：日历热力图、token 累计）；⑤反馈/日志导出（日志脱敏 zip，脱敏规则显式白名单）；⑥deep link（tauri deep-link 插件，`locowrk://workspace/open`，配资源管理器右键说明文档）；⑦数据目录设置页暴露（env 已支持，补 UI + 迁移确认弹窗）；⑧外部导入器（扫描 `~/.claude/skills`、`~/.claude/commands`、`~/.claude/agents` 等常见竞品目录 → 复制进 `<data_root>/` 对应目录，同名跳过；MCP 配置扫描挂账）——对同时用 Claude Code 的用户迁移成本低、收益直接。**规模：每项 S，共约一批半。**

### 5.16 Web 搜索与图片搜索（#33）

- **落法**：新插件 tool-search（`side_effect` 如实标 Compensable/Pure）；provider 可配置原则——支持 Tavily/Bing/SearXNG 自建（API key 进设置不进仓库）；域名 allowlist/blocklist 进 policy 层；结果摘要进上下文带来源 URL。图片搜索挂账（依赖可用免费源）。
- **依赖**：无。**规模 S–M**。

### 5.17 计划模式补正（#35）

- 现状已有 Pure 工具目录围栏（计划档硬拒在七序表首位）。补：①计划文件持久化（批准后 `<repo>/.Locowrk/plans/plan-<session_id>.md` 原子写入 + compact 后注入全文 reminder，对齐 ZCode 连续性语义）；②`/plan` 命令（§5.1）；③批准弹窗三态中的「拒绝 + 修改意见转真实 user message」（防模型自循环重提）。
- **依赖**：无。**规模 S–M**。

---

## §6 明确不对齐清单（记档防返工）

| 项 | 理由 |
|---|---|
| 登录 / OAuth / 套餐 / 支付 / Coding Plan / Team Plan | 用户 2026-10-11 明确裁定「暂时不需要」 |
| ARMS / RUM / 产品埋点双通道 | Locowrk 隐私立场：本地优先，audit.jsonl 已够排查用 |
| 官方云市场 + CDN 分片 + 16 家市场源注册 | Locowrk 市场定位本地/自管；可加 git 源挂账 |
| 三层运行时拆分（zcode-protocol stdio 服务、app-server） | Locowrk 单进程 Tauri 形态成立；远控走轻量 ws 投影不搬协议层 |
| 资源包式插件系统 | §3 #6 备注：目录约定已覆盖 90% 价值，不造第二插件系统 |
| 闲时任务云端取号/免费池 | 无服务端；可选「低峰窗口」本地变体挂账 |
| 外网中继远控 + Android 壳 + 上传式分享 | 无服务端设施，待用户立项 |
| 远程工作区（SSH/WSL/Docker） | §5.12：内核级改造，远期独立立项 |
| Computer Use（CUA） | ZCode 自己 macOS/Windows beta + kill switch；Locowrk 无 Helper 进程/权限主体地基，挂账 |
| Highspeed 加速卡 / 营销触达 / What's New 运营位 | 营销面，非功能面 |
| Web App Docker 形态 / ZCode Lite | 单机桌面定位不覆盖；远控一程天然覆盖「浏览器访问」主诉求 |
| `~/.agents/` 双目录家族兼容 | 只认 `.Locowrk/` 单一约定（§4.5）；外部导入器负责桥接 |

---

## §7 候选批次路线（供排序，不含时序承诺）

分七群。群内条目相对独立；跨群依赖用 ← 标注。**先后次序用户另行安排**；全部排在审计主线（AUDIT-FIX-PLAN-v1）收口之后，与记忆批 M1–M3、对话流批 C1–C4 共用同一「审计后队列」。

- **群 A 扩展生态**：A1 斜杠命令系统 + 自定义命令（§5.1，S–M）｜A2 Skills（§5.2 ←A1，S–M）｜A3 Hooks（§5.3，M–L）｜A4 MCP 强化（§5.7，M）｜A5 外部导入器（§5.15⑧，S–M）｜A6 子代理 profile（§5.8，S + 唤醒 M）
- **群 B 工作台**：B1 内置终端（§5.4，M）｜B2 后台 bash（§5.4，M）｜B3 文件阅读器 md+pdf（§5.6，S–M）｜B4 Git 审查强化（§5.5，S）｜B5 文件级 checkpoint/rewind（§5.5 ←对话流 C1 的 FileChanges，M）｜B6 会话内查找（§5.14，S–M）｜B7 轨迹查看器（§5.14，S–M）｜B8 分屏（§5.14 ←C1，L）｜B9 office 预览（§5.6，M）
- **群 C 对话运行时**：C1' 输入编排与 steer（§5.13 ←对话流 C1，M–L）｜C2 附件（§5.13，M）｜C3 goal（§5.13，S–M）｜C4 prompt enhance（§5.13，S）｜C5 会话上下文工具（§5.13，S–M）｜C6 计划模式补正（§5.17，S–M）｜C7 命令中心（§5.14，S–M）
- **群 D 自动化**：D1 cron + 台账 + 来源徽标（§5.10，M）
- **群 E 触达**：E1 headless CLI（§5.9，M）｜E2 HTML 会话导出（§5.11，S–M）｜E3 局域网远控（§5.11 ←对话流 C1，L）｜E4 Bots-Telegram（§5.11 ←E3 的交互投影经验，M）
- **群 F 工具面**：F1 tool-search（§5.16，S–M）
- **群 G 平台束**：G1 更新/快捷键/存储管理/用量/反馈导出/deep link/数据目录（§5.15①–⑦，一批半）

**依赖要点**：对话流设计稿的 **C1（数据链路投影）是最大汇聚点**——B5 checkpoint、B8 分屏、C1' steer、E3 远控全都踩它；建议无论最终怎么排序，C1 宜早。A 群全部不依赖别人，随时可插。E3 是最大的单体工程，做之前 E1/E2 可以先低成本验证会话投影的可消费性。

---

## 附录 A ZCode 证据索引（按域）

- 命令：`packages/shared/src/zcode-slash-command-help.ts`（21 条内置）；`apps/zcode-cli/packages/bootstrap/src/slash-command-surface.ts:16`；`apps/zcode-cli/packages/adapters/src/commands/index.ts:26-33`、`roots.ts:94-105`；`cli/src/command-center/slash-commands.ts:40`；`cli/src/custom-command-expand.ts`；`run.ts:553-602`（CLI dispatch）；docs/settings-command-list.md、plan-slash-command.md、zcode-init-agents-md-command.md、external-agent-commands-import.md
- 计划模式：docs/plan-mode-independent-state.md、plan-mode-approval-boundary.md、plan-mode-goal-command-guard.md、plan-mode-plan-file-compact-continuity.md
- 输入编排：docs/conversation-protocol-declaration.md:206-450（CommandInbox/steer/队列）、prompt-enhance.md、task-search.md、compact-media-capability-projection.md、remote-prompt-attachment-materialization.md
- Skills：docs/skills-product.md、zcode-guide-plugin-skill-discovery.md、settings-skill-list.md；`adapters/src/skills/roots.ts:9-34`
- Plugins：docs/plugin-marketplace-ui-ux.md、plugin-creator.md、plugin-hooks-compat.md、plugin-capabilities-settings.md、plugin-cdn-zip-source.md、plugin-github-archive-source.md、claude-code-marketplace-plugin-compat.md、UI_PLUGIN.md、docs/plugins/workspace-level-plugin-architecture.md
- Hooks：docs/hooks-runtime-contract.md、hooks-implementation-analysis.md、settings-hook-list.md、workspace-hook-trust-user-guide.md、workspace-hook-trust-mr-scope.md
- MCP：docs/mcp-platform-boundary.md、settings-mcp-list.md、mcp-config-compat-diagnostics.md、mcp-oauth-client-auth.md、mcp-oauth-two-phase.md、mcp-dual-era-version-negotiation.md
- 子代理：docs/subagent-compatibility.md、subagent-listing-attachment.md、subagent-respond-to-coordinator.md、subagent-runtime-refresh.md、subagent-session-directory.md、subagent-session-side-tabs.md、subagent-timeout-policy.md、subagents-built-in-model-overrides.md、settings-subagent-list.md；`bootstrap/src/subagents.ts:55`
- 外部导入：docs/external-agent-{skills,commands,plugins,mcp}-import.md、onboarding-external-agent-import-steps.md
- 终端：`packages/services/src/terminal/terminalService.ts`（node-pty）、docs/ui/side-pane-terminal-tabs.md、side-pane-terminal-session-keepalive.md、bash-background-parity.md、background-bash-output-details.md
- 代码/Git：docs/code-viewer-diffs.md、docs/git/requirements.md、docs/git/architecture.md、docs/git/git-graph.md、docs/git/checkpoint-service-foundation.md、file-summary-safe-rewind.md、rewind-button-any-message.md、git-review-file-context-menu.md
- Markdown/媒体：docs/assistant-message-markdown-type-scale.md、markdown-table-scroll-mode-toggle.md、streaming-markdown-reveal.md、image-preview-pointer-interaction.md
- Office/CUAB：docs/product-capability-map-office-computer-use.md、document-skills-plugin.md、docs/cua/windows-product-runtime.md、docs/cua-permission-broker/zcode-cua-permission-broker.md
- 浏览器：docs/browser-use/2026-07-08-webview-cdp-on-guest-pivot-spec.md、2026-07-12-browser-tab-selection-activation-spec.md、2026-07-31-browser-tab-residency-budget-spec.md、2026-07-15-browser-turn-final-screenshot-spec.md、desktop-html-preview-open.md
- 工作台：docs/v4-split-pane-workbench.md、v4-session-workbench-groups.md、v4-session-find-highlight.md、whiteboard.md、model-trajectory-viewer.md、feature-boundary-graph-visualization.md
- 上下文/任务：docs/context-window-usage-state.md、context-cache-hit-rate.md、docs/task/task-sqlite-index.md、task-list-refresh-semantics.md、docs/task/pinned-task-workspace-scope.md、session-context-tool.md、session-idle-deactivation.md
- 远控：docs/web-remote-control/web-remote-control-architecture.md、task-realtime-sync.md、web-remote-control-task-command-queue.md、single-owner-stream-rendering.md、snapshot-byte-budget.md、mobile-task-home-design.md、mobile-theme-control.md、mobile-remote-workspace-reconnect.md、docs/mobile-remote-control-android.md、mobile-stability-analysis.md
- 远程工作区：docs/wsl-remote-workspace-design.md、remote-workspace-session-in-current-window.md、ssh-remote-app-global-state-authority.md、docs/remote/server-remote-workspace-connection.md、docs/remote/provider-provisioning.md
- 分享：docs/conversation-share-v1.md、web-share.md、highspeed-share-client-generated.md
- Bots：docs/bots.md、bot-channel-runtime-split.md、bots-private-task-sync.md、bots-feishu-group-collaboration.md、bots-feishu-streaming-card-reliability.md
- 自动化：docs/ui/scheduled-tasks-main-view.md、automation-create-limit.md、automation-edit-status.md、automation-session-hourly-interval.md、docs/off-peak-task/spec.md
- 平台：docs/windows-tray.md、docs/desktop/shortcut-settings-spec.md、auto-update-baseline.md、feedback-simplified-flow.md、feedback-log-export-alignment.md、local-crash-capture.md、configurable-data-directory.md、main-window-size.md、finder-open-folder-deep-link.md、settings-storage-management.md、usage-stats-app-coding-plan-split.md、help-menu-whats-new.md、docs/desktop/log-export-retention.md
- 总图：docs/product-capability-map.md（三层边界与能力群权威）

## 附录 B Locowrk 现状锚点

- 插件 manifest：`plugins-src/{memory-sqlite,tool-echo,tool-edit,tool-fetch,tool-fs,tool-git,tool-mcp,tool-notify,tool-shell,tool-shout}/manifest.json`
- 内置工具注册：`desktop/src-tauri/src/tool_source.rs:44,98,147`；ask_user `src/ask.rs:229`；search_tools `src/search_tools.rs:37`；浏览器 `src/browser/tools.rs:51,214` + `src/browser/preview.rs:5,30`；树工具 `src/organize/tree_tools.rs:44,204`；search_titles `src/organize/search_titles.rs:43`；project_history `src/organize/project_history.rs:37`；调度 `src/scheduler/tools.rs:47,171,229` + `engine.rs:113`；subagent_verify `src/subagent_verify.rs:40`；媒体 `src/media/mod.rs:70,182` + `src/media/pdf.rs:64`
- 子代理：`crates/kernel-agent/src/subagent/tool.rs:74-85`、`subagent/filter.rs:16`
- 审批：`crates/kernel-agent/src/approval.rs:15-40`（中间件 trait；七序决策表见 DESIGN §7.2）
- 会话操作：`crates/session-store/src/session.rs:314,344,366-382`；桌面命令 `desktop/src-tauri/src/lib.rs:167`；导出 `src/export.rs:31`
- 前端：SlashPanel `desktop/src/lib/slash-commands.ts:18`；WorkPanel `desktop/src/stores/work-panel.ts:20` + `components/shell/WorkPanel.vue:18-20`；设置分区 `components/settings/SettingsPage.vue:33-45`；快捷键 `shell/use-global-shortcuts.ts:27-58`；主题 `lib/theme.ts:6`；markdown `lib/markdown.ts` + 复制 `lib/use-copy.ts:29,42`
- CLI：`crates/kernel-bin/src/cli.rs:9-28`
- 市场：`crates/plugin-service/src/market.rs:25,46`、`service.rs:36-275`；桌面 PluginMarket UI
- 数据根：`crates/plugin-api/src/fsutil.rs`（data_root = env LOCOWRK_DATA_DIR else ~/.Locowrk）
