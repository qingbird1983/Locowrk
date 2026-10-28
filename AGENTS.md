# AGENTS.md —— AI 协作入口（薄索引）

> 本文件只做导航与铁律，禁止记录状态、进度、阶段指针或流水账。
> AI 的“当前状态”唯一来源：`.agents/CONTEXT.md`（第一站，先读它）。
> 进度看板唯一在 `docs/roadmap/00-roadmap.md`；进展流水唯一在 `CHANGELOG.md`。

## 文档索引（按需进入）

| 你要做什么 | 去哪里 |
|---|---|
| 了解项目 / 对外介绍 | [README.md](README.md)（英）/ [README.zh-CN.md](README.zh-CN.md)（中） |
| 接手工作 / 看当前状态 | [.agents/CONTEXT.md](.agents/CONTEXT.md) → 交接规则见 [.agents/README.md](.agents/README.md) |
| 看进度 / 阶段规划 | [docs/roadmap/00-roadmap.md](docs/roadmap/00-roadmap.md)（唯一看板）+ 当前 phase 文档 |
| 查“为什么这么设计” | [docs/adr/](docs/adr/)（编号递增，只增不改） |
| 查系统设计与约束 | [docs/architecture/](docs/architecture/)（先读 01；接缝必读 03；插件 04；会话 05） |
| 查做事规则 | [docs/standards/](docs/standards/)（9 篇；规范自身的复查见 [standards/09](docs/standards/09-standards-review.md)） |
| 查术语 | [docs/glossary.md](docs/glossary.md) |
| 新增文档登记 | [docs/README.md](docs/README.md)（文档地图，禁止孤儿文档） |

## 铁律（细则见链接，违反即返工）

1. **文档先行**：当前阶段设计文档未定稿，禁止写实现代码 —— [standards/03](docs/standards/03-documentation.md)。
2. **决策留痕**：重大技术决策必须新增 ADR，不得只做不记 —— [standards/03](docs/standards/03-documentation.md)。
3. **收尾四件事**：重写 CONTEXT 交接卡、归档 handoff、沉淀 pitfalls/skills、体量核对；教训当轮提取 —— [.agents/README.md](.agents/README.md)、[standards/07 §7](docs/standards/07-ai-collaboration.md)。
4. **安全红线**：密钥、个人路径不入仓库；插件默认无能力、显式授权 —— [standards/06](docs/standards/06-security.md)。
5. **范围纪律**：只做当前阶段“做”清单内的事，超范围先 backlog 或 ADR —— [standards/07](docs/standards/07-ai-collaboration.md)。
6. **方向纪律**：内核以 pi / `sky-valley/pi` 的行为为蓝本；内核源码已 fork 进本仓库 `pi/`，可按需求修改，但行为偏离必须先写 ADR，并在 `pi/FORK.md` 登记，不得“顺手改得更好” —— [ADR-0012](docs/adr/ADR-0012-fork-kernel-into-repo.md)、[pi/FORK.md](pi/FORK.md)。
7. **外部代码**：GPL/LGPL 项目（PI-Desktop、pi-flash）源码严禁复制或翻译，只读思路；上游 pi（MIT）只用于理解行为，不整文件搬运；本仓库 `pi/` 是经 ADR-0012 批准的 MIT fork，保留许可证与版权头，改动留痕 —— [standards/07 §5](docs/standards/07-ai-collaboration.md)。
8. **体量红线**：docs / .agents / 代码 / 依赖的预算见 [standards/08](docs/standards/08-size-control.md)，超标当期处置或登记豁免。
9. **版本钉死**：`pi/` 记录上游基线提交，升级独立成提交并更新 `docs/research/01-sky-valley-pi.md` 与 CHANGELOG；MyGo 仍锁版本，升级独立提交并更新 `docs/research/03-mygo-stack.md`。

> 语言约定：文档与对用户的沟通用中文；代码、标识符、commit message 用英文。
