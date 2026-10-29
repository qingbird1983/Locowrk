# H-022：Phase 6 扩展打包工具

- 日期：2026-10-30
- 主题：`tools/extension-build`（esbuild 打包、可复现、`--check`）、夹具改构建产物

## 本轮做了什么

- `tools/extension-build/`：`package.json`（esbuild 0.28.2 锁定，`npm test`）、`build.mjs`、`test/build.test.mjs`、`README.md`。
- `build.mjs`：读 `extension.json`（name/version/capabilities/source/description），生成包装入口（`import factory` + `run(factory, meta)`），esbuild 打成 ESM 单文件，`node:*` external，shim 经 `@locowrk/shim` 别名内联；输出 `bundle.mjs` + `plugin.json` + `build.json`（esbuild 版本、入口、源与产物的 SHA-256，无时间戳）。
- 拒绝规则：`.node` 原生模块、`http(s)://` 远程导入、产物里的远程动态 `import()`/`require()`、未知能力、无 scope 能力。
- `--check`：重建后比对现有 `bundle.mjs` 与 `plugin.json`，不一致非零退出（CI 用）。
- `fixtures/extensions/orchestrator-sample` 增加 `extension.json` 源码描述，`bundle.mjs` 改用打包产物（自包含，9.3 KB），并提交 `build.json`。
- 新增 `internal/extensions/built_sample_test.go`：没装 `tools/extension-build/node_modules` 时跳过；构建到临时目录 → `Discover` → 信任 → 启动 → 调用 `sample_echo`。

## 关键结果

- `npm test`（tools）：4 项通过（打包、可复现、`--check` 通过/检出篡改、三类拒绝）。
- `TestBuiltSampleEndToEnd`、`TestNodeSampleEndToEnd`：通过。
- `gofmt`/`go vet`/`go test -race ./...`、`check-docs`：全绿；Phase 6 仍未打 tag。

## 遗留（Phase 6 未收尾）

1. 真实 `todo`/`rpc-demo`：GitHub/jsdelivr/镜像均不可达；不要用 Phase 2 spike 旧 bundle 顶替（P-010）。
2. 原生对话框 input/editor；面板撤销信任/通知/命令参数；技能信任撤销冒烟。
3. 真实 Node 崩溃/超时/进程树/工具冲突；发布构建缺运行时 fail-closed（当前开发回退只在开发模式）。
4. CI 尚未把 `tools/extension-build` 的 `npm ci && npm test` 与 `--check` 纳入门禁脚本（当前手工执行）。

## 已提取的沉淀

- 沿用 P-006（不要用 PowerShell 写中文文件）、P-010（受限网络）；无新坑。
