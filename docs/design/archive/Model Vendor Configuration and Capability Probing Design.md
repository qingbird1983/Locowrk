# Locowrk 模型供应商配置与能力探测设计

> 配套项目：Locowrk
> 版本：**2.0**（v1.0 用户初稿 + PI/ZCode 双家取参修订；修订摘要见 §0）
> 前置文档：《LLM 客户端与调度层设计》（LLM.md v3.0）、《UI shell and patterns》§3.30
> 目标：让用户"选供应商 → 填 Key → 选模型"，其余全部自动
> 参照系：PI-Desktop（models.dev 目录 / 打分匹配 / source 钉死 / 端点扫描）、ZCode（方言即数据 / 档位双旋钮 / maxOutput 预检钳制 / 辅助降档）

---

## 0. v2.0 修订摘要（相对 v1.0 初稿）

| # | v1.0 初稿 | v2.0 裁定 | 理由 |
|---|---|---|---|
| 1 | 档案库手维护 20 个模型 + 「Locowrk 官方仓库」远程更新 | **models.dev 快照随包**（惰性加载 + 手动刷新只进内存） | 免手维护；models.dev 覆盖本清单全部供应商并带价格/窗口/模态/推理档；Locowrk 无服务器，远程自更新无落点。PI 已验证此路线 |
| 2 | `~/.locowrk/config/providers.toml` 新存储 | **退役**。扩展现有 settings.json `llm` 区（`settings/llm.rs`） | v2.58 已有 LlmConfig/ProviderProfile/ProviderModel 全套 schema + 命令 + UI；另起炉灶 = 两份真相。能力字段走「目录投影 + source 钉死」，用户手改才落 settings |
| 3 | `ApiCompat::{OpenAiCompatible, Anthropic, Gemini, Ollama}` 四套线编码器 | **退役**。维持 v2.75 单线协议：kind 只决定鉴权头 + 端点归一；Google 走 `…/v1beta/openai/` 兼容端点（kind=openai） | 多模态输入侧在官方兼容层上单线可通（见多模态设计 v2.0 §5）；真需要多线的是原生 thinking budget / 图像输出 / 原生 PDF，另立批次裁 |
| 4 | 推理档位只有 `reasoning_effort` 值列表 | 补齐**方言映射**：每预设 `dialect` 字段（数据选表），Rust 侧少量静态线编码器 | ZCode 实证：档位→各家请求字段必须是数据；没有映射的档位列表是半成品 |
| 5 | `max_output` 只存不用 | **用户钉死值才随请求发送 + 每步预检钳制** `min(钉死值, ctx − 估用 − 1000)`；目录值只展示不自动发 | ZCode/PI 双方都发输出上限；v2.75 反对的是"派生限额"，用户显式值 + 钳制不冲突。**此为 v2.75 裁定的窄修订，见 §10 裁定表 D5** |
| 6 | Google base_url = `…/v1beta`（原生端点） | 改 `https://generativelanguage.googleapis.com/v1beta/openai`（兼容端点） | 原生端点会引入第二条线，与裁定 3 同源 |
| 7 | 预置清单 20 家（§二） | **与现有 22 预设取并集**（保留火山/小米/Together/Fireworks/NVIDIA/Baseten/Cerebras/HF/Z.AI），新增 Google 兼容/百川/零一/阶跃/LM Studio | v1.0 清单把 v2.58 已有的预设丢了 |
| 8 | API Key 明文存配置 | **keyring**（Windows Credential Manager 优先）+ 存量明文一次性迁移 + 双读过渡 | v1.0 §7.2 方向保留，补迁移与回落细节（§8.4） |
| 9 | — | 新增：会话级 thinking 档位持久化、辅助调用自动降档、发送前能力门控、端点扫描探测、自定义请求头、逐模型连通测试、目录状态行 | 双家已验证、本稿缺口（见 §5–§8） |
| 10 | （v1.0 时点 DeepSeek 尚无公开视觉） | DeepSeek 官方确认 deepseek-flash 视觉走**同一 chat completions 兼容层**（api-docs/guides/vision，2026-10 核）——§3.3 备注随更 | D2「输入侧单线可通」从推断升级为厂商实证；也坐实多模态稿 §8.1「图片必须投影到 user 消息」（DeepSeek 对非 user 消息带图直接 400） |
| 11 | （v1.0 按旧文档假设 deepseek-chat/reasoner 双模型分档、思考不可控） | **全量核 DeepSeek 官方文档**（thinking_mode / multi_round_chat / chat_prefix_completion / pricing / updates / kv_cache）：①现役 deepseek-flash/v4-pro（1M 窗口），旧名 2026-07-24 停用；②混合推理思考**可关可调档**（`thinking.type` + `reasoning_effort` low/high/max）→ 新方言 `deepseek_thinking`；③带 tools 必须回传 reasoning_content（§5.5 新增，走查核实）；④KV 硬盘缓存自动启用、前缀匹配——与 M1 纪律同构零改动；⑤峰谷定价闲时半价（目录价随更）；⑥前缀续写/FIM 为 beta + 独立 base_url，记档不做 | 官方文档为唯一事实源；设计稿按 2026-10 现役 API 面貌重校 |

---

## 一、与现状的关系（先读这节再动码）

**v2.58 已交付的骨架（不重建，只补差量）：**

| v1.0 稿中的"设计" | 现状 |
|---|---|
| §三 三步配置流（选供应商 → 填 Key → 拉模型） | `ServiceChooser.vue` 瓦片格 + `ProviderSetupDialog.vue` + `ModelPanes.vue` 双栏编辑器已全在 |
| 拉取模型列表 | `llm/models.rs::list_models`（openai GET /models、anthropic 同形带 x-api-key、ollama /api/tags）+ `llm_list_models` / `llm_probe_models` 命令 |
| 测试连接 | `llm_probe_models`（600ms 防抖 + 序号防旧覆新 + 存量 key 回落，`use-model-probe.ts`） |
| 供应商卡片 / 拖拽排序 / 两步删除 / 复制 | `ServiceList.vue` 全套 |
| 每模型上下文窗口 / 别名 | `ProviderModel{id, alias, context_window}`（llm.rs:15-21） |
| 会话级模型覆盖 | `model_override.rs`（旁车 `model`+`model_provider`） |
| 缺省模型 / 记录员模型档 | `DefaultModelMenu.vue` + `AppLlmMemoryManager`（organize/client.rs） |

**v2.0 的差量 = 六块：** ①能力档案库（models.dev）②推理档位 + 方言 ③输出上限 + 预检钳制 ④连接面（头/扫描/逐模型测试/keyring/代理）⑤能力投影到 UI（徽章/门控）⑥与多模态配套的门控（见多模态设计 v2.0）。

---

## 二、设计目标

| 目标 | 含义 |
|---|---|
| **零配置** | 选供应商、填 Key、选模型，其余自动 |
| **能力自动** | 窗口/最大输出/模态/推理档位/价格从档案库自动填充，用户手改即钉死 |
| **方言即数据** | 推理档位 → 各家请求字段的映射是**数据**（预设表 + 目录），Rust 侧只有少量静态编码器 |
| **宁缺勿假** | 目录没有的模型：能力未知如实标注，不编造；档位梯子为空 = 隐藏选择器 |
| **单线纪律** | kind 只决定鉴权头 + 端点归一（v2.75 不动）；方言差异用"往同一 chat/completions body 上写字段"表达 |
| **密钥不裸奔** | keyring 优先，存量明文一次性迁移 |

### 非目标

- 不做模型微调、不做 API 计费统计（第一期）、不做模型间自动路由
- 不做 OAuth 订阅账号（记档缓办，见阶段计划裁定表）
- 不做多线协议（见修订摘要 #3）
- 不移植 ZCode 的 CEL 解释器 / 规则引擎 / 三层 overlay / 远程目录热更（Locowrk 无服务器，家法是本地 settings.json + 字段级缺省；静态映射表够用）

---

## 三、预置供应商清单（并集）

### 3.1 预设 schema（扩展现有 `lib/provider-presets.ts`）

```ts
export interface ProviderPreset {
  id: string;
  name: string;
  baseUrl: string;
  kind: PresetKind;            // "openai" | "anthropic" | "ollama"（不动，v2.75）
  aliases?: string[];
  docsUrl?: string;            // 新： 「如何获取 API Key ↗」
  dialect?: ReasoningDialect;  // 新： 推理档位方言（§5.1），缺省 = 无梯子
  local?: boolean;             // 新： 本地服务免 Key（ollama / lmstudio）
}
```

> `models_endpoint` 字段**不需要**：端点形态由 `llm/models.rs` 按 kind 分派（Google 兼容端点同吃 `GET /models`），与 v1.0 §2.4 的 `AuthType::Query` 一起退役。

### 3.2 方言枚举（数据选表，不是每家一套编码器）

```ts
type ReasoningDialect =
  | "openai_effort"    // reasoning_effort: "minimal|low|medium|high"
  | "anthropic_effort" // reasoning_effort（官方兼容层映射 thinking budget）
  | "glm_thinking"     // thinking: {type: "enabled"|"disabled"}
  | "qwen_enable"      // enable_thinking: true|false
  | "deepseek_thinking" // thinking:{type:enabled|disabled} + reasoning_effort:low|high|max 双字段（官方 thinking_mode 指南，2026-10 核）
  | "none";            // 无梯子：选择器隐藏，不发任何字段
```

Rust 侧 `llm/dialect.rs` 为每个 dialect 一个纯函数（body patch），单测锁形状。**新增一家方言 = 加一个枚举值 + 一个纯函数 + 预设数据一行**，不碰请求管线。

### 3.3 预设并集表（在现有 22 家之上新增 ★ 行；base_url 均为兼容端点）

| 供应商 | id | kind | dialect | 备注 |
|---|---|---|---|---|
| DeepSeek | deepseek | openai | deepseek_thinking | **现役模型 deepseek-flash / deepseek-v4-pro**（1M 窗口 / 384K 输出；混合推理：思考默认开、effort 缺省 high，`thinking:{type}` 可关；旧名 deepseek-chat/reasoner 已于 2026-07-24 停用）。flash 支持图片输入（同 base_url 的 chat completions content 数组，图片仅限 user 消息）；v4-pro 纯文本。峰谷定价（闲时半价）随目录价格数据一并展示 |
| 智谱 AI / Z.AI | zhipu / zai | openai | glm_thinking | GLM 系 `[disabled, enabled]` |
| 通义千问 | dashscope | openai | qwen_enable | qwen3 系 `[disabled, enabled]` |
| 月之暗面 / MiniMax / 硅基流动 / 火山方舟 / 小米 | … | openai | none→目录补 | 现有 |
| OpenAI | openai | openai | openai_effort | gpt-5/o 系 `[minimal,low,medium,high]` |
| Anthropic | anthropic | anthropic | anthropic_effort | `[low,medium,high]`，兼容层映射**走查核实** |
| OpenRouter / Groq / xAI / Mistral / Together / Fireworks / NVIDIA / Baseten / Cerebras / HF | … | openai | none→目录补 | 现有 |
| Ollama 本地 | ollama-local | ollama | none | local |
| ★ Google Gemini | google | openai | none→目录补 | `https://generativelanguage.googleapis.com/v1beta/openai` |
| ★ 百川 | baichuan | openai | none | `https://api.baichuan-ai.com/v1` |
| ★ 零一万物 | yi | openai | none | `https://api.lingyiwanwu.com/v1` |
| ★ 阶跃星辰 | stepfun | openai | none→目录补 | `https://api.stepfun.com/v1` |
| ★ LM Studio | lmstudio | openai | none | `http://localhost:1234/v1`，local |
| Cohere / vLLM | — | — | — | **不预设**：Cohere 兼容层形态存疑、vLLM=自定义端点天然覆盖（宁缺勿假） |

---

## 四、模型能力档案库（models.dev 快照）

### 4.1 三条来源（保留 v1.0 §4.1 骨架，换实现）

| 来源 | 实现 | 覆盖 |
|---|---|---|
| **内置快照** | `desktop/src-tauri/resources/models.dev/api.json`（≈5.3MB，tauri.conf.json `bundle.resources` 打包，**惰性加载**不进启动路径） | 主流全量，含价格/窗口/模态/推理档/工具调用 |
| **在线刷新** | 设置页「刷新模型目录」按钮 → `GET https://models.dev/api.json`（10s 超时），**结果只进内存、永不落盘**（PI 同款）；无网络/失败回落快照 | 最新修正 |
| **用户钉死** | 用户在 ModelPanes 手改过的字段标 `source:"user"`，刷新与快照永不覆盖 | 用户意志最高 |

**合并优先级**：用户钉死 > 在线刷新 > 内置快照 > 预设基线 > 未知。

### 4.2 目录投影（只留需要的字段，加载时一次裁剪）

```rust
pub struct CatalogModel {
    pub provider_key: String,     // models.dev 供应商键（deepseek/zhipu/anthropic/…）
    pub model_id: String,
    pub display_name: Option<String>,
    pub context_window: Option<u64>,
    pub max_output: Option<u64>,
    pub input_modalities: Vec<String>,   // ["text","image",…]
    pub reasoning_options: Vec<String>,  // 目录原始档位（归一化后）
    pub tool_call: Option<bool>,
    pub pricing: Option<CatalogPricing>, // input/output/cache_read per Mtok（USD 原值）
}
```

### 4.3 匹配算法（取 PI 打分制，弃 v1.0 的正则层）

```
归一化：剥 -latest / 日期戳(2024-08-06|20241022) / @region 后缀；小写；取最后一个 '/' 后段
供应商定位：预设别名表命中 → 该 models.dev 供应商为首选（+100）；
           否则 base_url host 精确/后缀命中（+80）
模型匹配： 归一化 id 精确命中（+20）；供应商分最高者胜
无命中 → None（能力未知，如实标注，不猜）
```

**跨发布商"借池"不做**（记档）：中转站上的同名模型借官方元数据，初版宁缺勿假。

### 4.4 预设基线（ZCode 思想：无目录数据时的缺省基线，窗口与能力分开处理）

| 维度 | 基线 |
|---|---|
| 上下文窗口 | 未知 = 沿用现行保守回落（32k 保护语义），**不冒充 200k** |
| 最大输出 | 未知 = 不展示、不发送 |
| 模态 | 未知 = 纯文本；用户可手改（钉死） |
| 推理梯子 | 预设 `dialect` 决定档位集；目录 `reasoning_options` 可细化；两者皆无 = 隐藏选择器 |

### 4.5 命令与 UI

- `llm_catalog_status` → `{source: bundled|refreshed, updated, models_count}`（`LlmSection.vue:9` 的文档占位行转正为真状态行 + 刷新按钮）
- `llm_catalog_refresh` → 在线刷新（见 4.1）
- `llm_lookup_model {kind, base_url, model}` → 单模型目录查询（**自定义模型添加时就地升级**：用户手输 id 落库后查快照补元数据，id 保持原样；快照无 = 保持泛型种子）
- `ProviderModelView`（IPC 投影）扩展：

```rust
pub struct ProviderModelView {
    pub id: String,
    pub alias: Option<String>,
    pub context_window: Option<u64>,
    // ── 新增（目录 ∪ 用户钉死的合成视图）──
    pub max_output: Option<u64>,
    pub input_modalities: Vec<String>,
    pub reasoning_ladder: Vec<String>,   // 归一化档位（空 = 隐藏选择器）
    pub tool_call: Option<bool>,
    pub pricing: Option<PricingView>,
    pub sources: SourcesView,            // {window: catalog|user, …} 供「跟随 models.dev，编辑即钉死」提示
}
```

### 4.6 价格展示（P3，随批 A 数据顺带）

- 选择器/详情浮层展示 `每百万` 单价（目录 USD 原值，货币符号如实标注，不做汇率换算）
- 与现有 `chat.cacheHitPrice/cacheMissPrice`（会话累计估算，CNY 手填）**互不替换**：目录价是"这只模型贵不贵"的参考，会话价是"这个会话花了多少"的估算——两套口径并存，UI 分行展示

---

## 五、推理档位（thinking ladder）

### 5.1 数据模型

```
档位梯子 = 预设 dialect 的档位集 ∪ 目录 reasoning_options（归一化）
           （两者皆无 → 梯子空 → 选择器隐藏）
语义档位：disabled | minimal | low | medium | high | auto
         （"auto" 只读型：保留给未来"思考不可关"的模型；梯子=["auto"]，
           选择器显示"自动"标签不可调。DeepSeek V3.1 起混合推理思考可关，
           不再需要 auto）
方言写入 = dialect.rs 按 (dialect, 档位) 生成 body patch：
  openai_effort    → {"reasoning_effort": "low"}
  glm_thinking     → {"thinking": {"type": "enabled"}}
  qwen_enable      → {"enable_thinking": true}
  deepseek_thinking → disabled → {"thinking":{"type":"disabled"}}
                      low/high/max → {"thinking":{"type":"enabled"},
                                      "reasoning_effort":"<档>"}
                      （官方归一表：minimal→low、medium/high→high、
                        xhigh/ultra→max；思考模式忽略 temperature/
                        presence/frequency_penalty——我们不发，无冲突）
  anthropic_effort → {"reasoning_effort": "low"}（兼容层映射，走查核实）
  none → 不写
```

**映射不了的组合宁缺勿假**：目录档位值在方言表里没有对应写法 → 该档位不渲染（记日志一行），绝不发明字段。

### 5.2 会话级持久化与解析链（补 v1.0 缺口）

```
解析链（每请求，AppLlm::effective_cfg 扩展）：
  thinking_level = 会话旁车 thinking_level（有效时）
                   → clamp 到当前模型梯子（就近降档；"auto" 型恒 auto）
                   → 梯子空 = 不写 params
持久化： <id>.meta.json +thinking_level（ModelOverrideStore 随迁，
        session_set_model 命令扩展 or 新 session_set_thinking）
传递：   LlmRequest.params["thinking_level"] = 语义档位（内核只透传，
        不新增 LlmRequest 字段——12 处构造点零改动）
落线：   wire.rs build_payload 读 params.thinking_level + cfg（kind/preset/model）
        → dialect.rs 写字段
切换模型：前端 clamp（档位不在新梯子 → 就近降档或剥除，不猜）
新选择：  默认落梯子最高档（ZCode 同款）
```

### 5.3 UI

- Composer：`ModelSelector.vue` chip 旁新增**档位循环钮**（ZCode Ctrl+T 同款交互：点击按梯子序循环 + tooltip 显示当前档；是否换 PI 式滑杆见裁定表 D3）
- ModelPanes 高级区：每模型展示梯子（只读，来自目录/预设；用户自编辑映射记档缓办）
- 模型菜单行：推理档徽章（✨）与视觉徽章（👁）并排

### 5.4 辅助调用自动降档（ZCode 实证，纯节省）

| 调用 | 现状 | 批 B 起 |
|---|---|---|
| autoTitle 自动标题 | 主模型全速档 | 梯子最低档（disabled/minimal） |
| 记录员（memory manager） | 主模型 | 同上 |
| handoff 收束更新 | 主模型 | 同上 |
| 提取/嵌入 | 不走 chat | 不变 |

边界：混合推理模型（DeepSeek V3.1+、GLM、Claude）思考均可关/可降，降档直达 disabled；真正的"auto 不可关"型当前主流预设中已无实例（档位语义保留）。辅助调用**不发送 max_tokens**（宁缺勿假；ZCode 发 min(5000,max) 的做法记档不跟）。

### 5.5 DeepSeek 思考回传要求（带 tools 必须 400，批 B 走查核实）

官方 thinking_mode 指南：**带工具调用时，历史所有轮次的 `reasoning_content` 必须完整回传**（即使该轮未实际调用工具），否则 400；不带 tools 时传入也会被忽略。Locowrk 现状：`SessionEvent::Thinking` 已逐 step 落盘（v2.57）但投影层按"过程非上下文"跳过、wire 从不回传——**与该要求冲突**。矛盾证据：2026-10-07 M5/M6 实机走查（deepseek 真轮 + 工具，未回传）未见 400，疑似该要求随 V4.1-Flash（2026-09-10）收紧或实际执行宽松。处置：

1. 批 B 走查第一项 = deepseek-flash 真轮带工具复测（不回传是否 400）
2. 若成立：wire.rs 消息映射增加 reasoning 回传——配对来源现成（`Thinking` 事件紧贴 `AssistantMessage` 之前，投影时按相邻关系配对），随 assistant 消息发 `reasoning_content`；前缀纪律不破（历史 reasoning 是稳定追加字节）
3. PI 同款问题的先例：中转站严格时以 `[reasoning not retained]` 占位回传（其 zhipu/DeepSeek compat 旗标）——回退方案记档

---

## 六、输出上限与预检钳制

```
ProviderModel +max_output_tokens: Option<u64>   （用户钉死值，字段级缺省零迁移）
                 +max_output_source: Option<"user">

发送规则（wire.rs）：
  用户钉死值存在 → 发送 max_tokens = min(钉死值, ctx_window − estimate_tokens(req) − 1000)
                   钳制下限 1024（估用已超窗时不钳到负数——由上下文溢出语义接管）
  未钉死 / 仅目录有 → 不发送（v2.75 纪律不变：目录值只展示）
会话/辅助调用同吃此规则；参数里已有显式 max_tokens（决策环改写）时不重复钳
```

流式修复链保留：400 报文点名 max_tokens 相关字段 → 沿用现有 stream_options 同款"立即剥字段重试"先例（client.rs:156-161）扩展一个分支。

---

## 七、发送前能力门控（多模态的守门员）

```
时机：wire.rs 组装 payload 后、发出前
检查：请求 content parts 的模态 ⊆ 模型 input_modalities（目录 ∪ 用户钉死）
不通过 → LlmError::UnsupportedModality { modality, hint: "当前模型不支持图像，请切换到支持视觉的模型" }
       → 前端如实渲染错误条（附「打开模型选择器」动作）
```

上传时检查在前端（多模态设计 §9，吃同一份 `ProviderModelView` 投影）；发送前门控是第二道防线（会话中途换模型、历史重放等路径兜底）。ZCode 同款双道（`model.ts:155-185`）。

---

## 八、连接面

### 8.1 自定义请求头（PI 同款）

- `ProviderProfile +headers: BTreeMap<String,String>`（serde default，字段级零迁移）
- 保留键拒收（保存时校验）：`authorization, x-api-key, api-key, proxy-authorization, host, content-type, content-length, cookie, set-cookie, connection, transfer-encoding, te, trailer, upgrade`；上限 32 条
- 注入：`client.rs` 默认头之后**最后写入者**合并（用户头可覆盖 User-Agent 等，不可碰鉴权/逐跳头）
- UI：ProviderSetupDialog 高级区 key/value 行编辑器 + 常用头预设 + JSON 导入/复制

### 8.2 端点扫描探测（PI 同款，补中转站场景）

- 触发：`llm_list_models`/probe 对未知 base_url 失败时
- 候选（≤4，去重，每个 3s 超时）：现值（join_version 归一后）→ `{origin}/v1` → `{origin}/api/v1` → `{origin}/api`
- 胜出候选随探测结果回报 `effective_base_url`；表单在**字段未再编辑**时自动采纳 + 「已自动补全：{url}」提示

### 8.3 逐模型连通测试（ZCode 同款）

- `llm_test_model {profile_id, model}`：最小请求（`[{role:"user",content:"ping"}]`，非流式，无工具，无 max_tokens，15s 超时）
- 错误五分类（前端如实渲染）：`auth`(401/403) / `model_not_found`(404+模型名报文) / `rate_limit`(429) / `network`(超时/DNS/连接) / `server`(5xx)；其余 unknown 带截断报文
- 落点：ModelPanes 每行「测试」钮 + 结果徽章（现 `llm_probe_models` 保留作"拉列表"语义，两者并存）

### 8.4 API Key 加密（keyring）

```
crate：keyring（service="Locowrk", account=provider_id；Windows=Credential Manager）
schema：ProviderProfile +key_storage: Option<String>（缺省缺字段 = "file"，旧文件零迁移）
写入：  key 非空保存 → keyring.set → api_key=None + key_storage="keyring"
清除：  keyring.delete + 字段清空
读取：  api_key（存量明文）→ 否则 key_storage=="keyring" 时 keyring.get
迁移：  settings 加载时惰性迁移：读得明文 key → 写 keyring → 字段换标记（幂等）
回落：  keyring 不可用（Linux 无 libsecret / 便携环境）→ 回落文件存储 + 日志 warn（记档）
IPC：   现状不变——api_key 永不过线，只露 api_key_set: bool
```

### 8.5 网络代理（裁定表 D6）

- `llm +network_proxy: {mode: "system"|"direct"|"custom", url: Option<String>}`（缺省 system = reqwest 现状吃环境变量）
- custom → `reqwest::Proxy::all(url)`；设置保存即热换（client builder 重建）
- 动机：本机常驻 clash 类代理时显式可控（v2.45 走查记录）

---

## 九、数据存储（settings.json `llm` 区扩展）

> v1.0 §7.1 的 `providers.toml` 退役（修订摘要 #2）。全部新增字段 serde 缺省 + 旧文件零迁移。

```rust
// settings/llm.rs 增量
pub struct ProviderProfile {
    // …现有字段不动…
    pub headers: BTreeMap<String, String>,      // 新 §8.1
    pub key_storage: Option<String>,            // 新 §8.4
    pub models: Vec<ProviderModel>,
}
pub struct ProviderModel {
    // …现有 id/alias/context_window 不动…
    pub max_output_tokens: Option<u64>,          // 新 §6（用户钉死）
    pub max_output_source: Option<String>,       // 新（"user" 钉死标记）
    pub supports_image: Option<bool>,            // 新（tri-state：None=随目录）
    pub thinking_level_default: Option<String>,  // 新（预留，初版不用）
}
pub struct LlmConfig {
    // …active / providers 不动…
    pub network_proxy: Option<NetworkProxy>,     // 新 §8.5
}
```

目录投影（CatalogModel 等）**只进内存**，不落 settings；`models.dev` 快照是随包资源不是用户数据。

---

## 十、裁定记录（动码前确认）

| # | 裁定点 | 推荐 | 状态 |
|---|---|---|---|
| D1 | 档案库源：models.dev 快照 vs 手维护 20 模型 | **快照**（免维护、双家未走手维护路线） | 待裁 |
| D2 | 单线纪律维持：多模态输入侧走兼容层；原生 thinking budget / 图像输出 / 原生 PDF 另批 | **维持** | 待裁 |
| D3 | thinking 控件：循环钮（ZCode/Ctrl+T 式）vs 滑杆（PI 式） | **循环钮**（贴现有 composer 风格） | 待裁 |
| D5 | max_tokens 窄修订：用户钉死值才发送 + 预检钳制 | **采纳**（ZCode/PI 双证；目录值仍不自动发） | 待裁 |
| D6 | 网络代理设置进批 C | **进**（clash 用户实际收益） | 待裁 |
| D7 | keyring 方案确认（Windows Credential Manager 优先） | **确认**（v1.0 已选，v2.0 补迁移细节） | 待裁 |
| 缓办 | OAuth 订阅账号 / 目录借池 / 用户自编辑档位映射 / aux 调用发 max_tokens | 记档 | 已记 |

---

## 十一、常见坑（v2.0 更新）

| 坑 | 原因 | 解法 |
|---|---|---|
| 模型 ID 带日期后缀匹配不上 | 键名太精确 | 归一化剥后缀（§4.3），不做正则 |
| Provider API 只回模型 ID | 绝大多数如此 | 目录快照兜底 + 能力未知如实标注 |
| 目录价与本地中转价不符 | 中转站加价 | 价格只作参考展示，不进任何计算 |
| 中转站模型借不到官方元数据 | 借池未做 | 宁缺勿假，能力未知可手改钉死 |
| 推理档位字段各家不同 | 方言碎片化 | dialect 数据选表 + 静态编码器；映射不了不发 |
| anthropic_effort 兼容层行为未证实 | 官方兼容层文档有限 | 批 B 实机走查核实，不实则该预设梯子降级 none |
| max_tokens 触发不透明 400 | 网关兼容性 | 只在用户钉死时发 + 400 剥字段重试先例扩展 |
| keyring 在无密钥链环境崩 | Linux 无 libsecret | 读写全程回落文件 + warn（§8.4） |
| 端点扫描把凭据发去错误主机 | 盲试 URL | 候选只变路径不换 host；超时短 |
| 上下文窗口估错导致溢出 | 目录缺数据 | 保守回落 32k 不变 + 发送前钳制公式兜底 |

---

## 十二、一句话

**用户只做三件事：选哪家、填 Key、选模型。其余自动——而"自动"的燃料是 models.dev 快照，"各家不同"的解法是方言数据表，"安全"的底线是单线纪律与 keyring。**

---

*文档版本：v2.0（2026-10-07）*
*配套项目：Locowrk*
*关联文档：《LLM 客户端与调度层设计》《多模态支持设计 v2.0》《Model Capability and Multimodal Phase Plan》*
