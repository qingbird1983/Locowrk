# 文档地图

本目录是项目的**唯一事实来源**。任何会话、任何人都应能只靠这些文档理解项目并继续工作。

每份文档头部有状态标记：**草稿 → 评审中 → 定稿**。阶段开工的前提是相关设计文档已达「定稿」（流程见 `standards/03-documentation.md`）。

## 阅读顺序建议

| 你是谁 | 读什么 |
|---|---|
| 项目所有者（不读代码） | [README.zh-CN.md](../README.zh-CN.md) → `roadmap/00-roadmap.md`（看板）→ CHANGELOG → `adr/`（感兴趣时） |
| 新加入的 AI 会话 | `.agents/CONTEXT.md` → [AGENTS.md](../AGENTS.md) → 本页 → 当前 phase 文档 → `standards/` 全部 → 相关架构文档 |
| 未来的人类协作者 | 同 AI 会话路径 |

## 目录结构

```
.agents/                AI 知识库与交接中枢（仓库根；规范见其 README 与 standards/08）
docs/
├── README.md            本页：文档地图
├── glossary.md          术语表（面向非技术读者）
├── research/            调研沉淀（事实与数据，决策的依据）
├── adr/                 决策记录（为什么这么做，编号递增，只增不改）
├── architecture/        架构设计（系统是什么样、关键约束）
├── roadmap/             路线图（什么时候做什么、怎么验收）
├── standards/           仓库规范（怎么做事的规则，9 篇）
├── design/phase-N/      各阶段详细设计与进度报告
└── archive/             （按需）被取代但仍有参考价值的旧稿，需在文首标注取代关系
```

## 文档清单与状态

| 文档 | 状态 | 一句话说明 |
|---|---|---|
| architecture/locowrk.md | 定稿 | 立项方案 v1.1：Locowrk 的插件层完整设计与渲染路径（架构 01~05 的来源） |
| architecture/01-overview.md | 定稿 | 总体分层、依赖方向与里程碑形态 |
| architecture/02-kernel.md | 定稿 | `sky-valley/pi` 的封装、事件模型、工具与会话接入 |
| architecture/03-critical-seams.md | 定稿 | 防返工的接缝清单与可执行检查点 |
| architecture/04-plugin-system.md | 定稿 | 双轨插件系统：编排兼容、桌面原生、子进程隔离 |
| architecture/05-session-and-resources.md | 定稿 | 会话、压缩、技能、项目上下文与凭证边界 |
| adr/ADR-0001-reuse-sky-valley-pi-kernel.md | 定稿 | 内核直接复用 `sky-valley/pi`，不自研、不嵌 Node |
| adr/ADR-0002-mygo-desktop-host.md | 定稿 | 桌面宿主采用 MyGo，原生视图与 WebView 双路径 |
| adr/ADR-0003-orchestration-only-compat.md | 定稿 | 三分类兼容矩阵：支持 / 缓做 / TUI no-op + 诊断 |
| adr/ADR-0004-subprocess-plugin-isolation.md | 定稿 | 第三方插件走子进程 + IPC，不用 Go `plugin` 包 |
| adr/ADR-0005-follow-upstream-session-format.md | 定稿 | 沿用上游 JSONL 会话格式，不自立格式 |
| adr/ADR-0006-adopt-timo-pi-doc-governance.md | 定稿 | 采用 Timo-pi 式文档与规范治理，并记录差异 |
| adr/ADR-0007-license-and-identity.md | 定稿 | MIT 许可证、项目名 Locowrk（`wrk`，不加 `-pi`）与 Windows 优先 |
| adr/ADR-0008-a-track-runtime-spike.md | 定稿 | A 轨运行时在 Phase 2 用限时 spike 定稿，Phase 6 只接入 |
| research/01-sky-valley-pi.md | 定稿 | 内核依赖的事实清单：包、能力、缺口、版本钉 |
| research/02-pi-extension-surface.md | 定稿 | pi 四层扩展面与本项目的兼容边界（待 Phase 2 用 TS 源码复核） |
| research/03-mygo-stack.md | 草稿 | MyGo 能力、线程模型、风险与需要 spike 的问题 |
| research/04-governance-reference.md | 定稿 | 参考仓库 Timo-pi 的治理做法与取舍 |
| research/05-reference-pi-desktop.md | 定稿 | 参考项目 PI-Desktop 的插件系统、权限与隔离教训（LGPL，只读思路） |
| roadmap/00-roadmap.md | 定稿 | 总路线图、唯一进度看板、backlog、风险登记簿 |
| roadmap/phase-0-foundation.md | 定稿 | Phase 0：仓库、规范、文档基线 |
| roadmap/phase-1-kernel-events.md | 定稿 | Phase 1：内核接入、统一事件流、会话冒烟 |
| roadmap/phase-2-plugin-adapter.md | 定稿 | Phase 2：编排适配器骨架与 A 轨运行时选型（三分类、spike） |
| roadmap/phase-3-desktop-host.md | 草稿 | Phase 3：MyGo 最小宿主与实时事件可视化 |
| roadmap/phase-4-native-plugins.md | 草稿 | Phase 4：PluginHost 与首批原生 `ui.View` 插件 |
| roadmap/phase-5-webview-plugins.md | 草稿 | Phase 5：WebView 混合渲染与 Diff 审阅 |
| roadmap/phase-6-real-orchestration.md | 草稿 | Phase 6：真实 Pi 编排插件接入与技能复用 |
| roadmap/phase-7-dynamic-plugins.md | 草稿 | Phase 7（探索）：第三方子进程桌面插件 |
| standards/01-repo-structure.md | 定稿 | 目录布局、命名与根目录纪律 |
| standards/02-git-workflow.md | 定稿 | 分支、提交、里程碑 tag、CHANGELOG 义务 |
| standards/03-documentation.md | 定稿 | 文档先行流程、文件头、类型与模板 |
| standards/04-coding.md | 定稿 | Go/TS 代码门槛、错误处理、并发与依赖准入 |
| standards/05-testing.md | 定稿 | 测试分层、fixture、确定性与验收流程 |
| standards/06-security.md | 定稿 | 密钥、凭证、插件能力、工具审阅与供应链 |
| standards/07-ai-collaboration.md | 定稿 | 冷启动、阶段职责、外部代码与范围纪律 |
| standards/08-size-control.md | 定稿 | 文档、知识库、代码、依赖与构建产物的预算 |
| standards/09-standards-review.md | 定稿 | 规范自身的复查、触发信号与修正分级 |
| design/phase-0/report.md | 定稿 | Phase 0 进度报告与基线核对 |
| design/phase-1/report.md | 定稿 | Phase 1 进度报告：内核、事件流、会话恢复的验收结果与首次规范复查 |
| glossary.md | 定稿 | 术语表 |

> 规则：上表新增文档必须同步登记；改状态必须同步本表与 CHANGELOG。
