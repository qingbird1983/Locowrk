# H-027：Phase 6 收口

- 日期：2026-10-30
- 主题：验收 2/3/6 补齐、体量拆分、报告定稿与 tag

## 本轮做了什么

- 补验收：
  - `fixtures/extensions/orchestrator-sample` 增加 `sample_chain`（三步顺序 + 异步通知）、`sample_chain_long`（可中止）、`sample_deferred`（调用全部缓做/无终端成员）；`sample_events` 改为后完成者后 settle。
  - shim 收到 `abort` 时先分发处理器再回执；管理器新增 `AbortAll`，桌面 `Stop` 接到它（`Desktop.stop` = 内核中止 + 扩展中止）。
  - 新增 `internal/extensions/orchestration_test.go`：并行顺序与 `Seq`、链式/异步/取消、缓做诊断。
- 体量拆分（standards/08）：`manager.go` 拆为 manager/registry/gate/trust 四个文件；`cmd/locowrk/main.go` 拆出 `smoke.go`；`kernel.go` 拆出 `gate.go`；`measure` 夹具登记豁免。
- 收口文档：报告重写为定稿（九项验收、读数、体量核对、规范复查、下阶段影响），看板 Phase 6 完成，CHANGELOG 0.11.0，CONTEXT 重写，版本号 0.11.0。

## 关键结果

- 全部门禁与冒烟：`go test -race ./...`、web `check/build/test`、tools `npm test`、6 个夹具 `--check`、`check-docs`、faux/tool/cancel/plugin/diff/steer 冒烟、`steer-real`（DeepSeek）、`TestRealPiSamples` 全部通过。
- 生产代码最大文件 500 行（`web/src/transcript.ts`）；`manager.go` 最长为 362 行。

## 遗留

1. 面板按钮级冒烟、页面“本会话已注入”提示（UI 重设计时一起做）。
2. `ctx.sessionManager` 会话条目回放；`tool_call` 的 `Terminate` 与页面内切换确认。
3. 发布工程（签名/安装包/钥匙串/CI 打包）。

## 已提取的沉淀

- P-013（子进程 stdout 是协议通道）、P-012（注册被拒要落诊断）、P-011（steering 时机）、P-006 补记；本轮无新坑。
