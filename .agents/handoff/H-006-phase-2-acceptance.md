# H-006：Phase 2 验收与真实 provider 冒烟

- 日期：2026-10-28
- 主题：Phase 2 收尾、tag `phase-2.end`、DeepSeek 真实冒烟

## 本轮做了什么

1. 逐条执行 Phase 2 §5 的 11 个验收场景，结果写入 `docs/design/phase-2/report.md`（全部通过）。
2. 补测试：握手不答超时（`silent`）、超长帧（`flood`，>1 MiB → `protocol_error`）、孙进程占管道的进程树清理（`grandchild`）。
3. 修 `peer`：协议错误现在会终止进程（此前只记 readErr，进程残留会让状态停在 ready）。
4. 真实 provider 冒烟：按 DeepSeek 官方文档确认 base URL 与模型；用 `deepseek/deepseek-flash` 跑 `locowrk-smoke`，工具 `read` 成功，回答 `alpha-42`，56 个事件；key 只在环境变量，HOME/USERPROFILE 指向临时目录。
5. 同步看板（Phase 2 ✅）、Phase 2 状态、CHANGELOG 0.3.0、docs/README、CONTEXT。

## 遗留与下一步

- Phase 3 设计定稿与渲染路径选型；`kernel` 异步门面；`internal/bridge`。
- Node SEA / 随包运行时（Phase 6 前）。
- 迟到注册策略、长会话与多插件并发基准。
- standards/02 的 feature 分支要求与当前直接提交 main 的习惯不一致，留所有者决定。

## 已提取的沉淀

- 无新增；沿用 `P-004`（JSON-RPC id 归一化）。
