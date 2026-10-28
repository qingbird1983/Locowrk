# H-007：Phase 3 开工与 MyGo spike

- 日期：2026-10-28
- 主题：主壳路径定稿、MyGo 最小闭环、Phase 3 启动

## 本轮做了什么

1. 按所有者决定补“单人开发例外”（ADR-0010、standards/02 §1）。
2. 读 MyGo v0.7.1：`Bind`、`NewEvent`、`SetFrontend`、`WindowOptions.Hidden`、`Page.LoadHTML`、`EvalAs`、`App.Exit`。
3. 建 `fixtures/spike-mygo/`（独立 module），隐藏窗口验证：
   - 纯 Go 构建，单 exe；
   - WebView2 可用（Edge 155 UA）；
   - 页面→Go 调用 545.7 ms；
   - Go→页面类型化事件往返 550.2 ms；
   - 双向完成后 `Exit(0)`。
4. ADR-0011 定稿：Phase 3 主壳 WebView；原生 UI 从 Phase 4 插件插槽开始。
5. research/03 转定稿并补 spike 读数；Phase 3 文档定稿；看板、CHANGELOG 0.4.0、CONTEXT 同步。

6. `internal/bridge` 落地：DTO-only 门面（列表/新建/恢复/发消息/停止/历史/模型/思考档/订阅）；`kernel.ListSessions`；`kernel.Abort` 归类为 `ErrCanceled`（含内核与 bridge 契约测试）。
7. 验证接缝 #2：`go list -deps ./internal/bridge` 与 `internal/kernel` 均不含 MyGo。

## 遗留与下一步

- `cmd/locowrk` 最小主壳与前端页面（会话列表、消息流、输入、停止）。
- Phase 3 内验证：IME、长列表、钥匙串、打包/签名、三平台。

## 已提取的沉淀

- 无新增；MyGo 用法与读数记入 research/03 §8。
