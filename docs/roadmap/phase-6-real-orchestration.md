# Phase 6：真实 Pi 编排插件与技能复用

- 状态：定稿
- 日期：2026-10-28
- 关联：ADR-0003、ADR-0004、ADR-0008、ADR-0009、ADR-0015；architecture/04 §2、architecture/05；research/02；[runtime](../design/phase-6/runtime.md)、[extensions](../design/phase-6/extensions.md)、[skills](../design/phase-6/skills.md)

## 1. 目标

按 ADR-0009/ADR-0015 把真实 pi 编排扩展接进 A 轨：预打包 Node 子进程、显式信任、工具/命令/事件/基础对话框可用；三分类矩阵按最终状态给出可见诊断；桌面能看到并行/链式/异步子代理与 workflow；运行中可补充指令（steering）；用户技能自动加载、项目技能显式信任。

## 2. 开工前置条件

- Phase 5 完成（tag `phase-5.end`）；Phase 2 适配器、协议与三分类测试稳定。
- 本阶段设计定稿：`design/phase-6/runtime.md`、`extensions.md`、`skills.md`；ADR-0015 记录 Node 运行时目录。
- `runtime/node/` 按 ADR-0015 下载/校验，Windows 上能启动扩展宿主；不通过先改 ADR。
- 选定 1~2 个真实扩展样本（`todo`、`rpc-demo`，固定上游提交，记录来源/许可证），并能在本地构建；自写 `orchestrator-sample` 用于并行/链式/取消。
- 用户同意“A 轨扩展 = 受信任代码”的产品口径，安装/信任界面必须明示。

## 3. 做什么

### 3.1 运行时与打包

- 随包 Node 目录；`adapter.Options.NodePath`；发布缺运行时 fail-closed，开发回退 PATH 需警告。
- `tools/extension-build/`：esbuild 把扩展 + shim 打成 `bundle.mjs`，生成 `plugin.json`；拒绝原生模块/远程 import；构建可复现。
- shim `locowrk-shim/1`：JSONL 协议、工具/命令/事件/工具执行/基础对话框；三分类永不 throw；`tool_update` 与 `event_result` 可选方法。
- 真实样本不入库源码：构建时外部检出；`build.json` 记录来源/哈希。

### 3.2 发现、信任与生命周期

- 用户 `%AppData%\Locowrk\extensions`、项目 `<cwd>/.pi/extensions`、开发 `-extensions`。
- 设置 v2：扩展路径/哈希/开关/信任时间，工作区信任；v1 迁移；哈希变化回到未信任。
- `internal/extensions.Manager`：扫描/校验/启动/停止、工具与命令快照、事件扇出、诊断/通知、会话切换；单扩展失败不影响其他。
- 扩展状态面板（B 轨插件 `extensions-status`，`session.sidebar`）：状态、信任/撤销、启用/禁用、重试、诊断、通知、命令。
- 工具名冲突、入口缺失、Node 版本不符、哈希不符都只禁用该扩展并显示原因。

### 3.3 编排与 steering

- 工具注册进内核 `ExtraTools`，走同一审阅/取消；`tool_update` 显示进度。
- 事件：`subagent_spawned`/`subagent_settled`、`workflow_step`、异步完成；现有编排树与 workflow 条消费。
- 子代理由扩展自行 spawn 并上报；`spawn_subagent` 宿主调用保持缓做。
- steering：busy 时输入区“补充指令” → `Desktop.steer` → kernel.Steer → `steering_applied` 事件；协议 `steer` 在 Phase 6 不使用。
- 中止：`Desktop.stop` → kernel.Abort + 扩展 `abort`；取消不是错误。
- 基础对话框：`ctx.ui.select/confirm/input/editor` 映射原生对话框；取消返回空值。

### 3.4 技能

- 用户技能自动加载；项目 `<cwd>/.pi/skills` 需工作区信任；撤销只影响后续会话。
- 技能列表/诊断/注入数量在状态面板可见；不展示完整系统提示词。
- `kernel.Skills` + bridge DTO；新建/恢复会话传 `TrustProject`。

## 4. 不做什么

- 不做运行期安装/编译扩展、插件市场、签名分发、OS 级沙箱。
- 不做宿主代管子代理、链式上下文改写、自定义 renderer、TUI 渲染、快捷键、MCP、prompt templates、主题。
- 不做多会话并排的扩展进程池；Phase 6 只服务当前活动会话。
- 不做技能编辑器、项目 `.agents/skills`、父目录信任继承。
- 不做命令自动补全/斜杠菜单位置优化（留 UI 重设计）。

## 5. 验收场景

1. **真实扩展加载**：`todo` 成功启动；工具/命令/能力清单与状态面板一致；未信任时不得启动。
2. **parallel 委派**：`orchestrator-sample` 一次起两个子代理，桌面同时显示两个节点与各自工具过程；先完成者先 settle。
3. **chain/async**：链式步骤按顺序推进；异步任务完成后通知，取消能终止；事件顺序与 `Seq` 正确。
4. **steering**：运行中从输入区补充指令，下一轮请求带上；事件流出现 `steering_applied`；空闲时调用返回明确错误。
5. **UI 降级**：`rpc-demo` 调 `setWidget/setTitle/setStatus` 与快捷键返回零值、扩展继续、每成员一条诊断；`notify` 进通知区；select/confirm/input/editor 弹原生对话框。
6. **缓做项**：`ctx.executeTool`/`sendMessage`/`spawn_subagent` 等返回 `not_implemented` 诊断且不中断。
7. **技能**：用户技能自动可用；项目技能未信任不加载、信任后加载、撤销后新会话消失；诊断可见。
8. **故障**：扩展抛异常/崩溃/超时/哈希不符，宿主记录并禁用该扩展，其他扩展与会话不受影响；工具冲突只禁用冲突方。
9. **回归**：Phase 1 事件 fixture、Phase 2 协议/三分类、Phase 3/4/5 冒烟全绿；Node 目录缺失时 A 轨禁用但桌面可用。

## 6. 交付物

- `internal/extensions`（发现/信任/生命周期/工具/命令/事件/诊断/通知）与 bridge/kernel 接口补齐。
- shim、构建工具、Node 运行时目录说明与校验；真实样本构建记录（不泄密钥/个人路径）。
- 桌面扩展状态面板、信任/撤销、通知/命令/技能段；基础对话框映射。
- Go 单测/集成测试、shim 协议 fixture、`orchestrator-sample` 冒烟、真实 `todo` 手动/半自动记录。
- Phase 6 报告：三分类最终矩阵、真实读数、已知差异、Phase 7 建议。

## 7. 风险

- **shim 面**：真实扩展可能用到未覆盖成员；只承诺矩阵，不临时扩表；诊断必须可见。
- **链式改写缓做**：某些扩展的上下文改写不生效；用 `not_implemented` 诊断，不假装支持。
- **信任边界**：扩展可绕过 shim 直接用 Node fs/net/exec；只能按受信任代码处理，界面必须明示。
- **Node 目录体积/签名**：ADR-0015 已定目录；发布工程与安装包另行处理。
- **工具闸门**：`tool_call` 无响应时的 fail-open/fail-safe 在实现前定，默认 fail-open + 诊断。
- **真实样本依赖**：上游样本构建需要网络/本地检出；CI 用自写样本，真实样本记录人工结果。
- **技能效果**：自动化只能断言注入，模型是否使用留真实 provider 冒烟。
