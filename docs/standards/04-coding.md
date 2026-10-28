# 规范 04：代码规范

- 状态：定稿（Phase 1 开工前允许修订，修订走 ADR）
- 日期：2026-10-28

## 1. 工具链与硬门槛

- Go 1.27+（与 MyGo 的 1.27.3+ 取交集；仓库内内核基线声明 go 1.26）。
- `gofmt`（CI 检查）+ `go vet`（CI 硬门槛）+ `go test -race ./...`。
- 可选 `staticcheck`；启用后作为 CI 门槛，不启用不假装有。
- **禁止 cgo**：依赖链（仓库内内核 `pi/`、MyGo）均为纯 Go；引入 cgo 依赖须 ADR。
- 依赖版本锁定：`go.mod` + `go.sum` 入库；内核固定提交（见 ADR-0001）。

## 2. 错误处理

- 库/内部包：用 `fmt.Errorf("doing x: %w", err)` 保留错误链；需要判别时定义哨兵错误或类型。
- 可执行壳：把内部错误翻译为用户可读的 `{code, message, detail}`；原始错误只进脱敏日志。
- **禁止 panic 外泄**到 UI、IPC、插件边界；库代码不使用 `panic` 做控制流。必须在边界 `recover` 并转诊断。
- 取消：`context.Canceled` / `DeadlineExceeded` 不当作故障；UI 显示“已停止/超时”。
- 忽略错误必须显式 `_ =` 并注释原因；禁止静默吞掉影响用户结果的错误。

## 3. 并发

- 一切长任务带 `context.Context`；取消必须到达模型请求、工具、插件子进程。
- goroutine 必须有明确退出路径；禁止“发射后不管”。测试用 `-race`，必要时 `goleak`。
- channel 有界优先；向无缓冲 channel 发送必须有超时/取消分支。
- 锁：临界区尽量小；多锁时在注释写锁序；不得在持锁时调用外部回调（避免死锁）。
- 事件总线不得阻塞 Agent 循环（architecture/02 §2.3）。

## 4. 公共 API 纪律

- 公开项 100% doc comment（首行一句话，中文或英文均可，全仓统一用英文）。
- 默认 `internal/`；公开须有理由（供 `cmd/`/`desktop/` 或未来 SDK）。
- 公共 API 变更 = 语义级变更：先文档后代码；breaking change 在 CHANGELOG 标注。
- 内核类型（`pi/...`）不得出现在 `internal/kernel` 之外的公开签名里。

## 5. 依赖准入

- 原则：少而稳；同等功能不引入第二个。
- **新依赖流程**：在本篇附录 A 登记——名称、版本、用途、为何必须、维护活跃度、许可证、备选与否决理由。
- 禁止：未审计的 git 依赖、cgo 依赖（无 ADR）、与既有依赖功能重叠的包、许可证不兼容的包。

## 6. 前端（WebView 路径）

- TypeScript，严格模式；构建产物可复现；不在运行期 `eval`/远程加载代码。
- Go↔前端只通过生成的 Typed IPC 客户端；不手写第二套协议。
- 组件与样式跟随宿主设计语言；不引入第二套 UI 框架（除非 ADR）。
- `node_modules/` 不入库；锁文件入库。

## 7. 注释与可读性

- 注释写“为什么/约束”，不复述代码；临时方案必须带 `TODO(ADR-xxxx)` 或 issue 引用。
- 单文件 ≤500 行、单函数 ≤60 行为指导值（standards/08）；超限拆分或登记豁免。
- 测试与实现同包（Go 惯例）；集成测试入 `internal/*/..._test.go` 或 `tests/`。

## 附录 A：依赖准入表（初始版）

| 依赖 | 版本 | 用途 | 为何必须 | 许可证 | 备选与否决理由 |
|---|---|---|---|---|---|
| `pi/`（内核 fork） | 仓库内，基线 `6f6131ba` | Agent 内核（`agent`/`ai`/`coding`） | 由本项目维护与二次调整（ADR-0012） | MIT（上游） | 外部 module 依赖：无法在仓库内改内核，已否决 |
| `github.com/egoist/mygo` | v0.7.x（锁定） | 桌面宿主 | ADR-0002 既定 | MIT | Fyne/Wails/Gio：匹配度不足 |
| `golang.org/x/sync` | 最新稳定 | errgroup/并发工具 | 仅在标准库不足时引入 | BSD-3 | 手写 WaitGroup：易错；优先标准库 |
| `golang.org/x/text` | v0.37.0 | Unicode 规范化/排序（内核 `coding` 需要） | 内核直接依赖 | BSD-3 | 自研 Unicode 处理：不值 |
| `golang.org/x/image` | v0.41.0 | bmp/webp 解码（内核工具结果图片） | 内核直接依赖 | BSD-3 | 裁剪内核：会分叉维护面 |
| `google.golang.org/grpc` | 暂不引入 | 插件 IPC（备选） | 首版用 JSON-RPC | Apache-2.0 | JSON-RPC：更轻；需要双向流再评估 |

- 升级依赖：独立提交，跑全量门禁，更新本表与 CHANGELOG。
- 每包直接依赖数软上限 15；超出在表中说明。
