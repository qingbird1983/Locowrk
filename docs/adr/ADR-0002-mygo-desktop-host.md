# ADR-0002：桌面宿主采用 MyGo，原生视图与 WebView 双路径

- 状态：定稿
- 日期：2026-10-28
- 关联：architecture/01、04；research/03

## 背景

需要一个 Go 桌面框架，同时满足：纯 Go/无 cgo、单二进制、能画结构化高密度界面（会话流、树、时间线）、能嵌富交互网页（Diff、编辑器）、有类型安全的 Go↔前端调用。候选：MyGo、Fyne、Wails、Gio、Tauri/Electron（非 Go）。

## 决策

采用 `github.com/egoist/mygo`（v0.7.x）作为桌面宿主：

- 主壳与结构化可视化用原生 `ui.View`（Go 函数，每帧构建，GPU 绘制）。
- 富交互面板用系统 WebView（WKWebView / WebKitGTK / WebView2），通过 MyGo 生成的 TypeScript 客户端调用 Go。
- MyGo 只在 `cmd/`、`desktop/`、`internal/bridge` 出现；内核、事件、适配层不依赖 GUI。

## 理由

- 纯 Go、无 cgo，跨平台构建简单；单二进制、内存占用低，符合产品定位。
- 一个窗口可混合两种渲染路径，正好对应“原生编排可视化 + WebView 富交互”的插件设计。
- Typed IPC（`Bind`、`Channel`、`GenerateTypeScript`）省掉手写 bridge 与类型漂移。
- 自带桌面能力：菜单、托盘、对话框、通知、快捷键、深链、文件关联、钥匙串（Secrets）、更新器。
- MIT 许可证，与项目兼容。

## 备选（否决）

- **Fyne**：纯 Go，但自绘控件体系与高密度桌面工作台（虚拟化长列表、复杂文本编辑、WebView 嵌入）匹配度不足。
- **Wails**：成熟，但以 Web 前端为中心，原生界面与“网页嵌在原生层之下”的混合能力不如 MyGo；仍依赖外部 WebView 工具链。
- **Gio**：底层、可控，但组件与桌面集成要大量自建。
- **Tauri / Electron**：非 Go 技术栈，与“Go 内核 + 单二进制”目标冲突。

## 后果

- 正面：两条一等公民渲染路径；桌面能力齐全；与内核同语言。
- 负面/风险：
  - MyGo 仍是 v0.x，API 会漂移；必须锁版本、隔离用法、升级独立提交。
  - 原生 `ui` 相对年轻（输入法、无障碍、复杂控件需实测）；Phase 3 先做 spike，必要时主壳改用 WebView。
  - WebView 在各平台行为不同（焦点、层级、缩放），需要平台冒烟清单。
- 缓解：把 MyGo 用法收敛到少数文件；为关键交互写无窗口视图测试；维护平台验收清单。
