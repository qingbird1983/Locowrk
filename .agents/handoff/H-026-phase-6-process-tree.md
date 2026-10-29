# H-026：Phase 6 进程树、工厂异常与发布 fail-closed

- 日期：2026-10-30
- 主题：作业对象清理孙进程、工厂异常禁用、发布缺运行时 fail-closed

## 本轮做了什么

- Windows 进程树：`proc_windows.go` 用作业对象（`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`）在插件启动后托管它；`peer.kill()` 终止作业并等待插件退出（OpenProcess+WaitForSingleObject），释放句柄。非 Windows 仍用进程组（`Setpgid` + `kill(-pid)`）。
- `watchExit` 在检测到进程退出后立即清理进程树（崩溃也会带走孙进程）。
- 工厂异常：适配器收到 `factory_error` 诊断即置 failed 并杀进程；`Start` 不再把 failed 状态覆盖回 ready；管理器保留失败的适配器以便显示诊断，但 `Tools/Commands/Gate/Confirm` 只使用 ready 的适配器。
- 发布 fail-closed：`extensions.Options.RuntimeRequired`（由 `!mygo.IsDev()` 决定）；缺 `NodePath` 时整条 A 轨标记失败并说明原因；启动器同时打印一条提示。
- 夹具：新增 `fixtures/faults/grandchild`（子进程写心跳文件）与 `fixtures/faults/throw`（工厂抛错）；`mock-orchestrator` 的孙进程不再继承协议 stdout（P-013）。
- CI：`--check` 覆盖 6 个夹具。

## 关键结果

- `TestCrashKillsGrandchildren`：崩溃后心跳文件停止增长（约 1.9 s）。
- `TestFactoryErrorDisablesExtension`：failed + `factory_error`，工具数为 0（约 0.3 s）。
- `TestMissingRuntimeFailsClosed`：所有候选 failed，原因含运行时说明，工具为空。
- 发布模式实测：临时目录二进制 + `MYGO_ENV=production` 打印提示、冒烟通过；仓库根（有 `runtime/node`）无提示。
- 回归修复：`TestStopKillsProcessTree` 与 `TestHashChangeLosesTrust` 恢复通过。
- `go test -race ./...`、6 个夹具 `--check`、`check-docs`、plugin 冒烟全绿。

## 遗留（Phase 6 未收尾）

1. 面板按钮级冒烟（当前覆盖到回调与数据层）。
2. `ctx.sessionManager` 会话条目回放；阻断后的批终止（`Terminate`）。
3. Phase 6 定稿报告与 `phase-6.end` tag。

## 已提取的沉淀

- 新增 P-013（子进程 stdout 是协议通道）；P-006 补记两次 PowerShell 写文件事故。
