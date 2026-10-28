# pi —— 仓库内维护的内核分支

本目录是 Locowrk 自带的 agent 内核，源自 `sky-valley/pi`（MIT，见 `LICENSE`），**不是外部依赖**：从本目录起由本仓库维护，可按需求修改。

## 来源

- 上游仓库：`https://github.com/sky-valley/pi`
- 基线提交：`6f6131bae0dc`（解析版本 `v1.4.29-0.20261024235732-6f6131bae0dc`）
- 导入日期：2026-10-28
- 导入范围：`agent/`、`ai/`（含 `providers/` 与 `models_catalog.json`）、`coding/`、`internal/jstext`、`internal/nodepath`、`telemetry/` 的非测试源码；上游测试、`docs/`、`cmd/`、`examples/` 未导入。

## 与上游的差异

- 模块路径：`github.com/sky-valley/pi/...` → `github.com/qingbird1983/locowrk/pi/...`。
- 目前没有行为改动；首次本地调整必须在下表登记，并同步 CHANGELOG/ADR。

| 日期 | 文件/包 | 改动 | 关联 |
|---|---|---|---|
| 2026-10-28 | 全树 | 仅改 import 路径，导入仓库 | ADR-0012 |

## 维护规则

1. **上游是行为蓝本，不是自动合并源。** 需要跟进上游时，按上游新提交与基线 `6f6131ba` 做 diff，挑选与本项目相关的改动手工移植，移植后跑全量门禁。
2. **本地行为改动必须先写 ADR 或 Phase 文档**；纯兼容/修复也要在 CHANGELOG 留痕。改动处加 `// Locowrk fork:` 注释说明原因。
3. **不要让本目录 import 本仓库其他包**，依赖方向保持内核 → 标准库/第三方，`internal/*` 只做封装。
4. **许可证与版权头保留**；新增文件沿用 MIT 头或项目风格。
5. 升级内核是独立提交，更新 `docs/research/01-sky-valley-pi.md` 的基线版本与 CHANGELOG。

## 验证

```bash
go build ./...
go test -race ./...
```

上游测试未导入，回归以本仓库的 `internal/kernel`、`internal/bridge`、`internal/adapter` 测试与阶段验收为准。
