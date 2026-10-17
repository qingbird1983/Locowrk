# Locowrk

可热插拔、可回退、带桌面端（Tauri 2 + Vue 3）的 Rust Agent 内核。

核心思想是**一切皆插件**：工具、会话存储、Agent 循环的扩展面都走同一套插件契约，扩展模块没有特权；
用 Rust 的所有权模型做零开销的服务组合与精确的副作用撤销；用 Tauri 2 提供插件管理与会话界面。
两点常见误解先纠正：模型接入**不是插件**而是宿主内置服务（内核只认一个服务接口，实现在桌面组装点注册）；
沙箱插件已随权限模型重设计**整目录退役**——不隔离文件与网络就不算安全边界，命令执行只留 `shell` 一个入口。

## 现在能用什么

桌面端已经是天天能用的东西：真实模型对话与流式输出、思考链、多模态输入、工具调用与审批、
子代理、定时任务、会话持久化与重放、工作面板内嵌浏览器、记忆树整理与跨会话取回。
界面中英双语、明暗双主题。权限四档（计划 / 询问 / 自动编辑 / 完全访问）加项目粘性授权规则，
外加两类硬底线（路径逃逸、外部地址）与 ABI 边界——**完全访问也不等于没有底线**。

记忆与上下文这一整片**正在被重新设计**（实测下来不好用），现状与待回答的问题写在
[`specs/01-memory-context-redesign/spec.md`](specs/01-memory-context-redesign/spec.md)。

版本号、测试条数、命令条数这类**每次改动都会变的数字**不写在这里：跑 `make gates` 会打印当前读数，
历史轨迹在 git 与 [`docs/history/`](docs/history) 里。（本文件此前长期挂着 v2.149 那批规模数字，
就是这种写法留下的结果——它落后了 148 个版本。）

## 三分钟跑起来

```bash
pnpm -C desktop install          # 前端依赖（一次）
bash scripts/build_plugins.sh    # 暂存插件产物（改内核代码后重跑）
pnpm -C desktop tauri dev        # 起桌面端：聊天、看工具调用、看审批卡
```

克隆之后需要人工做一次（agent 不替你改仓库配置）：`git config core.hooksPath .githooks`，启用提交前检查。

## 命令都从一条入口走

| 想做的事 | 命令 |
| --- | --- |
| 提交前全套检查 | `make check` |
| 只跑结构检查（秒级） | `make quick` |
| 只跑测试 | `make test` |
| 新建一篇决策记录 | `make adr` |
| 看有哪些命令 | `make help` |

实现只写在 [`scripts/tasks.sh`](scripts/tasks.sh) 里（Makefile 只做转发）；没装 make 就
`sh scripts/tasks.sh <名字>`，完全等价。别再手敲零散的 cargo/pnpm 组合——历史上有两次"看着绿其实没跑成"
就是因为把命令接进了管道，`make` 那一层把退出码管住了。

验收叙事两条，改完内核建议跑一次：`cargo run -p kernel-bin -- demo`（装载旧版本 → 切新版本 → 回退旧版本，
全程不重启进程）、`cargo run -p kernel-bin -- demo4`（审批改写参数 + 两个 Agent 的工具视图互不可见）。

## 文档从这里进

| 你要找的东西 | 在哪 |
| --- | --- |
| 这个文件能不能动、谁是唯一真源 | [`STRUCTURE.md`](STRUCTURE.md) |
| 施工规则、硬约束、禁区 | [`AGENTS.md`](AGENTS.md) |
| 还没做完的阶段、现在到哪了 | [`docs/roadmap/`](docs/roadmap) |
| 某个阶段具体做什么、怎么算做完 | [`specs/`](specs) |
| 当初为什么这么定、否掉了什么方案 | [`docs/adr/`](docs/adr) |
| 内核架构与模块规划 | [`docs/design/authority/DESIGN.md`](docs/design/authority/DESIGN.md) |
| 各主题"现在有效的那一份"清单（含冲突时谁说了算） | [`docs/design/authority/README.md`](docs/design/authority/README.md) |
| 前端形态标准（同类只许一种形态） | [`docs/design/authority/Front-end Design Guidelines.md`](docs/design/authority/Front-end%20Design%20Guidelines.md) |
| 插件清单与"哪些是插件、哪些是宿主内置" | [`docs/design/authority/plugins.md`](docs/design/authority/plugins.md) |
| 还没实现的想法与设计稿 | [`docs/design/`](docs/design)（根上散着的那几份） |
| 已全部做完 / 已退役的设计稿（只读） | [`docs/design/archive/README.md`](docs/design/archive/README.md) |
| 以前的批次流水与旧路线表（只读） | [`docs/history/README.md`](docs/history/README.md) |

## 结构一览

```
crates/          内核 crate：plugin-api（冻结 ABI）· cordis-core · kernel-agent · plugin-host
                 plugin-service · session-store · memory-tree · kernel-bin
plugins-src/     内置插件源码（9 个 id，与内核同一个 Cargo workspace；见 plugins/README.md 的版本表）
plugins/ market/ 开发态插件安装目录与本地市场（产物不入库）
desktop/         桌面端：src/ 前端 · src-tauri/ 组装点（独立 Cargo 包）· tests/ 界面用例
docs/            roadmap/ 唯一进度表 · design/authority/ 各主题现役正身 · design/ 根上散着待做的稿子
                 design/archive/ 已兑现与已退役（只读）· adr/ 决策记录 · issue/ 工单 · prototype/ 原型
                 history/ 批次流水与旧表（只读）· references/ 外部参考笔记
specs/           阶段计划（一阶段一目录，含可测验收）
scripts/         构建与检查脚本（任务实现唯一位置）
tmp/             一次性实验（不入库）
```
