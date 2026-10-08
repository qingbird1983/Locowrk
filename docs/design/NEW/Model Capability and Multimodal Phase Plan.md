# Locowrk 模型能力与多模态阶段计划

> 配套项目：Locowrk
> 版本：1.0（2026-10-07）
> 上游设计：《模型供应商配置与能力探测设计 v2.0》《多模态支持设计 v2.0》
> 参照调查：PI-Desktop（F:\Warehouse\Github\PI-Desktop）、ZCode（F:\Warehouse\Github\ZCode）模型配置全量调查（2026-10-07）
> 状态：**六批全部完成（2026-10-08，v2.127–v2.132）**——用户 2026-10-08 指令「根据阶段计划开始执行」，裁定表 D1–D7 按推荐落档（各批动码时随批生效）；候补批 G：**G1 随批 B、G5 随批 F、G3 独立小批（v2.133）、G2 独立小批（v2.134）已落地**，G4 留作独立小批（tool-shell 批），G6/G7 记档

---

## 一、总览

六个批次，A→F 顺序执行；A/B 是 C/D/E/F 的数据与语义地基。每批独立交付、独立可回退，家法门禁照旧。

| 批 | 名称 | 一句话 | 规模感 | 依赖 |
|---|---|---|---|---|
| **A** ✅（v2.127，2026-10-08） | 模型能力目录 | models.dev 快照接入 + 打分匹配 + source 钉死 + 能力投影到 UI | 大（新模块 + 快照资源） | 无 |
| **B** ✅（v2.128，2026-10-08） | 推理档位 + 输出钳制 | 方言数据表 + 会话档位持久化 + composer 档位钮 + maxOutput 预检钳制 + 辅助降档 + 发送门控 | 大 | A（梯子/模态数据） |
| **C** ✅（v2.129，2026-10-08） | 连接与安全 | keyring 迁移 + 自定义请求头 + 端点扫描 + 逐模型测试 + 网络代理 | 中 | 无（可插在 A/B 间） |
| **D** ✅（v2.130，2026-10-08） | 多模态地基 | ContentPart + wire 编码 + AssetStore + 上传链路 + 渲染 + 估算 | **最大**（内核穿透） | A/B（模态数据 + 钳制公式） |
| **E** ✅（v2.131，2026-10-08） | 多模态进阶 | 查看器 + 工具结果投影 + 多模态工具 + 降级链 | 中 | D |
| **F** ✅（v2.132，2026-10-08） | 生态 | 子代理模型档 + 回退链 + 从其他工具导入 + 推荐三选 | 中 | A/B |
| **G**（候补） | 可靠性补强 | Turn Commit length 闸（随 B）+ Unknown Effect 恢复 + Revision Guard（✅ v2.133）+ 只读证明 + 子代理验收（随 F） | 各小-中 | 无硬依赖；G1 随 B、G5 随 F、G3 独立小批 |

> C 与 A/B 无依赖，若 keyring/代理急可先插；其余严格按序。

---

## 二、批次卡片

### 批 A — 模型能力目录 ✅（v2.127，2026-10-08 完成）

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

**落成记录（2026-10-08，v2.127）**：快照 5.34MB 随包（226 供应商 / ~5300 模型，2026-10-08 抓取）+ tauri `bundle.resources`；`llm/catalog.rs`（CatalogStore 惰性加载/在线刷新只进内存/裁剪投影/梯子归一化——toggle+effort 并存时 enabled 冗余不出现）+ `llm/catalog_match.rs`（预设 host 别名表 +100 / 快照 api host +80 / 归一化 id 两遍扫先原样后归一；**借池不做**，无定位即 None）；命令三件 + `settings/llm.rs::synthesize_view` 合成投影（sources 逐字段 user/catalog/None；ProviderModel 新增 max_output/max_output_source/supports_image 钉死面，目录数据永不落 settings——前端 save 剥 sources=catalog 派生值）；ModelPanes 高级区（最大输出钉死/图像输入三态/梯子只读 chips/价格行 +「编辑即钉死」info 悬浮）+ 右栏行视觉徽章（eye）+ ctx·max 徽章；自定义模型添加即查目录就地升级 + sources=null 行自动合并（未命中落定全未知防回环）；预设 22→27 家（Google/百川/零一/阶跃/LM Studio）+ docsUrl/local + 「获取 API Key ↗」（`open_external_url` 只放行 https）；价格展示三处（composer 选择器/默认项浮层/编辑模态详情行，USD 原值如实）。**实机走查（2026-10-08，CDP 独立数据根，目录数据离线可验）18/18 PASS**——状态行/在线刷新（30s）/徽章/梯子/价格/悬停钉死提示/32K 钉死落盘/重开 user+catalog 并存/自定义模型就地升级（v4-pro 真值 $0.66/$1.98 + 纯文本如实）。**实抓修复 ×2 + 调优 ×1**：刷新超时 10s→30s（5.3MB 载荷代理链路 10.2s 撞死）；addCustom 静态预填标 user 挡住目录合并（预填只写值不标来源）。**留待真 Key**：三家拉列表 + 断网快照兜底（目录面离线已全验）。门禁：clippy 0 / fmt / 测试绿（desktop 258 = +19）/ vue-tsc 0 / build / lint:style。

---

### 批 B — 推理档位 + 输出钳制 ✅（v2.128，2026-10-08 完成）

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

**落成记录（2026-10-08，v2.128）**：dialect.rs 6 方言纯函数 + 别名表三元组；会话旁车 thinking_level + session_set_thinking；AppLlm::prepare 解析链（clamp + aux 降档，内核零改动）；wire 预检钳制 + 400 剥字段重试 + G1 length 闸（候补 G1 随批兑现）；composer 循环钮 + 菜单 ✨/👁 徽章。**§5.5 复测结论（走查第一项）**：deepseek-flash 真轮 8 次工具调用不回传 reasoning_content **零 400**——回传义务未被供应商执行，维持现状不实现回传（记档复核结论）。**实机走查**：循环钮真点 默认→disabled→low→high；按轮归属 thinking 4+2→**0**→1→1（disabled 真关/high 真开）；autoTitle 辅助降档两标题生成；G1 两轮实锤（钉死 1024 长输出 → UI 截断文案 + outcome=failed + 不入史）；旁车档位落盘。留待：GLM 真轮 disabled/enabled、anthropic_effort 实机核实（无 Key）。

---

### 批 C — 连接与安全 ✅（v2.129，2026-10-08 完成；D6/D7 按推荐落档）

**落成记录**：①keyring（`keyring` crate 三平台 feature，service="Locowrk" + account=provider_id）——`ProviderProfile +key_storage`（None=文件零迁移）+ 双读回落（明文优先→凭据库）+ settings open **惰性迁移**（有变更立即落盘，安全改进不等下一次保存）+ 写失败回落文件 + IPC 面 `api_key_set` 不变（`has_key()` 两态点亮）；**测试构建不碰真实凭据库**（cfg(test) 按"不可用"走回落路径，防单测夹具写进开发机 Credential Manager）。②自定义请求头 `ProviderProfile +headers`（BTreeMap）：`validate_headers`（≤32 条/键 token 形/保留键 14 个拒收/值无控制字符）+ client `build_headers` HeaderMap（鉴权先入、用户头**最后写入者覆盖**，anthropic 双带现状不变）+ 探测同吃。③端点扫描 `sweep_candidates` 纯函数（现值剔除 + ≤3 候选 3s each；裸域名补 scheme——**本地形态 localhost/127.*/::1/无点补 http://，其余 https://**，走查实抓首版全 https 打不通本地 echo 服务器后改）+ `llm_probe_models` 返回 `ProbeOutcome{models,error,effectiveBaseUrl}` + 表单字段未再编辑时自动采纳 +「已自动补全端点」提示；**选择器路径不扫描**（无处采纳=扫了谎报，记档偏离设计稿）。④`llm_test_model`（最小 ping 15s）五分类 + unknown 带报文 + ModelPanes 行头「测试」钮 + 结果徽章常驻。⑤`llm.networkProxy{mode,url}`（system/direct/custom）+ `apply_proxy` 共享件（聊天客户端/探测/扫描/测试四处同吃）+ custom 代理地址草稿档门控（custom 必带 url，切档不落盘等地址提交）。UI：编辑模态折叠头编辑器（行 + 常用头 chips + JSON 导入/复制）+ 设置模型分区「网络代理」组 + LLM.md v3.3 + UI shell v4.23。**实机走查（CDP 独立数据根 + 复制真 Key + 本地 echo/scenario 服务器 18900）**：明文 Key 启动即迁移（settings 只剩标记 + Credential Manager `p-walk-ds.Locowrk`/`p-walk-echo.Locowrk` 取证 + 重启真轮读回成功）；种子头注入 + UI 加行保存双头随请求；裸域名扫描自动补全 http://…/v1 + 全程唯一 host；五分类 + network 七徽章逐例命中；custom 死代理落盘 → 真轮**无限重试不收束**（v2.75 裁定如实形态 + cfg 轮次绑定）→ 停止空收束退路 → system 恢复真轮完成。**走查教训记档**：①两驱动脚本轮次排队交错（前轮未收束后轮入队，代理翻转毒化排队轮 cfg——断言"错误事件"错，无限重试本就无错误事件）；②`llm_test_model` 走**已存**档位（表单未保存值不生效）。数据根已清 + Credential Manager 两条已删净（cmdkey /list 零残留）+ echo 服务器已停，脚本/截图留 tmp/v129-walk/。门禁（走查修复后复跑）：clippy 0、fmt、workspace 39 套件（desktop 283）、vue-tsc 0、pnpm build、lint:style 0。


**范围**：
1. keyring：`keyring` crate 接入 + 惰性迁移（明文 → keyring，幂等）+ 读回落 + key_storage 标记；IPC 面不变（api_key 永不过线）
2. 自定义请求头：ProviderProfile +headers + 保留键校验 + client.rs 最后写入者 + 高级区编辑器
3. 端点扫描：list_models/probe 失败时 ≤4 候选（3s each）+ effective_base_url 回报表单
4. `llm_test_model`：最小请求五分类错误 + ModelPanes 行内「测试」
5. 网络代理：llm +network_proxy（system/direct/custom）+ 热换

**落点**：`settings/llm.rs`、`llm/client.rs`、`llm/models.rs`、`commands/llm.rs`、`Cargo.toml`（+keyring）、`ProviderSetupDialog.vue`、`ModelPanes.vue`、新设置行（代理）。

**走查要点**：明文 key 迁移 → Credential Manager 取证（cmdkey / PowerShell）+ 重启读得回；自定义头实机（中转站 User-Agent 门禁例）；错端点扫描自动补全；测试模型五分类各造一例；代理 custom 落 reqwest。

---

### 批 D — 多模态地基（最大批）✅（v2.130，2026-10-08 完成；D2 随批落档）

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

**落成记录（2026-10-08，v2.130）**：①消息结构——session-store 新 `content.rs`：`ContentPart` tagged serde 枚举 `text|image|file` + `AssetRef{id,mime,name}`（无 ToolUse/ToolResult parts、无 detail 旋钮、无音频、无 URL 内联——全部记档不做）；`Message::User +parts` + `UserMessage +parts`（serde 字段级缺省 + `skip_serializing_if="Vec::is_empty"` = 纯文本消息磁盘字节逐位不变，v2.47 双气泡回归绿）；快照 schema_version 1→2 + `V1ToV2Upcaster`（serde default 填充缺 parts）——**实抓：snapshot.rs 缺 `#[cfg(test)] mod tests;` 声明，存量快照测试从未跑过，补声明后 upcaster 未注册随即被抓**。②穿透面——fork/rewind+undo 重写/树偏移投影/derive_messages/ctx-compact（parts 不进压缩摘要；`est_tokens` Image 1024/File 1000 进钳制估算，1024 锚点 = DeepSeek 官方单图 token 上限）/转录折叠 `[图片: name]`/export/extract/title 全部带 parts 或如实折叠。③AssetStore（`assets/` 模块三文件 store/validate/gc）——内容寻址 `<数据根>/assets/{images,files}/<sha256>.<ext>`（同文件天然去重）+ data-URL LRU(16) + magic 嗅探（PNG/JPEG/GIF87a·89a/RIFF-WEBP/%PDF-）+ **超尺寸拒收不转码**（转码记档不做）+ GC 三扫描面（sessions/*.jsonl + *.snapshot + sub-transcripts/*.jsonl，sha 十六进制子串粗匹配）+ 宽限期 `gcThresholdDays` 缺省 7（0=即净）+ 启动 + 24h 循环——**实抓：setup 上下文无 tokio runtime，`tokio::spawn` 启动即 panic，改 `tauri::async_runtime::spawn`**。④wire 编码——空 parts = 纯字符串 content（**M1 前缀缓存零扰动**，走查第二轮 cache_read>0 盘证）；非空 = content 数组 `{"type":"text"}` 先行稳定前缀 + `image_url` data URL / `file{filename,file_data}` 按部分顺序；`check_modalities` 后端发送门（目录未知 = None 放行宁缺勿假）。⑤双道门控——前端上传时（supportsImage 三态 + 目录 modalities，门卡带「仅发送文字/打开模型选择器」）+ 后端发送时各一道。⑥settings `multimodal` 区（maxDimension 2048/maxSizeMb 5/formats/autoGc/gcThresholdDays 7）字段级缺省零迁移。⑦前端——composer 预览条（56px 缩略图 + ✕ + 粘贴/拖拽/AttachButton 路由）+ MessageRow 96px 缩略图与文件 chip（点击 reveal）+ 重放恢复——**实抓产品缺陷：发送失败清空附件条（用户无法原样重试），改成功才清**。**实机走查（CDP 独立数据根 + keyring 标记真 Key 零明文 + deepseek-flash 真轮）**：walk1 13/13 PASS——老格式会话重放零变化/双气泡/零 parts 行；贴图真轮模型如实描述红底绿圆（E8232A/00CC44、圆占宽 60–65%）；jsonl user_message 落 parts 且 asset id 与盘上一致；第二轮 cache_read=3840>0 前缀缓存未塌；重放缩略图恢复。walk2——前端门卡（v4-pro 纯文本目录模态）+「仅发送文字」剥条；后端直发被拒（「当前模型不支持图片输入」如实报错）。GC 配对验证——days=7 `scanned=2 removed=0` 双存 / days=0 `removed=1` 孤儿删、被引用存（sessions jsonl 引用扫描实证）；**W6a 驱动伪失配记档**（宽限重启删孤儿断言受控复测无法复现，系驱动时序伪迹，GC 双向行为人工盘证）。数据根已清、vite 已停，脚本/截图留 tmp/v130-walk/。门禁：clippy 0、fmt、workspace 39 套件（desktop 292 = +9：assets 3/validate 3/gc 3）、vue-tsc 0、pnpm build 过、lint:style 0；check_length 新增超限 2 文件三问自检不拆（MessageRow 475 媒体条同职责面 / settings tests tools 403 测试道具），Composer 既超 530→782 同职责面扩张不拆。留待批 E：查看器/工具结果投影/多模态工具/降级链。

---

### 批 E — 多模态进阶 ✅（v2.131，2026-10-08 完成；D4 按推荐落档 = 投影）

**落成记录（2026-10-08，v2.131）**：**①工具结果投影（D4 兑现，wire.rs `project_tool_assets` 纯函数，`AppLlm::prepare` 调用点）**——tool output 的 `asset`/`assets`（mime `image/*`）挪出 tool 消息（tool 只吃文本）：尾部合成一条只带图片的 user 消息（`[{text:[工具产生的图片]},{image_url}]`）；**设计稿偏离记档**——「无下一条 user 保持占位」会让模型永远看不到图，主流程不可用，改合成消息（ZCode 媒体投影同思路）。三条纪律：tool 消息改写位置无关（`image_note`「[图片已附于本工具结果之后: name]」跨请求字节稳定，M1 不破）；历史轮图片**不重发**（每请求重发 = token 永久膨胀）；模型不支持（三态 `Some(false)`）降级不报错——引用号留原地 +「图片未随请求发送」note。调用点在 prepare = maxOutput 预检钳制估算吃投影后形状（设计稿记档的估算缺口在调用点闭合；ContextUsage 前端口径不含合成图记档）。**②多模态工具三件**（宿主内置，Pure）：`screenshot`（Always，`screenshots` crate，按 `multimodal.maxDimension` **缩边落库**——**实抓 #1**：真屏 3440×1440 撞死 2048 单边上限工具必死；缩边 = 机器产物采集参数，非用户文件转码，read_image 拒收语义不变）；`read_image`（OnDemand，`fsutil::allowed_roots` 围栏 + 先验后写——非图片在落盘前拒收）；`pdf_to_images`（OnDemand，**pdfium.dll 7.5MB 随包 `resources/pdfium/`**（bblanchon pdfium-binaries chromium/8086），库缺席不进目录宁缺勿假；页数缺省 8 硬顶 20 = 20K token 量级成本闸；目标宽按 cap×纵横比 floor+1px 余量——**实抓 #2**：Letter 页 1583×2049 超 1px 被 put 拒；**实抓 #3**：pdfium 全局绑定二次初始化即死（`PdfiumLibraryBindingsAlreadyInitialized`），进程级 OnceLock 单例）。`asset_save_as` 命令（原生另存对话框 + assets/ 内复制）。**③图片查看器**（`ImageViewer.vue` + `viewer.ts` store，App 根挂载）：teleport 全屏浮层——左右切换（列表 = `chat.viewerImages` 转录序 = 用户 parts + 工具产物）、缩放（滚轮/＋−/双击/0 复位）、拖移、Esc（滚动锁随开合）、另存为、在文件夹中显示；MessageRow 图片缩略点击改开查看器（reveal 保留给 PDF chip 与查看器内按钮）。**④工具行产物预览**（§8.3 三行形态）：`output.asset/assets` 图片缩略 + 查看大图/另存为两钮（pending 无产物不渲染）；动作名三键（截屏/读取图片/转换 PDF 为图片）+ camera/download/chevronLeft 三图标。**⑤降级链挂捆边界（P2 尾，最小形态落地）**：memory-tree 材料 UserMessage 折 `[图片: name]` 占位——捆边界后图片离开请求，记录员摘要还能提到它（§10.2「捆与捆之间全部降级为文本」最小实现）；「视觉模型生成描述替代原图」记档 P2 缓办（整理期视觉辅助调用 = 新辅助调用面，待裁定）。**detail 旋钮评估 = 裁掉**（单线兼容层 detail 支持不一，宁缺勿假；P2 记档维持）。**实机走查（echo 18900 抓包 + deepseek-flash 真轮 10 轮，隔离数据根 + 复制真实档案零明文/Credential Manager 共享）**：echo 投影 10/10——支持 = tool 消息剥 asset + note + 尾部合成 user 带 image_url data URL 抓包实锤；不支持 = asset 留原地 + 未发送 note + 无合成 user + 零错误事件。真轮：截屏描述屏幕内容如实（Win11 深色壁纸/任务栏，投影端到端）+ 工具行三行形态渲染；read_image 描述红绿图（红底绿圆如批 D）；pdf_to_images 双页渲染逐页描述（PAGE ONE/TWO MARKER 逐字读出）；查看器打开/计数/左右切换（按钮+键盘）/放大 120%/Esc 关闭+滚动解锁；捆边界：tree 会话贴图轮 → /整理 → L1 摘要含图片占位（降级不归零）+ 命中率 4864→0→3712 一次性回落再恢复（计划内失效语义）。**实抓修复 ×4**（①②③上述 + **④投影 assets 数组翻面**——逆序遍历的收尾 reverse 把单结果内部页序也翻了，模型如实读出「第 1 张图 = PAGE TWO」，改「结果间与结果内都先翻、末尾一次还原」+ 保序回归）。**走查环境教训记档**：echo SSE 服务器必须 HTTP/1.1（BaseHTTPRequestHandler 缺省 1.0 + 手写 chunked 帧 = reqwest 按裸流读、SSE 解析失败无限重试——v2.129 echo 只回非流式 JSON 故未暴露）。清理：数据根已删（无 Key 落盘）、echo/vite/应用进程已停；脚本/截图留 tmp/v131-walk/。**门禁**：clippy 0、fmt、根 workspace 418 绿 + desktop 301（292→+9：投影 4/media 5）、vue-tsc 0、pnpm build 过、lint:style 0；check_length 新增超 2 文件三问不拆（wire.rs 515 单线协议+投影前置纯函数 / llm/tests.rs 784 测试道具跨组共用）。**已知边界（记档）**：①ContextUsage 前端口径不含合成图片消息（≈1024/图低估，钳制侧已闭合）；②note 文本随会话换模型（支持↔不支持）一次字节漂移 = 显式变更接受；③pdfium 只随 Windows dll（跨平台资源名按平台常量就绪、打包名单记档）；④screenshot 缩边不可关（随 maxDimension 走，0 = 原尺寸）。

---

**范围（原卡片）**：图片查看器（左右切换/下载/Esc/缩放）；工具结果投影（wire 层挪下一条 user parts + 工具文本占位）；`screenshot` / `read_image` / `pdf_to_images` 工具（Pure）；降级链挂捆边界（P2 尾）；detail 旋钮评估（P2 尾，可裁）。

**走查要点**：screenshot 真轮 → 模型描述屏幕内容 + 下条 user 抓包证图片 part + 转录缩略图；树会话捆边界降级后命中率一次性回落再恢复（计划内失效语义）。

---

### 批 F — 生态

**范围**：
1. 子代理模型档：`available_for_subagents` 标志（批 A 字段随迁）+ 子代理钉主模型/档位（缺省 inherit 会话）+ 终端错误回退链（PI 案：窗口装不下已携上下文的候选跳过）——落点 kernel `subagent/` + desktop 组装（child_kernel scoped LlmService 先例）
2. 导入面板：claude-code / opencode / codex / cc-switch 四解析器（纯读）+ 预览确认 + 写入 ProviderProfile
3. 推荐三选：新服务取列表后自动预选 ≤3（族分散/toolCall 必备/超贵降权），替代现「全选 ≤8」——依赖批 A 目录价格数据

**走查要点**：子代理钉模型真轮盘证（子转录 jsonl 用的模型与父不同）；claude-code 本机配置导入往返；OpenRouter 拉列表 → 推荐三选命中。

**落成记录（2026-10-08，v2.132）**：**①子代理模型档 + 回退链**——`available_for_subagents` 标记 + `LlmConfig.subagent` 钉档（缺省跟随会话）；路由 = `params["route"]` sanctioned 通道（内核零改动）；`SubagentLlm` 回退链（钉档打头 + 标记池；凡上抛 Err = Fatal 推进；窗口装不下记因跳过；死供应商不回退 = v2.75 纪律偏离 PI 记档）；内核 `ChildModelFactory` 契约 + child_kernel 作用域换入；outcome 增 model/model_failures；FilterToolSource 剔 `subagent_` 前缀。**②导入面板**——四解析器纯读（claude-code `[1M]` 尾缀剥除 / codex wire_api note / opencode auth.json 回落 / cc-switch SQLite 容器分派），密钥不进 IPC（has_key 布尔 + apply 后端重扫取 Key）。**③推荐三选**——recommend 纯函数（toolCall 门/价格升序/族分散 ≤3）+ ProbeOutcome.recommended；**实抓：自动预选自 v2.58 起因 touched 误置从未生效，修复**。**④G5 subagent_verify**——Pure OnDemand 工具（转录紧凑化 + aux 降档 LLM 逐准则裁决）；claimed/verified 分离走工具结果通道（§7.6 新事件偏离记档）。**走查**：推荐三选 IPC+UI / 导入四源 + cc-switch 落档（缺省全选 × 脚本点击 = 14 条导入 + CM 迁移，清理记档）/ 钉模型真轮（outcome.model=v4-pro 父在 flash）/ 回退真轮（伪模型 400 → 推进）/ verify 真轮（如实判 failed 带证据）。门禁：clippy 0 / fmt / workspace 39 + desktop 321 / vue-tsc 0 / build / lint:style。已知边界见 CHANGELOG v2.132。


---

### 候补批 G — 可靠性补强（《Reliability Reinforcement Design v1.1》§13 已做逐机制对照与调优）

**与 A–F 无硬冲突**，拆件按依赖挂靠：

| 件 | 内容 | 排期 |
|---|---|---|
| G1 Turn Commit length 闸 | wire.rs 已捕获 finish_reason 但 client.rs:322 一律当完整——length 截断带部分 tool_calls 会被执行。截断不收束、工具不执行、如实提示 | **随批 B**（maxOutput 钳制提高截断概率，闸先行）；committed 标记 serde default |
| G2 Unknown Effect | 复用现有 ToolCall/ToolResult 配对（**不建双轨事件**）+ 崩溃恢复扫描（有 Call 无 Result → uncertain）+ uncertain UI + 用户决定事件 | ✅ **已落地**（独立小批 v2.134，2026-10-09） |
| G3 Revision Guard | tool-fs 插件层：read 回 revision、写类工具带 expected_revision、StaleObservation 错误 | ✅ **已落地**（独立小批 v2.133，2026-10-09——P0 最先做） |
| G4 只读证明 | shell_read 白名单之上的解析器证明层；硬锁/围栏/unlockRisky 全保留 | tool-shell 批（P1） |
| G5 子 Agent 独立验收 | subagent_verify（Pure）+ claimed/verified 分离 + 验收调用吃辅助降档 | ✅ **已随批 F 落地**（v2.132） |
| G6 In-Band Summary | 实测 compact.rs:149 不复用前缀，但压缩低频且本就重建前缀——记档为 legacy 可选优化，不立批；批 B 顺带复核形状 | 记档（P2） |
| G7 crate 行数软上限 | cordis-core 3000 / kernel-agent 5000，tokei 脚本挂 check_length 旁 | 随手搭车（P3） |

**G2 落成记录（2026-10-09，v2.134）**：三件全落，不新增双轨事件。①**扫描** = `session_store::scan_uncertain`（单遍：assistant 声明位 − ToolResult 事件 = 悬空，**两形态都认**——ToolCall 事件已落 / 死在两次落盘间只剩声明位；含裁决表与 unresolved 按声明序）；**derive_messages 悬空诚实闭合**（设计稿未言及的落地补充：不给合成则下一轮请求带悬空 tool_calls 被供应商整单拒收——未裁决合成 `status:"unknown"` 效果未知、有裁决按裁决闭合；合成在投影层，日志不回填 ToolResult）。②**uncertain UI** = 组外决策卡（折进过程组会被默认折叠藏住，决策面必须可见）+ 工具行 warn「结果未知」态；三裁决 = 视为已完成 / 视为未执行 / 检查后记录（带检查记录输入）——设计稿四选的 **Rerun 裁掉记档**（「未执行」裁决后模型下一轮自然重做，走全部既有审批/守卫门，不建旁路重执行机构）。③**用户决定事件** = `ToolExecutionUncertain {decision, note?, ts?}`（serde default，只写一次重放/重启不再询问）；桌面 `session_resolve_uncertain` 命令（忙锁 try_lock 同编辑重发；inspected 必带非空记录）实时回推 UncertainResolved + 合成 ToolResult，**与重放投影同形**（走查实抓：重放缺合成闭合 → 已裁决行恒 pending 的真缺陷，修投影后重放=实时同序列「建卡→裁决收束→行闭合」）；启动 restore_all 对未裁决悬空调用落观测日志。走查（CDP 隔离数据根 + CM 共享零明文 + deepseek 真轮 4 轮）：真 kill + 构造 mid-flush 悬空尾三段实锤——W1b 重放卡/行态 + **未裁决状态发轮模型如实引用 `status:"unknown"` 报文拒绝猜测**（原话「我不知道删没删……绝不凭猜测宣称已完成」）+ 检查后记录裁决落 jsonl（decision=inspected 带 note）卡收为记录行；W2 再重启重放不重复询问 + 裁决后请求形状真轮完成（hello.txt 落盘）。**已知边界（记档）**：桌面落盘粒度 = 每 step（run_turn 在 step_as 返回后一次 append_slice），悬空只在 **mid-flush 崩溃**出现（毫秒级窗口，走查以构造代表真实形态）；per-event 落盘 = 潜在后续批（内核需持久化钩子，本批不做）。门禁：fmt / clippy 0 / 根 workspace 431（39 套件）/ desktop 322（+1）/ vue-tsc 0 / build / lint:style / check_length 零新增超限。详见 CHANGELOG v2.134。

**G3 落成记录（2026-10-09，v2.133）**：共享基座 `plugin_api::fsutil`（content_revision = sha256 over **LF 归一全量字节**，\r\n 折叠与 edit_replace 容错同口径；check_revision 报文带双方 revision + 指路 fs_read——§3.3 兑现；plugin-api 增 sha2 与 desktop 同锁）+ `file_revision`/`arg_expected_revision`。五写类工具：fs_read 回全量 revision（与截断窗口解耦）；fs_write expected_revision **缺省/null = 新建语义**（§3.4，已存在即拒盲覆盖）；fs_delete 同守卫（§3.5 表）；edit_replace/lines/diff 必传（比对先于任何备份/写盘）；**成功输出带新 revision 供链式写**（多步编辑不重读，守卫不断链）；fs_restore 输出带恢复后 revision；undo 契约与审批面零改动。插件 tool-fs 0.1.5 / tool-edit 0.2.1（小迭代 +0.0.1）。§13 对齐注记：read_image/pdf_to_images asset 引用不带 revision（无 edit 消费方，评估裁掉记档）。走查（deepseek 真轮 3 轮）：§10.2 打地鼠全链实锤（读→外部注入→edit stale 拒→重读→成功且**注入行幸存**；模型收束报告逐字写明「工具因观测过期拒绝写入……这是安全机制要求的」）+ 链式 revision 逐字节相同零失败 + 删除守卫（陈旧 fs_delete → 拒 → 重读 → 删成功；复跑轮模型自觉先重读并主动报告外部改动异常）。实抓修复 ×2：tool-fs 测试锁分裂（read/write 各持私有 ENV_LOCK 互斥失效 → 上收 crate::testutil 共锁）、lines.rs 守卫绑定遮蔽 args content（clippy 抓到防住真 bug）；搭车 desktop catalog 测试同 pid 目录并行互删分家。已知边界：守卫核对与落盘间 TOCTOU 微窗（缩窗防线非事务，设计接受）。门禁：fmt / clippy 0 / 根 workspace 全绿（plugin-host FFI 端到端含盲覆盖与陈旧拒绝断言）/ desktop 321 / vue-tsc 0 / build / lint:style。详见 CHANGELOG v2.133。

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
| D1 | 档案库源 = models.dev 快照 | 推荐 | **已采纳**（用户指令开始执行，按推荐落档 2026-10-08；批 A 兑现） |
| D2 | 单线纪律维持（多模态输入侧走兼容层；原生 thinking budget / 图像输出 / 原生 PDF 另批） | 推荐 | **已采纳**（批 D 兑现，2026-10-08——content 数组走 OpenAI 兼容层，anthropic/ollama 亦走兼容端点） |
| D3 | thinking 控件 = 循环钮（vs PI 滑杆） | 循环钮 | **已采纳**（批 B 兑现，2026-10-08） |
| D4 | 工具结果图片投影 = 挪下一条 user 消息（vs 仅文本占位） | 投影 | **已采纳**（批 E 兑现，2026-10-08——wire 层投影 + 尾部合成 user 消息；设计稿「无下一条 user 保持占位」偏离记档：占位 = 模型永远看不到图，主流程不可用） |
| D5 | max_tokens 窄修订 = 用户钉死值才发送 + 预检钳制（v2.75 措辞修订） | 采纳 | **已采纳**（批 B 兑现，2026-10-08） |
| D6 | 网络代理进批 C | 进 | **已采纳**（批 C 兑现，2026-10-08） |
| D7 | keyring（Credential Manager 优先）+ 惰性迁移 | 确认 | **已采纳**（批 C 兑现，2026-10-08） |
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
