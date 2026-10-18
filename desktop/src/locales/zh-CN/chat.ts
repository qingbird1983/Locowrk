// locales/zh-CN/chat.ts —— 11 个命名域：chat / turn / ask / queue / composer / uncertain / notice / slash / idle / toast / notify
// 由批 F-22b 从 locales/zh-CN.ts 逐字节搬来；命名域必须**只在本文件**定义一次
// （index.ts 用对象展开合并，重复定义 = 后一份静默覆盖前一份，scripts/check-i18n.mjs 判红）。
export default {

  chat: {
    // 欢迎页问候（§3.14 v3.9 四态 + 当前索引项目名；v2.60：greetingBare =
    // 无任何已选项目时的不带名变体——项目名隐藏，"Locowrk" 兜底退役）
    greeting: {
      morning: "上午好，{name}。今天从哪个任务开始？",
      afternoon: "下午好，{name}。午后适合推进一件具体的事。",
      evening: "晚上好，{name}。今晚想把什么收尾？",
      lateNight: "夜深了，{name}。剩下的交给明天也可以。",
    },
    greetingBare: {
      morning: "上午好，今天从哪个任务开始？",
      afternoon: "下午好，午后适合推进一件具体的事。",
      evening: "晚上好，今晚想把什么收尾？",
      lateNight: "夜深了，剩下的交给明天也可以。",
    },
    placeholder: "输入 / 使用命令 · {'@'} 引用文件",
    send: "发送",
    copyMessage: "复制这条消息",
    editMessage: "编辑并重发",
    copyCode: "复制代码",
    // 长消息折叠（组件规格）：圆形展开/收起钮的 aria
    expandMessage: "展开长消息",
    collapseMessage: "收起长消息",
    replaying: "会话恢复中…",
    stopGeneration: "停止生成",
    widthAria: "对话宽度 {n} 像素",
    // v4.11 §3.39：发送键状态机 + 排队 + 回到底部
    placeholderQueue: "AI 正在执行，输入将排队…",
    queueAdd: "排队发送",
    backToLatest: "回到最新消息",
  },
  // 轮计时行（容器头部）与动作块折叠行（UI shell doc §3.7 / §3.9）
  turn: {
    working: "工作中 {t}",
    worked: "已工作 {t}",
    done: "已完成",
    stopped: "已停止",
    toolCount: "{n} 个工具",
    problemCount: "{n} 个问题",
  },
  // ask_user 选择卡（v4.11 §3.39①）：模型反向提问，composer 坞内作答
  ask: {
    cardTitle: "任务选择",
    queue: "还有 {n} 个待答",
    skip: "跳过",
    customPlaceholder: "或输入自定义回复…",
    send: "发送",
    submitFailed: "提交失败：{reason}",
    timeoutHint: "{n} 秒内未作答将自动跳过",
  },
  // 追加消息排队卡（v4.11 §3.39②）：运行中发送排队等待，收束后自动派发
  queue: {
    cardTitle: "{n} 条排队中",
    resume: "继续",
    sendNow: "立即发送",
    sendNowHint: "停止当前任务并立即发送这条",
    remove: "删除排队消息",
    clearAll: "清空排队",
    emptyPreview: "（空白消息）",
  },
  composer: {
    attach: "添加文件或图片",
    attachHint: "图片 / PDF 随消息发送 · 其余按路径引用",
  },
  // G2 崩溃恢复 uncertain 决策卡（可靠性稿 §5）
  uncertain: {
    title: "工具结果未知",
    detailMissing: "调用细节缺失（日志未及落盘）",
    desc: "应用在上次运行中退出，这个调用可能已执行、部分执行或没有执行。",
    completed: "视为已完成",
    notExecuted: "视为未执行",
    inspect: "检查后记录…",
    notePlaceholder: "记录你看到的实际状态…",
    confirm: "确认",
    resolved: "已裁决",
    rCompleted: "视为已完成",
    rNotExecuted: "视为未执行",
    rInspected: "检查记录",
  },
  notice: {
    action: {
      details: "查看详情",
      undo: "撤销",
      view: "查看",
    },
    editResendFailed: "编辑重发失败：{reason}",
    // ⑨ 压缩通知（[查看详情][撤销]，Context 文档 §3.8.3）
    contextCompacted: "已压缩早期对话，节省 {saved} token",
    // v2.143 整理发丝线：发起/收束/熔断同一条线就地改写（pushOrganizeNotice）
    organizeRunning: "正在整理对话上下文…",
    // M3 §3.37③：整理收束 / 熔断翻转（lw://organize-done / -paused）
    organizeDone: "已整理 {users} 轮为 {chapters} 个章节",
    organizeDoneDegraded:
      "已整理 {users} 轮为 {chapters} 个章节（记忆管理器不可用，已转规则切分）",
    // 队列 Q8b：轮内水位泄压（机械切分，摘要未补）——文案不许与「已整理」混用，
    // 那一刀是救援不是整理
    organizeReleased:
      "上下文已逼近窗口上限，先把本轮之前的历史收起 {chapters} 个章节（机械切分，摘要从简；本轮内容未动）",
    organizePaused:
      "本会话自动整理已暂停（连续 {rebounds} 次反弹达到熔断阈值 m={m}）；/整理 可手动整理并复位。",
    compactUndone: "已撤销压缩：恢复压缩前的完整上下文（其后的轮次已一并回退）。",
    compactUndoFailed: "撤销压缩失败：{reason}",
    // ⑭ 定时触发（调度引擎注入轮前的告知）
    scheduleFired: "定时任务触发：{title}",
    // LLM.md v3.0：传输重试 / 步数到顶
    llmRetried: "供应商暂时无响应，已自动重试（第 {attempt} 次）",
    turnCapped: "已达单轮步数上限（{steps} 步），本轮在此收束——发「继续」可接着跑；也可在 设置 → 工具与分发 调高上限",
  },
  // 斜杠命令面板（v2.143，§3.36① 扩展）：清单与提交拦截同源 lib/slash-commands.ts
  slash: {
    title: "命令",
    hint: "↑↓ 选择，Enter 或点击触发，Esc 关闭",
    none: "没有匹配的命令",
    desc: {
      organize: "整理当前会话上下文（手动整理不受熔断限制并复位熔断）",
    },
  },
  // 闲置徽章（IdleBadge，⑩ 接线 exposure 文档 §8.4/§11.3）：后端只给
  // 结构化 IdleInfo，前端按档位渲染"多久没用"人类文案——永不显示裸天数
  idle: {
    // 分级名（图例 / unit 缺失时的降级显示）
    fresh: "新启用",
    normal: "正常",
    idle: "开始闲置",
    stale: "长期闲置",
    abandoned: "疑似弃用",
    never_used: "从未使用",
    // §8.6 文案表
    today: "今天用过",
    this_week: "本周用过",
    last_week: "上周用过",
    last_month: "上个月用过",
    month: "{n} 个多月没用了",
    half_year: "半年多没用了",
    year: "{n} 年多没用了",
    // §8.7 从未使用（与"长时间未用"严格区分）+ 试用期保护
    just_installed: "刚装上",
    never2w: "装上两周了，还没用过",
    neverMonths: "装了 {n} 个多月，一次没用过",
    neverHalfYear: "装了半年多，一次没用过",
  },
  // 会话内系统消息（SystemNotice 模式）：文案是 UI 不是会话数据，store
  // 存 i18n key，渲染期翻译以跟随语言切换。
  toast: {
    exported: "已导出",
    exportFailed: "导出失败",
    deleted: "会话已删除",
    deleteFailed: "删除失败",
    renameFailed: "重命名失败",
    projectAdded: "已添加项目",
    projectAddFailed: "添加项目失败",
    projectRemoved: "已移除项目",
    projectRemoveFailed: "移除项目失败",
    revealFailed: "打开文件管理器失败",
  },
  // 通知浮层（§3.13，v2.51）：记录只存 kind + 参数，标题模板在这里渲染
  notify: {
    title: "通知",
    markAll: "全部已读",
    clear: "清空",
    all: "全部",
    unread: "未读",
    empty: "没有通知",
    emptyUnread: "没有未读",
    noContext: "无关联会话",
    scheduleFired: "定时任务已触发",
    subagentDone: "子 Agent 已完成",
    subagentFailed: "子 Agent 收束失败",
    contextCompacted: "上下文已压缩",
    savedTokens: "省下 {n} token",
    approvalTimeout: "审批超时，已自动拒绝",
  },
} as const;
