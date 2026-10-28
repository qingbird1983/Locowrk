# H-015：Phase 5 设计定稿

- 日期：2026-10-28
- 主题：WebView 混合渲染与 Diff 审阅的开工前设计

## 本轮做了什么

- 按 MyGo v0.7.1 源码核对多 WebView：同窗多张、隐藏保留页面、原生弹层覆盖、`Destroy` 回收；结论进 research/03 §10，明确“未实测”。
- 新增 ADR-0014：`session.tab` 由宿主托管，每个标签页一张 WebView，关闭销毁；插件接口不变。
- 新增 `design/phase-5/webview.md`：TabHost 契约、标签栏、生命周期、导航/CSP/IPC、焦点与测试。
- 新增 `design/phase-5/diff.md`：固定 git 命令、解析与 DTO、敏感路径、虚拟列表/并排、性能与安全。
- 重写 `roadmap/phase-5-webview-plugins.md` 为定稿；同步架构 04、Phase 4 待决 2、docs 地图、CHANGELOG 0.9.3、CONTEXT。

## 关键约束（实现时不要丢）

- 绝不在 `Build` 中创建/销毁 WebView；也不得持宿主主锁做（主线程死锁）。
- 最多 4 个标签；会话切换/关窗全部关闭。
- git 只读、固定参数、无 shell、5 秒超时、2 万行 / 8 MiB / 200 文件。
- 敏感路径只计数不返回内容；页面只用 `textContent`，禁止 `innerHTML`。
- 1 万行 1 秒是验收硬指标，必须有读数。

## 遗留与下一步

1. 先做 `fixtures/spike-mygo-tabs` 双 WebView 验证，结果补 research/03 §11；失败先改 ADR-0014。
2. 再按设计实现 tabs → diff 服务 → diff 页面 → 工具卡片接线 → `diff` 冒烟 + 7 场景回归。
3. 未决：标签内存、跨屏文本选择、焦点/IME 人工确认。

## 已提取的沉淀

- 本轮没有新踩坑；沿用 P-006（PowerShell 毁中文）、P-008（隐藏窗口无帧）。
