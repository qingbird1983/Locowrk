# AGENTS.md — Locowrk 施工指南

> 本文件是 agent 指令的**唯一事实源**，保持 ≤100 行、规则**可判定**。
> 每个文件是干什么的、能不能动 → [`STRUCTURE.md`](STRUCTURE.md)。还没做完的阶段 → [`docs/roadmap/`](docs/roadmap)。
> 一版变更写哪几处 → [`CONTRIBUTING.md`](CONTRIBUTING.md)。旧流水（只读 · ⊘ 不入库，只在作者本机）→ [`docs/history/`](docs/history)。

## 1. 这是什么

- 项目名 = 仓库名 = 文件夹名：`Locowrk`。可热插拔、可回退、带桌面端（Tauri 2 + Vue 3）的 Rust Agent 内核。
- 「一切皆插件」约束的是**无特权模块**，不是"所有代码都打成动态库"：工具、会话存储、Agent 循环扩展面走插件契约；
  现役 = 9 个动态库插件（`plugins-src/`）+ 一批宿主内置能力（模型接入、审批、子代理、定时、浏览器预览）。
  两条通路都经同一 `ToolSource` 进工具池。
- 沙箱插件已随 v2.145 **整目录退役**：不隔离文件与网络就不是安全边界；命令执行面收敛为 `shell` 单一入口。
- 技术栈：Rust（根 Cargo workspace 8 个 crate + `desktop/src-tauri` 独立包）· Tauri 2 · Vue 3 + TypeScript · pnpm · vitest。
- 当前阶段：单人自用内测。**不在范围内**：多用户服务端、云同步、生产部署。Linux **在范围内**：
  路径语义与插件生命周期要在 Windows 与本机 WSL 两侧都有证据（口径与代价见 [`docs/adr/0005`](docs/adr/0005-linux-verified-on-both-platforms.md)）。
- 权威设计文档 = [`docs/design/authority/DESIGN.md`](docs/design/authority/DESIGN.md)：改核心模块前先读对应章节，实现与它冲突时先对齐文档；
  其中 `agent-kernel/` 指内核 Cargo 工作区，crate 名沿用文档，对外项目名叫 Locowrk。

## 2. 常用命令（唯一入口，不要自创调用方式）

| 命令 | 作用 |
| --- | --- |
| `make setup` | 装依赖 + 打印一次性手动项 |
| `make check` | ★ 全量门禁：格式 + 类型 + 形态 + 全部结构闸 + 闸自证 + 两处 clippy + 三面测试（一批收尾之前跑） |
| `make quick` | 秒级档（全部结构闸 + 闸自证 + 前端形态）；只动文档 / 注释 / 脚本 / 零行为变化时用这一档 |
| `make selftest` | **改过闸或判据本身**之后必须跑：判据要为变异报红，打空 = 该项零证明力 |
| `make doc` | 文档规则闸（入口厚度 / 地图 / 决策记录 / 阶段计划 / 长行 / 旧规矩用词 / 历史索引） |
| `make gates` `make fmt` `make test` `make adr` | 只跑其中一项；`make adr` 新建一篇决策记录 |
| `make ledger LEDGER=<名>` | 重放某一刀的历史拆分证明。**不在常驻门禁里**（它比的是历史原文与当前产物，必然越用越红） |

没装 make 就 `sh scripts/tasks.sh <target>`，参数完全等价。**三条命令纪律**（本仓真踩过）：

1. 别把门禁接进管道（`cmd | tail` 的退出码是 tail 的 = 假绿）。
2. `cargo clippy --workspace --all-targets -- -D warnings` 的 `-D warnings` 必须写在 `--` 之后。
3. `desktop/src-tauri` 不在根 workspace，cargo 要在那个目录单独跑；它的 `generate_context!` 需要
   `desktop/dist/` 存在 ⇒ 先 `pnpm -C desktop run build` 一次。

其余：`bash scripts/build_plugins.sh`（插件产物，版本真源 = 脚本内 PLAN 清单）·
`cargo run -p kernel-bin -- demo` / `demo4`（热插拔与回退、审批改写参数与工具视图隔离的**验收叙事**，改内核后必跑）·
`pnpm -C desktop tauri dev`（起桌面端看效果）。[crates.io](https://crates.io) 直连超时时给 cargo 加
`--config 'source.crates-io.replace-with="rsproxy"' --config 'source.rsproxy.registry="sparse+https://rsproxy.cn/index/"'`
（命令行级覆盖，别改全局 `~/.cargo/config.toml`）。

## 3. 代码硬规则（违反即返工）

**依赖严格单向**（违反 = 设计错误，与 DESIGN §8 一致）：

```
kernel-bin / desktop/src-tauri → kernel-agent + plugin-service + session-store   （两个组装点）
kernel-agent → cordis-core + plugin-api + session-store
plugin-service → plugin-host + cordis-core
cordis-core / plugin-host / session-store → plugin-api
```

`plugin-api` 不依赖任何其他 crate；kernel-agent 与 plugin-host 互不依赖（共用的 `Tool` / `CallCtx` 定义在
plugin-api 宿主侧、不进 C ABI；plugin-host 只给固有 API `RegistryToolSource::resolve`，由组装点桥接）。

- `plugin-api` 是**冻结 ABI**：只许把字段追加到结构体末尾，改已有签名 = breaking；`PLUGIN_ABI_VERSION` 变更是大事。
- UI 永远不直接操作 `PluginRegistry`，只经 `PluginService`；Tauri command 是薄适配层，只做参数转换，不写业务逻辑。
- 流式输出走事件推送（`agent://chunk` 等），不塞进 command 返回值。
- 内核八条设计纪律（副作用带逆操作、永不 dlclose、FFI 进 blocking、先写 `ToolCall` 事件、事件只追加、
  不可逆默认审批、补偿幂等用 `call_id`、多版本切 `active` 指针）+ 它们的**实况订正**（每步批量落盘、
  磁盘快照从未接线、现在是七序决策表而非必弹卡、开发态每 id 只暂存当前版本）=
  [`docs/design/authority/DESIGN.md`](docs/design/authority/DESIGN.md) 附录 C，写内核代码前读。
- 单文件建议 ≤400 行：超了先做三问自检（只管一件事？名字概括得住？改它常牵动无关部分？），
  三问皆是 → 可不拆并在提交正文给理由；任一否 → 拆成目录式子模块。`check_length.sh` 只拦"新增超限"。
- `lib.rs` / `main.rs` 只放模块声明与文档注释；实现必须落到 DESIGN 规划的模块文件；
  `#[cfg(test)]` 测试放独立文件；内部类型一律 `pub(crate)`。动手前先列"新建/修改哪些文件、各放什么"。

## 4. 产品与界面硬规则

- **可配置原则**（两条同源，用户 2026-10-03 定）：① 凡是有不确定性的数值，一律做成设置页可调项并给合理缺省；
  ② 界面上写死的禁止/限制，都要有开关可开可关（含最危险的动作）；开发类与危险类开关和数值收进「开发者」分区
  （侧栏版本号连点 7 次解锁）。
- **前端唯一形态标准** = [`docs/design/authority/Front-end Design Guidelines.md`](docs/design/authority/Front-end%20Design%20Guidelines.md)：
  同类只许一种形态；动前端代码前先查对应类目；要新形态 = 先申请（是什么/在哪/为什么/影响）→ 批准 → **先改标准再写码**；
  豁免要登记标准 §七 并在行内留凭证。机械闸 = stylelint（CSS 声明维度）+ `scripts/check-style.mjs`（模板维度）。
  界面文案一律中英成对（`check-i18n.mjs` 拦漏一边）。

## 5. 禁区（[LOCK]，动前先停下来向人说明并等确认）

- `crates/plugin-api` 的既有对外签名 · `Cargo.lock` 与任何 lock 文件（由包管理器更新，禁止手改）
- `.githooks/` · `.github/workflows/` · `docs/design/authority/DESIGN.md` 的规范语义
- `.env` 与任何真实密钥、token、密码、私钥：**不许写进任何项目文件**
- 不要自行执行：递归删除、强制推送、hard reset、发布部署、访问生产环境、装系统级依赖、
  `curl | sh`、**改 git 配置**、改操作系统设置
- 删除 / 覆盖 / 迁移 / 发布、动用户数据目录：先向人确认。发现问题就登记，别在同一刀里顺手修无关代码。

## 6. 工作流（每次任务按此执行）

1. 需求模糊不要猜 → 先写 `specs/<编号>-<名>/spec.md`（含**可测验收**与**非目标**）等确认。
2. 任务较大先给计划（步骤 + 影响文件）等确认。
3. 实现 + 同步补测试 → 按改动面选档跑门禁（见 §2 那张表），**全绿才算完成**；不许为了过闸删测试或放宽规则。
4. 收尾按 [`CONTRIBUTING.md`](CONTRIBUTING.md)：进度只在 `docs/roadmap/` 那一行改勾选，改了哪些文件、实测
   读数、遗留风险写在提交正文；长期取舍新写 `docs/adr/`；**每轮收尾落一个提交**（commit 不必问，2026-10-18 定；push / 合并 / 禁区仍一步一停）。
5. 动了界面，交付前先自己在真实窗口跑一遍并截图核对，再请你亲自看；给你的永远是"打开哪、点哪、该看到什么"； **截图里一眼能看出的不合理（错位、留白、裁切、间距）当场修，别攒到下一刀**（2026-10-19 对话流线补）。
6. Windows 开发机：shell 是 Git Bash，路径写正斜杠；插件产物在 Windows 是 `.dll`；
   vite dev 已钉 `server.host: "::1"` 绕开本机回环过滤驱动（**别改回双栈**，否则首帧回到 30 秒以上）。
7. Linux 侧取证 = 本机 WSL（Ubuntu-24.04）里 clone 一份，**先补两份不入库的产物**（`bash scripts/build_plugins.sh && bash scripts/build_market.sh`）再跑 `cargo test --workspace`；跨平台的性质不许用 `#[cfg]` 单侧跳过。
