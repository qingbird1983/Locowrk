# 调研 05：参考项目 PI-Desktop（插件与桌面架构）

- 状态：定稿
- 日期：2026-10-28
- 关联：ADR-0003、ADR-0004、ADR-0008；architecture/04；standards/06、07

> 参考对象：<https://github.com/vastsa/PI-Desktop>，`main` 分支提交 `7bfe7530`（2026-10-22），`package.json` 版本 `0.21.0-beta.4`。
> 许可证：**LGPL-3.0**。本项目只读其文档与公开设计，**不复制、不翻译、不结构性照搬源码或文档文字**；下文是事实摘要与自己的判断。
> 体量参考：该仓库 ADR 编号已到 0100（实际 363 篇），`docs/` 约 583 篇 Markdown。它是成熟产品，不是我们可直接照抄的文档规模。
> 方法：以文档与 ADR 为主，未逐行审计全部源码；结论用于借鉴，不当作对 PI-Desktop 当前行为的保证。

## 1. 它是什么

PI-Desktop 是一个 Electron 桌面工作台：React 渲染层、Electron 主进程、Rust `host-core` 侧车、Node/TypeScript 的 pi Agent 侧车。四层职责分得很开：

- **UI**：会话、设置、插件管理、命令面板；渲染层不拿 Node 权限。
- **Electron main**：窗口、IPC 路由、进程监督、更新。
- **Rust host-core**：工具执行、权限网关、插件宿主服务、SQLite 与持久化、审计。
- **Node pi sidecar**：模型接入、Agent 循环、工具调用编排；pi 包固定在同一版本线（1.3.0）。

它和我们的共同点是“内核不自研、桌面壳自建”；不同点是它选择 Electron + Rust + Node，我们选择 Go + MyGo。

## 2. 插件系统的事实

### 2.1 形态与贡献点

- 插件是目录包（另有 `.piplug` zip 分发格式），根目录 `manifest.json` 必填。
- manifest 包含：`id` / `version` / `main` / `renderer` / `contributes` / `permissions` / `fs` 范围 / `net.domains` / `engines`。
- 贡献点：命令、面板、工作区视图、Agent 工具、技能、MCP server、主题、设置、后台服务、插件间消息总线。
- 安装目录分 `installed/`、`disabled/`、`data/`、`logs/`；插件只能读自己的数据目录，不能打开宿主数据库。

### 2.2 运行时与生命周期

- 每个插件一个独立 utility process；调用链是“插件 RPC → 宿主 API 白名单 → 权限网关 → 宿主服务 → 审计”。
- 插件崩溃：未完成调用以 `PLUGIN_CRASHED` 失败，贡献点注销、面板关闭、UI 提示；监督器按退避重启整个插件。
- 状态机：`discovered → validated → installed → enabled → loaded → running`，失败进 `load_error` / `install_error` / `invalid`。
- `onLoad` 失败会自动禁用并回滚已注册贡献；卸载/禁用会注销命令、工具、主题、MCP 连接与消息订阅。
- 重启策略（其文档口径）：1s、2s、4s、8s、16s，封顶 30s；最多 5 次；稳定 60s 后清零；用户手动启停优先于监督器。
- 干净退出与崩溃分开处理：退出前标记 disposing、取消待重启；未标记的进程退出才按崩溃报告（含退出码）。

### 2.3 权限模型

- 默认拒绝：未声明 = 不可用；未确认的高风险动作不执行；不在宿主 API 白名单里的调用不存在。
- **“权限是开关，manifest 带范围”**：`fs.read/write/delete` 必须带路径范围，`net` 必须带域名白名单；范围缺失 = 无权限（fail-closed）。
- 全树写/删（如 `**`、`**/*`）在 manifest 校验阶段直接判非法；删除只走系统回收站、非递归，并有速率限制。
- 高风险权限单列：注册 Agent 工具、运行 Agent 扩展、注册 provider、读取会话、跨会话控制等，安装时明确确认。
- 主题 CSS 在进入渲染层前由宿主清洗；插件不能直接写宿主 DOM。

### 2.4 受信任扩展（最接近我们的 A 轨）

- `contributes.agentExtensions` 让插件携带 TypeScript/JavaScript 模块，在 **Agent sidecar 内**运行，拿到 `ExtensionAPI`，注册工具、命令、事件钩子。
- 它实现的是 `@earendil-works/pi-coding-agent` 的**显式子集**，不是全量兼容；shim 有独立的版本标记，与宿主版本解耦。
- 需要单独的 `agent.extension` 高风险授权；没有授权就跳过并审计。它不自动导入 `~/.pi`。
- 因为扩展运行在 Agent sidecar 内，**它与 Agent 同权**：能注册工具/钩子，就等于能跑 Agent 能做的事。插件沙箱不覆盖这类模块。
- 项目范围就是插件的激活范围；“受信任”不是进程隔离，而是用户明确授权。

### 2.5 UI 与面板

- 插件面板是隔离的网页；插件自有面板内容，但宿主保留窗口控制胶囊和固定的安全区（其 ADR 约定 46px）。
- UI 插槽（composer、消息动作、工具卡片、块渲染器）由宿主的渲染进程执行模块；这是**契约**，不是沙箱，靠动作白名单限制。
- 主题是数据 + 清洗后的 CSS；插件不能注入脚本或任意选择器。

### 2.6 常驻服务与插件间总线

- 后台服务必须在 manifest 声明（每插件最多 4 个），由宿主在 `onLoad` 后启动、`onUnload` 前停止，状态可见、可重启。
- 消息总线按 manifest 声明的主题发布/订阅；主题是审查面，发布者不会收到自己的消息，消息是数据不是能力，不能借总线洗权限。
- 有载荷大小、订阅数、发布速率上限。

## 3. 它自己承认的限制（最重要）

- 其 ADR-0008 实现记录明确写着：插件进程仍是 Node 环境，**权限网关只覆盖 `pi.*` API，不覆盖进程内直接 `require("node:fs")`**；CPU/内存限制尚未强制执行；当时声明权限在加载时自动授予。
- 其插件规范也写明：在能力沙箱落地前，市场插件仍是“用户权限下的不受限代码”。
- 结论：**进程隔离 + API 白名单 ≠ 安全沙箱**。前者解决崩溃、生命周期与可控调用，后者要靠运行时裁剪或 OS 级隔离。
- 这些是观察时点的口径，后续可能演进；用它的结论时要回看当前代码/文档。

## 4. 与我们定位的对照

| 维度 | PI-Desktop | 我们 | 判断 |
|---|---|---|---|
| 内核 | pi TS 包 + Node sidecar | `sky-valley/pi` Go 库 | 同为“复用内核”，语言不同 |
| 宿主 | Electron + Rust host-core | MyGo 单 Go 进程 | 我们更简单，隔离边界更少 |
| A 轨 | Agent 内的 trusted extensions，显式子集 shim | 适配器 + 运行时 spike | 它的 B 路线经验可直接参考 |
| B 轨 | 网页面板 / UI 插槽 / 主题 | 原生 `ui.View` + WebView | 贡献模型相同，渲染不同 |
| 权限 | 权限 + manifest 范围 | 目前只有能力字符串 | 我们要补“范围” |
| 隔离 | 一插件一进程 + RPC 白名单 | 计划子进程 + JSON-RPC | 方向一致，需写清“不是沙箱” |
| 会话 | 宿主 SQLite 为权威 | 上游 JSONL 为权威 | 保持我们的 ADR-0005 |
| 文档 | 363 ADR / 583 文档 | 40 篇、软上限 120 | 借结构，不借体量 |

## 5. 能直接借鉴的做法

1. **权限 + 范围**：`fs.read/write/delete` 带路径 glob，`net` 带域名白名单；缺失即无权限；全树写删非法；删除走回收站、非递归、限速。
2. **受信任扩展模型**：显式 API 子集、独立版本标记、不自动导入 `~/.pi`、安装时高风险授权、按项目激活范围生效；不把“能加载”说成“全兼容”。
3. **监督器语义**：加载失败自动禁用并回滚；退避重启 + 次数上限 + 健康重置；干净退出与崩溃分开；崩溃报告带退出码，但不复制插件 stdout（可能含数据/密钥）。
4. **面板边界**：插件拥有内容，宿主保留窗口控制与安全区；插件拿不到通用窗口原语；WebView /面板不实现与宿主冲突的模态。
5. **数据与日志隔离**：插件私有数据目录、独立日志通道、宿主数据库/会话不可直接访问。
6. **贡献注册表**：启停、加载失败、权限、服务状态都有可见状态与审计，不做“静默失败”。

## 6. 不照搬的部分

- Electron / React / Rust / Node 技术栈；我们坚持 Go + MyGo 单二进制。
- 先在市场铺开、后补沙箱的路径；其文档已承认中间态是“不受限代码”。
- 直接执行带原生能力的 Node 插件而不加限制；若我们选 Node 桥接，只能按“受信任插件”定位，或加运行时/OS 限制。
- 363 篇 ADR / 583 篇文档的规模；我们的预算是 120 篇、单篇 300 行。
- 把宿主 SQLite 当会话权威；我们沿用上游 JSONL（ADR-0005）。
- 加载时自动授予声明权限；我们保留显式授权。

## 7. 对我们下一阶段的动作

- **Phase 2**：manifest 采用“权限 + 范围”；spike 用真实扩展验证 shim 覆盖面；ADR-0009 必须明确“受信任插件”还是“沙箱插件”，不能含糊。
- **Phase 3/4**：若 WebView 面板采用“插件自有内容”，先定安全区与宿主控制胶囊契约；原生插件继续由宿主布局。
- **Phase 7**：动态插件 manifest 加范围、私有数据目录、独立日志、删除安全规则。
- **规范**：A 轨“与 Agent 同权、按受信任代码处理”的措辞已进 ADR-0003 / architecture/04；Phase 2 设计定稿时再核对 standards/06 是否需要补一句。
- **文档纪律**：借它的“ADR / spec / implementation 分层”，不借它的篇数；新增文档继续登记、受预算约束。

## 8. 来源索引

- 仓库：<https://github.com/vastsa/PI-Desktop>
- 总体架构：`docs/spec/02-architecture/01-architecture.md`
- pi 依赖边界：`docs/spec/02-architecture/06-pi-runtime-dependency-boundary.md`
- 插件总览：`docs/spec/07-plugins/01-plugin-system.md`
- manifest：`docs/spec/07-plugins/02-plugin-manifest-schema.md`
- 生命周期：`docs/spec/07-plugins/05-plugin-lifecycle.md`
- 安全：`docs/spec/07-plugins/04-plugin-security.md`
- 权限矩阵：`docs/spec/07-plugins/13-plugin-permissions-matrix.md`
- 受信任扩展：`docs/spec/07-plugins/16-trusted-extensions.md`
- 常驻服务与总线：`docs/adr/0040-plugin-resident-services-and-message-bus.md`
- 进程隔离实现记录：`docs/adr/0008-plugin-runtime-isolation-target.md`
- 面板表面：`docs/adr/0092-plugin-owned-panel-surface.md`
