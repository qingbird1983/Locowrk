# ADR-0011：Phase 3 主壳采用 MyGo WebView 路径

- 状态：定稿
- 日期：2026-10-28
- 关联：ADR-0002；research/03；roadmap/phase-3；fixtures/spike-mygo

## 背景

ADR-0002 定下 MyGo 的“双路径”：原生 `ui.View` 与系统 WebView。Phase 3 开工前必须决定主壳先走哪条，避免边写边换。

Phase 3 的界面以会话流为主：长列表、流式富文本、代码块、输入框、工具卡片。MyGo v0.7.1 的 WebView 路径直接用系统内核（Windows WebView2 / macOS WKWebView / Linux WebKitGTK），原生路径则要求自己处理文本编辑、输入法与超长列表。

## 决策

Phase 3 主壳走 **MyGo WebView 路径**：

- 窗口由 MyGo 创建，页面由应用自带（`SetFrontend` 或 `LoadHTML`）；Go 服务通过 `mygo.Bind` 暴露，事件用 `mygo.NewEvent` 推送。
- TypeScript 客户端由 `mygo generate` 生成，不手写第二套协议。
- 原生 `ui.View` 从 Phase 4 的插件插槽开始落地；主壳是否迁移到原生，Phase 4 后用实测再定（新 ADR）。

## 依据（2026-10-28 spike）

`fixtures/spike-mygo` 在同一台 Windows 11 机器上验证了最小闭环（窗口隐藏，不干扰桌面）：

| 检查 | 结果 |
|---|---|
| 纯 Go 构建（无 cgo） | 通过；`go build` 直接出单个 exe |
| 隐藏窗口 + WebView2 | 通过；UA 为 Edge 155 |
| 页面 → Go 调用 | 通过；进程启动到页面回调 **545.7 ms** |
| Go → 页面类型化事件 | 通过；事件往返 **550.2 ms** |
| 退出与清理 | 通过；两个方向都完成后 `Exit(0)`，无残留窗口 |

未在 spike 覆盖、留 Phase 3 内验证：中文 IME、5,000+ 条消息的滚动、钥匙串、安装包体积与签名、三平台差异。

## 理由

- 会话流的“文本渲染 + 滚动 + 输入法”交给系统 WebView，成熟度高于 v0.x 的原生控件；输入法与富文本是 Phase 3 的主要风险，WebView 直接绕开。
- Typed IPC 与事件是 MyGo 的一等能力，页面与 Go 的契约可生成、可测试；桥接层仍只出项目 DTO（接缝 #2）。
- 插件原生路径不受影响：Phase 4 仍可在同一窗口内混排 `ui.View` 与 WebView（ADR-0002）。
- 前端只用于界面，不承载业务：内核、事件、适配器仍全在 Go。

## 后果

- 需要前端工程（开发期工具链、构建产物入库策略、CSP/资源白名单）；运行期不执行远程代码。
- 需要生成并校验 TypeScript 客户端；契约测试以 Go 侧 DTO 为准，前端只做类型检查。
- WebView 的平台差异（焦点、层级、缩放、高 DPI）进入 Phase 3 冒烟清单；弹层统一由宿主管理。
- 原生主壳推迟到 Phase 4 评估；若 WebView 路径在长列表或输入法上不达标，先修前端，不急于迁原生。

## 护栏

- 页面只加载应用自带资源与显式白名单 origin；`TrustedOrigins` 不使用 `"*"`。
- Go ↔ 前端只通过生成的 Typed IPC；不手写第二套 JSON 协议。
- 不在页面里持有凭证；设置页只显示“已配置/未配置”。
- 窗口关闭先取消进行中的请求，会话已落盘；插件（Phase 4 起）先停。
- 原生控件与 WebView 的混排规则、覆盖层由宿主统一管理（architecture/04 §3.4）。
