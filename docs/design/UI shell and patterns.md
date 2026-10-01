# Locowrk UI 壳与页面模式（UI shell and patterns）

> 壳批次（v2.17 排期，2026-09-29 确认）落点文档。壳只锁**结构与规则**：
> 导航模型、页面模式库、事件→UI 流向图、弹层规则、design tokens。
> 内容留给功能批次（⑨–⑫ 只做"往模式里填东西"）；视觉抛光仍归最后。
>
> 实现落点：`desktop/src/`（`components/patterns/` + `components/{chat,
> plugins,settings}/` + `style.css` tokens）。

---

## 一、目的

功能越多 UI 越乱的根因是**缺落位规则**：新功能来了不知道该放哪个按钮、
哪个面板、点了往哪流。本文件把规则定死，后续批次照表扩容：

1. 新功能 = 新数据流（store）+ 模式组件的实例化 + 流向图加一行；
2. 不改 App 骨架、不发明新交互形态；
3. 想放的东西在表里没有落位 → 先改本文件（对齐规则），再写码。

---

## 二、导航模型（定稿）

| 项 | 规则 |
|---|---|
| 顶层视图 | 恒 4 个：**对话 chat / 插件 plugins / 记忆 memory / 设置 settings**（`App.vue` 的 `view` ref，头部 pill nav） |
| 切侧边栏判据 | 顶层视图 **>4 个**时，pill nav 换左侧边栏；4 个以内不引入侧边栏，也不引入 vue-router（`view` ref 足够，切侧边栏时再评估） |
| 保活策略 | 对话视图 `v-show` 保状态（进行中的会话不因切页丢失）；其余视图 `v-if` 按需挂载 |
| 全局层 | `ApprovalModal` 挂 App 根（跨视图弹出，§五弹层规则）；头部状态徽标（就绪/运行中）恒显 |
| 开发者面板 | 挂**设置视图内**（`settings.developer*` 分区，⑫ 落地），不占顶层视图名额 |

---

## 三、页面模式库

### 3.1 列表页模式（`components/patterns/ListPage.vue`）

一切"条目集合"页面共用：**头部（筛选 chips 左 + 搜索框右）→ 内容区（空态优先）→ 底部批量操作栏（`batching` 时显示）**。

- 行内容与动作由使用方以默认插槽提供；模式组件不认识业务类型。
- `v-model:search` 未绑定 → 搜索框整个隐藏；`filters` 未传 → chips 行隐藏。
- 批量栏内容经 `#batch` 插槽；**批量卸载必须二次确认并列出插件名**（exposure 文档 §9.5）。
- 第二排 chips：`subFilters` / `v-model:subFilter`（⑩ 已落地，闲置档位与状态档分离，§9.2）；不传不渲染。
- 实例：插件-已安装页（`plugins/InstalledTab.vue`，第一排 origin + 第二排闲置档 + 批量栏 + 提示条）、记忆页（`MemoryPage.vue`，仅搜索）。

### 3.2 设置分区模式（`components/settings/`）

**settings.json 分区 ↔ 面板分区一一对应**，顺序即文件里 `AppSettings` 的逻辑分组：

| 面板分区 | settings.json | 状态 |
|---|---|---|
| `LlmSection` | `llm` | ✅（既有） |
| `ContextSection` | `context`（⑨） | ✅（窗口预设+自定义 / 阈值滑条双显示 / 策略三档；后端投影带生效值与建议值，前端不做第二套换算） |
| memory 分区 | `memory`（⑪ 新增） | 占位 |
| `SessionsSection` | —（只读展示 dataDir） | ✅ |
| `LanguageSection` | `locale` | ✅（既有） |
| `ShortcutSection` | `shortcut` | ✅（既有） |
| developer 分区 | `developer`（⑫ 新增） | 占位 |

- 每个分区一个自管组件（表单 + 保存动作），容器 `SettingsPanel.vue` 只做堆叠与错误条。
- 分区落位时：删占位 `PlaceholderSection`，建同名组件 + settings.json 同名分区；**占位必须在功能落地同一批替换**，不留永久空壳。
- **拆分触发点**（用户 2026-09-30 确认）：单页滚动是现状定稿；分区数 >8 或单分区超 200 行/需要子分组时，才把容器升级为分类导航（先吸顶锚点 chips，再左列+右分区）——改造只落在 `SettingsPanel.vue` 容器，分区组件与 settings.json 一一对应的关系不动。最可能先膨胀的是 developer 分区（⑫ 调试面板 + 自动停用设置项），其出路是设置视图内子页签，不是新顶层视图。

### 3.3 会话内系统消息模式（`components/chat/SystemNotice.vue`）

"Agent 对会话状态的陈述"（压缩通知 / "已记住：xxx" / 审批结果）统一形态：**非气泡、非工具卡，居中窄条 + 可选动作按钮**（如压缩通知的 [查看详情] [撤销]）。

- chat store 的 `system` 类别：存 **i18n key**（`notice.*` 区）而非渲染文本——文案是 UI 不是会话数据，渲染期翻译以跟随语言切换。
- 动作只上抛 `action(id)`，实际行为由宿主接线（⑨ 已接：压缩通知 details→行内展开 / undo→`context_undo_compact`）。
- 生产入口两个：`chat.pushNotice(key, params, actions)`（i18n key 条）；`chat.pushRawSystem(text)`（**原文条**，`key = "__raw__"`，渲染 `params.text`——只用于"查看详情"展开的内容型陈述，如压缩摘要预览）。首个生产者 ⑨，随后 ⑪ "已记住"。

### 3.4 徽章体系 + 进度条模式

**徽章**（全部 `rounded-full px-2 py-0.5 text-[10px]`）：

| 族 | 组件/落点 | 配色 |
|---|---|---|
| 副作用族 | `SideEffectBadges.vue` | pure 青绿 / compensable 黄 / irreversible 红 |
| 闲置族 | `IdleBadge.vue`（⑩ 已接线 `plugin_list_with_usage`） | fresh/normal 不显示；idle 灰 / stale+never_used 黄 / abandoned 红（exposure 文档 §9.4）；文案表 = locales idle.*（§8.6/§8.7 档位） |
| 状态族 | InstalledTab 行内 | 运行中绿 / 待激活黄 / 已禁用灰 / 可更新蓝 |
| 来源族 | `SourceBadge.vue`（⑪ 记忆页行内） | explicit 蓝 / extracted 紫 / 未知灰；文案 = locales memory.source_*（后端只给结构化 source 串，§8.1 纪律） |

- 闲置徽章铁则（exposure 文档 §8.4）：**后端只给结构化 `{level, value, unit}`，前端渲染"多久没用"人类文案**；14 天试用保护由后端裁决。
- 新徽章先进对应族，不许在页面里裸写 span 徽章。

**进度条**：`ContextProgressBar.vue`，挂会话视图顶部；绿 <60% / 黄 60–99% / 红 ≥阈值；无数据（`null`）整个不渲染。⑨ 接 `contextUsage` 数据源。

### 3.5 数字呈现规范（《Setting and Constrained Optimization.md》§6，2026-09-30 入规）

面向用户的数字用人类尺度的档位与取整，不暴露机器精度的原始值；能显示比例就优先显示比例。

| ❌ 机器语言 | ✅ 人的语言 |
|---|---|
| 262,144 token | 256K |
| 52,428.8 token | ≈51.2K（compactNumber 按 1024 进位） |
| 102,400 字节 | 100KB |
| 1790770863（时间戳） | 3 天前 |
| 87 个工具 / 20 个暴露 | 已优化 87 → 20 |

- 大数用档位（K/M/G）、小数取整、单位不重复（写 `256K`，不写 `256K token`）。
- 已对齐先例：IdleBadge 人类文案铁则（§3.4，后端结构化/前端人话）、进度条整数百分比。
- ✅ S2 已接线：LlmSection 窗口预设（256K）、ContextSection 阈值预览（≈51.2K）、
  ContextProgressBar 比例段（`≈52K / 256K`，窗口未知时只显示百分比）、
  压缩 SystemNotice 的"已节省"档位化。
- 前端统一走一个 format 助手（`lib/format.ts` 的 `compactNumber`），
  组件不许各自手写换算。

---

## 四、事件→UI 流向图

> 功能批次只**扩表**，不改既有行的路径。

### 4.1 事件 → store → 组件

| 事件 | store | 组件 | 备注 |
|---|---|---|---|
| `agent://chunk` | chat `appendAssistant` | ChatMessages 助手气泡 | |
| `agent://tool-call` | chat push tool 项 | ChatMessages 工具卡 | 先事件后执行（内核侧纪律） |
| `agent://tool-result` | chat 按 call_id 补丁 | ChatMessages 工具卡结果区 | |
| `agent://turn-done` | chat `running=false` | 头部状态徽标 | |
| `agent://approval-request` | approvals pending 队列 | ApprovalModal（全局） | 启动补拉 `list_pending_approvals` |
| `plugin://state-changed` | plugins 整表重拉 | PluginMarket 两页签 | 策略：本地 IO 毫秒级，不做增量 |
| `agent://context-usage`（⑨） | chat `contextUsage` | ContextProgressBar | 每 step 后推送；used 与压缩判定同一估算口径 |
| `agent://context-compacted`（⑨） | chat `pushNotice('notice.contextCompacted', …, ['details','undo'])` + `noticeDetails` | SystemNotice | 撤销→`context_undo_compact`；详情→`pushRawSystem` |

### 4.2 命令 → 面板

| 命令 | 调用方 | 返回去向 |
|---|---|---|
| `agent_chat` / `agent_approve` / `list_pending_approvals` | chat / approvals store | 内容全走事件，返回值只表达收束状态 |
| `context_undo_compact`（⑨） | ChatMessages（压缩通知 undo） | 会话 id 不变；结果走 SystemNotice（notice.compactUndone） |
| `plugin_list` / `plugin_registry_list` | plugins store | PluginMarket |
| `plugin_install/update/uninstall/rollback/set_enabled` | plugins store | 完成后 state-changed 广播 → 整表重拉 |
| `settings_get` / `settings_set` | settings store | SettingsPanel 各分区 |
| `plugin_list_with_usage`（⑩） | plugins store `usage` | InstalledTab（IdleInfo → IdleBadge；filter_key → 闲置 chips） |
| `plugin_batch_set_enabled` / `plugin_batch_uninstall`（⑩） | plugins store 批量动作 | InstalledTab 批量栏（卸载二次确认列名，§9.5）；逐条成败 BatchResult |
| `memory_list/update/delete/clear_all`（⑪） | 新 memory store | MemoryPage；"已记住"走 SystemNotice |
| `tool_exposure_debug`（⑫） | DeveloperSection 挂载/刷新按钮 | 开发者分区只读面板：暴露分布（⑬ 起含语义检索入选行）+ 节省 token 估算（§3.5 compactNumber 档位）；打开分区时拉取，不推送不轮询 |
| `llm_probe_embedding`（⑬） | EmbeddingSection 检测按钮 | 分区内就地显示向量维度 / 错误条；不落盘只探测 |
| `experience_list/delete/clear_all`（⑬） | MemoryPage 挂载/行内删除/底部清空 | MemoryPage 经历区块（§11.3 可见可删；无编辑语义——自动沉淀原文）；经经历删除走行内二次确认 |

铁则重申：**UI 永不直接操作 PluginRegistry**（只经 PluginService / 宿主命令）；流式内容永远走事件推送，不塞 command 返回值；Tauri command 是薄适配层。

---

## 五、弹层规则

| 形态 | 判据 | 实例 |
|---|---|---|
| **Modal** | 阻断性 + 必须决策 + 现在不决定会话就卡住；**全局同时至多一个** | 审批弹窗（唯一现役）；⑩ 批量卸载二次确认（列名单，modal 形态） |
| **行内展开** | 单条目的详情查看/编辑，上下文就在列表里 | 记忆条目编辑（⑪）；压缩详情（⑨） |
| **独立页面** | 集合类功能：列表 + 筛选 + 批量运营 | 插件页、记忆页（顶层视图） |
| **默认不弹** | 设置类就地保存、状态变化用徽标/系统消息表达 | 保存成功 flash、state-changed 徽标 |

判据冲突时从严：能行内的不弹 modal，能徽标的不开新页。

---

## 六、design tokens（`style.css` `:root` `--lw-*`）

- **tokens 是唯一样式事实源**：表面三级 / 文本四级 / 语义色五 / 圆角四档 / 间距五档，取值与 Tailwind 调色板同值。
- 存量组件继续用 Tailwind 原子类不动；新组件优先用 `var(--lw-*)`；改皮肤只改 `:root` 一处。
- 壳批次**不做**视觉抛光（明暗主题切换、动效、精调间距归 ⑬ 后统一做）。

---

## 七、扩展规则（功能批次照做）

1. 新数据 → 新 store 或既有 store 加字段；事件进 §4.1 表、命令进 §4.2 表。
2. 新页面 → 先过 §二判据（是否值得顶层视图；不是就进设置或行内）；集合类一律 §3.1 列表页模式起手。
3. 新交互 → 先过 §五弹层规则；新徽章 → 进 §3.4 对应族。
4. settings.json 新分区 → §3.2 表加行，占位同批替换。
5. 改 App 骨架 / 模式组件本身 → 视为规则变更，先改本文件再动码。
