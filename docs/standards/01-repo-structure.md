# 规范 01：仓库结构与命名

- 状态：定稿
- 日期：2026-10-28

## 1. 目录布局（含后续阶段的预留）

```
/                     仓库根：README.md（英）+ README.zh-CN.md（中）/ AGENTS.md / CHANGELOG.md / LICENSE / .gitignore / .gitattributes / .editorconfig
.agents/              AI 开发知识库与交接中枢（规范见其 README；预算受 standards/08 约束）
docs/                 全部文档（唯一事实来源；结构见 docs/README.md）
cmd/                  可执行入口（Phase 1: locowrk-smoke 临时壳；Phase 3: locowrk）
internal/             私有实现（不可被外部 import）
  kernel/             sky-valley/pi 的唯一封装点
  eventbus/           项目事件与订阅
  adapter/            编排插件适配器
  skills/             技能/上下文资源的项目侧封装（如需）
  bridge/             HarnessService（Typed IPC 门面，无 UI 类型）
desktop/              桌面宿主与内置插件（Phase 3 起）
  host.go             PluginHost 与插槽
  plugin.go           DesktopPlugin 接口
  plugins/            编排树、时间线、工具检查器、Diff
web/                  WebView 前端（Phase 3/5；源码与构建配置）
proto/                子进程插件协议、schema、fixture（Phase 2 起）
fixtures/             测试夹具（事件序列、会话样例、插件 mock）
scripts/              门禁与构建脚本（跨平台优先）
```

- 目录随首批内容建立，**空目录不进 git**。
- `internal/` 下新增包须有清晰职责；不得建“utils/common/helpers”类杂物包。
- 计划中的 `web/`、`proto/`、`desktop/` 在对应阶段前不存在，属于预留命名。

## 2. 命名约定

| 对象 | 规则 | 示例 |
|---|---|---|
| Go module | 小写域名路径（发布前确认） | `github.com/qingbird1983/locowrk` |
| Go 包 | 小写单词，不用下划线/驼峰 | `eventbus`、`kernel` |
| Go 文件 | snake_case | `plugin_host.go` |
| 类型 | UpperCamelCase；接口用 -er 或角色名 | `DesktopPlugin`、`HarnessService` |
| 常量/方法 | 遵循 Go 惯例；缩写统一大小写 | `SessionID`、`HTTPClient` |
| 文档文件 | 两位序号 + kebab-case（ADR 用 `ADR-XXXX-` 前缀，phase 用 `phase-N-`） | `03-critical-seams.md` |
| 前端文件 | kebab-case；组件 PascalCase | `diff-review.tsx`、`DiffPanel` |
| 分支 | `feat|fix|docs|chore|refactor|test/<短横线描述>` | `feat/event-bus` |
| tag | `phase-N.start` / `phase-N.end`（annotated）；发布 `vX.Y.Z` | `phase-1.end` |

## 3. 根目录纪律

- 根目录只放本规范列出的文件；新增根文件须先修订本规范。
- `README.md`（英）与 `README.zh-CN.md`（中）**只做项目说明与导航**，两份内容保持一致；禁止放进度看板、状态或流水账。
- 根 `AGENTS.md` 是**薄索引入口**（导航 + 铁律链接），禁止记录状态/进度/阶段指针；“当前状态”唯一来源是 `.agents/CONTEXT.md`。

## 4. 内容纪律

- 仓库内**禁止**出现：密钥/凭证、个人绝对路径（Windows 用户目录、Unix 的 home 目录等具体本机路径）——引用外部项目用仓库相对描述或 URL。
- 生成物不入库：`node_modules/`、构建目录、二进制、覆盖率报告、日志。
- 大数据/二进制资产（>1MB）入库须先写 ADR，并说明来源与许可证。
- 依赖的本地替换（`replace` 指向本机路径）禁止入库；开发期用完即删。

## 5. 文档归属

- 正式事实进 `docs/`；过程笔记进 `.agents/`；唯一追加式流水是 `CHANGELOG.md`。
- 新文档必须登记 `docs/README.md`；门禁 `scripts/check-docs.sh` 会拦截孤儿文档。
- 文档内引用代码路径必须真实存在；引用外部项目用 URL，不写本机路径。
