# P-013：扩展子进程的 stdout 是协议通道

## 现象

给 `mock-orchestrator` 夹具加“孙进程”模式后，`TestStopKillsProcessTree` 开始失败：孙进程很快就消失。调试日志显示插件退出码 1，读取错误是 `parse error: invalid character '…'`。

## 根因

夹具里的孙进程是 `cmd /c ping -n 60 127.0.0.1`，它的 stdout 被设成了插件的 `os.Stdout`——也就是宿主解析 JSONL 的通道。ping 的第一行中文输出进入协议流，宿主按 JSON 解析失败、判定协议违规并杀掉插件；作业对象（Kill-on-close）随之结束孙进程，测试里“孙进程应当还活着”的断言就失败了。

旧实现只杀插件、不杀进程树，孙进程侥幸存活，所以这个夹具缺陷一直没暴露。

## 修复

- 夹具的孙进程不再继承协议 stdout（默认丢弃），只保留“长期存活”这一用途。
- 宿主侧维持原规则：协议解析失败即视为致命，杀进程树并标记 failed。
- 新增 `TestCrashKillsGrandchildren`：用心跳文件证明崩溃后孙进程确实停止。

## 预防

- 任何扩展/夹具的子进程都不得写插件 stdout；需要日志就写 stderr（宿主只保留尾部用于诊断）。
- 给夹具加进程时，先想清楚它的输出流向；协议通道只允许 JSONL 一行一帧。
- 看到“进程莫名退出 + parse error”，先查是不是子进程污染了 stdout，再查插件代码。

证据：`fixtures/plugins/mock-orchestrator/main.go`（`spawnGrandchild`）、`internal/adapter/proc_windows.go`（作业对象）、`internal/extensions/faults_test.go`。
