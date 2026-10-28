# 工作上下文交接卡

- 更新：2026-10-28（会话主题：**Phase 2 A 轨运行时 spike 定稿**）

## 当前阶段与状态

- **项目名：Locowrk**（拼写是 `wrk`，不是 `work`/`wrok`；不加 `-pi` 后缀）。远端：`github.com/qingbird1983/Locowrk`（**私有**）；旧 Rust 历史在 `archive/pre-go-locowrk`。
- **本地目录**：`E:\Warehouse\AIwork\Locowrk`；Go module `github.com/qingbird1983/locowrk`。
- **Phase 0**：✅ 完成，tag `phase-0.end`。
- **Phase 1（内核接入与事件流）**：✅ 完成，tag `phase-1.end`。
- **Phase 2（编排适配器与 A 轨运行时选型）**：🚧 进行中。
  - ✅ 协议与 manifest schema 定稿（`proto/`）。
  - ✅ **运行时定稿：B 路线（Node 兼容子进程 + 显式子集 shim）**，ADR-0009；报告 `docs/design/phase-2/runtime-spike.md`。
  - ⏳ 待做：`internal/adapter`、mock 插件、故障注入、三分类验收、Phase 2 报告。
- **远端**：`origin` 为 SSH（HTTPS 在本机被重置）；`main` 与 tag 已推送。看板唯一来源：`docs/roadmap/00-roadmap.md`。

## 上一轮完成了什么

1. 从 GitHub 取上游 `earendil-works/pi` 固定提交 `38ff99177fba21e9c57a670f4e95adcf407c99c7`（只读出，不入库）。
2. 建 `fixtures/spike-runtime/`：共享 shim（成员带 supported/stub/noop/deferred 状态）、三个自写探针、esbuild 打包器、QuickJS 宿主、Node 宿主、Go 重写对照、测量器。
3. 同一协议跑三条路线：A 14ms/9.2MiB（编排样本执行失败）、B 55~63ms/49MiB（全部通过）、C 7ms/6.8MiB（仅重写项）。
4. 上游真实样本：rpc-demo、todo 在 A/B 均可运行；subagent 在 A/B 均可加载注册（执行需 `pi` 与特权，未跑）。
5. 复核上游扩展 API（41 事件、工具注册、`ctx.ui` 方法集），更新 research/02 与 architecture/04；ADR-0009 定稿。

## 下一步（按优先级）

1. **`internal/adapter`**：按 `proto/plugin-protocol.md` 实现 Load/Start/Events/SendCommand/Tools/Stop；Node 子进程生命周期、超时、心跳、进程树清理。
2. **mock 插件**：`fixtures/plugins/mock-orchestrator`，含 `single`/`parallel`/`deferred`/`tui-noop`/`crash`/`hang` 模式；三分类与故障注入用例。
3. **Phase 2 验收**：逐条跑 §5 场景，写 `docs/design/phase-2/report.md`；再决定是否打 tag。
4. **Node SEA / 随包运行时**：Phase 6 开工前定分发形态（ADR-0009 的未验证项）。
5. **真实 provider 冒烟**：找有 key 的环境跑一次（非阻塞）。
6. **Phase 3 开工前**：定主界面渲染路径（原生 `ui.View` vs WebView），必要时新增 ADR。

## 未决问题

- **Node 分发形态**：SEA 单文件 vs 随包运行时目录；影响安装包体积、签名与升级。
- **shim 兼容矩阵落地**：spike shim 只是骨架（真做 2~4 个成员）；接入时要逐成员补实现与诊断。
- **能力闸的真实边界**：B 下扩展可绕过 shim 直接调用 Node fs/net/exec；OS 级隔离是否做、何时做。
- **`kernel.Prompt` 是阻塞调用**：Phase 3 需在它之外加 `Start`/`Wait` 异步门面，不改本阶段语义。
- **事件 DTO v0**：`tool_execution_*` 的图片结果暂只传文本；Phase 3 展示图片时新增字段（向后兼容）。
- **`sky-valley/pi` 版本策略**：固定提交；升级规则见 ADR-0001，升级独立成提交并全量 `-race`。
- **MyGo v0.x API 漂移**：用法限制在 `desktop/` 与 `internal/bridge`，内核不依赖 GUI。

## 关键提醒

- **项目名是 `Locowrk`**：`wrk`，不是 `work`/`wrok`；远端私有，未经所有者允许不得转公开或外发。
- **ADR 不可改**：`docs/adr/` 只增不改；状态类事实写 CONTEXT/看板/CHANGELOG（`pitfalls/P-001`）。
- **上游只能出现在 `internal/kernel`**：其他包只认 `eventbus` 的 DTO；`internal/eventbus` 也不得 import 上游。
- **上游代码不搬运**：真实样本只在临时检出里跑；仓库只留自写夹具与读数（AGENTS 铁律 7）。
- **A 轨 = 受信任代码**：能注册工具/钩子就等同 Agent 权限；进程隔离只防崩溃，不是沙箱。**B 路线拦不住 Node 原生 fs/net/exec**，安装与运行界面必须明示。
- **事件契约**：生命周期事件与 `agent.Ev*` 一一对应，不自造；只新增编排级事件；`Seq` 由 bus 按会话分配，消费端靠它补洞。
- **Agent 循环不可阻塞**：`Bus.Publish` 不得阻塞；慢订阅者只影响自己（见 bus 测试）。
- **测试必须换主目录**：上游把会话/技能写死在 `~/.pi`，测试统一用 `testHome(t)`（`pitfalls/P-002`）。
- **faux 优先**：无 key 路径用 `providers.RegisterFauxProvider` 脚本化响应；不要在测试里依赖真实模型。
- **不写自己的会话格式**：读写走 `coding.SessionRecorder` / `LoadSessionTree` + `LoadBranch`（ADR-0005）。
- **项目技能默认不加载**：`<cwd>/.pi/skills` 需显式 `TrustProject`；桌面端必须用户可见授权。
- **插件协议**：线上形状以 `proto/plugin-protocol.md` 为准；错误码区分 `unsupported`/`not_implemented`/`permission_denied`。
- **提交前门禁**：`gofmt -l . && go vet ./... && go test -race ./...` 与 `bash scripts/check-docs.sh`；spike 模块单独 `go vet`。
- **文档门禁**：新增文档必须登记 `docs/README.md`，否则 `check-docs.sh` 失败。
- **体量**：docs 单篇 ≤300 行、ADR ≤100 行、代码单文件 ≤500 行、CONTEXT ≤150 行。
- 踩坑索引：`pitfalls/P-001`、`P-002`、`P-003-short-sha-fetch.md`；方法：`skills/S-001-runtime-comparison-spike.md`。
