# Locowrk 模型能力与多模态阶段计划

> 配套项目：Locowrk
> 版本：1.0（2026-10-07）
> 上游设计：《模型供应商配置与能力探测设计 v2.0》《多模态支持设计 v2.0》
> 参照调查：PI-Desktop（F:\Warehouse\Github\PI-Desktop）、ZCode（F:\Warehouse\Github\ZCode）模型配置全量调查（2026-10-07）
> 状态：**待用户裁定后动码**——裁定表见 §四

---

## 一、总览

六个批次，A→F 顺序执行；A/B 是 C/D/E/F 的数据与语义地基。每批独立交付、独立可回退，家法门禁照旧。

| 批 | 名称 | 一句话 | 规模感 | 依赖 |
|---|---|---|---|---|
| **A** | 模型能力目录 | models.dev 快照接入 + 打分匹配 + source 钉死 + 能力投影到 UI | 大（新模块 + 快照资源） | 无 |
| **B** | 推理档位 + 输出钳制 | 方言数据表 + 会话档位持久化 + composer 档位钮 + maxOutput 预检钳制 + 辅助降档 + 发送门控 | 大 | A（梯子/模态数据） |
| **C** | 连接与安全 | keyring 迁移 + 自定义请求头 + 端点扫描 + 逐模型测试 + 网络代理 | 中 | 无（可插在 A/B 间） |
| **D** | 多模态地基 | ContentPart + wire 编码 + AssetStore + 上传链路 + 渲染 + 估算 | **最大**（内核穿透） | A/B（模态数据 + 钳制公式） |
| **E** | 多模态进阶 | 查看器 + 工具结果投影 + 多模态工具 + 降级链 | 中 | D |
| **F** | 生态 | 子代理模型档 + 回退链 + 从其他工具导入 + 推荐三选 | 中 | A/B |
| **G**（候补） | 可靠性补强 | Turn Commit length 闸（随 B）+ Unknown Effect 恢复 + Revision Guard + 只读证明 + 子代理验收（随 F） | 各小-中 | 无硬依赖；G1 随 B、G5 随 F |

> C 与 A/B 无依赖，若 keyring/代理急可先插；其余严格按序。

---

## 二、批次卡片

### 批 A — 模型能力目录

**目标**：能力数据地基。模型配置页从"只有名字"升级为"有窗口/输出/模态/梯子/价格"。

**范围**：
1. 快照资源：`desktop/src-tauri/resources/models.dev/api.json` + tauri.conf.json `bundle.resources`；`llm/catalog.rs` 惰性加载 + 投影裁剪（CatalogModel，只留 §4.2 字段）
2. 匹配：`llm/catalog_match.rs`（归一化剥后缀 + 别名/host 打分；守 400 行拆两文件）
3. 命令三件：`llm_catalog_status` / `llm_catalog_refresh`（在线只进内存）/ `llm_lookup_model`
4. `ProviderModelView` 扩展（max_output / input_modalities / reasoning_ladder / tool_call / pricing / sources）
5. ModelPanes：ctx·max 徽章、模态徽章、「跟随 models.dev，编辑即钉死」HelpIcon、`source:"user"` 钉死语义（`max_output_source` 等）
6. 自定义模型添加接 lookup 就地升级；LlmSection 目录状态行 + 刷新按钮（文档占位转正）
7. 预设并集扩充（Google 兼容/百川/零一/阶跃/LM Studio）+ `docsUrl`/`local` 字段 + 「如何获取 Key ↗」
8. 价格展示（选择器/详情浮层，USD 原值如实）

**明确不做**：目录借池（记档）；远程自更新服务器；目录数据落 settings。

**落点**：`settings/llm.rs`（ProviderModel 新字段）、`llm/catalog*.rs`（新）、`commands/llm.rs`、`desktop/src/lib/model-catalog.ts`（新类型）、`lib/provider-presets.ts`、`ModelPanes.vue`、`ServiceChooser.vue`、`LlmSection.vue`、`tauri.conf.json`。

**走查要点**：deepseek/zhipu/openai 三家真 Key 拉列表 → 目录元数据命中（窗口/价格/模态徽章）；手改窗口 → 刷新目录不被覆盖（钉死）；自定义模型 id → 就地升级；断网 → 快照兜底。

---

### 批 B — 推理档位 + 输出钳制

**目标**：两个用户旋钮（推理档 + 输出上限）全链路，方言即数据。

**范围**：
1. `llm/dialect.rs`（新）：6 方言 × 档位 → body patch 纯函数 + 单测锁形状；映射不了不发
2. 预设 `dialect` 字段接线；目录 `reasoning_options` 归一化并入梯子
3. 会话档位持久化：`model_override.rs` +thinking_level + `session_set_thinking`（或扩展 session_set_model）
4. 解析链：`AppLlm::effective_cfg` 档位 clamp → `LlmRequest.params["thinking_level"]`（**内核零改动**）
5. wire.rs：params.thinking_level → dialect patch；max_output 钉死值 → `min(钉死, ctx − est − 1000)` 预检钳制；400 点名 max_tokens → 剥字段重试分支
6. 辅助降档：autoTitle / 记录员 / handoff 走梯子最低档（deepseek_auto 降不了如实记档）
7. 发送前能力门控：parts 模态 ⊆ input_modalities → `UnsupportedModality`（批 D 前先对文本请求恒过）
8. Composer：`ModelSelector.vue` 档位循环钮（tooltip 当前档；切模型 clamp；新选默认最高档）；模型菜单 ✨/👁 徽章
9. **DeepSeek 思考回传要求核实（供应商稿 §5.5，本批走查第一项）**：deepseek-flash 真轮带工具、不回传 reasoning_content，看是否 400（官方文档要求必须回传；2026-10-07 旧走查未见 400，证据矛盾）。若成立 → wire.rs 按 `Thinking`↔`AssistantMessage` 相邻配对回传 `reasoning_content`（历史字节稳定追加，M1 不破）；PI 占位回退方案记档
10. anthropic_effort 兼容层行为实机核实——不实则该预设梯子降级 none（如实）

**落点**：`llm/dialect.rs`（新）、`llm/wire.rs`、`app_llm.rs`、`model_override.rs`、`commands/sessions.rs`、`organize/client.rs`（记录员）、session 标题/handoff 调用点、`ModelSelector.vue`、`DefaultModelMenu.vue`。

**走查要点**：deepseek-flash 真轮带工具复测思考回传（§5.5，第一项）；deepseek-flash 档位 disabled/low/high/max 切换 → 抓包/日志盘证 `thinking.type` + `reasoning_effort` 双字段落盘（官方归一：medium→high）；GLM 真轮 disabled/enabled；clamp：切到无梯子模型 → 档位剥除不发；输出钳制：钉死 max_output 小值 → 抓包核 `max_tokens`；辅助调用抓包核低档（DeepSeek 现在思考可关，标题/记录员降档真实生效）。

---

### 批 C — 连接与安全

**范围**：
1. keyring：`keyring` crate 接入 + 惰性迁移（明文 → keyring，幂等）+ 读回落 + key_storage 标记；IPC 面不变（api_key 永不过线）
2. 自定义请求头：ProviderProfile +headers + 保留键校验 + client.rs 最后写入者 + 高级区编辑器
3. 端点扫描：list_models/probe 失败时 ≤4 候选（3s each）+ effective_base_url 回报表单
4. `llm_test_model`：最小请求五分类错误 + ModelPanes 行内「测试」
5. 网络代理：llm +network_proxy（system/direct/custom）+ 热换

**落点**：`settings/llm.rs`、`llm/client.rs`、`llm/models.rs`、`commands/llm.rs`、`Cargo.toml`（+keyring）、`ProviderSetupDialog.vue`、`ModelPanes.vue`、新设置行（代理）。

**走查要点**：明文 key 迁移 → Credential Manager 取证（cmdkey / PowerShell）+ 重启读得回；自定义头实机（中转站 User-Agent 门禁例）；错端点扫描自动补全；测试模型五分类各造一例；代理 custom 落 reqwest。

---

### 批 D — 多模态地基（最大批）

**范围**（多模态设计 v2.0 §12 阶段 1–6）：
1. `session-store/src/content.rs`（新）：ContentPart/AssetRef；`Message::User +parts`、`SessionEvent::UserMessage +parts`（serde default）；快照 schema_version 递增
2. 穿透面逐点过 + 回归：fork / rewind+undo 重写 / 树偏移投影 / derive_messages / ctx-compact（parts 不进摘要）/ 记忆转录折叠 `[图片]`
3. **v2.47 双气泡重放回归必须绿**（老 jsonl 逐字节不动）
4. AssetStore：`desktop/src-tauri/src/assets.rs`（新）+ `asset_put` 命令 + 上传校验（尺寸/体积/格式）+ GC（三扫描面 + 7 天）
5. wire.rs：User parts → content 数组（text 在前稳定前缀 + image_url data URL）；空 parts = 现网字节不变（M1 缓存零扰动）
6. estimate_tokens 扩展（图片 1024 / PDF 1000 平铺；1024 锚点 = DeepSeek 官方单图 token 上限）
7. 前端：AttachButton 扩展（粘贴/拖拽/选择 + 预览条 + 上传门控提示卡）+ MessageRow parts 缩略图渲染 + `agent://user-message` 投影带 parts
8. settings `multimodal` 区（maxDimension/maxSizeMb/formats/autoGc/gcDays）

**明确不做**：输出侧图片（记档）；detail 旋钮（P2）；音频（P2）；图片转码（记档）。

**落点**：`crates/session-store`（content.rs/session.rs/event.rs/snapshot）、`crates/kernel-agent`（estimate_tokens 及构造点 parts 透传）、`desktop/src-tauri/llm/wire.rs`、`assets.rs`（新）、`commands/`、前端 `AttachButton.vue`/`MessageRow.vue`/chat store/types.ts。

**走查要点**：老会话重放零变化；贴图真轮（**deepseek-flash 首选**——走查机现配 deepseek，官方已确认视觉走同一 chat completions；备选 qwen-vl / claude）→ 模型如实描述图片内容 + jsonl 落 parts + 前缀缓存命中率不塌（M1 面板）；不支持图片模型贴图 → 门控两道各触发一次；GC：删会话 → 7 天宽限内不删、人工触发过期删。

---

### 批 E — 多模态进阶

**范围**：图片查看器（左右切换/下载/Esc/缩放）；工具结果投影（wire 层挪下一条 user parts + 工具文本占位）；`screenshot` / `read_image` / `pdf_to_images` 工具（Pure）；降级链挂捆边界（P2 尾）；detail 旋钮评估（P2 尾，可裁）。

**走查要点**：screenshot 真轮 → 模型描述屏幕内容 + 下条 user 抓包证图片 part + 转录缩略图；树会话捆边界降级后命中率一次性回落再恢复（计划内失效语义）。

---

### 批 F — 生态

**范围**：
1. 子代理模型档：`available_for_subagents` 标志（批 A 字段随迁）+ 子代理钉主模型/档位（缺省 inherit 会话）+ 终端错误回退链（PI 案：窗口装不下已携上下文的候选跳过）——落点 kernel `subagent/` + desktop 组装（child_kernel scoped LlmService 先例）
2. 导入面板：claude-code / opencode / codex / cc-switch 四解析器（纯读）+ 预览确认 + 写入 ProviderProfile
3. 推荐三选：新服务取列表后自动预选 ≤3（族分散/toolCall 必备/超贵降权），替代现「全选 ≤8」——依赖批 A 目录价格数据

**走查要点**：子代理钉模型真轮盘证（子转录 jsonl 用的模型与父不同）；claude-code 本机配置导入往返；OpenRouter 拉列表 → 推荐三选命中。

---

### 候补批 G — 可靠性补强（《Reliability Reinforcement Design v1.1》§13 已做逐机制对照与调优）

**与 A–F 无硬冲突**，拆件按依赖挂靠：

| 件 | 内容 | 排期 |
|---|---|---|
| G1 Turn Commit length 闸 | wire.rs 已捕获 finish_reason 但 client.rs:322 一律当完整——length 截断带部分 tool_calls 会被执行。截断不收束、工具不执行、如实提示 | **随批 B**（maxOutput 钳制提高截断概率，闸先行）；committed 标记 serde default |
| G2 Unknown Effect | 复用现有 ToolCall/ToolResult 配对（**不建双轨事件**）+ 崩溃恢复扫描（有 Call 无 Result → uncertain）+ uncertain UI + 用户决定事件 | 独立小批（P1） |
| G3 Revision Guard | tool-fs 插件层：read 回 revision、写类工具带 expected_revision、StaleObservation 错误 | 可随时并行插批（P0） |
| G4 只读证明 | shell_read 白名单之上的解析器证明层；硬锁/围栏/unlockRisky 全保留 | tool-shell 批（P1） |
| G5 子 Agent 独立验收 | subagent_verify（Pure）+ claimed/verified 分离 + 验收调用吃辅助降档 | **随批 F** |
| G6 In-Band Summary | 实测 compact.rs:149 不复用前缀，但压缩低频且本就重建前缀——记档为 legacy 可选优化，不立批；批 B 顺带复核形状 | 记档（P2） |
| G7 crate 行数软上限 | cordis-core 3000 / kernel-agent 5000，tokei 脚本挂 check_length 旁 | 随手搭车（P3） |

---

## 三、两稿阶段表 → 批次映射

| 供应商稿 v2.0 阶段 | 批 | 多模态稿 v2.0 阶段 | 批 |
|---|---|---|---|
| 1 预置清单扩充 | A | 1 消息结构 | D |
| 2 添加供应商 UI | 已存在（差量随 A） | 2 wire 编码 | D |
| 3 拉模型列表 | 已存在（v2.58） | 3 AssetStore | D |
| 4 档案库（models.dev） | A | 4 输入链路 | D |
| 5 匹配算法 | A | 5 渲染 | D |
| 6 选择器能力展示 | A（徽章）+B（✨） | 6 估算 | D |
| 7 keyring | C | 7 查看器 | E |
| 8 档位自动适配 | B | 8 工具投影 | E |
| 9 上传能力检查 | D（吃 A 投影） | 9 降级链 | E |
| 10 视觉备用自动切换 | 不做（记档） | 10 音频 | E+ |
| 11 档案远程更新 | A（手动刷新，无服务器） | — | — |
| 12 自定义供应商 | 已存在（v2.58） | — | — |
| 13 价格展示 | A | — | — |

---

## 四、裁定记录（§四 即"等通知"的清单）

| # | 裁定点 | 推荐 | 状态 |
|---|---|---|---|
| D1 | 档案库源 = models.dev 快照 | 推荐 | **待裁** |
| D2 | 单线纪律维持（多模态输入侧走兼容层；原生 thinking budget / 图像输出 / 原生 PDF 另批） | 推荐 | **待裁** |
| D3 | thinking 控件 = 循环钮（vs PI 滑杆） | 循环钮 | **待裁** |
| D4 | 工具结果图片投影 = 挪下一条 user 消息（vs 仅文本占位） | 投影 | **待裁** |
| D5 | max_tokens 窄修订 = 用户钉死值才发送 + 预检钳制（v2.75 措辞修订） | 采纳 | **待裁** |
| D6 | 网络代理进批 C | 进 | **待裁** |
| D7 | keyring（Credential Manager 优先）+ 惰性迁移 | 确认 | **待裁** |
| — | 缓办记档：OAuth 订阅账号 / 目录借池 / 用户自编辑档位映射 / aux 发 max_tokens / 视觉备用自动切换 / 图像转码 | — | 已记 |

---

## 五、通用门禁与家法（每批适用）

- **文档先行**：动码前先改《UI shell and patterns》对应节 + LLM.md；本阶段计划为唯一进度表，每批完成先勾它（主 ROADMAP 照常同步）
- **schema 纪律**：全部新增字段 serde 字段级缺省零迁移；旧文件/旧 jsonl/旧快照读得进是回归测试的硬断言
- **行数纪律**：400 行建议值三问自检；catalog_match / dialect / assets 预留拆分
- **门禁一次跑**：fmt / clippy --all-targets / 测试在**全部文件编辑收尾后**一次跑（v2.125 教训）；vue-tsc + pnpm build + lint:style
- **实机走查**：CDP + 独立数据根（LOCOWRK_DATA_DIR），走查数据用后即清；涉及真 Key 的批（A/B/C）用用户现配的供应商，改动的设置逐字节还原
- **宁缺勿假**：目录没有的能力如实未知；映射不了的档位不发；降不了的场景如实降不了

---

*文档版本：1.0（2026-10-07）*
*配套项目：Locowrk*
*关联：《模型供应商配置与能力探测设计 v2.0》《多模态支持设计 v2.0》《LLM 客户端与调度层设计》*
