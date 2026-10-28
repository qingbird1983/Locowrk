# H-019：Phase 6 设计定稿

- 日期：2026-10-28
- 主题：真实编排扩展、Node 运行时、扩展信任与技能信任

## 本轮做了什么

- Phase 5 收尾完成：报告定稿、tag `phase-5.end`、CHANGELOG 0.10.0。
- ADR-0015：随包 Node 运行时目录，不用 SEA；适配器新增 NodePath，发布缺运行时 fail-closed。
- `design/phase-6/runtime.md`：扩展包格式、esbuild 构建、`locowrk-shim/1` 成员矩阵、`tool_update`/`event_result`、真实样本策略。
- `design/phase-6/extensions.md`：用户/项目发现、设置 v2 哈希信任、Manager、工具/命令/事件、steering、原生扩展状态面板、基础对话框。
- `design/phase-6/skills.md`：用户技能自动、项目 `.pi/skills` 信任、撤销只影响后续会话、诊断与 DTO。
- 更新 Phase 6 路线图、看板、docs 地图、CHANGELOG 0.10.1、CONTEXT。

## 关键边界（实现时不要丢）

- 扩展 = 受信任代码；哈希变化回未信任；项目扩展与技能信任分开。
- 宿主不代管子代理；扩展自行 spawn 并上报事件；`spawn_subagent` 保持 not_implemented。
- 链式上下文改写、`ctx.executeTool`/`sendMessage` 等缓做，必须有诊断且不 throw。
- steering 走 kernel.Steer；协议 `steer` 在 Phase 6 不使用。
- 用户技能自动加载；项目技能进入系统提示词前必须显式信任。

## 开工前置

1. 下载/校验 `runtime/node/`，确认 Windows 启动扩展宿主。
2. 构建 `todo`/`rpc-demo`（上游固定提交，不入库）；写 `orchestrator-sample`。
3. 设置 v1→v2 迁移与信任哈希。

## 已提取的沉淀

- 沿用 P-009/P-008；本轮无新坑。
