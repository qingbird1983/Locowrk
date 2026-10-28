# P-002：上游把会话与技能写死在用户主目录

- 首次记录：2026-10-28（Phase 1 内核测试）

## 现象

`coding.StartSession` / `DefaultSessionDir` 把会话写到 `~/.pi/agent/sessions/`，`AgentDir()` 同样固定在用户主目录下的 `.pi/agent`，**没有环境变量可覆盖**。测试或冒烟命令一旦忘记隔离，就会往开发者真实的 `.pi` 里写测试会话（本次跑 `locowrk-smoke` 就留下两个文件，已手动删除）。

## 根因

上游为了与 pi CLI 共享同一份会话目录和数据格式（ADR-0005 的直接收益），把目录写死；换主目录是它唯一的隔离手段。

## 修复

- `internal/kernel/kernel_test.go` 的 `testHome(t)`：`HOME` 与 `USERPROFILE` 都指向 `t.TempDir()`，所有用例开头调用。
- 手动冒烟留下的会话文件按文件名定位删除，只删本轮产生的文件。

## 预防（怎么不再犯）

- 任何会触发写盘的测试/脚本，第一步先换主目录；Windows 上 `HOME` 与 `USERPROFILE` 都要设。
- 不要把“会话目录可配置”当成已实现能力：它属于 backlog（便携模式），当前不支持。
- 手工跑 `locowrk-smoke` 时，若要避免污染，先设 `USERPROFILE`/`HOME` 到临时目录。

## 失效条件

若上游提供 `PI_CONFIG_DIR`/`PI_HOME` 之类的覆盖入口，或本项目实现便携模式，本条改为指向该机制。

## 再次发生记录

- 暂无。
