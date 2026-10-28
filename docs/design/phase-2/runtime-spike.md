# Phase 2：A 轨运行时 spike 报告

- 状态：定稿（2026-10-28）
- 日期：2026-10-28
- 关联：ADR-0008（比尺）、ADR-0009（结论）、architecture/04、research/02、roadmap/phase-2
- 复现：`fixtures/spike-runtime/`（命令与边界见其 README）

## 1. 结论

**选定 B 路线：Node 兼容子进程 + 显式子集 shim**（ADR-0009）。

一句话理由：A（QuickJS）能跑最小一致性路径，也明显更快更省，但**编排样本**依赖 `node:child_process` / `node:fs` / `node:os` / `node:path`；A 要补的不是几个 shim 成员，而是宿主侧的 Node 语义（流式 spawn、临时文件、路径、异步 fs），维护成本越过淘汰线。B 在同一 shim 下原生执行这些调用，冷启动 55~63ms、常驻 49MiB，均在及格线内。C（Go 重写）只在两条路都不可行时兜底：它能跑探针，但每个扩展都要重写，兼容面退化为“编排语义”。

## 2. 方法

- 上游样本：`earendil-works/pi` 提交 `38ff99177fba21e9c57a670f4e95adcf407c99c7`（2026-10-24）的 `rpc-demo.ts`、`todo.ts`、`subagent/index.ts`。只读取、不入库；打包时以 `--sample` 指向本地检出。
- 自写探针（入库）：`ui-probe`（工具 + 事件 + notify + `setWidget` no-op）、`state-probe`（session 读 + `ctx.ui.custom`）、`orchestrator-probe`（`spawn` + 临时文件，形状对齐 subagent）。
- 同一份 shim、同一份 JSONL 协议驱动三条路线；宿主 `host-a`（QuickJS）、`host-b.mjs`（Node）、`host-c`（Go 重写）。
- 冷启动 = 进程启动到 `initialize` 返回 ready，10 次取中位数；RSS = 注册完成后对进程工作集采样 5 次取中位数；一致性检查逐项记录。
- 环境：Windows 11 x64、Go 1.27.3、Node v24.14.0；未使用真实模型 key。

## 3. 读数

### 3.1 自写探针

| 样本 | 路线 | 冷启动中位数 | 常驻 RSS | 产物大小 | 一致性 |
|---|---|---|---|---|---|
| ui-probe | A QuickJS | **14.1 ms** | **9.2 MiB** | 17,295 B | 6/6 通过 |
| ui-probe | B Node | 57.0 ms | 48.9 MiB | 16,274 B | 6/6 通过 |
| ui-probe | C Go | 7.5 ms | 6.6 MiB | — | 5/5 通过（仅重写语义） |
| state-probe | A QuickJS | 14.0 ms | 9.1 MiB | 17,429 B | 6/6 通过 |
| state-probe | B Node | 54.5 ms | 48.9 MiB | 16,394 B | 6/6 通过 |
| orchestrator-probe | A QuickJS | 12.3 ms | 9.2 MiB | 18,894 B | **失败：工具执行** |
| orchestrator-probe | B Node | 62.6 ms | 49.6 MiB | 17,952 B | 5/5 通过 |
| orchestrator-probe | C Go | 6.8 ms | 6.8 MiB | — | 5/5 通过（仅重写语义） |

A 的失败点：样本加载、`registerTool`、`session_start` 通知都正常；执行工具时 `node:fs.promises.mkdtemp` 命中能力桩并返回 `not_implemented`。要让它通过，必须在宿主侧实现 fs/spawn 的 Node 语义（见 §5）。

### 3.2 上游真实样本

| 样本 | 路线 | 冷启动中位数 | 常驻 RSS | 产物大小 | 结果 |
|---|---|---|---|---|---|
| rpc-demo.ts | A | 13.6 ms | 9.2 MiB | 18,283 B | 加载、命令注册、`setWidget/setTitle/setStatus` no-op + 诊断 |
| rpc-demo.ts | B | 54.9 ms | 48.9 MiB | 17,248 B | 同上 |
| todo.ts | A | 14.8 ms | 9.2 MiB | 25,509 B | 工具注册、`session_start` 重建、`todo list` 返回 |
| todo.ts | B | 54.0 ms | 48.9 MiB | 24,074 B | 同上 |
| subagent/index.ts | A | 18.1 ms | 9.8 MiB | 57,850 B | **只验证加载**（工具注册成功；执行需 Node 能力，未跑） |
| subagent/index.ts | B | 55.9 ms | 49.8 MiB | 54,917 B | 同上（执行需 `pi` 二进制与特权，未跑） |

subagent 的加载值钱：它把 `node:child_process`、`node:fs`、`node:os`、`node:path`、`typebox`、`pi-tui` 全带上，B 直接打包通过；A 只有给每个内置模块加能力桩才能打包，执行即失败。

### 3.3 shim 覆盖面（样本用到的成员）

| 口径 | 读数 |
|---|---|
| 用到的成员 | 10~16 个/样本（含 `ctx.ui`、`sessionManager`、工具上下文属性） |
| 静默缺失（无诊断） | 0 |
| 明确处理（supported + stub/noop/deferred 诊断） | 100% |
| 真做（仅 supported） | 2~4 个/样本（`on`、`registerTool`、`registerCommand`、`notify`） |

“明确处理 100%”是 spike shim 的记账方式，不等于兼容矩阵已完成：绝大多数成员是 stub/noop，只保证“不抛错、有诊断”。

## 4. 对照淘汰线

| 指标（ADR-0008） | 及格线 / 淘汰线 | A QuickJS | B Node | C Go |
|---|---|---|---|---|
| 冷启动到 ready | ≤500ms / >2s | 14ms ✅ | 63ms ✅ | 7ms ✅ |
| 常驻内存 | ≤100 MiB / >300 MiB | 9.2 MiB ✅ | 49.7 MiB ✅ | 6.8 MiB ✅ |
| Windows 可用性 | 全部场景通过 | 基本路径通过，编排执行失败 ❌ | 全部通过 ✅ | 仅重写项通过（不可比） |
| shim 覆盖面 | ≥80% / <50% | API 面 100%；Node 内置能力 0% ❌ | API 面 100%；内置模块原生 ✅ | 不适用 |
| 打包复杂度 | 无需另装 | 单文件 + 单二进制 ✅ | 需随包分发 Node（约 50MB 常驻；SEA 未验证）⚠️ | 无运行时 ✅ |
| 维护成本 | ≤2 人日/季度 / >5 | **>5**：要重造 fs/spawn 语义 ❌ | ≤2：跟 shim 表追上游 ✅ | >5：每扩展重写 ❌ |

## 5. 关键发现

1. **A 的短板不是速度，是 Node 生态。** 真实编排扩展（子代理是最典型的）用 `spawn` + 临时文件 + 路径处理。QuickJS 里这些只能由宿主桥接；`spawn` 还要求流式 stdout/stderr、退出码、取消与进程树清理——等于在适配层重写 Node 的一角。
2. **B 的代价是“随包带一个 Node”。** 本轮用系统 Node v24.14.0 测；正式分发要么带运行时目录，要么用 Node SEA。SEA 未在本轮验证，列入 Phase 6 开工项，不当作已解决。
3. **三条路线都能过最小一致性路径。** 所以“能加载”不能当结论——ADR-0008 的“必须跑通注册工具→事件→notify→setWidget”已全部满足，真正区分路线的是编排样本。
4. **进程隔离 ≠ 安全沙箱，B 更明显。** Node 下扩展可直接 `require("node:fs")`，manifest 的能力闸拦不住原生调用；只能按“受信任插件”处理，或后续加 OS 级隔离。这条要写进 ADR-0009 与用户可见说明。
5. **shim 是长期资产，不是一次性胶水。** 事件名与签名以固定提交的上游源码为蓝本；本轮已复核 `ExtensionAPI`/`ExtensionUIContext` 的方法集与 41 个事件名（见 research/02 §4）。
6. **打包可复现且无运行期安装。** esbuild 在安装期把扩展与 shim 打成单文件（16~58KB），Node 内置模块保持 external；运行期不执行 `npm install`、不远程加载代码。

## 6. 结论与边界

- 运行时：B（Node 子进程）；shim 是显式子集并带版本标记；每成员三分类，永不 throw。
- A（QuickJS）保留为“未来 UI 型扩展快路径”的研究结论，**不进入生产路径**；要用须新 ADR。
- C 只作为兜底：若将来 B 因分发/许可证不可行，需把对外口径改为“编排语义兼容”。
- 兼容矩阵（ADR-0003）不变；本轮不扩表，只把 `ctx.ui.custom` 明确归入 TUI no-op。
- 安全口径收紧：A 轨扩展 = 受信任代码；未声明能力仍然拒绝，但 Node 原生 fs/net/exec 在 B 下无法被 shim 拦截，必须对用户可见。

## 7. 未验证 / 下一步

| 项 | 处理 |
|---|---|
| Node SEA 单文件分发 | Phase 6 开工前 spike；不通过则随包带运行时目录 |
| 长会话（10k+ 消息）与多插件并发 | Phase 6 性能基准 |
| 带原生模块（`.node`）的扩展 | 明确不支持；manifest 校验拒绝 |
| macOS / Linux 读数 | Phase 6 三平台冒烟清单 |
| 真实模型调用的端到端 | 有 key 环境补一次（非阻塞） |
| `spawn` 孙进程清理 | Phase 2 适配器故障注入已列场景，接 B 后补进程树测试 |
