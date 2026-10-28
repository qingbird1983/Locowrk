# 调研 03：MyGo 桌面栈

- 状态：定稿（2026-10-28；Phase 3 spike 最小闭环通过，见 §8；Phase 4 混合渲染 spike 通过，见 §9）
- 日期：2026-10-28
- 关联：ADR-0002、ADR-0011；architecture/01、04

> 依据：MyGo 官方 README 与 pkg.go.dev（v0.7.1，2026-10-27）。API 仍在演进；本文记录“已知事实 + 待验证问题”，不把宣传语当合同。

## 1. 身份

| 项 | 值 |
|---|---|
| 模块 | `github.com/egoist/mygo` |
| 版本 | v0.7.1（2026-10-27 发布；v0.x，未稳定） |
| 许可证 | MIT |
| 语言 | 纯 Go，**无 cgo** |
| Go 要求 | 1.27.3+ |
| 平台 | Windows（WebView2）、macOS（WKWebView）、Linux（WebKitGTK） |
| 产物 | 单个 Go 二进制（数 MB 级） |

## 2. 两条界面路径

| | WebView 路径 | 原生 `ui` 路径 |
|---|---|---|
| 渲染 | 系统 WebView | MyGo 在 GPU 上自绘 |
| 写法 | 任意 Web 前端 + 生成的 TypeScript 客户端 | Go 函数 `ui.View(func(*ui.Context))`，每帧构建 |
| 适合 | 富文本、Diff、图表、已有 Web 组件 | 高密度结构化界面、低内存、快速启动 |
| 依赖 | 前端工具链（Bun 等，仅开发期） | 仅 Go |
| 混合 | 原生窗口内可嵌 WebView；菜单/对话框等覆盖在网页之上 | 同左 |

## 3. 关键机制

- **Typed IPC**：`mygo.Bind(service)` / `BindAs` 暴露方法；`mygo.NewEvent`、`Channel` 支持事件与流；`GenerateTypeScript` / `WriteTypeScript` 生成客户端。绑定服务的方法每次调用跑在独立 goroutine，可阻塞。
- **线程模型**：`App.Run()` 必须从 `main()` 调用；非主线程调用会被转发到主线程并等待；事件监听器在主线程执行，必须短小；慢活丢 goroutine。
- **原生视图**：视图是应用状态的函数；元素在下一帧前有效，不能缓存；持久状态靠位置/`Key` 保持（与立项方案的约束一致）。
- **桌面能力**：窗口、菜单、托盘、对话框、通知、全局快捷键、深链、文件关联、最近文档、剪贴板、屏幕、电源、文件监听、自动更新、打包/签名/公证。
- **凭证**：提供 `Secrets` 模块（系统钥匙串的 Get/Set/Delete），可作为 `ai.CredentialStore` 的实现基础。
- **测试**：官方称视图可脱离真实窗口测试（无窗口 tester），适合插件的帧生命周期断言。

## 4. 与本项目的匹配度

| 需求 | MyGo 能力 | 结论 |
|---|---|---|
| 单二进制、无 Node/cgo | 原生支持 | 强匹配 |
| 会话流、树、时间线 | 原生 `ui` + 虚拟化列表 | 待 spike（长列表、文本选择、滚动性能） |
| Diff/富文本 | 系统 WebView | 强匹配 |
| 插件插槽 | 无内置“插件槽”原语 | 需自建 `PluginHost`（已设计） |
| 中文输入 | 官方称 IME-aware | 待实测（Windows 优先） |
| 无障碍 | 官方称支持屏幕阅读器 | 待实测 |
| 钥匙串 | `Secrets` | 待验证三平台行为 |
| 自动更新 | 内置 updater（签名/增量） | 后期使用 |

## 5. 风险

1. **v0.x API 漂移**：升级可能破坏编译；必须锁版本，把用法收敛到 `desktop/` 与 `internal/bridge`。
2. **原生 UI 成熟度**：复杂文本编辑、超长列表、拖拽、右键菜单、焦点管理需要逐项验收。
3. **WebView 平台差异**：Linux 需要系统 WebKitGTK；焦点/层级/缩放/高 DPI 在三平台表现不同。
4. **构建环境**：原生路径要求 Go 1.27.3+；WebView 路径在 Linux 可能仍需系统开发库（待确认）。
5. **文档与示例覆盖**：v0.x 文档在补齐；关键结论以 spike 代码与测试为准，不以博客为准。

## 6. Phase 3 必做 spike（开工前）

1. **长列表**：10,000 条消息 + 流式增量，滚动稳定、内存可控、空闲 CPU 接近 0。
2. **中文输入**：Windows 下 IME 组合、候选窗、粘贴、撤销。
3. **混合渲染**：原生界面内嵌 WebView，弹层覆盖、焦点切换、缩放与高 DPI。
4. **钥匙串**：Windows Credential Manager 的读写/删除与失败路径。
5. **打包**：Windows 安装包体积、启动时间、代码签名占位。
6. **测试**：无窗口视图测试能否覆盖插件生命周期断言。

产出：更新本文状态为定稿；若原生主壳不达标，写 ADR 决定主壳改 WebView（插件原生路径不受影响）。

## 7. 结论（暂定）

MyGo 是目前最贴合“Go 内核 + 双渲染路径 + 单二进制”的选项。**先用 WebView 做主壳（Phase 3）以降低风险，原生 `ui.View` 从插件插槽（Phase 4）开始落地**；spike 结果若支持，再评估主壳迁移到原生。

## 8. Phase 3 spike 结果（2026-10-28）

夹具：`fixtures/spike-mygo/`（独立 module，MyGo v0.7.1，窗口隐藏，不干扰桌面）。

| 检查 | 结果 |
|---|---|
| 纯 Go 构建（无 cgo） | 通过；单 exe |
| WebView2 隐藏窗口 | 通过；UA Edge 155（Chrome 155 内核） |
| 页面 → Go 绑定调用 | 通过；进程启动到回调 **545.7 ms** |
| Go → 页面类型化事件（`mygo.NewEvent`） | 通过；事件往返 **550.2 ms** |
| 双向完成后退出 | 通过；`App.Exit(0)`，无残留窗口 |

- 本机（Windows 11）已装 WebView2 运行时；Linux 的 WebKitGTK 依赖与 macOS WKWebView 未在本轮验证。
- 未覆盖：中文 IME、5,000+ 条消息滚动、钥匙串、安装包体积/签名、三平台高 DPI 与焦点。这些进 Phase 3 冒烟清单，不阻塞主壳路径定稿。
- 决策：主壳走 WebView（ADR-0011）；原生路径 Phase 4 起用于插件插槽，主壳是否迁移以后再评估。

## 9. Phase 4 spike 结果（2026-10-28）

夹具：`fixtures/spike-mygo-ui/`（独立 module，MyGo v0.7.1，隐藏窗口）。目标是把“原生外框 + 内嵌页面”这条路走通，而不是做产品界面。

| 检查 | 结果 |
|---|---|
| 原生 `WindowOptions.Content` + `win.NewWebView("/")` | 通过；窗口 74.6 ms、WebView 93.3 ms（进程启动起算） |
| 页面 → Go 绑定调用（WebView 内） | 通过；679.5 ms |
| Go → 页面事件 → Go 回调 | 通过；684.4 ms（事件段约 5 ms） |
| 原生栏 / 顶条 / 右栏与页面同帧布局 | 通过；弹层锚定在原生控件上 |
| 无窗口多帧（100 行 + 500 事件计数） | 通过；120 帧平均 0.18 ms、最慢 0.65 ms |

结论与依据（源码可查）：

- **两种窗口互斥**：页面窗口的 `NewWebView` 返回 `errNoContent`；原生窗口用 `ui.WebView` 放置页面。因此 Phase 4 必须把主窗口改成原生窗口，页面内嵌（ADR-0013）。
- `Event.Emit(win, …)` 在原生窗口上会发给其所有 WebView；页面内的绑定、事件、`SetFrontend` 资源与普通页面窗口一致。
- `ui.NewTester` 可无窗口跑多帧视图测试，适合插件生命周期断言。
- 覆盖/焦点/Tab 的边界行为由上游 `ui/webview_test.go` 覆盖；中文输入法、真实滚动手感仍需人工确认。
- 本机（Windows 11）已装 WebView2；macOS/Linux 的 WebView 未在本轮验证，按 ADR-0007 只保证可构建。

