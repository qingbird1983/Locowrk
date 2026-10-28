# 工作上下文交接卡

- 更新：2026-10-28（会话主题：**Phase 1 内核接入与事件流落地并验收**）

## 当前阶段与状态

- **项目名：Locowrk**（拼写是 `wrk`，不是 `work`/`wrok`；不加 `-pi` 后缀）。远端：`github.com/qingbird1983/Locowrk`（**私有**）；旧 Rust 历史在 `archive/pre-go-locowrk`。
- **本地目录**：`E:\Warehouse\AIwork\Locowrk`；Go module `github.com/qingbird1983/locowrk`。
- **Phase 0**：✅ 完成，tag `phase-0.end`。
- **Phase 1（内核接入与事件流）**：✅ **完成**，tag `phase-1.end`。`internal/eventbus`、`internal/kernel`、`cmd/locowrk-smoke` 已落地，7 条验收全过。
- **Phase 2（编排适配器与 A 轨运行时选型）**：下一步。文档仍是草稿，需先定稿再开工；含 5 天限时 spike（ADR-0008）。
- 看板唯一来源：`docs/roadmap/00-roadmap.md`。

## 上一轮完成了什么

1. 建模块并锁定上游：`sky-valley/pi` → 解析版本 `v1.4.29-0.20261024235732-6f6131bae0dc`（提交 `6f6131bae0dc`）。
2. `internal/eventbus`：项目事件 + 每会话 `Seq`；每订阅者有界队列与投递协程，文本增量合并、溢出丢弃计数、控制事件溢出/超时断开订阅者。
3. `internal/kernel`：唯一 import 上游的包；建会话/续接/Prompt/Steer/Abort/History/换模型与思考档/Close，事件翻译成项目 DTO。
4. `cmd/locowrk-smoke`：事件 JSONL 到 stdout，支持 `--resume`、`--faux`（无 key 也能跑）。
5. 验收与文档：7 条场景全过（faux 驱动），首次规范复查完成，Phase 1 报告落盘，看板与 CHANGELOG 更新。

## 下一步（按优先级）

1. **Phase 2 设计定稿**：`docs/roadmap/phase-2-plugin-adapter.md` 从草稿转定稿；把 spike 样本、指标、淘汰线写细。
2. **A 轨 spike**：5 天限时；结论写 ADR-0009；spike 前不对外承诺“支持 pi 扩展”。
3. **Phase 2 实现**：三分类 + `not_implemented` + TUI no-op + `plugin_diagnostic`；manifest 权限带范围。
4. **真实 provider 冒烟**：找有 key 的环境跑一次（非阻塞项）。
5. Phase 3 开工前定“主界面渲染路径”（原生 `ui.View` vs WebView + TS），必要时新增 ADR。

## 未决问题

- **A 轨运行时**：Phase 2 spike 后由 ADR-0009 定；这是 Phase 2 唯一能改变整体架构的变量。
- **主界面渲染路径**：Phase 3 前用一次 spike 定稿。
- **`kernel.Prompt` 是阻塞调用**：Phase 3 需要在它之外加 `Start`/`Wait` 异步门面，不改本阶段语义。
- **事件 DTO v0**：`tool_execution_*` 的图片结果暂只传文本；Phase 3 展示图片时新增字段（向后兼容）。
- **`sky-valley/pi` 版本策略**：固定提交；升级规则见 ADR-0001，升级独立成提交并全量 `-race`。
- **MyGo v0.x API 漂移**：MyGo 用法限制在 `desktop/` 与 `internal/bridge`，内核不依赖 GUI。

## 关键提醒

- **项目名是 `Locowrk`**：`wrk`，不是 `work`/`wrok`；不加 `-pi` 后缀。远端私有，未经所有者允许不得转公开或外发。
- **ADR 不可改**：`docs/adr/` 只增不改；状态类事实写 CONTEXT/看板/CHANGELOG（见 `pitfalls/P-001`）。
- **上游只能出现在 `internal/kernel`**：其他包只认 `eventbus` 的 DTO；`internal/eventbus` 也不得 import 上游。
- **事件契约**：生命周期事件与 `agent.Ev*` 一一对应，不自造；只新增编排级事件；`Seq` 由 bus 按会话分配，消费端靠它补洞。
- **Agent 循环不可阻塞**：`Bus.Publish` 不得阻塞；慢订阅者只影响自己（见 bus 测试）。
- **测试必须换主目录**：上游把会话/技能写死在 `~/.pi`，测试统一用 `testHome(t)`（`t.Setenv` 改 `HOME`/`USERPROFILE`），见 `pitfalls/P-002`。
- **faux 优先**：无 key 路径用 `providers.RegisterFauxProvider` 脚本化响应；不要在测试里依赖真实模型。
- **不写自己的会话格式**：读写走 `coding.SessionRecorder` / `LoadSessionTree` + `LoadBranch`（ADR-0005）。
- **项目技能默认不加载**：`<cwd>/.pi/skills` 需显式 `TrustProject`；桌面端必须用户可见授权。
- **插件兼容**：支持项真做、缓做项 `not_implemented`、TUI 项 no-op + `unsupported`；永不 throw；每插件每成员诊断一次。
- **A 轨是受信任代码**：能注册工具/钩子就等同 Agent 权限；进程隔离只防崩溃，不是沙箱（research/05）。
- **MyGo 线程模型**：`App.Run` 从 main 调用；监听器在主线程；插件视图只在主线程构建。
- **本地没有 CI**：`sky-valley/pi` 自身无 CI，本仓库门禁是唯一防线；提交前跑 `gofmt -l . && go vet ./... && go test -race ./...` 与 `bash scripts/check-docs.sh`。
- **文档门禁**：新增文档必须登记 `docs/README.md`，否则 `check-docs.sh` 失败。
- **体量**：docs 单篇 ≤300 行、ADR ≤100 行、代码单文件 ≤500 行、CONTEXT ≤150 行。
- 踩坑索引：`pitfalls/P-001-status-facts-in-adr.md`、`pitfalls/P-002-pi-writes-to-home.md`。
