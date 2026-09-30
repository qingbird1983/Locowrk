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

## 二、当前排期（v2.17，2026-09-29 确认）

### 壳批次 —— UI 骨架（下一批，先于⑨）

> 动因：功能越多 UI 越乱的根因是缺落位规则。壳只锁结构与规则，内容留给
> 功能批次；视觉抛光仍归最后。详见 DESIGN.md §14 v2.17 壳批次段。

| 功能 | 状态 | 说明 |
|---|---|---|
| 导航模型定稿 | ⬜ | 顶层 4 视图（对话/插件/记忆/设置）；>4 视图时 pill nav 切侧边栏；开发者面板挂设置内 |
| 列表页模式 | ⬜ | 插件页/记忆页共用：搜索、筛选 chips、多选批量栏、行内徽章、空态 |
| 设置分区模式 | ⬜ | settings.json 分区 ↔ 面板分区一一对应（llm / context / memory / shortcut） |
| 会话内系统消息模式 | ⬜ | 压缩通知[查看详情][撤销] / "已记住：xxx" / 审批结果统一组件 |
| 徽章体系 + 进度条模式 | ⬜ | SideEffectBadges 并入闲置徽章（灰/黄/红） |
| 事件→UI 流向图 | ⬜ | `agent://` → store → 组件 + command → 面板 映射表，功能批次只扩表 |
| 弹层规则 | ⬜ | modal（审批）/ 行内展开 / 独立页面 的判据 |
| design tokens | ⬜ | 间距/圆角/色板 CSS 变量（不做全面抛光） |
| PluginMarket.vue 拆分 | ⬜ | 现 291 行，400 行纪律必拆 |

### ⑨ 上下文管理收口（P0）

> 详见 DESIGN.md §14 v2.17 ⑨段 + 《Context and Memory.md》§3。

| 功能 | 状态 | 说明 |
|---|---|---|
| ctx-compact 中间件本体 | ◐ | 已存在（v2.14 固定 32k + keep_recent 8），本批改配置驱动 |
| 模型窗口来源 | ⬜ | LlmService 拿 context window（设置字段 + 预设表）——开工先做，阈值按百分比算的前置 |
| 压缩阈值设置面板 | ⬜ | 滑条 + 百分比/绝对 token 双显示 + 下限 20% 警告（原收尾 backlog 项转正） |
| 压缩策略三档 | ⬜ | 滑动窗口 / 分层保留（默认）/ 仅摘要；压到阈值 50% 留缓冲 |
| 动态建议值 | ⬜ | 按窗口大小建议（本地 30% / 中等 35% / 大窗 25% / 1M 15%） |
| ContextCompacted 事件 + 撤销 | ⬜ | 进 session-store（serde default 兼容旧日志）+ rewind_to |
| 会话内实时进度条 | ⬜ | 绿/黄/红分级事件推送 + 压缩系统消息 |

### ⑩ 暴露声明 + 使用统计地基 + 闲置 UI

> 详见 DESIGN.md §14 v2.17 ⑩段 + 《Tool exposure strategy and usage
> statistics.md》§4/§3/§8。

| 功能 | 状态 | 说明 |
|---|---|---|
| ToolSpec `exposure` 字段 | ⬜ | always/on_demand/hidden 缺省 on_demand；manifest 可选字段追加零破坏，ABI 版本不动；always>10 告警 |
| 暴露管线（直通态） | ⬜ | always 直通 + on_demand 全暴露，行为暂不变（截断归⑫） |
| ToolCall 事件补 plugin_id | ⬜ | 现仅有 plugin_version；serde default 兼容旧日志 |
| usage.sqlite3 统计库 | ⬜ | 宿主侧自有（与 memory.sqlite3 分离）；tool_usage / plugin_usage 两表；5s 批量刷盘 |
| IdleLevel / IdleInfo | ⬜ | 六级闲置分类；后端只给结构化数据，前端渲染"多久没用"人类文案；14 天试用保护 |
| 闲置 UI | ⬜ | plugin_list_with_usage 命令 + 筛选器 + 批量操作（批量卸载二次确认列名） |
| 停用落盘核对 | ⬜ | lockfile set_enabled 已存在（lockfile.rs），仅核对启动只加载 enabled:true |

### ⑪ 记忆注入 + 记忆管理 UI

> 详见 DESIGN.md §14 v2.17 ⑪段 + 《Context and Memory.md》§4/§5。

| 功能 | 状态 | 说明 |
|---|---|---|
| memory-sqlite 插件本体 | ◐ | 已存在（v2.15）；本批对齐工具名 + 标 exposure=always |
| Memory-Inject 中间件 | ⬜ | 读记忆库 top-5 注入 system prompt；链位 ToolExposure → Memory-Inject → ctx-compact；**system prompt 永不压缩** |
| memory.put 同步落盘 | ⬜ | 不走异步队列（"说了记住却没记住"最伤信任） |
| 记忆管理页 | ⬜ | 搜索/列表/编辑/删除/清空；每条显示来源（explicit/extracted）与时间 |

### ⑫ 打分截断 + 元工具

> 详见 DESIGN.md §14 v2.17 ⑫段 + 《Tool exposure strategy and usage
> statistics.md》§5/§6。

| 功能 | 状态 | 说明 |
|---|---|---|
| EWMA 打分 | ⬜ | decay = exp(-days/7)，半衰期约 5 天 |
| Top-N 截断 + 调试面板 | ⬜ | N=20 可配；开发者选项看暴露分布与节省 token 估算 |
| 会话内热度 | ⬜ | 会话内 1.0 > 全局 EWMA 0.7 > 检索 0.5，always 满分直通 |
| search_tools 元工具 | ⬜ | always + Pure 主动拉取兜底；空结果回退 top-10 高频 |

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
| desktop session 持久化 | 会话事件落盘 | ⑨（ContextCompacted 依赖会话日志） |
| LLM 流式输出 | genai 流式 → 事件推送 | 看需求 |
| 市场 wasm 打包分发 | ⑧⑧ 遗留 | 看需求 |
| kv_get 所有权协议 | | 看需求 |
| profile.toml 设置面板接线 | | 看需求 |
| 托盘菜单 i18n | | 壳批次 |
| UI 视觉抛光 | 壳批次 tokens 之后统一做 | ⑬ 后 |
