# H-008：内核 fork 进仓库（ADR-0012）

- 日期：2026-10-28
- 主题：`sky-valley/pi` 从外部依赖改为仓库内自有分支

## 本轮做了什么

1. 复制依赖闭包的非测试源码到 `pi/`：`agent`、`ai`（含 `providers`、`models_catalog.json`）、`coding`、`internal/jstext`、`internal/nodepath`、`telemetry`；共 132 文件、约 2.6 MB。
2. 模块路径重写为 `github.com/qingbird1983/locowrk/pi/...`；删除 go.mod 的 `github.com/sky-valley/pi` require；`golang.org/x/text`、`x/image` 变为直接依赖。
3. 保留上游 `pi/LICENSE`，新增 `pi/FORK.md`：基线提交 `6f6131ba`、导入范围、差异表、维护/升级规则。
4. 新增 ADR-0012（取代 ADR-0001）；同步 AGENTS 铁律 6/7/9、standards 01/04/06/07、architecture 01/02/03/04/05、research/01、README 两份、看板、CHANGELOG 0.5.0、CONTEXT。
5. 验证：`go build ./...`、`go test -race ./...` 全绿；`pi/` 不 import 本仓库 `internal/`/`cmd/`；`cmd/locowrk-smoke` 改用 `kernel.WithFauxModel`，不再直接引内核包。

## 遗留与下一步

- 第一次内核本地行为改动：ADR/Phase 文档 + `pi/FORK.md` 差异表 + CHANGELOG，三者齐全。
- 跟进上游安全修复：每阶段末对比基线 diff，手工移植。
- Phase 3 主壳（`cmd/locowrk`）与前端。

## 已提取的沉淀

- 无新增；ADR-0012 记录了边界与维护规则。
