# 工作上下文交接卡

- 更新：2026-10-28（会话主题：**Phase 2 验收通过并收尾**）

## 当前阶段与状态

- **项目名：Locowrk**（拼写是 `wrk`，不是 `work`/`wrok`；不加 `-pi` 后缀）。远端：`github.com/qingbird1983/Locowrk`（**私有**）；旧 Rust 历史在 `archive/pre-go-locowrk`。
- **本地目录**：`E:\Warehouse\AIwork\Locowrk`；Go module `github.com/qingbird1983/locowrk`。
- **Phase 0 / Phase 1 / Phase 2**：✅ 完成，tag `phase-0.end` / `phase-1.end` / `phase-2.end`。
  - Phase 2 交付：运行时 Node 子进程（ADR-0009）、`internal/adapter`、mock 插件、故障注入、内核外部工具桥；11 条验收全过（`docs/design/phase-2/report.md`）。
  - 真实 provider 冒烟已完成：DeepSeek `deepseek-flash`，读文件工具调用成功。
- **Phase 3（MyGo 最小宿主）**：未开始；开工前要定主界面渲染路径（原生 `ui.View` vs WebView）。
- **远端**：`origin` 为 SSH；`main` 与 tag 已推送。看板唯一来源：`docs/roadmap/00-roadmap.md`。

## 上一轮完成了什么

1. Phase 2 验收：11 条场景逐条执行并记录，报告落 `docs/design/phase-2/report.md`；打 tag `phase-2.end`。
2. 补齐缺口测试：握手不答超时、超长帧（>1 MiB）判 `protocol_error`、孙进程占管道的进程树清理。
3. 真实 provider 冒烟：DeepSeek 官方文档确认 `base_url=https://api.deepseek.com`、模型 `deepseek-flash` / `deepseek-v4-pro`；用库里 `deepseek/deepseek-flash` 跑通读文件工具，56 个事件，回答正确。
4. 同步看板、Phase 2 状态、CHANGELOG 0.3.0、docs/README、CONTEXT 与 H-006。

## 下一步（按优先级）

1. **Phase 3 设计**：`docs/roadmap/phase-3-desktop-host.md` 从草稿转定稿；先定渲染路径（必要时 ADR）。
2. **`kernel` 异步门面**：`Start`/`Wait` 包住阻塞的 `Prompt`，供 UI 不卡主线程。
3. **`internal/bridge`（HarnessService）**：只出项目 DTO 的门面，桌面壳只消费它。
4. **Node 分发形态**（Phase 6 前）：Node SEA 或随包运行时目录；定体积、签名、升级。
5. **长会话与多插件并发基准**（Phase 6）。

## 未决问题

- **主界面渲染路径**：Phase 3 前用一次 spike 定稿（原生 vs WebView）。
- **Node 分发形态**：SEA 未验证；随包运行时体积与签名未定。
- **能力闸真实边界**：B 路线插件可绕过 shim 调 Node 原生 fs/net/exec；OS 级隔离是否做。
- **迟到注册**：`ready` 后注册被拒绝；真实扩展若异步注册会丢工具，Phase 6 复核。
- **`Stop(ctx)` 忽略调用方 ctx**：统一用 `ShutdownTimeout`；Phase 3 门面统一时处理。
- **`kernel.Prompt` 阻塞**：Phase 3 外加异步门面，不改本阶段语义。
- **事件 DTO v0**：工具结果图片暂只传文本；Phase 3 展示图片时加字段。
- **流程偏差**：standards/02 要求 feature 分支，实际一直直接提交 `main`；留所有者决定是否改规范或改习惯。

## 关键提醒

- **项目名是 `Locowrk`**：`wrk`，不是 `work`/`wrok`；远端私有，未经所有者允许不得转公开或外发。
- **ADR 不可改**：`docs/adr/` 只增不改；状态类事实写 CONTEXT/看板/CHANGELOG（`pitfalls/P-001`）。
- **上游只出现在 `internal/kernel`**：适配器工具用项目 DTO，转换在 `kernel.ToolSpec`（接缝 #12）。
- **A 轨 = 受信任代码**：进程隔离只防崩溃；B 路线拦不住 Node 原生 fs/net/exec，安装与运行界面必须明示。
- **协议新增方法是兼容变更**：不改 `protocol`；改语义/删字段才 +1，并同步 `proto/`、CHANGELOG、Phase 文档。
- **请求处理不得重入**：`peer` 读循环同步处理插件请求；处理器里再 `call` 会死锁。
- **JSON-RPC id 规范化**：数字与字符串 id 统一去引号比较（`pitfalls/P-004`）。
- **真实 key 只走环境变量**：`DEEPSEEK_API_KEY` 已在用户环境；冒烟时把 `HOME`/`USERPROFILE` 指到临时目录，不写用户 `~/.pi`，日志里不得出现 key。
- **测试必须换主目录**：`testHome(t)`（`pitfalls/P-002`）；faux 优先，真实 provider 只做手动冒烟。
- **不写自己的会话格式**：走 `coding.SessionRecorder` / `LoadSessionTree` + `LoadBranch`（ADR-0005）。
- **项目技能默认不加载**：`<cwd>/.pi/skills` 需显式 `TrustProject`。
- **提交前门禁**：`gofmt -l . && go vet ./... && go test -race ./...` 与 `bash scripts/check-docs.sh`；spike 模块单独 `go vet`。
- **体量**：docs 单篇 ≤300 行、ADR ≤100 行、代码单文件 ≤500 行、CONTEXT ≤150 行。
- 踩坑索引：`P-001`、`P-002`、`P-003-short-sha-fetch.md`、`P-004-jsonrpc-id-normalization.md`；方法：`skills/S-001-runtime-comparison-spike.md`。
