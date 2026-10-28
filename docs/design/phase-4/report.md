# Phase 4 进度报告：插件宿主与首批原生插件

- 状态：草稿（自动验收全绿；可见窗口、输入法与真实 provider 待人工确认）
- 日期：2026-10-28
- 关联：[phase-4-native-plugins.md](../../roadmap/phase-4-native-plugins.md)、[host.md](host.md)、ADR-0013、ADR-0011
- 环境：Windows 11 专业版 10.0.26200；Go 1.27.3；Node 24.14.0 / npm 11.21.1；MyGo v0.7.1；内核 `pi/` 基线 `sky-valley/pi@6f6131ba`

## 1. 结论

Phase 4 的实现完成，自动验收 7 项通过、1 项部分通过（回归 5/6，`real` 需 API key，未跑）。主窗口已按 ADR-0013 改为“原生外框 + 内嵌网页”：右侧插件栏与顶部 workflow 条是原生控件，Phase 3 的会话界面原样留在中间；三个插件、启停持久化、故障隔离与工具卡片联动都已接通。

**仍需人工确认**：可见窗口的观感与右栏折叠、原生栏与页面之间的焦点/中文输入法、`real` 冒烟。确认后再定稿本报告并打 `phase-4.end`。

## 2. 本轮改动

| 改动 | 说明 |
|---|---|
| 窗口形态 | `WindowOptions.Content` 原生窗口 + `NewWebView("/")`；页面用 `Event.EmitPage` 收事件 |
| 插件宿主 | `desktop/plugin.go`、`host.go`：按事件类型分发、会话切换清空、插槽级 recover、设置持久化（`%AppData%\Locowrk\settings.json`） |
| 原生外壳 | `desktop/shell.go`：顶条（workflow + 插件菜单）、右栏（编排树 / 工具检查器）、可折叠并记忆 |
| 三个插件 | 编排树（200 节点上限）、workflow 时间线（6 步展示）、工具检查器（200 条环形缓冲、参数/结果各 2 KiB） |
| 页面联动 | 工具卡片点击 → `Desktop.SelectToolCall` → 原生栏选中；只新增这一个绑定 |
| 冒烟 | 新增 `plugin`、`plugin-crash`；`-show`、`-config`；Phase 3 五场景回归 |
| 隐藏窗口适配 | 内嵌页面 rAF 在窗口隐藏时暂停，批量刷新加 120 ms 定时器兜底；隐藏窗口不构建原生内容，故 crash 场景显示窗口 |

## 3. 验收结果（Phase 4 §5）

| # | 场景 | 结果 | 证据 |
|---|---|---|---|
| 1 | 插槽可见 / 启停保持 | ✅（外观待人工） | 无窗口测试断言插槽标题与勾选；`TestPluginMenuTogglesAndPersists` 落盘后新宿主仍禁用 |
| 2 | 实时更新 200 ms | ✅ | `plugin` 冒烟：注入后一帧内 `nodes=2 steps=2 tools=2`；tester 单帧断言 |
| 3 | 元素纪律 | ✅ | `TestPluginsHoldNoElements` 反射遍历；`TestPluginPanicIsIsolated` 连建多帧不丢状态 |
| 4 | 性能 | ✅ | 100 节点 + 500 调用：60 帧 **0.70 ms/帧**（上限 16 ms） |
| 5 | 隔离 | ✅ | 崩溃插件停用 + 1 条诊断，其他插件与页面正常；`plugin-crash` 冒烟 `frames=4` |
| 6 | 空状态 | ✅ | `暂无子代理`、`暂无工具调用`、`选择一条调用查看详情` |
| 7 | 边界 | ✅ | `plugins_test.go` 解析 6 个文件，无 `internal/kernel` / `pi/` 依赖 |
| 8 | 迁移回归 | ✅ 5/6 | `faux / tool / cancel / long / restore` 通过；`real` 未跑（无 key） |

补充读数：`faux` 2,652 ms（隐藏，按 120 ms 定时器刷新）；`long` 5,000 行、挂载 29 行、279 ms；`restore` 250 行、1,121 ms；前端 `app.js` 28.9 KB。生成客户端重跑 `git diff` 仅 `selectToolCall`。

**判定说明**：`plugin-crash` 会短暂显示窗口（Windows 只对可见窗口构建原生内容），这属于验收契约的一部分，不是可见性回归。

## 4. 发现问题与修复

1. **内嵌页面收不到事件（阻断级）**：`win.Page()` 在原生窗口上不存在；改为 `App.SetPage(web.Page())` + `Event.EmitPage`，页面恢复实时流。
2. **隐藏窗口页面不更新（阻断级）**：内嵌 WebView 在窗口隐藏/遮挡时暂停 `requestAnimationFrame`，transcript 的合批永远不落 DOM。改为 rAF + 120 ms 定时器兜底；定时器在 rAF 触发时清理，避免第二个 bug（`pendingTimer` 挡住后续合批）。
3. **隐藏窗口没有原生帧**：Windows 不构建隐藏窗口的原生内容，`frames=0`。验收拆分：数据用隐藏冒烟，渲染/崩溃用无窗口 tester 与 `-show` 的 crash 场景。
4. **工具卡片冒烟时序**：隐藏页面合批有 120 ms 延迟，断言先于挂载。`scenarioTool/Plugin` 改为等待 `.tool` 出现。
5. **工具计数重复**：编排树与检查器都上报工具数，`Snapshot` 相加变 2。约定每项指标只由一个插件上报（节点=树，步骤=时间线，调用=检查器）。

## 5. 自动化入口

```bash
# 插件数据（隐藏窗口）
go run ./cmd/locowrk -smoke -smoke-scenario plugin

# 崩溃隔离（短暂显示窗口，5 秒内退出）
go run ./cmd/locowrk -smoke -smoke-scenario plugin-crash

# Phase 3 回归
go run ./cmd/locowrk -smoke
go run ./cmd/locowrk -smoke -smoke-scenario tool
go run ./cmd/locowrk -smoke -smoke-scenario cancel
go run ./cmd/locowrk -smoke -smoke-scenario long
node scripts/seed-long-session.mjs <home> <cwd> 250
go run ./cmd/locowrk -smoke -smoke-scenario restore -smoke-expect "long marker" -cwd <cwd>

# 无窗口插件测试
go test ./desktop/
```

真实 provider（key 仅环境变量）：`go run ./cmd/locowrk -smoke -smoke-scenario real`。

## 6. 接缝核对（architecture/03）

| 接缝 | 结论 |
|---|---|
| #1 事件契约 | ✅ 插件只消费 `eventbus.Event`；会话不匹配即丢弃；新增测试覆盖 |
| #2 UI 无关门面 | ✅ `internal/bridge` 未改，依赖树仍无 MyGo |
| #3 取消贯穿 | ✅ 未改；`cancel` 冒烟回归通过 |
| #4 会话单写者 | ✅ 插件只读事件，不写会话文件 |
| #7 插件视图生命周期 | ✅ 反射断言无 `ui.Element`；每帧重建；WebView 在视图外创建一次 |
| #10 凭证脱敏 | ✅ 插件不接触凭证；设置文件无凭证 |
| #12 内核边界 | ✅ 6 个插件/宿主文件无 `internal/kernel`、`pi/` import |

#5、#6、#8、#9、#11 本阶段未触碰（A 轨与子进程插件属 Phase 6/7）。

## 7. 体量核对（standards/08）

- docs/：新增 3 篇（ADR-0013、设计稿、本报告），总数 52 篇（软上限 120）；单篇最大 274 行。
- .agents/：16 个文件（上限 40）；CONTEXT 59 行；handoff 保留最近 5 份。
- 代码：新增 >500 行文件：无；最大 `desktop/host.go` 390 行、`cmd/locowrk/main.go` 315 行。
- 依赖：新增 1 个间接依赖 `github.com/go-text/typesetting`（MyGo `ui` 的文本排版，MIT）；无直接新依赖。
- 前端资产：`app.js` 28.9 KB、`style.css` 4.5 KB；单文件远小于 1 MB。
- 豁免登记：无。

## 8. 规范复查（standards/09 四问）

- 漏洞：隐藏窗口的“原生内容不构建”与 rAF 暂停没有写进规范；已进设计稿 §5/§10 与 P-008，暂无升格为规范的证据（只影响冒烟与测试写法）。
- 过严：`standards/08 §6` 禁止提交 `dist/`，与 Phase 3 起的“前端产物入库供 embed”冲突；本轮沿用现状，建议在发布工程时一并修订（不在本阶段扩范围）。
- 过松：插件故障只要求“诊断可见”，未规定诊断留存时长；Phase 4 只在内存，重启即清，报告已记录。
- 冗余矛盾：未发现新增重复；`architecture/04` 的接口与插槽表已按实现修订并留日期。

## 9. 已知问题与风险

- **可见窗口观感未确认**：原生栏宽度、1px 分隔线、顶条留白、高 DPI 需要人眼；中文输入法在原生栏与页面之间的切换同理。
- **`real` 未跑**：Phase 3 的 `real` 断言未重跑；有 key 时补一次即可，不改变主壳代码。
- **隐藏冒烟耗时**：页面按 120 ms 定时器刷新，`faux` 约 2.6 s；可见窗口约 20 ms，不代表交互延迟。
- **panic 恢复代价**：丢一帧、当帧构建计数偏移；测试与冒烟未出现输入丢失，暂不引入双帧预检。
- **插件状态生命周期**：会话切换即清空，切回旧会话不恢复树/时间线（只从后续事件重建）；如需历史回放，随 Phase 6 的适配器数据源一并考虑。
- **macOS/Linux**：`ui.WebView` 未在真机验证；按 ADR-0007 只保证可构建。
- **里程碑**：设计与实现同轮推进，未单独打 `phase-4.start`；人工确认后直接打 `phase-4.end`。

## 10. 文档更新清单

- 新增：`docs/design/phase-4/host.md`、本报告、ADR-0013、`fixtures/spike-mygo-ui/`
- 修订：`docs/roadmap/phase-4-native-plugins.md`、`docs/roadmap/00-roadmap.md`、`docs/architecture/04-plugin-system.md`、`docs/research/03-mygo-stack.md` §9、`docs/README.md`、`CHANGELOG.md` 0.9.0/0.9.1、`.agents/CONTEXT.md`、pitfall P-007/P-008、handoff H-012/H-013
