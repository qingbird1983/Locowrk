# Phase 3 进度报告：最小桌面主壳

- 状态：定稿
- 日期：2026-10-28
- 关联：[phase-3-desktop-host.md](../../roadmap/phase-3-desktop-host.md)、[shell.md](shell.md)、[frontend.md](frontend.md)、ADR-0011、ADR-0012
- 环境：Windows 11 专业版 10.0.26200；Go 1.27.3；Node 24.14.0 / npm 11.21.1；MyGo v0.7.1；内核 `pi/` 基线 `sky-valley/pi@6f6131ba`

## 1. 结论

Phase 3 的 8 条验收场景全部通过；窗口运行与中文输入已由所有者于 2026-10-28 人工确认。主壳可以日常使用：真实会话逐字输出、工具卡片、停止、重启恢复、空闲零 CPU、5,000 条消息窗口化。

**范围说明**：安装包/签名、钥匙串、macOS/Linux 运行验证**不是 Phase 3 的验收项**，已登记到路线图 backlog（发布工程）。它们要么需要所有者提供证书/真机，要么属于后续实现，不应作为本阶段收尾条件。

## 2. 本轮改动

| 改动 | 说明 |
|---|---|
| 消息 ID 唯一化 | 上游消息没有 ID 且多轮助手消息共享时间戳；DTO ID 改为“角色 + 时间戳 + transcript 位置”，`History` 按同序重放，跨轮/重启稳定 |
| 渲染窗口化 | `web/src/transcript.ts`：上下 spacer + 实测行高，只挂载视口附近；5,000 条消息挂载约 20 行 |
| 按帧合批 | `message_update` 只改内存模型，`requestAnimationFrame` 每帧刷新一次；不再每个 delta 触发一次 DOM/滚动 |
| 事件语义修正 | 系统消息不渲染；用户消息只由事件回显（去掉重复气泡）；工具结果并入工具卡片，历史与实时一致 |
| 验收脚手架 | `-smoke-scenario faux/real/tool/cancel/long/restore`；`scripts/seed-long-session.mjs`；`-hidden` 供空闲 CPU 观测 |
| 凭证状态 | `Info.credentials` 只返回布尔值；状态行显示“凭证：已配置/未配置” |

## 3. 验收结果（Phase 3 §5）

| # | 场景 | 结果 | 证据 |
|---|---|---|---|
| 1 | 真实会话 | ✅ | `-smoke-scenario real`（DeepSeek，key 仅环境变量）：1.2 s 返回 `pong`，页面逐字渲染；日志无 key |
| 2 | 工具过程 | ✅ | `-smoke-scenario tool`：`read` 卡片含参数、`locowrk smoke note` 结果、状态“完成 (0 ms)”，重启后卡片内容一致 |
| 3 | 取消 | ✅ | `-smoke-scenario cancel`：流式输出中途停止，保留已产生文本，随后一条正常完成 |
| 4 | 恢复 | ✅ | 同一 HOME 两次运行，session id 与文本一致；工具卡片结果随历史恢复 |
| 5 | 空闲 | ✅ | 隐藏窗口静置 60 s，进程 CPU 增量 0.000 s；工作集启动后 76.6 MB，静置后回落到 27 MB |
| 6 | 性能 | ✅ | 真实 5,000 条会话：打开到渲染 208 ms，5000 行，挂载 18 行；合成 5,000 条渲染 10~16 ms，挂载 20 行 |
| 7 | 边界 | ✅ | `go vet ./...`、`go test -race ./...` 全绿；`internal/bridge` 依赖树无 MyGo |
| 8 | 安全 | ✅ | `Info` 不返回 key；`TestEventsDoNotLeakAPIKeys` 通过；页面/日志无完整 key |

补充读数：`locowrk.exe` 22.3 MB；前端产物 `app.js` 26.5 KB、`style.css` 4.3 KB、`index.html` 1.7 KB；生成客户端重跑 `git diff` 为空。

**跨平台构建（ADR-0007）**：按“Windows 优先；macOS/Linux 保证可构建”复核，`CGO_ENABLED=0` 下 `windows/amd64`、`darwin/arm64`、`darwin/amd64`、`linux/amd64` 四个目标 `go build ./cmd/locowrk` 全部通过。运行验证不属于 Phase 3，发布前再按需补真机。

## 4. 发现问题与修复

1. **多轮消息 ID 冲突**（阻断级）：第二轮助手消息与第一轮共享 `role+timestamp`，页面互相覆盖，工具轮尤其明显。改为位置后缀后修复，新增 `TestMessageIDsAreUniqueAcrossTurns` 防回归。
2. **空系统气泡 / 重复用户气泡**：事件流包含 `system` 与 `user` 的 start/end，旧页面会各插一次。现在跳过 `system`，用户消息以事件为准。
3. **无窗口化**：旧实现为 5,000 条各建节点并逐条滚动到底，验收会明显卡顿。改为窗口化 + 合批后达标。
4. **工具结果重复**：实时卡片与 `toolResult` 气泡各显示一次；现在结果只进卡片，历史恢复同样处理。

## 5. 自动化入口

```bash
# 基础 / 工具 / 取消 / 长列表
go run ./cmd/locowrk -smoke
go run ./cmd/locowrk -smoke -smoke-scenario tool
go run ./cmd/locowrk -smoke -smoke-scenario cancel
go run ./cmd/locowrk -smoke -smoke-scenario long

# 真实 provider（key 走环境变量；HOME/USERPROFILE 指到临时目录）
go run ./cmd/locowrk -smoke -smoke-scenario real

# 重启恢复（两次运行共用 HOME 与 cwd）
go run ./cmd/locowrk -smoke -cwd <cwd>
go run ./cmd/locowrk -smoke -smoke-scenario restore -smoke-expect "faux assistant reply" -cwd <cwd>

# 5,000 条真实会话
node scripts/seed-long-session.mjs <home> <cwd> 5000
go run ./cmd/locowrk -smoke -smoke-scenario restore -smoke-expect "long marker" -cwd <cwd>
```

## 6. 范围外与风险

- **已人工确认**：窗口可正常打开、中文输入与显示正常（2026-10-28，所有者）。
- **平台约定（ADR-0007）**：Windows 优先；macOS/Linux 只要求“保证可构建”，已用交叉编译验证。运行验证与跨平台 CI 不属 Phase 3，发布前再补。
- **范围外（backlog：发布工程）**：安装包/代码签名（签名需证书）、钥匙串/凭据存储（当前 key 只走环境变量）。这两项按所有者决定暂缓，不进当前主线。
- **高 DPI**：125%/150% 缩放的观感未单独记录；窗口化已保证节点数，必要时补一轮人工检查。
- **消息 ID 的边界**：位置基于“消息追加顺序”；若内核压缩/回放插入未发 `message_start` 的消息，实时与历史 ID 可能错位。Phase 3 未启用压缩；启用前需要复核。
- **长会话内存**：5000 行全量模型约数 MB，未测 10k+；按设计留给后续基准。

## 7. 下一步建议

1. 开始 Phase 4（先写 PluginHost 与原生 `ui.View` 设计稿）；插件视图复用 `transcript` 的窗口化模型。
2. 发布工程暂缓（所有者 2026-10-28 决定不发布）：打包/签名/钥匙串留在 backlog；macOS/Linux 运行验证需要时再补真机。
3. 10k+ 长会话与压缩/回放场景下复核消息 ID。
