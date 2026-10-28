# ADR-0007：MIT 许可证、项目名 Locowrk 与 Windows 优先

- 状态：定稿（2026-10-28 名称与远端确定）
- 日期：2026-10-28
- 关联：README.md；standards/01、06

## 背景

仓库需要许可证、对外名称与平台优先级。依赖链的许可证：pi 为 MIT，`sky-valley/pi` 为 MIT，MyGo 为 MIT；参考项目 PI-Desktop 为 LGPL-3.0、pi-flash 为 GPL-3.0（仅可读思路，不复制代码）。

项目名曾用 `pi-go-desktop`（描述性占位）。所有者决定复用其此前停掉的项目名 **Locowrk**，并明确：拼写是 `wrk`，不是 `work`/`wrok`；不再加 `-pi` 后缀。

## 决策

- **项目名**：`Locowrk`（首字母大写，其余小写；`Loco` + `wrk`）。
- **拼写是契约**：任何界面、文档、安装包、可执行文件都不得写成 `Locowork` / `LocoWork` / `Lumo-pi`；正确写法是 `Locowrk`（`wrk`，不是 `work`/`wrok`）。
- **不加后缀**：不再使用 `pi-go-desktop` 作为产品名；它只保留在历史提交与迁移前的目录名中。
- **远端仓库**：`github.com/qingbird1983/Locowrk`（**私有**，未开放）。
- **Go module**：`github.com/qingbird1983/locowrk`（module 路径用小写；仓库显示名保留 `Locowrk`）。
- **许可证**：MIT。
- **平台优先级**：Windows 优先；macOS/Linux 保证可构建，逐步补齐验收。
- **版权行**：Locowrk contributors + 上游 MIT 版权声明（见 LICENSE）。

## 理由

- 复用所有者已有的项目名，认知连续，不必再发明品牌。
- `Locowrk` 短、可做仓库名、可执行文件名与包名；一个词，不靠连字符。
- 去掉 `-pi` 后缀，避免被误认为 pi 官方项目；与 pi 的关系在 README 与文档中说明即可。
- MIT 与依赖链一致，分发与插件生态最简单。

## 备选（否决）

- **继续用 `pi-go-desktop`**：描述准确但像内部代号，且带 `-pi` 易被当成官方项目。
- **Lumo-pi / Lumopi**：与 Proton 的 AI 助手 Lumo 同品类撞名，且 `LUMO π*` 是化学术语（2026-10-28 讨论结论）。
- **Apache-2.0**：专利条款更明确，但与上游 MIT 生态的兼容说明成本更高，收益不明显。
- **GPL-3.0**：与“可被广泛集成”的目标冲突，且会限制插件生态。
- **暂不设许可证**：默认版权保留，别人无法合法使用；不利于开源协作。

## 后果

- 需要一次命名迁移：README、docs、LICENSE、CI、包名、可执行文件名、插件命名空间。
- **远端已切换**：`github.com/qingbird1983/Locowrk`（私有）；旧 Rust 仓库历史归档在 `archive/pre-go-locowrk` 分支，`main` 已替换为文档基线。
- **仓库目录当前仍是 `pi-go-desktop`**；本地目录改名会改变本会话工作目录，后续单独做。Go module 计划用 `github.com/qingbird1983/locowrk`。
- 正式发布前做一次可用性检查：GitHub 仓库名、域名/包名、目标市场软件类商标；有冲突再写新 ADR。
- 产品正式名变更时，同步 README、`cmd/` 目录名、安装包名与 CHANGELOG；不另写 ADR（属命名调整）。
