# H-021：Phase 6 steering 验收与随包 Node 运行时

- 日期：2026-10-29
- 主题：steering 端到端验收、`runtime/node/` 与版本闸、换机环境恢复

## 本轮做了什么

- 内核 `Steer`：空闲回合返回新增的 `ErrNotRunning`；`desktop.App.Steer` 转成中文提示“当前没有正在运行的回合，无法补充指令”。
- 页面冒烟新增 `steer` 场景：运行中补充指令 → 断言 `steering_applied` → 回合结束后历史含补充文本 → 空闲补充必须报错。Go 侧 `cmd/locowrk` 增加 `steer-real`（用真实 provider，页面仍跑 `steer`）。
- 脚本模型场景：`kernel.WithFauxScenario` 增 `steer`（40 tps 慢流 + 第二轮回答）。
- 新增 `TestSteeringRequiresRunningTurnAndReachesNextRequest`：空闲 `ErrNotRunning`、运行中入队、`steering_applied`、历史含 `STEER-OK`。
- `scripts/fetch-node-runtime.mjs`：按钉死版本 `v26.13.0` 下载（curl + 镜像候选 + 发布方 `SHASUMS256.txt` 校验），只保留 `node.exe` 与 `LICENSE`，写 `runtime/README.md`（版本/来源/SHA-256）；`--check` 只校验；`runtime/node/` 加入 `.gitignore`。
- `internal/extensions`：`PinnedNodeVersion`、`CheckNodeVersion`；`Manager.Prepare`/`startIfNeeded` 在校验失败时把整条 A 轨标为 failed 并写明原因（ADR-0015 §3）。`cmd/locowrk` 仅对随包运行时下发版本，`-node`/`LOCOWRK_NODE`/PATH 属开发覆盖，不校验。
- 环境：补 ripgrep（复制到 PATH，`grep`/`glob` 工具需重启应用后生效）、MSYS2 gcc 16.2.0（`go test -race` 恢复）、DeepSeek key 写入用户环境变量。

## 关键结果

- `-smoke-scenario steer`（脚本模型）：通过，约 8.2 s，`attempts=2`。
- `-smoke-scenario steer-real`（DeepSeek flash）：通过，约 4.0 s、4 条消息；回复含 `pong` 的 real 冒烟亦通过（约 0.9 s）。
- `TestBundledRuntimeMatchesPin`、`TestNodeRuntimeVersionGate`：通过（`runtime/node/` 已就位，SHA-256 `dcc60d59…b59150a`）。
- `gofmt`/`go vet`/`go test -race ./...`、`npm run check/build` 全绿；Phase 6 仍未打 tag。

## 遗留（Phase 6 未收尾）

1. 真实 `todo`/`rpc-demo`：本机 GitHub/jsdelivr/镜像全不可达，未构建；不要拿 Phase 2 spike 的旧 bundle 顶替。
2. `tools/extension-build/`（esbuild 打包、`build.json`、拒绝原生模块/远程 import）尚未落地。
3. 原生对话框 input/editor；面板撤销信任/通知/命令参数；技能信任撤销冒烟。
4. 真实 Node 崩溃/超时/进程树/工具冲突；发布构建缺运行时 fail-closed（当前开发回退只在开发模式）。

## 已提取的沉淀

- 新增 P-010：受限网络下取 Node 运行时（curl + 镜像 + 校验）、旧 spike bundle 不可冒充、换机工具链恢复。
