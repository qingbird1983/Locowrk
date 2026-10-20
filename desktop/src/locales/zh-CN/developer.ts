// locales/zh-CN/developer.ts —— 3 个命名域：developer / approval / perm
// （S4 刀② 删掉 memory 域——记忆面整线退役；相对时间那族迁到 shell.ts 的 time）
// 由批 F-22b 从 locales/zh-CN.ts 逐字节搬来；命名域必须**只在本文件**定义一次
// （index.ts 用对象展开合并，重复定义 = 后一份静默覆盖前一份，scripts/check-i18n.mjs 判红）。
export default {

  // ⑫ 暴露调试面板（开发者选项只读区；与 settings.developer* 分开——
  // 面板是"用户来找"的信息，不是设置分区本体）
  developer: {
    hint: "开发者与危险开关。默认隐藏：侧栏底部版本号连点 7 次解锁。",
    allowDeleteRunning: "允许删除运行中的会话",
    allowDeleteRunningHint: "默认禁止，开启后强删可能造成转录不完整。",
    allowSwitchRunning: "允许切走运行中的会话",
    allowSwitchRunningHint: "默认禁止，开启后未结束的对话会混入新会话记录。",
    hide: "隐藏开发者选项",
    hideHint: "锁回隐藏态（危险开关保留原值）；版本号连点 7 次可再次解锁。",
    hidden: "已隐藏开发者选项",
    panelTitle: "最近一次分发分布",
    refresh: "刷新",
    empty: "还没有数据——发起一次对话后回来看。",
    disabled: "截断未启用，当前全量分发工具目录。",
    exposed: "本轮分发 {exposed} / {total} 个工具",
    always: "always 直通（{n}）",
    sessionHot: "会话内用过（{n}）",
    globalHot: "全局高频（{n}）",
    retrieved: "语义检索入选（{n}）",
    saved: "节省上下文：{n} token",
    cacheTitle: "前缀缓存读数",
    cacheHint: "会话累计与本轮用量的缓存读数，命中率不含缓存写入。",
    cacheEmpty: "供应商尚未报告缓存字段——发起对话后回来看。",
    cacheHitRate: "会话累计命中率：{pct}%（{steps} 步）",
    cacheHitMiss: "命中 / 未命中 / 总输入：{hit} / {miss} / {input} tokens",
    cacheTurn: "本轮：输入 {input} · 命中 {hit} · 输出 {out}",
    cacheAlertTitle: "缓存命中率告警",
    // M6.1 断裂诊断（KV v2 §7.4）
    cacheDiagnostics: "前缀断裂诊断",
    cacheDiagnosticsHint: "每次请求对前缀做分块指纹并与上一轮比对，定位命中率下降的断裂块；默认关。",
    cacheDiagTitle: "最近一步诊断",
    cacheDiagEmpty: "还没有记录（开着本开关跑一轮对话后出现）。",
    cacheDiagHealthy: "第 {step} 步：前缀健康，纯追加（{n} 条消息）。",
    cacheDiagBroken: "第 {step} 步断裂：{blocks}。",
    cacheAlertHint: "会话命中率跌破阈值时提醒，每个会话最多一次。",
    cacheAlertPercent: "告警阈值（%）",
    prefixTitle: "前缀纪律",
    prefixHint: "保持请求开头稳定以命中缓存，改动会让当前会话缓存重建一次。",
    pinSession: "工具集会话钉死",
    pinSessionHint: "工具清单本会话固定，新工具用 search_tools 找；关闭后每轮重新挑选，命中率会下降。",
  },
  approval: {
    // ㉕·对话区① §3.17 内联决策卡（ApprovalModal 退役；审批不是弹窗）
    cardTitle: "需要权限",
    queue: "还有 {n} 个待处理",
    envProject: "项目 {path}",
    envTemp: "临时会话",
    timeoutHint: "若 {n} 秒内未响应将自动拒绝",
    deny: "拒绝",
    allowSession: "允许本次对话",
    allowProject: "本项目始终允许",
    allowProjectHint: "保存粘性授权规则到本项目（shell 命令按精确命令记录；工具类按工具记录），跨会话生效。可在 设置→工具与分发 管理。",
    allowOnce: "允许一次",
    // ⑮ 子 Agent 派生的调用标识（session_id 非当前会话时显示）
    subagentTag: "子 Agent 调用",
    decisionFailed: "决策提交失败：{reason}",
    // 侧栏待决标记（卡不在视野内的兜底）
    pendingBadge: "有审批等待决策",
    // ㉘·B 越界门 / ㉘·C 隐私门（硬门卡只有两键的说明）
    externalHint: "目标在已声明工作区根之外，只能逐次允许",
    sensitiveLead: "触及隐私或高危目标：{kind}",
    sensitive: {
      ssh: "SSH 凭据目录",
      aws: "AWS 凭据目录",
      gpg: "GPG 密钥环",
      kube: "Kubernetes 集群凭据",
      creds_file: "凭据文件（.git-credentials / .netrc）",
      docker: "Docker registry 凭据",
      browser: "浏览器凭据库（登录数据 / Cookie）",
      os_store: "系统凭据库访问",
      cred_dump: "凭据转储工具",
      catastrophic: "毁灭性命令（格式化 / 整盘删除等）",
    },
  },
  perm: {
    title: "权限模式",
    // 名称/描述照抄参考对象（用户 2026-10-04 裁定「名字都一样的写 照抄」）
    short: {
      plan: "计划模式",
      ask: "变更前确认",
      acceptEdits: "自动编辑",
      auto: "完全访问",
    },
    desc: {
      plan: "只读调研，写类工具收起。",
      ask: "改文件前先问我。",
      acceptEdits: "自动编辑文件。",
      auto: "全部放行，基本不再弹窗；仅越界/隐私拦截。",
    },
    setFailed: "切换失败：{reason}",
  },} as const;
