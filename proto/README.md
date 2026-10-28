# proto/ —— 子进程插件协议

本目录是**子进程插件协议的唯一事实源**。`internal/adapter`、mock 插件与后续真实扩展运行时都按这里的定义实现；代码与文档冲突时，先改文档再改代码（standards/03）。

| 文件 | 内容 | 状态 |
|---|---|---|
| [plugin-protocol.md](plugin-protocol.md) | JSON-RPC 2.0 over stdio：帧格式、方法、错误码、生命周期、超时与能力 | 定稿 |
| [plugin-manifest.schema.json](plugin-manifest.schema.json) | manifest（名称、入口、协议版本、权限 + 范围、资源上限） | 定稿 |
| `fixtures/` | 协议回放样本与 mock 插件的期望输出 | Phase 2 实现时加入；mock 在 `fixtures/plugins/mock-orchestrator` |
| manifest 文件名 | 插件目录下的 `plugin.json`，按 schema 校验；入口路径相对插件目录 | 定稿 |

## 规则

1. **版本是整数**：`manifest.protocol` 与 `initialize.params.protocol` 必须相等；破坏性变更才 +1，新增可选字段不算破坏。
2. **改协议要留痕**：修改本文或 schema 必须在 `CHANGELOG.md` 登记一句，并在 Phase 文档或 ADR 里说明动机。
3. **不写实现细节**：本文只定义线上形状与时限；Go 结构体、重试实现、UI 映射放在 `internal/adapter` 与 architecture/04。
4. **安全语义优先**：能力缺失一律拒绝（fail-closed）；协议不提供“默认放行”的开关。
5. **信任前提**：A 轨插件是**受信任代码**；本协议的能力声明约束的是宿主代管的操作，不是进程内原生能力（见 plugin-protocol.md §8）。
