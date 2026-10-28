# ADR-0003：插件兼容策略——仅兼容编排类子集，TUI 成员 no-op + 诊断

- 状态：定稿（2026-10-28 修订：由“拒绝”改为“no-op + 诊断”，补三分类矩阵与基础对话框）
- 日期：2026-10-28
- 关联：ADR-0004、ADR-0008；architecture/03、04；research/02

## 背景

pi 的扩展是 TypeScript，能力面既有工具/命令/事件钩子，也有大量终端界面能力（`ctx.ui.*`、自定义 renderer、快捷键）。全量兼容需要移植 TUI 并复刻宿主环境，成本高且与桌面目标冲突；多数 TUI 扩展在 GUI 上也无意义。

`sky-valley/pi` 没有 pi 的 extensions runtime，A 轨兼容要由本项目补一层。兼容策略必须先于运行时选型定下来，否则协议、诊断与 UI 会各自漂移。

## 决策

对 pi 扩展实现**三分类兼容矩阵**（完整清单见 `architecture/04-plugin-system.md`）：

1. **支持（真实实现）**：`registerTool`、`registerCommand`、事件钩子、工具开关、模型/思考档设置；子代理、workflow、异步编排；`ctx.ui.notify/select/confirm/input` 映射为宿主对话框/通知。
2. **缓做（deferred）**：provider/虚拟模型/MCP 注册、session 读写与切换、`registerFlag`、autocomplete、markdown transformer 等——**成员存在，返回“未实现”诊断，不抛错**，后续版本按需转正。
3. **不支持（TUI 面）**：widget/footer/header/title、编辑器操控、自定义 renderer、工具 `renderCall`/`renderResult`、`navigateTree`、快捷键、`shutdown`——**成员存在但 no-op，永不 throw，每扩展每成员记一条诊断**。

安全能力（文件写删、命令执行、网络）走独立的能力闸：未声明 = 拒绝，与上述兼容分类无关。兼容降级不得绕开安全拒绝。

## 理由

- 扩展代码通常不写 `try/catch`；对 UI 调用抛 `ErrUIUnsupported` 会中断整个扩展。no-op + 诊断既让扩展继续跑，又让用户看见哪些能力没生效。
- 逐成员分类比“按 manifest 开关”准确：`notify` 可以映射，`setWidget` 只能 no-op，二者不该同一处理。
- 矩阵即兼容契约。pi 上游没有 semver，不承诺逐版本追平；清单增减走 ADR/文档修订。
- 安全拒绝与兼容降级分开：前者是 fail-closed，后者是体验降级，混在一起会让两者都不可信。

## 备选（否决）

- **全量兼容（含 TUI 模拟）**：等价于在桌面里跑终端模拟器，违背项目目标。
- **完全不兼容扩展**：放弃 pi 生态中最有价值的编排能力。
- **对 TUI 调用抛错**：扩展没有异常处理，等于让“界面上用不了”升级为“整个插件不能用”；诊断已能解决“静默忽略”的担忧。
- **所有 UI 统一返回空值且不诊断**：用户无法知道功能缺失，否决。

## 后果

- 需要 A 轨 shim：成员存在性、返回值语义、诊断去重、每扩展每成员的降级记录。
- 需要“兼容性诊断”UI，展示支持/缓做/不支持与实际调用。
- **A 轨运行时必须在 Phase 2 用 spike 定稿（ADR-0008）**；在定稿前不对外承诺“可运行 pi 扩展”。
- Phase 6 只做真实扩展接入与矩阵补齐，不再承担“选运行时”的决策。
- 文档必须明确：**不提供终端界面渲染**；不要用“完全兼容 pi 插件”做宣传。
- prompt templates 仍暂缺，不计入本矩阵。
