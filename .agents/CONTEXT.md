# 工作上下文交接卡

- 更新：2026-10-28（会话主题：**Phase 3 开工：WebView 主壳定稿**）

## 当前阶段与状态

- **项目名：Locowrk**（拼写是 `wrk`，不是 `work`/`wrok`；不加 `-pi` 后缀）。远端：`github.com/qingbird1983/Locowrk`（**私有**）；旧 Rust 历史在 `archive/pre-go-locowrk`。
- **本地目录**：`E:\Warehouse\AIwork\Locowrk`；Go module `github.com/qingbird1983/locowrk`。
- **Phase 0 / 1 / 2**：✅ 完成，tag 已打（`phase-2.end`）。
- **Phase 3（MyGo 最小宿主）**：🚧 进行中：
  - ✅ 主壳路径定稿：MyGo WebView（ADR-0011）。
  - ✅ MyGo spike：隐藏窗口 + WebView2 + Typed IPC 双向，启动到页面回调约 0.55 s（`fixtures/spike-mygo/`）。
  - ✅ `internal/bridge`：DTO-only 门面（列表/新建/恢复/发消息/停止/历史/模型/思考档/事件订阅）+ 契约测试；`kernel.ListSessions`；Abort 归类为“已取消”。
  - ⏳ 待做：`cmd/locowrk` 主壳与最小界面（会话列表、消息流、输入、停止、设置）。
- **远端**：`origin` 为 SSH；`main` 与 tag 已推送。看板唯一来源：`docs/roadmap/00-roadmap.md`。

## 上一轮完成了什么

1. 规范：按所有者决定新增“单人开发例外”（ADR-0010 + standards/02），允许直接提交 main，门禁前置。
2. Phase 2 收尾后继续开工 Phase 3：读 MyGo v0.7.1 API（`Bind`/`NewEvent`/`SetFrontend`/`LoadHTML`），建独立 spike module。
3. spike 实测：纯 Go 无 cgo 构建；隐藏窗口 WebView2（Edge 155）；页面→Go 调用 545.7 ms；Go→页面事件往返 550.2 ms；双向完成后正常退出。
4. ADR-0011 定稿：Phase 3 主壳走 WebView；原生 `ui.View` 从 Phase 4 插件插槽开始；research/03 转定稿并补 §8 读数。
5. Phase 3 文档定稿、看板改“进行中”、CHANGELOG 0.4.0。
6. `internal/bridge` 落地：列表/新建/恢复/发消息/停止/历史/模型/思考档/事件订阅；`kernel.ListSessions` 支持重启后的最近会话；`kernel.Abort` 现在把中断归类为“已取消”，契约测试覆盖（CHANGELOG 0.4.1）。

## 下一步（按优先级）

1. **`cmd/locowrk` 主壳**：MyGo 窗口 + 生成 TS 客户端 + 最小界面（会话列表、消息流、输入、停止）；窗口关闭先取消进行中的请求。
2. **Phase 3 内验证**：中文 IME、5,000+ 消息滚动、钥匙串、安装包体积/签名、三平台差异。
3. **Node 分发形态**（Phase 6 前）；长会话/多插件基准。

## 未决问题

- **会话列表**：已由 `kernel.ListSessions`（上游 `coding.ListSessions`）解决；排序沿用上游（按 header 时间），若产品要“最后活动时间”再调整。
- **异步门面**：`Prompt` 仍阻塞；MyGo 的绑定方法每次调用跑独立 goroutine，bridge 用阻塞 + `Cancel` 即可，暂不需要 `Start/Wait`。
- **前端产物入库策略**：WebView 前端构建产物是否入库、如何复现；Phase 3 先最小内联页面，后续再定。
- **能力闸边界**（Phase 6）：Node 原生 fs/net/exec 绕过 shim，按受信任插件处理。
- **迟到注册**：`ready` 后拒绝；真实扩展异步注册待 Phase 6 复核。
- **事件 DTO v0**：工具结果图片只传文本；展示图片时新增字段。
- **流程**：单人开发例外已生效（ADR-0010）。

## 关键提醒

- **项目名是 `Locowrk`**：`wrk`，不是 `work`/`wrok`；远端私有，未经所有者允许不得转公开或外发。
- **ADR 不可改**：`docs/adr/` 只增不改；状态类事实写 CONTEXT/看板/CHANGELOG（`pitfalls/P-001`）。
- **Upstream 只在 `internal/kernel`**：bridge/UI 只认项目 DTO；MyGo 只在 `cmd/` 与未来的 `desktop/`，`internal/bridge` 与 `internal/kernel` 不得 import MyGo（接缝 #2）。
- **主壳是 WebView**：页面只加载自带资源与白名单 origin；不手写第二套 IPC，用 `mygo generate`。
- **A 轨 = 受信任代码**：进程隔离只防崩溃；安装与运行界面必须明示。
- **协议新增方法是兼容变更**：不改 `protocol`；改语义/删字段才 +1。
- **JSON-RPC id 归一化**、**请求处理不得重入**（`pitfalls/P-004`）。
- **真实 key 只走环境变量**：`DEEPSEEK_API_KEY`；冒烟把 `HOME`/`USERPROFILE` 指到临时目录，日志不得出现 key。
- **测试**：`testHome(t)`（`P-002`）；faux 优先；真实 provider 只做手动冒烟。
- **提交前门禁**：`gofmt -l . && go vet ./... && go test -race ./...` 与 `bash scripts/check-docs.sh`；spike 模块单独 `go vet`。
- **体量**：docs 单篇 ≤300 行、ADR ≤100 行、代码单文件 ≤500 行、CONTEXT ≤150 行。
- 踩坑索引：`P-001`~`P-004`；方法：`skills/S-001-runtime-comparison-spike.md`。
