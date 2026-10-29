# H-025：Phase 6 故障隔离与技能信任

- 日期：2026-10-30
- 主题：崩溃/超时/工具重名夹具与测试、技能信任链路

## 本轮做了什么

- 管理器：`Extensions()` 跟随适配器状态——进程失败时状态行转 `failed` 并用 `Err()` 填原因，恢复后回到 `running`。
- 适配器：工具调用超时（`ErrTimeout`）记录 `invoke_tool`/`timeout` 诊断，不再只有调用方看到错误。
- 新增 `fixtures/faults/`：
  - `crash`：命令 `crash` 直接 `process.exit(7)`，另有一个正常工具。
  - `slow`：`fault_slow` 永不返回（`timeoutMs: 1000`），`fault_ok` 证明进程仍活着。
  - `tool-conflict`：注册 `sample_echo`，与 `orchestrator-sample` 冲突。
- 测试：`TestCrashedExtensionIsDisabledAndIsolated`、`TestToolTimeoutRecordsDiagnostic`、`TestToolConflictDisablesOnlyTheSecond`。
- 技能信任：`TestSkillsTrustRoundTrip`（bridge 层，`Skills`/`SetProjectTrust` 面板同一调用链）：未信任只列用户技能 → 信任后出现项目技能 → 撤销后隐藏。
- 打包工具：`extension.json` 读取兼容 BOM（PowerShell 写文件留下的坑）。
- CI：夹具 `--check` 循环覆盖 `fixtures/faults/*`。

## 关键结果

- 崩溃：状态 failed、`crashed` 诊断、兄弟扩展工具仍可用（约 0.4 s）。
- 超时：1 秒内失败并留 `timeout` 诊断，随后 `fault_ok` 正常（约 1.2 s）。
- 重名：后注册方 failed，原因 `tool "sample_echo" is already provided by orchestrator-sample`。
- `go test -race ./...`、夹具 `--check`、`check-docs` 全绿。

## 遗留（Phase 6 未收尾）

1. 崩溃时的孙进程清理（现有测试只覆盖主动停止）。
2. “扩展抛异常即禁用”未单独注入（handler 异常目前只记诊断）。
3. 发布构建缺运行时 fail-closed；`ctx.sessionManager` 会话条目回放。
4. 面板按钮级冒烟（当前覆盖到回调与数据层，未点原生按钮）。

## 已提取的沉淀

- 新增 P-012：插件注册被拒必须落诊断（超时夹具的 `timeoutMs: 300` 曾静默丢工具）。
