# 规范 02：Git 工作流

- 状态：定稿
- 日期：2026-10-28

## 1. 分支模型

- `main`：常绿分支，任何时候 CI 应可通过。
- 一切变更（含文档）经 feature 分支合并入 main；**禁止直接 push 到 main**（首次初始化提交除外）。
- **单人开发例外**（[ADR-0010](../adr/ADR-0010-solo-main-commits.md)）：只有一名人类所有者、无 CI、无外部评审时，允许直接提交并推送 `main`；推送前必须跑完整门禁，阶段里程碑照常打 annotated tag，已推送历史禁止 force-push。
- 例外在出现第二名贡献者、CI 评审或受保护分支需求时终止，恢复 feature 分支规则并记 ADR。
- 分支生命周期短平快：一个分支对应一个可独立描述的变更集；完成即合并，不长期滞留。
- 大阶段可以并行分支，但合并顺序与冲突处理要写进当前 phase 文档。

## 2. 提交信息（Conventional Commits）

- 格式：`type(scope): subject`（英文，祈使句，≤72 字符）；body 可中文，说明“为什么”。
- type：`feat` / `fix` / `docs` / `refactor` / `test` / `chore` / `build` / `perf`。
- scope：受影响模块（如 `kernel`、`adapter`、`desktop`、`docs`、`ci`）。
- 示例：
  - `feat(eventbus): add per-session sequence numbers`
  - `docs(adr): add ADR-0002 on MyGo host`
  - `fix(adapter): reject ui capability without failing plugin`

## 3. 合并与里程碑

- 合并方式：merge 或 squash 皆可，但合并后的历史必须能看出变更集语义。
- 阶段里程碑：开工打 `phase-N.start`、验收通过打 `phase-N.end`（annotated tag，附一句话验收结论）。
- 热修：`fix/...` 分支；合并后补 CHANGELOG。
- 禁止 force-push 已公开历史；确需回滚用 `git revert`。

## 4. CHANGELOG 义务

- 每次合并入 main 都必须更新 `CHANGELOG.md`：**面向非技术读者措辞**（说“能做什么/修了什么”，不说重构细节）。
- 进度类变更（阶段状态、验收结果）同日更新 `docs/roadmap/00-roadmap.md` 与 `.agents/CONTEXT.md`。
- 版本号与阶段里程碑共用一个序列；文档阶段也登记。

## 5. 代码评审（含 AI 自审）

合并前至少检查：

1. 是否只做当前阶段清单内的事（standards/07）。
2. 是否新增依赖/许可证问题（standards/04 §5、06 §6）。
3. 是否触碰接缝（architecture/03）且未更新检查点。
4. 是否更新了文档/ADR/CHANGELOG（文档先行）。
5. 是否留下个人路径、密钥、调试输出、无主 TODO。

## 6. 发布

- 代码发布用 semver `vX.Y.Z`；发布前跑全量门禁与平台冒烟。
- 发布说明面向用户，技术细节链接 CHANGELOG/ADR。
- 构建产物、签名密钥、证书**不入库**；签名流程单独文档化。
