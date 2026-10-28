# ADR-0012：内核源码进仓库，作为自有分支维护

- 状态：定稿
- 日期：2026-10-28
- 关联：ADR-0001（被取代）；ADR-0005；architecture/01、02；standards/01、04、07；`pi/FORK.md`

## 背景

ADR-0001 决定以 Go module 依赖复用 `sky-valley/pi`，不复制源码。Phase 1~3 按此执行，内核升级靠固定提交。

所有者现在明确要求：**内核源码必须进本仓库，与上游分离，后续由本项目维护，并可能按需求二次调整**。继续用外部模块依赖无法满足“在仓库里直接改内核、与上游解耦”的目标。

这是对 ADR-0001 的取代，不是补丁：依赖形态、升级方式与代码所有权都变了。

## 决策

把 `sky-valley/pi` 的**运行所需源码** fork 进本仓库 `pi/`，改模块路径，从此由本仓库维护。

- 导入范围（非测试源码）：

  | 目录 | 内容 |
  |---|---|
  | `pi/agent/` | Agent loop、工具调用、steering、transcript |
  | `pi/ai/`（含 `providers/`、`models_catalog.json`） | 模型协议、provider、流式解析、校验 |
  | `pi/coding/` | coding agent、JSONL 会话、压缩、技能、shell 工具 |
  | `pi/internal/jstext/`、`pi/internal/nodepath/` | 上述包的内部依赖 |
  | `pi/telemetry/` | 内核依赖的遥测接口（默认关闭） |

- 不导入：上游测试、`docs/`、`cmd/`、`examples/`、`chord/`、`env/`、`eval/`（当前依赖闭包不需要）。
- 模块路径改为 `github.com/qingbird1983/locowrk/pi/...`；`go.mod` 不再 require `github.com/sky-valley/pi`。
- 保留上游 `LICENSE`（MIT）与版权头；`pi/FORK.md` 记录基线提交、导入范围、差异与维护规则。
- **依赖方向**：`pi/` 只依赖标准库、第三方包与自身子包；不得 import 本仓库的 `internal/*`、`cmd/*`。`internal/kernel` 仍是唯一引用 `pi/...` 的包，其他包继续只认项目 DTO。
- **上游角色**：未改动部分的行为仍以 pi / `sky-valley/pi` 为蓝本；本地行为调整必须先写 ADR 或 Phase 文档，并在 `pi/FORK.md` 的差异表与 CHANGELOG 留痕。
- **升级方式**：以上游新提交与基线 `6f6131ba` 做 diff，挑选相关改动手工移植（含安全修复），跑全量门禁；不做自动合并。

## 理由

- 目标从“复用内核”变成“拥有并演进内核”：in-repo fork 是唯一能同时满足“可改、可控、可审计”的形态。
- 只导入依赖闭包而非整个仓库：上游测试/文档/Cmd 对本项目没有直接价值，反而放大体量与升级噪声；回归由本仓库的分层测试与阶段验收承担。
- 保留上游许可证与基线记录，既合规，也让“哪些是上游行为、哪些是我们改的”可追溯。

## 后果

- 内核 bug 与安全修复不再随 `go get -u` 自动到来，必须主动跟踪上游；这是自有分支的主要代价。
- 上游测试未入库：升级后若怀疑行为回归，需要临时检出上游跑其测试，或在本地补针对性测试。
- 仓库体量增加约 **2.6 MB / 132 个文件**（vendored 源码，standards/08 已登记豁免）；`internal/kernel` 的封装边界不变。
- `docs/research/01` 的“版本钉”从 module 版本改为“上游基线提交 + 本地差异”。
- 代码评审要区分“上游原有代码”与“本地改动”；本地改动处用 `// Locowrk fork:` 注释。

## 护栏

- 许可证与版权头不得删除；新增文件沿用 MIT 或项目风格。
- `pi/` 内不得出现本仓库的 import；`go list -deps ./pi/...` 不包含 `internal/`、`cmd/`。
- 每次本地行为改动：ADR/Phase 文档 + `pi/FORK.md` 差异表 + CHANGELOG，三者齐全才算完成。
- 升级内核独立提交，更新基线提交号，跑 `gofmt`、`vet`、`-race` 与阶段验收。
- 不把上游测试/文档整批搬入；需要参考时按 commit 检出，不复制进仓库。

## 备选（否决）

- **继续 module 依赖**：无法在仓库内改内核，与所有者要求相反。
- **整仓复制（含测试/docs/cmd）**：体量与维护噪声大，测试还引用上游 testdata，收益低。
- **Git submodule**：仍需额外克隆与提交指针，编辑体验不是“本仓库代码”，且与“与上游分离”冲突。
