# 仓库蓝图（STRUCTURE.md）

> **本仓库唯一的地图**，不要另建一份。规则在 [`AGENTS.md`](AGENTS.md)，本文件只回答"这个文件是干什么的、能不能动"。
> **新增 / 移动 / 删除文件后同步更新本文件** —— 一张说谎的地图比没有地图更糟。

三个标记：

- `★` **唯一事实源**：改这里，其他地方只引用、不复制
- `[LOCK]` **禁区**：改动前必须向人说明并获批准
- `⊘` **不入库**：`.gitignore` 排除了、永不推送远端 ⇒ 别人 clone 后这些路径是**空的**。
  在役文档里指向它们的链接必须带「不入库」字样（`check-docs.mjs` 的 R10 判，名单唯一出处 = 直接问 `git check-ignore`）

## 整仓形状

```
Locowrk/                    可热插拔、可回退的 Rust Agent 内核 + Tauri 2 桌面端（单人自用内测）
│
├── AGENTS.md                        # ★ [LOCK] agent 入口：命令 / 硬规则 / 禁区 / 工作流（≤100 行硬闸，只写可判定规则）
├── STRUCTURE.md                     # ★        本文件
├── README.md                        #          给人看的入口：现在能用什么 + 三分钟跑起来 + 文档地图
├── CONTRIBUTING.md                  #          提交约定 + 一版变更写哪几处 + 加一道闸的四步规矩
├── Makefile                         #          命令入口薄封装（只转发到 scripts/tasks.sh，不抄清单）
├── Cargo.toml                       # [LOCK]   根 workspace：8 个 crate + plugins-src（desktop/src-tauri 被 exclude）
├── Cargo.lock                       # [LOCK]   由 cargo 更新、要入库，禁止手改
├── .gitignore                       # ★        不入库名单的唯一出处（构建产物 + ⊘ 那几层）
├── .gitattributes                   #          换行与文本属性
├── .githooks/pre-commit             # [LOCK]   提交档：按暂存面挑要跑哪几条闸（克隆后要手动启用 hooksPath）
├── .github/workflows/ci.yml         # [LOCK]   CI：只有一行 `sh scripts/tasks.sh gates`
│
├── crates/                          # ★        内核 8 个 crate。依赖只能单向往下流（完整图在 AGENTS.md §3，违反 = 设计错误）
│   ├── plugin-api/                  # [LOCK]   冻结 ABI（只许往结构体末尾加字段，改已有签名就是破坏兼容）· 工具与调用上下文的共用定义 · 记忆目录名的唯一算式
│   ├── cordis-core/                 #          事件总线底座：事件只往后追加 · 一个事件按注册顺序交给多个监听者 · 调度骨架
│   ├── plugin-host/                 #          把插件装进来再调出去：装了就不卸载 · 耗时调用不卡主循环
│   ├── plugin-service/              #          服务组合点：两条工具通路都经它进同一个工具池
│   ├── session-store/               #          会话持久化与重放：事件流 + 每步批量落盘
│   ├── kernel-agent/                #          Agent 循环（最大的一块）：审批 · 子代理 · 中间件 · 上下文注入 · 工具暴露
│   ├── memory-tree/                 #          记忆树：分层章节 · 整理 · 跨会话取回
│   └── kernel-bin/                  #          验收叙事入口：`demo`（热插拔与回退）· `demo4`（审批改写与工具视图隔离）
│
├── plugins-src/                     # ★        插件源码，与内核同一个 Cargo workspace；清单与版本真源 = scripts/build_plugins.sh 的 PLAN
│   ├── tool-fs/ tool-edit/ tool-shell/ tool-git/        # 文件与命令类（命令执行只此 shell 一个入口）
│   ├── tool-fetch/ tool-mcp/                                # 外部地址取回 · 外部工具协议
│   ├── tool-notify/ tool-echo/                              # 系统通知 · 双版本夹具（热插拔与回退的证据）
│   ├── memory-sqlite/                                       # memories 表的插件实现（与宿主 memory.rs 同源，check-schema-parity 盯着）
│   └── tool-shout/                                          # 市场验收夹具：只进 market/，不在开发态 PLAN 里
├── plugins/  market/                 # ⊘       开发态安装目录与本地市场：产物不入库，只有各自 README.md 入库
│
├── desktop/                          # ★       Tauri 2 + Vue 3 桌面端
│   ├── src/                          #         前端（界面永远不直接碰 PluginRegistry，只经 PluginService）
│   │   ├── App.vue  main.ts  i18n.ts  style.css          # 外壳 · 语言切换 · 字号基座 --lw-ui-font-size
│   │   ├── components/shell/ chat/ memory/ plugins/ settings/ schedule/ patterns/ lab/
│   │   │                                                 # 界面分区：外壳 · 对话流 · 记忆页 · 插件页 · 设置 · 定时 · 通用模式 · 演示夹具
│   │   ├── stores/                   #         pinia：状态与 IPC 编排的唯一去处（组件不许自己 apply）；最厚的是 chat.ts
│   │   ├── lib/                      #         api.ts（command 薄封装）· tool-detail.ts · use-popover.ts · ui-font.ts
│   │   ├── locales/                  # ★       zh-CN 与 en 双语包，键集必须相等（check-i18n 判）
│   │   ├── types/                    #         前端这边的类型声明：字段名要和后端返回的对上（漂一个字母 ⇒ 类型检查全绿而界面空白）
│   │   └── 最容易改坏的几处（超限点名名单里的前端件）：components/chat/Composer.vue · TranscriptFlow.vue ·
│   │     MessageRow.vue · PermissionCard.vue · BuiApprovalCard.vue，设置侧 ModelPanes.vue · LlmSection.vue ·
│   │     ProviderSetupDialog.vue
│   ├── src-tauri/                    #         组装点：独立 Cargo 包。形状约定 = 模块文件与同名目录成对（孤儿文件由拆分形状闸判）
│   │   ├── commands/ commands.rs     #         Tauri command 薄适配层：只做参数转换，不写业务（llm.rs · sessions.rs 最厚）
│   │   ├── runner/ llm/ prompts/ events.rs tool_source.rs
│   │   │                             #         执行循环 · 模型接入与线格式（client.rs · wire.rs）· 提示词分层 · 事件通道常量 · 工具接线
│   │   ├── approval/ permission_mode/ permission_rules/ privacy/ autodisable.rs
│   │   │                             #         审批与决策表（approval/decide.rs：固定七步的裁决序；approval/mod.rs：问人、挂起等待、豁免、守夜超时）· 四档权限 · 粘性授权规则 · 底线 · 自动禁用
│   │   ├── memory/ memory.rs memory_learn/ memory_files/ memory_index/ memory_reconcile/ memory_tree_reconcile/
│   │   │                             #         宿主侧记忆族 · 学习列 · 文件化 · 索引 · 两条对账（memory.rs 与插件那侧同源）
│   │   ├── settings/ state/ workspace.rs data_root.rs plugins_root.rs timeutil.rs
│   │   │                             #         设置页分区 · 全局状态 · 工作区解析 · 数据根 %USERPROFILE%\.Locowrk · 插件根 · 时间口径
│   │   ├── scheduler/ subagent_verify/ sub_transcript/ idle/ ask.rs stop.rs
│   │   │                             #         定时任务 · 子代理校验与转录 · 空闲态 · 追问卡 · 叫停
│   │   ├── instruction_files/ fragments/ inject_view/ repo_context/ search_tools/ text/ title/
│   │   │                             #         指令文件 · 片段 · 注入视图 · 工作区上下文 · 工具检索 · 正文与标题
│   │   ├── organize/ recall_log/ recall_receipt/ usage/ export/ session_log.rs thinking_language.rs model_override.rs
│   │   │                             #         记忆整理 · 取回日志与回执 · 用量 · 导出 · 会话日志 · 思考语言 · 逐轮改模型
│   │   ├── browser/ media/ assets/ notifications.rs desktop_notify.rs tray.rs
│   │   │                             #         工作面板内嵌浏览器 · 多模态 · 图标资源 · 托盘与系统通知
│   │   ├── lib.rs main.rs demo_llm.rs demo_tools.rs testutil/
│   │   │                             #         包入口（只放模块声明）· 界面内演示夹具 · 测试助手
│   │   └── tests/ tests_approval/ tests_session_log/ tests_hotpath.rs tests_permission.rs
│   │                                 #         tests_plugins_root.rs tests_stream.rs tests_tree_gate.rs —— 行为用例簇（#[cfg(test)] 一律独立文件）
│   └── tests/                        #         vitest 用例（happy-dom 环境；跑法见 AGENTS.md §2）
│
├── scripts/                          # ★       工程脚本与门禁实现：任务定义只在这里，Makefile 只转发
│   ├── tasks.sh                      # ★       任务与**闸清单的唯一出处**（t_gates 列全部闸，CI 调它）
│   ├── check_length.sh               #         单文件建议 ≤400 行：超限只许在点名名单里，新增超限即红
│   ├── check-i18n.mjs                #         中英键集闭合 + 引用面 + 豁免登记
│   ├── check-commands.mjs            #         Tauri command 三向对账：定义 / 注册 / 前端引用
│   ├── check-middleware-wiring.mjs   #         中间件监听器：实现数与接线数对齐
│   ├── check-split-equivalence.mjs   #         目录式模块形状：孤儿源文件（编译器不报）· 测试 mod.rs 用例数
│   ├── check-reserved.mjs            #         「预留」标注与清单对账，防口头预留
│   ├── check-docs.mjs                #         文档判据 R1–R10：入口厚度 / 地图 / ADR / spec / 长行 / 旧仪式 / 索引 / 断链 / 形状 / 不入库指针
│   ├── check-channels.mjs            #         事件通道：常量 · 变体 · 映射 · 订阅 · 处理 · 白名单
│   ├── check-single-source.mjs       # ★       同一数字/名字写在几处 → 往它的 CHECKS 加项，**不新建闸**
│   ├── check-schema-parity.mjs       #         memories 表两侧实现同源：建表串 · 迁移函数 · 调用顺序 · 写路径白名单
│   ├── check-style.mjs               #         前端模板维度形态闸（配 stylelint 的 CSS 声明维度）
│   ├── build_plugins.sh / build_market.sh
│   │                                 #         插件与市场产物：PLAN 是版本真源
│   └── split-ledger/                 #         历史拆分登记表：`make ledger LEDGER=<名>` 手动重放，不在常驻门禁里
│
├── specs/                            # ★       阶段计划：一阶段一目录，必须有可测验收与非目标
│   ├── _template/spec.md             #         模板（复制它才许开新阶段）
│   ├── 00-doc-governance/            #         文档规范治理：立规矩、历史归位、门禁重写、四个去处（进行中，剩检查脚本合并）
│   ├── 01-memory-context-redesign/   #         记忆与上下文重构：⏸ 等你给方向，里面列着"重构必须回答的四件事"
│   ├── 02-tool-cards/                #         工具卡做全：27 类工具样式 · 折叠分层 · 状态词 · 停止与重试表现
│   └── 03-structural-debt/           #         结构性欠账台账 T1–T17：分「可直接做」与「卡在人的一侧」两节
│
├── docs/
│   ├── roadmap/README.md             # ★       唯一进度表：只列未完成阶段，一行一阶段 + 勾选 + 一句人话状态
│   ├── adr/                          # ★       决策记录，只增不改
│   │   ├── 0000-template.md          #         ADR 模板与规则本体（编号怎么算、什么时候写、不写什么）
│   │   ├── 0001-doc-rules-from-template.md          # 旧文档规矩整套废除、换成模板那套；细则见 CONTRIBUTING
│   │   ├── 0002-gates-rewritten-to-template.md      # 门禁重写：删掉编译器已承重的判据、台账重放降级为手动（七项实测为据）
│   │   └── 0003-private-doc-layers.md               # 文档分「契约层」与「本机层」，本机层永不推送（authority 例外留下）
│   ├── design/                       # ⊘       只许下面两个子目录 + 根上散稿（R9 判）；要新建第三个目录先问人
│   │   ├── authority/                # ★       各主题「现在有效的那一份」：改了代码必须回来同步它
│   │   │   ├── DESIGN.md             # [LOCK]  内核权威设计 = 唯一需求来源；附录 C 是内核八条纪律与它们的实况订正
│   │   │   ├── UI Design System.md   # ★       UI 层最高权威：令牌映射 · 字号相对制 · 圆角嵌套 · 冲突裁决表
│   │   │   ├── Front-end Design Guidelines.md
│   │   │   │                         # ★       前端唯一形态标准：同类只许一种形态 + 特例申请制 + 两道机械闸
│   │   │   ├── Context and Memory.md · Memory Tree Design v2.md · Memory File-ization Design.md ·
│   │   │   │   Cross-Session Recall Design v1.md · KV Prefix Cache Design v2.md
│   │   │   │                         #         上下文与记忆族正身（装配 · 树 · 文件化 · 跨会话取回 · 前缀缓存）
│   │   │   ├── LLM.md · plugins.md · Plug-in packaging.md
│   │   │   │                         #         模型接入 · 「哪些是插件 / 宿主内置 / 已退役」的唯一真源 · 打包与暂存
│   │   │   ├── Tool exposure strategy and usage statistics.md · Setting and Constrained Optimization.md ·
│   │   │   │   edit_replace.md · Organize Summary Design v1.md
│   │   │   │                         #         工具暴露与用量 · 设置与约束 · 编辑替换 · 整理摘要
│   │   │   └── README.md             #         这一层的清单与"冲突时谁说了算"
│   │   └── archive/                  # 只读 ⊘  全名 docs/design/archive/ —— 已兑现 / 已退役 / 被取代的稿子；索引 = archive/README.md
│   ├── 根上的方案稿                   # ⊘       **接下来要开发或需要重构的设计**：稿上的事全部做完、或换了做法重开 ⇒ 整份搬 archive/
│   │                                 #         含下位细则 UI shell and patterns.md（与 UI Design System 冲突时以上位为准）
│   │                                 #         与 ⚠ 待并档的 Memory-KV Roadmap.md（第二张进度表，并进 roadmap 后退役）
│   ├── prototype/                    #         静态原型（css/ js/）= 方向参考，**不是像素合同**，演示件禁搬进生产码
│   ├── issue/                        # ⊘       全名 docs/issue/ —— 工单与缺陷清单；状态行须与自身处理记录一致
│   ├── references/                   # ⊘       全名 docs/references/ —— 外部参考笔记（cordis-rs · rutis）
│   └── history/                      # 只读 ⊘  旧批次流水 · 旧路线表 · 旧变更日志 · 已批准的计划；索引 = history/README.md
│       └── audit/ 与 docs/history/audit/parts/
│                                     # 只读 ⊘  审计报告 + 修复台账 + 分区明细（§7 欠账已先抄进 specs/03 才归档）
│
└── tmp/                              # ⊘       一次性实验与判据脚本（2026-10-17 台账 T8 已清空）
```

**已经长出来的**：内核 8 个 crate 与 9 个在册插件（`plugins-src/`）· 桌面端组装点与前端八个界面分区 ·
十道常驻结构闸 + 两道前端形态闸 + 两支闸自证 · 三份决策记录（另有模板）与四份阶段计划 ·
`docs/design/authority/` 十五份规范正身加一份清单。

**当前还不存在、按阶段才会长出来的**（不预留空目录，缺什么才建什么）：`specs/02` 的 27 类工具卡与六种状态词 ·
产出物与文件页签 · 斜杠命令与技能包子代理预设 · 后台子代理与运行队列 · 看板在真实窗口里的验收 ·
度量基线夹具 · 台账里那十七条欠账该结的部分 —— 具体到哪一步做什么，全在 [`docs/roadmap/`](docs/roadmap/) 那张表与它指向的 spec 里。

## 什么该入库

一句话判据：**下一个 clone 的人（或他的 agent）手里没有它，会不会做错事？**
会 → 入库（它是契约）；不会 → 不入库（它是过程或记忆）。

- **产物类一律不入库**：`target/` `desktop/src-tauri/target/` `desktop/dist/` `plugins/` 与 `market/` 的二进制 `tmp/`。
- **例外**：`Cargo.lock` 由 cargo 生成但要入库 —— 它是复现同一组依赖版本的凭据；只禁止手工编辑（AGENTS.md §5）。
- **运行期数据永不入库**：`%USERPROFILE%\.Locowrk` 下的会话、记忆库、配置与日志，路径真源 =
  `desktop/src-tauri/src/data_root.rs`（测试与便携场景用环境变量隔离）。这一层动它要点头，删它更要点头。
- **`⊘` 那几层的两件实话**（2026-10-17 裁定：authority 留在库里，其余排除）：
  ① 光加 `.gitignore` 不会让**已跟踪**的文件停止跟踪，必须 `git rm -r --cached`（本批已做，本地文件一个没删）；
  ② 它们**仍留在已有提交记录里** —— 裁定是「只管以后、不动历史」，所以这套排除不是保密措施，
  只是不让远端继续长出这些内容。真要清干净得重写历史，那是一刀不可逆操作，没做。
- **`tmp/` 的特殊性**：能长期挡错的少数几条升进 `scripts/`，其余用完即弃。它从来没进过 git ⇒
  里面的东西删了**任何地方都找不回**，所以清理前必须把要留的结论落进 `scripts/`、提交正文或文档正文。

## 目录按需生长

不要为将来预留空目录和空表格。缺什么才建什么：

| 何时 | 建 / 改什么 |
| --- | --- |
| 已建（别当待办再建一遍） | 结构闸与闸的自证都在 `scripts/`，两份清单唯一出处 = `tasks.sh` 的 `t_gates` 与 `t_selftest`（条数别抄进文档，加一档改一次就又漂） · 进度只在 `docs/roadmap/README.md` 一张表 · 决策在 `docs/adr/`（编号只增，篇数别抄进文档） · 阶段计划在 `specs/`（一阶段一目录，实例逐条增减 ⇒ 地图点 `specs/` 这一层，不逐个点名） · 规范正身在 `docs/design/authority/` 一层 |
| 阶段 02 开工（对话流工具卡做全） | `desktop/src/components/chat/` 里各工具卡组件 + `lib/tool-detail.ts` 的对应投影；形态先过 `Front-end Design Guidelines.md` 的类目，新形态要走特例申请 |
| 阶段「产出物与文件页签」开工 | 先建 `specs/04-…/spec.md`（模板复制），再动界面；页签的数据源若后端没有，欠账先登记进 specs/03 |
| 阶段「常用命令与技能包」「子任务与后台任务」开工 | 各自一个 spec；新命令要同时过 `check-commands.mjs` 的三向对账（定义 / 注册 / 前端引用） |
| 记忆与上下文重构拿到方向 | 新方案稿进 `docs/design/` 根（⊘），旧形状按稿上写清"被哪份取代"；兑现后 `authority/` 里那几份要回来同步，退役稿搬 `archive/` |
| 有第一个"为什么这么定"的决策 | `docs/adr/NNNN-<主题>.md`（`make adr` 复制模板并算编号；**只增不改**，「被否决的方案」必填） |
| 加了第一个新工具 / 新插件 | `plugins-src/<id>/`，并在 `scripts/build_plugins.sh` 的 PLAN 登记版本（版本真源只在这里） |
| 同一事实出现第二个出处 | **不新建文档、不新建闸**：往 `scripts/check-single-source.mjs` 的 CHECKS 加一项；跨实现的代码块同源才走 `check-schema-parity.mjs` |
| 术语开始被混用 | **不建 `docs/glossary.md`**（副本必然漂移）：术语定义住在 `docs/design/authority/` 对应规范的首节，发现混用就回写那一节并在提交正文登记 |
| 有发布物 | `CHANGELOG.md`（本项目至今不用：版本号只在 git 与桌面端设置页，批次叙事进提交正文） |

本文件树是**当前实际状态**，不写愿望。若本文件与仓库不符，相信仓库，并修正本文件。
