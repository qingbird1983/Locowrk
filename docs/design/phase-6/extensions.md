# 设计：Phase 6 A 轨扩展接入与桌面接线

- 状态：定稿
- 日期：2026-10-28
- 关联：ADR-0003、ADR-0004、ADR-0009、ADR-0015；architecture/04 §2；research/02；[runtime.md](runtime.md)、[skills.md](skills.md)

## 1. 目标与非目标

**目标**：发现并信任真实 pi 扩展，用 `runtime.md` 的 Node 子进程运行；扩展注册的工具进入内核，事件进入现有编排树/workflow 条，命令、通知、基础对话框与兼容诊断在桌面可见；运行中可从输入区补充指令（steering）。

**不做**：运行期安装/编译扩展、插件市场、签名分发、OS 级沙箱、宿主代管子代理（`spawn_subagent` 仍缓做）、链式上下文改写（缓做）、TUI 渲染、MCP、prompt templates。

## 2. 组成与依赖

```text
cmd/locowrk       装配 kernel → extensions.Manager → bridge → App/Host
internal/extensions  发现、信任、启动/停止、工具/命令/诊断/通知、事件扇出
internal/adapter     已有子进程协议；补 SetSession、SendHostEvent、Abort、UI 回调
internal/kernel      已有 ExtraTools/Steer/TrustProject；补 Skills 查询
internal/bridge      只传 DTO：tools 提供者、Steer、信任、技能查询
desktop/             原生扩展状态面板（B 轨插件）与信任/命令入口
```

- `internal/extensions` 可 import `adapter`/`kernel`/`eventbus`；不得 import MyGo。
- 扩展进程只通过协议与宿主交互；不读宿主设置/会话文件。
- 桌面只消费 DTO；不把 `adapter.Manifest` 直接暴露给页面。

## 3. 发现、安装与信任

**位置（Phase 6）**

| 来源 | 路径 | 说明 |
|---|---|---|
| 用户 | `%AppData%\Locowrk\extensions\<name>\` | 手动安装的预打包目录 |
| 项目 | `<cwd>/.pi/extensions/<name>\` | 需项目信任 + 扩展信任 |
| 开发/冒烟 | `-extensions <dir>` | 仅进程参数，不写设置；测试用显式信任 |

- 只扫描固定深度（一级子目录），不跟随符号链接；每个目录必须有 `plugin.json`，否则跳过并诊断。
- 不自动扫描 `~/.pi` 的 TypeScript 扩展；用户扩展必须手动复制预打包目录（安装器留到 Phase 7/发布）。
- 同名扩展以用户目录优先；项目与用户重名时项目副本禁用并诊断。

**信任**

- 首次发现或 `plugin.json`/`bundle.mjs` 哈希变化：不启动，状态 `untrusted`，界面显示名称、来源路径、哈希前 8 位、声明能力与“仅受信任代码”提示。
- 用户点“信任并启用”后写设置：`{name, path, sha256, enabled, trustedAt}`；此后启动校验哈希，变化即回到未信任。
- 项目扩展额外要求 `projectTrusted`（工作区信任）；撤销工作区信任同时禁用项目扩展。
- 关闭启用（enabled=false）不删除信任记录；换路径视为新扩展。

**设置文件（version 2）**

```json
{
  "version": 2,
  "plugins": {"orchestration-tree": true},
  "rail": "open",
  "extensions": {
    "todo": {"path": "<abs>", "sha256": "…", "enabled": true, "trustedAt": "2026-10-28T…"}
  },
  "projectTrust": {"<abs cwd>": "2026-10-28T…"},
  "projects": {"<abs cwd>": {"skills": ["name-a", "name-b"]}}
}
```

- v1 读取后迁移为 v2；未知字段丢弃；损坏文件回退默认 + `settings` 诊断。
- 只存路径、哈希、时间、开关；不存扩展源码、会话内容或凭证。
- `projectTrust` 只按规范化绝对路径记录；路径消失时保留，设置界面可清理。

## 4. 管理器（`internal/extensions`）

```go
type Options struct {
    Bus      *eventbus.Bus
    Cwd      string
    NodePath string
    Settings Settings
    UI       HostUI // notify/select/confirm/input/editor
}

type Manager struct { /* manifests, adapters, diagnostics, notices */ }

func (m *Manager) Prepare(ctx context.Context) error      // 发现+信任+启动
func (m *Manager) Tools() []kernel.ToolSpec               // 稳定；内核建会话时注入
func (m *Manager) Commands() []CommandInfo
func (m *Manager) RunCommand(ctx context.Context, name, args string) error
func (m *Manager) SetSession(id string)                   // 切换会话，不重启进程
func (m *Manager) Dispatch(ev eventbus.Event)             // 内核事件扇出到扩展
func (m *Manager) Diagnostics() []adapter.Diagnostic
func (m *Manager) Notices() []Notice
func (m *Manager) SetEnabled(name string, on bool) error
func (m *Manager) Trust(name string) error
func (m *Manager) Stop() error
```

- 启动顺序：扫描 → 校验 → 逐个 `adapter.Start`（10 秒握手）→ 收集工具/命令 → 订阅内核事件。单个失败不阻塞其他扩展。
- 工具名与内置工具、其他扩展冲突：禁用该扩展并记 `tool_conflict`；不静默覆盖。
- 工具 `ToolSpec.Execute` 捕获扩展名与适配器；调用 `InvokeTool`，把 context 取消/超时映射为工具错误；结果文本进会话。
- 命令列表只读快照；执行失败显示在状态面板。
- 一个扩展进程服务当前工作区；`SetSession` 只改 `sessionId` 并发送合成 `session_shutdown`/`session_start`（不进 eventbus）。
- 关闭：先停事件泵，再 `Stop` 所有适配器，杀进程树。

## 5. 事件与编排

- 内核事件：管理器单独订阅 bus（`SessionID` 为空但过滤当前会话），只转发**生命周期/工具/消息**类事件到每个扩展的 `event`；不转发编排事件/诊断/steering，避免回环。
- 编排事件：扩展 `emit_event`/`workflow_step` → 适配器 → bus → 现有 `orchestration-tree`/`workflow-timeline`；事件 session 取适配器当前会话，Seq 由 bus 分配。
- 子代理：扩展自行 spawn 子进程并上报 `subagent_spawned`/`subagent_settled`；宿主**不**实现 `spawn_subagent`（继续 `not_implemented` 诊断）。并行/链式/异步由样本扩展负责，宿主只保证事件顺序与 UI。
- `tool_update`（新可选方法）映射为 `tool_execution_update`，让长工具在卡片上显示进度。
- 工具注册走内核现有审阅；插件工具不能绕过 `BeforeToolCall`。
- steering：桌面输入区在 busy 时提供“补充指令”，调用 `Desktop.steer(sessionID,text)` → bridge → kernel.Steer；内核发布 `steering_applied`，扩展通过 `event` 收到；协议里的 `steer` 方法 Phase 6 不使用（保留给插件驱动模式）。
- 中止：`Desktop.stop` → kernel.Abort；管理器向扩展发 `abort`，并等 `agent_end`/`session` 事件自然收敛。

## 6. UI

**原生扩展状态面板**（B 轨插件 `extensions-status`，`session.sidebar` 第二节）

- 列表：扩展名、来源（用户/项目/开发）、状态（运行/未信任/禁用/失败）、工具/命令数量。
- 每行操作：信任/撤销、启用/禁用、重试；未信任时显示哈希与“受信任代码”提示。
- 诊断区：按 `plugin + member` 去重，分类 `not_implemented`/`unsupported`，显示次数；安全拒绝单独显示。
- 通知区：最近 20 条 `ui_notify`（时间、级别、文本），不弹系统通知以免打断。
- 命令区：扩展命令列表；点击执行，参数用输入框。

**基础对话框**

- `ctx.ui.select/confirm/input/editor` 走适配器 UI 回调 → 原生 `mygo` 对话框（主线程）；取消返回 `undefined`/false，不抛错。
- 对话框打开时不阻塞内核事件泵；超过 60 秒无操作视为取消并诊断。

**页面**

- 状态行/输入区增加 steering；命令执行结果显示在消息流或状态行；不新增第二个设置窗口。
- 扩展状态面板是功能粗稿，视觉留 UI 重设计。

## 7. 与 bridge/kernel 的接口

```go
// bridge.Options（回调由 cmd 注入，bridge 不 import internal/extensions）
Tools             func() []kernel.ToolSpec
Extensions        func() []ExtensionRow
TrustExtension    func(name string, trusted bool) error
SetExtensionEnabled func(name string, enabled bool) error
Steer             func(sessionID, text string) error
ProjectTrusted    func(cwd string) bool
SetProjectTrust   func(cwd string, trusted bool) error
Skills            func(cwd string) SkillsView
```

- `ExtensionRow`/`SkillsView` 是 bridge 的 DTO；desktop 只消费 DTO，不 import `internal/extensions` 或 `adapter`（接缝 #2/#12）。
- `CreateSession`/`OpenSession` 把 `ExtraTools` 传给内核；`TrustProject` 从设置读取。
- `Skills(cwd)` 返回用户技能 + （已信任时）项目技能、诊断、注入状态；只读 DTO。
- `Steer` 仅在会话忙时可用；空闲返回明确错误，不排队。

## 8. 测试与验收

| 检查 | 手段 | 断言 |
|---|---|---|
| 发现/信任 | 临时目录 + 设置 | 未信任不启动；信任后启动；哈希变化回未信任；项目未信任禁用 |
| 工具 | 真实 `todo` | 工具注册、内核可调用、审阅与取消 |
| 命令 | `todo`/`rpc-demo` | 命令列出并执行；错误可见 |
| 事件 | `orchestrator-sample` | parallel 两节点、chain 顺序、async 完成、取消 |
| steering | faux/真实 | busy 输入补充指令；下一轮请求包含；`steering_applied` 事件 |
| UI 降级 | `rpc-demo` | `setWidget/setTitle` 返回零值、扩展继续、每成员一条诊断 |
| 对话框 | shim 探针 | select/confirm/input/editor 映射原生对话框，取消不抛错 |
| 技能 | `skills.md` 场景 | 未信任/信任/撤销 |
| 故障 | mock/真实 | 崩溃、超时、哈希不符、工具冲突只禁用该扩展 |
| 回归 | Phase 1/2/3/4/5 | 全绿 |

## 9. 风险与待决

- **shim 覆盖面**：链式改写缓做会让部分扩展行为不完整；诊断必须显眼，不能假装全兼容。
- **`tool_call` 闸门**：插件无响应时 fail-open 还是 fail-safe；实现前定，默认倾向 fail-open + 诊断，避免卡死工具。
- **命令 UI**：Phase 6 只做列表+执行，不做斜杠自动补全（留 UI 重设计）。
- **项目扩展信任**：与工作区技能信任分开记录；撤销工作区信任时两者都停。
- **多会话**：进程共享当前会话；Phase 6 桌面只有一个活动会话，多会话并排留 backlog。
- **Node 目录体积/签名**：ADR-0015 已定；发布工程另行处理。
