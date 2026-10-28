# H-012：Phase 4 设计定稿与混合渲染 spike

- 日期：2026-10-28
- 主题：原生外框 + 内嵌 WebView 的可行性、Phase 4 设计稿与 ADR

## 本轮做了什么

1. 读 MyGo v0.7.1 源码确认窗口模型：页面窗口不能建 WebView（`errNoContent`），原生窗口用 `ui.WebView` 放置页面；`Event.Emit(窗口)` 会送到该窗口下所有 WebView；`ui.NewTester` 可无窗口跑多帧。
2. 新增夹具 `fixtures/spike-mygo-ui/`（独立 module，隐藏窗口）：原生顶条/左右栏 + 内嵌页面；页面→Go 679.5 ms、Go→页面事件 684.4 ms、原生窗口 74.6 ms、WebView 93.3 ms（进程启动起算）；无窗口 100 行 × 120 帧平均 0.18 ms、最慢 0.65 ms。
3. 新增 ADR-0013：Phase 4 主窗口改为“原生外框 + 内嵌 WebView”，只决定窗口形态，不决定长期是否把会话界面迁原生。
4. 新增 `docs/design/phase-4/host.md`：插槽布局、插件接口（构建式 `Build`）、三个插件的数据模型、启停与设置文件、panic 隔离、前端只加 `selectToolCall`、测试与 Phase 3 六场景回归门禁。
5. 文档同步：phase-4 转定稿（含 `session.tool.detail` 改到原生右栏的说明）、research/03 加 §9、docs/README、看板 Phase 4 状态、pitfall P-007、CHANGELOG 0.9.0。

## 遗留与下一步

- 实现顺序：① 原生外框 + 页面迁移，先让 Phase 3 六场景回归全绿；② Host（注册/分发/启停/设置/诊断）；③ 三个插件；④ 冒烟 `plugin`/`plugin-crash` + 无窗口测试；⑤ 报告与插槽契约回填 architecture/04。
- 迁移风险：`win.Page()` → `web.Page()`、页面可用宽度 ~620 px、原生栏与页面之间的焦点/中文输入法需人工确认。
- panic 恢复先按“插槽级 recover、丢一帧”实现；若实测丢输入，再评估“探针构建”。

## 已提取的沉淀

- P-007：MyGo 页面窗口与原生窗口互斥，混排只能走原生窗口。
