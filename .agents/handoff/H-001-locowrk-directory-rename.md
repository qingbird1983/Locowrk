# H-001：本地目录迁移为 Locowrk

- 日期：2026-10-28
- 主题：仓库目录 `pi-go-desktop` → `Locowrk`，新路径开工检查

## 本轮做了什么

1. 所有者改名后在新路径 `E:\Warehouse\AIwork\Locowrk` 重开会话；`git rev-parse --show-toplevel`、`git remote -v`、`git status`、`bash scripts/check-docs.sh` 四项检查全部通过。
2. 未动 remote、未重新 `git init`、未改 go module 计划（仍为 `github.com/qingbird1983/locowrk`，尚未 init）。
3. 更新状态类文档：`.agents/CONTEXT.md`（覆盖式重写）、`docs/roadmap/00-roadmap.md` 看板 Phase 0 行、`docs/design/phase-0/report.md`（风险与待办）、`docs/roadmap/phase-1-kernel-events.md`（模块路径备注）、`CHANGELOG.md`（0.1.5）。
4. ADR-0007 未改（只增不改），其“目录仍叫 `pi-go-desktop`”一句按历史记录保留。

## 遗留与下一步

- Phase 0 只差所有者确认许可证（ADR-0007 默认 MIT）；确认后更新看板为完成并打 tag `phase-0.end`。
- Phase 1 文档仍「评审中」；定稿后按 `go mod init` → `sky-valley/pi` 固定提交 → `internal/kernel` + `internal/eventbus` 的顺序开工。
- Phase 0 报告里的体量数字是迁移前统计，后续阶段报告以当期实测为准。

## 已提取的沉淀

- `pitfalls/P-001-status-facts-in-adr.md`：状态类事实不写进不可修改的 ADR。
