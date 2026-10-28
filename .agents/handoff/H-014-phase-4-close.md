# H-014：Phase 4 收尾与 UI 范围决定

- 日期：2026-10-28
- 主题：功能验收关闭、真实 provider 补跑、界面视觉后置

## 本轮做了什么

1. 补跑 `-smoke -smoke-scenario real`：769 ms 返回 `pong`；Phase 3 六场景在原生外框下全绿。
2. 加数据流边界测试：树 200 节点上限（全 running 也淘汰最旧）、workflow 终态不回退、工具 200 条环形缓冲、`clipText` 保持 UTF-8。
3. 所有者决定：不看当前界面；原生外框与网页都是功能粗稿，UI 皮在全部功能完成后单独设计。报告据此只判定功能/数据流/接线，并转定稿。
4. 收尾：报告定稿、看板 Phase 4 完成、CHANGELOG 0.9.2、backlog 增“界面 UI 重设计”、tag `phase-4.end`。

## 遗留与下一步

- Phase 5：Diff 审阅插件，首次启用 `session.tab`；先写 `docs/design/phase-5/`。
- 插件状态在会话切换时清空、切回不恢复；随 Phase 6 的适配器数据源再评估。
- `standards/08 §6` 的“禁止提交 dist/”与前端 embed 入库冲突，留发布工程处理。

## 已提取的沉淀

- 无新增：P-007（窗口类型）、P-008（隐藏窗口/rAF）已覆盖本轮教训；UI 范围决定属状态，写进 CONTEXT 与看板。
