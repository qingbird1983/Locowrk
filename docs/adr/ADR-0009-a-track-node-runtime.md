# ADR-0009：A 轨采用 Node 子进程运行时

- 状态：定稿
- 日期：2026-10-28
- 关联：ADR-0003、ADR-0004、ADR-0008；architecture/04；research/02；design/phase-2/runtime-spike.md

## 背景

ADR-0008 要求在 Phase 2 用限时 spike 比较三条路线，并在退出前定稿。spike 用固定上游提交 `38ff99177fba21e9c57a670f4e95adcf407c99c7` 的三个真实样本（rpc-demo、todo、subagent）与三个自写探针，跑同一份 shim 与协议，读数见 `docs/design/phase-2/runtime-spike.md`。

结论摘要：A（QuickJS）冷启动 14ms、常驻 9.2MiB，最小一致性路径全过；但编排样本依赖 `node:child_process` / `node:fs` / `node:os` / `node:path`，A 执行该样本失败。B（Node 子进程）冷启动 55~63ms、常驻约 49MiB，全部样本通过。C（Go 重写）只有重写过的探针能过。

## 决策

A 轨运行时定为 **B：Node 兼容子进程 + 显式子集 shim**。

- 扩展在独立 Node 子进程中运行，宿主经 JSONL/JSON-RPC 提供 `ExtensionAPI` shim；进程边界只用于崩溃与资源隔离。
- shim 是**显式声明的子集**，带独立版本标记（`locowrk-shim/0`），不导入 `~/.pi`，不声称全兼容；每成员按 ADR-0003 三分类：支持、缓做（`not_implemented`）、TUI no-op（`unsupported`），永不 throw。
- 安装期用 esbuild 把扩展与 shim 打成单文件（Node 内置模块保持 external）；运行期禁止 `npm install`、禁止远程加载代码。
- Node 运行时随产品分发并锁版本；分发形态（随包目录或 Node SEA）在 Phase 6 开工前定，不把 SEA 当已解决问题。
- 对用户的口径：A 轨扩展是**受信任代码**，与 Agent 同权；不是安全沙箱。

## 依据

| 依据 | 读数/事实 |
|---|---|
| 真实样本可执行 | 三个上游样本 B 全部加载；rpc-demo、todo 的执行路径通过，subagent 只验加载（执行需 `pi` 与特权，非本轮范围） |
| 淘汰线 | B：冷启动 62.6ms（及格 ≤500ms）、RSS 49.6MiB（及格 ≤100MiB）、Windows 全过、维护 ≤2 人日/季度 |
| A 的硬缺口 | 编排样本执行失败于 `node:fs.promises.mkdtemp`；补齐需宿主实现 Node 语义的 fs/spawn（流、退出码、取消、进程树），维护成本判 >5 人日/季度 |
| C 的硬缺口 | 只对重写过的探针成立；每个扩展都要重写，兼容面从“运行扩展”退化为“编排语义兼容” |
| 打包可复现 | 单文件产物 16~58KB，构建无网络、无运行期安装 |

## 理由

- 兼容矩阵的主干是工具、命令、事件钩子与**子代理/workflow**；后者的现实写法就是 Node 内置模块。选 A 等于把 Node 生态的坑搬进适配层。
- A 的速度与内存优势真实，但不在淘汰线上；为省 40MiB 去重造 fs/spawn 语义，长期成本高于收益。
- B 的代价（随包带 Node）是一次性的分发问题，可用 SEA 或运行时目录解决；A 的代价是持续维护。
- 三条路线的安全边界相近：进程隔离不等于沙箱；B 下 Node 原生能力更直白，因此把“仅受信任插件”写成产品口径，而不是假装拦得住。

## 后果

- 每个 A 轨插件多一个 Node 进程：约 49MiB 常驻、55~65ms 冷启动；长会话内存另测。
- 分发体积增加（Node 运行时）；安装包签名与升级流程需覆盖运行时版本。
- **能力闸的边界要说清**：manifest 的 `fs`/`net`/`exec` 声明仍是契约与审计依据，但 B 下扩展可绕过 shim 直接调用 Node API；未加 OS 级隔离前，只能按受信任代码处理，UI 必须提示。
- `ctx.ui.custom`、`setWidget/setFooter/setHeader/setTitle/setStatus`、快捷键、自定义 renderer 继续按 TUI no-op + 诊断；基础对话框映射宿主。
- A（QuickJS）与 C（Go 重写）不进入生产路径；A 若将来作为“UI 型扩展快路径”重启，须新 ADR 并重新跑本报告的场景。

## 护栏

- Node 版本钉死；升级独立提交，重跑 spike 的场景与 Phase 2 适配器测试。
- 扩展只执行安装期打包产物；扩展依赖的锁文件入库，构建可复现。
- 子进程退出/崩溃不影响宿主；清理进程树（含孙进程），读取有界；超时与心跳按协议。
- 诊断每插件每成员一条、`count` 累加；安全拒绝单独记录，不与兼容诊断混用。
- 项目内扩展默认不加载，须显式信任工作区。
- shim 表以固定上游提交为蓝本；上游 API 变化先改 research/02 与矩阵，再改 shim。

## 重新评估触发条件

1. Node SEA 或随包运行时在三平台不可行，且体积/签名不可接受；
2. 上游提供与运行时解耦的扩展协议（不再依赖 Node 内置模块）；
3. 出现“只装 UI 型扩展、要求 <10MiB 常驻”的明确产品需求。
