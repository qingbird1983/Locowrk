// IPC 封装（DESIGN.md §12.2 前端侧）：命令走 invoke，事件走 listen。
// 命令只做发起，内容全部从 `agent://` 事件来。
// 台账第三节 · 第三批第二刀（T38 · 814 → 本文件只剩接面）：22 个后端命令族沉进 `api/`，
// 本片序 = 原文序（保序搬运，一条没重排）。公开面靠 22 条星号再导出原样带出 ⇒ 二十余处
// 消费点的 import 路径一字不改；零声明式改动是这一刀与 Rust 刀最大的形状差（原文件顶层
// 本来就全是导出的单行包装，抬不了任何可见性）。通道表与它的三个订阅入口进了 `api/channels.ts`，
// 对账闸在前端整根里按锚定位（T23 解锁后不必钉文件名），本文件的说明因此不复述那张表的声明式写法。
export * from "./api/chat";
export * from "./api/assets";
export * from "./api/context";
export * from "./api/approvals";
export * from "./api/agent-stop";
export * from "./api/asks";
export * from "./api/channels";
export * from "./api/plugins";
export * from "./api/settings";
export * from "./api/sessions";
export * from "./api/llm";
export * from "./api/session-io";
export * from "./api/picker";
export * from "./api/notifications";
export * from "./api/subagent";
export * from "./api/schedule";
export * from "./api/session-title";
export * from "./api/probe";
export * from "./api/memory";
export * from "./api/developer";
export * from "./api/tree";
export * from "./api/pushes";

