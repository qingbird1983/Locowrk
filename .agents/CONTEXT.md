# 工作上下文交接卡

- 更新：2026-10-28（会话主题：**内核 fork 进仓库，Phase 3 Bridge 落地**）

## 当前阶段与状态

- **项目名：Locowrk**（拼写是 `wrk`，不是 `work`/`wrok`；不加 `-pi` 后缀）。远端：`github.com/qingbird1983/Locowrk`（**私有**）；旧 Rust 历史在 `archive/pre-go-locowrk`。
- **本地目录**：`E:\Warehouse\AIwork\Locowrk`；Go module `github.com/qingbird1983/locowrk`。
- **Phase 0 / 1 / 2**：✅ 完成，tag 已打（`phase-2.end`）。
- **内核形态**：✅ 已改为仓库内 fork（ADR-0012）。源码在 `pi/`，基线 `sky-valley/pi@6f6131ba`，MIT；维护规则见 `pi/FORK.md`。不再 require 外部内核 module。
- **Phase 3（MyGo 最小宿主）**：🚧 进行中：
  - ✅ 主壳路径定稿：MyGo WebView（ADR-0011）；spike 双向 IPC 约 0.55 s。
  - ✅ `internal/bridge`：DTO-only 门面 + 契约测试；`kernel.ListSessions`；Abort 归类为“已取消”。
  - ⏳ 待做：`cmd/locowrk` 主壳与最小界面（会话列表、消息流、输入、停止、设置）。
- **远端**：`origin` 为 SSH；`main` 与 tag 已推送。看板唯一来源：`docs/roadmap/00-roadmap.md`。

## 上一轮完成了什么

1. **内核进仓库**：把 `agent`、`ai`（含 providers 与模型目录）、`coding`、`internal/jstext`、`internal/nodepath`、`telemetry` 的非测试源码复制到 `pi/`，模块路径改为 `github.com/qingbird1983/locowrk/pi/...`；保留 MIT 许可证，新增 `pi/FORK.md`（基线、范围、差异表、维护规则）。
2. 移除 go.mod 的外部内核依赖；`go build`/`go test -race ./...` 全过；`pi/` 不反向依赖本仓库 `internal/`、`cmd/`。
3. 新增 ADR-0012 取代 ADR-0001；AGENTS 铁律 6/7/9、standards 01/04/06/07、architecture 01/02/03/04/05、research/01、README 两份、看板与 CHANGELOG 全部同步。
4. Phase 3 继续推进：`internal/bridge` 落地（列表/新建/恢复/发消息/停止/历史/模型/思考档/订阅），`kernel.ListSessions`，Abort 统一为 `ErrCanceled`；CHANGELOG 0.4.1。
5. `kernel.WithFauxModel` 收口 faux 冒烟，`cmd/locowrk-smoke` 不再直接 import 内核包。

## 下一步（按优先级）

1. **`cmd/locowrk` 主壳**：MyGo 窗口 + 生成 TS 客户端 + 最小界面（会话列表、消息流、输入、停止）；窗口关闭先取消进行中的请求。
2. **Phase 3 内验证**：中文 IME、5,000+ 消息滚动、钥匙串、安装包体积/签名、三平台差异。
3. **内核本地调整流程**：第一次需求驱动改动时，走 ADR/Phase 文档 + `pi/FORK.md` 差异表 + CHANGELOG。
4. **Node 分发形态**（Phase 6 前）；长会话/多插件基准。

## 未决问题

- **前端产物与 Typed IPC**：WebView 主壳是否一开始就用 `mygo generate`（需要前端工具链），还是先内联页面；Phase 3 内定。
- **内核跟进上游**：安全修复需手工 diff；尚无固定的跟进节奏，建议每阶段末检查一次。
- **`pi/` 体量**：约 2.6 MB / 132 文件，已在 ADR-0012 登记豁免；后续本地改动要防止在 vendored 树里堆业务逻辑。
- **Windows 之外**：MyGo/WebView 只在 Windows 冒烟；Linux/macOS 未验证。
- **能力闸边界**：Node 原生 fs/net/exec 绕过 shim，按受信任插件处理。
- **迟到注册**：`ready` 后拒绝；真实扩展异步注册待 Phase 6 复核。

## 关键提醒

- **项目名是 `Locowrk`**：`wrk`，不是 `work`/`wrok`；远端私有，未经所有者允许不得转公开或外发。
- **ADR 不可改**：`docs/adr/` 只增不改；ADR-0001 已标“被 ADR-0012 取代”；状态类事实写 CONTEXT/看板/CHANGELOG。
- **内核在仓库里**：`pi/` 是自有 fork（MIT），可改；行为改动先 ADR/Phase 文档 + `pi/FORK.md` + CHANGELOG。上游是未改动部分的行为蓝本。
- **`pi/` 依赖方向**：只依赖标准库/第三方/自身子包；不得 import 本仓库 `internal/`、`cmd/`。
- **导入边界**：正式代码只有 `internal/kernel` 引用 `pi/...`；测试文件可为 fixture 直接引用；内核类型不得进入其他包公开签名。
- **主壳是 WebView**：页面只加载自带资源与白名单 origin；不手写第二套 IPC，优先 `mygo generate`。
- **A 轨 = 受信任代码**：进程隔离只防崩溃；安装与运行界面必须明示。
- **协议新增方法是兼容变更**：不改 `protocol`；改语义/删字段才 +1。
- **真实 key 只走环境变量**：`DEEPSEEK_API_KEY`；冒烟把 `HOME`/`USERPROFILE` 指到临时目录，日志不得出现 key。
- **提交前门禁**：`gofmt -l . && go vet ./... && go test -race ./...` 与 `bash scripts/check-docs.sh`；spike 模块（`fixtures/spike-*`）单独 `go vet`。
- **体量**：docs 单篇 ≤300 行、ADR ≤100 行、代码单文件 ≤500 行、CONTEXT ≤150 行。
- 踩坑索引：`P-001`~`P-004`；方法：`skills/S-001-runtime-comparison-spike.md`。
