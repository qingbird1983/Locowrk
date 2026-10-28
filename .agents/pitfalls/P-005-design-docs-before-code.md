# P-005：阶段设计文档没落盘就开工

## 现象

Phase 3 已经做了 MyGo spike、写了 `internal/bridge`、开始加 MyGo 依赖，但 `docs/design/phase-3/` 还不存在；所有者指出“P3 文档没有先行，怎么开工”。

## 根因

把“phase 文档 §3 做什么”当成了设计文档；看到 Phase 3 文档已定稿，就默认满足“文档先行”，跳过了 `docs/design/phase-N/` 的设计稿。项目既有模式（Phase 2 有 `runtime-spike.md`）没有形成硬检查点。

## 修复

1. 撤回未提交的依赖改动（`go.mod`/`go.sum`），实现停在文档之前。
2. 补 `docs/design/phase-3/shell.md` 与 `frontend.md`，登记 `docs/README.md`，Phase 3 文档链到设计稿。
3. 重新走“设计定稿 → 实现 → 验收 → 报告”的顺序。

## 预防

- **开工前检查**：`docs/design/phase-N/` 至少有本阶段设计稿，且被 phase 文档与 `docs/README.md` 引用；没有就不写代码、不加依赖。
- 设计稿回答“接口/布局/数据流/测试/边界”，不是复述 phase 文档的“做什么”。
- 依赖类改动（`go get`、新模块）视为实现起点，同样受此门禁约束。
- 阶段报告记录设计稿路径；缺稿直接判文档先行失败。
