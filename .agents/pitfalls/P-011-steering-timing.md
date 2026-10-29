# P-011：steering 的时机决定它落在哪一次请求

## 现象

`TestSteeringRequiresRunningTurnAndReachesNextRequest` 在补充指令后断言“下一轮回答出现”，但个别运行只有第一次回答，历史里却已有补充的 user 消息。加 `-race` 后稳定复现，且脚本模型配置了两条回答，第二条从未被消费。

## 根因

`Agent.Steer` 只是把消息放进队列；队列在“下一次 provider 请求前”被取走。测试在 `Prompt` 刚启动（`IsStreaming` 刚为真、第一次请求尚未发出）时补充，消息被并入**第一次**请求，于是历史顺序是「用户提问 → 补充 → 回答」，回合正常结束，第二条脚本回答永远用不上。真实页面里模型要流式输出几秒，补充自然落在第二轮，所以手工冒烟看不出问题。

## 修复

- 测试先等到 `message_update`（真正开始流式输出）再补充，然后断言第二轮回答与 `steering_applied`。
- 页面冒烟同样先确认 `busy`，并在失败时短暂重试（内核把“未在运行”变成明确错误后，这一层是必要的）。

## 预防

- 任何“steering 进入下一轮”的断言都要以流式事件为起点，不要以 `Prompt` 返回或 busy 标志为起点。
- 脚本模型准备两条回答，并在断言里区分第一轮与第二轮文本，避免“只跑了一轮也算通过”。
- 相关：`kernel.Steer` 在空闲时返回 `ErrNotRunning`（Phase 6），页面据此给出明确提示。

证据：`internal/kernel/kernel_test.go`（`TestSteeringRequiresRunningTurnAndReachesNextRequest`）；`web/src/smoke.ts`（`scenarioSteer`）。
