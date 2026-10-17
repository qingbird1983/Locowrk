# ADR-0006：插件元数据两份声明的权威划分——能力看代码，展示看清单，重叠区必须相等

- 状态：已接受（其中"市场页徽章要不要改成运行期来源"仍待用户裁定，见后果·后续）
- 日期：2026-10-18
- 决策人：青鸟（对第二节十条的总授权，原话"这10条可以收账，同意"）；重叠区内部怎么划由本轮实测判定
- 出处：台账 [`specs/03`](../../specs/03-structural-debt/spec.md) 的 T11 ·
  审计批 F-21c 登记的那条"双真源漂移、方向待裁定"（`docs/history/audit/AUDIT-FIX-PLAN-v1.md`，⊘ 不入库）
- 取代：无
- 被取代：无

## 背景

一个内置插件的元数据同时存在两个出处，而且字段重叠：

1. **插件代码的 `describe`**——`plugins-src/<id>/src/lib.rs` 里构造的 `PluginManifest`（展示位经
   `plugin_api::meta::official_meta` 填）。宿主装载时读它：工具池由它的 `tools` 建立，
   审批判定读它的 `side_effect`。
2. **磁盘 `plugins-src/<id>/manifest.json`**——`plugin-service::display::read_display` 读它，
   UI 的名称、图标、描述与副作用徽章都从这里画；打包时原样复制进版本目录。

重叠区 = 每个工具的「描述 / schema / side_effect / exposure」+ 七项展示元数据。
它已经真的咬过人：审计 H8 抓到清单写 `Pure` 而代码早已 `Irreversible`（**用户看到的徽章与内核
真实的审批行为不一致**）；修完之后逐插件再比，重叠区仍然长期各写各的。

约束（这些决定了哪条路走不通）：

- `crates/plugin-api` 是冻结 ABI：只许把字段追加到结构体末尾，改已有签名算 breaking。
- UI 必须在插件**未装载**时也能画卡片——市场目录列的是打包产物，禁用态插件根本不进内存。
- `tool-mcp` 的远端工具只在运行期存在（从 `<数据根>/mcp.json` 发现，连 side_effect 由用户配置的
  grade 决定），静态清单承载不了它们。
- `version` 由 `scripts/build_plugins.sh` 的 PLAN 清单注入两侧（代码侧 env 宏 + 产物 JSON 的 sed），
  源码里那份只是占位。

## 决策

**能力字段（`tools` / `schema` / `side_effect` / `exposure` / 工具描述）以插件代码的 `describe` 为权威；
展示字段（`display_name` / `description` / `icon` / `author` / `origin` / `homepage` / `license`）
以磁盘 `manifest.json` 为权威；两份在重叠区必须逐字段相等，由每个插件自带的**一条 `cargo test`
对账**承重（插件 crate 都在根 workspace 里，跑 `make check` 就跑得到）。**

- 比对器、对账入口、以及只在 env 门控下才动文件的同步器都放在 `plugin_api::meta`
  （`manifest_drift` / `drift_of_describe` / `sync_manifest_file`），十个插件只写那一条同形状的测试——
  让十个插件各写一遍"怎么读磁盘、比哪些字段"，就一定有第九个漏比某个字段，而漏掉的那个字段
  正是漂移藏身的地方。
- 条数与在场性由 `scripts/check-single-source.mjs` 的 SS7 / SS7b 盯：期望值取**插件目录数**，
  不是写死的条数，所以加一个插件而没配对账就当场红。
- `version` 刻意不比；运行期追加工具按"清单必须是代码清单的前缀"放行——顺序在这里就是边界。

## 被否决的方案

| 方案 | 否决原因 |
| --- | --- |
| 反过来：以磁盘 `manifest.json` 为能力权威 | 实测那份在能力面上是**旧的**：`fs_write`/`fs_delete` 与三个 `edit_*` 缺 `expected_revision`（模型目录里那份并发写保护的说明与 required 项）、`memory_write` 缺 `scope`（项目层/全局层）、四处 `exposure: always` 缺失（丢了就被踢回候选池，可能被 Top-N 挤出目录）。照它装载 = 把这些保护抹掉，是行为退化不是收敛 |
| 删掉清单的 `tools` 段，UI 一律读 `describe` | 未装载的插件没有 `describe`，市场目录与禁用态卡片就失去工具清单与徽章 = 用户可见信息倒退。这不是技术选择而是产品取舍，不在这次授权范围内 |
| 打包时 `dlopen` 产物、由 `describe` 生成清单 | 能同时拿到"单源"和"市场可见"，但把"插件能不能加载"引进构建链（Windows 与 WSL 两侧都要 dlopen，多一类失败模式），代价超出这一刀；登记为后续可选形状 |
| 只把当下这批文案对齐，不立判据 | 审计 H8 已经证明它会漂：修过一次而没有闸，下一轮改代码的人照样不知道清单要同步 |
| 给 `tool-mcp` 的运行期计数留特例（展示描述里写"本次发现 N 个"） | 实测展示面**零消费者**（UI 读磁盘、`plugin-host` 只用 `.tools`），而"发现了几个"本来就由 `mcp_status` 这个管理工具报。留特例等于为一个没人读的字段给判据开口子 |

## 后果

**正面：**重叠区再漂就是 `make check` 红，而且是**逐字段点名**的红（差异清单照抄就能改对）；
UI 的徽章与内核真实的审批行为第一次被机器保证同源；`fs_write`/`edit_*`/`memory_write` 那三处
"清单根本不知道"的能力字段第一次进清单。

**负面 / 需要接受的代价：**改一个工具的描述要动两处（代码 + 跑一次同步），中间那次对账红由人读；
`origin` 这类安全默认仍同时存在于代码（`official_meta`）与清单，靠判据而不是靠结构保证；
十个插件各带一条同形状的测试（机械重复，但由 SS7 数着条数，缺一个就红）。

**后续需要跟进的事项：**

- 市场页徽章要不要改成"运行期来源 + 未装载时明写未知"（要用户裁定；它决定清单的 `tools` 段最终能不能删）。
- 清单成为展示权威后，`official_meta` 的展示实参是否收掉——它在 `plugin-api` 的既有对外签名里，属 `[LOCK]`。
- 打包期生成（第三条被否决方案）如果要做，先解决"两侧都要 dlopen"的失败模式与耗时。

## 参考

- 台账 T11：[`specs/03-structural-debt/spec.md`](../../specs/03-structural-debt/spec.md)
- [`ADR-0005`](0005-linux-verified-on-both-platforms.md)（同一条判据在 Windows 与 Linux 两侧都要跑）
- 审计批 F-21c 的"双真源漂移（方向待裁定）"登记：`docs/history/audit/AUDIT-FIX-PLAN-v1.md`（⊘ 不入库）
