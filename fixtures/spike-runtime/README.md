# A 轨运行时 spike 夹具

Phase 2 的运行时比试（ADR-0008）。**不是生产路径**：`internal/adapter` 不引用这里的任何代码，结论见 [runtime-spike.md](../../docs/design/phase-2/runtime-spike.md) 与 ADR-0009。

## 组成

| 路径 | 作用 |
|---|---|
| `samples/` | 三个自写探针，按上游示例的用法覆盖工具、事件、notify、TUI no-op、Node 内置模块 |
| `shim/` | 显式 pi API 子集（`ExtensionAPI` / `ctx.ui` / `sessionManager` / pi-tui / typebox / node 能力桩），每个成员带状态：`supported` / `stub` / `noop` / `deferred` |
| `cmd/bundle` | 用 esbuild 把样本 + shim 打成单文件；neutral = A 轨，node = B 轨 |
| `cmd/host-a` | QuickJS 宿主（`modernc.org/quickjs`，纯 Go 无 cgo），JSONL 协议 |
| `runner/host-b.mjs` | Node 宿主，同一协议、同一 shim |
| `cmd/host-c` | Go 重写对照（只重写探针语义） |
| `cmd/measure` | 冷启动 10 次取中位数、RSS、一致性检查、覆盖率 |
| `results/` | 2026-10-28 的读数（Windows 11 / Go 1.27.3 / Node v24.14.0） |

## 复现

```bash
cd fixtures/spike-runtime
go build -o build/bundle.exe ./cmd/bundle
go build -o build/host-a.exe ./cmd/host-a
go build -o build/host-c.exe ./cmd/host-c
go build -o build/measure.exe ./cmd/measure

./build/bundle.exe --sample samples/ui-probe.ts --out build/ui.a.js --platform neutral
./build/bundle.exe --sample samples/ui-probe.ts --out build/ui.b.js --platform node
./build/measure.exe --route a --sample ui --bundle build/ui.a.js --runs 10
./build/measure.exe --route b --sample ui --bundle build/ui.b.js --runs 10
./build/measure.exe --route c --sample ui --runs 10
```

真实样本从上游固定提交取用，**不入库**（AGENTS 铁律 7）：`earendil-works/pi` 提交 `38ff99177fba21e9c57a670f4e95adcf407c99c7` 的 `packages/coding-agent/examples/extensions/{rpc-demo.ts,todo.ts,subagent/index.ts}`，用同样的 `bundle` 命令打包后以 `--sample real-rpc-demo|real-todo|real-subagent` 运行。

## 边界

- 样本与 shim 只验证“能否真实执行 + 三分类诊断”，不验证完整 pi 语义。
- 能力桩（`node-fs` / `node-child_process`）用于暴露 A 轨缺口，不构成可用实现。
- 不测长会话、并发多插件、真实模型调用。
