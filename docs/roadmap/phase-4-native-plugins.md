# Phase 4：PluginHost 与首批原生插件

- 状态：草稿
- 日期：2026-10-28
- 关联：ADR-0002、ADR-0003；architecture/03、04

## 1. 目标

定义 `DesktopPlugin` / `PluginHost`，用原生 `ui.View` 实现三个结构化插件：编排树、workflow 时间线、工具检查器；验证元素生命周期与插槽机制。

## 2. 开工前置条件

- Phase 3 完成，主壳稳定。
- MyGo 原生 `ui` 的长列表、文本、焦点、无窗口测试能力经 spike 验证。
- 插槽布局与插件启停交互定稿。

## 3. 做什么

### 3.1 宿主

- `desktop/plugin.go`：`DesktopPlugin`、`PluginView`、`Command` 接口。
- `desktop/host.go`：插槽注册表、按事件分发、构建调度、启停、诊断。
- 插槽：`session.sidebar`、`session.timeline.above`、`session.tool.detail`、`session.tab`、`settings.section`。
- 插件注册表：内置插件静态注册；启停状态持久化到设置。

### 3.2 插件

| 插件 | 插槽 | 数据来源 |
|---|---|---|
| `orchestration-tree` | `session.sidebar` | `subagent_spawned/settled` + 工具事件 |
| `workflow-timeline` | `session.timeline.above` | `workflow_step` |
| `tool-inspector` | `session.tool.detail` | `tool_execution_*` 的参数/结果/耗时 |

### 3.3 生命周期纪律

- 插件只持有数据状态；视图每帧构建；`ui.Element` 不落状态。
- 事件回调只更新数据 + 标脏；不直接操作窗口。
- 插件崩溃/panic 被宿主 recover，禁用该插件并显示诊断。

## 4. 不做什么

- 不做第三方动态插件（Phase 7）。
- 不做 WebView 插件（Phase 5）。
- 不做插件市场、签名、自动更新。

## 5. 验收场景

1. **插槽可见**：三个插件分别出现在约定位置；可独立启用/禁用，重启后状态保持。
2. **实时更新**：mock 编排事件触发时，编排树 200ms 内出现/更新节点；workflow 阶段条推进；工具检查器显示参数与结果。
3. **元素纪律**：多帧测试断言插件状态中无 Element；切换页面/重建视图不 panic、不丢数据。
4. **性能**：100 个子代理节点 + 500 个工具事件，界面不卡；事件密集时合并刷新。
5. **隔离**：一个插件 panic 不影响其他插件与主壳；诊断可见。
6. **空状态**：没有编排事件时显示“暂无”，不占位、不报错。
7. **边界**：插件只依赖 `desktop` 的公开接口，不 import 内核内部包。

## 6. 交付物

- `desktop/` 宿主与三个插件。
- 插槽契约文档（补进 architecture/04）。
- 多帧/生命周期测试与性能读数。
- Phase 4 报告。

## 7. 风险

- 原生控件表达力不足：允许插件退化为“简洁列表 + 详情”，不硬做复杂图形。
- 事件过多导致重绘风暴：合并 + 节流；必要时只刷新可见区域。
- 插件 API 过早固化：先内部使用，Phase 7 前不承诺第三方稳定。
