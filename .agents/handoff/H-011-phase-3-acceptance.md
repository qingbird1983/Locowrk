# H-011：Phase 3 验收与性能收口

- 日期：2026-10-28
- 主题：消息 ID 唯一化、窗口化渲染、验收脚手架与报告

## 本轮做了什么

1. 修内核消息身份：`internal/kernel` 的 DTO ID 改为“角色 + 时间戳 + transcript 位置”；实时事件用 `idSource` 记位置，`History` 按同序重放；新增跨轮唯一性测试。
2. 前端重写渲染层：新增 `web/src/transcript.ts`（spacer + 实测行高的窗口化列表、rAF 合批、工具结果并入卡片、跳过系统消息、用户消息只由事件回显）；`main.ts` 只做装配；`smoke.ts` 放验收场景。
3. 验收脚手架：`-smoke-scenario faux|real|tool|cancel|long|restore`、`-smoke-expect`、`-hidden`；`kernel.WithFauxScenario`；`scripts/seed-long-session.mjs`。
4. 凭证状态：`Info.credentials` 只返回布尔值，状态行显示“已配置/未配置”；生成客户端已重跑。
5. 实测：真实 key 1.2 s 返回；5,000 条真实会话加载 208 ms / 挂载 18 行；5,000 条合成渲染 10~16 ms；空闲 60 s CPU 增量 0；exe 22.3 MB。
6. 文档：新增 `docs/design/phase-3/report.md`，更新 shell/frontend 设计稿、看板、CHANGELOG 0.8.0、CONTEXT。

## 遗留与下一步

- 人工确认：中文 IME、可见窗口滚动/高 DPI、安装包与签名、macOS/Linux。
- 人工项完成后把看板 Phase 3 改为完成并打 `phase-3.end`。
- 若启用压缩/回放，复核位置型消息 ID 的稳定性（报告 §6）。

## 已提取的沉淀

- P-006：用 PowerShell 改 UTF-8 源文件会破坏中文，必须用编辑工具或显式 UTF-8。
