# H-009：Phase 3 设计文档补齐（流程修正）

- 日期：2026-10-28
- 主题：撤回提前实现，补 `design/phase-3/` 后再开工

## 本轮做了什么

1. 所有者指出 Phase 3 未先写设计文档；撤回未提交的 MyGo 依赖改动（`go.mod`/`go.sum`）。
2. 新增 `docs/design/phase-3/shell.md`：进程装配（`cmd/locowrk` → `desktop` → `bridge` → `kernel`）、绑定方法契约、窗口布局、事件→界面映射、恢复/取消/关闭、性能与测试、验收对应。
3. 新增 `docs/design/phase-3/frontend.md`：TypeScript + esbuild、`mygo generate` 生成客户端、产物入库与复现、CSP/安全、体量预算、偏离登记。
4. 更新 `docs/README.md`、Phase 3 文档链接、看板状态、CHANGELOG 0.6.0、CONTEXT。
5. 沉淀 `P-005-design-docs-before-code.md`。

## 遗留与下一步

- 按设计实现 `cmd/locowrk` + `desktop/` + `web/`；先 faux 隐藏窗口冒烟，再真实 key。
- Phase 3 内验证：IME、5k 消息、钥匙串、打包/签名、三平台。

## 已提取的沉淀

- `P-005-design-docs-before-code.md`
