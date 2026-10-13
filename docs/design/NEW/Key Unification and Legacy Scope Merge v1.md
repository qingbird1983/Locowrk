# Key Unification and Legacy Scope Merge v1（Q7c 取证稿）

> **状态**：**已兑现（v2.197 · 提交 `cfadccd`）** —— §4 的方案与 §5 的清单已在 Q7c 主体批落地；
> ⚠ **本稿 §2 的喂点列举有两处失真（处数 + 行号），订正注写在 §2 末尾**；§4 的落点随实测改了（从「喂点各过一次」改成「视图构造点一处收口」）。
> **状态（立稿时）**：待研究 —— 本稿只交付**实测与方案**，不含任何已开工的代码或磁盘动作。
> **依据**：施工队列 Q7c（本表 §6 第 15 条的挂账已由 v2.195 续批结清，剩这两面）、
> 《Memory Redesign (ZCode Alignment) v1》§8 的 M3、`plugin-api/src/wskey.rs` 头部那段
> 明写的「不动记忆树的 scope 编码……涉用户数据目录、要单独确认」。
> **读数时刻**：2026-10-14 本机实测（`C:/Users/42527/.Locowrk`，`LOCOWRK_DATA_DIR` 未设 =
> 走缺省 `USERPROFILE\.Locowrk`）。体积与条数都是**当刻**读数，跨批复跑必须重取
> （v2.195 第 60/61 条），不许照本稿的数字做判断。
> **取证工具**：`tmp/q7c-scan.mjs`（全盘列写法）、`tmp/q7c-hash.mjs`（两套目录编码的复现）。

## 0. 一句话结论

键位在**数据库主键**那一侧已经归一（Q2 兑现），但在**磁盘目录名**这一侧还没归一，而且
磁盘侧本来就有两套互不相同的编码。后果不是"目录名难看"，而是**同一个项目的记忆现在正躺在
两个目录里、各自被一条通路读写**，并且它会继续出现在记忆页的会话清单里（扫描按目录遍历，
不按键位）。这一刀 = 把三条通路喂给磁盘的串统一到 `wskey::normalize`，再把存量两份合并成一份。

## 1. 磁盘实测（当刻读数，全部可复核）

| 位置 | 目录 / 文件 | 内容 | mtime（UTC） | 判定 |
|---|---|---|---|---|
| `memory-tree/` | `F__Warehouse_TITY-94676677/` | 3 份投影 json（章节 5／7／2，游标 434／270／144） | 2026-10-13T15:39 | **活的**，走的是机器编码 |
| `memory/projects/` | `TITY-94676677/` | `tree.md` 19,649 B | 2026-10-13T07:18 | 人读层的**旧写法**落点 |
| `memory/projects/` | `tity-9242b25e/` | `MEMORY.md` 641 B + `facts/`（6 个 md） | 2026-10-13T15:39 | 人读层的**归一后**落点 |
| `memory/.sync/projects/` | `tity-9242b25e/` | `facts` 同步标记 | 2026-10-13T07:17 | 只有归一形 = 新通路自洽 |
| `memory.sqlite3` | 表 `memories` | workspace 分组 = `f:\warehouse\tity` **6 行（一组）** | — | 主键侧已归一，零迁移 |

同一项目的旁车写法集合（`sessions/` 八个文件）：`.jsonl` 与 `.meta.json` 里都是
`F:\Warehouse\TITY`；其中 `desktop-1791905124157.jsonl` **同时**含 `f:\warehouse\tity`
（某条事件参数写的是归一形）。⇒ 结论：**旁车里两种写法并存是事实，且旁车不许改**
（会话事件只追加不删除，§设计纪律 5）——归一只能发生在"把串喂给键位的那一刻"，不在存储侧。

## 2. 三套形状在哪（代码点，逐个带证据）

| 通路 | 磁盘形状 | 编码构造点 | 输入是否已归一 | 证据 |
|---|---|---|---|---|
| A `memories` 主键 | sqlite 列值 | 宿主 `workspace::scope_key` → `plugin_api::wskey::normalize`；插件 `WsCtx` 同一份 | ✅ 是 | `wskey.rs:40`、`memory-sqlite/src/lib.rs:275`、实测 6 行同组 |
| B 树投影目录 | `memory-tree/<scope_dir(ws)>/<session>.json` | `memory-tree/src/storage.rs:31 scope_dir`（整串安全化取前 40 + fnv1a 高 32 hex8） | ❌ 否 | 磁盘目录名 = 未归一形的复现值（§3 复现） |
| C 人读 md 目录 | `memory/projects/<project_dir_name(ws)>/…` | `plugin-api/src/memlayout/mod.rs:63 project_dir_name`（**路径末段**安全化 + 同一 fnv 短哈希） | ❌ 否（tree.md 侧）／✅ 是（facts 侧） | 同一目录里两个名字，各对应一种输入形状 |

B 与 C 的喂点（全部现读，不是推测）：`organize/assemble.rs:154` 与 `:205`、
`organize/backfill.rs:56`、`organize/collapse.rs:36`、`organize/debug.rs:49`、
`organize/task.rs:222` 与 `:269` —— 清一色 `(self.workspace_of)(session_id).unwrap_or_default()`，
即**旁车原样串**；同文件里只有 `organize/facts.rs:50` 与 `:67` 过了 `scope_key`。
⇒ 同一条链上，事实卡走归一形（所以 `tity-9242b25e` 里是新的），树投影与 tree.md 走原样形
（所以 `F__Warehouse_TITY-94676677` 与 `TITY-94676677` 里也是活的）。这就是"双键位劈成两份"在
磁盘上的成因，**不是历史遗照**。

> ⚠ **§2 这段的订正（v2.197 落地时实测；登记在 ROADMAP v2.197 第②条）**：
> ①**处数错**——上面列的 7 行不是喂点全量，视图调用点实测 **16 处**（`memory.rs` 2 + `organize/` 内 14：
>   assemble 2／backfill 1／collapse 1／debug 1／pressure 1／project_history 1／recovery 1／
>   search_titles 1／summary 1／tree_tools 2／trigger 2）。错因 = 取证时 `grep` 的输出被 head 截断，而我把截断的读数当全量读数写进文档。
> ②**行号错**——`organize/task.rs:222` 与 `:269` 那两行**不是** `(self.workspace_of)(…)` 直取，
>   它们是 `self.sync_tree_md(&task.workspace)`，吃的是 `OrganizeTask.workspace`（该字段本源仍是视图，所以归一后自动同形）。
> ③**方案随读数改了**——原 §4「五处过 `scope_key`」不成立（16 处逐个补必漏），落点收在 `workspace_view` 一处。
> ④**本稿 §2 还漏了整个第五个面**——`approval/` 两处经 `ctx_of` 直取 resolver 产出，落到项目授权规则库
>   文件名（fnv1a64 吃原样串），**未归一未迁移** = 队列新增 **Q7d**。

## 3. 复现前置判据（不许拿想象当目录名）

合并动作的依据是"目录名能不能被算出来"，所以复现必须先自证：拿磁盘上**已有的三个答案**反验
`tmp/q7c-hash.mjs`（fnv-1a 64 位 + 安全化 + 高 32 位 hex8，两份实现同一算法）——

| 反验项 | 复现值 | 磁盘实际 | 结果 |
|---|---|---|---|
| `scope_dir("F:\Warehouse\TITY")` | `F__Warehouse_TITY-94676677` | 同 | OK |
| `project_dir_name("F:\Warehouse\TITY")` | `TITY-94676677` | 同 | OK |
| `project_dir_name("f:\warehouse\tity")` | `tity-9242b25e` | 同 | OK |

`HASH_REPRO_FAILED=0` 三条全中 ⇒ 同一工具算出的**归一后目标名**可用：

- `scope_dir("f:\warehouse\tity")` = **`f__warehouse_tity-9242b25e`**（磁盘上不存在 ⇒ 无同名冲突）
- `project_dir_name("f:\warehouse\tity")` = **`tity-9242b25e`**（已存在，但其下**没有** `tree.md` ⇒ 移入不覆盖）

顺带测出的一件事：同一个项目的三种**写法**（反斜杠／正斜杠／带尾分隔符）会算出**三个不同哈希**
（`94676677`／`bb23643f`／`a1298b2c`）——这正是 `wskey.rs` 立项理由的磁盘版证据。

## 4. 合一方案（代码面，先于任何磁盘动作）

1. **五个喂点过 `scope_key`**：`organize/{assemble,backfill,collapse,debug,task}.rs` 里
   `(self.workspace_of)(…)` 的输出统一走 `crate::workspace::scope_key(Some(ws))`。
   一处归一、三套形状同形；旁车字节不动。
2. **B 与 C 的编码保持各自独立**（不合并算法）：`scope_dir` 是机器投影目录（整串前缀，
   可寻址到"哪个盘哪个目录"），`project_dir_name` 是人读目录（末段前缀，要好看）。
   **要合一的是"喂进去的键"，不是"两套编码"** —— 这一条是本稿与"顺手把两个函数并成一个"的
   分界，后者会让人读目录名变长并且使存量 `tity-*` 全部换名（扩大风险面而不解决问题）。
3. **投影内的 `workspace` 字段跟着改**：三份 json 里现在是 `F:\Warehouse\TITY`。
   `scan_all` 第二遍按 `scope_dir(summary.workspace)` 重取 mtime（`storage.rs:163`），
   所以**只换目录名不改字段 = 路径与实际目录不符**，清单里 mtime 归 0、排序垫底。
   ⇒ 合并面有两半：目录名 + json 字段。这一条是本稿最容易被漏掉的点。

## 5. 存量合并清单（交用户逐条过，动手前必须先备份）

| 序 | 动作 | 源 | 目标 | 冲突 | 回退 |
|---|---|---|---|---|---|
| 1 | 备份整个数据根的这三处 | `memory-tree/`、`memory/projects/`、`memory.sqlite3` | `backups/q7c-<时间戳>/` 逐字节副本 + sha 清单 | — | 反向复制 |
| 2 | 目录换名 | `memory-tree/F__Warehouse_TITY-94676677/` | `memory-tree/f__warehouse_tity-9242b25e/` | 目标**不存在** ⇒ 纯改名 | 改回原名 |
| 3 | 改写 json 字段（3 份） | `"workspace": "F:\\Warehouse\\TITY"` | `"workspace": "f:\\warehouse\\tity"` | 无 | 用步骤 1 的副本写回 |
| 4 | 人读层并入 | `memory/projects/TITY-94676677/tree.md` | `memory/projects/tity-9242b25e/tree.md` | 目标目录存在但**无同名文件** ⇒ 不覆盖 | 移回 |
| 5 | 空壳处置 | `memory/projects/TITY-94676677/`（步骤 4 后为空） | 改名 `TITY-94676677.legacy-<时间戳>` 保留 | — | 改回 |
| 6 | sqlite | **不动**（实测 6 行已是归一形） | — | — | — |

执行时机 = 应用**停机**窗口内（整理调度与副车写入都在跑，边写边换名会造出第三种状态）。

**实际裁定与落点（v2.197）**：两个决策点由用户原话「那些记忆数据无所谓的，本来就是些测试数据，没啥用的，继续下一步吧」当场有解——合并形态取**甲**（一次性显式迁移，不留兼容读分支）；旧名去留从「改名 `.legacy` 保留」降级为**备份后迁移**。**他的「无所谓」免除的是停机窗口与旧名保留，不是「保住树游标」**：删除会让那 3 份投影的 `chaptered_until`（434／270／144）归零、下次整理把整段历史重切一遍，而改名 + 精确改写 json 字段零成本保住它，所以动作从本稿原写的「备份后删除」改成「备份后迁移」。**停机窗口的替代** = 两阶段迁移 + A 阶段末重扫文件集合（不一致即 BLOCK），而不是把应用关掉。
本稿不代为执行：以上六条在用户逐条确认前一条都不动。

## 6. 两个待裁定决策点（已由用户 2026-10-14 裁定；实际取向见 §5 末尾的落点段）

**决策一 · 合并形态**

- **甲（推荐）一次性显式迁移**：按 §5 六条在停机窗口做完。优点 = 代码里不留兼容分支、清单立刻
  单份、一次可回退。代价 = 要停机一次，且迁移本身是"动用户磁盘"。
- **乙 惰性兼容读**：`load`／`scan` 未命中归一形时回落到旧编码，下一次 save 自然搬到新名。
  优点 = 不停机。代价 = 兼容分支永久住在代码里；`list_scopes` 按目录遍历 ⇒ 旧目录在搬空前
  会**继续出现在会话清单里**（双份来源不消除）；搬完还留空壳需要另一次清理。

**决策二 · 旧名字的去留**：合并后旧目录（与旧 json 字段值）是 ①删除、②改名 `.legacy-<时间戳>`
保留一个版本周期、还是 ③原地不碰。本稿建议 ②——与本仓"退役要能回退"的既有做法同形
（`memory.sqlite3.bak-20261004` 就是现役先例）。

## 7. 判据设计（动工那一批要交付的东西）

1. **形状判据**：五个喂点必须调用 `scope_key`（源码级判据，历批 §6 第 13 条：接线存在性只能
   由源码证明）；`organize/` 子树里不许再出现裸的 `workspace_of)(…).unwrap_or_default()` 直接喂路径。
2. **行为用例**：同一项目的四种写法（`F:\Warehouse\TITY`／`f:/warehouse/tity`／带尾分隔符／
   verbatim 前缀）必须落进**同一个** `scope_dir` 与同一个 `project_dir_name`。
3. **复现判据**：`tmp/q7c-hash.mjs` 的三条反验必须全中才允许脚本给出目标名 ——
   **合并依据必须是被磁盘验过的算法复现**，不是"我以为哈希是这个值"。
4. **迁移自证**：迁移前后逐条对账 —— 文件数守恒、每份 json 的 `chaptered_until` 与章节数不变、
   移入的 `tree.md` 逐字节相同、备份目录 sha 清单与源逐项一致。任一项不符 = 回退而不是继续。
5. **变异靶（各证一档独有）**：把某个喂点的 `scope_key` 摘掉 ⇒ 形状判据必须红；把迁移脚本的
   字段改写去掉只改名 ⇒ 对账必须红；把备份步骤跳过 ⇒ 迁移必须拒绝执行（而不是先做完再谈回退）。

## 8. 边界（如实）

- 本机只有**一个项目**（TITY）与 8 份旁车 ⇒ 冲突清单是"这一台机器"的，不是通用的；
  迁移实现必须按**扫出来的实际目录**算，不许把本稿的目录名写进代码。
- 跨机器/跨平台不统一（`wskey.rs` 已裁定：POSIX 不折叠大小写）。
- 会话旁车里的原样串**不改**（追加不删除纪律）；因此旁车与键位会永久差一个"归一"映射，
  这是设计而不是缺陷，本稿写破它以免下一批当 bug 修。
- `tree.md` 的人改保护按 mtime 判定，换名后 mtime 由文件系统保留；若步骤 4 触发"人改更新"
  判断导致下一次不重建，属预期行为（宁缺勿假）。
- 无基准数，未真机走查（走查项 = 记忆页会话切换器在迁移前后各截一次清单，比对条目数不翻倍）。
