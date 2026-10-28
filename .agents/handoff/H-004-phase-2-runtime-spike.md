# H-004：Phase 2 A 轨运行时 spike

- 日期：2026-10-28
- 主题：三条运行时路线实测，定稿 ADR-0009

## 本轮做了什么

1. 取上游样本：`earendil-works/pi` 提交 `38ff99177fba21e9c57a670f4e95adcf407c99c7`，用 rpc-demo、todo、subagent 三个示例；只在临时检出运行，未入库。
2. 建 `fixtures/spike-runtime/`（独立 Go module）：共享 shim、三个自写探针（ui/state/orchestrator）、esbuild 打包（neutral/node）、QuickJS 宿主（modernc.org/quickjs）、Node 宿主、Go 重写对照、测量器。
3. 同一协议下测量三条路线：冷启动 10 次中位数、RSS 5 次中位数、一致性检查、覆盖率。
4. 结论：A 快而省但编排样本执行失败（`node:fs`/`child_process`）；B 全部通过，55~63ms、约 49MiB；C 只能重写。选 B，写 ADR-0009 与 `docs/design/phase-2/runtime-spike.md`。
5. 复核上游扩展 API，更新 research/02（41 事件、工具注册、`ctx.ui` 方法集、加载与相互调用）与 architecture/04（运行时、矩阵、安全边界）。
6. 同步看板、Phase 2 / Phase 6 文档、docs/README、CHANGELOG 0.1.9。

## 遗留与下一步

- `internal/adapter` + mock 插件 + 故障注入 + 三分类验收（Phase 2 主线）。
- Node SEA / 随包运行时（Phase 6 前）。
- 长期会话与多插件并发基准；三平台冒烟。
- 是否做 OS 级能力隔离（当前只按受信任代码处理）。

## 已提取的沉淀

- `pitfalls/P-003-short-sha-fetch.md`
- `skills/S-001-runtime-comparison-spike.md`
