# 子进程插件协议（A 轨）

- 状态：定稿（Phase 2 设计）
- 日期：2026-10-28
- 关联：ADR-0003、ADR-0004、ADR-0008；architecture/04；phase-2-plugin-adapter.md

## 1. 范围

定义宿主（Locowrk）与**编排型插件**子进程之间的通信：握手、注册、事件、命令、诊断、能力与关闭。

不在范围：插件 UI 渲染（Phase 4/7）、插件分发的签名（Phase 7 之后）、运行期执行 `npm install` 或远程加载代码（永不做）。

## 2. 传输与帧

- JSON-RPC 2.0，运行在子进程的 **stdin/stdout** 上。
- 帧格式：**逐行 JSON（JSONL）**，UTF-8，无 BOM，只用 `\n` 分隔；Windows 上同样输出 `\n`，不写 `\r\n`。
- 一行一个完整的 JSON-RPC 对象；对象内不得有裸换行。
- 单行上限 **1 MiB**；超限视为 `protocol_error`，宿主终止插件并记录。
- 输出速率上限：**256 KiB/s 均值，1 MiB 突发**；超限先告警，再按故障处理。
- `stderr` 只放人类可读日志，宿主保留最后 100 行（脱敏）；`stderr` 内容不参与协议，也不得承载数据。
- 子进程不得读取宿主 stdin（协议独占）；孙进程不得继承 stdout/stderr（Windows 用 Job Object，Unix 用进程组）。

## 3. 版本协商

- `manifest.protocol` 是整数；宿主当前支持 `1`。
- 握手：

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{
  "protocol":1,
  "host":{"name":"Locowrk","version":"0.1.0"},
  "session":{"id":"01a1...","cwd":"<workspace>"}}}
```

```json
{"jsonrpc":"2.0","id":1,"result":{
  "protocol":1,
  "plugin":{"name":"mock-orchestrator","version":"0.1.0"},
  "capabilities":[{"name":"fs.read","scope":["<workspace>/**"]}]}}
```

- 协议版本不等 → 宿主返回 `incompatible_version` 并终止；插件返回的 `capabilities` 是**已接受**的能力，宿主实际授予的是两者交集。
- 握手时限 **10 秒**；超时视为启动失败。

## 4. 生命周期与时限

```
spawn → initialize → initialized → register* → ready → (running) → shutdown_plugin → exited
                                              ↘ crashed / timed out → failed（可重试）
```

| 阶段 | 时限 | 超时处置 |
|---|---|---|
| 启动 + 握手 | 10s | 终止，记录 `timeout` |
| 单次 RPC（默认） | 30s | 返回 `timeout`，插件继续运行 |
| 单次 RPC（`long_task:true`） | 插件声明，上限 10min | 同上 |
| 心跳 `ping` | 每 5s，`pong` 2s 内 | 连续 3 次失败 → 判定失联并终止 |
| 关闭 `shutdown_plugin` | 5s | 取消 context → 2s → 杀进程树 |

- 崩溃：记录退出码与最后 100 行 `stderr`；默认最多重启 3 次，退避 1s / 2s / 4s；仍失败则禁用并提示。
- 心跳只在 `ready` 之后启用；`initialize` 期间不发。

## 5. 宿主 → 插件

| 方法 | 参数 | 说明 |
|---|---|---|
| `initialize` | 见 §3 | 握手 |
| `ping` | `{}` | 心跳，插件必须回 `pong` |
| `prompt` | `{sessionId,text,images?}` | 用户消息；插件据此驱动编排 |
| `steer` | `{sessionId,text}` | 运行中插话 |
| `follow_up` | `{sessionId,text}` | 本轮结束后的追加消息 |
| `abort` | `{sessionId}` | 取消当前运行 |
| `tool_result` | `{sessionId,toolCallId,text?,isError}` | 宿主执行完工具后的回填 |
| `event` | `{sessionId,kind,payload,seq}` | 上游生命周期事件（只读通知，不需要回复结果） |
| `invoke_command` | `{sessionId,commandId,args?}` | 用户触发插件注册的命令 |
| `shutdown_plugin` | `{reason}` | 宿主关闭插件；插件须在 5s 内退出 0 |

`event` 是通知（无 `id`）还是请求（有 `id`）由插件能力决定：默认**通知**，插件要回执时在 `initialize` 结果里声明 `wantsEventAck:true`。

## 6. 插件 → 宿主

| 方法 | 参数 | 说明 |
|---|---|---|
| `register_tool` | `{name,description,parameters,timeoutMs?}` | `parameters` 是 JSON Schema；`name` 需匹配 `^[a-z][a-z0-9_]{0,62}$` |
| `register_command` | `{name,description,args?}` | 命令面板条目 |
| `emit_event` | `{kind,payload}` | 编排事件，`kind` 限 `subagent_spawned`、`subagent_settled`、`workflow_step`、`plugin_diagnostic` |
| `spawn_subagent` | `{prompt,model?,tools?}` | 请求宿主起子代理；结果为子会话 id |
| `workflow_step` | `{name,status,detail?}` | `status` ∈ `pending`/`running`/`done`/`failed`/`skipped` |
| `diagnostic` | `{member,category,code,message}` | 兼容性诊断，见 §7 |
| `tool_result` | `{sessionId,toolCallId,text?,isError}` | 插件自行执行工具后的回填（`invoke_tool` 的异步完成） |

- 所有响应必须带与请求相同的 `id`；未知 `id` 的响应被丢弃并计数。
- 插件不得发送 `initialize` 之外的请求给宿主自身（无重入）。

## 7. 三分类与诊断

| 分类 | 线上表现 |
|---|---|
| **支持** | 正常实现；错误只用于真实失败 |
| **缓做** | 返回零值 + `diagnostic{category:"not_implemented"}`；**不得抛错** |
| **不支持（含 TUI）** | 返回零值 + `diagnostic{category:"unsupported"}`；**永不 throw**；每插件每成员只记一条，`count` 累加 |

- 诊断去重键：`plugin + member + category + code`；重复只累加计数。
- 安全拒绝**不走**诊断：能力缺失或越界时宿主返回错误码 `permission_denied`，并写入独立的安全审计记录。
- 禁止用异常表达“未实现/不支持”——那会中断扩展。

## 8. 能力与信任

能力名与范围：

| 能力 | 范围 | 说明 |
|---|---|---|
| `fs.read` | 路径 glob 数组 | 只读文件；空数组 = 无权限 |
| `fs.write` | 路径 glob 数组 | 写入；禁止全树写 |
| `fs.delete` | 路径 glob 数组 | 删除走系统回收站、非递归、限速；禁止全树删 |
| `exec` | 命令白名单（可空表示仅用户确认后执行） | 默认关闭 |
| `net` | 域名白名单数组 | 只允许列出的域名；空 = 无网络 |

- 缺失 = 拒绝（fail-closed）；`scope` 缺失或为空不构成“全放行”。
- 硬拒绝（无论是否声明）：`.env*`、SSH 凭证、`.git/**` 的读写；全树写/删。
- **诚实边界**：A 轨插件进程是受信任代码。宿主只能约束**经宿主代管**的文件/网络操作；插件进程若自带原生运行时（如 Node），进程内直连 fs/net 不受本协议限制。因此本协议**不是沙箱**，安装插件必须显式信任（research/05）。

## 9. 错误码

| code | 含义 | 处置 |
|---|---|---|
| `-32700` | parse error | 计数后丢弃该行 |
| `-32600` | invalid request | 回错误，插件继续 |
| `-32601` | method not found | 回错误，插件继续 |
| `-32602` | invalid params | 回错误，插件继续 |
| `-32603` | internal error | 回错误，计入故障率 |
| `4400` | `unsupported` | 回错误 + 诊断 |
| `4401` | `not_implemented` | 回错误 + 诊断 |
| `4402` | `permission_denied` | 回错误 + 安全审计（不进兼容诊断） |
| `4403` | `timeout` | 回错误；可重试 |
| `4404` | `crashed` | 宿主侧生成，插件不可发 |
| `4405` | `protocol_error` | 终止插件 |
| `4406` | `incompatible_version` | 终止插件 |

## 10. 关闭与残留

1. 宿主发 `shutdown_plugin`；插件停止接收新请求，完成在途工作或返回 `timeout`。
2. 5s 内进程未退出 → 取消 context；再 2s → 杀进程树（含孙进程）。
3. 关闭 stdin，等待 stdout EOF；有界读取，避免管道写满导致僵死。
4. 记录退出码、耗时与是否被强杀；非 0 退出进故障统计，不静默。

## 11. 变更流程

- 新增可选字段：兼容，本文与 schema 同步，不改 `protocol`。
- 改字段语义、删字段、改错误码：`protocol` +1，写 ADR 或 Phase 文档说明，旧版本直接拒绝启动。
- 本文与 `docs/architecture/04-plugin-system.md` 冲突时：以本文的线上形状为准，语义以架构文档为准，并当轮修正其中一处。
