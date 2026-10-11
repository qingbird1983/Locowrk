# market/ — 市场目录（阶段七，DESIGN.md §13.3）

本地目录市场：桌面端"发现"页的市场目录就是这里的文件布局。
真实市场服务出现前的替身，由 `Market` trait 抽象（`plugin-service::market`）。

## 布局

```
market/
└── tool-shout/          # 插件 id（kebab-case）
    ├── 1.0.0/
    │   ├── tool_shout.dll      # 平台库（Windows；.so/.dylib 同名规则）
    │   ├── manifest.json       # packaging v2 全量 manifest（见下）
    │   └── assets/icon.svg     # manifest.icon 指向的相对路径
    └── 1.1.0/
        └── ...
```

> **⚠ v2.149 批 E 订正（两处）**：
> ① `manifest.json` **不是**早期设想的 `{ id, version, description }` 三键 v1 形状——
> 本目录由 `scripts/build_market.sh` 生成 **《Plug-in packaging.md》v2 全量 manifest**
> （`abi_version` / `kind` / 双语 `display_name`·`description` / `icon` / `author` /
> `origin` / `license` / `tools[]`（含 `schema` 与 `side_effect`）），installer 与市场
> 目录都按它解析；这份 JSON **硬编码在构建脚本里**（`plugins-src/tool-shout/` 没有
> manifest.json 源文件），改元数据要改脚本——与 `plugins/` 侧「源 manifest + sed 版本号」
> 是两条不同通路（同源重复，已登记《AUDIT-FIX-PLAN-v1》批 F·F13）。
> ② 版本基准：v2.56 用户裁定「全部插件 0.1.0 起步」，`plugins-src/` 九件已随批重置，
> **本夹具仍是 1.0.0 / 1.1.0**（`fallback.rs` 等集成测试按目录动态枚举版本、不写死号，
> 保留 1.x 恰好持续覆盖「旧 1.x lockfile 条目由 `resolve_latest` 天然回退」这条路径）。
> 属**已知偏差、非缺陷**，是否随基准重置为 0.1.0/0.2.0 待用户裁定，批 E 不擅自动产物。

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
