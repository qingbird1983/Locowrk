# Permission Model Redesign（权限模型重设计·对齐 ZCode v1）

> **2026-10-10 v2.145 落地标注（全落 + 实机走查通过）**：§0 五条裁定全部兑现。实现落点
> 与原稿的偏差/补充记档五条：①`DENY_HARD` 未保留为 `NO_BLANKET_PREFIX` 集合——v1 粘性
> 规则只做精确命令、无前缀建议，钳制无消费方，集合整删（将来做智能前缀建议时再以
> ZCode `HIGH_RISK_ROOT_COMMANDS` 同款形态回归）；②粘性规则文件名 = `permissions/
> <fnv1a-hex8>.json`（工作区原值留档在文件 `workspace` 字段），每次查询直读文件无缓存
> ——人改 JSON 即生效，兼作撤销授权的手工出口，设置页另有逐工作区清除 + 打开文件夹；
 > ③子会话改道 = `ApprovalHub::resolve` 按 `-sub-` 形态映射父根会话（桌面会话 id 无该
> 形态无歧义），权限档/豁免/规则键位/越界门围栏/卡片工作区全取父；④设置页「命令终端」
> 分区（ShellSection + `shell.unlockRisky`）应用户裁定整区退役（2026-10-10 追加口谕
> 「终端的分类应该也没有用用了 删掉吧」），SettingsPage 删区回落外观兜底（持久化 id 落
> 已删分区不再白屏）；⑤走查校准两则记档：`.env` 从来不在隐私门清单（v2.26「宁漏勿错杀」
> 裁定，Auto 档 shell 读 .env 直跑系新旧一致行为），隐私门 path 类只认 `\` 归一后的
> 绝对路径形态（相对形态漏报 = 设计内，档位兜底）。
>
> **实机走查（CDP 隔离数据根 + CM 共享零明文 + deepseek 真轮；vite 5174 + 独立
> WEBVIEW2_USER_DATA_FOLDER + CDP 9226）**：W1 `node --version` 经 shell_read 免审直跑
> （mode:pure 审计，v24.14.0 实回）；W2 shell python 弹卡（卡上「本项目始终允许」+ 归属
> 工作区环境行）→ 落盘精确命令规则 `permissions/d11ef3989d493537.json` → 二轮同命令
> `rule:project-allow` 免卡直行（jsonl 铁证 + 截图「无需再次确认（本会话该命令已获批）」）；
> W3 subagent_run 派生零卡（mode:pure）+ 子内 shell 卡挂父会话带「子 Agent 调用」徽章
> （截图实证）→ 允许本次对话 → 第二次派生子内同工具零卡（改道父根豁免生效）；W4 Auto 档
> rm 真跑零弹窗（文件真删）；W5 Auto 档绝对路径 `.ssh` → 隐私门强制卡「SSH 凭据目录」
> + 两键硬门卡 + 允许一次执行；W6 tool-sandbox 不在插件加载清单（dev.log 实证）+ 设置
> 导航无「命令终端」；W7 「项目权限规则」管理块在工具与分发（计数 + 打开文件夹，截图）。
> 走查环境教训记档：种子 settings 需 `keyStorage: "keyring"` 标记（CM 双读钥匙，布尔
> true 直接解析失败降级演示模型）+ `active` 指到真实档 id；侧栏会话树按 fsRoots 字符串
> 精确匹配工作区（正反斜杠不一致 = 组空），组默认折叠需先点 `.ws-head` 展开。清理：数据
> 根整删（零明文盘证）、实例已停、tauri.conf.json 还原 v2.144 工作树态；脚本/截图留
> tmp/v145-walk/。门禁（最后一次编辑后全量复跑）：fmt 双侧 CLEAN、clippy 根 0 + desktop
> --all-targets 0、根 workspace 39 套件全绿、desktop 356 绿（+6 审批决策表/子会话改道
> +4 粘性规则，政策面测试按新语义重写）、vue-tsc 0、pnpm build 过、lint:style 0、
> check_length 32 < 基线 33（ShellSection 删除净降一；approval.rs 543 / tests_approval.rs
> 640 / PermissionCard.vue 455 系既有超限同职责面增长三问不拆）。
>
> 2026-10-10 立稿（v2.145 批先行）。病根：权限限制死板到真实项目没法用——解释器类命令
> 永久锁死（`node --version` 都不行）、`shell` 白名单外一律拒绝、`sandbox_run` 连解锁开关都
> 没有、连派生子代理都要弹审批卡。用户原话：「这个不能用 那个不能开……真实做项目的话就
> 没办法使用了。除非用户一直坐电脑边上看着，zcode 的机制就很完美的。」本稿按 ZCode 的权限
> 模型（无命令级永久禁令 / 只读免审 / 副作用审批 + 粘性授权 / yolo 字面全放行）重设计。
>
> 参考调研：ZCode 源码（apps/zcode-cli/packages/{core/permission, core/tool/handlers/bash-*, bootstrap/permission-options, adapters/config}，2026-10-10 实测六命令对照）；Locowrk 现状测绘（desktop approval.rs 决策表 + tool-shell/tool-sandbox 政策层 + kernel subagent 门控）。

---

## §0 用户裁定（2026-10-10，本稿地基）

1. **蓝本 = ZCode 模型**：无任何命令级永久禁令；解释器走审批不走禁止；只读分类免审；粘性授权；全自动档字面全放行。
2. **sandbox_run 退役**：ZCode 把沙箱整体拆掉（代码留有「sandbox 撤除」注释，执行面只留审批门）；我们的 sandbox_run 只限内存/CPU/进程数、不隔离文件系统与网络，工具描述自认这点，且策略层比 shell 更死。删插件，shell 成为唯一命令执行面。
3. **Auto 档 = ZCode yolo 同款**：除两道既有硬门（隐私清单 + 越出 fsRoots 的命令路径——此前「完全访问≠无安全」裁定保留）外零弹窗，rm/python/npm 全自动。
4. **子代理派生不再弹卡**：subagent_run 副作用 Irreversible → Pure。
5. **粘性授权**是「不用坐电脑边上」的正解：新增「本项目始终允许」持久规则。

## §1 问题诊断（现状，全部实证自代码）

### 1.1 三层锁死（用户实测六命令全灭）

| 用户实测 | 病根落点 |
|---|---|
| `shell → node/python/python3 --version` ❌ 永久锁定 | `tool-shell/src/policy.rs` `DENY_HARD`（约 50 项解释器/包装/网络/LOLBin），报文原文 "permanently locked … never allowed, not even via config" |
| `sandbox_run → node --version` ❌ 永久锁定 | `tool-sandbox/src/policy.rs` 同款 DENY_HARD 源码拷贝，且**无解锁开关**（"no unlock switch" 分支） |
| `shell → pwd` ❌ 不在白名单 | `shell` 工具策略末分支：非白名单 = 拒绝（"not whitelisted … extend via LOCOWRK_SHELL_ALLOW"） |
| 子代理派生必弹卡 | `kernel-agent/src/subagent/tool.rs:84` `SideEffect::Irreversible`（"一次派生 = LLM 花销 + 白名单工具执行，闸门交给人"）+ Ask 档决策表必达卡片 |

关键结构事实：`shell` 工具本身是 Irreversible——**审批机制早就在位**，政策层却把解释器拦死在「递不到审批」的位置，把白名单外命令拦死在「连审批都不给」。两套正交机制（审批管「要不要问」，政策黑名单管「有没有资格问」）中后者盖死了前者。

### 1.2 测绘中顺带发现的两个真缺陷（子代理面）

- **「允许本次对话」对子代理永不生效**：豁免集按 `(session_id, tool)` 键控（approval.rs:55），子会话 id 每次派生都是全新 `{parent}-sub-{n}` → 同一会话里第二次派生照样弹卡。
- **子会话权限档回落设置缺省而非继承父会话**：`PermissionModeStore::mode_of` 读子会话旁车（不存在）→ 静默回落 settings 缺省。父会话调到 Auto、全局缺省 Ask 时，子代理内每个非 Pure 工具都弹卡。

### 1.3 ZCode 对照（调研结论，实现时回查）

- **无命令级禁令**：全仓唯一硬编码集合 `HIGH_RISK_ROOT_COMMANDS`（bash/cmd/powershell/rm/chmod/dd 等）只管「审批卡不为其建议 Always-allow 前缀规则」，命令本身照常进审批。
- **只读分类器**：`READONLY_EXACT_ARGV_COMMANDS` = `["node","-v"]` / `["node","--version"]` / `["python","--version"]` / `["python3","--version"]` / `["ip","addr"]` → 免审直跑；`pwd` commandOnly；`node script.js` 类 → build 档弹卡。
- **粘性授权**：允许一次 / 本项目始终允许（持久化规则 `{tool, content?}`，`prefix:*` token 边界前缀 + 精确串）/ 本会话始终允许（内存、工具级）/ 拒绝（不产生规则）。
- **决策序**：alwaysAsk → yolo 全放行 → 配置 deny → 项目 deny/ask → plan 只读 → 项目 allow → 风险兜底。
- **沙箱退役**：契约残留 `sandbox` 字段零消费方；「vm 不是安全沙箱，隔离仍由上层权限与审批 gate 承担」。
- **子代理**：Agent 工具 `readOnly/low/needsApproval:false` 免派生卡；子 = 完整 runtime 共享父 PermissionService，审批经 broker 改写 sessionId 到父根会话（正对应我们 1.2 的两缺陷）。

## §2 设计

### 2.1 新决策序（desktop approval.rs，序即优先级）

1. **Plan 档非 Pure 硬拒**（不变）
2. **硬门**：隐私清单 + 越出 fsRoots 的 shell 命令路径 → 强制卡只许一次，全档位含 Auto（不变）
3. **项目拒绝规则 → deny**（新增；置于模式放行之前 = 用户明示拒绝连 Auto 也拦——比 ZCode 的 yolo 先于 deny 更安全，有意偏离记档）
4. **模式放行**：Pure / Auto（全放行）/ AcceptEdits（Compensable）（不变）
5. **项目允许规则 → allow**（新增）
6. **会话豁免**（不变）
7. **决策卡**（不变）

致命级（Setting and Constrained Optimization.md §3 语义）自此收敛为三项：**隐私门、越界门、元字符结构性拒绝**——兑现该文档自己的「致命数量要少到极致」原则。

### 2.2 tool-shell 政策层

- `decide()` 收敛两态：**Deny 仅结构性**（元字符/空命令/引号未闭合）；**其余一律 Allow**（shell 保持 Irreversible，放行即审批）。
- `DENY_HARD` 退役为 `NO_BLANKET_PREFIX`：唯一剩余职责 = 粘性规则建议钳制为精确命令（ZCode HIGH_RISK_ROOT_COMMANDS 同款）。
- `DENY_RISKY` + `unlockRisky` + `BASE_ALLOW` + `LOCOWRK_SHELL_ALLOW` 全退役：默认进审批是原「解锁放行进审批」的超集；白名单的「免拒绝」职责由只读面（shell_read）与粘性规则承接。
- proof.rs（shell_read 免审面，宁严勿松口径不变）：新增 `EXACT_ARGV` 精确表（node -v/--version、python/python3 --version，ZCode 原样四条）+ `pwd`（单词形态）+ git 集合追加 `--version`；敏感路径/git 全局选项/写旗标拒绝全保留。

### 2.3 粘性规则（新 `permission_rules.rs`）

- 存储：数据根 `permissions/<workspace_id>.json`，`{version:1, allow:[{tool, content?}]}`；坏 JSON 当空。
- 匹配：tool 名匹配；content 缺省 = 工具级；`prefix:*` = token 边界前缀（`subject == prefix || subject.starts_with(prefix + " ")`）；其余精确串。
- v1 粘性建议 = 精确命令（shell）/ 工具级（其余）；不做 CLI 注册表智能前缀（已知边界，matcher 语法已支持前缀供手改）。
- 设置页最小管理行：本项目规则计数 + 清除按钮。

### 2.4 子代理

- `subagent_run` → `SideEffect::Pure`（派生本身不动用户世界，子工具各有其门）；副产品：Plan 档可派生只读研究子代理（子目录被 Plan 围栏 Pure-only，ZCode Explore 同款）。
- 子会话审批改道：`ApprovalHub::resolve` 检测 `-sub-` 会话 id 改写为父根 id 再进决策表——一修三得：权限档继承父会话、「允许本次对话」跨派生生效、审批卡挂父会话（徽章已有）。kernel 契约零改动。

## §3 已知边界（记档）

1. 粘性规则 v1 精确优先，无智能前缀建议（无 CLI 注册表；matcher 支持 `:*` 前缀语法可手改 JSON）。
2. 项目拒绝规则先于 Auto = 比 ZCode yolo 安全向偏离（有意）。
3. `LOCOWRK_SHELL_ALLOW` / `LOCOWRK_SHELL_UNLOCK_RISK` env 幻影化（设置即启动 warn 一次，忽略）；`shell.unlockRisky` 设置键幻影化。
4. 已存会话钉死集看不到 sandbox_run（v2.64 回映射 miss 语义，自然退役）。
5. shell 工具保持 Irreversible——Ask 档下未粘住的命令每次弹卡（粘性规则正是承接面）；Auto 档不受影响。
6. shell_read 免审面只认 EXACT_ARGV + READ_ONLY 表，解释器其余形态一律不进免审面（宁严勿松）。

## §4 验收（实机走查六场景）

1. `node --version` 免审直跑（Ask 档）。
2. `python xxx.py` 弹卡 → 「本项目始终允许」 → 同命令二轮免审。
3. 子代理派生零卡；子内工具卡挂父会话带徽章；「允许本次对话」第二次派生不再问。
4. Auto 档 `rm` 真跑零弹窗。
5. `cat .env` 隐私门仍拦（Auto 也不例外，只许一次）。
6. sandbox_run 从工具目录消失。
