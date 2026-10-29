# P-012：插件注册的边界值会被拒绝，别让它静默

## 现象

写超时夹具时给工具声明 `timeoutMs: 300`，扩展启动正常、进程健康，但 `fault_slow` 工具没出现在工具列表里。日志里没有报错，只是“少了一个工具”。

## 根因

适配器校验 `register_tool` 的 `timeoutMs` 必须在 1000~600000 ms 之间；越界返回 `invalid params`。插件是用**通知**（无 id）发的注册请求，错误响应无处可去，旧实现直接丢弃，于是表现为“注册了但没有”。

## 修复

- `handleNotify` 把被拒绝的插件通知记成诊断（`member=<method>`、`code=rejected`），状态面板能看见原因。
- 夹具改用合法范围（`timeoutMs: 1000`），测试断言在 3 秒内超时并留下 `timeout` 诊断。

## 预防

- 插件主动发的通知（注册、事件、通知）一旦被拒，必须落成诊断；不要依赖“响应会告诉插件”。
- 写夹具时先对照 `proto/plugin-protocol.md` 的边界（工具名、timeoutMs、参数 schema），越界在测试里应显式失败。
- 工具数量与声明不一致时，先看诊断，不要先怀疑 shim。

证据：`internal/adapter/handlers.go`（`handleNotify`）、`internal/extensions/faults_test.go`、`fixtures/faults/slow`。
