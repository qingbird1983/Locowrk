# H-020：Phase 6 基础设施与 shim 样本

- 日期：2026-10-29
- 主题：A 轨管理器、信任、桥接、shim 与原生面板

## 本轮做了什么

- `internal/extensions`：发现用户/项目/开发目录；SHA-256 信任；启停/撤销；工具、命令、事件、通知、诊断、会话切换；测试含哈希失效。
- 设置 v2：扩展路径/哈希/开关/信任时间、工作区信任；v1 迁移；`desktop.Host` 作为 Store。
- adapter：`NodePath`、`SetSession`、`SendHostEvent`、`Abort`、`HostUI`（select/confirm/input/editor）、`tool_update`、shim 版本校验。
- kernel/bridge：`Skills`；`ExtraTools`、`Steer`、技能/扩展/命令 DTO 与回调。
- 桌面：原生“扩展与技能”面板；页面 busy 输入改为补充指令。
- shim `extensions/shim/host.mjs` + `fixtures/extensions/orchestrator-sample`；端到端测试通过。

## 关键结果

- `TestNodeSampleEndToEnd`：sample_echo 工具、sample_events 命令、subagent/workflow 事件通过。
- `go test -race ./...`、`npm run check/build/test`、`check-docs` 通过；diff/plugin 冒烟通过。
- 设置 v1→v2、哈希变化回未信任测试通过。

## 遗留（Phase 6 未收尾）

1. 构建并验证上游真实 `todo`/`rpc-demo`（固定提交，不入库）；shim 面不足时补诊断/成员。
2. 实现 `desktop.HostUI`，把原生对话框接到 adapter；当前 select/confirm 等会降级诊断。
3. 技能信任/撤销冒烟；steering 真实运行冒烟；真实 Node 崩溃/超时/进程树。
4. `runtime/node/` 下载/校验；发布缺运行时 fail-closed。
5. 扩展面板通知/命令参数/撤销按钮细节。

## 已提取的沉淀

- 沿用 P-009/P-008；无新坑。
