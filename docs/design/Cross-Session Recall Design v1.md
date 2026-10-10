# Cross-Session Recall Design v1

> v2.142（2026-10-10）。跨会话记忆取回闭环——清 v2.136 已知边界④「跨会话反查半闭环」的账。
>
> 用户裁定（2026-10-10 口述）：
> 1. 批 1（工具面闭环）照做；批 2（常驻进展行注入）**暂缓**——后期实际验证确实需要时再考虑。
> 2. **时间线优先**：最先查到的应该是时间线里的**总结 + 该总结所包含的标题**，这就够了；
>    需要某标题的详细内容时再深入（expand）追寻。

## §1 问题

树投影按 `(workspace, session)` 键位隔离存储（有意设计：目录只增保前缀缓存），数据都在盘上
（`scan_workspace` 一把可取全部会话的树）；断的是模型侧取回链路，共四处：

1. **目录面**：system 尾的树目录只投影当前会话（TreeDirectoryInject）——历史会话的 L0 标题
   在新会话不可见。设计本身正确，副作用是「模型默认世界观里只有本会话」。
2. **搜索面窄**：`memory_search_titles` 向量打分只对捆标题（L1 摘要只作 160 字跟随展示不进
   打分，v2.141 段落总结正文完全不进索引）——「之前做了什么」这类宽查询对短标题打分极易空手。
3. **下钻断裂（最致命）**：`memory_expand` 拿 `ctx.session_id` 加载当前会话的树，schema 无
   session 参数；而 search_titles 的描述叫模型「拿 chapter_id 去 expand」——照做必失败
   （v2.136 已知边界④记档在案，一直未立批）。
4. **时间线缺席**：项目跨会话总结链（v2.141 裁定④「进展记忆锚点」）只落在 UI
   （ProjectTreeView）与 tree.md（人读），模型面零暴露。

## §2 设计（批 1 四件）

### §2.1 memory_expand 跨会话化

- schema 加**可选** `session_id`（缺省 = 当前会话，存量调用逐字节兼容）；章节寻址变
  `(session_id, chapter_id)` 复合。tool-shell 0.1.4 同款「缺省 = 旧语义」。
- **workspace 含入当前会话**：workspace 仍由 `ctx.session_id` 解析——目标会话的树必须存在于
  当前项目的 scope 下才可展开（跨项目天然拒，无新增门）。L2 原文按**目标会话**日志的
  `event_range` 切片（`load_session` 基建已有）。
- **`mark_expanded` 只在目标 = 当前会话时写**：`expanded_at` 是「本会话上下文里有展开内容」
  的记账（自动收起的挑选依据）；跨会话展开的内容在本会话消息里、不在目标会话消息里，把记账
  写进目标会话的树会让它的自动收起误判。跨会话展开 = 纯读。
- 描述改写：写明可跨会话、id 来源 = memory_search_titles / memory_project_history。
- 版本 1.0.0 → 1.1.0。

### §2.2 search_titles 索引扩容

- **打分文本**从「标题」扩为 `标题 + \n + L1 摘要 + \n + 所在轮总结正文`（章节的
  `event_range` 落在某 `RunSummary [from, to]` 内即归属该轮；无总结覆盖 = 前两段）。
  v2.141 总结是浓缩叙事，语义检索价值最高。
- 关键词降级模式同扩（contains 对同一拼接文本）。
- hits 增 `session_title`（旁车标题，`session_log::list` 廉价读取——与 view.rs tree_project
  同源）；构造函数增 `sessions_dir` 参。
- 已知边界（记档）：同一轮总结文本会重复出现在该轮每章的打分文本里，一轮多章可能挤占
  top-5——模型仍拿到相关命中，接受；候选量级 = 捆数×会话数（桌面场景数百内），现打分即可，
  不建向量表。

### §2.3 新工具 memory_project_history（时间线 = 第一答案面）

- **无 query**：返回当前项目全部会话的时间正序时间线。用户裁定：第一答案面 = 每轮总结 +
  该总结所含章节标题，足够回答「之前做了什么」；细节再 expand 下钻。
- 每会话行 = 旁车标题 + `summaries[]`（总结正文 + 其所含章节 `{id, title}`）+
  `other_chapter_titles[]`（无总结覆盖的章节，v2.141 之前的存量会话）。
  归属判定 = 章节区间 ⊆ 总结区间（recorder 材料与总结同轮切片，包含关系成立）。
- **标题不带 L1 正文**（裁定：总结+标题够了，控制体积；L1 走 expand level=1）。
- **钳制**：最近 20 个会话（session_id 升序取尾 = 时间取尾），超出置 `truncated: true` 并
  指路 memory_search_titles 检索更早主题。
- 数据源与 `Organizer::tree_project` 同源（scan_workspace + session_log::list 标题映射），
  但工具不依赖 Organizer（组装点在 Organizer 构建之前）——自带同款聚合。
- Pure + OnDemand：读侧零风险；计划模式围栏天然放行；TreeToolGate 无需改（只管 collapse）。

### §2.4 repo_context 提醒段加钩子

静态提醒段追加一句（按需制措辞，v2.139 纪律——不开场仪式）：用户问及本项目的过往
（历史/之前的工作/以前的决定）→ 先 memory_project_history 看时间线，需要细节再
memory_search_titles / memory_expand 下钻。跨轮逐字节稳定不破。

## §3 不做的事

- **项目级大一统树目录**（全部会话捆并进 system 目录）否决：别的会话每次整理都改写本会话的
  system 前缀 = 全项目缓存互相打断；目录膨胀不可控；会话边界叙事混乱。分层方案（常驻
  一行式 + 按需下钻）回答同样的问题但零缓存代价。
- **批 2 常驻进展行注入暂缓**（用户裁定）：待实际验证确有需要再立批。
- 未归属会话（无 workspace）不进时间线（v2.59 严格口径一致）。
- 不为总结正文建独立向量表（量级不需要）。

## §4 测试

- tree_tools/tests.rs：跨会话 expand 三态（显式 session_id 命中且不写目标会话记账 / 缺省仍
  当前会话 / 跨会话 L2 切片自目标日志）。
- organize/tests.rs：search_titles 关键词命中走 L1/总结正文（标题不含查询词也命中）+
  hits 带 session_title；project_history 分组（总结含标题 / 存量无总结章节 / 截断钳制）。

## §5 已知边界

1. search_titles 同轮总结文本在多章打分文本中重复，可能挤占 top-5（§2.2，接受）。
2. project_history 20 会话钳制——更早主题靠 search_titles（其候选池无钳制）。
3. 时间线会话排序按 session_id 字典序（桌面 id 含毫秒时间戳 = 时间序）；非该命名法的
   id 排序语义不保证。
4. 批 2 挂账：常驻进展行注入，待实测验证。
