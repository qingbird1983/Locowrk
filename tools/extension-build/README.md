# extension-build

把 A 轨扩展源码打包成可直接运行的扩展包：`plugin.json` + `bundle.mjs`（shim 已内联）+ `build.json`。设计与边界见 [design/phase-6/runtime.md](../../docs/design/phase-6/runtime.md) §4。

```bash
cd tools/extension-build && npm install

# 打包
node tools/extension-build/build.mjs \
  --src fixtures/extensions/orchestrator-sample \
  --out /tmp/orchestrator-sample

# 校验已有产物与源码一致（CI 用）
node tools/extension-build/build.mjs --src <src> --out <pkg> --check

# 工具自测（可复现、拒绝原生模块/远程 import、--check）
cd tools/extension-build && npm test
```

源码目录需要：

```text
extension.json   {name, version, capabilities?, description?, source?}
index.ts|index.mjs|index.js   export default function (pi) { ... }
```

约定：

- `name` 小写 kebab-case；`version` 以 `x.y.z` 开头；`capabilities` 每项必须带非空 `scope`（与适配器一致的 fail-closed）。
- 只允许纯 JS 依赖；`.node` 原生模块、`http(s)://` 远程 import、远程动态 import 一律拒绝。
- 产物不含时间戳，同样输入产生同样字节；`build.json` 记录 esbuild 版本、入口与 `bundle.mjs` 的 SHA-256。
- `node:*` 保持 external；shim 来自仓库 `extensions/shim/host.mjs`，随包内联。
- 运行期不构建、不 `npm install`、不远程加载代码（ADR-0015 护栏）。
