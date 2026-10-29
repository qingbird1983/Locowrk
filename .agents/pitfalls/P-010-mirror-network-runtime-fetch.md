# P-010：镜像网络下取 Node 运行时与上游样本

## 现象

2026-10-29 重装系统后：`https://github.com` 连接重置，`cdn.jsdelivr.net`、`raw.gitmirror.com`、`ghproxy.net` 均不可达；`https://nodejs.org` 能连但下载约 40 KB/s（110 MB 的 Windows 包要 40 分钟以上）。Node 内置 `fetch` 对 nodejs.org 会长时间无进展且不报错。

## 根因

机器处于受限网络：国际站点时通时断，部分 CDN 被整体阻断；`fetch` 没有默认超时，下载卡住时表现为“命令挂住、无输出”。PowerShell 的 `Invoke-WebRequest` 同样会超时，命令行 `curl.exe` 反而更稳定。

## 修复

- 运行时下载脚本 `scripts/fetch-node-runtime.mjs`：优先用 `curl` 子进程（带 `--max-time`），按 `--mirror` → `LOCOWRK_NODE_MIRROR` → `https://mirrors.ustc.edu.cn/node` → `https://nodejs.org/dist` 顺序尝试；每个来源都读取该来源的 `SHASUMS256.txt` 并校验后，才复制 `node.exe`/`LICENSE`。USTC 镜像实测约 2.7 MB/s。
- 真实上游扩展样本（`sky-valley/pi` 的 `todo.ts`、`rpc-demo.ts`）在 GitHub 不可达时**不要**从 Phase 2 spike 的旧 bundle 里抠源码：那份是 spike 自带 shim 与旧协议，和 `locowrk-shim/1` 不是一回事。等网络可访问（或拿到离线包）再按固定提交构建，并在报告中如实记为未完成。
- 工具链缺失的恢复：`grep`/`glob` 工具依赖 `rg.exe`，可从应用资源目录复制到已在 PATH 的目录；`go test -race` 需要 C 工具链，用 MSYS2 的 `mingw-w64-x86_64-gcc`（UCRT/POSIX 任一）并把 `C:\msys64\mingw64\bin` 加入 PATH。

## 预防

- 新增任何“下载外部产物”的脚本：先设超时、走 `curl`、校验发布方摘要，再写下版本与校验值；不要用无超时的 `fetch` 拉大文件。
- 报告与 CHANGELOG 写清“被网络挡住的验收项”，不要用旧夹具冒充。
- 重装系统后第一件事跑门禁命令，缺什么补什么，并把缺口写进 CONTEXT。

证据：`docs/design/phase-6/report.md` §3/§4；`scripts/fetch-node-runtime.mjs`；`runtime/README.md`。
