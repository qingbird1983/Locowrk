// 中文文案（默认语言，DESIGN.md §13.5）。会话内容属于数据，不在此列。
export default {
  app: {
    subtitle: "阶段六 · 审批流已接入 · 模型 = 脚本替身",
  },
  status: {
    ready: "就绪",
    running: "运行中…",
  },
  chat: {
    empty:
      "发送任意消息开始对话。消息里包含 “echo” 触发 tool-echo 插件；包含 “email”（或“邮件”）触发 send_email——它是不可逆工具，会弹出审批确认。",
    placeholder: "输入消息…（Enter 发送，Shift+Enter 换行）",
    send: "发送",
  },
  tool: {
    params: "参数",
    result: "结果",
    running: "执行中…",
    success: "成功",
    failed: "失败",
  },
  effect: {
    pure: "纯",
    compensable: "可补偿",
    irreversible: "不可逆",
  },
  approval: {
    title: "需要确认",
    subtitle: "该工具会产生不可逆的副作用，请审阅参数后决定。",
    toolLabel: "工具",
    argsLabel: "参数",
    deny: "拒绝",
    allow: "允许执行",
  },
  error: {
    unknown: "发生错误",
  },
} as const;
