# 工作上下文交接卡

- 更新：2026-10-28（会话主题：**Phase 2 适配器、mock 与故障注入落地**）

## 当前阶段与状态

- **项目名：Locowrk**（拼写是 `wrk`，不是 `work`/`wrok`；不加 `-pi` 后缀）。远端：`github.com/qingbird1983/Locowrk`（**私有**）；旧 Rust 历史在 `archive/pre-go-locowrk`。
- **本地目录**：`E:\Warehouse\AIwork\Locowrk`；Go module `github.com/qingbird1983/locowrk`。
- **Phase 0 / Phase 1**：✅ 完成，tag 已打。
- **Phase 2**：🚧 进行中，实现主体已落地：
  - ✅ 协议（`proto/`）与 manifest schema；运行时定稿（ADR-0009，Node 子进程）。
  - ✅ `internal/adapter`：启动/握手/注册/心跳/超时/崩溃重启与禁用、三分类、审计、事件转发。
  - ✅ `fixtures/plugins/mock-orchestrator`：支持项、缓做项、TUI no-op、越权写、崩溃、挂起。
  - ✅ `internal/kernel` 外部工具桥：JSON Schema → 上游 `agent.AgentTool`，faux 端到端测试。
  - ⏳ 待做：Phase 2 验收逐条复核、孙进程管道清理测试、Phase 2 报告与 tag。
- **远端**：`origin` 为 SSH；`main` 与 tag 已推送。看板唯一来源：`docs/roadmap/00-roadmap.md`。

## 上一轮完成了什么

1. 协议补四个方法并同步文档：`invoke_tool`（宿主调用插件工具）、`ui_notify`、`ui_set_widget`（TUI no-op）、`fs_write`（能力闸代写）。
2. `internal/adapter` 按 500 行/文件拆分为 manifest / protocol / peer / handlers / lifecycle / registry / rpc_api / security。
3. 三分类与诊断：支持项真执行；`spawn_subagent` 返回 `not_implemented`；`ui.setWidget` 零值 + `unsupported`，同成员只累加 `count`。
4. 安全与兼容分账：未声明/越界的写请求返回 `permission_denied` 并进独立审计；插件 stderr 做密钥脱敏。
5. 故障注入：启动即崩溃、运行中崩溃（含一次重启）、RPC 超时、心跳丢失、停止后可重启；宿主始终存活。
6. 内核桥：`kernel.ToolSpec` 转 `agent.AgentTool`，参数 schema 交给上游校验；faux 回合证明模型能调用插件工具并落历史。
7. 顺手修了 `eventbus` 的 flaky 测试：合并断言改为直接测 `enqueue`，观察性测试只校验顺序与内容。

## 下一步（按优先级）

1. **孙进程占管道测试**：mock 起一个继承 stdout 的子进程，Stop 必须杀进程树、读取有界。
2. **Phase 2 验收复核**：逐条对照 §5，产出 `docs/design/phase-2/report.md`；通过后打 `phase-2.end`。
3. **Node SEA / 随包运行时**（Phase 6 前）。
4. **真实 provider 冒烟**（有 key 环境，非阻塞）。
5. **Phase 3 开工前**：定主界面渲染路径（原生 `ui.View` vs WebView），必要时新增 ADR。

## 未决问题

- **`Stop(ctx)` 当前忽略 ctx**：关闭用固定 `ShutdownTimeout`；是否让调用方 ctx 也参与，等 Phase 3 门面统一。
- **能力闸的真实边界**：B 路线下插件可绕过 shim 直接调用 Node fs/net/exec；OS 级隔离是否做、何时做。
- **迟到注册**：当前 `ready` 之后拒绝注册并记诊断；真实扩展若异步注册会丢工具，Phase 6 接入时复核。
- **`kernel.Prompt` 是阻塞调用**：Phase 3 需在它之外加 `Start`/`Wait` 异步门面。
- **事件 DTO v0**：`tool_execution_*` 的图片结果暂只传文本；Phase 3 展示图片时新增字段（向后兼容）。
- **`sky-valley/pi` 版本策略**：固定提交；升级独立提交并全量 `-race`（ADR-0001）。
- **MyGo v0.x API 漂移**：用法限制在 `desktop/` 与 `internal/bridge`。

## 关键提醒

- **项目名是 `Locowrk`**：`wrk`，不是 `work`/`wrok`；远端私有，未经所有者允许不得转公开或外发。
- **ADR 不可改**：`docs/adr/` 只增不改；状态类事实写 CONTEXT/看板/CHANGELOG（`pitfalls/P-001`）。
- **上游只出现在 `internal/kernel`**：适配器工具用项目 DTO，转换在 `kernel.ToolSpec`（接缝 #12）。
- **A 轨 = 受信任代码**：进程隔离只防崩溃；B 路线拦不住 Node 原生 fs/net/exec，安装与运行界面必须明示。
- **协议新增方法是兼容变更**：不改 `protocol`；改语义/删字段才 +1，并同步 `proto/`、CHANGELOG、Phase 文档。
- **请求处理不得重入**：`peer` 的读循环同步处理插件请求；处理器里再 `call` 会死锁（mock 的 `spawn_subagent` 走 goroutine）。
- **JSON-RPC id 规范化**：数字与字符串 id 统一按去引号比较，否则插件响应匹配不上（`pitfalls/P-004`）。
- **测试必须换主目录**：上游把会话/技能写死在 `~/.pi`，用 `testHome(t)`（`pitfalls/P-002`）。
- **faux 优先**：无 key 路径用 `providers.RegisterFauxProvider`；不在测试里依赖真实模型。
- **不写自己的会话格式**：走 `coding.SessionRecorder` / `LoadSessionTree` + `LoadBranch`（ADR-0005）。
- **项目技能默认不加载**：`<cwd>/.pi/skills` 需显式 `TrustProject`；桌面端必须用户可见授权。
- **提交前门禁**：`gofmt -l . && go vet ./... && go test -race ./...` 与 `bash scripts/check-docs.sh`；spike 模块单独 `go vet`。
- **体量**：docs 单篇 ≤300 行、ADR ≤100 行、代码单文件 ≤500 行、CONTEXT ≤150 行。
- 踩坑索引：`P-001`、`P-002`、`P-003-short-sha-fetch.md`、`P-004-jsonrpc-id-normalization.md`；方法：`skills/S-001-runtime-comparison-spike.md`。
