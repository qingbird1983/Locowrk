# H-017：Phase 5 功能实现完成，待人工验收

- 日期：2026-10-28
- 主题：标签页宿主、Diff 服务与页面落地

## 本轮做了什么

- `desktop/tabs.go`：TabHost、会话/最多 4 个 Diff 标签、原生标签栏、`⋯` 菜单、WebView 创建/销毁、导航护栏、`diff-reload`。
- `desktop/diff.go` + `diff_parse.go`：固定只读 git 命令、15k 行解析、DTO、敏感路径隐藏、截断；`Desktop.OpenDiff` 按工具路径定位。
- `web/diff.html` + `web/src/diff/*`：并排/统一、虚拟列表、折叠、轻量着色、复制路径、刷新/错误态；工具卡片“查看变更”。
- 冒烟：`diff` 与 `diff-tool` 通过；10k 行 `diffMs≈425`；切标签滚动/模式保持；关闭后 `tabs=0`。
- 回归：`faux|tool|cancel|long|plugin|plugin-crash` 通过；`restore` 未种子、`real` 无 key。
- 文档：Phase 5 报告草案、CHANGELOG 0.9.5、看板/路线图、CONTEXT。

## 关键读数

- 小 diff：5 行、`diffMs=341`。
- 10k 行：`diffMs=425`（隐藏窗口，页面脚本起算）。
- `go test -race ./...`、`npm run check`、`npm test` 通过；darwin/arm64、linux/amd64 交叉构建通过。
- 前端：`diff.js` 20.6 KB、`diff.css` 5.1 KB；无新增依赖。

## 遗留与下一步

1. 可见窗口人工：切标签后焦点、菜单关闭后焦点回网页、中文输入法、高 DPI。
2. 报告转定稿、补 `real`/`restore`、打 `phase-5.end`。
3. 报告已注明：跨屏文本选择不承诺，未跟踪/已暂存不显示。

## 已提取的沉淀

- 沿用 P-009（零值 WebView 句柄）；P-008（隐藏窗口无原生帧）。
