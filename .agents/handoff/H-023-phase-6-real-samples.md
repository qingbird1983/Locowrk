# H-023：Phase 6 真实上游样本接入

- 日期：2026-10-30
- 主题：`todo`/`rpc-demo` 打包运行、shim 兼容面、两处静默失败修复

## 本轮做了什么

- 经本机代理（`127.0.0.1:7897`）取回 `earendil-works/pi@38ff99177fba21e9c57a670f4e95adcf407c99c7` 的 `packages/coding-agent/examples/extensions/{todo.ts,rpc-demo.ts}`（不入库，放在 `%TEMP%\locowrk-samples\`），加 `extension.json` 后用 `tools/extension-build` 打包。
- `extensions/shim/compat/`：新增 `pi-ai.mjs`（`StringEnum`）、`pi-tui.mjs`（`Text`/`truncateToWidth`/`matchesKey` 等惰性组件）、`typebox.mjs`（输出纯 JSON Schema 的 `Type` 构建器）；`build.mjs` 把这三个 specifier 映射到兼容模块并随包内联。
- shim：`registerTool`/`registerCommand` 同时接受对象与 `(name, options)` 两种签名；事件处理器补第二个参数 `ctx`；`ctx.sessionManager` 提供 `getBranch/getEntries/getLeafId`（返回空，不抛错）。
- 适配器：命令/工具名允许连字符（`rpc-input` 之前被静默拒绝）；插件通知被拒绝时记 `member=<method> code=rejected` 诊断（此前无声丢弃）。
- 新增 `internal/extensions/real_samples_test.go`（`TestRealPiSamples`）：由 `LOCOWRK_REAL_SAMPLE_DIR` 触发，默认跳过；覆盖 todo 工具 list/add/list、`/todos` 命令、rpc-demo 3 命令、`setTitle/setWidget/setStatus` 诊断、input/editor 对话框与通知回写。
- `fixtures/extensions/orchestrator-sample/bundle.mjs` 重新打包（含新 shim）；`--check` 通过。

## 关键结果

- `TestRealPiSamples`：通过（约 0.5 s）；todo bundle 19,327 B、rpc-demo 11,333 B，均自包含 shim。
- `TestHyphenatedCommandNamesAndRejectedNotifications`：通过。
- `go test -race ./...`、`npm test`（tools）、夹具 `--check`、`check-docs`：全绿。

## 遗留（Phase 6 未收尾）

1. `select`/`confirm` 的触发事件未跑（tool_call 闸门回执 `wantsEventAck` 未接线；`session_before_switch` 无宿主触发点）。
2. 技能信任/撤销 UI 冒烟；真实 Node 崩溃/超时/进程树/工具冲突；发布缺运行时 fail-closed。
3. `ctx.sessionManager` 无真实会话条目，依赖历史重建的扩展状态会丢。
4. 样本复现依赖网络/代理；CI 不跑真实样本。

## 已提取的沉淀

- P-010 追加：本机代理用法与端口探测；真实样本不要用 spike 旧 bundle 顶替。
