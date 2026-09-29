// 中文文案（默认语言，DESIGN.md §13.5）。会话内容属于数据，不在此列。
export default {
  app: {
    subtitle: "阶段七 · 插件市场 + 热插拔管理 · 模型 = 脚本替身",
  },
  nav: {
    chat: "聊天",
    plugins: "插件",
  },
  status: {
    ready: "就绪",
    running: "运行中…",
  },
  chat: {
    empty:
      "发送任意消息开始对话。消息里包含 “echo” 触发 tool-echo 插件；包含 “shout”（或“喊”）触发 tool-shout——需要先在「插件」页从市场安装；包含 “email”（或“邮件”）触发 send_email——它是不可逆工具，会弹出审批确认。",
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
  market: {
    discover: "发现",
    installed: "已安装",
    search: "搜索插件…",
    install: "安装",
    installing: "安装中…",
    installedBadge: "已安装",
    updateAvailable: "可更新到 v{version}",
    update: "更新",
    enable: "启用",
    disable: "禁用",
    rollback: "回退到…",
    uninstall: "卸载",
    versionLabel: "版本",
    stateActive: "运行中",
    statePending: "待激活",
    stateDisabled: "已禁用",
    emptyDiscover: "市场目录为空。在仓库根目录运行 bash scripts/build_market.sh 暂存演示插件后重启应用。",
    emptyInstalled: "还没有已安装的插件。去「发现」页安装一个吧。",
  },
  error: {
    unknown: "发生错误",
  },
} as const;
