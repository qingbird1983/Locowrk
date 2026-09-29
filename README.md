# Locowrk

可热插拔、可回退、带桌面端（Tauri 2）的 Rust Agent 内核。

核心思想：借鉴 DeepSeek Harness / Cordis 的"一切皆插件"哲学——模型适配器、工具、
会话存储、沙箱、Agent 循环本身都是插件；用 Rust 所有权模型实现零开销服务组合、
精确副作用撤销、多 Agent 进程内隔离；用 Tauri 2 提供插件管理桌面端。

- 权威设计文档：[docs/design/DESIGN.md](docs/design/DESIGN.md)
- 当前状态：工作区骨架已建，处于落地路径**阶段一**（内核骨架 + 事件日志）

## 结构

```
crates/
  plugin-api/       冻结 ABI（唯一，改动即破坏兼容）
  cordis-core/      Context / Effect / Fiber / Scope / EventBus
  kernel-agent/     Agent 循环 / Waterfall / 中间件
  plugin-host/      dlopen Loader / 多版本 Registry / FFI 适配
  plugin-service/   插件安装/卸载/更新编排（CLI 与桌面端共用）
  session-store/    会话事件日志 / 快照 / 回放 / 补偿
  kernel-bin/       CLI 可执行入口
desktop/            Tauri 2 + Vue 3 桌面端（阶段五脚手架）
plugins/            插件安装目录（多版本并存）
config/             profile.toml 插件组合配置
```
