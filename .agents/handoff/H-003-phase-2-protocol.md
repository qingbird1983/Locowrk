# H-003：Phase 2 协议定稿与 spike 前置

- 日期：2026-10-28
- 主题：子进程插件协议、manifest schema、Phase 2 文档定稿

## 本轮做了什么

1. 新增 `proto/`：`plugin-protocol.md`（JSONL 帧、方法表、错误码、时限、能力、诊断、关闭）、`plugin-manifest.schema.json`（权限 + 范围、路径/入口校验、资源上限）、`proto/README.md`（版本与变更规则）。
2. `docs/roadmap/phase-2-plugin-adapter.md` 转定稿：前置条件标注完成，§3.1 改为引用 `proto/` 并记录决定，§3.0 增加运行时比尺（启动/内存/Windows/shim 覆盖/打包/维护成本，各含及格线与淘汰线）与决策顺序 A→B→C。
3. 看板 Phase 2 置「进行中：协议定稿，spike 待网络」；`docs/README` 状态同步；CHANGELOG 0.1.8。
4. 未写任何适配器代码：Phase 2 的实现要等 spike 结论（ADR-0009），避免先写后改。

## 遗留与下一步

- **阻塞**：本机到 github.com:443 连接被重置/超时，`git push` 与拉取真实扩展样本都失败；push 命令已留在 CONTEXT“下一步”第 1 条。
- 网络恢复后：push 本地 6 个提交与 3 个 tag → 拉样本 → 按比尺跑 spike → ADR-0009 → `internal/adapter`。
- spike 期间不对外说“支持 pi 扩展”；口径仍是“编排语义兼容”。

## 已提取的沉淀

- 无新增 pitfall：本轮没有新的重复性问题；`P-001`（状态写 ADR）与 `P-002`（测试写主目录）仍然适用。
