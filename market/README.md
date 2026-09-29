# market/ — 市场目录（阶段七，DESIGN.md §13.3）

本地目录市场：桌面端"发现"页的市场目录就是这里的文件布局。
真实市场服务出现前的替身，由 `Market` trait 抽象（`plugin-service::market`）。

## 布局

```
market/
└── tool-shout/          # 插件 id（kebab-case）
    ├── 1.0.0/
    │   ├── tool_shout.dll      # 平台库（Windows；.so/.dylib 同名规则）
    │   └── manifest.json       # 市场元数据 { id, version, description }
    └── 1.1.0/
        └── ...
```

## 重新生成

```bash
bash scripts/build_market.sh
```

二进制产物**不入库**（.gitignore，与 plugins/ 同规则）。新克隆后先跑
构建脚本，桌面端市场页才有可安装项；`plugin-service` 的编排集成测试
同样依赖这里的暂存（缺失时相关用例打印 skip 通过）。

## 与开发态 plugins/ 的关系

两套目录互不重叠（§11.3）：`plugins/` 是内核开发直接装载的开发态，
`market/` 是"从市场安装"的货源地，安装后落到安装态
`%USERPROFILE%\.Locowrk\plugins\`（`LOCOWRK_DATA_DIR` 可覆盖），
由 lockfile 管理、桌面端插件面板操作。
