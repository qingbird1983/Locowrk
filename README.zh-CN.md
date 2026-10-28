# Locowrk

[English](README.md) | 简体中文

**以纯 Go 内核为底座、给 [pi](https://github.com/earendil-works/pi) 做的原生桌面工作台。**

Locowrk 把纯 Go 内核 [`sky-valley/pi`](https://github.com/sky-valley/pi) **放进本仓库**（`pi/`，MIT），内核随产品一起演进，不再等外部模块。桌面宿主用 [MyGo](https://github.com/egoist/mygo) 构建。未改动的内核行为仍以 pi 为准；终端专用扩展调用在桌面边界被挡掉；插件界面有两条一等公民路径：原生 `ui.View` 与系统 WebView。

- **内核在仓库里**：Agent 循环、模型接入、工具、会话、技能与压缩都在 `pi/`，fork 自 `sky-valley/pi`（MIT），记录上游基线提交并由本仓库维护；只有 `internal/kernel` 直接依赖它。
- **编排型插件兼容**：只注册工具、命令与事件钩子的 pi 扩展可被适配；终端界面调用降级为空操作并给出诊断。
- **桌面插件**：结构化可视化用原生 Go 视图，富交互面板用 WebView，第三方插件走子进程隔离。
- **文档先行**：架构、决策记录（ADR）、规范与阶段验收标准先于实现落盘。

进度与当前状态看 [路线图](docs/roadmap/00-roadmap.md) 与 [更新日志](CHANGELOG.md)；本页只做项目说明与导航。

## 从这里开始

| 你要找什么 | 去哪里 |
|---|---|
| 文档地图 | [docs/README.md](docs/README.md) |
| 路线图与进度看板 | [docs/roadmap/00-roadmap.md](docs/roadmap/00-roadmap.md) |
| 决策与理由 | [docs/adr/](docs/adr/) |
| 做事规则 | [docs/standards/](docs/standards/) |
| AI 会话交接 | [.agents/CONTEXT.md](.agents/CONTEXT.md) |
| 进展流水 | [CHANGELOG.md](CHANGELOG.md) |
| 术语表 | [docs/glossary.md](docs/glossary.md) |

## 仓库结构

| 路径 | 内容 |
|---|---|
| `docs/` | 唯一事实来源：调研、ADR、架构、路线图、规范、阶段报告 |
| `.agents/` | AI 会话的工作记忆：交接卡、踩坑、可复用方法 |
| `cmd/` | 桌面可执行文件（Phase 3 建立） |
| `internal/` | 内核封装、事件总线、插件适配器、技能、宿主桥（Phase 1 建立） |
| `desktop/` | 插件宿主与桌面插件（Phase 3 建立） |
| `proto/` | 子进程插件协议（Phase 2 建立） |

## 许可证

[MIT](LICENSE)。`pi/` 内核 fork 自 `sky-valley/pi`（MIT）；MyGo 为 MIT。
