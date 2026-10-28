# H-018：Phase 5 收尾并进入 Phase 6 设计

- 日期：2026-10-28
- 主题：Diff 审阅完成、阶段 tag、下一阶段设计

## 本轮做了什么

- Phase 5 报告定稿：7 条验收全部通过（所有者确认可见窗口焦点/输入法/高 DPI）；补规范复查四问与体量核对。
- 看板 Phase 5 标 ✅，CHANGELOG 0.10.0，tag `phase-5.end`；CONTEXT 进入 Phase 6 设计。
- Phase 5 实现回顾：`desktop/tabs.go`、`diff.go`、`diff_parse.go`、`web/diff.html`、`web/src/diff/*`、冒烟 `diff`/`diff-tool`。
- 关键读数：10k 行 `diffMs≈425`；标签切换保持滚动/模式；关闭 `tabs=0`；跨平台构建通过。

## 下一阶段（Phase 6）

1. 先读 ADR-0003/0008/0009、architecture/04 §2、research/02、Phase 2 设计/报告与 `internal/adapter`。
2. 写 `docs/design/phase-6/`：真实扩展发现/信任、三分类矩阵、子代理/workflow/async、steering、技能信任与诊断。
3. 选 1~2 个真实扩展样本（记录版本/来源），确认打包与随包 Node 分发。
4. 设计定稿后实现；未定稿不写功能代码。

## 遗留

- Phase 5 已知限制：跨屏文本选择不承诺、未跟踪/已暂存不显示、macOS/Linux 未真机运行。
- `real` 冒烟无 key 未跑；`restore` 已用 250 条种子通过（种子误写用户目录一次，已清理）。

## 已提取的沉淀

- 无新坑；沿用 P-009（WebView 零值句柄）、P-008（隐藏窗口无帧）。
