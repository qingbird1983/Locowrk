# Phase 5 阶段进度报告（草案：功能实现完成，待人工验收）

- 日期：2026-10-28
- 本阶段目标（一句话）：主区域可打开 Diff 审阅标签页，Go 只读取工作区变更，前端并排/统一渲染，切换不丢状态。

## 1. 完成了什么（不写行话）

- 主区域多了“会话”和“变更”标签：从工具卡片的“查看变更”打开；最多同时 4 个变更页，关掉即释放，切回会话再回来滚动位置与并排/统一模式还在。
- 变更页只显示“已跟踪文件的未暂存修改”，固定只读命令、5 秒超时、最多 2 万行 / 8 MiB / 200 个文件；超出显示截断，敏感文件只提示“已隐藏”。
- 变更页支持并排/统一、行号、按文件/片段折叠、轻量语法着色、复制路径、刷新；工具卡片可从具体文件定位。
- 标签栏、关闭按钮与 `⋯` 菜单由原生层绘制，菜单能压在网页上；外部链接交系统浏览器。
- 冒烟与单测覆盖：Diff 解析/服务/标签宿主、10,000 行打开、工具卡片链路、切标签状态保持、关闭后 `tabs=0`。

## 2. 验收场景执行结果（对照 roadmap/phase-5 §5）

| # | 场景 | 结果 | 证据 |
|---|---|---|---|
| 1 | 并排审阅 | ✅ 通过 | `-smoke-scenario diff-tool`：write 工具改写跟踪文件 → 点“查看变更” → 页面 `ok`、1 文件、5 行；截屏 `docs/design/phase-5/diff-page.png` |
| 2 | 大 Diff 1 秒 | ✅ 通过 | `-smoke-diff-lines 5000`：10,000 行，Diff 页 `diffMs=425`（本机，隐藏窗口） |
| 3 | 状态保持 | ✅ 通过 | 同一冒烟切到“会话”再切回：滚动位置非 0，`统一`按钮 `aria-pressed=true` |
| 4 | 混合层级 | 🟡 部分 | 无窗口 tester 断言 `⋯` 菜单覆盖页面并可点；Phase 5 spike 的 OS 截屏证明弹层压网页；**真实窗口焦点回网页/中文输入法待人工** |
| 5 | 生命周期 | ✅ 通过 | `tabs_test` 工厂计数：创建一次、切换不重建、关闭销毁、重开新建；冒烟关闭后 `tabs=0` |
| 6 | 平台 | 🟡 部分 | Windows 11 冒烟通过；`darwin/arm64`、`linux/amd64` `CGO_ENABLED=0` 构建通过；未真机运行 |
| 7 | 安全 | ✅ 通过 | 固定 git 参数/无 shell/超时/输出上限；敏感路径与长行单测；CSP；导航白名单 `mygo://localhost`，外链转系统浏览器，`window.open` 拒绝 |

## 3. 实测读数

| 项 | 读数 |
|---|---|
| 小 Diff（默认冒烟） | 页面 `diffMs=341`，5 行、1 文件 |
| 10,000 行 Diff | 页面 `diffMs=425`，10000 行、1 文件（隐藏窗口） |
| Go 测试 | `go test -race ./...` 全绿；`desktop` 约 4.6 s |
| TS 检查/测试 | `npm run check` 通过；`node --test` 4 项通过 |
| 前端产物 | `app.js` 31.6 KB、`diff.js` 20.6 KB、`diff.css` 5.1 KB |

> 打开耗时从 Diff 页面脚本开始到首次渲染完成；不含窗口启动。未测滚动帧率与内存峰值。

## 4. 接缝核对（architecture/03）

| 接缝 | 落点 |
|---|---|
| #2 宿主门面 | `Diff` 绑定只传 DTO；`desktop` 之外不出现 MyGo 类型 |
| #7 插件视图生命周期 | 标签页 WebView 在视图外创建；`Build` 只读快照；`ui.Element` 不落状态 |
| #10 凭证脱敏 | diff 不读凭证；敏感路径隐藏；git 环境过滤 diff 覆盖变量 |
| #12 内核边界 | Diff 只 import `desktop`/MyGo；不 import `internal/kernel` |

## 5. 体量核对（standards/08）

- docs/：新增本报告与图片；设计稿、phase 文档在预算内（≤300 行）。
- `.agents/`：CONTEXT 61 行；handoff 5 份；pitfalls 9 篇；文件数未超 40。
- 代码：新增最大文件 `desktop/host.go` 420 行、`web/src/transcript.ts` 500 行；均未超 500。
- 依赖：无新增 npm/Go 依赖；`diff.js` 20.1 KB，前端合计远低于 10 MB。
- 豁免登记：无。

## 6. 已知问题与风险

- **真实窗口焦点/输入法**：标签切换后键盘进入 Diff 页、菜单关闭后焦点回网页、中文输入法未人工确认（acceptance 4 未完全关闭）。
- **平台**：macOS/Linux 只构建，未运行；WebView 平台差异仍按 ADR-0007 记录。
- **文本选择**：虚拟列表只挂载视口附近，跨屏选择/复制整段 diff 不承诺。
- **未跟踪/已暂存**：不显示；界面有脚注。
- **真实模型**：`real` 场景无 key 未跑；不影响 Diff 功能，但阶段回归清单保留。
- **restore 场景**：已用 `scripts/seed-long-session.mjs` 种子 250 条消息重跑，`restore` 通过（1090 ms，250 行，`long marker` 命中）。种子写入了本机用户目录一次，已清理；后续种子应显式指定临时 HOME。
- **层级证据**：原生 `CapturePage` 不含子 WebView；层级用 tester + OS 截屏，不用原生捕获。

## 7. 对下阶段的影响

- 标签页宿主与 Diff 服务可供文件预览等复用；Phase 6 不依赖 WebView 标签。
- 标签栏视觉仍是功能粗稿，UI 重设计统一处理。
- 关闭阶段前需：人工确认 acceptance 4、决定是否接受“跨屏文本选择不承诺”，然后定稿本报告、更新看板/CHANGELOG、打 `phase-5.end`。

## 8. 文档更新清单

- `docs/design/phase-5/report.md`（本报告，草案）
- `docs/design/phase-5/diff-page.png`（并排视图截屏）
- `docs/research/03-mygo-stack.md` §11（双 WebView 与测试边界）
- `docs/README.md`、`docs/roadmap/00-roadmap.md`、`CHANGELOG.md`、`.agents/CONTEXT.md`（已同步）
