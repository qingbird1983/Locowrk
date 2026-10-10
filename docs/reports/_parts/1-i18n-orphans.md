# 前端 i18n 孤儿键（Team Lead 独立复算，含方法学与反例）

## 方法
1. 解析 `desktop/src/locales/{zh-CN.ts,en.ts}` 的嵌套对象为点分键集合（按缩进/花括号栈解析；**注意坑**：`en.ts` 里长英文串被 prettier 折到下一行（`key:` 后换行 `"..."`），解析器若只认「同一行 key: value」会把 18 个键误判为「zh 有 en 无」——首轮正是如此，已验证撤回。修正后 **zh = 949 键、en = 949 键，双向差集均为 0**，即两语言完全对齐、无缺失翻译）。
2. 扫描 `desktop/src/**`（排除 `locales/`）建立两类证据：
   - **静态键**：源码中带引号出现的完整点分路径（含 `$t('a.b')`、`t("a.b")`）；
   - **动态键前缀**：源码中出现的模板字面量 `${...}` 拼接前缀（全仓提取，不限 `t(` 调用点），实测 15 个：
     `approval.sensitive / chat.greeting / chat.greetingBare / effect / idle / market / memory.time / nav / notice.action / perm.desc / perm.short / time / tool.label / tool.type / tool.value`；另提取 `t("prefix." + ...)` 形态。
   - 全动态键（`` t(`${a}.${b}`) ``，连前缀都没有）：**0 处**（已专项检查）。
3. 判定：键若「无静态引用」且「其任一父前缀不在动态前缀集合中」且「叶名从未以带引号形式出现」→ 判为孤儿。

## 结论
- **确证孤儿键 50 个**：上述三条判据全部不成立 → 两个语言文件里共 **100 条死文案**。
- **待确认 9 个**：叶名（`ready/thinking/loading/success/region/done/title/baseUrl/unknown` 这类常见词）在源码里以带引号形式出现过，可能是无关标识符，也可能有未识别的拼接消费。

### 确证孤儿（50）
```
app.subtitle            status.* 中的 status.ready（待确认，见下）
turn.thinking*          → 待确认
model.loading*          → 待确认
tool.params             tool.result            tool.success*          tool.pluginBadge
sidebar.devUnlockHint   sidebar.menu.moveInto  sidebar.menu.moveOut
toast.movedInto         toast.movedOut         toast.associateFailed
subagent.region*  subagent.working  subagent.done*  subagent.count  subagent.doneCount
subagent.coordinating   subagent.main          subagent.child
developer.stabilityRounds      developer.stabilityRoundsHint
memory.viewOriginal            memory.hideOriginal
approval.riskHigh       approval.riskMedium    approval.riskLow
approval.askLead        approval.askTail       approval.inputLabel
approval.inputCopy      approval.timeoutNow
settings.title*         settings.chatTitle     settings.llmHint      settings.provider
settings.provActive     settings.provSetActive settings.provEdit     settings.provDelete
settings.provDeleteConfirm  settings.provAdd   settings.provEmpty    settings.provFormHint
settings.provLabel      settings.provNoModel   settings.provFetch    settings.provFetching
settings.provSave       settings.provCancel    settings.labelRequired
settings.baseUrl*       settings.baseUrlPlaceholder  settings.apiKeyPlaceholder
settings.subagentNoneMarked
error.unknown*
```
（标 `*` 者 = 9 个「待确认」项；其余 50 个为确证。）

`settings.*` 一区就占 23 个（provider 配置面板改名/重构后的旧文案）、`approval.*` 8 个、`subagent.*` 8 个（v4.13 `SubagentCard.vue` 大卡片退役后遗留）、`developer.stabilityRounds*` 2 个（v2.137 ctx-compact 稳定期设置退役后遗留）——**孤儿键与历次 UI 退役批次高度吻合**，可作为「退役时只删组件未删文案」的证据。

## 建议
1. 直接删 50 个确证键（两语言共 100 行）；9 个待确认项由人工 2 分钟核一遍（判据命令见上）。
2. **加一道防回归门**（本次审计最有价值的流程建议之一）：新增 `scripts/check-i18n.mjs`，做两件事——① zh/en 键集合必须双向相等（本次实测已相等，作为基线）；② 报告「无静态引用且不在动态前缀内」的键（输出为报告式、不拦截，与 `check_length.sh` 同风格）。挂到 `.githooks/pre-commit`（只对 `.vue/.ts` 变更触发）。这能拦住「删组件不删文案」这一类持续至今的漂移。
