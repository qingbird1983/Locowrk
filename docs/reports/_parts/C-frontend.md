# C · 前端健康度审计（desktop/src + 前端配置）

审计面：`desktop/src/**`（92 `.vue` / 51 `.ts` / 1 `.css` / 2 locale）、`desktop/package.json`、`vite.config.ts`、`tsconfig.json`、`tailwind.config.js`、`postcss.config.js`、`stylelint.config.cjs`。
**不含** `desktop/src-tauri/`（Rust 另有人负责）。对象 = 当前工作树（含 v2.145 未提交改动）。**只读审计，未改任何源码**。
门禁本身（`vue-tsc`/`build`/`stylelint`）已全绿，故本文件只列类型系统看不见的面。

## 判定方法（每条结论可复现）

- **孤儿 `.vue`**：取 92 个 `.vue` 的 stem，正则 `(?<![\w$])Stem(?![\w$])` 扫全部 `desktop/src/**/*.{ts,vue}`，零命中（自身除外）= 孤儿。**结论：0 个**。
- **模块可达性**：解析全部 `from "..."` 与 `import("...")` 相对路径 → 反向比对文件表。命中 1 个零 import 模块（C1）。
- **死导出**：对每个 `.ts` 提取 `export (const|function|class|interface|type|enum) NAME`，分别数「本文件内除声明外引用」与「其它文件引用」（正则 `(?<![\w$])NAME(?![\w$])`，允许 `.NAME` 属性形态）→ 两者皆 0 = 死导出（C2）；外部 0 且仅内部用 = 过度导出（本文件并入 C2 备注，不单列）。
- **i18n**：Node 内 `new Function` 求值两份 locale（`export default {...} as const` → `module.exports`）点分扁平化 → **zh 949 / en 949，双向差集均 0**（无单向缺键）。零引用判定 = 对 142 个非 locale 源文件做键字面量边界正则 `(^|[^\w.])KEY($|[^\w])`；动态键前缀（`` t(`tool.label.${x}`) `` / `'a.b.' + x`）单独收集 78 条，命中前缀者不计孤儿。每条孤儿另跑一次全仓 `Select-String`（含 Rust/docs）交叉验证：除 locale 自身外命中数为 0。
- **代码→键（缺失键）**：正则抽取 `t()/tr()/$t()/te()/tm()` 内字面量 + `labelKey/searchKeys` 数组 → 减去 locale 键集合（C5）。
- **依赖**：runtime 依赖查 `import ... from "<pkg>"`；构建期依赖查四个配置文件与 `scripts`（C22）。

---

## C1【孤儿模块：`lib/side-effect.ts` 整文件零 import，且含已退役 `sandbox_run`】高置信

- `desktop/src/lib/side-effect.ts:55` `export function toolSideEffect(tool: string): SideEffect | null`，全文 57 行。
- 模块可达性扫描：**全部 143 个 `.ts`/`.vue` 中无任何文件 import 该模块**（唯一非声明命中 = 本文件自身 + `style.css` 无关注释）。
- 该表内仍有 `desktop/src/lib/side-effect.ts:44` `sandbox_run: "irreversible",`——工具与 tool-sandbox 插件已在 v2.145 整目录退役；`:42 shout: "pure"` 亦无生产入口。
- 真正的副作用分级现在由后端给：`desktop/src/types.ts:92` `side_effect: string;`、`:349` `side_effects: string[];`，消费面 `components/SideEffectBadges.vue:9` `defineProps<{ kinds: string[] }>()` ← `PluginCard.vue:62` / `plugins/InstalledTab.vue:246` 传 `display.side_effects`。
- 判定：前端这张映射表是旧架构（前端自造分级）的遗留，后端字段已接手，整文件是死面。
- 建议：删整文件；`SideEffect` 类型一并删（外部零引用）。
- 删除风险：**低**（真正接线在后端字段；删后 `vue-tsc` 即可证伪）。

## C2【死 API 包装 ×3：`assetGc` / `llmListModels` / `llmProbeWindow`】高置信

- `desktop/src/lib/api.ts:105` `export function assetGc(): Promise<{ scanned: number; removed: number }> {` → `invoke("asset_gc")`
- `desktop/src/lib/api.ts:419` `export function llmListModels(): Promise<ProviderModels[]> {` → `invoke("llm_list_models")`
- `desktop/src/lib/api.ts:607` `export function llmProbeWindow(baseUrl: string, model: string): Promise<number> {` → `invoke("llm_probe_window")`
- 三者「本文件内除声明外引用 0」且「其它文件引用 0」；全仓 `Select-String` 仅命中 `api.ts` 声明行 + Rust 命令定义/注册行（`src-tauri/src/commands.rs:367`、`commands/assets.rs:87`、`commands/llm.rs:63`、`lib.rs:172/194/221`）。
- 对照：同族的 `llmProbeModels`(451)、`llmProbeEmbedding`(614) 都有消费方（`use-model-probe.ts`、`EmbeddingSection.vue:47` 一带），说明这三条是 v2.129/v2.58 迭代后被替换掉的旧通道（模型清单改走 probe 通道、窗口改走 `lib/model-profiles.ts` 静态档案 + 档位手填）。
- 建议：删三行包装函数（各 2–4 行）。**顺带记档**：对应 Rust 命令随之成为无前端入口的死命令——Rust 侧归别人负责，本文件只报"前端侧已死"。
- 删除风险：**低**（无调用点；后端命令保留不影响）。

## C3【i18n 孤儿键 61 条 · 甲组：已退役功能残留（33 键）】高置信

判定方式见「判定方法」；下列每条键在 `zh-CN.ts` 与 `en.ts` **同时存在且同时零引用**（两侧键集合完全相等），已用全仓 `Select-String` 交叉验证（除 locale 自身外命中 0）。

| 键 | zh 行号 | 原文 | 退役依据 |
|---|---|---|---|
| `developer.stabilityRounds` | `locales/zh-CN.ts:575` | `stabilityRounds: "稳定期轮数",` | v2.137「thresholdPct/strategy/stability_rounds 三键幻影化」，`ContextSection.vue:6` 注释自陈"随 ctx-compact 桌面链退役"，`DeveloperSection.vue:55` 同 |
| `developer.stabilityRoundsHint` | `locales/zh-CN.ts:610` | `stabilityRoundsHint: "压缩或整理后的 N 轮内不再改动请求前缀，范围 1–10。"` | 同上（en 侧 615）；`locales/zh-CN.ts:1105` 注释亦写"稳定期随 ctx-compact 桌面链退役" |
| `sidebar.menu.moveInto` | `locales/zh-CN.ts:217` | `moveInto: "移入「{name}」",` | `components/shell/SessionRow.vue:6` 注释「v4.11 裁定③：会话移入其它项目功能退役……侧栏不再有入口」；现菜单只有 rename/export/delete（`SessionRow.vue:60-72`） |
| `sidebar.menu.moveOut` | `locales/zh-CN.ts:218` | `moveOut: "移出工作区",` | 同上 |
| `toast.movedInto` / `toast.movedOut` / `toast.associateFailed` | `locales/zh-CN.ts:371-373` | `movedInto: "已移入工作区"` 等 | 同上（这三条是那次退役的 toast 文案） |
| `approval.riskHigh` / `riskMedium` / `riskLow` | `locales/zh-CN.ts:681-683` | `riskHigh: "高风险",` | v2.145 致命级收敛为三项、`DENY_RISKY+unlockRisky` 退役 → 卡面不再有风险档文字 |
| `approval.askLead` / `askTail` / `inputLabel` / `inputCopy` / `timeoutNow` | `locales/zh-CN.ts:684-691` | `askLead: "允许 ",` `askTail: " 运行吗？"` | v2.145 PermissionCard 重写（`components/chat/PermissionCard.vue` 现用 `approval.allowOnce/allowSession/allowProject/deny/...`） |
| `subagent.region` / `working` / `done` / `count` / `doneCount` / `coordinating` / `main` / `child` | `locales/zh-CN.ts:452-459` | `region: "子 Agent",` `coordinating: "正在协调 {n} 个委派任务",` | 旧子代理面板文案；现面板 `components/shell/SubagentsMenu.vue:55/60/69/77` 改用 `topbar.subagents` + `tool.running/tool.failed/tool.value.*` |
| `turn.thinking` / `turn.thinkingFor` | `locales/zh-CN.ts:68-69` | `thinking: "思考",` `thinkingFor: "持续了 {s} 秒",` | 思考行现用 `components/chat/ThinkingRow.vue:70` `t('tool.type.thinking')` |
| `tool.params` / `result` / `success` / `pluginBadge` | `locales/zh-CN.ts:230-239` | `params: "参数",` `success: "成功",` | 旧 ToolRow 四段式；现明细由 `lib/tool-detail.ts` 的 `tool.label.*` 供给 |
| `status.ready` | `locales/zh-CN.ts:16` | `ready: "就绪",` | 同块 `status.running` 仍被 3 处使用（`shell/ProjectGroup.vue:101`、`shell/SessionRow.vue:175`、`shell/Topbar.vue:79`），「就绪」态已无渲染点 |
| `memory.viewOriginal` / `hideOriginal` | `locales/zh-CN.ts:650-651` | `viewOriginal: "看原文",` | 同块 `originalLoading`(652)/`originalFailed`(653) 仍在用（`memory/TreeSection.vue:533/539`），这两个折叠/展开标签已无引用 |
| `app.subtitle` | `locales/zh-CN.ts:4` | `subtitle: "阶段八 · 真实模型接入 · 当前模型 = {model}",` | 阶段占位文案，阶段八早已走完（同块 `app.modelFallback` **仍在用**：`components/chat/Composer.vue:99`——同一命名域里只有这一条死） |

- 建议：**整组删**（连同 en 侧同键）。若担心历史回溯，可先删再跑一次 `vue-tsc` + 起一次应用目检英文/中文界面。
- 删除风险：**低**（零引用已双验证；唯一"看似有用"的是 `approval.risk*`，但风险档文字在 v2.145 后确无渲染点）。

## C4【i18n 孤儿键 · 乙组：设置页 legacy provider 块 22 键 + 杂项 6 键】高置信

**乙-1 连续死块 `settings.*`（`locales/zh-CN.ts:834-855`，22 键）**——整段是 ㉚ 模型配置页改版前的旧 UI 文案，现模型区由 `settings/model/*` 组件 + `settings.model*` 新键承担：

```
834:    llmHint: "API Key 只保存在本机，可配置多个服务商并随时切换模型。",
835:    provider: "提供商",
836:    provActive: "默认",        837: provSetActive: "设为默认",
838:    provEdit: "编辑",          839: provDelete: "删除",      840: provDeleteConfirm: "确认删除",
841:    provAdd: "添加服务商档位",  842: provEmpty: "还没有服务商档位——…",
843:    provFormHint: "…",         844: provLabel: "档位名称",    845: provNoModel: "未选模型",
846:    provFetch: "拉取清单",      847: provFetching: "拉取中…",  848: provSave: "保存档位",
849:    provCancel: "取消",        850: labelRequired: "档位名称不能为空",
851:    model: "模型",             852: baseUrl: "Base URL",
853:    baseUrlPlaceholder: "…",   854: apiKey: "API Key",        855: apiKeyPlaceholder: "未保存",
```
- 紧邻的 `856: apiKeyKeep: "已保存 · 留空保持不变"` **仍在使用**（`settings/EmbeddingSection.vue:115`）——即该块是被"截断"的遗留，不是整段命名域废弃。
- 证据：22 键逐个全仓 `Select-String` 命中 0（`settings.prov` 前缀全仓命中数 = 0，仅 locale）。对照 `settings.providers*`/`modelServicesGroup`/`modelAddService` 等新键均有用。

**乙-2 杂项孤儿（6 键，各自孤立不在任何死块内）**：`settings.chatTitle`(`zh:814` "对话行为"，ChatSection 现用 `settings.autoTitleTitle/chatWidthTitle/collapseToolTitle/…` 逐行标题，无分区总标题)、`settings.title`(`zh:782` "设置"，页面标题现走 `nav.settings`)、`settings.subagentNoneMarked`(`zh:967`，同域 `subagentTitle/Row/Hint/FollowSession/ThinkingRow/ThinkingFollow` 均有用，唯此条零引用)、`model.loading`(`zh:181` "正在拉取模型清单…")、`sidebar.devUnlockHint`(`zh:204` "连点 7 次进入开发者选项")、`error.unknown`(`zh:1115` "发生错误"，`error` 命名域仅此一键)。

- 建议：乙-1 整块删；乙-2 逐条删。删前建议 `grep -rn "settings\.prov" desktop/src` 复核一次（本次已复核 0）。
- 删除风险：**低**（唯一的近似风险是将来有人重新引入同名键——但 git 历史可恢复）。

## C5【i18n 反向缺口 ×4 代码键 + 1 搜索索引键（中英双缺，用户可见）】高置信 —— 本次最像 bug 的一条

代码里 `t()` 的键在 `zh-CN.ts` / `en.ts` **都不存在**，vue-i18n 会原样吐键名或回落：

| 键 | 位置 | 原文引用 | 影响 |
|---|---|---|---|
| `settings.proxySavedToast` | `components/settings/LlmSection.vue:296` | `toast.push("ok", t("settings.proxySavedToast"));` | **用户可见**：保存代理后 toast 直接显示 `settings.proxySavedToast` |
| `common.close` | `components/settings/model/ImportDialog.vue:110` | `:aria-label="t('common.close')"` | 无障碍名直显键名（`common` 块 `zh:19-23` 只有 back/cancel/refresh） |
| `developer.modeMemoryManager` | `components/settings/MemoryTreePanel.vue:103` | `mode: r.mode === "recorder" ? t("developer.modeMemoryManager") : t("developer.modeRule")` | 开发者面板记忆树模式列显示键名 |
| `developer.modeRule` | 同上 | 同上 | 同上 |
| `settings.panelTitle` | `lib/settings-index.ts:136` | `searchKeys: ["settings.developerTitle", "settings.panelTitle", "settings.maxTurnSteps"]` | 仅影响 ⌘K 设置搜索命中文案（搜索索引里出现键名碎片） |

- 复核方式：`node` 取 zh 键集合后做集合差（非模糊匹配），4 条代码键确认不在集合内；`common.close` 处 `common` 块仅 3 键（`zh-CN.ts:19-23`）。
- 建议：补文案（4 条代码键必补；`settings.panelTitle` 要么补键、要么从 `searchKeys` 摘掉）。**风险：低**，纯补文案；补完 `vue-tsc` 与界面目检即可。

## C6【已退役 `sandbox_run` 残留在活跃渲染层（6 处 + 1 条文案）】高置信

v2.145 已「tool-sandbox 退役——整目录删」，`plugins-src/` 与 `market/` 均无 `sandbox_run`（已列 manifest 核验），但活跃代码仍在为它分支：

- `lib/tool-detail.ts:62` `sandbox_run: { type: "terminal", icon: "terminal" },`（KIND_TABLE 表项）
- `lib/tool-detail.ts:123` `if (tool === "shell" || tool === "shell_read" || tool === "sandbox_run") {`（折叠行描述分支）
- `lib/tool-detail.ts:332` `/** shell / shell_read / sandbox_run 共用输出形状（exec.rs 同源）。 */` + `:526` 同款 `if (tool === "shell" ··· "sandbox_run")`（明细分支）
- `lib/tool-summary.ts:14` `sandbox_run: "tool.action.sandbox",`（动作名映射）
- `lib/tool-summary.ts:125` `if (tool === "shell" || tool === "shell_read" || tool === "sandbox_run") {`
- 连带 i18n：`locales/zh-CN.ts:330` `sandbox: "沙箱运行",`（en 328）——**这就是"文案指向已删功能"的实例**。注意它**不会被"孤儿键"扫描抓到**（因为 `tool-summary.ts:14` 仍字面引用它），只能靠工具清单比对发现。
- 判据补强：`tool-shell` manifest 只宣告 `shell` / `shell_read`；`tool-sandbox` 目录已不存在。
- 建议：从 KIND_TABLE、两处 `if`、动作名映射删掉 `sandbox_run` 分支，并删 `tool.action.sandbox` 文案（zh/en 各 1 行）。
- 删除风险：**低**——唯一需确认的是"已存会话回放里带 sandbox_run 的历史调用"：此时 `toolKind()` 会回落到 `{type:"tool", icon:"plug"}`、`toolAction()` 回落 `null` → 工具行显示原始 id + 插件徽章，属既有表外工具路径，不会崩。若要保留历史美观，可只删文案与 `tool.action.sandbox`（低价值），分支留 1 行兜底——但当前项目口径是「已存会话看不到 sandbox_run」，故建议全删。

## C7【陈旧工具映射：`fs_edit` 不存在的工具】高置信

- `lib/tool-summary.ts:18` `fs_edit: "tool.action.edit",`
- `fs_edit` 在 `plugins-src/` 全部 manifest 中**不存在**（tool-fs 只有 `fs_read/fs_list/fs_grep/fs_write/fs_delete/fs_restore`；编辑工具是 `edit_replace/edit_lines/edit_diff`，且 `:19-21` 已分别映射）。
- 全仓 `fs_edit` 命中仅剩：本行 + `docs/design/ui-prototype/index.html`（原型夹具）+ `docs/design/UI shell and patterns.md` 说明文字。
- 建议：删该行。**风险：低**（无工具能产出此 id；等价分支 `edit_*` 已在场）。

## C8【重复实现：命令包装剥离两份（regex 逐字相同）】中置信

- `lib/tool-summary.ts:63-84` `function commandSummary(raw: string): string` 与 `lib/tool-detail.ts:151-172` `function stripCommandWrapping(raw: string): string`：`cd\s+…&&` 与 `(?:bash|sh)\s+(?:-l\w*\s+)*-c\s+"…"` 两条正则、`hidden` 计数、`&&` 分段逻辑**逐字相同**，唯一差异是 `clip(first, 48)` vs `clip(first, 40)`。
- 建议：抽到 `lib/format.ts`（或新 `lib/command-text.ts`）：`stripCommandWrapping(raw, max)`，两处调用传各自长度。**风险：低**（纯函数搬迁，参数化截断长度）。

## C9【重复实现：工作区相对化两份】中置信

- `lib/tool-summary.ts:88-98` `relativize(path, roots)` 与 `lib/tool-detail.ts:176-…` 同名函数：同款「反斜杠归一 → 去尾斜杠 → 前缀大小写不敏感比对 → 回落 basename」。
- 差异点（有价值）：tool-detail 版多一步 `path.replace(/\\\\\?\\/g, "")` 剥 Windows verbatim 前缀 `\\?\`（v2.144 走查实抓的产物）。
- 建议：合并为一份（保留 `\\?\` 剥离，它对手写 `\\?\` 以外的输入无副作用），放 `lib/format.ts`。**风险：低**。

## C10【重复实现：相对时间渲染 `timeText` ×4，且都建在 `format.relativeTime` 之上】中置信

- `lib/format.ts:15` `export function relativeTime(tsSeconds, nowMs): RelativeTime` 已是唯一换算法。
- 但四处组件各自再写一遍「`r.n === undefined ? t(\`memory.time.${r.key}\`) : t(\`memory.time.${r.key}\`, { n: r.n })`」：
  `components/memory/ProjectTree.vue:74`、`components/memory/TreeSection.vue:225`、`components/MemoryPage.vue:149`、`components/shell/NotificationPopover.vue:99`（四段函数体一致，仅入参类型不同）。
- 建议：在 `lib/format.ts` 增 `relativeTimeText(ts, t)`（或 `lib/i18n-helpers.ts` 内，该文件已存在），四处替换。**风险：低**（纯展示层）。

## C11【重复实现：v-html 代码块复制委托处理器 ×2】中置信

- `components/chat/MessageRow.vue:171-182` `async function onBodyClick(e)` 与 `components/chat/ToolDetailView.vue:58-69` `async function onCodeClick(e)`：`.copy-btn` 选择器、`.code-block` → `pre code` 取文本、`navigator.clipboard.writeText` + `flashDom` + 静默 catch，**逐行相同**。
- 二者都已用同一 `useCopyFeedback()` composable（`lib/use-copy.ts:12`），但委托处理这一段没下沉。
- 注意：这不是"绕过 composable"的问题（`use-copy.ts` 确实被这两处+1 处 import，非死文件）。
- 建议：把该处理器也放进 `use-copy.ts`（如 `copyDelegatedCode(e: MouseEvent): Promise<void>`）。**风险：低**。

## C12【重复实现：外点关闭 + Esc 三件套 ×10 处】中置信

- `function onDocPointerDown` 常见形态（`AttachButton.vue:40`、`UsageRing.vue:28`、`PermissionModePicker.vue:72` 三处**逐字相同**）：
  ```ts
  function onDocPointerDown(e: PointerEvent) {
    if (!open.value) return;
    if (!(root.value?.contains(e.target as Node) ?? false)) open.value = false;
  }
  ```
  同文件紧跟 `onDocKeydown`（Esc 关）+ `onMounted/onBeforeUnmount` 注册。合计 7 处 `onDocPointerDown`（另有 `Composer.vue:60`、`PermissionCard.vue:78`、`WorkspacePicker.vue:96`、`SubagentsMenu.vue:118`）+ 3 处 `onOutside`（`SettingsSelect.vue:113`、`NotificationPopover.vue:46`、`RowMenu.vue:95`）= **10 个弹层的关闭逻辑各写一遍**。
- 建议：抽 `lib/use-dismissable.ts`（`useDismissable(rootRef, isOpenRef, { esc: true })`），10 处替换。**风险：低-中**——各处差异需逐个核对（个别用 capture 阶段、个别只在打开时挂监听），建议先抽三处逐字相同者。

## C13【重复实现：`stores/settings.ts` 保存样板 ×9】中置信

- 9 个 save 动作各写一遍「`if (this.saving) return; this.saving = true; this.savedFlash = false; try { view = await setSettings(...); savedFlash = true; setTimeout(() => savedFlash = false, 2000) } catch … finally { saving = false }`」：
  行号对：`73-74`、`123-124`、`140-141`、`157-158`、`174-175`、`191-192`、`223-224`、`240-241`、`255-256`（`savedFlash = true` + `setTimeout` 两行 ×9），方法体分布 `settings.ts:66-80 / 116-130 / 133-147 / 150-164 / 167-181 / 184-198 / 216-230 / 233-247 / 248-262`。
- 计：每块约 15 行 → 约 135 行中约 90 行是同构样板；`setTimeout` 无句柄保存，后一次保存会与上一次的定时器并存（短暂闪两次，无害但是同源缺陷）。
- 建议：内联 `async runSave(patch: SettingsPatch): Promise<boolean>` 收口（含 timer 句柄复用）。**风险：低-中**——各动作错误处理略有差异（有的 `return false`、有的只置 error），需保留分支语义。

## C14【未使用图标：`shield` / `star`】低置信（低影响）

- `lib/icons.ts:28` 联合成员 `| "shield"`、`:46` `| "star"`，且 `PATHS: Record<IconName, string>` 内各有路径（`vue-tsc` 全绿说明 Record 完整性成立）。
- 全仓对 `"shield"` / `"star"` 字面量的引用命中 0（`shieldCheck`/`shieldAlert` 有用：`chat/PermissionModePicker.vue:25-26`）。
- 建议：删两行 union + 两行 PATH。**风险：低**（`IconName` 收窄，`vue-tsc` 会立刻暴露漏网引用）。

## C15【注释腐烂 / 失效指令 ×4 处（零代码风险，纯可信度损耗）】中置信

- `lib/settings-index.ts:45-47`：注释写「「对话」「关于与更新」分区未实装，随各自批次补索引条目」，但 `:78-88` **已有 `chat` 分区**（`id: "chat"`，`settings.nav.chat`）。→ 注释与实际不符，容易误导后续维护者以为可加"对话分区"。
- `lib/tool-summary.ts:150`：`/** 剥 cd 前缀 / bash -lc 包装，&& 串联只留首段（与旧 toolArgSummary 同规）。 */` —— 它自己就在为 C8 的双份实现背书；合并后此注释应删。
- `lib/side-effect.ts:3-4`：注释自称分级「映射表 = 插件源码逐一对出」，但表内含已删的 `sandbox_run`（C1/C6），文件本身已无人 import。
- **失效的 lint 豁免指令 ×2**：`components/chat/MessageRow.vue:235` 与 `components/chat/ToolDetailView.vue:128` 各有一条 `<!-- eslint-disable-next-line vue/no-v-html —— 内容已过 DOMPurify -->`，但**本项目根本没有 ESLint**：全仓无 `.eslintrc*` / `eslint.config.*`，`desktop/package.json:22-33` 的 devDependencies 无 `eslint`，`scripts` 里也没有 lint 任务（只有 `lint:style` = stylelint）。→ 这两条注释是惰性注解，`v-html` 实际无任何规则守护；真正起作用的是 `lib/markdown.ts:6` 的 DOMPurify。
- 建议：随 C1/C8 一并处理；`settings-index.ts` 注释改为「当前 12 个分区即全部实装面」；`vue/no-v-html` 豁免注释要么删、要么把 ESLint + `eslint-plugin-vue` 真接进门禁（后者是新增工作量，需用户裁定）。**风险：无**（纯注释）。

## C16【优化：locale 单文件 1153 / 1135 行 → 按命名域拆多文件】中置信

- `locales/en.ts`（1153 行）与 `locales/zh-CN.ts`（1135 行）为单对象字面量；顶层命名域块边界（zh 行号）：`app:3 status:15 turn:61 model:173 sidebar:187 tool:229 toast:365 subagent:451 developer:536 memory:612 approval:677 settings:781 error:1114 palette:1117 …`。
- 结构性问题：①每加一条文案都要动同一个 1100+ 行文件，合并冲突面最大；②**两文件必须手工保持键集合一致**——本次比对虽全等（949=949），但 C3/C4/C5 合计 66 条漂移（**代码在用却缺键 5 条** + **存在却零引用 61 条**）说明这个手工约束已经在漏水。
- 建议（分两步，风险递增）：①先加一条**机械门禁**——一个小脚本比对两文件扁平键集合，差集非空即失败（比拆文件收益更高、风险接近 0）；②再按命名域拆 `locales/zh-CN/{chat,tool,settings,memory,…}.ts` + `index.ts` 汇总，en 同构镜像。**风险**：①低；②中（`i18n.ts` 的 import 面与 `as const` 类型面需同步，`vue-tsc` 可兜底）。

## C17【优化：`stores/chat.ts` 1064 行单 store】中置信

- `stores/chat.ts`：state `179-228`、getters `229-285`、actions `286-1064`（约 780 行 actions）。结构上混了四件事：①会话生命周期（`switchSession/send/editResend/stop`）、②事件投影（`agent://` 各分支 → items 变换）、③附件管线（`settleAttachments:912-926` 轮询、上传/删除）、④浏览器/用量等旁路状态。
- 建议拆分方向（按现有边界，不动行为）：`stores/chat/events.ts`（事件→item 投影纯函数，可单测）、`stores/chat/attachments.ts`（附件与上传）、store 本体只留会话编排。**风险：中**（Pinia 跨模块引用需注意 `this` 语义，拆分时优先"纯函数下沉"而非"拆 store"）。

## C18【优化：`types.ts` 1035 行单文件】中置信

- `desktop/src/types.ts` 1035 行，混合了：后端 DTO（`SessionMeta`/`ProviderView`/`ImportScanOutcome`…）、AgentEvent 判别联合（`types.ts:26-44` 一片 `export interface XxxEvent`）、设置视图（`ChatSettingsView`/`MemorySettingsView`/`AppearanceView`…）、记忆树视图。
- 附带发现：**71 个导出「仅本文件/本模块内使用」**（外部零引用），典型：`types.ts` 里 `ToolUncertainEvent/ContextUsageEvent/LlmRetryEvent/... ` 各只作为同文件 `AgentEvent` 联合成员出现；`lib/tool-detail.ts` 的 `Tr/ToolDetailCtx/PropRow/ListRow/DetailBlock/ToolDetail`、`lib/diff-parse.ts` 的 `DiffLine/DiffHunk/ParsedDiff/RenderBlock/COLLAPSE`、`lib/sidebar-preference.ts` 的 `SidebarPrefs/SIDEBAR_DEFAULT`、`stores/chat.ts` 的 `SummaryItem/TurnUsage/SessionUsage` 等。
- 判定说明：这些**不是死代码**（内部在用），是"`export` 过度"。价值：删掉多余的 `export` 可让「谁在用这个类型」一眼可查，也是 C18 拆文件的自然产物。
- 建议：拆 `types/agent-events.ts`、`types/settings.ts`、`types/memory.ts`、`types/llm.ts` + `index.ts` 汇总；顺手把零外部引用的 71 项去掉 `export`（**风险：低**，`vue-tsc` 立刻验证）。

## C19【优化：超长单组件（>570 行 6 个）拆分方案】中置信

| 文件 | 行数 | 拆点 |
|---|---|---|
| `components/chat/Composer.vue` | 832 | 已是"输入卡 + 附件条 + 模型选择 + 权限档 + 斜杠面板"的合体；附件条（`v-for a in chat.attachments` 起）、`onDocPointerDown/Keydown` 弹层收口（C12）、拖拽附件 handler 可各自成子组件/composable |
| `components/settings/model/ProviderSetupDialog.vue` | 650 | 两视图（预设瓦片 ↔ 凭据+ModelPanes）+ 自定义头列表 + 导入/导出 JSON；头部行编辑块可抽出 |
| `components/lab/BuiApprovalCard.vue` | 635 | 纯演示/夹具卡（仅 `lab/LabPanel.vue` 引用），与生产 PermissionCard 已分家——**可考虑移出 `src/` 到开发态目录**（结构性收益：不参与生产构建） |
| `lib/tool-detail.ts` | 622 | 已是单一职责（工具明细构建）但含 12 个分支函数；按工具族分文件（fs/edit/shell/memory/subagent/…）+ 索引，顺带解决 C8/C9 的去重 |
| `components/settings/model/ModelPanes.vue` | 616 | 双栏选择 + 高级区（窗口/推理档/自定义头），高级区可抽 |
| `components/settings/LlmSection.vue` | 574 | 服务列表 + 默认模型菜单 + 代理行 + 测试连接 |

- 判定口径：只报"结构上可机械切分"的，不评价美观。**风险**：拆分本身低，但会与 C5（`LlmSection.vue:296` 缺键）等现网缺陷的修复撞车，建议先修 C5 再拆。

## C20【优化：消息流渲染无虚拟化 + 每用户行一个 ResizeObserver】中置信

- `components/chat/TranscriptFlow.vue:75-116` `const blocks = computed(...)`：每次 items 变化**全量重建** blocks 数组（含 `names` 去重、`tsList`/`Math.min` 扫描）；`ChatTranscript.vue:138` 把 `chat.items` 整表交给它，模板 `TranscriptFlow.vue:301-411` 递归 `v-for` 渲染全部轮次/块/条目——**无窗口化/无 `content-visibility` 分块**。
- `components/chat/use-message-fold.ts:128-131`：每个用户消息行 `new ResizeObserver(remeasure)` + `observe`；长会话（数百条用户消息）＝数百个观察者。
- 实测口径说明（避免夸大）：`blocks` 追踪的是 `item.kind/tool/pending/ok/ts` 等字段，**流式 delta 改写正文文本不会触发重算**（Vue 属性级依赖），所以这不是"每帧重算"；问题在**条目数增长时的整体渲染成本**。
- 建议：①给"已收束的历史轮次"加 `content-visibility: auto` + `contain-intrinsic-size`（改动最小、收益接近虚拟化）；②或对 `items` 做尾部窗口（最近 N 轮精细渲染 + 更早折叠为摘要行）；③`use-message-fold` 的 RO 改为共享单例（一个 RO 观察所有目标元素）。**风险**：①低；②中（滚动锚点与"回到底部"逻辑耦合，需回归 `ChatTranscript.vue:74-109`）。

## C21【优化：附件就绪等待用 80ms 轮询，而非事件化】低置信（低影响）

- `stores/chat.ts:912-926` `settleAttachments()`：`while (attachments.some(a => a.status === "uploading")) { if (Date.now() > deadline) break; await new Promise(r => setTimeout(r, 80)); }`，30s 上限。
- 结构性意见：把"上传完成"做成 `Promise`（每个附件持有 resolve）即可免轮询；当前实现还有"超时后把仍在 uploading 的附件按已就绪处理"的语义缝（`:918` 只拦 `error`，不拦超时的 `uploading`）。
- 建议：事件化；或至少在超时分支显式处理 `uploading`。**风险：低-中**（改的是发送前门控，需覆盖"上传中发送"用例）。

## C22【依赖判定：runtime 7 + dev 10 全部 used，无未使用依赖】高置信

| 依赖 | 判定 | 证据 |
|---|---|---|
| `@tauri-apps/api` | used | `lib/api.ts:3-4`（core/event）、`stores/browser.ts:7`、`shell/BrowserTab.vue:13`、`main.ts:5-6`、`shell/Sidebar.vue:12`(app)、`shell/WindowControls.vue:12`(window) |
| `dompurify` | used | `lib/markdown.ts:6` `import DOMPurify from "dompurify";`（唯一落点，`v-html` 消毒面：`chat/MessageRow.vue:235`、`chat/ToolDetailView.vue:128` 注释引用） |
| `marked` | used | `lib/markdown.ts:5` |
| `pinia` | used | `main.ts:2` + 11 个 `stores/*.ts:defineStore` |
| `shiki` | used | `lib/highlight.ts:6-8`（含 `shiki/engine/javascript` 子路径）；入口 `main.ts:18` `import("./lib/highlight")` 动态加载 |
| `vue` | used | 全部 SFC + `lib/icons.ts:6` |
| `vue-i18n` | used | `i18n.ts:4`、`lib/settings-index.ts:11`（`Composer["t"]` 类型）、几乎所有组件 `useI18n` |
| `@tauri-apps/cli` | used | `package.json:11` `"tauri": "tauri"`；构建链在 `src-tauri` |
| `@vitejs/plugin-vue` | used | `vite.config.ts:2,6` |
| `autoprefixer` | used | `postcss.config.js:4` |
| `postcss` | used | `postcss.config.js`（插件宿主）+ `stylelint.config.cjs:11` `customSyntax: "postcss-html"` |
| `postcss-html` | used | `stylelint.config.cjs:11` |
| `stylelint` | used | `package.json:9` `lint:style` |
| `tailwindcss` | used | `postcss.config.js:3` + `tailwind.config.js`（`content: ["./index.html","./src/**/*.{vue,ts}"]`） |
| `typescript` | used | `tsconfig.json` + `vue-tsc` 依赖（`build` 脚本 `vue-tsc --noEmit`） |
| `vite` | used | `package.json:8` `build`/`preview` + `vite.config.ts` |
| `vue-tsc` | used | `package.json:8` |

- 判定口径：`import ... from "<pkg>"` 全量文本命中 + 配置文件引用；**无 unused、无 uncertain**。构建配置本身无冗余项（`stylelint.config.cjs` 的 `customSyntax` 与 devDep `postcss-html` 对齐；`tailwind.config.js` 的颜色角色表被 `style.css` 的 `--lw-*` 令牌消费）。
- 附带（配置面，非缺陷，仅记档）：`vite.config.ts:15` `host: "::1"` 与 `:9` `port: 5173, strictPort: true` 是有注释背书的刻意选择（注释 `:11-14` 说明 IPv4 回环被过滤驱动拖慢），**不是**硬编码残留。

---

## 已核查为健康的部分（明确排除，避免后续重复劳动）

1. **孤儿 `.vue`：0 个**。92 个 `.vue` 全部至少被 1 处引用（含动态/字符串形态已覆盖）；被引用次数最少的一批（`AttachButton`/`SlashPanel`/`UncertainCard`/`ContextSummaryBlock`/`BrowserTab`/`ReviewTab`/`WindowControls`/`ToastStack`/`ImageViewer` 等）均有明确单一宿主，非孤儿。
2. **i18n 键集合两语言完全对齐**：zh 949 / en 949，`zh-only = 0`、`en-only = 0`（无"英文界面直显中文键"类回归）。
3. **无 TODO / FIXME / HACK / XXX / `@ts-ignore` / `@ts-expect-error` / `as any`**；`eslint-disable` 仅 2 条且项目未配 ESLint（详见 C15）。
4. **无被注释掉的代码块**：≥3 行连续 `//` 且含代码特征的块 = 0；`.vue` 模板内注释掉整段标签 = 0；`v-if="false"` / `v-show="false"` = 0。
5. **调试输出干净**：全仓仅 5 处 `console.*`，全部在错误/异常路径（`main.ts:30/38` 启动、`lib/highlight.ts:65/82/121` shiki 降级），无 `console.log` 残留。
6. **无硬编码本地绝对路径 / 测试 marker / 写死端口残留**：命中的 `localhost:11434`、`localhost:1234/v1`（`lib/provider-presets.ts:54-55`）、`127.0.0.1:7890`（`LlmSection.vue:471` 代理占位符）均为产品语义默认值，非本机痕迹。
7. **`lib/api.ts` 命令包装与 Rust 命令注册面对齐**（除 C2 三条外，其余包装均有消费方；`assetPut/assetPutPath/assetRead/assetReveal/assetSaveAs`、项目权限规则三命令 `listProjectRules/deleteProjectRules/projectRulesDir` 等 v2.145 新增项均已接线）。
8. **重复实现的"反面"**：`lib/format.ts`（数字/时长/摘引）、`lib/use-copy.ts`、`components/shell/use-session-search.ts`（250ms 防抖 + 序号竞态守卫）、`lib/settings-index.ts`（导航轨与 ⌘K 共用一份索引）等**确有唯一真源**，搜索类异步调用均已防抖——未发现"搜索未防抖"类缺陷。
9. **Pinia store 内无 `JSON.parse(JSON.stringify())` 深拷贝、无 `structuredClone`、无 `deep: true` 重 watcher**（全仓命中 0）。

## 不确定项 / 待验证（不凑数）

- **78 条"动态键不可判"**：以下前缀表明这些键由运行时拼接，本审计无法证明其零引用（故未计入 C3/C4 的 61 条）：`approval.sensitive.`(10)、`chat.greeting.`/`chat.greetingBare.`(8)、`idle.`(6)、`memory.time.`(6)、`notice.action.`(3)、`perm.desc.`/`perm.short.`(8)、`tool.label.`(8)、`tool.type.`(16)、`tool.value.`(10)、`market.`(4)。若要收口，需把 `` t(`...${x}`) `` 改成显式映射表（或加一张"允许的动态键前缀白名单"门禁）。
- **`tool-summary.ts` 动作名表缺 `memory_delete` / `memory_undo`**：这两个工具在 `plugins-src/memory-sqlite/manifest.json` 中**存在**，但 `lib/tool-summary.ts:11-43` 的表里没有 → 界面会回落成裸 id + 「插件」徽章（`tool-summary.ts:58 return null`）。**待验证**：可能是有意的（删除/撤销动作名待裁定），也可能只是漏补；本次未找到注释说明，故列为待验证而非缺陷。
- **`lab/BuiApprovalCard.vue`(635) / `BuiRecommendationCard.vue`(354) 的性质**：仅 `lab/LabPanel.vue` ← `DeveloperSection.vue` 引用（devOnly 分区），是"演示夹具"还是"待接线真件"未在本审计中判定；若是纯夹具，建议移出生产树（见 C19）。
- **未执行门禁**：按任务要求未跑 `pnpm build` / `vue-tsc` 全量（Lead 负责）；C1/C2/C6/C7/C14 的删除风险结论基于静态引用分析，落地后需由 Lead 跑一次门禁复验。
